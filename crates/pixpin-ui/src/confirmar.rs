//! El cuadro que pregunta antes de meter ficheros en el proyecto.
//!
//! Soltar algo en la ventana o pegarlo mete un mensaje en el cuaderno, y eso
//! no se deshace: por eso se pregunta. Ensena QUE se va a meter —con su
//! nombre y su tamano—, deja escribir un pie y tiene dos salidas claras.
//!
//! Geometria pura, como el resto del crate. Las medidas siguen al cuadro de
//! enviar ficheros de Telegram Desktop; **de ahi solo medidas, nunca su
//! codigo, que es GPL-3.0.**
//!
//! Dos decisiones que no son evidentes:
//!
//! - **No se desplaza.** Se ensenan como mucho cuatro ficheros y, si hay
//!   mas, una linea que dice cuantos quedan. Un cuadro de confirmar con
//!   barra de desplazamiento invita a leerlo entero, que es justo lo que no
//!   hace falta: lo que importa es cuantos son y de que clase.
//! - **Se aprieta antes que salirse.** En una ventana pequena el cuadro se
//!   encoge; los botones se colocan desde abajo para que, pase lo que pase,
//!   siga habiendo como cancelar.

use pixpin_geom::{Punto, Rect};

/// Medidas en pixeles logicos (al 100 %).
pub const ANCHO: u32 = 420;
pub const RADIO: u32 = 10;
pub const MARGEN_VENTANA: u32 = 24;
/// La cabecera, con el titulo.
pub const CABECERA: u32 = 54;
pub const TITULO_X: u32 = 22;
pub const TITULO_TAM: f32 = 15.0;
/// Cada fichero de la lista.
pub const FILA: u32 = 56;
pub const FILA_X: u32 = 22;
pub const MINIATURA: u32 = 40;
pub const MINIATURA_RADIO: u32 = 6;
pub const NOMBRE_X: u32 = 74;
pub const NOMBRE_Y: u32 = 10;
pub const NOMBRE_TAM: f32 = 13.0;
pub const TAMANO_Y: u32 = 30;
pub const TAMANO_TAM: f32 = 12.0;
/// Cuantos ficheros se ensenan antes de resumir el resto.
pub const FILAS_MAXIMAS: usize = 4;
/// La caja del pie, y los botones.
pub const PIE_ALTO: u32 = 48;
pub const PIE_X: u32 = 22;
pub const PIE_TAM: f32 = 13.0;
pub const BOTONES_ALTO: u32 = 56;
pub const BOTON_ALTO: u32 = 36;
pub const BOTON_HUECO: u32 = 8;
pub const BOTON_MARGEN: u32 = 14;
pub const BOTON_RADIO: u32 = 6;
pub const BOTON_TAM: f32 = 13.0;

/// El cuadro ya colocado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dialogo {
    /// Todo el cuadro.
    pub caja: Rect,
    pub cabecera: Rect,
    /// Donde van las filas de fichero (y, si hay, la linea del resto).
    pub lista: Rect,
    pub pie: Rect,
    pub cancelar: Rect,
    pub aceptar: Rect,
    /// Cuantas filas se ensenan de verdad.
    pub filas: usize,
    /// Cuantos ficheros no caben y se resumen en una linea.
    pub resto: usize,
}

