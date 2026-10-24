//! **La hojita** (F6) como logica pura: donde va el papel y sus mandos.
//!
//! En el movil sale **debajo de la barra**, que sube para dejarle sitio, y es
//! de tamano limitado: «ni inmensa ni diminuta». Aqui igual: una tarjeta
//! centrada bajo la barra de herramientas con el papel arriba y una fila de
//! mandos debajo —los cinco papeles, borrar todo, insertar en el lienzo,
//! pegar como pin y cerrar—. Los mandos van en la misma tarjeta y no en la
//! barra del editor porque son de la hojita: al cerrarla se van con ella.
//!
//! Pixeles de VENTANA, como el riel (`riel_marcas`).

use pixpin_geom::{Punto, Rect};

/// El papel, en pixeles logicos. Cabe un recado de cinco lineas a mano y no
/// tapa medio lienzo en un portatil.
pub const ANCHO_PAPEL: u32 = 480;
pub const ALTO_PAPEL: u32 = 320;
const RELLENO: u32 = 8;
const ALTO_MANDOS: u32 = 36;
const LADO_PAPEL: u32 = 26;
const ANCHO_BOTON: u32 = 88;

/// Lo que hay bajo un punto de la ventana con la hojita abierta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinoHojita {
    /// El papel: ahi se dibuja.
    Papel,
    /// La muestra del papel `i` (`pixpin_motor2d::hojita::PAPELES`).
    ColorPapel(usize),
    BorrarTodo,
    Insertar,
    PegarComoPin,
    Cerrar,
    /// La tarjeta, fuera de todo mando.
    Hueco,
    Fuera,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hojita {
    pub tarjeta: Rect,
    pub papel: Rect,
    pub muestras: Vec<Rect>,
    pub borrar: Rect,
    pub insertar: Rect,
    pub pegar: Rect,
    pub cerrar: Rect,
    pub escala_por_cien: u32,
}

impl Hojita {
    /// Coloca la hojita en una ventana de `ancho` x `alto`, bajo la barra
    /// (`bajo_la_barra`, la y de su borde de abajo). En una ventana muy
    /// estrecha el papel se encoge, pero nunca por debajo de la mitad: una
    /// hojita donde no cabe una palabra no sirve.
    pub fn colocar(
        ancho: u32,
        alto: u32,
        escala_por_cien: u32,
        papeles: usize,
        bajo_la_barra: i32,
    ) -> Hojita {
        let px = |v: u32| (v * escala_por_cien.max(1) / 100).max(1);
        let relleno = px(RELLENO);
        let ancho_papel = px(ANCHO_PAPEL)
            .min(ancho.saturating_sub(4 * relleno))
            .max(px(ANCHO_PAPEL) / 2);
        let alto_papel = px(ALTO_PAPEL)
            .min((alto as i32 - bajo_la_barra - px(ALTO_MANDOS + 40) as i32).max(0) as u32)
            .max(px(ALTO_PAPEL) / 2);
        let tarjeta = Rect {
            x: (ancho as i32 - (ancho_papel + 2 * relleno) as i32) / 2,
            y: bajo_la_barra + px(8) as i32,
            ancho: ancho_papel + 2 * relleno,
            alto: alto_papel + px(ALTO_MANDOS) + 3 * relleno,
        };
        let papel = Rect {
            x: tarjeta.x + relleno as i32,
            y: tarjeta.y + relleno as i32,
            ancho: ancho_papel,
            alto: alto_papel,
        };
        let fila_y = papel.abajo() + relleno as i32;
        let alto_fila = px(ALTO_MANDOS);
        let lado = px(LADO_PAPEL);
        let muestras: Vec<Rect> = (0..papeles as u32)
            .map(|i| Rect {
                x: papel.x + (i * (lado + px(4))) as i32,
                y: fila_y + ((alto_fila - lado) / 2) as i32,
                ancho: lado,
                alto: lado,
            })
            .collect();
        // Los botones, de derecha a izquierda desde la cruz: asi la cruz queda
        // en la esquina, donde se busca siempre.
        let cerrar = Rect {
            x: papel.derecha() - alto_fila as i32,
            y: fila_y,
            ancho: alto_fila,
            alto: alto_fila,
        };
        let boton = |derecha: i32| Rect {
            x: derecha - px(ANCHO_BOTON) as i32,
            y: fila_y,
            ancho: px(ANCHO_BOTON),
            alto: alto_fila,
        };
        let pegar = boton(cerrar.x - px(4) as i32);
        let insertar = boton(pegar.x - px(4) as i32);
        let borrar = boton(insertar.x - px(4) as i32);
        Hojita {
            tarjeta,
            papel,
            muestras,
            borrar,
            insertar,
            pegar,
            cerrar,
            escala_por_cien,
        }
    }

    pub fn destino(&self, p: Punto) -> DestinoHojita {
        if self.papel.contiene(p) {
            return DestinoHojita::Papel;
        }
        if let Some(i) = self.muestras.iter().position(|m| m.contiene(p)) {
            return DestinoHojita::ColorPapel(i);
        }
        for (r, d) in [
            (self.borrar, DestinoHojita::BorrarTodo),
            (self.insertar, DestinoHojita::Insertar),
            (self.pegar, DestinoHojita::PegarComoPin),
            (self.cerrar, DestinoHojita::Cerrar),
        ] {
            if r.contiene(p) {
                return d;
            }
        }
        if self.tarjeta.contiene(p) {
            DestinoHojita::Hueco
        } else {
            DestinoHojita::Fuera
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn centro(r: Rect) -> Punto {
        Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        }
    }

    #[test]
    fn la_hojita_sale_bajo_la_barra_centrada_y_dentro_de_la_ventana() {
        let h = Hojita::colocar(1920, 1080, 100, 5, 64);
        assert!(h.tarjeta.y >= 64);
        assert!(h.tarjeta.abajo() < 1080);
        let izquierda = h.tarjeta.x;
        let derecha = 1920 - h.tarjeta.derecha();
        assert!(
            (izquierda - derecha).abs() <= 1,
            "centrada: {izquierda} y {derecha}"
        );
    }

    #[test]
    fn cada_mando_dice_lo_que_es_y_no_se_pisan() {
        let h = Hojita::colocar(1920, 1080, 150, 5, 64);
        assert_eq!(h.destino(centro(h.papel)), DestinoHojita::Papel);
        assert_eq!(h.destino(centro(h.borrar)), DestinoHojita::BorrarTodo);
        assert_eq!(h.destino(centro(h.insertar)), DestinoHojita::Insertar);
        assert_eq!(h.destino(centro(h.pegar)), DestinoHojita::PegarComoPin);
        assert_eq!(h.destino(centro(h.cerrar)), DestinoHojita::Cerrar);
        for (i, m) in h.muestras.iter().enumerate() {
            assert_eq!(h.destino(centro(*m)), DestinoHojita::ColorPapel(i));
            assert!(
                m.interseccion(h.borrar).is_none(),
                "la muestra {i} pisa «borrar»"
            );
        }
    }

    #[test]
    fn fuera_de_la_tarjeta_es_fuera() {
        let h = Hojita::colocar(1920, 1080, 100, 5, 64);
        assert_eq!(h.destino(Punto { x: 5, y: 1000 }), DestinoHojita::Fuera);
    }

    #[test]
    fn en_una_ventana_pequena_el_papel_se_encoge_pero_no_desaparece() {
        let h = Hojita::colocar(400, 300, 100, 5, 64);
        assert!(h.papel.ancho >= ANCHO_PAPEL / 2);
        assert!(h.papel.alto >= ALTO_PAPEL / 2);
    }
}
