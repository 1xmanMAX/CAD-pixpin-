//! El rectangulo que el usuario esta eligiendo, con sus ocho tiradores.
//!
//! Toda la maquina de estados del arrastre y del redimensionado esta aqui,
//! como logica pura. El overlay se limita a traducir eventos de raton y de
//! teclado a estas llamadas y a dibujar el resultado, de modo que el
//! comportamiento se puede probar entero sin abrir una ventana.

use crate::monitores::DisposicionMonitores;
use crate::punto::Punto;
use crate::rect::Rect;

/// Los ocho asideros: cuatro esquinas y cuatro bordes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tirador {
    NoroesteEsquina,
    NorteBorde,
    NoresteEsquina,
    EsteBorde,
    SuresteEsquina,
    SurBorde,
    SuroesteEsquina,
    OesteBorde,
}

/// Que se esta haciendo ahora mismo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gesto {
    Ninguno,
    /// Arrastre libre desde un ancla fija.
    Trazando {
        ancla: Punto,
    },
    /// Redimension moviendo un tirador; el resto del rectangulo se conserva.
    Redimensionando {
        tirador: Tirador,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seleccion {
    rect: Rect,
    gesto: Gesto,
}

impl Default for Seleccion {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Seleccion {
    pub fn nueva() -> Self {
        Self {
            rect: Rect {
                x: 0,
                y: 0,
                ancho: 0,
                alto: 0,
            },
            gesto: Gesto::Ninguno,
        }
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn establecer(&mut self, r: Rect) {
        self.rect = r;
    }

    pub fn iniciar_arrastre(&mut self, p: Punto) {
        self.gesto = Gesto::Trazando { ancla: p };
        self.rect = Rect::desde_esquinas(p, p);
    }

    pub fn iniciar_redimension(&mut self, tirador: Tirador) {
        self.gesto = Gesto::Redimensionando { tirador };
    }

    /// Mueve el punto activo del gesto en curso. Sin gesto, no hace nada.
    pub fn arrastrar_a(&mut self, p: Punto) {
        match self.gesto {
            Gesto::Ninguno => {}
            Gesto::Trazando { ancla } => {
                self.rect = Rect::desde_esquinas(ancla, p);
            }
            Gesto::Redimensionando { tirador } => {
                // Se calculan los cuatro lados por separado y se reconstruye
                // con `desde_esquinas`, que normaliza. Asi arrastrar un borde
                // mas alla del opuesto no produce medidas negativas: el
                // rectangulo simplemente se da la vuelta.
                let (mut izq, mut arr) = (self.rect.izquierda(), self.rect.arriba());
                let (mut der, mut aba) = (self.rect.derecha(), self.rect.abajo());

                match tirador {
                    Tirador::NoroesteEsquina => {
                        izq = p.x;
                        arr = p.y;
                    }
                    Tirador::NorteBorde => arr = p.y,
                    Tirador::NoresteEsquina => {
                        der = p.x;
                        arr = p.y;
                    }
                    Tirador::EsteBorde => der = p.x,
                    Tirador::SuresteEsquina => {
                        der = p.x;
                        aba = p.y;
                    }
                    Tirador::SurBorde => aba = p.y,
                    Tirador::SuroesteEsquina => {
                        izq = p.x;
                        aba = p.y;
                    }
                    Tirador::OesteBorde => izq = p.x,
                }

                self.rect =
                    Rect::desde_esquinas(Punto { x: izq, y: arr }, Punto { x: der, y: aba });
            }
        }
    }

    pub fn terminar_arrastre(&mut self) {
        self.gesto = Gesto::Ninguno;
    }

    /// Que tirador hay bajo el punto, dentro de `radio` pixeles.
    ///
    /// Las esquinas ganan a los bordes cuando ambos estan a tiro: en una
    /// seleccion pequena un punto puede caer en el radio de una esquina y de
    /// dos bordes, y apuntar a la esquina es lo que el usuario quiere.
    pub fn tirador_en(&self, p: Punto, radio: u32) -> Option<Tirador> {
        let r = radio as i32;
        let (izq, der) = (self.rect.izquierda(), self.rect.derecha());
        let (arr, aba) = (self.rect.arriba(), self.rect.abajo());
        let medio_x = izq + (self.rect.ancho as i32) / 2;
        let medio_y = arr + (self.rect.alto as i32) / 2;

        let cerca = |a: i32, b: i32| (a - b).abs() <= r;

        // Esquinas primero, por la regla de precedencia.
        if cerca(p.x, izq) && cerca(p.y, arr) {
            return Some(Tirador::NoroesteEsquina);
        }
        if cerca(p.x, der) && cerca(p.y, arr) {
            return Some(Tirador::NoresteEsquina);
        }
        if cerca(p.x, der) && cerca(p.y, aba) {
            return Some(Tirador::SuresteEsquina);
        }
        if cerca(p.x, izq) && cerca(p.y, aba) {
            return Some(Tirador::SuroesteEsquina);
        }
        // Luego los bordes, exigiendo cercania al punto medio del lado para
        // no capturar toda la arista.
        if cerca(p.y, arr) && cerca(p.x, medio_x) {
            return Some(Tirador::NorteBorde);
        }
        if cerca(p.y, aba) && cerca(p.x, medio_x) {
            return Some(Tirador::SurBorde);
        }
        if cerca(p.x, izq) && cerca(p.y, medio_y) {
            return Some(Tirador::OesteBorde);
        }
        if cerca(p.x, der) && cerca(p.y, medio_y) {
            return Some(Tirador::EsteBorde);
        }
        None
    }

    /// Mueve el rectangulo entero. Para las flechas del teclado.
    pub fn desplazar(&mut self, dx: i32, dy: i32) {
        self.rect.x = self.rect.x.saturating_add(dx);
        self.rect.y = self.rect.y.saturating_add(dy);
    }

    /// Recorta la seleccion a lo que hay de escritorio.
    ///
    /// Si queda del todo fuera, la deja vacia en vez de conservar algo
    /// imposible de capturar.
    pub fn sujetar_a(&mut self, d: &DisposicionMonitores) {
        self.rect = d.recortar_al_escritorio(self.rect).unwrap_or(Rect {
            x: self.rect.x,
            y: self.rect.y,
            ancho: 0,
            alto: 0,
        });
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::monitores::Monitor;

    fn seleccion_de(x: i32, y: i32, ancho: u32, alto: u32) -> Seleccion {
        let mut s = Seleccion::nueva();
        s.establecer(Rect { x, y, ancho, alto });
        s
    }

    #[test]
    fn arrastrar_en_cualquier_direccion_da_un_rectangulo_valido() {
        // Las cuatro direcciones, incluidas las dos que van "hacia atras".
        for destino in [
            Punto { x: 150, y: 120 },
            Punto { x: 50, y: 120 },
            Punto { x: 150, y: 20 },
            Punto { x: 50, y: 20 },
        ] {
            let mut s = Seleccion::nueva();
            s.iniciar_arrastre(Punto { x: 100, y: 70 });
            s.arrastrar_a(destino);
            s.terminar_arrastre();
            let r = s.rect();
            assert!(
                !r.esta_vacio(),
                "arrastre a {destino:?} dio un rectangulo vacio"
            );
            assert_eq!(r.ancho, 50);
            assert_eq!(r.alto, 50);
        }
    }

    #[test]
    fn los_ocho_tiradores_se_detectan_en_sus_esquinas_y_bordes() {
        let s = seleccion_de(100, 100, 200, 100);
        let casos = [
            (Punto { x: 100, y: 100 }, Tirador::NoroesteEsquina),
            (Punto { x: 200, y: 100 }, Tirador::NorteBorde),
            (Punto { x: 300, y: 100 }, Tirador::NoresteEsquina),
            (Punto { x: 300, y: 150 }, Tirador::EsteBorde),
            (Punto { x: 300, y: 200 }, Tirador::SuresteEsquina),
            (Punto { x: 200, y: 200 }, Tirador::SurBorde),
            (Punto { x: 100, y: 200 }, Tirador::SuroesteEsquina),
            (Punto { x: 100, y: 150 }, Tirador::OesteBorde),
        ];
        for (p, esperado) in casos {
            assert_eq!(s.tirador_en(p, 8), Some(esperado), "fallo en {p:?}");
        }
    }

    #[test]
    fn no_hay_tirador_en_el_centro_ni_lejos() {
        // Caso negativo: sin esto, tirador_en podria devolver siempre algo y
        // el arrastre del interior se volveria un redimensionado.
        let s = seleccion_de(100, 100, 200, 100);
        assert_eq!(s.tirador_en(Punto { x: 200, y: 150 }, 8), None);
        assert_eq!(s.tirador_en(Punto { x: 500, y: 500 }, 8), None);
        // Justo fuera del radio.
        assert_eq!(s.tirador_en(Punto { x: 100, y: 120 }, 8), None);
    }

    #[test]
    fn las_esquinas_ganan_a_los_bordes_cuando_ambos_estan_a_tiro() {
        // En una seleccion pequena, un punto puede estar dentro del radio de
        // una esquina y de dos bordes a la vez. Debe ganar la esquina: es lo
        // que el usuario quiere al apuntar ahi.
        let s = seleccion_de(0, 0, 10, 10);
        assert_eq!(
            s.tirador_en(Punto { x: 0, y: 0 }, 8),
            Some(Tirador::NoroesteEsquina)
        );
        assert_eq!(
            s.tirador_en(Punto { x: 10, y: 10 }, 8),
            Some(Tirador::SuresteEsquina)
        );
    }

    #[test]
    fn redimensionar_por_una_esquina_mueve_solo_esa_esquina() {
        let mut s = seleccion_de(100, 100, 200, 100);
        s.iniciar_redimension(Tirador::SuresteEsquina);
        s.arrastrar_a(Punto { x: 400, y: 300 });
        s.terminar_arrastre();
        assert_eq!(
            s.rect(),
            Rect {
                x: 100,
                y: 100,
                ancho: 300,
                alto: 200
            }
        );
    }

    #[test]
    fn redimensionar_por_un_borde_solo_cambia_una_dimension() {
        let mut s = seleccion_de(100, 100, 200, 100);
        s.iniciar_redimension(Tirador::EsteBorde);
        s.arrastrar_a(Punto { x: 400, y: 999 });
        s.terminar_arrastre();
        // El alto y la Y no se han tocado pese al 999.
        assert_eq!(
            s.rect(),
            Rect {
                x: 100,
                y: 100,
                ancho: 300,
                alto: 100
            }
        );
    }

    #[test]
    fn redimensionar_mas_alla_del_lado_opuesto_no_da_medidas_negativas() {
        // Caso negativo clasico: arrastrar el borde este hasta la izquierda
        // del oeste. Sin normalizar saldria un ancho negativo.
        let mut s = seleccion_de(100, 100, 200, 100);
        s.iniciar_redimension(Tirador::EsteBorde);
        s.arrastrar_a(Punto { x: 20, y: 150 });
        s.terminar_arrastre();
        let r = s.rect();
        assert_eq!(r.x, 20);
        assert_eq!(r.ancho, 80);
    }

    #[test]
    fn desplazar_mueve_sin_cambiar_el_tamano() {
        let mut s = seleccion_de(100, 100, 200, 100);
        s.desplazar(-10, 5);
        assert_eq!(
            s.rect(),
            Rect {
                x: 90,
                y: 105,
                ancho: 200,
                alto: 100
            }
        );
    }

    #[test]
    fn sujetar_recorta_lo_que_sale_del_escritorio() {
        let d = DisposicionMonitores::nueva(vec![Monitor {
            id: 1,
            area: Rect {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 1080,
            },
            area_trabajo: Rect {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 1040,
            },
            escala_por_cien: 100,
            principal: true,
        }]);
        let mut s = seleccion_de(-50, -50, 200, 200);
        s.sujetar_a(&d);
        assert_eq!(
            s.rect(),
            Rect {
                x: 0,
                y: 0,
                ancho: 150,
                alto: 150
            }
        );
    }
}
