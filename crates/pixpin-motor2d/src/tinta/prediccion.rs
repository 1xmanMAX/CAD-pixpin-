//! Prediccion del puntero para la tinta en curso.
//!
//! Aunque se pinte en cuanto llega el punto, entre leer el raton y verlo en
//! pantalla pasan un refresco y la composicion de DWM: medido en el equipo
//! del usuario, la punta iba unos 30 ms detras del cursor. Presentar antes
//! no lo arreglo (misma cifra con la cola de fotogramas en 1), porque ese
//! retraso es el propio ciclo de la pantalla.
//!
//! Lo que hacen Windows Ink y los navegadores (`getPredictedEvents`) es
//! pintar la punta donde ESTARA el puntero cuando el fotograma se vea: la
//! velocidad reciente por el horizonte. Solo se pinta; el trazo guardado es
//! el real.
//!
//! Las dos protecciones que evitan que la punta «se adelante» de mas:
//! - un tope de distancia, y
//! - apagarla cuando el movimiento es lento o acaba de torcer, que es
//!   justo cuando extrapolar en linea recta se equivoca y se veria.

use std::collections::VecDeque;

use crate::elemento::{Elemento, Figura};
use crate::vector::Punto2;

/// Ventana de muestras con la que se mide la velocidad.
const VENTANA_MS: f64 = 50.0;
/// Por debajo de esta velocidad (pixeles de pantalla por segundo) no se
/// predice: al frenar para terminar un trazo, una punta adelantada se ve
/// como un pelo que sobra.
const VELOCIDAD_MINIMA: f32 = 60.0;
/// Si la direccion de los ultimos puntos se separa de la de la ventana mas
/// de esto (coseno), el trazo esta girando y se predice menos.
const COSENO_GIRO: f32 = 0.7;

#[derive(Debug, Default)]
pub struct Predictor {
    muestras: VecDeque<(Punto2, f64)>,
}

impl Predictor {
    pub fn nuevo() -> Self {
        Self::default()
    }

    /// Olvida el trazo anterior: un trazo nuevo no hereda velocidad.
    pub fn reiniciar(&mut self) {
        self.muestras.clear();
    }

    /// Anota un punto del trazo en el instante `ms` (reloj monotono).
    pub fn anotar(&mut self, p: Punto2, ms: f64) {
        self.muestras.push_back((p, ms));
        while let Some(&(_, t)) = self.muestras.front() {
            if ms - t > VENTANA_MS && self.muestras.len() > 2 {
                self.muestras.pop_front();
            } else {
                break;
            }
        }
    }

    /// Donde estara el puntero dentro de `horizonte_ms`, sin pasar de `tope`
    /// de distancia. `a_pantalla` pasa la velocidad a pixeles de pantalla
    /// (el zoom) para decidir si es lenta. `None` si no hay datos o no
    /// conviene predecir.
    pub fn predecir(&self, horizonte_ms: f32, tope: f32, a_pantalla: f32) -> Option<Punto2> {
        let &(ultimo, t1) = self.muestras.back()?;
        let &(primero, t0) = self.muestras.front()?;
        let dt = (t1 - t0) as f32;
        // Menos de 8 ms de historia: la velocidad seria ruido.
        if dt < 8.0 {
            return None;
        }
        let vx = (ultimo.x - primero.x) / dt;
        let vy = (ultimo.y - primero.y) / dt;
        let velocidad = (vx * vx + vy * vy).sqrt();
        if velocidad * 1000.0 * a_pantalla < VELOCIDAD_MINIMA {
            return None;
        }

        // Direccion reciente: el tramo que acaba en el ultimo punto desde la
        // muestra del medio. Si tuerce respecto a la media, se reduce.
        let medio = self.muestras[self.muestras.len() / 2].0;
        let (rx, ry) = (ultimo.x - medio.x, ultimo.y - medio.y);
        let largo_r = (rx * rx + ry * ry).sqrt();
        let mut factor = 1.0;
        if largo_r > 0.0 {
            let coseno = (rx * vx + ry * vy) / (largo_r * velocidad);
            if coseno <= 0.0 {
                return None;
            }
            if coseno < COSENO_GIRO {
                factor = 0.5;
            }
        }

        let (mut dx, mut dy) = (vx * horizonte_ms * factor, vy * horizonte_ms * factor);
        let largo = (dx * dx + dy * dy).sqrt();
        if largo > tope && largo > 0.0 {
            dx *= tope / largo;
            dy *= tope / largo;
        }
        Some(Punto2::nuevo(ultimo.x + dx, ultimo.y + dy))
    }
}

/// Una copia del elemento en curso con su punta llevada a `q`, el punto
/// predicho: el trazo gana un punto; linea, flecha y cota mueven su extremo;
/// las figuras de caja crecen desde `origen`, donde se pulso. `None` si el
/// elemento no tiene punta que adelantar.
pub fn con_punta(e: &Elemento, origen: Option<Punto2>, q: Punto2) -> Option<Elemento> {
    let mut c = e.clone();
    match &mut c.figura {
        Figura::Lapiz {
            puntos, presiones, ..
        } => {
            puntos.push(q);
            if let Some(&u) = presiones.last() {
                presiones.push(u);
            }
        }
        Figura::Resaltador { puntos } => puntos.push(q),
        Figura::Linea { puntos } | Figura::Flecha { puntos, .. } | Figura::Cota { puntos }
            if puntos.len() >= 2 =>
        {
            *puntos.last_mut()? = q;
        }
        Figura::Rectangulo
        | Figura::Rombo
        | Figura::Mosaico { .. }
        | Figura::Elipse
        | Figura::Foco { .. }
        | Figura::EscalaGrafica => {
            let o = origen?;
            c.x = o.x.min(q.x);
            c.y = o.y.min(q.y);
            c.ancho = (q.x - o.x).abs();
            c.alto = (q.y - o.y).abs();
        }
        _ => return None,
    }
    Some(c)
}

