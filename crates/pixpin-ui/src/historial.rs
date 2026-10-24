//! El historial de un proyecto: los mensajes en burbujas.
//!
//! Geometria pura, como `chat`. Las medidas salen del mismo analisis de
//! Telegram Desktop; **de ahi solo medidas y tecnicas, nunca su codigo, que
//! es GPL-3.0.**
//!
//! Dos reglas que no son evidentes y conviene no deshacer:
//!
//! - **El historial se ancla abajo.** Lo ultimo es lo que importa, asi que
//!   al abrir un proyecto se empieza pegado al final. Anclarlo arriba haria
//!   que vieras el primer mensaje de hace dos anos.
//! - **Aqui no se mide texto.** Medir necesita las fuentes del sistema, que
//!   no son puras ni se pueden probar. Quien pinta mide y pasa el alto ya
//!   hecho en `Entrada::alto`; esto solo reparte.

use pixpin_geom::{Punto, Rect};

/// Medidas en pixeles logicos (al 100 %), las de Telegram Desktop.
///
/// El ancho de una burbuja tiene un tope DURO de 430, no una proporcion de
/// la columna: una linea de mas de eso se lee mal por ancha que sea la
/// ventana.
pub const BURBUJA_MAXIMA: u32 = 430;
/// Lo que se separa del borde de su lado, y lo que se le reserva al lado
/// contrario para que se vea de un vistazo de quien es cada mensaje.
pub const MARGEN: u32 = 16;
pub const MARGEN_CONTRARIO: u32 = 56;
/// El relleno de dentro de la burbuja (`msgPadding`).
pub const RELLENO_X: u32 = 11;
pub const RELLENO_Y: u32 = 8;
pub const RADIO: u32 = 16;
/// Entre dos burbujas seguidas del mismo lado, y al cambiar de lado.
pub const HUECO: u32 = 2;
pub const HUECO_GRUPO: u32 = 8;
/// El «rato»: dos mensajes del mismo lado dentro de esta ventana van
/// pegados. Cinco minutos, como PixPin Android (y como Telegram).
pub const RATO_MS: i64 = 5 * 60 * 1000;
/// La pildora con la fecha que separa los dias: 24 de alto, con 10 de aire
/// encima y 2 debajo, y 12 de relleno a cada lado.
pub const SEPARADOR_PILDORA: u32 = 24;
pub const SEPARADOR_MARGEN: u32 = 10;
pub const SEPARADOR_RELLENO_X: u32 = 12;
pub const SEPARADOR: u32 = SEPARADOR_MARGEN + SEPARADOR_PILDORA + 2;
pub const SEPARADOR_TAM: f32 = 13.0;
pub const TEXTO_TAM: f32 = 13.0;
pub const HORA_TAM: f32 = 13.0;
/// El hueco entre el final de la ultima linea y la hora (`msgDateSpace`).
pub const HORA_HUECO: u32 = 12;
/// Lo que la hora invade el relleno de la burbuja, a la derecha y abajo.
pub const HORA_INVADE_X: u32 = 2;
pub const HORA_INVADE_Y: u32 = 5;
/// La vista previa de un lienzo dentro de su burbuja. Cabe entera dentro de
/// esta caja, sin deformarse: un dibujo apaisado y uno alto se ensenan
/// igual de legibles.
pub const VISTA_ANCHO: u32 = 260;
pub const VISTA_ALTO: u32 = 180;

/// Un mensaje a colocar, ya medido por quien pinta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entrada {
    /// El alto del contenido, sin el relleno de la burbuja.
    pub alto: u32,
    /// El ancho del contenido, para que la burbuja no sea mas ancha de lo
    /// que ocupa: una de ancho fijo con dos palabras queda fatal.
    pub ancho: u32,
    /// Mio: va a la derecha. De otro aparato: a la izquierda.
    pub mio: bool,
    /// Cuando se escribio, en milisegundos. Decide si va pegado al de
    /// arriba: dos del mismo lado dentro del mismo «rato» se agrupan.
    pub cuando: i64,
    /// El dia (local) al que pertenece, para saber donde va un separador.
    pub dia: i64,
}

