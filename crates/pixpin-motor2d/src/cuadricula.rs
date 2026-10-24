//! **El papel pautado**: el fondo de cuadros del lienzo, y su iman.
//!
//! Port de `Cuadricula.kt` del PixPin de Android, mas una cosa que alli no
//! hay y aqui si (§ *El iman de la rejilla*).
//!
//! Dibujando sobre un lienzo en blanco no hay contra que orientarse. Esto no
//! es el iman de [`crate::enganche`] —que engancha a lo que ya hay
//! dibujado— sino lo de **antes**: la referencia que esta ahi antes del
//! primer trazo, para que la primera raya salga recta y la segunda a la
//! misma distancia.
//!
//! Tiene que ser finisima. Una rejilla que se ve tanto como el dibujo compite
//! con el y cansa a los dos minutos; la buena es la que se nota cuando la
//! buscas y desaparece cuando dibujas. Por eso hay dos formas: la de lineas,
//! para alinear, y la de puntos, que dice lo mismo ensuciando la mitad.
//!
//! # No viaja en el fichero
//!
//! Es andamio del editor, no parte del dibujo: como los tiradores de la
//! seleccion. Se ve mientras se trabaja y no sale al exportar. Por eso esto
//! es un ajuste de la ventana y no un campo de la escena. (Excalidraw si
//! tiene `appState.gridSize`, y el movil tampoco lo usa: `ExcalidrawStore`
//! lo lee y lo devuelve, nada mas.)
//!
//! # El iman de la rejilla
//!
//! **El movil no imanta a los cuadros a proposito** (`Cuadricula.kt:16-21`:
//! «No imanta, es andamio visual»), y con un dedo tiene razon: el paso de
//! veinte pixeles es mas fino que la punta del dedo, asi que el iman tiraria
//! siempre y trazar a pulso seria imposible.
//!
//! Con raton el calculo es otro —se apunta al pixel— y una rejilla que no
//! imanta se queda en adorno: se ven los cuadros y las rayas siguen sin
//! caber en ellos. Asi que aqui se anade, **con interruptor propio y
//! apagado no cambia nada**: [`Cuadricula::imanta`]. Lo que no se toca es el
//! fichero, asi que un dibujo hecho con el iman de la rejilla puesto y otro
//! sin el llegan igual de bien al movil.

use crate::elemento::{ColorRgba, EstiloTrazo};
use crate::pintado::Orden;
use crate::vector::Punto2;

/// Como se pauta el fondo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EstiloRejilla {
    #[default]
    Ninguna,
    Lineas,
    Puntos,
}

/// El lado del cuadro, en pixeles de escena. Veinte es el del original.
pub const PASO: f32 = 20.0;

/// Lo poco que puede medir un cuadro **en pantalla** antes de que la rejilla
/// deje de leerse.
///
/// Por debajo de esto las rayas se tocan y lo que queda es una trama gris que
/// tapa el dibujo. En vez de dejar de pintarla —que seria perder la
/// referencia justo cuando se mira el conjunto— se **dobla el paso**: la
/// rejilla se hace mas basta y sigue diciendo lo mismo.
const MINIMO_EN_PANTALLA: f32 = 14.0;

/// Tope duro de rayas por eje.
///
/// Con coordenadas absurdas —un zoom degenerado, un infinito que se colo—
/// esto se llama en cada fotograma y no puede intentar pintar un millon de
/// rayas antes de darse cuenta.
const MAXIMAS_LINEAS: usize = 400;

/// Cuantas veces se dobla el paso como mucho. Doce dobleces son cuatro mil
/// veces el paso: mas alla no queda zoom que valga.
const MAX_DOBLECES: u32 = 12;

/// Radio de captura del iman de la rejilla, **en pixeles de pantalla**.
///
/// Mas estrecho que el del iman de figuras (catorce) a proposito: los cruces
/// de la rejilla estan por todas partes, y con el radio ancho no habria un
/// solo sitio del lienzo donde el cursor no estuviera capturado.
pub const RADIO_IMAN_PX: f32 = 8.0;

/// Lo palido que se pinta el pautado.
const COLOR: ColorRgba = ColorRgba {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.07,
};

/// Lado del puntito de la rejilla de puntos, en pixeles de pantalla.
const LADO_PUNTO_PX: f32 = 1.5;