#[cfg(test)]
mod pruebas {
    use crate::elemento::{ColorRgba, EstiloTrazo};
    use crate::formas::TipoPunta;

    fn elemento(figura: Figura) -> Elemento {
        Elemento {
            id: 1,
            figura,
            x: 10.0,
            y: 10.0,
            ancho: 20.0,
            alto: 20.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    #[test]
    fn el_rectangulo_crece_desde_donde_se_pulso_hasta_la_punta_predicha() {
        let e = elemento(Figura::Rectangulo);
        let c = con_punta(
            &e,
            Some(Punto2::nuevo(10.0, 10.0)),
            Punto2::nuevo(50.0, 5.0),
        )
        .unwrap();
        assert_eq!((c.x, c.y, c.ancho, c.alto), (10.0, 5.0, 40.0, 5.0));
        // Caso negativo: sin origen no se inventa la caja.
        assert!(con_punta(&e, None, Punto2::nuevo(50.0, 5.0)).is_none());
    }

    #[test]
    fn la_flecha_mueve_su_extremo_y_el_lapiz_gana_un_punto() {
        let f = elemento(Figura::Flecha {
            puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(5.0, 5.0)],
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos: false,
        });
        let c = con_punta(&f, None, Punto2::nuevo(9.0, 9.0)).unwrap();
        match c.figura {
            Figura::Flecha { puntos, .. } => assert_eq!(puntos[1], Punto2::nuevo(9.0, 9.0)),
            _ => unreachable!(),
        }
        let l = elemento(Figura::Lapiz {
            puntos: vec![Punto2::nuevo(0.0, 0.0)],
            presiones: vec![0.5],
            opciones: None,
        });
        match con_punta(&l, None, Punto2::nuevo(3.0, 0.0)).unwrap().figura {
            Figura::Lapiz {
                puntos, presiones, ..
            } => {
                assert_eq!(puntos.len(), 2);
                assert_eq!(presiones, vec![0.5, 0.5]);
            }
            _ => unreachable!(),
        }
    }

    use super::*;

    fn recta(pred: &mut Predictor, vx: f32, vy: f32, n: usize, cada_ms: f64) {
        for i in 0..n {
            let t = i as f64 * cada_ms;
            pred.anotar(Punto2::nuevo(vx * t as f32, vy * t as f32), t);
        }
    }

    #[test]
    fn a_velocidad_constante_la_punta_va_donde_estara_el_cursor() {
        let mut p = Predictor::nuevo();
        // 1,5 px por ms = 1500 px/s, una muestra cada 4 ms.
        recta(&mut p, 1.5, 0.0, 20, 4.0);
        let ultimo = 1.5 * 76.0;
        let q = p.predecir(30.0, 1000.0, 1.0).unwrap();
        assert!((q.x - (ultimo + 45.0)).abs() < 0.5, "{q:?}");
        assert!(q.y.abs() < 1e-3);
    }

    #[test]
    fn la_prediccion_no_pasa_del_tope() {
        let mut p = Predictor::nuevo();
        recta(&mut p, 10.0, 0.0, 20, 4.0);
        let ultimo = 10.0 * 76.0;
        let q = p.predecir(30.0, 40.0, 1.0).unwrap();
        assert!((q.x - (ultimo + 40.0)).abs() < 1e-3, "{q:?}");
    }

    #[test]
    fn despacio_no_se_predice_para_no_dejar_un_pelo_al_frenar() {
        let mut p = Predictor::nuevo();
        // 0,02 px/ms = 20 px/s.
        recta(&mut p, 0.02, 0.0, 20, 4.0);
        assert_eq!(p.predecir(30.0, 40.0, 1.0), None);
    }

    #[test]
    fn sin_historia_suficiente_no_se_inventa_nada() {
        let mut p = Predictor::nuevo();
        assert_eq!(p.predecir(30.0, 40.0, 1.0), None);
        p.anotar(Punto2::nuevo(0.0, 0.0), 0.0);
        p.anotar(Punto2::nuevo(5.0, 0.0), 4.0);
        assert_eq!(p.predecir(30.0, 40.0, 1.0), None, "4 ms no bastan");
    }

    #[test]
    fn al_dar_la_vuelta_se_apaga() {
        let mut p = Predictor::nuevo();
        // Va a la derecha 36 ms y ACABA de dar la vuelta: dos muestras hacia
        // la izquierda. Seguir la media llevaria la punta a la derecha, justo
        // al lado contrario de donde va la mano.
        for i in 0..10 {
            p.anotar(Punto2::nuevo(i as f32 * 6.0, 0.0), i as f64 * 4.0);
        }
        for i in 1..3 {
            p.anotar(
                Punto2::nuevo(54.0 - i as f32 * 12.0, 0.0),
                36.0 + i as f64 * 4.0,
            );
        }
        assert_eq!(p.predecir(30.0, 40.0, 1.0), None);
    }

    #[test]
    fn las_muestras_viejas_salen_de_la_ventana() {
        let mut p = Predictor::nuevo();
        recta(&mut p, 1.0, 0.0, 100, 4.0);
        let span = p.muestras.back().unwrap().1 - p.muestras.front().unwrap().1;
        assert!(span <= VENTANA_MS + 4.0, "{span}");
    }

    #[test]
    fn reiniciar_olvida_la_velocidad_del_trazo_anterior() {
        let mut p = Predictor::nuevo();
        recta(&mut p, 1.5, 0.0, 20, 4.0);
        p.reiniciar();
        assert_eq!(p.predecir(30.0, 40.0, 1.0), None);
    }
}