/// Donde acaba cada mensaje.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Puesto {
    pub burbuja: Rect,
    /// La pildora de fecha que va justo encima, si este mensaje estrena dia.
    pub separador: Option<Rect>,
}

impl Puesto {
    /// Donde va el contenido dentro de la burbuja.
    pub fn dentro(&self, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        Rect {
            x: self.burbuja.x + e(RELLENO_X) as i32,
            y: self.burbuja.y + e(RELLENO_Y) as i32,
            ancho: self.burbuja.ancho.saturating_sub(2 * e(RELLENO_X)),
            alto: self.burbuja.alto.saturating_sub(2 * e(RELLENO_Y)),
        }
    }

    /// Lo alto que ocupa con su pildora, si la lleva.
    pub fn arriba(&self) -> i32 {
        self.separador.map_or(self.burbuja.y, |s| s.y)
    }
}

/// Lo ancho que puede ser el CONTENIDO de una burbuja en esta columna. Es lo
/// que quien pinta usa para medir el texto antes de llamar a `colocar`.
pub fn ancho_contenido(area: Rect, escala_por_cien: u32) -> u32 {
    let e = |v: u32| v * escala_por_cien / 100;
    // Lo que queda quitando el margen del lado propio y el que se reserva al
    // contrario; y por ancha que sea la ventana, nunca mas de 430.
    let util = area.ancho.saturating_sub(e(MARGEN) + e(MARGEN_CONTRARIO));
    util.min(e(BURBUJA_MAXIMA)).saturating_sub(2 * e(RELLENO_X))
}

/// Reparte los mensajes de arriba abajo, del mas viejo al mas nuevo, con el
/// origen en el alto cero. El desplazamiento lo aplica quien pinta.
///
/// Devuelve tambien el alto total que ocupa todo junto.
pub fn colocar(area: Rect, entradas: &[Entrada], escala_por_cien: u32) -> (Vec<Puesto>, u32) {
    let e = |v: u32| v * escala_por_cien / 100;
    let mut puestos = Vec::with_capacity(entradas.len());
    let mut y = e(HUECO_GRUPO) as i32;
    let mut dia_anterior: Option<i64> = None;
    let mut mio_anterior: Option<bool> = None;
    let mut cuando_anterior: Option<i64> = None;
    let ancho_maximo = ancho_contenido(area, escala_por_cien) + 2 * e(RELLENO_X);

    for entrada in entradas {
        // Un dia nuevo estrena pildora de fecha; el primero tambien la lleva,
        // que si no el historial empieza sin decir de cuando es.
        let separador = if dia_anterior != Some(entrada.dia) {
            let r = Rect {
                x: area.x,
                y,
                ancho: area.ancho,
                alto: e(SEPARADOR),
            };
            y += e(SEPARADOR) as i32;
            Some(r)
        } else {
            // Dos del mismo lado y del mismo rato se pegan; si cambia el lado
            // o pasa mucho tiempo se separan mas, que es lo que hace que se
            // lea de un vistazo de quien es cada tanda.
            let mismo_grupo = mio_anterior == Some(entrada.mio)
                && cuando_anterior.is_some_and(|c| entrada.cuando - c <= RATO_MS);
            y += if mismo_grupo {
                e(HUECO) as i32
            } else {
                e(HUECO_GRUPO) as i32
            };
            None
        };

        let ancho = (entrada.ancho + 2 * e(RELLENO_X)).clamp(2 * e(RELLENO_X), ancho_maximo);
        let alto = entrada.alto + 2 * e(RELLENO_Y);
        let x = if entrada.mio {
            area.derecha() - e(MARGEN) as i32 - ancho as i32
        } else {
            area.x + e(MARGEN) as i32
        };
        puestos.push(Puesto {
            burbuja: Rect { x, y, ancho, alto },
            separador,
        });
        y += alto as i32;
        dia_anterior = Some(entrada.dia);
        mio_anterior = Some(entrada.mio);
        cuando_anterior = Some(entrada.cuando);
    }
    (puestos, (y + e(HUECO_GRUPO) as i32).max(0) as u32)
}

