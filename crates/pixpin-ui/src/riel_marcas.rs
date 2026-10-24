//! **El riel de marcadores del lienzo** (F5), como logica pura: donde va cada
//! cosa y que hay bajo el raton. Puerto de `ui/RielDeMarcas.kt` del movil.
//!
//! En el movil el riel es comun a los tres sitios (lienzo, PDF y libros) y
//! va **siempre a la derecha**: a la izquierda esta el panel de estilo y
//! debajo la barra. En el PC es igual: el panel lateral del editor va a la
//! izquierda y la barra arriba en el centro, asi que la derecha esta libre.
//!
//! Tres piezas:
//!
//! - **La isla de botones**, arriba a la derecha: poner una marca y sacar la
//!   hojita (F6). Es lo que en el movil son el boton «Marcador aqui» y el
//!   gesto de tres dedos; aqui hacen falta botones porque sin ellos solo lo
//!   encontraria quien se sepa los atajos.
//! - **Los puntos**, uno por marca y en el orden del dibujo (de arriba
//!   abajo), debajo de la isla. En reposo pequenos; el que tiene el raton
//!   encima crece y **se adelanta hacia dentro de la pantalla**, que es lo que
//!   hace el movil con el dedo («debajo del dedo no se veia cual era»).
//! - **La tira de emoticonos**, cuando se esta poniendo una marca: justo
//!   debajo de la barra de herramientas. En el movil salian detras de la
//!   barra y no se podian tocar (v0.81.2); aqui se colocan contra ella y no
//!   debajo de nada.
//!
//! Todo va en pixeles de la VENTANA (0,0 en su esquina), no del escritorio:
//! quien llama resta el origen de la ventana al punto del raton.

use pixpin_geom::{Punto, Rect};

/// Lado de un boton de la isla, en pixeles logicos: el de la barra.
const LADO_BOTON: u32 = 36;
/// Aire entre el borde de la ventana y la isla.
const BORDE: u32 = 16;
/// Relleno de la isla alrededor de sus botones.
const RELLENO: u32 = 4;
/// Lo que ocupa cada punto a lo alto. Menos que en el movil (38 dp): un
/// raton apunta mejor que un dedo, y asi caben las 24 en un portatil.
const PASO: u32 = 30;
/// Diametro de un punto en reposo, con el raton en el riel y el elegido.
pub const PUNTO_REPOSO: u32 = 20;
pub const PUNTO_CERCA: u32 = 26;
pub const PUNTO_ELEGIDO: u32 = 44;
/// Cuanto se adelanta el elegido hacia dentro de la pantalla.
pub const SALTO: u32 = 40;
/// Una celda de la tira de emoticonos.
const CELDA_TIRA: u32 = 40;

/// Lo que hay bajo un punto de la ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinoRiel {
    /// El boton de poner una marca.
    PonerMarca,
    /// El boton de la hojita.
    Hojita,
    /// El marco para imprimir (`marco_imprimir` de la app).
    Imprimir,
    /// El punto de la marca `i` (en el orden del riel).
    Marca(usize),
    /// El emoticono `i` de la tira.
    Emoji(usize),
    /// La cruz de la tira: dejar de poner la marca.
    CerrarTira,
    /// Dentro del riel o de la tira pero fuera de todo boton: no es del
    /// lienzo (un clic ahi no puede dejar un punto de tinta), pero no hace
    /// nada.
    Hueco,
    /// Fuera: le toca al lienzo.
    Fuera,
}

