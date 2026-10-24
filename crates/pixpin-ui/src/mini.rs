//! El panel de una mini-aplicacion, dentro de la conversacion.
//!
//! Geometria pura, como el resto del crate: aqui NO se sabe si lo que hay
//! debajo son tareas, gastos o una ruleta. Todas las mini-apps del movil
//! (`mini/MiniActivity.kt`) se pintan con el mismo esqueleto —titulo arriba,
//! un tablero grande, una o dos filas de botones, una lista y una caja de
//! anadir abajo— y lo unico que cambia entre ellas es **cuantas de esas
//! piezas se usan**. Por eso [`Disposicion::calcular`] las recibe como
//! banderas en vez de haber siete disposiciones distintas: una sola pieza que
//! probar, y siete pantallas que no se pueden descuadrar entre si.
//!
//! Tres decisiones que no son evidentes:
//!
//! - **Las piezas que no se usan miden cero, no desaparecen.** Un `Rect` de
//!   alto cero se pinta como nada y se puede seguir preguntando por el sin un
//!   `Option`; quien pinta no tiene que saber que mini-app es para decidir si
//!   mirar el tablero.
//! - **Nada se invierte en un hueco pequeno.** En `u32` un rectangulo del
//!   reves no es un dibujo raro, es un desbordamiento que pinta fuera de la
//!   ventana. Todas las restas van por `saturating_sub` o `min`, y la lista se
//!   queda en cero antes que robarle sitio a la cabecera.
//! - **La lista es lo unico que se desplaza.** El tablero y los botones se
//!   quedan quietos: en un cronometro, ver el numero es justo lo que no se
//!   puede perder al bajar por las vueltas.

use pixpin_geom::{Punto, Rect};

/// Medidas en pixeles logicos (al 100 %).
/// La barra del titulo, donde tambien va «Esc para volver».
pub const CABECERA_ALTO: u32 = 36;
/// El tablero: el numero grande del cronometro, la hora de la alarma, el
/// total de los gastos o el nombre que salio en la ruleta.
pub const TABLERO_ALTO: u32 = 92;
/// Lo que ocupa una fila de botones.
pub const BOTONES_ALTO: u32 = 44;
/// La caja de escribir de abajo.
pub const ANADIR_ALTO: u32 = 38;
/// Una linea de la lista.
pub const FILA_ALTO: u32 = 32;
/// El aire a los lados de todo.
pub const MARGEN: u32 = 12;
/// El hueco entre dos botones de la misma fila.
pub const HUECO_BOTON: u32 = 8;
/// El aspa de borrar de cada fila, cuadrada y al final de la linea. Los
/// otros iconos de la fila (corregir, subir, bajar) miden lo mismo y van a su
/// izquierda ([`Disposicion::icono_de_fila`]).
pub const ASPA_ANCHO: u32 = 28;
/// La casilla de una tarea, al principio de la fila: su icono y aire.
pub const CASILLA_ANCHO: u32 = 28;
/// La linea del avance de una lista de tareas: «3 de 7» y su barrita.
pub const AVANCE_ALTO: u32 = 28;

pub const TITULO_TAM: f32 = 15.0;
pub const TABLERO_TAM: f32 = 40.0;
pub const FILA_TAM: f32 = 14.0;
pub const BOTON_TAM: f32 = 13.0;
pub const ANADIR_TAM: f32 = 14.0;

/// Que piezas quiere una mini-app.
///
/// Se pide entero y no pieza a pieza para que anadir una octava mini-app sea
/// escribir un literal y no tocar la firma de [`Disposicion::calcular`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reparto {
    pub con_tablero: bool,
    /// Cuantas filas de botones: el contador quiere dos (los dos grandes y
    /// los pasos), el cronometro una, la ruleta ninguna hasta que hay nombres.
    pub filas_de_botones: u32,
    pub con_lista: bool,
    pub con_anadir: bool,
    /// La linea fina de avance bajo los botones («3 de 7» y una barra). Solo
    /// la lista de tareas la pide.
    pub con_avance: bool,
}

impl Reparto {
    /// Lo mas corriente: una fila de botones y una lista donde anadir.
    pub fn lista_con_botones() -> Reparto {
        Reparto {
            con_tablero: false,
            filas_de_botones: 1,
            con_lista: true,
            con_anadir: true,
            con_avance: false,
        }
    }
}