/// El paso que toca a este zoom, doblando el de partida hasta que se vea.
///
/// Es tambien la guarda de rendimiento: sin ella, alejarse al diez por ciento
/// sobre un lienzo infinito pide **miles** de rayas por fotograma, que es la
/// forma mas tonta de que una aplicacion de dibujo se arrastre.
///
/// Doblando —y no multiplicando por cualquier cosa— los cuadros nuevos siguen
/// cayendo encima de los viejos: al alejar, la rejilla se hace basta sin que
/// las rayas se muevan de sitio.
pub fn paso_visible(zoom: f32, paso: f32) -> f32 {
    if paso.is_nan() || paso <= 0.0 {
        return PASO;
    }
    let z = zoom.max(0.0001);
    let mut p = paso;
    let mut dobleces = 0;
    while p * z < MINIMO_EN_PANTALLA && dobleces < MAX_DOBLECES {
        p *= 2.0;
        dobleces += 1;
    }
    p
}

/// Donde caen las rayas entre `desde` y `hasta`, en coordenadas de escena.
///
/// Alineadas al **cero de la escena** y no al borde de la pantalla: asi la
/// rejilla se queda quieta respecto del dibujo mientras se panea, que es lo
/// unico que la hace servir de referencia.
pub fn rayas(desde: f32, hasta: f32, paso: f32) -> Vec<f32> {
    if paso.is_nan() || paso <= 0.0 || !desde.is_finite() || !hasta.is_finite() || hasta < desde {
        return Vec::new();
    }
    let primera = (desde / paso).ceil() * paso;
    if !primera.is_finite() {
        return Vec::new();
    }
    let cuantas = (((hasta - primera) / paso) as i64) + 1;
    if cuantas <= 0 {
        return Vec::new();
    }
    let tope = (cuantas as usize).min(MAXIMAS_LINEAS);
    (0..tope).map(|i| primera + i as f32 * paso).collect()
}

/// La rejilla del editor: como se ve y si tira del cursor.
///
/// Se llama `Cuadricula` y no `Rejilla` porque en este crate `Rejilla` ya es
/// el indice espacial del picado (`indice.rs`), que no tiene nada que ver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cuadricula {
    pub estilo: EstiloRejilla,
    /// El lado del cuadro en pixeles de escena, antes de doblarlo por zoom.
    pub paso: f32,
    /// Si el cursor se pega a los cruces. Ver la nota de la cabecera.
    pub imanta: bool,
}

impl Default for Cuadricula {
    /// Apagada, como el movil y como Excalidraw: quien la quiere la pide.
    fn default() -> Self {
        Self {
            estilo: EstiloRejilla::Ninguna,
            paso: PASO,
            imanta: false,
        }
    }
}

impl Cuadricula {
    /// Si hay algo que pintar.
    pub fn se_ve(&self) -> bool {
        self.estilo != EstiloRejilla::Ninguna
    }

    /// Como se pinta sobre la caja de mundo `vista`.
    ///
    /// La rejilla de puntos sale como cuadraditos rellenos y no como rayas
    /// cortas: el punto tiene que medir lo mismo mire uno al zoom que mire, y
    /// una raya de un pixel a zoom 0,2 desaparece.
    pub fn ordenes(&self, vista: (f32, f32, f32, f32), zoom: f32) -> Vec<Orden> {
        if !self.se_ve() {
            return Vec::new();
        }
        let (x0, y0, x1, y1) = vista;
        let paso = paso_visible(zoom, self.paso);
        let escala = 1.0 / zoom.max(0.0001);
        let xs = rayas(x0, x1, paso);
        let ys = rayas(y0, y1, paso);

        match self.estilo {
            EstiloRejilla::Ninguna => Vec::new(),
            EstiloRejilla::Lineas => {
                let grosor = (1.0 * escala).max(0.01);
                let mut fuera = Vec::with_capacity(xs.len() + ys.len());
                for x in &xs {
                    fuera.push(Orden::Polilinea {
                        puntos: vec![Punto2::nuevo(*x, y0), Punto2::nuevo(*x, y1)],
                        color: COLOR,
                        grosor,
                        estilo: EstiloTrazo::Solido,
                    });
                }
                for y in &ys {
                    fuera.push(Orden::Polilinea {
                        puntos: vec![Punto2::nuevo(x0, *y), Punto2::nuevo(x1, *y)],
                        color: COLOR,
                        grosor,
                        estilo: EstiloTrazo::Solido,
                    });
                }
                fuera
            }
            EstiloRejilla::Puntos => {
                let r = LADO_PUNTO_PX * escala / 2.0;
                let mut fuera = Vec::with_capacity(xs.len() * ys.len());
                for x in &xs {
                    for y in &ys {
                        fuera.push(Orden::Relleno {
                            puntos: vec![
                                Punto2::nuevo(x - r, y - r),
                                Punto2::nuevo(x + r, y - r),
                                Punto2::nuevo(x + r, y + r),
                                Punto2::nuevo(x - r, y + r),
                            ],
                            color: COLOR,
                        });
                    }
                }
                fuera
            }
        }
    }

