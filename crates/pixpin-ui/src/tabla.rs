//! La rejilla de una hoja de calculo: donde cae cada celda y cada rotulo.
//!
//! Geometria pura, como el resto del crate. Aqui NO se sabe que hay escrito
//! en la tabla: las celdas son `(columna, fila)` en `u32` desde cero, y quien
//! pinta pregunta el contenido a `pixpin_proyecto::tabla`. Mantenerlo asi
//! deja probar la rejilla entera sin construir una tabla, y evita que la capa
//! de interfaz arrastre el dominio.
//!
//! Tres decisiones que no son evidentes:
//!
//! - **Las celdas no tienen hueco entre ellas.** Una hoja de calculo se lee
//!   por sus lineas de rejilla, que son de las dos celdas vecinas a la vez;
//!   dejar aire entre ellas la convertiria en una cuadricula de fichas.
//! - **El hueco pequeno no invierte nada.** Si la ventana no da ni para las
//!   cabeceras, el area de celdas se queda en cero en vez de salir del reves.
//!   Todas las restas van por `saturating_sub` o se recortan con `min`: en
//!   `u32` un rectangulo del reves no es un dibujo raro, es un desbordamiento.
//! - **El desplazamiento no deja la rejilla flotando.** Los topes se calculan
//!   contra lo que ocupan las columnas y filas que hay: si caben enteras el
//!   tope es cero, y nunca se puede empujar el final de la hoja por encima
//!   del borde del area de celdas dejando un vacio debajo.

use pixpin_geom::{Punto, Rect};

/// Medidas en pixeles logicos (al 100 %).
/// El alto de una fila y el ancho de una columna sin tocar.
pub const FILA_ALTO: u32 = 24;
pub const COLUMNA_ANCHO: u32 = 88;
/// La cabecera de la izquierda, donde van los numeros de fila (1, 2, 3...).
pub const CABECERA_FILAS_ANCHO: u32 = 44;
/// La cabecera de arriba, donde van las letras de columna (A, B, C...).
pub const CABECERA_COLUMNAS_ALTO: u32 = 24;
/// La barra de formulas, encima de todo.
pub const BARRA_FORMULAS_ALTO: u32 = 32;
pub const BARRA_FORMULAS_X: u32 = 10;
pub const BARRA_FORMULAS_TAM: f32 = 13.0;
/// Los rotulos de las cabeceras: un punto mas pequenos que el contenido,
/// porque son referencia y no dato.
pub const CABECERA_TAM: f32 = 12.0;
pub const CELDA_TAM: f32 = 13.0;
/// El aire a cada lado del texto de una celda, para que no toque la linea.
pub const CELDA_RELLENO: u32 = 5;

/// Como queda repartida la hoja dentro de su hueco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposicion {
    /// Todo el hueco de la hoja.
    pub hoja: Rect,
    /// La barra de formulas, arriba del todo y a todo lo ancho.
    pub barra_formulas: Rect,
    /// El cuadradito muerto donde se cruzan las dos cabeceras.
    pub esquina: Rect,
    /// La tira de letras, a la derecha de la esquina.
    pub cabecera_columnas: Rect,
    /// La tira de numeros, debajo de la esquina.
    pub cabecera_filas: Rect,
    /// Lo que queda: donde se pintan las celdas y lo unico que se desplaza.
    pub celdas: Rect,
}