/// Como queda repartido el panel dentro de su hueco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposicion {
    /// Todo el hueco del panel.
    pub panel: Rect,
    pub cabecera: Rect,
    /// El numero grande. Alto cero si esta mini-app no lo usa.
    pub tablero: Rect,
    /// Todas las filas de botones juntas. Alto cero si no hay ninguna.
    pub botones: Rect,
    /// La linea de avance, bajo los botones. Alto cero si no se pidio.
    pub avance: Rect,
    /// Lo que queda: la lista, y lo unico que se desplaza.
    pub lista: Rect,
    /// La caja de escribir de abajo. Alto cero si no se puede anadir.
    pub anadir: Rect,
    /// Cuantas filas de botones se pidieron, para repartir [`Disposicion::botones`].
    filas_de_botones: u32,
}

impl Disposicion {
    /// Reparte `hueco` de arriba abajo, y le da a la lista lo que sobre.
    ///
    /// El orden de reparto es el orden de importancia: si no cabe todo, lo
    /// primero que se queda sin sitio es la lista, porque es lo unico que se
    /// puede desplazar. Quitarle sitio al tablero dejaria un numero cortado
    /// por la mitad, que no se lee.
    pub fn calcular(hueco: Rect, escala_por_cien: u32, reparto: Reparto) -> Disposicion {
        let e = |v: u32| v * escala_por_cien / 100;

        let cabecera = Rect {
            alto: e(CABECERA_ALTO).min(hueco.alto),
            ..hueco
        };
        let mut queda = hueco.alto - cabecera.alto;

        let alto_tablero = if reparto.con_tablero {
            e(TABLERO_ALTO).min(queda)
        } else {
            0
        };
        let tablero = Rect {
            x: hueco.x,
            y: cabecera.abajo(),
            ancho: hueco.ancho,
            alto: alto_tablero,
        };
        queda -= alto_tablero;

        let alto_botones = (e(BOTONES_ALTO) * reparto.filas_de_botones).min(queda);
        let botones = Rect {
            x: hueco.x,
            y: tablero.abajo(),
            ancho: hueco.ancho,
            alto: alto_botones,
        };
        queda -= alto_botones;

        let alto_avance = if reparto.con_avance {
            e(AVANCE_ALTO).min(queda)
        } else {
            0
        };
        let avance = Rect {
            x: hueco.x,
            y: botones.abajo(),
            ancho: hueco.ancho,
            alto: alto_avance,
        };
        queda -= alto_avance;

        // La caja de anadir se reserva ANTES que la lista: escribir tiene que
        // seguir siendo posible en una ventana baja, aunque no se vea ni una
        // linea de lo que ya hay.
        let alto_anadir = if reparto.con_anadir {
            e(ANADIR_ALTO).min(queda)
        } else {
            0
        };
        queda -= alto_anadir;

        let alto_lista = if reparto.con_lista { queda } else { 0 };
        let lista = Rect {
            x: hueco.x,
            y: avance.abajo(),
            ancho: hueco.ancho,
            alto: alto_lista,
        };
        let anadir = Rect {
            x: hueco.x,
            y: hueco.abajo() - alto_anadir as i32,
            ancho: hueco.ancho,
            alto: alto_anadir,
        };

        Disposicion {
            panel: hueco,
            cabecera,
            tablero,
            botones,
            avance,
            lista,
            anadir,
            filas_de_botones: reparto.filas_de_botones,
        }
    }

    /// Donde cae el boton `n` de `de` botones, en la fila `fila`.
    ///
    /// Se reparten a partes iguales todo lo ancho: los botones de una
    /// mini-app se pulsan sin mirar —parar el cronometro, sumar una caja— y
    /// un boton estrecho obliga a apuntar.
    pub fn boton(&self, fila: u32, n: usize, de: usize, escala_por_cien: u32) -> Rect {
        if de == 0 || fila >= self.filas_de_botones || self.botones.alto == 0 {
            return Rect {
                x: self.botones.x,
                y: self.botones.y,
                ancho: 0,
                alto: 0,
            };
        }
        let e = |v: u32| v * escala_por_cien / 100;
        let margen = e(MARGEN);
        let hueco = e(HUECO_BOTON);
        let alto_fila = self.botones.alto / self.filas_de_botones.max(1);
        let util = self
            .botones
            .ancho
            .saturating_sub(margen * 2)
            .saturating_sub(hueco * (de as u32 - 1));
        let ancho = util / de as u32;
        // El resto de la division se lo queda el ultimo, para que la fila
        // acabe justo en el margen y no dos pixeles antes.
        let ancho_n = if n + 1 == de {
            util - ancho * (de as u32 - 1)
        } else {
            ancho
        };
        Rect {
            x: self.botones.x + margen as i32 + (n as u32 * (ancho + hueco)) as i32,
            y: self.botones.y + (fila * alto_fila) as i32 + (e(4)) as i32,
            ancho: ancho_n,
            alto: alto_fila.saturating_sub(e(8)),
        }
    }