    /// El cruce de rejilla al que se pega `p`, o `None` si no imanta o si
    /// ninguno cae cerca.
    ///
    /// Devuelve `Option` y no el punto ya movido para que quien llama pueda
    /// **ensenar** que ha enganchado, igual que hace el iman de figuras. Un
    /// iman invisible es magia: el trazo se va dos pixeles y no hay forma de
    /// distinguir un enganche de un fallo de punteria.
    ///
    /// El radio va en pixeles de pantalla por lo mismo que el del iman de
    /// figuras: el cursor apunta con la misma precision mires al zoom que
    /// mires.
    pub fn ajustar(&self, p: Punto2, zoom: f32) -> Option<Punto2> {
        if !self.imanta {
            return None;
        }
        let paso = paso_visible(zoom, self.paso);
        if paso.is_nan() || paso <= 0.0 {
            return None;
        }
        let cruce = Punto2::nuevo((p.x / paso).round() * paso, (p.y / paso).round() * paso);
        // El radio se topa en medio paso: con cuadros muy pequenos en
        // pantalla, un radio mayor que el cuadro haria que el cursor nunca
        // estuviera libre, que es justo el motivo por el que el movil no
        // imanta.
        let radio = (RADIO_IMAN_PX / zoom.max(0.0001)).min(paso / 2.0);
        (cruce.distancia(p) <= radio).then_some(cruce)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_paso_se_dobla_hasta_que_el_cuadro_se_ve_en_pantalla() {
        // A zoom 1 el cuadro mide 20 px: se ve, no se dobla.
        assert_eq!(paso_visible(1.0, PASO), PASO);
        // Al 25 % medirian 5 px y las rayas se tocarian: se dobla hasta 80,
        // que en pantalla son 20.
        assert_eq!(paso_visible(0.25, PASO), 80.0);
        // Acercandose no se afina: los cuadros se separan y ya estaba bien.
        assert_eq!(paso_visible(4.0, PASO), PASO);
    }

    #[test]
    fn doblar_no_mueve_las_rayas_de_sitio() {
        // Es la razon de doblar en vez de multiplicar por cualquier cosa: al
        // alejar, la rejilla se hace basta y las rayas que quedan son las que
        // ya estaban.
        let fino = rayas(0.0, 400.0, paso_visible(1.0, PASO));
        let basto = rayas(0.0, 400.0, paso_visible(0.25, PASO));
        for x in &basto {
            assert!(fino.contains(x), "{x} no estaba en la rejilla fina");
        }
    }

    #[test]
    fn las_rayas_se_alinean_al_cero_de_la_escena_y_no_al_borde_de_la_vista() {
        // Sin esto la rejilla se arrastra con el paneo y deja de servir de
        // referencia.
        let r = rayas(37.0, 100.0, 20.0);
        assert_eq!(r, vec![40.0, 60.0, 80.0, 100.0]);
    }

    #[test]
    fn hay_tope_de_rayas_por_muy_absurda_que_sea_la_vista() {
        assert_eq!(rayas(0.0, 1_000_000.0, 1.0).len(), MAXIMAS_LINEAS);
    }

    #[test]
    fn una_vista_del_reves_o_un_paso_invalido_no_dan_rayas() {
        // Caso negativo: esto corre en cada fotograma y no puede entrar en un
        // bucle infinito ni devolver NaN.
        assert!(rayas(100.0, 0.0, 20.0).is_empty());
        assert!(rayas(0.0, 100.0, 0.0).is_empty());
        assert!(rayas(0.0, 100.0, -5.0).is_empty());
        assert!(rayas(f32::NAN, 100.0, 20.0).is_empty());
        assert_eq!(
            paso_visible(1.0, 0.0),
            PASO,
            "un paso nulo vuelve al de fabrica"
        );
    }

    #[test]
    fn apagada_no_pinta_nada() {
        let c = Cuadricula::default();
        assert!(!c.se_ve());
        assert!(c.ordenes((0.0, 0.0, 500.0, 500.0), 1.0).is_empty());
    }

    #[test]
    fn la_de_lineas_da_una_raya_por_cruce_de_eje_y_la_de_puntos_un_punto_por_cruce() {
        let vista = (0.0, 0.0, 100.0, 60.0);
        let lineas = Cuadricula {
            estilo: EstiloRejilla::Lineas,
            ..Default::default()
        };
        // x: 0,20,40,60,80,100 -> 6. y: 0,20,40,60 -> 4.
        assert_eq!(lineas.ordenes(vista, 1.0).len(), 10);

        let puntos = Cuadricula {
            estilo: EstiloRejilla::Puntos,
            ..Default::default()
        };
        assert_eq!(puntos.ordenes(vista, 1.0).len(), 24, "6 por 4 cruces");
    }

    #[test]
    fn el_punto_de_la_rejilla_no_crece_con_el_zoom() {
        let c = Cuadricula {
            estilo: EstiloRejilla::Puntos,
            ..Default::default()
        };
        let lado = |zoom: f32| {
            let Orden::Relleno { puntos, .. } = &c.ordenes((0.0, 0.0, 20.0, 20.0), zoom)[0] else {
                panic!("los puntos se pintan rellenos");
            };
            puntos[0].distancia(puntos[1])
        };
        assert!((lado(1.0) - LADO_PUNTO_PX).abs() < 0.01);
        assert!((lado(2.0) - LADO_PUNTO_PX / 2.0).abs() < 0.01);
    }

    #[test]
    fn el_iman_apagado_no_mueve_el_cursor() {
        // Caso negativo, y el que mas importa: es lo que hace que anadir este
        // iman al PC no cambie nada para quien no lo pide.
        let c = Cuadricula {
            estilo: EstiloRejilla::Lineas,
            imanta: false,
            ..Default::default()
        };
        assert!(c.ajustar(Punto2::nuevo(1.0, 1.0), 1.0).is_none());
    }

    #[test]
    fn el_iman_pega_al_cruce_mas_cercano_y_solo_si_esta_cerca() {
        let c = Cuadricula {
            estilo: EstiloRejilla::Lineas,
            imanta: true,
            ..Default::default()
        };
        assert_eq!(
            c.ajustar(Punto2::nuevo(38.0, 62.0), 1.0),
            Some(Punto2::nuevo(40.0, 60.0))
        );
        // En mitad del cuadro no engancha: si no, el cursor no estaria libre
        // en ningun sitio del lienzo.
        assert!(c.ajustar(Punto2::nuevo(30.0, 30.0), 1.0).is_none());
    }

    #[test]
    fn el_radio_del_iman_nunca_pasa_de_medio_cuadro() {
        // Con el paso doblado por zoom, el radio en escena crece; topado a
        // medio cuadro, entre dos cruces siempre queda sitio libre.
        let c = Cuadricula {
            estilo: EstiloRejilla::Lineas,
            imanta: true,
            paso: 4.0,
        };
        // Paso 4 a zoom 1 -> se dobla a 16 (16 >= 14). Medio cuadro son 8;
        // el radio de pantalla tambien es 8, asi que manda el tope.
        let justo_en_medio = Punto2::nuevo(8.0, 0.0);
        assert_eq!(
            c.ajustar(justo_en_medio, 1.0),
            Some(Punto2::nuevo(16.0, 0.0)),
            "el empate cae al cruce de la derecha por el redondeo, pero engancha"
        );
        assert!(
            c.ajustar(Punto2::nuevo(8.1, 0.0), 1.0).is_some(),
            "pasado el medio, el cruce de la derecha queda a menos de medio cuadro"
        );
    }
}