/// El desplazamiento con el que el ultimo mensaje queda pegado abajo.
///
/// Puede salir NEGATIVO, y esa es la gracia: si los mensajes no llenan la
/// columna, se apoyan sobre la caja de escribir en vez de quedarse arriba
/// con un hueco debajo. Es como se ve una conversacion de tres mensajes en
/// cualquier chat.
pub fn scroll_maximo(area: Rect, alto_total: u32) -> i32 {
    alto_total as i32 - area.alto as i32
}

/// Lo mas arriba que se puede ir. Con un historial que no llena la columna
/// no hay nada que subir: el minimo y el maximo son el mismo sitio.
pub fn scroll_minimo(area: Rect, alto_total: u32) -> i32 {
    scroll_maximo(area, alto_total).min(0)
}

/// Deja el desplazamiento dentro de lo que existe.
pub fn scroll_ajustado(area: Rect, alto_total: u32, scroll: i32) -> i32 {
    scroll.clamp(
        scroll_minimo(area, alto_total),
        scroll_maximo(area, alto_total),
    )
}

/// En que mensaje se pincho, si se pincho en alguno.
///
/// `punto` va en coordenadas de la ventana; `scroll` es el mismo que usa
/// quien pinta. Fuera del area no hay mensaje aunque las cuentas cuadren:
/// el historial esta recortado.
pub fn mensaje_en(area: Rect, puestos: &[Puesto], scroll: i32, punto: Punto) -> Option<usize> {
    if !area.contiene(punto) {
        return None;
    }
    // Al documento: se le quita el origen del area y se le suma lo bajado.
    let en_documento = Punto {
        x: punto.x,
        y: punto.y - area.y + scroll,
    };
    puestos
        .iter()
        .position(|p| p.burbuja.contiene(en_documento))
}

