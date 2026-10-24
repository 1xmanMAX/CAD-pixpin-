//! **La bolita: se pasa por encima de lo que se quiera y va entrando en la
//! seleccion** (`Tool.BOLITA` del movil, `DrawController.pasarLaBolita`).
//!
//! No hay rectangulo que encuadrar ni tecla que mantener: se pasa por algo y
//! entra; se vuelve a pasar por algo que ya estaba dentro y sale. Cada figura
//! se decide **una vez por barrido** (`yaTocados`): sin eso, pararse encima la
//! encenderia y apagaria en cada aviso del raton. Tocar una pieza de un grupo
//! coge el grupo entero, como con la mano.
//!
//! El radio va en pixeles de pantalla (`RADIO_DE_LA_BOLITA = 30`), porque es
//! lo que se ve: en unidades del dibujo cogeria media escena de lejos.

use std::collections::HashSet;

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo};
use crate::pintado::Orden;
use crate::seleccion::Seleccion;
use crate::vector::{Punto2, distancia_a_segmento};

/// El radio de la bolita en pixeles de pantalla (el del movil).
pub const RADIO_DE_LA_BOLITA: f32 = 30.0;

/// Un barrido de la bolita en curso.
#[derive(Debug, Clone, Default)]
pub struct Bolita {
    /// Donde esta ahora, para pintarla. `None` sin barrido.
    pub centro: Option<Punto2>,
    ya_tocados: HashSet<u64>,
}

/// Si la bolita de radio `radio` centrada en `p` toca a `e`: por su trazo, o
/// por dentro si es una figura que se pica por dentro.
fn toca(e: &Elemento, p: Punto2, radio: f32) -> bool {
    if e.borrado || e.bloqueado {
        return false;
    }
    if crate::impacto::toca(e, p) {
        return true;
    }
    let tramos = crate::perimetros::segmentos_de(e, crate::perimetros::PASO_PERIMETRO);
    if !tramos.is_empty() {
        return tramos
            .iter()
            .any(|(a, b)| distancia_a_segmento(p, *a, *b) <= radio);
    }
    // Lo que no da contorno (texto, punto): su caja girada.
    let c = crate::impacto::esquinas_giradas(e);
    (0..4).any(|i| distancia_a_segmento(p, c[i], c[(i + 1) % 4]) <= radio)
}

impl Bolita {
    /// Empieza un barrido: lo tocado en el anterior vuelve a poder decidirse.
    pub fn empezar(&mut self) {
        self.ya_tocados.clear();
        self.centro = None;
    }

    /// Termina el barrido: la bolita deja de verse.
    pub fn terminar(&mut self) {
        self.ya_tocados.clear();
        self.centro = None;
    }

    /// **Pasa la bolita por `p`**: enciende y apaga lo que toca. `radio` ya en
    /// unidades del documento. Devuelve si cambio la seleccion.
    pub fn pasar(
        &mut self,
        p: Punto2,
        radio: f32,
        escena: &crate::Escena,
        seleccion: &mut Seleccion,
    ) -> bool {
        self.centro = Some(p);
        let mut cambio = false;
        let tocados: Vec<u64> = escena
            .elementos
            .iter()
            .filter(|e| !self.ya_tocados.contains(&e.id) && toca(e, p, radio))
            .map(|e| e.id)
            .collect();
        for id in tocados {
            if self.ya_tocados.contains(&id) {
                continue;
            }
            let grupo = crate::organizar::hermanos_de(escena, id);
            let dentro = seleccion.contiene(id);
            for &g in &grupo {
                self.ya_tocados.insert(g);
                if seleccion.contiene(g) == dentro {
                    seleccion.alternar(g);
                }
            }
            cambio = true;
        }
        cambio
    }

