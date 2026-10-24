//! **El puntero laser** (F14): una estela roja que sigue al raton y se
//! desvanece sola, para senalar sin dejar rastro en el dibujo.
//!
//! El movil no lo tiene (`Scene.kt`: «quedan fuera frame, embeddable, laser
//! y bote»); es la herramienta de Excalidraw (`LaserTrails.ts`, tecla K alli),
//! y en un escritorio es la que se usa al presentar o al explicar un plano
//! en una llamada. Por eso va sobre todo con **Presentar** (G5).
//!
//! No toca la escena ni el historial: vive en el gesto como el lazo, y lo
//! pinta quien pinta lo de encima (`dibujo::pintar::pintar_encima`). Las
//! cuentas son las de Excalidraw: cada punto se apaga en un segundo
//! (`DECAY_TIME`) y la estela no pasa de cincuenta puntos por detras de la
//! punta (`DECAY_LENGTH`), con la suavidad `easeOut` de su `sizeMapping`.

use crate::elemento::ColorRgba;
use crate::pintado::Orden;
use crate::vector::Punto2;

/// Lo que tarda en apagarse un punto de la estela (`DECAY_TIME`).
pub const DURACION_MS: f64 = 1000.0;
/// Cuantos puntos por detras de la punta llega la estela (`DECAY_LENGTH`).
pub const LARGO: usize = 50;
/// El grueso de la punta, en pixeles de pantalla.
pub const ANCHO_PX: f32 = 5.0;
/// El rojo del laser de Excalidraw.
pub const COLOR: ColorRgba = ColorRgba {
    r: 1.0,
    g: 0.1,
    b: 0.1,
    a: 0.9,
};

/// `easeOut` de Excalidraw: `1 - (1 - k)^4`.
fn suave(k: f32) -> f32 {
    1.0 - (1.0 - k.clamp(0.0, 1.0)).powi(4)
}

/// Las estelas que hay a la vista y si el boton sigue pulsado.
#[derive(Debug, Clone)]
pub struct PunteroLaser {
    /// Cada arrastre es una estela: `(punto, ms)` en el reloj de `reloj`.
    estelas: Vec<Vec<(Punto2, f64)>>,
    pulsado: bool,
    reloj: std::time::Instant,
}

impl Default for PunteroLaser {
    fn default() -> Self {
        Self {
            estelas: Vec::new(),
            pulsado: false,
            reloj: std::time::Instant::now(),
        }
    }
}

impl PunteroLaser {
    /// Los milisegundos del reloj de la estela.
    pub fn ahora(&self) -> f64 {
        self.reloj.elapsed().as_secs_f64() * 1000.0
    }

    pub fn pulsar_en(&mut self, p: Punto2, ms: f64) {
        self.podar(ms);
        self.pulsado = true;
        self.estelas.push(vec![(p, ms)]);
    }

    /// Un punto mas de la estela en curso. Sin boton pulsado no hace nada:
    /// el laser, como en Excalidraw, se enciende al pulsar.
    pub fn mover_en(&mut self, p: Punto2, ms: f64) -> bool {
        if !self.pulsado {
            return false;
        }
        if let Some(e) = self.estelas.last_mut() {
            e.push((p, ms));
        }
        true
    }

    pub fn soltar_en(&mut self, ms: f64) {
        self.pulsado = false;
        self.podar(ms);
    }

    pub fn pulsar(&mut self, p: Punto2) {
        let ms = self.ahora();
        self.pulsar_en(p, ms);
    }

    pub fn mover(&mut self, p: Punto2) -> bool {
        let ms = self.ahora();
        self.mover_en(p, ms)
    }

    pub fn soltar(&mut self) {
        let ms = self.ahora();
        self.soltar_en(ms);
    }

    /// Se olvida todo (al cambiar de herramienta).
    pub fn vaciar(&mut self) {
        self.estelas.clear();
        self.pulsado = false;
    }

    /// Quita lo que ya se apago del todo.
    fn podar(&mut self, ms: f64) {
        let pulsado = self.pulsado;
        let n = self.estelas.len();
        let mut i = 0;
        self.estelas.retain_mut(|e| {
            let viva = pulsado && i + 1 == n;
            i += 1;
            let apagados = e.iter().take_while(|(_, t)| ms - t >= DURACION_MS).count();
            // De la estela en curso se guarda siempre la punta.
            let quitar = if viva {
                apagados.min(e.len().saturating_sub(1))
            } else {
                apagados
            };
            e.drain(..quitar);
            // Y nunca mas de lo que se pinta.
            if e.len() > LARGO * 2 {
                let sobra = e.len() - LARGO * 2;
                e.drain(..sobra);
            }
            !e.is_empty()
        });
    }

    /// Si todavia hay algo que ensenar: mientras sea que si, quien pinta
    /// tiene que seguir repintando aunque no lleguen eventos.
    pub fn vivo_en(&self, ms: f64) -> bool {
        self.pulsado
            || self
                .estelas
                .iter()
                .any(|e| e.iter().any(|(_, t)| ms - t < DURACION_MS))
    }

    pub fn vivo(&self) -> bool {
        self.vivo_en(self.ahora())
    }

