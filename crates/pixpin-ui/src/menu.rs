//! Un menu desplegable, como el del clip de Telegram.
//!
//! Geometria pura. Las medidas salen del analisis de Telegram Desktop; **de
//! ahi solo medidas y tecnicas, nunca su codigo, que es GPL-3.0.**
//!
//! Crece HACIA ARRIBA desde su ancla, porque el boton que lo abre esta abajo
//! del todo: un menu que creciera hacia abajo se saldria de la ventana. Y si
//! aun asi no cabe arriba, se pega al borde en vez de salirse: es preferible
//! taparle algo a la conversacion a que la ultima entrada quede fuera de la
//! pantalla y no se pueda pulsar.

use pixpin_geom::{Punto, Rect};

/// Medidas en pixeles logicos (al 100 %).
pub const ANCHO_MINIMO: u32 = 156;
pub const ANCHO_MAXIMO: u32 = 300;
/// Relleno de una fila: 54 por la izquierda, que es donde va el icono.
pub const RELLENO_IZQUIERDA: u32 = 54;
pub const RELLENO_DERECHA: u32 = 17;
pub const RELLENO_VERTICAL: u32 = 8;
/// El icono, y donde empieza dentro de su fila.
pub const ICONO: u32 = 24;
pub const ICONO_X: u32 = 15;
pub const TEXTO_TAM: f32 = 13.0;
/// El relleno del contenedor, alrededor de las filas.
pub const RELLENO_CAJA: u32 = 10;
pub const RADIO: u32 = 8;

/// Que se puede adjuntar. Las tres primeras son las de Telegram; la ultima es
/// de PixPin, porque un proyecto se alimenta sobre todo de lienzos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entrada {
    Imagen,
    Archivo,
    Lienzo,
    Tabla,
    /// Traer algo del movil por la wifi: se ensena un codigo y el movil lo
    /// escanea. Va en este menu porque es otra forma de meter cosas en el
    /// proyecto, que es lo que hace el clip.
    DelMovil,
}

impl Entrada {
    pub const TODAS: [Entrada; 5] = [
        Entrada::Imagen,
        Entrada::Archivo,
        Entrada::Lienzo,
        Entrada::Tabla,
        Entrada::DelMovil,
    ];

    /// La clave de su rotulo traducido.
    pub fn clave(self) -> &'static str {
        match self {
            Entrada::Imagen => "adjuntar-imagen",
            Entrada::Archivo => "adjuntar-archivo",
            Entrada::Lienzo => "adjuntar-lienzo",
            Entrada::Tabla => "adjuntar-tabla",
            Entrada::DelMovil => "adjuntar-del-movil",
        }
    }
}

/// Un menu ya colocado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Menu {
    /// Todo el recuadro, con su relleno.
    pub caja: Rect,
    /// Lo alto que es cada fila.
    pub alto_fila: u32,
    pub cuantas: usize,
}

/// Lo alto que es una fila con una letra de `alto_texto`.
pub fn alto_fila(alto_texto: u32, escala_por_cien: u32) -> u32 {
    alto_texto + 2 * (RELLENO_VERTICAL * escala_por_cien / 100)
}

/// Coloca el menu encima de su ancla.
///
/// `ancla` es el boton que lo abre; el menu apoya su base en el borde
/// superior de ese boton y se alinea por su izquierda. `anchos_de_texto` los
/// mide quien pinta, que es quien tiene la fuente.
pub fn desplegar(
    ancla: Rect,
    limite: Rect,
    anchos_de_texto: &[f32],
    alto_texto: u32,
    escala_por_cien: u32,
) -> Menu {
    let e = |v: u32| v * escala_por_cien / 100;
    let cuantas = anchos_de_texto.len();
    let fila = alto_fila(alto_texto, escala_por_cien);

    // El ancho lo manda el rotulo mas largo, entre el minimo y el maximo.
    let mas_largo = anchos_de_texto.iter().fold(0.0f32, |a, b| a.max(*b));
    let natural = mas_largo.max(0.0).ceil() as u32 + e(RELLENO_IZQUIERDA) + e(RELLENO_DERECHA);
    let ancho = natural.clamp(e(ANCHO_MINIMO), e(ANCHO_MAXIMO));
    let alto = fila * cuantas as u32 + 2 * e(RELLENO_CAJA);

    // Hacia arriba desde el ancla; si no cabe, pegado al borde de arriba.
    Menu {
        caja: Rect {
            x: ancla.x.min(limite.derecha() - ancho as i32).max(limite.x),
            y: (ancla.y - alto as i32).max(limite.y),
            ancho,
            alto,
        },
        alto_fila: fila,
        cuantas,
    }
}

impl Menu {
    /// La fila numero `indice`.
    pub fn fila(&self, indice: usize, escala_por_cien: u32) -> Rect {
        let relleno = RELLENO_CAJA * escala_por_cien / 100;
        Rect {
            x: self.caja.x + relleno as i32,
            y: self.caja.y + relleno as i32 + (indice as u32 * self.alto_fila) as i32,
            ancho: self.caja.ancho.saturating_sub(2 * relleno),
            alto: self.alto_fila,
        }
    }

    /// El icono de una fila, ya centrado en su carril.
    pub fn icono(&self, fila: Rect, escala_por_cien: u32) -> Rect {
        let lado = ICONO * escala_por_cien / 100;
        Rect {
            x: fila.x + (ICONO_X * escala_por_cien / 100) as i32,
            y: fila.y + (fila.alto as i32 - lado as i32) / 2,
            ancho: lado,
            alto: lado,
        }
    }