    /// El circulo que se ve mientras se pasa, del mismo tamano en pantalla a
    /// cualquier zoom: sin verlo, uno no sabe cuanto coge el gesto.
    pub fn orden(&self, zoom: f32) -> Option<Orden> {
        let c = self.centro?;
        let r = RADIO_DE_LA_BOLITA / zoom.max(0.0001);
        let puntos = (0..=32)
            .map(|i| {
                let t = i as f32 * std::f32::consts::TAU / 32.0;
                Punto2::nuevo(c.x + r * t.cos(), c.y + r * t.sin())
            })
            .collect();
        Some(Orden::Polilinea {
            puntos,
            // El lila de la marquesina: es la misma idea, elegir.
            color: ColorRgba {
                r: 0.41,
                g: 0.40,
                b: 0.84,
                a: 0.9,
            },
            grosor: 1.5 / zoom.max(0.0001),
            estilo: EstiloTrazo::Solido,
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::Escena;
    use crate::elemento::Figura;

    fn raya(a: (f32, f32), b: (f32, f32)) -> Elemento {
        Elemento {
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
            },
            x: a.0.min(b.0),
            y: a.1.min(b.1),
            ancho: (b.0 - a.0).abs(),
            alto: (b.1 - a.1).abs(),
            grosor: 2.0,
            ..Default::default()
        }
    }

    fn escena() -> (Escena, u64, u64) {
        let mut e = Escena::nueva();
        let a = e.anadir(raya((0.0, 0.0), (0.0, 100.0)));
        let b = e.anadir(raya((200.0, 0.0), (200.0, 100.0)));
        (e, a, b)
    }

    #[test]
    fn pasar_por_encima_mete_en_la_seleccion_y_volver_a_pasar_la_saca() {
        let (escena, a, b) = escena();
        let mut sel = Seleccion::nueva();
        let mut bolita = Bolita::default();
        bolita.empezar();
        // Un barrido que pasa por las dos.
        for x in (0..=200).step_by(10) {
            bolita.pasar(Punto2::nuevo(x as f32, 50.0), 30.0, &escena, &mut sel);
        }
        assert!(sel.contiene(a) && sel.contiene(b));
        bolita.terminar();
        // Otro barrido solo por la primera: sale, la otra se queda.
        bolita.empezar();
        bolita.pasar(Punto2::nuevo(5.0, 50.0), 30.0, &escena, &mut sel);
        assert!(!sel.contiene(a) && sel.contiene(b));
    }

    #[test]
    fn quedarse_encima_no_la_enciende_y_apaga_en_cada_aviso() {
        let (escena, a, _) = escena();
        let mut sel = Seleccion::nueva();
        let mut bolita = Bolita::default();
        bolita.empezar();
        for _ in 0..7 {
            bolita.pasar(Punto2::nuevo(3.0, 50.0), 30.0, &escena, &mut sel);
        }
        assert!(sel.contiene(a), "un barrido decide una sola vez");
    }

    #[test]
    fn lejos_de_todo_no_coge_nada_y_lo_bloqueado_tampoco() {
        let (mut escena, a, _) = escena();
        let mut sel = Seleccion::nueva();
        let mut bolita = Bolita::default();
        bolita.empezar();
        assert!(!bolita.pasar(Punto2::nuevo(100.0, 50.0), 30.0, &escena, &mut sel));
        escena.buscar_mut(a).unwrap().bloqueado = true;
        assert!(!bolita.pasar(Punto2::nuevo(0.0, 50.0), 30.0, &escena, &mut sel));
        assert!(sel.ids().is_empty());
    }

    #[test]
    fn tocar_una_pieza_de_un_grupo_coge_el_grupo_entero() {
        let (mut escena, a, b) = escena();
        for id in [a, b] {
            escena.buscar_mut(id).unwrap().grupos = vec!["g".into()];
        }
        let mut sel = Seleccion::nueva();
        let mut bolita = Bolita::default();
        bolita.empezar();
        bolita.pasar(Punto2::nuevo(0.0, 50.0), 30.0, &escena, &mut sel);
        assert!(sel.contiene(a) && sel.contiene(b));
    }

    #[test]
    fn la_bolita_se_ve_solo_mientras_se_pasa() {
        let mut bolita = Bolita::default();
        assert!(bolita.orden(1.0).is_none());
        bolita.centro = Some(Punto2::nuevo(0.0, 0.0));
        assert!(bolita.orden(2.0).is_some());
        bolita.terminar();
        assert!(bolita.orden(1.0).is_none());
    }
}