impl Disposicion {
    /// Reparte `hueco` en barra de formulas, cabeceras y area de celdas.
    ///
    /// Las cabeceras se recortan a lo que haya: en un hueco mas pequeno que
    /// ellas se quedan con lo que queda y el area de celdas vale cero, que se
    /// pinta como nada. La alternativa —restar a pelo— daria un ancho enorme
    /// por desbordamiento de `u32` y pintaria la hoja fuera de la ventana.
    pub fn calcular(hueco: Rect, escala_por_cien: u32) -> Disposicion {
        let e = |v: u32| v * escala_por_cien / 100;

        let barra_formulas = Rect {
            alto: e(BARRA_FORMULAS_ALTO).min(hueco.alto),
            ..hueco
        };
        let resto_alto = hueco.alto - barra_formulas.alto;
        let esquina = Rect {
            x: hueco.x,
            y: barra_formulas.abajo(),
            ancho: e(CABECERA_FILAS_ANCHO).min(hueco.ancho),
            alto: e(CABECERA_COLUMNAS_ALTO).min(resto_alto),
        };
        let cabecera_columnas = Rect {
            x: esquina.derecha(),
            y: esquina.y,
            ancho: hueco.ancho - esquina.ancho,
            alto: esquina.alto,
        };
        let cabecera_filas = Rect {
            x: hueco.x,
            y: esquina.abajo(),
            ancho: esquina.ancho,
            alto: resto_alto - esquina.alto,
        };
        Disposicion {
            hoja: hueco,
            barra_formulas,
            esquina,
            cabecera_columnas,
            cabecera_filas,
            celdas: Rect {
                x: cabecera_columnas.x,
                y: cabecera_filas.y,
                ancho: cabecera_columnas.ancho,
                alto: cabecera_filas.alto,
            },
        }
    }

    /// Donde cae una celda, con el desplazamiento ya restado.
    ///
    /// Puede salir fuera del area: recortar es cosa de quien pinta, que es
    /// quien sabe si esta dibujando texto o un recuadro de seleccion.
    pub fn celda(
        &self,
        columna: u32,
        fila: u32,
        scroll_x: i32,
        scroll_y: i32,
        escala_por_cien: u32,
    ) -> Rect {
        let (paso_x, paso_y) = pasos(escala_por_cien);
        Rect {
            x: self.celdas.x + (columna * paso_x) as i32 - scroll_x,
            y: self.celdas.y + (fila * paso_y) as i32 - scroll_y,
            ancho: paso_x,
            alto: paso_y,
        }
    }

    /// El recuadro donde va la letra de una columna, dentro de su cabecera.
    pub fn cabecera_de_columna(&self, columna: u32, scroll_x: i32, escala_por_cien: u32) -> Rect {
        let (paso_x, _) = pasos(escala_por_cien);
        Rect {
            x: self.cabecera_columnas.x + (columna * paso_x) as i32 - scroll_x,
            y: self.cabecera_columnas.y,
            ancho: paso_x,
            alto: self.cabecera_columnas.alto,
        }
    }

    /// El recuadro donde va el numero de una fila, dentro de su cabecera.
    pub fn cabecera_de_fila(&self, fila: u32, scroll_y: i32, escala_por_cien: u32) -> Rect {
        let (_, paso_y) = pasos(escala_por_cien);
        Rect {
            x: self.cabecera_filas.x,
            y: self.cabecera_filas.y + (fila * paso_y) as i32 - scroll_y,
            ancho: self.cabecera_filas.ancho,
            alto: paso_y,
        }
    }

    /// Donde va el texto dentro de una celda, y cuanto sitio tiene.
    pub fn texto_celda(&self, celda: Rect, escala_por_cien: u32) -> (i32, u32) {
        let relleno = CELDA_RELLENO * escala_por_cien / 100;
        (
            celda.x + relleno as i32,
            celda.ancho.saturating_sub(2 * relleno),
        )
    }

    /// Que trozo de la hoja se ve: `(primera_columna, cuantas_columnas,
    /// primera_fila, cuantas_filas)`.
    ///
    /// Va con una de margen por cada lado, como la cuadricula de `info`: asi
    /// la celda que asoma a medias por el borde ya esta pintada antes de que
    /// el desplazamiento la descubra, y no se ve entrar en blanco.
    pub fn visibles(
        &self,
        scroll_x: i32,
        scroll_y: i32,
        escala_por_cien: u32,
    ) -> (u32, u32, u32, u32) {
        let (paso_x, paso_y) = pasos(escala_por_cien);
        let primera_columna = (scroll_x.max(0) as u32 / paso_x).saturating_sub(1);
        let primera_fila = (scroll_y.max(0) as u32 / paso_y).saturating_sub(1);
        // Dos de mas por los dos margenes y una tercera porque la primera
        // suele estar cortada por arriba.
        let columnas = self.celdas.ancho / paso_x + 3;
        let filas = self.celdas.alto / paso_y + 3;
        (primera_columna, columnas, primera_fila, filas)
    }

