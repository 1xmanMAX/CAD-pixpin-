//! **rough.js, como lo lleva el movil** (`Rough.kt`): el trazo «a mano» de
//! los rectangulos, los rombos y las elipses.
//!
//! Antes el escritorio dibujaba estas tres figuras con una aproximacion
//! propia (`formas::linea`): cada lado con los extremos sacudidos hasta 4 px y
//! la panza hasta 8, y la elipse con 32 vertices sacudidos hasta 4 px cada uno
//! por su lado. El usuario lo vio (2-oct-2026): «se ven muy angulosas y poco
//! realistas —el circulo, el cuadrado y el rombo— en el modo dibujado a mano».
//! Las esquinas no cerraban (cada lado temblaba sus extremos por separado,
//! cuatro veces mas que en Excalidraw), el circulo salia abollado y el doble
//! trazo se separaba tanto que parecian dos figuras.
//!
//! Esto es el original, numero a numero: las mismas operaciones, **en el
//! mismo orden de llamadas al azar** (cada `siguiente` avanza el generador, y
//! una llamada de mas o de menos cambia el dibujo entero), con los mismos
//! valores de fabrica (`maxRandomnessOffset` 2, `bowing` 1, `curveFitting`
//! 0,95, `curveStepCount` 9). Con la misma semilla, el mismo rectangulo sale
//! igual aqui, en el movil y en excalidraw.com.
//!
//! Devuelve ordenes de ruta ([`Op`]) —mover, recta, cubica— como el original;
//! [`a_pasadas`] las convierte en las polilineas que entiende `pintado`, con
//! las cubicas muestreadas lo bastante finas para que no se vea ni un pico.

use crate::azar::Azar;
use crate::vector::Punto2;

/// Las opciones de rough.js que cambian el aspecto (`RoughOptions` del
/// movil). Lo que no esta aqui vale lo de fabrica y no se toca.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opciones {
    /// `roughness`, ya ajustado al tamano (`adjustRoughness`).
    pub rugosidad: f64,
    /// `preserveVertices`: los extremos de cada tramo no se mueven. Es lo
    /// que Excalidraw pide por debajo de la rugosidad de dibujante, y lo que
    /// hace que las esquinas de un rectangulo cierren.
    pub preservar: bool,
    /// `disableMultiStroke`: una sola pasada. Excalidraw la pide en los
    /// trazos discontinuos, donde dos pasadas se verian como rayas sueltas.
    pub una_pasada: bool,
    /// `curveFitting`: 0,95 de fabrica; 1 en la elipse, que es lo que
    /// Excalidraw fuerza para que el circulo no salga abollado.
    pub ajuste_de_curva: f64,
}

impl Opciones {
    pub fn nuevas(rugosidad: f32, preservar: bool, una_pasada: bool) -> Self {
        Self {
            rugosidad: rugosidad.max(0.0) as f64,
            preservar,
            una_pasada,
            ajuste_de_curva: 0.95,
        }
    }
}

/// `maxRandomnessOffset`, `bowing` y `curveStepCount` de fabrica.
const DESVIO_MAXIMO: f64 = 2.0;
const PANZA: f64 = 1.0;
const PASOS_DE_CURVA: f64 = 9.0;

/// Un punto en doble precision, como el `Pt` del movil.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

impl Pt {
    pub fn nuevo(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Una orden de ruta. Las mismas que `Op` del movil y que un camino SVG.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Mover(Pt),
    Recta(Pt),
    Cubica(Pt, Pt, Pt),
}

/// El generador de UNA figura. Se crea uno por figura con su semilla: si se
/// compartiera, mover una cambiaria el garabato de la siguiente.
pub struct Rough {
    o: Opciones,
    azar: Azar,
}

impl Rough {
    pub fn nuevo(o: Opciones, semilla: u32) -> Self {
        Self::con_azar(o, Azar::nuevo(semilla))
    }

    /// Con un generador ya sembrado (el del elemento, en `pintado`).
    pub fn con_azar(o: Opciones, azar: Azar) -> Self {
        Self { o, azar }
    }

    /// El generador tal como queda, para quien siga gastando de el.
    pub fn azar(self) -> Azar {
        self.azar
    }

    fn al_azar(&mut self) -> f64 {
        self.azar.siguiente_f64()
    }

