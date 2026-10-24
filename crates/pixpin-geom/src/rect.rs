//! Rectangulos en pixeles fisicos, con la convencion de **media apertura**:
//! el borde superior izquierdo pertenece al rectangulo y el inferior derecho
//! no. Es la misma regla que usa Windows, y sin ella dos monitores adyacentes
//! se solapan en una fila de pixeles.

use crate::punto::Punto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub ancho: u32,
    pub alto: u32,
}

impl Rect {
    /// Rectangulo entre dos esquinas cualesquiera.
    ///
    /// Normaliza, porque el usuario arrastra en las cuatro direcciones y las
    /// dos que van hacia arriba o hacia la izquierda darian medidas negativas.
    pub fn desde_esquinas(a: Punto, b: Punto) -> Rect {
        let x = a.x.min(b.x);
        let y = a.y.min(b.y);
        // La resta se hace en i64 porque dos extremos opuestos del escritorio
        // virtual pueden distar mas de lo que cabe en i32 sin desbordar.
        let ancho = (i64::from(a.x) - i64::from(b.x)).unsigned_abs() as u32;
        let alto = (i64::from(a.y) - i64::from(b.y)).unsigned_abs() as u32;
        Rect { x, y, ancho, alto }
    }

    pub fn izquierda(&self) -> i32 {
        self.x
    }

    pub fn arriba(&self) -> i32 {
        self.y
    }

    /// Primer pixel **fuera** del rectangulo por la derecha.
    pub fn derecha(&self) -> i32 {
        self.x.saturating_add(self.ancho as i32)
    }

    /// Primera fila **fuera** del rectangulo por abajo.
    pub fn abajo(&self) -> i32 {
        self.y.saturating_add(self.alto as i32)
    }

    pub fn esta_vacio(&self) -> bool {
        self.ancho == 0 || self.alto == 0
    }

    /// Area en pixeles. En `u64` porque un escritorio de varios 8K sumados
    /// desborda `u32`.
    pub fn area(&self) -> u64 {
        u64::from(self.ancho) * u64::from(self.alto)
    }

    pub fn contiene(&self, p: Punto) -> bool {
        p.x >= self.izquierda()
            && p.x < self.derecha()
            && p.y >= self.arriba()
            && p.y < self.abajo()
    }

    /// Solape real. `None` si solo se tocan por el borde: dos rectangulos
    /// pegados no se cortan, y devolver uno de area cero volveria erratico el
    /// ajuste automatico.
    pub fn interseccion(&self, otro: Rect) -> Option<Rect> {
        let izquierda = self.izquierda().max(otro.izquierda());
        let arriba = self.arriba().max(otro.arriba());
        let derecha = self.derecha().min(otro.derecha());
        let abajo = self.abajo().min(otro.abajo());

        if derecha <= izquierda || abajo <= arriba {
            return None;
        }
        Some(Rect {
            x: izquierda,
            y: arriba,
            ancho: (derecha - izquierda) as u32,
            alto: (abajo - arriba) as u32,
        })
    }

