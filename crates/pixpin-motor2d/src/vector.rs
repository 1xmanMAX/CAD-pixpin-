//! Vectores 2D en coma flotante.
//!
//! El resto del proyecto trabaja en pixeles fisicos enteros (`pixpin_geom`),
//! pero un trazo a mano necesita subpixel: media docena de operaciones sobre
//! `f32` es todo lo que hace falta, y son cuarenta lineas. Traer una libreria
//! de algebra para esto seria mas dependencia que codigo.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Punto2 {
    pub x: f32,
    pub y: f32,
}

impl Punto2 {
    pub const fn nuevo(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn sumar(self, otro: Punto2) -> Punto2 {
        Punto2::nuevo(self.x + otro.x, self.y + otro.y)
    }

    pub fn restar(self, otro: Punto2) -> Punto2 {
        Punto2::nuevo(self.x - otro.x, self.y - otro.y)
    }

    pub fn escalar(self, k: f32) -> Punto2 {
        Punto2::nuevo(self.x * k, self.y * k)
    }

    /// El perpendicular en sentido horario: la base del grosor de un trazo,
    /// que se dibuja desplazando la linea central a un lado y al otro.
    pub fn perpendicular(self) -> Punto2 {
        Punto2::nuevo(self.y, -self.x)
    }

    pub fn producto(self, otro: Punto2) -> f32 {
        self.x * otro.x + self.y * otro.y
    }

    pub fn longitud(self) -> f32 {
        self.producto(self).sqrt()
    }

    pub fn distancia(self, otro: Punto2) -> f32 {
        self.restar(otro).longitud()
    }

    /// Unitario. Un vector de longitud cero se queda como esta: normalizarlo
    /// daria NaN, y un NaN dentro de una geometria la borra entera de la
    /// pantalla sin ningun error visible.
    pub fn unitario(self) -> Punto2 {
        let l = self.longitud();
        if l <= f32::EPSILON {
            self
        } else {
            self.escalar(1.0 / l)
        }
    }

    /// Interpolacion lineal: `t=0` da este punto, `t=1` da el otro.
    pub fn hacia(self, otro: Punto2, t: f32) -> Punto2 {
        self.sumar(otro.restar(self).escalar(t))
    }

    /// Este punto desplazado `distancia` en la direccion dada.
    pub fn proyectar(self, direccion: Punto2, distancia: f32) -> Punto2 {
        self.sumar(direccion.escalar(distancia))
    }

    /// Girado `angulo` radianes alrededor de `centro`.
    pub fn girar(self, centro: Punto2, angulo: f32) -> Punto2 {
        let (s, c) = angulo.sin_cos();
        let d = self.restar(centro);
        Punto2::nuevo(centro.x + d.x * c - d.y * s, centro.y + d.x * s + d.y * c)
    }
}

/// Distancia de un punto al SEGMENTO ab (no a la recta infinita).
///
/// La diferencia importa: con la recta, un punto a un kilometro mas alla del
/// final de una linea corta "tocaria" la linea. Es la base del hit-test.
pub fn distancia_a_segmento(p: Punto2, a: Punto2, b: Punto2) -> f32 {
    let ab = b.restar(a);
    let largo2 = ab.producto(ab);
    if largo2 <= f32::EPSILON {
        // Segmento degenerado: es un punto.
        return p.distancia(a);
    }
    // Proyeccion escalar de ap sobre ab, sujeta a [0,1] para no salirse.
    let t = (p.restar(a).producto(ab) / largo2).clamp(0.0, 1.0);
    p.distancia(a.sumar(ab.escalar(t)))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_perpendicular_forma_angulo_recto() {
        let v = Punto2::nuevo(3.0, 4.0);
        assert_eq!(v.producto(v.perpendicular()), 0.0);
        assert_eq!(v.longitud(), v.perpendicular().longitud());
    }

    #[test]
    fn normalizar_el_vector_cero_no_produce_nan() {
        // Un NaN aqui se propaga a la geometria y el elemento desaparece de
        // la pantalla sin error ninguno: el fallo mas dificil de diagnosticar.
        let u = Punto2::nuevo(0.0, 0.0).unitario();
        assert!(u.x.is_finite() && u.y.is_finite());
    }

    #[test]
    fn la_distancia_al_segmento_no_es_la_distancia_a_la_recta() {
        let a = Punto2::nuevo(0.0, 0.0);
        let b = Punto2::nuevo(10.0, 0.0);
        // Sobre la recta pero MUY lejos del segmento: la distancia debe
        // medirse al extremo, no ser cero.
        let lejos = Punto2::nuevo(1000.0, 0.0);
        assert_eq!(distancia_a_segmento(lejos, a, b), 990.0);
        // Y en medio, la perpendicular de siempre.
        assert_eq!(distancia_a_segmento(Punto2::nuevo(5.0, 3.0), a, b), 3.0);
    }

    #[test]
    fn un_segmento_degenerado_mide_como_punto() {
        let a = Punto2::nuevo(2.0, 2.0);
        assert_eq!(distancia_a_segmento(Punto2::nuevo(2.0, 5.0), a, a), 3.0);
    }

    #[test]
    fn girar_un_cuarto_de_vuelta_lleva_el_eje_x_al_eje_y() {
        let p = Punto2::nuevo(1.0, 0.0);
        let g = p.girar(Punto2::nuevo(0.0, 0.0), std::f32::consts::FRAC_PI_2);
        assert!(g.x.abs() < 1e-6, "x deberia ser ~0, es {}", g.x);
        assert!((g.y - 1.0).abs() < 1e-6, "y deberia ser ~1, es {}", g.y);
    }

    #[test]
    fn interpolar_a_los_extremos_da_los_extremos() {
        let a = Punto2::nuevo(1.0, 2.0);
        let b = Punto2::nuevo(9.0, 6.0);
        assert_eq!(a.hacia(b, 0.0), a);
        assert_eq!(a.hacia(b, 1.0), b);
        assert_eq!(a.hacia(b, 0.5), Punto2::nuevo(5.0, 4.0));
    }
}