    /// Cual de los `de` botones de esa fila cae bajo el punto, si alguno.
    pub fn cual_boton(
        &self,
        p: Punto,
        fila: u32,
        de: usize,
        escala_por_cien: u32,
    ) -> Option<usize> {
        (0..de).find(|&n| self.boton(fila, n, de, escala_por_cien).contiene(p))
    }

    /// Donde cae la linea `n` de la lista, con el desplazamiento ya restado.
    ///
    /// Puede caer fuera de [`Disposicion::lista`]: recortar es cosa de quien
    /// pinta, que es quien sabe si esta dibujando texto o un recuadro.
    pub fn fila(&self, n: usize, scroll: i32, escala_por_cien: u32) -> Rect {
        let alto = FILA_ALTO * escala_por_cien / 100;
        let margen = (MARGEN * escala_por_cien / 100) as i32;
        Rect {
            x: self.lista.x + margen,
            y: self.lista.y + (n as i64 * alto as i64) as i32 - scroll,
            ancho: self.lista.ancho.saturating_sub(margen as u32 * 2),
            alto,
        }
    }

    /// El aspa de borrar de una fila: un cuadrado al final de la linea.
    pub fn aspa(&self, fila: Rect, escala_por_cien: u32) -> Rect {
        self.icono_de_fila(fila, 0, escala_por_cien)
    }

    /// La casilla de una tarea: el cuadrado del principio de la fila. Es lo
    /// unico que tacha, como el `Checkbox` del movil (`MiniActivity.kt`,
    /// `DeTareas`): el resto de la fila elige la tarea para moverla o
    /// corregirla, y tocarla para eso no puede tacharla de paso.
    pub fn casilla(&self, fila: Rect, escala_por_cien: u32) -> Rect {
        Rect {
            ancho: (CASILLA_ANCHO * escala_por_cien / 100).min(fila.ancho),
            ..fila
        }
    }

    /// El icono `n` de una fila contando desde la derecha: el 0 es el aspa,
    /// el 1 el que va a su izquierda, y asi. Todos cuadrados y del mismo
    /// lado, para que la columna de iconos quede alineada fila a fila.
    ///
    /// Si ya no cabe, mide cero de ancho: un icono que no se ve no puede
    /// seguir pulsandose encima del texto.
    pub fn icono_de_fila(&self, fila: Rect, n: u32, escala_por_cien: u32) -> Rect {
        let lado = ASPA_ANCHO * escala_por_cien / 100;
        let derecha = fila.ancho.saturating_sub(lado.saturating_mul(n));
        let ancho = lado.min(derecha);
        Rect {
            x: fila.x + (derecha - ancho) as i32,
            y: fila.y,
            ancho,
            alto: fila.alto,
        }
    }

    /// Que linea de la lista cae bajo el punto, de las `cuantas` que hay.
    ///
    /// `None` fuera del area de la lista: un clic en la caja de anadir no
    /// puede marcar la ultima tarea solo porque su fila quede debajo.
    pub fn cual_fila(
        &self,
        p: Punto,
        cuantas: usize,
        scroll: i32,
        escala_por_cien: u32,
    ) -> Option<usize> {
        if !self.lista.contiene(p) {
            return None;
        }
        (0..cuantas).find(|&n| self.fila(n, scroll, escala_por_cien).contiene(p))
    }