    /// **Lo que se pinta**: cada estela como una mancha de tinta que adelgaza
    /// hacia la cola, mas un punto en la punta mientras se tiene pulsado.
    /// `zoom` es el aumento: el grueso es de pantalla, no del dibujo.
    pub fn ordenes_en(&self, ms: f64, zoom: f32) -> Vec<Orden> {
        let escala = 1.0 / zoom.max(1e-4);
        let mut v = Vec::new();
        let n = self.estelas.len();
        for (k, e) in self.estelas.iter().enumerate() {
            let total = e.len();
            // El grueso de cada punto: el menor entre lo que se ha apagado
            // con el tiempo y lo lejos que queda de la punta.
            let anchos: Vec<f32> = e
                .iter()
                .enumerate()
                .map(|(i, (_, t))| {
                    let por_tiempo = 1.0 - ((ms - t) / DURACION_MS) as f32;
                    let desde_la_punta = (total - 1 - i).min(LARGO) as f32;
                    let por_largo = (LARGO as f32 - desde_la_punta) / LARGO as f32;
                    suave(por_tiempo).min(suave(por_largo)) * ANCHO_PX * escala
                })
                .collect();
            if total >= 2 {
                if let Some(c) = contorno(e, &anchos) {
                    v.push(Orden::Tinta {
                        contorno: c,
                        color: COLOR,
                    });
                }
            }
            if self.pulsado && k + 1 == n {
                let (p, _) = e[total - 1];
                v.push(Orden::Tinta {
                    contorno: circulo(p, ANCHO_PX * 0.8 * escala),
                    color: COLOR,
                });
            }
        }
        v
    }

    pub fn ordenes(&self, zoom: f32) -> Vec<Orden> {
        self.ordenes_en(self.ahora(), zoom)
    }
}

/// El contorno de una raya de grueso variable: los bordes de un lado y, de
/// vuelta, los del otro. `None` si todo se apago.
fn contorno(e: &[(Punto2, f64)], anchos: &[f32]) -> Option<Vec<Punto2>> {
    let n = e.len();
    let desde = anchos.iter().position(|w| *w > 0.05)?;
    let tramo = &e[desde..];
    let anchos = &anchos[desde..];
    if tramo.len() < 2 {
        return None;
    }
    let mut izq = Vec::with_capacity(tramo.len());
    let mut der = Vec::with_capacity(tramo.len());
    for i in 0..tramo.len() {
        let a = tramo[i.saturating_sub(1)].0;
        let b = tramo[(i + 1).min(tramo.len() - 1)].0;
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let l = dx.hypot(dy);
        let (nx, ny) = if l > 1e-6 {
            (-dy / l, dx / l)
        } else {
            (0.0, 0.0)
        };
        let p = tramo[i].0;
        let m = anchos[i] / 2.0;
        izq.push(Punto2::nuevo(p.x + nx * m, p.y + ny * m));
        der.push(Punto2::nuevo(p.x - nx * m, p.y - ny * m));
    }
    let _ = n;
    der.reverse();
    izq.extend(der);
    Some(izq)
}

fn circulo(c: Punto2, r: f32) -> Vec<Punto2> {
    (0..16)
        .map(|i| {
            let a = i as f32 / 16.0 * std::f32::consts::TAU;
            Punto2::nuevo(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn p(x: f32) -> Punto2 {
        Punto2::nuevo(x, 0.0)
    }

    #[test]
    fn la_estela_se_ve_mientras_se_arrastra_y_se_apaga_en_un_segundo() {
        let mut l = PunteroLaser::default();
        l.pulsar_en(p(0.0), 0.0);
        for i in 1..20 {
            l.mover_en(p(i as f32 * 5.0), i as f64 * 10.0);
        }
        assert!(l.vivo_en(200.0));
        assert_eq!(l.ordenes_en(200.0, 1.0).len(), 2, "estela y punta");
        l.soltar_en(200.0);
        assert!(l.vivo_en(900.0), "recien soltado todavia se ve");
        assert_eq!(l.ordenes_en(900.0, 1.0).len(), 1, "sin la punta");
        // Caso negativo: pasado el segundo del ultimo punto, nada.
        assert!(!l.vivo_en(1300.0));
        l.soltar_en(1300.0);
        assert!(l.ordenes_en(1300.0, 1.0).is_empty());
    }

    #[test]
    fn sin_pulsar_moverse_no_deja_estela() {
        let mut l = PunteroLaser::default();
        assert!(!l.mover_en(p(10.0), 0.0));
        assert!(!l.vivo_en(0.0));
        assert!(l.ordenes_en(0.0, 1.0).is_empty());
    }

    #[test]
    fn la_estela_adelgaza_hacia_la_cola_y_no_pasa_de_su_largo() {
        let mut l = PunteroLaser::default();
        l.pulsar_en(p(0.0), 0.0);
        for i in 1..300 {
            l.mover_en(p(i as f32), 0.0);
        }
        l.soltar_en(0.0);
        assert!(l.estelas[0].len() <= LARGO * 2);
        let o = l.ordenes_en(0.0, 1.0);
        let Orden::Tinta { contorno, .. } = &o[0] else {
            panic!("una mancha de tinta");
        };
        // El contorno va de la cola a la punta y vuelve: el mas ancho, cerca
        // de la punta.
        let alto = |q: &Punto2| q.y.abs();
        let cola = alto(&contorno[0]);
        let punta = alto(&contorno[contorno.len() / 2 - 1]);
        assert!(punta > cola, "{punta} > {cola}");
    }

    #[test]
    fn el_grueso_es_de_pantalla_y_no_del_dibujo() {
        let mut l = PunteroLaser::default();
        l.pulsar_en(p(0.0), 0.0);
        l.mover_en(p(10.0), 0.0);
        let ancho = |zoom: f32| match &l.ordenes_en(0.0, zoom)[0] {
            Orden::Tinta { contorno, .. } => contorno.iter().map(|q| q.y).fold(0.0f32, f32::max),
            _ => 0.0,
        };
        assert!((ancho(1.0) - 2.0 * ancho(2.0)).abs() < 1e-3);
    }
}
