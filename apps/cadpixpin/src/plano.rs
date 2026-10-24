//! **El plano en la tarjeta grafica**: el dibujo se pasa a rayas y
//! triangulos con el convertidor de `pixpin-cad` (el mismo del visor de
//! PixPin) en otro hilo, cada vez que cambia, y se sube cuando llega. La
//! camara es la de la mesa, en coordenadas absolutas del dibujo.

use std::sync::mpsc::{Receiver, Sender, channel};

use cad_nucleo::geo::{Caja, P};
use cad_nucleo::opencadcodec::CadDocument;
use pixpin_cad::gpu::{Gpu, PlanoGpu, Vista};
use pixpin_cad::modelo::Modelo;

/// Que punto del dibujo esta en el centro de la ventana y cuantas unidades
/// mide un pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camara {
    pub centro: P,
    pub px: f64,
}

impl Camara {
    pub fn plano(&self, x: f64, y: f64, w: f64, h: f64) -> P {
        [
            self.centro[0] + (x - w / 2.0) * self.px,
            self.centro[1] - (y - h / 2.0) * self.px,
        ]
    }

    pub fn pantalla(&self, p: P, w: f64, h: f64) -> (f32, f32) {
        (
            ((p[0] - self.centro[0]) / self.px + w / 2.0) as f32,
            (h / 2.0 - (p[1] - self.centro[1]) / self.px) as f32,
        )
    }

    /// Acercar `factor` (<1 acerca) dejando quieto el punto bajo (x, y).
    pub fn zoom(&self, factor: f64, x: f64, y: f64, w: f64, h: f64) -> Camara {
        let antes = self.plano(x, y, w, h);
        let mut c = Camara {
            centro: self.centro,
            px: (self.px * factor).clamp(1e-9, 1e9),
        };
        let despues = c.plano(x, y, w, h);
        c.centro = [
            c.centro[0] + antes[0] - despues[0],
            c.centro[1] + antes[1] - despues[1],
        ];
        c
    }

    /// La caja entera dentro de la zona `zona` (x, y, ancho, alto en pixeles)
    /// de una ventana `w` x `h`.
    pub fn encuadrar(caja: Caja, zona: [f64; 4], w: f64, h: f64) -> Camara {
        let (cw, ch) = ((caja[2] - caja[0]).max(1e-9), (caja[3] - caja[1]).max(1e-9));
        let mut px = (cw / (zona[2] * 0.9).max(1.0)).max(ch / (zona[3] * 0.9).max(1.0));
        if !(px.is_finite() && px > 0.0) {
            px = 1.0;
        }
        // El centro de la caja en el centro de la zona (no de la ventana).
        let medio = [(caja[0] + caja[2]) / 2.0, (caja[1] + caja[3]) / 2.0];
        let (zx, zy) = (zona[0] + zona[2] / 2.0, zona[1] + zona[3] / 2.0);
        Camara {
            centro: [
                medio[0] - (zx - w / 2.0) * px,
                medio[1] + (zy - h / 2.0) * px,
            ],
            px,
        }
    }
}

/// Un pedido al hilo que convierte: la version del dibujo y una copia.
type Pedido = (u64, Box<CadDocument>);
type Llegado = (u64, Result<Modelo, String>);

/// **El plano subido** y el hilo que lo convierte.
pub struct Plano {
    pedir: Sender<Pedido>,
    llega: Receiver<Llegado>,
    /// La version pedida por ultima vez.
    pub pedida: u64,
    /// La version que se ve.
    pub vista: u64,
    pub gpu: Option<(Modelo, PlanoGpu)>,
    pub error: Option<String>,
}

impl Plano {
    pub fn nuevo() -> Plano {
        let (pedir, rx) = channel::<Pedido>();
        let (tx, llega) = channel::<Llegado>();
        let _ = std::thread::Builder::new()
            .name("plano-convertir".into())
            // El convertidor recorre bloques dentro de bloques: pila holgada.
            .stack_size(64 * 1024 * 1024)
            .spawn(move || {
                while let Ok(mut p) = rx.recv() {
                    // Solo el ultimo pedido: los de en medio ya no valen.
                    while let Ok(q) = rx.try_recv() {
                        p = q;
                    }
                    let (version, doc) = p;
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        pixpin_cad::convertir::Convertidor::nuevo(&doc)
                            .convertir()
                            .0
                    }))
                    .map_err(|_| "el plano no se pudo dibujar".to_string());
                    if tx.send((version, r)).is_err() {
                        break;
                    }
                }
            });
        Plano {
            pedir,
            llega,
            pedida: 0,
            vista: 0,
            gpu: None,
            error: None,
        }
    }

    /// Pide dibujar el dibujo de la version `v` si aun no se pidio.
    pub fn pedir(&mut self, v: u64, doc: &CadDocument) {
        if v != self.pedida {
            self.pedida = v;
            let _ = self.pedir.send((v, Box::new(doc.clone())));
        }
    }

    /// Si esta convirtiendo (para seguir mirando el canal).
    pub fn esperando(&self) -> bool {
        self.pedida != self.vista
    }

    /// Sube lo que haya llegado. Dice si cambio lo que se ve.
    pub fn recibir(&mut self, gpu: &Gpu) -> bool {
        let mut cambio = false;
        while let Ok((v, r)) = self.llega.try_recv() {
            self.vista = v;
            match r {
                Ok(m) => match gpu.subir(&m) {
                    Ok(p) => {
                        self.gpu = Some((m, p));
                        self.error = None;
                    }
                    Err(e) => self.error = Some(format!("el plano no cabe en la tarjeta: {e}")),
                },
                Err(e) => self.error = Some(e),
            }
            cambio = true;
        }
        cambio
    }
}

/// La vista del sombreador para el modelo `m` con la camara `c`.
pub fn vista_de(m: &Modelo, c: &Camara, w: u32, h: u32) -> Vista {
    Vista {
        escala: [
            (2.0 / (w.max(1) as f64 * c.px)) as f32,
            (2.0 / (h.max(1) as f64 * c.px)) as f32,
        ],
        centro: [
            (c.centro[0] - m.origen[0]) as f32,
            (c.centro[1] - m.origen[1]) as f32,
        ],
        // El color 7 (blanco o negro segun el fondo): blanco sobre oscuro.
        color7: [1.0; 4],
        px: c.px as f32,
        modo: 1.0,
        relleno: [0.0; 2],
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_camara_va_y_vuelve() {
        let c = Camara {
            centro: [100.0, 50.0],
            px: 0.5,
        };
        let p = c.plano(10.0, 20.0, 800.0, 600.0);
        let (x, y) = c.pantalla(p, 800.0, 600.0);
        assert!((x - 10.0).abs() < 1e-4 && (y - 20.0).abs() < 1e-4);
        // Acercar deja quieto el punto del cursor.
        let z = c.zoom(0.5, 10.0, 20.0, 800.0, 600.0);
        let q = z.plano(10.0, 20.0, 800.0, 600.0);
        assert!((q[0] - p[0]).abs() < 1e-9 && (q[1] - p[1]).abs() < 1e-9);
    }

    #[test]
    fn encuadrar_en_una_zona_que_no_es_toda_la_ventana() {
        let caja = [0.0, 0.0, 1000.0, 500.0];
        let zona = [0.0, 100.0, 700.0, 400.0];
        let c = Camara::encuadrar(caja, zona, 1000.0, 600.0);
        let (x, y) = c.pantalla([500.0, 250.0], 1000.0, 600.0);
        assert!(
            (x - 350.0).abs() < 1e-3 && (y - 300.0).abs() < 1e-3,
            "{x} {y}"
        );
    }
}