/// El riel ya colocado para una ventana, una escala y un numero de marcas.
#[derive(Debug, Clone, PartialEq)]
pub struct Riel {
    /// La isla de los tres botones.
    pub isla: Rect,
    pub poner_marca: Rect,
    pub hojita: Rect,
    /// El marco para imprimir, debajo del lapiz.
    pub imprimir: Rect,
    /// La columna de los puntos (vacia sin marcas).
    pub columna: Rect,
    /// El centro de cada punto, en el orden del riel.
    pub puntos: Vec<Punto>,
    /// La tira de emoticonos, si esta abierta: su marco, cada celda y la
    /// cruz del final.
    pub tira: Option<Tira>,
    pub escala_por_cien: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tira {
    pub marco: Rect,
    pub celdas: Vec<Rect>,
    pub cerrar: Rect,
}

fn e(v: u32, escala_por_cien: u32) -> u32 {
    (v * escala_por_cien.max(1) / 100).max(1)
}

impl Riel {
    /// Coloca el riel en una ventana de `ancho` x `alto`. `bajo_la_barra` es
    /// la y (de ventana) del borde de abajo de la barra de herramientas: la
    /// tira de emoticonos se pone ahi. `emojis` es cuantos lleva la tira.
    pub fn colocar(
        ancho: u32,
        alto: u32,
        escala_por_cien: u32,
        cuantas: usize,
        tira_abierta: bool,
        emojis: usize,
        bajo_la_barra: i32,
    ) -> Riel {
        let px = |v: u32| e(v, escala_por_cien);
        let lado = px(LADO_BOTON);
        let relleno = px(RELLENO);
        let ancho_isla = lado + 2 * relleno;
        let isla = Rect {
            x: ancho as i32 - (px(BORDE) + ancho_isla) as i32,
            y: px(BORDE) as i32,
            ancho: ancho_isla,
            alto: 3 * lado + 2 * relleno,
        };
        let poner_marca = Rect {
            x: isla.x + relleno as i32,
            y: isla.y + relleno as i32,
            ancho: lado,
            alto: lado,
        };
        let hojita = Rect {
            y: poner_marca.abajo(),
            ..poner_marca
        };
        let imprimir = Rect {
            y: hojita.abajo(),
            ..poner_marca
        };
        // Los puntos, debajo de la isla. Si no caben al paso de siempre se
        // aprietan: las 24 tienen que verse enteras, porque una marca que no
        // se ve en el riel es una marca que no existe.
        let arriba = isla.abajo() + px(12) as i32;
        let hueco = (alto as i32 - arriba - px(BORDE) as i32).max(0) as u32;
        let paso = if cuantas == 0 {
            px(PASO)
        } else {
            px(PASO).min(hueco / cuantas as u32).max(1)
        };
        let centro_x = isla.x + (ancho_isla / 2) as i32;
        let puntos: Vec<Punto> = (0..cuantas)
            .map(|i| Punto {
                x: centro_x,
                y: arriba + (paso * i as u32 + paso / 2) as i32,
            })
            .collect();
        let columna = if cuantas == 0 {
            Rect {
                x: isla.x,
                y: arriba,
                ancho: 0,
                alto: 0,
            }
        } else {
            Rect {
                x: isla.x,
                y: arriba,
                ancho: ancho_isla,
                alto: paso * cuantas as u32,
            }
        };
        let tira = tira_abierta.then(|| {
            let celda = px(CELDA_TIRA);
            let n = emojis as u32 + 1;
            let ancho_tira = n * celda + 2 * relleno;
            let marco = Rect {
                x: (ancho as i32 - ancho_tira as i32) / 2,
                y: bajo_la_barra + px(8) as i32,
                ancho: ancho_tira,
                alto: celda + 2 * relleno,
            };
            let celda_en = |i: u32| Rect {
                x: marco.x + (relleno + i * celda) as i32,
                y: marco.y + relleno as i32,
                ancho: celda,
                alto: celda,
            };
            Tira {
                marco,
                celdas: (0..emojis as u32).map(celda_en).collect(),
                cerrar: celda_en(emojis as u32),
            }
        });
        Riel {
            isla,
            poner_marca,
            hojita,
            imprimir,
            columna,
            puntos,
            tira,
            escala_por_cien,
        }
    }

    /// Que hay bajo `p` (pixeles de ventana).
    pub fn destino(&self, p: Punto) -> DestinoRiel {
        if let Some(t) = &self.tira {
            if let Some(i) = t.celdas.iter().position(|c| c.contiene(p)) {
                return DestinoRiel::Emoji(i);
            }
            if t.cerrar.contiene(p) {
                return DestinoRiel::CerrarTira;
            }
            if t.marco.contiene(p) {
                return DestinoRiel::Hueco;
            }
        }
        if self.poner_marca.contiene(p) {
            return DestinoRiel::PonerMarca;
        }
        if self.hojita.contiene(p) {
            return DestinoRiel::Hojita;
        }
        if self.imprimir.contiene(p) {
            return DestinoRiel::Imprimir;
        }
        if self.isla.contiene(p) {
            return DestinoRiel::Hueco;
        }
        if let Some(i) = self.marca_en(p) {
            return DestinoRiel::Marca(i);
        }
        DestinoRiel::Fuera
    }

    /// El punto de la columna a esa altura, como `Lectura.puntoBajoElDedo`
    /// del movil: dentro de la columna, el mas cercano. Se reparte la altura
    /// entera y no solo el redondel, porque apuntar a un punto de 20 px con
    /// el raton en marcha es mas dificil de lo que parece.
    pub fn marca_en(&self, p: Punto) -> Option<usize> {
        if self.puntos.is_empty() || !self.columna.contiene(p) {
            return None;
        }
        self.puntos
            .iter()
            .enumerate()
            .min_by_key(|(_, c)| (c.y - p.y).abs())
            .map(|(i, _)| i)
    }