    /// `_offset`: ruido en `[min, max)` escalado por la rugosidad.
    fn desvio(&mut self, min: f64, max: f64, ganancia: f64) -> f64 {
        self.o.rugosidad * ganancia * (self.al_azar() * (max - min) + min)
    }

    /// `_offsetOpt`: ruido simetrico alrededor de cero.
    fn desvio_sim(&mut self, x: f64, ganancia: f64) -> f64 {
        self.desvio(-x, x, ganancia)
    }

    /// `_line`: una pasada de recta, una cubica con panza. `repaso` es la
    /// segunda (`overlay`), con la mitad de ruido.
    fn linea(&mut self, a: Pt, b: Pt, mover: bool, repaso: bool) -> Vec<Op> {
        let largo2 = (a.x - b.x).powi(2) + (a.y - b.y).powi(2);
        let largo = largo2.sqrt();
        let ganancia = if largo < 200.0 {
            1.0
        } else if largo > 500.0 {
            0.4
        } else {
            -0.0016668 * largo + 1.233334
        };
        let mut desvio = DESVIO_MAXIMO;
        if desvio * desvio * 100.0 > largo2 {
            desvio = largo / 10.0;
        }
        let medio = desvio / 2.0;
        let divergencia = 0.2 + self.al_azar() * 0.2;
        let panza_x = PANZA * DESVIO_MAXIMO * (b.y - a.y) / 200.0;
        let panza_y = PANZA * DESVIO_MAXIMO * (a.x - b.x) / 200.0;
        let panza_x = self.desvio_sim(panza_x, ganancia);
        let panza_y = self.desvio_sim(panza_y, ganancia);
        let quieto = self.o.preservar;
        let d = if repaso { medio } else { desvio };
        let mut ops = Vec::with_capacity(2);
        if mover {
            let x = a.x
                + if quieto {
                    0.0
                } else {
                    self.desvio_sim(d, ganancia)
                };
            let y = a.y
                + if quieto {
                    0.0
                } else {
                    self.desvio_sim(d, ganancia)
                };
            ops.push(Op::Mover(Pt::nuevo(x, y)));
        }
        let c1x = panza_x + a.x + (b.x - a.x) * divergencia + self.desvio_sim(d, ganancia);
        let c1y = panza_y + a.y + (b.y - a.y) * divergencia + self.desvio_sim(d, ganancia);
        let c2x = panza_x + a.x + 2.0 * (b.x - a.x) * divergencia + self.desvio_sim(d, ganancia);
        let c2y = panza_y + a.y + 2.0 * (b.y - a.y) * divergencia + self.desvio_sim(d, ganancia);
        let fx = b.x
            + if quieto {
                0.0
            } else {
                self.desvio_sim(d, ganancia)
            };
        let fy = b.y
            + if quieto {
                0.0
            } else {
                self.desvio_sim(d, ganancia)
            };
        ops.push(Op::Cubica(
            Pt::nuevo(c1x, c1y),
            Pt::nuevo(c2x, c2y),
            Pt::nuevo(fx, fy),
        ));
        ops
    }

    /// `_doubleLine`: las dos pasadas que dan el aspecto de boceto.
    pub fn doble_linea(&mut self, a: Pt, b: Pt) -> Vec<Op> {
        let mut ops = self.linea(a, b, true, false);
        if !self.o.una_pasada {
            ops.extend(self.linea(a, b, true, true));
        }
        ops
    }

    /// `linearPath`: tramos rectos, cerrado o no.
    pub fn camino_lineal(&mut self, puntos: &[Pt], cerrar: bool) -> Vec<Op> {
        let mut p = puntos.to_vec();
        if cerrar && puntos.len() > 2 {
            p.push(puntos[0]);
        }
        let mut ops = Vec::new();
        for par in p.windows(2) {
            ops.extend(self.doble_linea(par[0], par[1]));
        }
        ops
    }

    pub fn poligono(&mut self, puntos: &[Pt]) -> Vec<Op> {
        self.camino_lineal(puntos, true)
    }

    pub fn rectangulo(&mut self, x: f64, y: f64, w: f64, h: f64) -> Vec<Op> {
        self.poligono(&[
            Pt::nuevo(x, y),
            Pt::nuevo(x + w, y),
            Pt::nuevo(x + w, y + h),
            Pt::nuevo(x, y + h),
        ])
    }