    /// El menor rectangulo que contiene a los dos.
    pub fn union(&self, otro: Rect) -> Rect {
        let izquierda = self.izquierda().min(otro.izquierda());
        let arriba = self.arriba().min(otro.arriba());
        let derecha = self.derecha().max(otro.derecha());
        let abajo = self.abajo().max(otro.abajo());
        Rect {
            x: izquierda,
            y: arriba,
            ancho: (derecha - izquierda) as u32,
            alto: (abajo - arriba) as u32,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn desde_esquinas_normaliza_el_arrastre_hacia_arriba_a_la_izquierda() {
        // El usuario arrastra de abajo-derecha a arriba-izquierda. Sin
        // normalizar saldrian anchos negativos y todo lo demas se rompe.
        let r = Rect::desde_esquinas(Punto { x: 100, y: 80 }, Punto { x: 20, y: 10 });
        assert_eq!(
            r,
            Rect {
                x: 20,
                y: 10,
                ancho: 80,
                alto: 70
            }
        );
    }

    #[test]
    fn desde_esquinas_con_el_mismo_punto_da_un_rectangulo_vacio() {
        let p = Punto { x: 5, y: 5 };
        let r = Rect::desde_esquinas(p, p);
        assert!(r.esta_vacio());
        assert_eq!(r.area(), 0);
    }

    #[test]
    fn funciona_con_coordenadas_negativas() {
        // El escritorio virtual tiene coordenadas negativas en cuanto hay un
        // monitor a la izquierda o encima del principal.
        let r = Rect::desde_esquinas(Punto { x: -50, y: -30 }, Punto { x: -10, y: -5 });
        assert_eq!(
            r,
            Rect {
                x: -50,
                y: -30,
                ancho: 40,
                alto: 25
            }
        );
        assert_eq!(r.derecha(), -10);
        assert_eq!(r.abajo(), -5);
    }

    #[test]
    fn contiene_incluye_el_borde_superior_izquierdo_y_excluye_el_inferior_derecho() {
        // Media apertura, como los rectangulos de Windows. Sin esta regla dos
        // monitores adyacentes se solapan en una fila de pixeles.
        let r = Rect {
            x: 0,
            y: 0,
            ancho: 10,
            alto: 10,
        };
        assert!(r.contiene(Punto { x: 0, y: 0 }));
        assert!(r.contiene(Punto { x: 9, y: 9 }));
        assert!(!r.contiene(Punto { x: 10, y: 5 }));
        assert!(!r.contiene(Punto { x: 5, y: 10 }));
        assert!(!r.contiene(Punto { x: -1, y: 0 }));
    }

    #[test]
    fn interseccion_devuelve_none_cuando_solo_se_tocan_por_el_borde() {
        // Caso negativo: dos rectangulos pegados NO se cortan. Una
        // implementacion que usara >= en vez de > devolveria un rectangulo de
        // area cero y el ajuste automatico se volveria erratico.
        let a = Rect {
            x: 0,
            y: 0,
            ancho: 10,
            alto: 10,
        };
        let b = Rect {
            x: 10,
            y: 0,
            ancho: 10,
            alto: 10,
        };
        assert_eq!(a.interseccion(b), None);
    }

    #[test]
    fn interseccion_recorta_al_solape_real() {
        let a = Rect {
            x: 0,
            y: 0,
            ancho: 10,
            alto: 10,
        };
        let b = Rect {
            x: 5,
            y: 5,
            ancho: 10,
            alto: 10,
        };
        assert_eq!(
            a.interseccion(b),
            Some(Rect {
                x: 5,
                y: 5,
                ancho: 5,
                alto: 5
            })
        );
        // Y es simetrica.
        assert_eq!(a.interseccion(b), b.interseccion(a));
    }

    #[test]
    fn interseccion_con_uno_contenido_devuelve_el_pequeno() {
        let grande = Rect {
            x: 0,
            y: 0,
            ancho: 100,
            alto: 100,
        };
        let pequeno = Rect {
            x: 10,
            y: 10,
            ancho: 5,
            alto: 5,
        };
        assert_eq!(grande.interseccion(pequeno), Some(pequeno));
    }

    #[test]
    fn union_abarca_ambos_incluso_con_coordenadas_negativas() {
        let a = Rect {
            x: -20,
            y: -10,
            ancho: 10,
            alto: 10,
        };
        let b = Rect {
            x: 30,
            y: 40,
            ancho: 10,
            alto: 10,
        };
        assert_eq!(
            a.union(b),
            Rect {
                x: -20,
                y: -10,
                ancho: 60,
                alto: 60
            }
        );
    }

    #[test]
    fn el_area_no_desborda_con_un_escritorio_enorme() {
        // Un escritorio de varios 8K sumados cabe de sobra en u64 pero no en
        // u32. Este test falla si alguien devuelve u32.
        let r = Rect {
            x: 0,
            y: 0,
            ancho: 100_000,
            alto: 100_000,
        };
        assert_eq!(r.area(), 10_000_000_000u64);
    }
}