/// Coloca el cuadro centrado en la ventana.
pub fn colocar(ventana: Rect, cuantos: usize, escala_por_cien: u32) -> Dialogo {
    let e = |v: u32| v * escala_por_cien / 100;
    let filas = cuantos.min(FILAS_MAXIMAS);
    let resto = cuantos.saturating_sub(filas);
    // La linea del resto ocupa media fila: dice un numero, no ensena un
    // fichero.
    let alto_lista = e(FILA) * filas as u32 + if resto > 0 { e(FILA) / 2 } else { 0 };
    let alto_deseado = e(CABECERA) + alto_lista + e(PIE_ALTO) + e(BOTONES_ALTO);

    // Nunca mas grande que la ventana menos su margen: en una ventana
    // pequena vale mas un cuadro apretado que uno que se sale.
    let ancho = e(ANCHO).min(ventana.ancho.saturating_sub(2 * e(MARGEN_VENTANA)));
    let alto = alto_deseado.min(ventana.alto.saturating_sub(2 * e(MARGEN_VENTANA)));
    let caja = Rect {
        x: ventana.x + (ventana.ancho as i32 - ancho as i32) / 2,
        y: ventana.y + (ventana.alto as i32 - alto as i32) / 2,
        ancho,
        alto,
    };

    let cabecera = Rect {
        alto: e(CABECERA).min(caja.alto),
        ..caja
    };
    // De abajo hacia arriba: los botones y el pie tienen sitio fijo y lo que
    // sobra es para la lista. Al reves, con la ventana muy baja, los botones
    // se saldrian por debajo y no habria como cancelar.
    let botones = Rect {
        x: caja.x,
        y: caja.abajo() - e(BOTONES_ALTO).min(caja.alto) as i32,
        ancho: caja.ancho,
        alto: e(BOTONES_ALTO).min(caja.alto),
    };
    let pie = Rect {
        x: caja.x,
        y: (botones.y - e(PIE_ALTO) as i32).max(cabecera.abajo()),
        ancho: caja.ancho,
        alto: e(PIE_ALTO),
    };
    let arriba = cabecera.abajo();
    let lista = Rect {
        x: caja.x,
        y: arriba,
        ancho: caja.ancho,
        alto: (pie.y - arriba).max(0) as u32,
    };

    // Los dos botones, a la derecha y del mismo ancho: ninguno de los dos es
    // tan raro como para merecer mas sitio que el otro.
    let ancho_boton = caja
        .ancho
        .saturating_sub(2 * e(BOTON_MARGEN) + e(BOTON_HUECO))
        / 2;
    let alto_boton = e(BOTON_ALTO).min(botones.alto);
    let y = botones.y + (botones.alto as i32 - alto_boton as i32) / 2;
    let aceptar = Rect {
        x: caja.derecha() - e(BOTON_MARGEN) as i32 - ancho_boton as i32,
        y,
        ancho: ancho_boton,
        alto: alto_boton,
    };
    let cancelar = Rect {
        x: aceptar.x - e(BOTON_HUECO) as i32 - ancho_boton as i32,
        ..aceptar
    };

    Dialogo {
        caja,
        cabecera,
        lista,
        pie,
        cancelar,
        aceptar,
        filas,
        resto,
    }
}

impl Dialogo {
    /// La fila numero `indice`, contando desde arriba de la lista.
    pub fn fila(&self, indice: usize, escala_por_cien: u32) -> Rect {
        let alto = FILA * escala_por_cien / 100;
        Rect {
            x: self.lista.x,
            y: self.lista.y + (indice as u32 * alto) as i32,
            ancho: self.lista.ancho,
            alto,
        }
    }

    /// La linea que dice cuantos ficheros mas hay, si los hay.
    pub fn linea_resto(&self, escala_por_cien: u32) -> Option<Rect> {
        if self.resto == 0 {
            return None;
        }
        let alto = (FILA * escala_por_cien / 100) / 2;
        Some(Rect {
            x: self.lista.x,
            y: self.fila(self.filas, escala_por_cien).y,
            ancho: self.lista.ancho,
            alto,
        })
    }

    /// La miniatura de una fila, centrada en su carril.
    pub fn miniatura(&self, fila: Rect, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let lado = e(MINIATURA);
        Rect {
            x: fila.x + e(FILA_X) as i32,
            y: fila.y + (fila.alto as i32 - lado as i32) / 2,
            ancho: lado,
            alto: lado,
        }
    }

    /// Donde empieza el texto de una fila, y cuanto sitio tiene.
    pub fn texto(&self, fila: Rect, escala_por_cien: u32) -> (i32, u32) {
        let e = |v: u32| v * escala_por_cien / 100;
        let x = fila.x + e(NOMBRE_X) as i32;
        let ancho = (fila.derecha() - e(FILA_X) as i32 - x).max(0) as u32;
        (x, ancho)
    }

    /// Que boton hay bajo el punto, si hay alguno.
    pub fn boton_en(&self, p: Punto) -> Option<Boton> {
        if self.aceptar.contiene(p) {
            Some(Boton::Aceptar)
        } else if self.cancelar.contiene(p) {
            Some(Boton::Cancelar)
        } else {
            None
        }
    }
}