    /// Hasta donde se puede bajar por la lista.
    ///
    /// Cero cuando todo cabe: sin esto se podria empujar la ultima linea por
    /// encima del borde y dejar la lista vacia con el raton.
    pub fn tope_scroll(&self, cuantas: usize, escala_por_cien: u32) -> i32 {
        let alto = (FILA_ALTO * escala_por_cien / 100) as i64;
        let total = cuantas as i64 * alto;
        (total - self.lista.alto as i64).max(0) as i32
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hueco() -> Rect {
        Rect {
            x: 10,
            y: 20,
            ancho: 400,
            alto: 600,
        }
    }

    #[test]
    fn las_piezas_van_de_arriba_abajo_y_no_se_pisan() {
        let d = Disposicion::calcular(
            hueco(),
            100,
            Reparto {
                con_tablero: true,
                filas_de_botones: 1,
                con_lista: true,
                con_anadir: true,
                con_avance: true,
            },
        );
        assert_eq!(d.cabecera.y, 20);
        assert_eq!(d.tablero.y, d.cabecera.abajo());
        assert_eq!(d.botones.y, d.tablero.abajo());
        assert_eq!(d.avance.y, d.botones.abajo());
        assert_eq!(d.avance.alto, AVANCE_ALTO);
        assert_eq!(d.lista.y, d.avance.abajo());
        assert_eq!(d.anadir.y, d.lista.abajo());
        assert_eq!(d.anadir.abajo(), hueco().abajo());
    }

    #[test]
    fn lo_que_no_se_pide_mide_cero_y_no_deja_agujero() {
        let d = Disposicion::calcular(
            hueco(),
            100,
            Reparto {
                con_tablero: false,
                filas_de_botones: 0,
                con_lista: true,
                con_anadir: false,
                con_avance: false,
            },
        );
        assert_eq!(d.tablero.alto, 0);
        assert_eq!(d.botones.alto, 0);
        assert_eq!(d.avance.alto, 0);
        assert_eq!(d.anadir.alto, 0);
        // La lista se queda con todo lo que no es cabecera.
        assert_eq!(d.lista.y, d.cabecera.abajo());
        assert_eq!(d.lista.abajo(), hueco().abajo());
    }

    #[test]
    fn un_hueco_enano_no_invierte_ningun_rectangulo() {
        let enano = Rect {
            x: 0,
            y: 0,
            ancho: 50,
            alto: 10,
        };
        let d = Disposicion::calcular(enano, 100, Reparto::lista_con_botones());
        for r in [d.cabecera, d.tablero, d.botones, d.lista, d.anadir] {
            assert!(r.alto <= enano.alto, "{r:?} se salio de {enano:?}");
        }
        assert_eq!(d.lista.alto, 0, "la lista es lo primero que cede el sitio");
    }

    #[test]
    fn la_caja_de_anadir_sobrevive_a_una_ventana_baja() {
        // Cabecera 36 + anadir 38 = 74: con 90 de alto entra todo menos lista.
        let bajo = Rect {
            x: 0,
            y: 0,
            ancho: 300,
            alto: 90,
        };
        let d = Disposicion::calcular(
            bajo,
            100,
            Reparto {
                con_tablero: false,
                filas_de_botones: 0,
                con_lista: true,
                con_anadir: true,
                con_avance: false,
            },
        );
        assert_eq!(d.anadir.alto, ANADIR_ALTO, "escribir no puede perderse");
        assert_eq!(d.lista.alto, 90 - CABECERA_ALTO - ANADIR_ALTO);
    }

    #[test]
    fn los_botones_de_una_fila_llenan_el_ancho_sin_solaparse() {
        let d = Disposicion::calcular(hueco(), 100, Reparto::lista_con_botones());
        let a = d.boton(0, 0, 3, 100);
        let b = d.boton(0, 1, 3, 100);
        let c = d.boton(0, 2, 3, 100);
        assert!(a.derecha() < b.x && b.derecha() < c.x, "{a:?} {b:?} {c:?}");
        assert_eq!(
            c.derecha(),
            hueco().derecha() - MARGEN as i32,
            "el ultimo acaba en el margen"
        );
        assert_eq!(a.y, b.y);
    }

    #[test]
    fn un_boton_que_no_existe_no_devuelve_un_rectangulo_util() {
        let d = Disposicion::calcular(hueco(), 100, Reparto::lista_con_botones());
        // Fila que no se pidio, y cero botones: los dos casos dan area nula.
        assert_eq!(d.boton(3, 0, 2, 100).ancho, 0);
        assert_eq!(d.boton(0, 0, 0, 100).ancho, 0);
        assert_eq!(d.cual_boton(Punto { x: 20, y: 20 }, 9, 3, 100), None);
    }

    #[test]
    fn se_pulsa_la_fila_que_esta_debajo_y_no_otra() {
        let d = Disposicion::calcular(hueco(), 100, Reparto::lista_con_botones());
        let f2 = d.fila(2, 0, 100);
        let dentro = Punto {
            x: f2.x + 5,
            y: f2.y + 5,
        };
        assert_eq!(d.cual_fila(dentro, 5, 0, 100), Some(2));
        // Caso negativo: fuera de la lista no hay fila, aunque la cuenta diga
        // que esa linea existiria ahi abajo.
        let debajo = Punto {
            x: f2.x + 5,
            y: d.lista.abajo() + 4,
        };
        assert_eq!(d.cual_fila(debajo, 100, 0, 100), None);
        // Y una lista vacia no devuelve nunca una fila.
        assert_eq!(d.cual_fila(dentro, 0, 0, 100), None);
    }

    #[test]
    fn bajar_por_la_lista_sube_las_filas() {
        let d = Disposicion::calcular(hueco(), 100, Reparto::lista_con_botones());
        let quieta = d.fila(4, 0, 100);
        let corrida = d.fila(4, 30, 100);
        assert_eq!(corrida.y, quieta.y - 30);
    }

    #[test]
    fn el_tope_es_cero_cuando_cabe_todo() {
        let d = Disposicion::calcular(hueco(), 100, Reparto::lista_con_botones());
        assert_eq!(d.tope_scroll(0, 100), 0);
        assert_eq!(d.tope_scroll(2, 100), 0, "dos filas caben de sobra");
        let muchas = d.tope_scroll(1000, 100);
        assert_eq!(muchas, 1000 * FILA_ALTO as i32 - d.lista.alto as i32);
    }

    #[test]
    fn el_aspa_cae_al_final_de_su_fila_y_no_se_sale() {
        let d = Disposicion::calcular(hueco(), 100, Reparto::lista_con_botones());
        let f = d.fila(0, 0, 100);
        let a = d.aspa(f, 100);
        assert_eq!(a.derecha(), f.derecha());
        assert_eq!(a.alto, f.alto);
        // Caso negativo: una fila mas estrecha que el aspa no la desborda.
        let estrecha = Rect {
            x: 0,
            y: 0,
            ancho: 10,
            alto: 20,
        };
        assert_eq!(d.aspa(estrecha, 100).ancho, 10);
    }

    #[test]
    fn los_iconos_de_la_fila_van_en_columna_desde_la_derecha() {
        let d = Disposicion::calcular(hueco(), 100, Reparto::lista_con_botones());
        let f = d.fila(0, 0, 100);
        let aspa = d.icono_de_fila(f, 0, 100);
        let lapiz = d.icono_de_fila(f, 1, 100);
        assert_eq!(aspa, d.aspa(f, 100));
        assert_eq!(lapiz.derecha(), aspa.x, "pegado a la izquierda del aspa");
        assert_eq!(lapiz.ancho, ASPA_ANCHO);
        // Caso negativo: en una fila estrecha el que no cabe mide cero y no
        // se sale por la izquierda.
        let estrecha = Rect {
            x: 100,
            y: 0,
            ancho: 40,
            alto: 20,
        };
        let tercero = d.icono_de_fila(estrecha, 2, 100);
        assert_eq!(tercero.ancho, 0);
        assert!(tercero.x >= estrecha.x);
        // La casilla, al principio y sin salirse de la fila.
        let c = d.casilla(f, 100);
        assert_eq!((c.x, c.ancho, c.alto), (f.x, CASILLA_ANCHO, f.alto));
        assert_eq!(d.casilla(estrecha, 100).ancho, CASILLA_ANCHO.min(40));
    }

    #[test]
    fn la_escala_estira_todas_las_piezas() {
        let d = Disposicion::calcular(hueco(), 200, Reparto::lista_con_botones());
        assert_eq!(d.cabecera.alto, CABECERA_ALTO * 2);
        assert_eq!(d.fila(1, 0, 200).y - d.lista.y, FILA_ALTO as i32 * 2);
    }
}