    /// Que celda hay bajo el punto.
    ///
    /// `None` en las cabeceras, en la barra de formulas y fuera de la hoja:
    /// pulsar una cabecera no es pulsar su primera celda, que es lo que
    /// pasaria si aqui se dejara colar.
    pub fn celda_en(
        &self,
        p: Punto,
        scroll_x: i32,
        scroll_y: i32,
        escala_por_cien: u32,
    ) -> Option<(u32, u32)> {
        if !self.celdas.contiene(p) {
            return None;
        }
        let (paso_x, paso_y) = pasos(escala_por_cien);
        let dentro_x = p.x - self.celdas.x + scroll_x;
        let dentro_y = p.y - self.celdas.y + scroll_y;
        // Con el desplazamiento en negativo la rejilla queda corrida a la
        // derecha y a su izquierda no hay celda ninguna, aunque el punto si
        // este dentro del area.
        if dentro_x < 0 || dentro_y < 0 {
            return None;
        }
        Some((dentro_x as u32 / paso_x, dentro_y as u32 / paso_y))
    }

    /// Hasta donde se puede desplazar una hoja de `columnas` por `filas`.
    ///
    /// Si lo que hay cabe entero en el area, el tope es cero: no tiene
    /// sentido poder empujar la hoja hasta dejarla flotando sobre un hueco.
    pub fn topes(&self, columnas: u32, filas: u32, escala_por_cien: u32) -> (i32, i32) {
        let (paso_x, paso_y) = pasos(escala_por_cien);
        (
            (columnas * paso_x).saturating_sub(self.celdas.ancho) as i32,
            (filas * paso_y).saturating_sub(self.celdas.alto) as i32,
        )
    }

    /// El desplazamiento que hace falta para que una celda se vea entera.
    ///
    /// Mueve lo MINIMO: si ya se ve, no toca nada. Es lo que se espera al
    /// andar con las flechas —la hoja no salta— y lo que hace cualquier hoja
    /// de calculo. No se sujeta a los topes a proposito: al escribir en una
    /// celda mas alla de lo escrito, la hoja tiene que poder seguirla, y esa
    /// celda todavia no cuenta para el tamano de la tabla.
    pub fn seguir(
        &self,
        columna: u32,
        fila: u32,
        scroll_x: i32,
        scroll_y: i32,
        escala_por_cien: u32,
    ) -> (i32, i32) {
        let (paso_x, paso_y) = pasos(escala_por_cien);
        let seguir_eje = |indice: u32, paso: u32, visible: u32, scroll: i32| {
            let inicio = (indice * paso) as i32;
            let fin = inicio + paso as i32;
            if inicio < scroll {
                inicio
            } else if fin > scroll + visible as i32 {
                // Que quede pegada al borde de abajo (o de la derecha).
                fin - visible as i32
            } else {
                scroll
            }
        };
        (
            seguir_eje(columna, paso_x, self.celdas.ancho, scroll_x).max(0),
            seguir_eje(fila, paso_y, self.celdas.alto, scroll_y).max(0),
        )
    }

    /// El desplazamiento, sujeto entre cero y su tope.
    pub fn sujetar(
        &self,
        scroll_x: i32,
        scroll_y: i32,
        columnas: u32,
        filas: u32,
        escala_por_cien: u32,
    ) -> (i32, i32) {
        let (tope_x, tope_y) = self.topes(columnas, filas, escala_por_cien);
        (scroll_x.clamp(0, tope_x), scroll_y.clamp(0, tope_y))
    }
}