    /// `_curve`: la spline de Catmull-Rom por `puntos`, en cubicas
    /// (`curveTightness` 0). Pasa por todos menos el primero y el ultimo,
    /// que solo dan la tangente.
    fn curva(&mut self, p: &[Pt]) -> Vec<Op> {
        let n = p.len();
        let mut ops = Vec::new();
        if n > 3 {
            ops.push(Op::Mover(p[1]));
            let mut i = 1;
            while i + 2 < n {
                let c1 = Pt::nuevo(
                    p[i].x + (p[i + 1].x - p[i - 1].x) / 6.0,
                    p[i].y + (p[i + 1].y - p[i - 1].y) / 6.0,
                );
                let c2 = Pt::nuevo(
                    p[i + 1].x + (p[i].x - p[i + 2].x) / 6.0,
                    p[i + 1].y + (p[i].y - p[i + 2].y) / 6.0,
                );
                ops.push(Op::Cubica(c1, c2, p[i + 1]));
                i += 1;
            }
        } else if n == 3 {
            ops.push(Op::Mover(p[1]));
            ops.push(Op::Cubica(p[1], p[2], p[2]));
        } else if n == 2 {
            ops.extend(self.linea(p[0], p[1], true, true));
        }
        ops
    }

    /// `_bezierTo`: una cubica de la ruta (la esquina de un redondeado), dos
    /// veces, con los tiradores sacudidos `maxRandomnessOffset` (2 y 2,3 por
    /// la rugosidad). Con `preservar` los extremos no se mueven, y la
    /// esquina empalma con las rectas de los lados.
    pub fn cubica_a(&mut self, desde: Pt, c1: Pt, c2: Pt, a: Pt) -> Vec<Op> {
        let desvios = [DESVIO_MAXIMO, DESVIO_MAXIMO + 0.3];
        let vueltas = if self.o.una_pasada { 1 } else { 2 };
        let quieto = self.o.preservar;
        let mut ops = Vec::with_capacity(2 * vueltas);
        for (i, &d) in desvios.iter().enumerate().take(vueltas) {
            if i == 0 || quieto {
                ops.push(Op::Mover(desde));
            } else {
                let x = desde.x + self.desvio_sim(desvios[0], 1.0);
                let y = desde.y + self.desvio_sim(desvios[0], 1.0);
                ops.push(Op::Mover(Pt::nuevo(x, y)));
            }
            let fin = if quieto {
                a
            } else {
                let x = a.x + self.desvio_sim(d, 1.0);
                let y = a.y + self.desvio_sim(d, 1.0);
                Pt::nuevo(x, y)
            };
            let t1x = c1.x + self.desvio_sim(d, 1.0);
            let t1y = c1.y + self.desvio_sim(d, 1.0);
            let t2x = c2.x + self.desvio_sim(d, 1.0);
            let t2y = c2.y + self.desvio_sim(d, 1.0);
            ops.push(Op::Cubica(Pt::nuevo(t1x, t1y), Pt::nuevo(t2x, t2y), fin));
        }
        ops
    }