/// Cuales de los puestos se ven con este desplazamiento, para pintar solo
/// esos. `scroll` cuenta desde arriba, ya ajustado.
pub fn visibles(area: Rect, puestos: &[Puesto], scroll: i32) -> (usize, usize) {
    let arriba = scroll;
    let abajo = scroll + area.alto as i32;
    let primero = puestos
        .iter()
        .position(|p| p.burbuja.abajo() >= arriba)
        .unwrap_or(puestos.len());
    let cuantos = puestos[primero..]
        .iter()
        .position(|p| p.arriba() > abajo)
        .unwrap_or(puestos.len() - primero);
    (primero, cuantos)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn area() -> Rect {
        Rect {
            x: 300,
            y: 94,
            ancho: 700,
            alto: 600,
        }
    }

    fn entrada(alto: u32, ancho: u32, mio: bool, dia: i64) -> Entrada {
        Entrada {
            alto,
            ancho,
            mio,
            // Mismo dia y misma hora: lo que cambia en las pruebas es el
            // lado, salvo donde se diga.
            cuando: dia * 86_400_000,
            dia,
        }
    }

    #[test]
    fn una_burbuja_no_pasa_de_su_tope_ni_se_estira_si_es_corta() {
        let a = area();
        let maximo = ancho_contenido(a, 100);
        let (p, _) = colocar(
            a,
            &[entrada(20, 10_000, true, 1), entrada(20, 30, true, 1)],
            100,
        );
        assert_eq!(
            p[0].burbuja.ancho,
            maximo + 2 * RELLENO_X,
            "la larga, al tope"
        );
        assert_eq!(
            p[1].burbuja.ancho,
            30 + 2 * RELLENO_X,
            "la corta, a su medida"
        );
        // Y ninguna se sale de la columna.
        for q in &p {
            assert!(q.burbuja.x >= a.x, "{q:?}");
            assert!(q.burbuja.derecha() <= a.derecha(), "{q:?}");
        }
    }

    #[test]
    fn lo_mio_va_a_la_derecha_y_lo_de_otro_aparato_a_la_izquierda() {
        let a = area();
        let (p, _) = colocar(
            a,
            &[entrada(20, 100, true, 1), entrada(20, 100, false, 1)],
            100,
        );
        assert_eq!(p[0].burbuja.derecha(), a.derecha() - MARGEN as i32);
        assert_eq!(p[1].burbuja.x, a.x + MARGEN as i32);
    }

    #[test]
    fn cada_dia_estrena_su_pildora_de_fecha_y_solo_una() {
        let a = area();
        let (p, _) = colocar(
            a,
            &[
                entrada(20, 100, true, 10),
                entrada(20, 100, true, 10),
                entrada(20, 100, true, 11),
            ],
            100,
        );
        assert!(p[0].separador.is_some(), "el primero dice de cuando es");
        assert!(p[1].separador.is_none(), "mismo dia, sin repetir");
        assert!(p[2].separador.is_some(), "dia nuevo");
        // Y van en orden, sin solaparse.
        assert!(p[0].burbuja.abajo() <= p[1].burbuja.y);
        assert!(p[1].burbuja.abajo() <= p[2].separador.unwrap().y);
    }

    #[test]
    fn dos_del_mismo_lado_se_pegan_mas_que_dos_de_lados_distintos() {
        let a = area();
        let (juntos, _) = colocar(
            a,
            &[entrada(20, 100, true, 1), entrada(20, 100, true, 1)],
            100,
        );
        let (cambio, _) = colocar(
            a,
            &[entrada(20, 100, true, 1), entrada(20, 100, false, 1)],
            100,
        );
        let pegado = juntos[1].burbuja.y - juntos[0].burbuja.abajo();
        let separado = cambio[1].burbuja.y - cambio[0].burbuja.abajo();
        assert_eq!(pegado, HUECO as i32);
        assert_eq!(separado, HUECO_GRUPO as i32);
        assert!(pegado < separado);
    }

    #[test]
    fn un_historial_corto_se_apoya_abajo_y_no_se_puede_mover() {
        let a = area();
        let cortas: Vec<Entrada> = (0..3).map(|_| entrada(20, 100, true, 1)).collect();
        let (_, alto) = colocar(a, &cortas, 100);
        assert!(alto < a.alto, "esto prueba el caso de que sobra sitio");
        // Negativo: empuja los mensajes hacia abajo, contra la caja.
        assert_eq!(scroll_maximo(a, alto), alto as i32 - a.alto as i32);
        assert!(scroll_maximo(a, alto) < 0);
        // Y no hay nada que subir ni que bajar: un solo sitio posible.
        assert_eq!(scroll_minimo(a, alto), scroll_maximo(a, alto));
        assert_eq!(scroll_ajustado(a, alto, 500), scroll_maximo(a, alto));
        assert_eq!(scroll_ajustado(a, alto, -500), scroll_maximo(a, alto));
    }

    #[test]
    fn un_historial_largo_si_se_desplaza_y_no_pasa_de_los_extremos() {
        let a = area();
        let muchas: Vec<Entrada> = (0..200).map(|i| entrada(20, 100, true, i / 10)).collect();
        let (_, alto) = colocar(a, &muchas, 100);
        assert_eq!(scroll_maximo(a, alto), alto as i32 - a.alto as i32);
        assert!(scroll_maximo(a, alto) > 0);
        assert_eq!(scroll_minimo(a, alto), 0, "arriba del todo es el principio");
        assert_eq!(scroll_ajustado(a, alto, -50), 0, "no se sube de mas");
        assert_eq!(scroll_ajustado(a, alto, 999_999), scroll_maximo(a, alto));
    }

    #[test]
    fn de_doscientos_mensajes_solo_se_pintan_los_que_entran() {
        let a = area();
        let muchas: Vec<Entrada> = (0..200).map(|_| entrada(20, 100, true, 1)).collect();
        let (p, alto) = colocar(a, &muchas, 100);
        // Pegado al final, que es donde se abre.
        let abajo = scroll_maximo(a, alto);
        let (primero, cuantos) = visibles(a, &p, abajo);
        assert!(cuantos < 30, "demasiadas: {cuantos}");
        assert_eq!(primero + cuantos, p.len(), "el ultimo se ve");
        // Y desde arriba se ve el primero.
        let (primero, cuantos) = visibles(a, &p, 0);
        assert_eq!(primero, 0);
        assert!(cuantos < 30);
    }

    #[test]
    fn el_contenido_va_dentro_de_su_burbuja_con_su_relleno() {
        let a = area();
        let (p, _) = colocar(a, &[entrada(40, 200, true, 1)], 200);
        let d = p[0].dentro(200);
        let b = p[0].burbuja;
        assert_eq!(
            d.x - b.x,
            (RELLENO_X * 2) as i32,
            "al 200 % el relleno dobla"
        );
        assert_eq!(d.ancho, 200, "el contenido cabe justo");
        assert_eq!(d.alto, 40);
        assert!(d.abajo() <= b.abajo());
    }

    #[test]
    fn sin_mensajes_no_hay_nada_que_colocar() {
        let (p, alto) = colocar(area(), &[], 100);
        assert!(p.is_empty());
        // Sin nada, el unico sitio posible; da igual cual sea mientras no
        // se pueda mover ni se pinte nada.
        assert_eq!(scroll_minimo(area(), alto), scroll_maximo(area(), alto));
        assert_eq!(visibles(area(), &p, 0), (0, 0));
    }

    #[test]
    fn dos_del_mismo_lado_se_pegan_solo_si_son_del_mismo_rato() {
        let a = area();
        let base = 10 * 86_400_000;
        let con = |desfase: i64| Entrada {
            alto: 20,
            ancho: 100,
            mio: true,
            cuando: base + desfase,
            dia: 10,
        };
        // Un minuto despues: misma tanda, pegados.
        let (juntos, _) = colocar(a, &[con(0), con(60_000)], 100);
        assert_eq!(
            juntos[1].burbuja.y - juntos[0].burbuja.abajo(),
            HUECO as i32
        );
        // Media hora despues: otra tanda, se separan aunque sean del mismo.
        let (lejos, _) = colocar(a, &[con(0), con(30 * 60_000)], 100);
        assert_eq!(
            lejos[1].burbuja.y - lejos[0].burbuja.abajo(),
            HUECO_GRUPO as i32
        );
        // Justo en el limite del rato todavia cuenta como la misma tanda.
        let (limite, _) = colocar(a, &[con(0), con(RATO_MS)], 100);
        assert_eq!(
            limite[1].burbuja.y - limite[0].burbuja.abajo(),
            HUECO as i32
        );
        // Y un milisegundo mas, ya no.
        let (fuera, _) = colocar(a, &[con(0), con(RATO_MS + 1)], 100);
        assert_eq!(
            fuera[1].burbuja.y - fuera[0].burbuja.abajo(),
            HUECO_GRUPO as i32
        );
    }

    #[test]
    fn se_sabe_en_que_burbuja_se_pincho_y_donde_no_hay_ninguna() {
        let a = area();
        let unas: Vec<Entrada> = (0..5).map(|_| entrada(20, 100, true, 1)).collect();
        let (p, _) = colocar(a, &unas, 100);
        // Justo en medio de la segunda.
        let b = p[1].burbuja;
        let dentro = pixpin_geom::Punto {
            x: b.x + b.ancho as i32 / 2,
            y: b.y + b.alto as i32 / 2 + a.y,
        };
        assert_eq!(mensaje_en(a, &p, 0, dentro), Some(1));
        // A la izquierda de una burbuja mia hay hueco, no mensaje.
        let hueco = pixpin_geom::Punto {
            x: a.x + 5,
            y: dentro.y,
        };
        assert_eq!(mensaje_en(a, &p, 0, hueco), None);
        // Y fuera del area, nada, aunque las coordenadas cuadren.
        let fuera = pixpin_geom::Punto {
            x: dentro.x,
            y: a.y - 10,
        };
        assert_eq!(mensaje_en(a, &p, 0, fuera), None);
    }

    #[test]
    fn con_la_lista_desplazada_se_acierta_igual() {
        let a = area();
        let muchas: Vec<Entrada> = (0..80).map(|_| entrada(20, 100, true, 1)).collect();
        let (p, alto) = colocar(a, &muchas, 100);
        let scroll = scroll_maximo(a, alto);
        // La ultima burbuja queda pegada abajo del area.
        let ultima = p[p.len() - 1].burbuja;
        let punto = pixpin_geom::Punto {
            x: ultima.x + 5,
            y: ultima.y + ultima.alto as i32 / 2 + a.y - scroll,
        };
        assert_eq!(mensaje_en(a, &p, scroll, punto), Some(p.len() - 1));
    }
}