/// Lo que mide una celda a esta escala, a lo ancho y a lo alto.
///
/// Nunca cero: con paso cero las divisiones de `visibles` y `celda_en`
/// reventarian, y una escala ridicula no es motivo para caerse.
fn pasos(escala_por_cien: u32) -> (u32, u32) {
    (
        (COLUMNA_ANCHO * escala_por_cien / 100).max(1),
        (FILA_ALTO * escala_por_cien / 100).max(1),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hueco() -> Rect {
        Rect {
            x: 40,
            y: 60,
            ancho: 800,
            alto: 500,
        }
    }

    #[test]
    fn la_hoja_sigue_a_la_celda_elegida_solo_cuando_hace_falta() {
        let d = Disposicion::calcular(hueco(), 100);
        // Una celda que ya se ve no mueve nada: andar con las flechas por el
        // trozo visible no puede hacer saltar la hoja.
        assert_eq!(d.seguir(1, 1, 0, 0, 100), (0, 0));

        // Una muy a la derecha se trae pegada al borde derecho.
        let (x, _) = d.seguir(40, 0, 0, 0, 100);
        assert_eq!(x, (41 * COLUMNA_ANCHO) as i32 - d.celdas.ancho as i32);

        // Y al volver hacia atras, pegada al borde izquierdo y sin pasarse
        // de cero: un desplazamiento negativo dejaria la hoja flotando.
        assert_eq!(d.seguir(0, 0, 900, 0, 100), (0, 0));
    }

    #[test]
    fn las_piezas_de_la_hoja_van_en_orden_y_llenan_el_hueco() {
        let d = Disposicion::calcular(hueco(), 100);
        assert_eq!(d.barra_formulas.y, hueco().y);
        assert_eq!(d.barra_formulas.alto, BARRA_FORMULAS_ALTO);
        // De arriba abajo: barra, cabecera de columnas, celdas.
        assert_eq!(d.esquina.y, d.barra_formulas.abajo());
        assert_eq!(d.cabecera_columnas.y, d.esquina.y);
        assert_eq!(d.celdas.y, d.cabecera_columnas.abajo());
        assert_eq!(d.celdas.abajo(), hueco().abajo());
        // Y de izquierda a derecha: cabecera de filas, celdas.
        assert_eq!(d.cabecera_filas.x, hueco().x);
        assert_eq!(d.celdas.x, d.cabecera_filas.derecha());
        assert_eq!(d.celdas.derecha(), hueco().derecha());
        // La esquina es el cruce de las dos cabeceras, sin solapar ninguna.
        assert_eq!(d.esquina.ancho, CABECERA_FILAS_ANCHO);
        assert_eq!(d.esquina.alto, CABECERA_COLUMNAS_ALTO);
        assert_eq!(d.cabecera_filas.ancho, d.esquina.ancho);
        assert_eq!(d.cabecera_columnas.alto, d.esquina.alto);
    }

    #[test]
    fn en_un_hueco_mas_pequeno_que_las_cabeceras_no_sale_nada_del_reves() {
        // Caso negativo: el hueco no da ni para la barra de formulas. Todo se
        // recorta a cero en vez de desbordar la resta en u32, que pintaria un
        // rectangulo de cuatro mil millones de ancho.
        let minusculo = Rect {
            x: 0,
            y: 0,
            ancho: 20,
            alto: 10,
        };
        let d = Disposicion::calcular(minusculo, 100);
        assert_eq!(d.barra_formulas.alto, 10, "se queda con lo que hay");
        assert_eq!(d.celdas.ancho, 0);
        assert_eq!(d.celdas.alto, 0);
        for r in [
            d.barra_formulas,
            d.esquina,
            d.cabecera_columnas,
            d.cabecera_filas,
            d.celdas,
        ] {
            assert!(r.derecha() <= minusculo.derecha(), "{r:?}");
            assert!(r.abajo() <= minusculo.abajo(), "{r:?}");
        }
        // Y sin area de celdas no hay celda que pulsar.
        assert_eq!(d.celda_en(Punto { x: 5, y: 5 }, 0, 0, 100), None);
    }

    #[test]
    fn las_celdas_van_pegadas_y_en_linea_con_sus_cabeceras() {
        let d = Disposicion::calcular(hueco(), 100);
        let a1 = d.celda(0, 0, 0, 0, 100);
        assert_eq!((a1.x, a1.y), (d.celdas.x, d.celdas.y));
        assert_eq!((a1.ancho, a1.alto), (COLUMNA_ANCHO, FILA_ALTO));
        // Pegadas: la rejilla se lee por sus lineas, no por el aire.
        let b1 = d.celda(1, 0, 0, 0, 100);
        assert_eq!(b1.x, a1.derecha());
        assert_eq!(b1.y, a1.y);
        let a2 = d.celda(0, 1, 0, 0, 100);
        assert_eq!(a2.y, a1.abajo());
        assert_eq!(a2.x, a1.x);
        // Cada rotulo, sobre su columna y a la izquierda de su fila.
        assert_eq!(d.cabecera_de_columna(1, 0, 100).x, b1.x);
        assert_eq!(d.cabecera_de_columna(1, 0, 100).abajo(), d.celdas.y);
        assert_eq!(d.cabecera_de_fila(1, 0, 100).y, a2.y);
        assert_eq!(d.cabecera_de_fila(1, 0, 100).derecha(), d.celdas.x);
    }

    #[test]
    fn al_desplazarse_las_celdas_y_sus_cabeceras_se_mueven_juntas() {
        let d = Disposicion::calcular(hueco(), 100);
        let (sx, sy) = (COLUMNA_ANCHO as i32 * 2, FILA_ALTO as i32 * 5);
        let c = d.celda(4, 9, sx, sy, 100);
        // La columna cuarta con dos de desplazamiento cae donde la segunda.
        assert_eq!(c.x, d.celda(2, 0, 0, 0, 100).x);
        assert_eq!(c.y, d.celda(0, 4, 0, 0, 100).y);
        // Y los rotulos no se quedan atras.
        assert_eq!(d.cabecera_de_columna(4, sx, 100).x, c.x);
        assert_eq!(d.cabecera_de_fila(9, sy, 100).y, c.y);
    }

    #[test]
    fn solo_se_pintan_las_celdas_que_se_ven_mas_una_de_margen() {
        let d = Disposicion::calcular(hueco(), 100);
        let (pc, cc, pf, cf) = d.visibles(0, 0, 100);
        assert_eq!((pc, pf), (0, 0), "arriba del todo no hay margen que dar");
        // Ni una hoja enorme obliga a pintar mas de lo que cabe y su aire.
        assert!(cc <= d.celdas.ancho / COLUMNA_ANCHO + 3, "{cc}");
        assert!(cf <= d.celdas.alto / FILA_ALTO + 3, "{cf}");
        assert!(cc * COLUMNA_ANCHO >= d.celdas.ancho);
        assert!(cf * FILA_ALTO >= d.celdas.alto);

        // Desplazada, se empieza una antes de la primera que asoma.
        let (pc, cc, pf, cf) = d.visibles(COLUMNA_ANCHO as i32 * 10, FILA_ALTO as i32 * 20, 100);
        assert_eq!((pc, pf), (9, 19));
        // Y lo pintado cubre de sobra lo que se ve.
        assert!((pc + cc) * COLUMNA_ANCHO >= 10 * COLUMNA_ANCHO + d.celdas.ancho);
        assert!((pf + cf) * FILA_ALTO >= 20 * FILA_ALTO + d.celdas.alto);
    }

    #[test]
    fn se_acierta_la_celda_bajo_el_raton_y_no_las_cabeceras() {
        let d = Disposicion::calcular(hueco(), 100);
        let dentro = |r: Rect| Punto {
            x: r.x + 3,
            y: r.y + 3,
        };
        assert_eq!(d.celda_en(dentro(d.celdas), 0, 0, 100), Some((0, 0)));
        let c = d.celda(3, 7, 0, 0, 100);
        assert_eq!(d.celda_en(dentro(c), 0, 0, 100), Some((3, 7)));
        // Con desplazamiento, el mismo punto es otra celda.
        let (sx, sy) = (COLUMNA_ANCHO as i32, FILA_ALTO as i32 * 4);
        assert_eq!(d.celda_en(dentro(c), sx, sy, 100), Some((4, 11)));

        // Casos negativos: las cabeceras, la barra y fuera de la hoja no son
        // ninguna celda.
        assert_eq!(d.celda_en(dentro(d.cabecera_columnas), 0, 0, 100), None);
        assert_eq!(d.celda_en(dentro(d.cabecera_filas), 0, 0, 100), None);
        assert_eq!(d.celda_en(dentro(d.esquina), 0, 0, 100), None);
        assert_eq!(d.celda_en(dentro(d.barra_formulas), 0, 0, 100), None);
        let fuera = Punto {
            x: d.hoja.derecha() + 10,
            y: d.celdas.y + 10,
        };
        assert_eq!(d.celda_en(fuera, 0, 0, 100), None);
    }

    #[test]
    fn el_desplazamiento_no_deja_la_rejilla_flotando() {
        let d = Disposicion::calcular(hueco(), 100);
        // Una hoja que cabe entera no se desplaza: tope cero.
        assert_eq!(d.topes(2, 3, 100), (0, 0));
        assert_eq!(d.sujetar(500, 500, 2, 3, 100), (0, 0));

        // Una grande se desplaza justo hasta que su ultima celda toca el
        // borde, y ni un pixel mas.
        let (tope_x, tope_y) = d.topes(100, 400, 100);
        assert_eq!(tope_x, (100 * COLUMNA_ANCHO - d.celdas.ancho) as i32);
        assert_eq!(tope_y, (400 * FILA_ALTO - d.celdas.alto) as i32);
        let ultima = d.celda(99, 399, tope_x, tope_y, 100);
        assert_eq!(ultima.derecha(), d.celdas.derecha());
        assert_eq!(ultima.abajo(), d.celdas.abajo());
        // Pasarse por arriba o por abajo se sujeta a los dos extremos.
        assert_eq!(d.sujetar(99_999, -50, 100, 400, 100), (tope_x, 0));
        assert_eq!(d.sujetar(-1, 99_999, 100, 400, 100), (0, tope_y));
    }

    #[test]
    fn el_texto_de_una_celda_deja_aire_y_no_toca_las_lineas() {
        let d = Disposicion::calcular(hueco(), 100);
        let c = d.celda(0, 0, 0, 0, 100);
        let (x, ancho) = d.texto_celda(c, 100);
        assert_eq!(x, c.x + CELDA_RELLENO as i32);
        assert_eq!(ancho, COLUMNA_ANCHO - 2 * CELDA_RELLENO);
        assert!(x + ancho as i32 <= c.derecha());
        // Caso negativo: en una celda mas estrecha que su relleno no queda
        // sitio, y sale cero en vez de un ancho al reves.
        let estrecha = Rect { ancho: 4, ..c };
        assert_eq!(d.texto_celda(estrecha, 100).1, 0);
    }

    #[test]
    fn todo_escala_con_el_dpi() {
        let d = Disposicion::calcular(hueco(), 200);
        assert_eq!(d.barra_formulas.alto, BARRA_FORMULAS_ALTO * 2);
        assert_eq!(d.cabecera_filas.ancho, CABECERA_FILAS_ANCHO * 2);
        let c = d.celda(1, 1, 0, 0, 200);
        assert_eq!((c.ancho, c.alto), (COLUMNA_ANCHO * 2, FILA_ALTO * 2));
        assert_eq!(c.x, d.celdas.x + (COLUMNA_ANCHO * 2) as i32);
        // Y una escala absurda no divide por cero.
        let d = Disposicion::calcular(hueco(), 1);
        assert!(d.celda_en(Punto { x: 100, y: 200 }, 0, 0, 1).is_some());
    }
}