    /// `_computeEllipsePoints`: los puntos de una vuelta. Devuelve los del
    /// trazo (con el arranque y el cierre solapados) y los del nucleo, que
    /// son los del relleno.
    fn puntos_de_elipse(
        &mut self,
        paso: f64,
        (cx, cy, rx, ry): (f64, f64, f64, f64),
        desvio: f64,
        solape: f64,
    ) -> (Vec<Pt>, Vec<Pt>) {
        let mut nucleo = Vec::new();
        let mut todos = Vec::new();
        if self.o.rugosidad == 0.0 {
            let paso = paso / 4.0;
            todos.push(Pt::nuevo(cx + rx * (-paso).cos(), cy + ry * (-paso).sin()));
            let mut a = 0.0;
            while a <= std::f64::consts::TAU {
                let p = Pt::nuevo(cx + rx * a.cos(), cy + ry * a.sin());
                nucleo.push(p);
                todos.push(p);
                a += paso;
            }
            todos.push(Pt::nuevo(cx + rx, cy));
            todos.push(Pt::nuevo(cx + rx * paso.cos(), cy + ry * paso.sin()));
        } else {
            let giro = self.desvio_sim(0.5, 1.0) - std::f64::consts::FRAC_PI_2;
            let x = self.desvio_sim(desvio, 1.0) + cx + 0.9 * rx * (giro - paso).cos();
            let y = self.desvio_sim(desvio, 1.0) + cy + 0.9 * ry * (giro - paso).sin();
            todos.push(Pt::nuevo(x, y));
            let fin = std::f64::consts::TAU + giro - 0.01;
            let mut a = giro;
            while a < fin {
                let x = self.desvio_sim(desvio, 1.0) + cx + rx * a.cos();
                let y = self.desvio_sim(desvio, 1.0) + cy + ry * a.sin();
                let p = Pt::nuevo(x, y);
                nucleo.push(p);
                todos.push(p);
                a += paso;
            }
            for (factor, angulo) in [
                (1.0, giro + std::f64::consts::TAU + solape * 0.5),
                (0.98, giro + solape),
                (0.9, giro + solape * 0.5),
            ] {
                let x = self.desvio_sim(desvio, 1.0) + cx + factor * rx * angulo.cos();
                let y = self.desvio_sim(desvio, 1.0) + cy + factor * ry * angulo.sin();
                todos.push(Pt::nuevo(x, y));
            }
        }
        (todos, nucleo)
    }