    /// Donde empieza el rotulo de una fila.
    pub fn texto(&self, fila: Rect, escala_por_cien: u32) -> i32 {
        fila.x + (RELLENO_IZQUIERDA * escala_por_cien / 100) as i32
    }

    /// Que fila hay bajo el punto, si hay alguna.
    pub fn fila_en(&self, p: Punto, escala_por_cien: u32) -> Option<usize> {
        if !self.caja.contiene(p) {
            return None;
        }
        (0..self.cuantas).find(|n| self.fila(*n, escala_por_cien).contiene(p))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn limite() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1024,
            alto: 768,
        }
    }

    fn clip() -> Rect {
        // Un boton abajo del todo, como el del clip junto a la caja.
        Rect {
            x: 560,
            y: 700,
            ancho: 44,
            alto: 46,
        }
    }

    fn anchos() -> Vec<f32> {
        vec![60.0, 50.0, 90.0]
    }

    #[test]
    fn el_menu_crece_hacia_arriba_desde_su_boton() {
        let m = desplegar(clip(), limite(), &anchos(), 18, 100);
        // Apoya su base en el borde de arriba del boton: hacia abajo se
        // saldria de la ventana, que es donde esta el boton.
        assert_eq!(m.caja.abajo(), clip().y);
        assert_eq!(m.caja.x, clip().x, "alineado por la izquierda");
        assert_eq!(m.cuantas, 3);
        // Las filas caben dentro, en orden y sin pisarse.
        for n in 0..3 {
            let f = m.fila(n, 100);
            assert!(f.y >= m.caja.y && f.abajo() <= m.caja.abajo(), "{f:?}");
            if n > 0 {
                assert_eq!(f.y, m.fila(n - 1, 100).abajo());
            }
        }
    }

    #[test]
    fn el_ancho_lo_manda_el_rotulo_mas_largo_entre_sus_topes() {
        // Rotulos cortos: el minimo, no una tira estrecha.
        let m = desplegar(clip(), limite(), &[10.0, 12.0], 18, 100);
        assert_eq!(m.caja.ancho, ANCHO_MINIMO);
        // Uno larguisimo: el maximo, no se estira sin fin.
        let m = desplegar(clip(), limite(), &[900.0], 18, 100);
        assert_eq!(m.caja.ancho, ANCHO_MAXIMO);
        // Uno intermedio manda: el mas largo mas los dos rellenos.
        let m = desplegar(clip(), limite(), &[120.0, 40.0], 18, 100);
        assert_eq!(m.caja.ancho, 120 + RELLENO_IZQUIERDA + RELLENO_DERECHA);
    }

    #[test]
    fn si_no_cabe_arriba_se_pega_al_borde_en_vez_de_salirse() {
        // Caso negativo: el ancla casi arriba del todo y muchas entradas.
        let ancla = Rect {
            x: 10,
            y: 40,
            ancho: 44,
            alto: 46,
        };
        let muchos: Vec<f32> = (0..20).map(|_| 60.0).collect();
        let m = desplegar(ancla, limite(), &muchos, 18, 100);
        assert_eq!(m.caja.y, limite().y, "pegado arriba");
        assert!(m.caja.abajo() > ancla.y, "y por eso tapa parte del ancla");
    }

    #[test]
    fn un_menu_pegado_al_borde_derecho_se_mete_hacia_dentro() {
        let ancla = Rect {
            x: limite().derecha() - 20,
            y: 700,
            ancho: 44,
            alto: 46,
        };
        let m = desplegar(ancla, limite(), &anchos(), 18, 100);
        assert!(m.caja.derecha() <= limite().derecha(), "{:?}", m.caja);
    }

    #[test]
    fn se_acierta_la_fila_que_se_pulsa_y_el_relleno_no_cuenta() {
        let m = desplegar(clip(), limite(), &anchos(), 18, 100);
        let segunda = m.fila(1, 100);
        let dentro = Punto {
            x: segunda.x + 5,
            y: segunda.y + 5,
        };
        assert_eq!(m.fila_en(dentro, 100), Some(1));
        // El relleno de arriba del recuadro no es ninguna fila.
        let arriba = Punto {
            x: m.caja.x + 5,
            y: m.caja.y + 2,
        };
        assert_eq!(m.fila_en(arriba, 100), None);
        // Y fuera del menu, nada.
        let fuera = Punto {
            x: m.caja.x - 5,
            y: segunda.y,
        };
        assert_eq!(m.fila_en(fuera, 100), None);
    }

    #[test]
    fn el_icono_va_en_su_carril_y_el_texto_detras() {
        let m = desplegar(clip(), limite(), &anchos(), 18, 100);
        let f = m.fila(0, 100);
        let i = m.icono(f, 100);
        assert_eq!(i.ancho, ICONO);
        assert_eq!(i.alto, ICONO);
        // Centrado en vertical dentro de la fila.
        assert_eq!(i.y - f.y, f.abajo() - i.abajo());
        // Y el texto empieza despues del carril del icono.
        assert!(m.texto(f, 100) > i.derecha());
    }

    #[test]
    fn cada_entrada_tiene_su_propia_clave() {
        let claves: std::collections::BTreeSet<&str> =
            Entrada::TODAS.iter().map(|e| e.clave()).collect();
        assert_eq!(claves.len(), Entrada::TODAS.len(), "ninguna se repite");
        assert!(!claves.contains(""), "ninguna se queda sin rotulo");
    }
}