/// Las dos salidas del cuadro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boton {
    Cancelar,
    Aceptar,
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ventana() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1024,
            alto: 768,
        }
    }

    #[test]
    fn el_cuadro_se_centra_y_sus_partes_van_en_orden() {
        let d = colocar(ventana(), 1, 100);
        assert_eq!(d.caja.ancho, ANCHO);
        // Centrado: sobra lo mismo a cada lado, y arriba y abajo.
        assert_eq!(d.caja.x, ventana().derecha() - d.caja.derecha());
        assert_eq!(d.caja.y, ventana().abajo() - d.caja.abajo());
        // De arriba abajo: cabecera, lista, pie, botones. Sin huecos.
        assert_eq!(d.cabecera.y, d.caja.y);
        assert_eq!(d.lista.y, d.cabecera.abajo());
        assert_eq!(d.pie.y, d.lista.abajo());
        assert!(d.aceptar.abajo() <= d.caja.abajo());
        assert_eq!((d.filas, d.resto), (1, 0));
        assert!(d.linea_resto(100).is_none(), "con uno no sobra ninguno");
    }

    #[test]
    fn con_muchos_ficheros_se_ensenan_cuatro_y_se_cuenta_el_resto() {
        let d = colocar(ventana(), 20, 100);
        assert_eq!(d.filas, FILAS_MAXIMAS);
        assert_eq!(d.resto, 16);
        let resto = d.linea_resto(100).expect("sobran dieciseis");
        // La linea del resto va justo despues de la ultima fila y cabe.
        assert_eq!(resto.y, d.fila(FILAS_MAXIMAS - 1, 100).abajo());
        assert!(resto.abajo() <= d.pie.y, "no se mete en el pie");
    }

    #[test]
    fn los_dos_botones_miden_lo_mismo_y_van_a_la_derecha() {
        let d = colocar(ventana(), 2, 100);
        assert_eq!(d.cancelar.ancho, d.aceptar.ancho);
        assert_eq!(d.aceptar.derecha(), d.caja.derecha() - BOTON_MARGEN as i32);
        // En ese orden y sin tocarse: aceptar a la derecha del todo.
        assert_eq!(d.cancelar.derecha() + BOTON_HUECO as i32, d.aceptar.x);
        assert!(d.cancelar.x >= d.caja.x);
        // Y se aciertan al pulsarlos.
        let dentro = |r: Rect| Punto {
            x: r.x + 4,
            y: r.y + 4,
        };
        assert_eq!(d.boton_en(dentro(d.aceptar)), Some(Boton::Aceptar));
        assert_eq!(d.boton_en(dentro(d.cancelar)), Some(Boton::Cancelar));
        // Caso negativo: la cabecera no es ningun boton.
        assert_eq!(d.boton_en(dentro(d.cabecera)), None);
    }

    #[test]
    fn en_una_ventana_pequena_el_cuadro_cabe_y_los_botones_siguen_dentro() {
        // Caso negativo del centrado: si el cuadro no se apretara se saldria,
        // y entonces no habria como cancelar.
        let pequena = Rect {
            x: 0,
            y: 0,
            ancho: 300,
            alto: 240,
        };
        let d = colocar(pequena, 20, 100);
        assert!(d.caja.ancho <= pequena.ancho, "{:?}", d.caja);
        assert!(d.caja.alto <= pequena.alto);
        assert!(d.caja.x >= pequena.x && d.caja.derecha() <= pequena.derecha());
        assert!(d.aceptar.abajo() <= d.caja.abajo(), "los botones, dentro");
        assert!(d.aceptar.y >= d.caja.y);
        assert!(d.cancelar.x >= d.caja.x);
    }

    #[test]
    fn la_miniatura_y_el_texto_de_una_fila_no_se_pisan() {
        let d = colocar(ventana(), 3, 100);
        let f = d.fila(0, 100);
        let m = d.miniatura(f, 100);
        assert_eq!((m.ancho, m.alto), (MINIATURA, MINIATURA));
        // Centrada en vertical dentro de su fila.
        assert_eq!(m.y - f.y, f.abajo() - m.abajo());
        let (x, ancho) = d.texto(f, 100);
        assert!(x >= m.derecha(), "el texto empieza despues del carril");
        assert!(ancho > 0);
        assert!(
            x + ancho as i32 <= f.derecha(),
            "y no se sale por la derecha"
        );
    }
}