    /// La elipse de centro `(cx, cy)`: el trazo y el poligono del relleno
    /// (`ellipse` del movil, `ellipseWithParams` de rough.js).
    pub fn elipse(&mut self, cx: f64, cy: f64, w: f64, h: f64) -> (Vec<Op>, Vec<Pt>) {
        // `generateEllipseParams`: cuantos puntos, segun el perimetro.
        let psq =
            (std::f64::consts::TAU * (((w / 2.0).powi(2) + (h / 2.0).powi(2)) / 2.0).sqrt()).sqrt();
        let pasos = PASOS_DE_CURVA
            .max(PASOS_DE_CURVA / 200f64.sqrt() * psq)
            .ceil();
        let paso = std::f64::consts::TAU / pasos;
        let (mut rx, mut ry) = ((w / 2.0).abs(), (h / 2.0).abs());
        let holgura = 1.0 - self.o.ajuste_de_curva;
        rx += self.desvio_sim(rx * holgura, 1.0);
        ry += self.desvio_sim(ry * holgura, 1.0);

        let interior = self.desvio(0.4, 1.0, 1.0);
        let solape = paso * self.desvio(0.1, interior, 1.0);
        let (todos, nucleo) = self.puntos_de_elipse(paso, (cx, cy, rx, ry), 1.0, solape);
        let mut ops = self.curva(&todos);
        if self.o.rugosidad != 0.0 && !self.o.una_pasada {
            let (todos2, _) = self.puntos_de_elipse(paso, (cx, cy, rx, ry), 1.5, 0.0);
            ops.extend(self.curva(&todos2));
        }
        (ops, nucleo)
    }
}

/// Lo que la quebrada puede separarse, como mucho, de la cubica que
/// muestrea: una vigesima de unidad, que al 400 % sigue siendo un quinto de
/// pixel. Cuantos trocitos hacen falta sale de lo que se dobla la curva (lo
/// que sus tiradores se apartan de la cuerda), no de lo que mide: un lado de rectangulo, casi recto,
/// se queda en ocho puntos, y la elipse pone los que su curva pide. Repartir
/// por largo ponia 64 en cada lado largo sin que se viera nada.
const DESVIO_DE_LA_QUEBRADA: f64 = 0.05;

/// Las ordenes en polilineas: cada `Mover` abre una; las cubicas se
/// muestrean con la cota de [`DESVIO_DE_LA_QUEBRADA`].
pub fn a_pasadas(ops: &[Op]) -> Vec<Vec<Punto2>> {
    let a32 = |p: Pt| Punto2::nuevo(p.x as f32, p.y as f32);
    let mut salida: Vec<Vec<Punto2>> = Vec::new();
    let mut actual: Vec<Punto2> = Vec::new();
    let mut ultimo = Pt::nuevo(0.0, 0.0);
    for op in ops {
        match *op {
            Op::Mover(p) => {
                if actual.len() > 1 {
                    salida.push(std::mem::take(&mut actual));
                }
                actual.clear();
                actual.push(a32(p));
                ultimo = p;
            }
            Op::Recta(p) => {
                if actual.is_empty() {
                    actual.push(a32(ultimo));
                }
                actual.push(a32(p));
                ultimo = p;
            }
            Op::Cubica(c1, c2, p) => {
                if actual.is_empty() {
                    actual.push(a32(ultimo));
                }
                // Lo que la curva se aparta de su cuerda es como mucho 3/4 de
                // lo que se apartan los tiradores; con n trozos la quebrada
                // se queda a esa flecha / n^2.
                let fuera = |c: Pt| {
                    let (dx, dy) = (p.x - ultimo.x, p.y - ultimo.y);
                    let largo = (dx * dx + dy * dy).sqrt();
                    if largo < 1e-9 {
                        ((c.x - p.x).powi(2) + (c.y - p.y).powi(2)).sqrt()
                    } else {
                        ((c.x - ultimo.x) * dy - (c.y - ultimo.y) * dx).abs() / largo
                    }
                };
                let m = fuera(c1).max(fuera(c2));
                let trozos =
                    ((0.75 * m / DESVIO_DE_LA_QUEBRADA).sqrt().ceil() as usize).clamp(2, 64);
                for k in 1..=trozos {
                    let t = k as f64 / trozos as f64;
                    let u = 1.0 - t;
                    let (a, b, c, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    actual.push(Punto2::nuevo(
                        (ultimo.x * a + c1.x * b + c2.x * c + p.x * e) as f32,
                        (ultimo.y * a + c1.y * b + c2.y * c + p.y * e) as f32,
                    ));
                }
                ultimo = p;
            }
        }
    }
    if actual.len() > 1 {
        salida.push(actual);
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn a_mano(rugosidad: f32) -> Opciones {
        Opciones::nuevas(rugosidad, rugosidad < 2.0, false)
    }

    fn cubicas(ops: &[Op]) -> usize {
        ops.iter().filter(|o| matches!(o, Op::Cubica(..))).count()
    }

    fn movimientos(ops: &[Op]) -> usize {
        ops.iter().filter(|o| matches!(o, Op::Mover(..))).count()
    }

    #[test]
    fn la_elipse_es_dos_vueltas_de_curvas_y_no_una_quebrada() {
        for r in [1.0f32, 2.0] {
            let (ops, nucleo) = Rough::nuevo(a_mano(r), 7).elipse(100.0, 100.0, 200.0, 200.0);
            assert_eq!(movimientos(&ops), 2, "dos pasadas, como rough.js (r={r})");
            assert!(
                !ops.iter().any(|o| matches!(o, Op::Recta(_))),
                "ni una recta (r={r})"
            );
            // 200 de ancho: 16 pasos (`generateEllipseParams`), y la vuelta
            // entera son curvas, no vertices unidos con rectas.
            assert_eq!(nucleo.len(), 16);
            assert!(cubicas(&ops) >= 2 * 16, "{} cubicas", cubicas(&ops));
        }
    }

    #[test]
    fn la_elipse_a_mano_cierra_con_un_poco_de_solape() {
        // El final de la primera pasada se pasa del arranque: una elipse a
        // mano casi nunca cierra exacta, y ese exceso es lo que la hace
        // creible. Pero poco: el ultimo punto cae cerca del primero.
        let ops = Rough::nuevo(a_mano(1.0), 3)
            .elipse(0.0, 0.0, 200.0, 120.0)
            .0;
        let pasadas = a_pasadas(&ops);
        let v = &pasadas[0];
        let (primero, ultimo) = (v[0], *v.last().unwrap());
        assert!(
            primero.distancia(ultimo) < 25.0,
            "cierra: {}",
            primero.distancia(ultimo)
        );
    }

    #[test]
    fn la_elipse_no_se_aleja_de_la_ideal_mas_que_unos_pixeles() {
        // El tope a mano no es el temblor (1 px por punto) sino el gancho del
        // cierre de rough.js: la vuelta acaba hacia dentro, por el punto al 98 %
        // del radio con el del 90 % de tirador, como en excalidraw.com.
        for (r, tope) in [(0.0f32, 0.2f32), (1.0, 6.0), (2.0, 9.0)] {
            let (w, h) = (300.0f32, 180.0f32);
            let o = Opciones {
                ajuste_de_curva: 1.0,
                ..a_mano(r)
            };
            let ops = Rough::nuevo(o, 11).elipse(0.0, 0.0, w as f64, h as f64).0;
            for v in a_pasadas(&ops) {
                for p in v {
                    // Distancia radial aproximada a la elipse ideal.
                    let k = ((p.x / (w / 2.0)).powi(2) + (p.y / (h / 2.0)).powi(2)).sqrt();
                    let radio = (p.x.powi(2) + p.y.powi(2)).sqrt();
                    let fuera = (radio - radio / k.max(1e-6)).abs();
                    assert!(fuera < tope, "r={r}: un punto a {fuera} de la elipse");
                }
            }
        }
    }

    #[test]
    fn sin_rugosidad_la_elipse_es_una_sola_vuelta_cerrada_y_exacta() {
        let (ops, _) = Rough::nuevo(a_mano(0.0), 5).elipse(0.0, 0.0, 100.0, 100.0);
        assert_eq!(movimientos(&ops), 1);
        let v = &a_pasadas(&ops)[0];
        assert!(v[0].distancia(*v.last().unwrap()) < 0.5, "cerrada");
        for p in v {
            assert!(((p.x.powi(2) + p.y.powi(2)).sqrt() - 50.0).abs() < 0.2);
        }
    }

    #[test]
    fn la_misma_semilla_da_el_mismo_dibujo_y_otra_semilla_otro() {
        let hacer = |s| Rough::nuevo(a_mano(1.0), s).elipse(0.0, 0.0, 120.0, 80.0).0;
        assert_eq!(hacer(42), hacer(42));
        assert_ne!(hacer(42), hacer(43));
        let rect = |s| Rough::nuevo(a_mano(1.0), s).rectangulo(0.0, 0.0, 120.0, 80.0);
        assert_eq!(rect(9), rect(9));
    }

    #[test]
    fn el_rectangulo_artista_cierra_sus_esquinas_y_dobla_cada_lado() {
        // `preserveVertices` (rugosidad < 2): cada pasada empieza y acaba en
        // el vertice exacto, asi que las esquinas cierran.
        let ops = Rough::nuevo(a_mano(1.0), 1).rectangulo(10.0, 20.0, 200.0, 100.0);
        assert_eq!(movimientos(&ops), 8, "cuatro lados, dos pasadas");
        let esquinas = [(10.0, 20.0), (210.0, 20.0), (210.0, 120.0), (10.0, 120.0)];
        for v in a_pasadas(&ops) {
            let (a, b) = (v[0], *v.last().unwrap());
            let es_esquina = |p: Punto2| {
                esquinas
                    .iter()
                    .any(|&(x, y)| (p.x - x).abs() < 1e-3 && (p.y - y).abs() < 1e-3)
            };
            assert!(es_esquina(a) && es_esquina(b), "{a:?} {b:?}");
            // Y la panza es pequena: nada se va mas de 3 px de su lado.
            let lado_h = (a.y - b.y).abs() < 1e-3;
            for p in &v {
                let d = if lado_h {
                    (p.y - a.y).abs()
                } else {
                    (p.x - a.x).abs()
                };
                assert!(d < 3.0, "una panza de {d}");
            }
        }
    }

    #[test]
    fn la_geometria_es_ligera_para_el_pintor() {
        // Un rectangulo grande y una elipse grande no llenan la ruta de
        // Direct2D de puntos: la quebrada sigue a la curva, no al largo.
        let n = |v: Vec<Vec<Punto2>>| v.iter().map(Vec::len).sum::<usize>();
        let rect = a_pasadas(&Rough::nuevo(a_mano(1.0), 2).rectangulo(0.0, 0.0, 900.0, 600.0));
        assert!(n(rect) < 260, "rectangulo");
        let elipse = a_pasadas(
            &Rough::nuevo(a_mano(1.0), 2)
                .elipse(0.0, 0.0, 900.0, 600.0)
                .0,
        );
        assert!(n(elipse) < 1200, "elipse");
    }

    #[test]
    fn de_dibujante_los_vertices_tiemblan_y_las_dos_pasadas_no_coinciden() {
        let ops = Rough::nuevo(a_mano(2.0), 1).rectangulo(0.0, 0.0, 200.0, 100.0);
        let p = a_pasadas(&ops);
        assert_ne!(p[0][0], Punto2::nuevo(0.0, 0.0));
        assert_ne!(p[0], p[1]);
    }

    #[test]
    fn discontinuo_va_en_una_sola_pasada() {
        let o = Opciones::nuevas(1.0, true, true);
        assert_eq!(
            movimientos(&Rough::nuevo(o, 1).rectangulo(0.0, 0.0, 50.0, 50.0)),
            4
        );
        assert_eq!(
            movimientos(&Rough::nuevo(o, 1).elipse(0.0, 0.0, 50.0, 50.0).0),
            1
        );
    }

    /// Los numeros de rough.js 4.6.6 (el de excalidraw.com) con la misma
    /// semilla, sacados con node: `generator.rectangle(10, 20, 200, 100,
    /// {seed: 9, ...})` y `generator.ellipse(100, 100, 300, 180, {seed: 11,
    /// curveFitting: 1, preserveVertices: true})`. Si esto falla, el mismo
    /// fichero ya no se ve igual aqui que alla.
    #[test]
    fn sale_lo_mismo_que_rough_js_con_la_misma_semilla() {
        let cerca = |a: Pt, x: f64, y: f64| (a.x - x).abs() < 1e-3 && (a.y - y).abs() < 1e-3;
        let ops = Rough::nuevo(Opciones::nuevas(1.0, true, false), 9)
            .rectangulo(10.0, 20.0, 200.0, 100.0);
        let Op::Cubica(c1, c2, f) = ops[1] else {
            panic!("{:?}", ops[1])
        };
        assert!(
            cerca(c1, 49.7519, 20.9244) && cerca(c2, 91.0261, 18.9387) && cerca(f, 210.0, 20.0),
            "{:?}",
            ops[1]
        );
        let ops = Rough::nuevo(Opciones::nuevas(2.0, false, false), 9)
            .rectangulo(10.0, 20.0, 200.0, 100.0);
        let Op::Mover(m) = ops[0] else { panic!() };
        assert!(cerca(m, 9.4877, 21.2149), "{m:?}");
        let Op::Cubica(_, _, f) = ops[15] else {
            panic!()
        };
        assert!(cerca(f, 9.9333, 21.7845), "{f:?}");
        let o = Opciones {
            ajuste_de_curva: 1.0,
            ..Opciones::nuevas(1.0, true, false)
        };
        let ops = Rough::nuevo(o, 11).elipse(100.0, 100.0, 300.0, 180.0).0;
        let Op::Mover(m) = ops[0] else { panic!() };
        assert!(cerca(m, 49.5809, 15.9195), "{m:?}");
        let Op::Cubica(c1, _, f) = ops[1] else {
            panic!()
        };
        assert!(
            cerca(c1, 63.9407, 11.2190) && cerca(f, 99.5976, 10.1578),
            "{:?}",
            ops[1]
        );
    }

    #[test]
    fn la_esquina_de_un_redondeado_empalma_y_es_lisa() {
        // `_bezierTo` con los vertices quietos: las dos pasadas empiezan y
        // acaban donde la ruta, asi la esquina empalma con los lados.
        let (a, c1, c2, b) = (
            Pt::nuevo(0.0, 0.0),
            Pt::nuevo(20.0, 0.0),
            Pt::nuevo(32.0, 12.0),
            Pt::nuevo(32.0, 32.0),
        );
        let ops = Rough::nuevo(Opciones::nuevas(1.0, true, false), 3).cubica_a(a, c1, c2, b);
        assert_eq!(movimientos(&ops), 2);
        for v in a_pasadas(&ops) {
            assert_eq!(v[0], Punto2::nuevo(0.0, 0.0));
            assert_eq!(*v.last().unwrap(), Punto2::nuevo(32.0, 32.0));
            for w in v.windows(3) {
                let (p, q) = (w[1].restar(w[0]), w[2].restar(w[1]));
                let coseno = p.producto(q) / (p.longitud() * q.longitud()).max(1e-6);
                assert!(coseno > 0.8, "un pico: {coseno}");
            }
        }
    }
}