    /// Lo que ocupa todo lo que pinta el riel: sirve para saber que hay que
    /// repintar la interfaz al cambiar lo que tiene el raton encima.
    pub fn contiene(&self, p: Punto) -> bool {
        !matches!(self.destino(p), DestinoRiel::Fuera)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn riel(cuantas: usize, tira: bool) -> Riel {
        Riel::colocar(1600, 900, 100, cuantas, tira, 12, 60)
    }

    #[test]
    fn el_riel_va_a_la_derecha_y_no_se_sale_de_la_ventana() {
        let r = riel(3, false);
        assert!(r.isla.derecha() <= 1600);
        assert!(
            r.isla.x > 1600 - 100,
            "pegado al borde derecho: {:?}",
            r.isla
        );
        for p in &r.puntos {
            assert!(p.x > 1500 && p.y < 900);
        }
    }

    #[test]
    fn cada_boton_y_cada_punto_dicen_lo_que_son() {
        let r = riel(3, false);
        let centro = |c: Rect| Punto {
            x: c.x + c.ancho as i32 / 2,
            y: c.y + c.alto as i32 / 2,
        };
        assert_eq!(r.destino(centro(r.poner_marca)), DestinoRiel::PonerMarca);
        assert_eq!(r.destino(centro(r.hojita)), DestinoRiel::Hojita);
        assert_eq!(r.destino(centro(r.imprimir)), DestinoRiel::Imprimir);
        for (i, p) in r.puntos.iter().enumerate() {
            assert_eq!(r.destino(*p), DestinoRiel::Marca(i));
        }
    }

    #[test]
    fn el_lienzo_sigue_siendo_del_lienzo() {
        let r = riel(3, false);
        assert_eq!(r.destino(Punto { x: 800, y: 450 }), DestinoRiel::Fuera);
        // Debajo de la ultima marca ya no hay riel.
        let ultima = *r.puntos.last().unwrap();
        assert_eq!(
            r.destino(Punto {
                x: ultima.x,
                y: ultima.y + 200
            }),
            DestinoRiel::Fuera
        );
        // Sin marcas, la columna no coge nada.
        let vacio = riel(0, false);
        assert!(vacio.puntos.is_empty());
        assert_eq!(
            vacio.destino(Punto {
                x: vacio.isla.x + 10,
                y: vacio.isla.abajo() + 30
            }),
            DestinoRiel::Fuera
        );
    }

    #[test]
    fn las_veinticuatro_caben_en_una_ventana_baja() {
        let r = Riel::colocar(1366, 500, 100, 24, false, 12, 60);
        assert_eq!(r.puntos.len(), 24);
        assert!(r.puntos.last().unwrap().y < 500);
        // Y cada una sigue siendo alcanzable por separado.
        for (i, p) in r.puntos.iter().enumerate() {
            assert_eq!(r.marca_en(*p), Some(i));
        }
    }

    #[test]
    fn la_tira_sale_debajo_de_la_barra_y_no_detras() {
        let r = riel(0, true);
        let t = r.tira.as_ref().unwrap();
        assert!(
            t.marco.y >= 60,
            "la barra acaba en 60 y la tira empieza en {}",
            t.marco.y
        );
        assert_eq!(t.celdas.len(), 12);
        let c = t.celdas[4];
        assert_eq!(
            r.destino(Punto {
                x: c.x + 5,
                y: c.y + 5
            }),
            DestinoRiel::Emoji(4)
        );
        assert_eq!(
            r.destino(Punto {
                x: t.cerrar.x + 5,
                y: t.cerrar.y + 5
            }),
            DestinoRiel::CerrarTira
        );
        // Cerrada, ahi vuelve a estar el lienzo.
        let cerrada = riel(0, false);
        assert_eq!(
            cerrada.destino(Punto {
                x: c.x + 5,
                y: c.y + 5
            }),
            DestinoRiel::Fuera
        );
    }

    #[test]
    fn a_doble_escala_todo_mide_el_doble() {
        let uno = Riel::colocar(3000, 2000, 100, 2, false, 12, 60);
        let dos = Riel::colocar(3000, 2000, 200, 2, false, 12, 60);
        assert_eq!(dos.poner_marca.ancho, uno.poner_marca.ancho * 2);
    }
}
