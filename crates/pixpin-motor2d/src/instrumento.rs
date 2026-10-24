//! **El instrumento de graficar**: unos ejes que saben cuanto vale una
//! unidad. Uno solo, con el numero de ejes como dato.
//!
//! Con **un eje** es la recta numerica, con **dos** el plano cartesiano y con
//! **tres** el espacio de tres ejes que se gira. No son tres herramientas: es
//! la misma figura (`Figura::Instrumento`), con el mismo pintado, el mismo
//! picado, la misma seleccion, el mismo estirado, el mismo deshacer y la
//! misma exportacion; lo que cambia de una a otra es el numero de ejes y la
//! proyeccion de cada uno sobre el papel. Asi lo pidio el usuario
//! (13-oct-2026): las variantes son datos, no codigo aparte.
//!
//! Puerto de `Plano.kt` y `Espacio.kt` de PixPin Android, que ya comparten
//! las cuentas (la unidad, los pasos que se doblan cuando no caben, la punta
//! de flecha, como se escriben las cifras). En el fichero se conserva su
//! formato tal cual: tres tipos (`pixpin-number-line`, `pixpin-axes`,
//! `pixpin-space`) con las claves `unidad`, `pasoDeNumeros`, `pasoDeCuadros`
//! y, en el espacio, `azimut` y `elevacion`. Aqui los tres tipos se leen al
//! mismo instrumento y se escriben de vuelta con el tipo de su numero de ejes
//! (`excalidraw.rs`).
//!
//! ## Lo que decide todo: la unidad
//!
//! Es cuantos pixeles de escena mide una unidad. La caja partida por la
//! unidad es el intervalo que se ve; los numeros caen cada `paso_numeros`
//! unidades y las rayas finas cada `paso_cuadros`. Por eso estirarlo hace dos
//! cosas: por un **lado** crece la caja y la unidad se queda (salen mas
//! numeros), y por una **esquina** crecen las dos (los mismos numeros, mas
//! grandes). Se aplica en `transformar::escalar`.
//!
//! ## El espacio se gira, no se inclina
//!
//! El tirador de giro, en el de tres ejes, cambia `azimut` y deja la caja
//! derecha: los numeros se siguen leyendo (`transformar::girar_el_espacio`).
//!
//! Es geometria pura, sin pantalla: donde cae cada raya y cada cifra es lo que
//! se equivoca en silencio, y se prueba aqui.

use serde::{Deserialize, Serialize};

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use crate::pintado::Orden;
use crate::vector::Punto2;

pub mod tabla;

/// Los datos del instrumento: cuantos ejes y como se reparten.
///
/// Los numeros van en `f64` y opcionales, como `Element.kt`: el movil escribe
/// `unidad: 40.0` y lo que aqui se lee tiene que volver con los mismos bytes,
/// y una clave que no vino no se estrena al moverlo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Instrumento {
    /// 1 recta numerica, 2 plano cartesiano, 3 espacio.
    pub ejes: u8,
    /// Pixeles de escena que mide una unidad (`unidad`).
    #[serde(default)]
    pub unidad: Option<f64>,
    /// Cada cuantas unidades se escribe un numero (`pasoDeNumeros`).
    #[serde(default)]
    pub paso_numeros: Option<f64>,
    /// Cada cuantas unidades va una raya fina (`pasoDeCuadros`).
    #[serde(default)]
    pub paso_cuadros: Option<f64>,
    /// Solo con tres ejes: la vuelta dada al espacio, en grados (`azimut`).
    #[serde(default)]
    pub azimut: Option<f64>,
    /// Solo con tres ejes: lo que se mira desde arriba, en grados (`elevacion`).
    #[serde(default)]
    pub elevacion: Option<f64>,
}

/// Lo que vale una unidad y cada cuanto se rotula si el elemento no lo dice
/// (`UNIDAD_POR_DEFECTO`, `PASO_POR_DEFECTO`).
pub const UNIDAD_POR_DEFECTO: f64 = 40.0;
pub const PASO_POR_DEFECTO: f64 = 1.0;
/// La vista de partida del espacio y su inclinacion (`AZIMUT_BASE`,
/// `ELEVACION_POR_DEFECTO`).
pub const AZIMUT_BASE: f64 = -120.0;
pub const ELEVACION_POR_DEFECTO: f64 = 25.0;
/// La rejilla: un pelo de escena y translucida, igual en las tres salidas
/// (`GROSOR_DE_LA_REJILLA`, `ALFA_DE_LA_REJILLA`).
pub const GROSOR_DE_LA_REJILLA: f32 = 0.75;
pub const ALFA_DE_LA_REJILLA: f32 = 0.22;
/// El tamano de las cifras si el elemento no trae uno (`TAM_DE_LA_CIFRA`).
pub const TAM_DE_LA_CIFRA: f32 = 11.0;
/// Lo que mide de alto la recta de fabrica (`ALTO_DE_LA_RECTA`).
pub const ALTO_DE_LA_RECTA: f32 = 56.0;

const MAXIMAS: i64 = 300;
const MINIMO_CUADRO: f64 = 6.0;
const MINIMO_ENTRE_NUMEROS: f64 = 34.0;
const PUNTA_BASE: f64 = 9.0;
const ANGULO_DE_LA_PUNTA: f64 = 25.0;
const MARGEN_DE_LETRAS: f64 = 12.0;
const MARCA_DEL_ESPACIO: f64 = 4.0;
const MINIMO_CUADRO_DEL_ESPACIO: f64 = 10.0;
const MAXIMAS_DEL_ESPACIO: i64 = 60;

impl Instrumento {
    /// El de fabrica con `ejes` ejes (`planoDeFunciones`, `rectaDeFunciones`,
    /// `espacioDeFunciones` de `Biblioteca.kt`): nace con la unidad y los
    /// pasos puestos, y el espacio con su vista del cuaderno.
    pub fn de_fabrica(ejes: u8) -> Instrumento {
        let ejes = ejes.clamp(1, 3);
        Instrumento {
            ejes,
            unidad: Some(UNIDAD_POR_DEFECTO),
            paso_numeros: Some(PASO_POR_DEFECTO),
            paso_cuadros: Some(PASO_POR_DEFECTO),
            azimut: (ejes == 3).then_some(0.0),
            elevacion: (ejes == 3).then_some(ELEVACION_POR_DEFECTO),
        }
    }

    /// El tipo del fichero que le toca (`Element.kt`).
    pub fn tipo(&self) -> &'static str {
        match self.ejes {
            1 => "pixpin-number-line",
            3 => "pixpin-space",
            _ => "pixpin-axes",
        }
    }

    /// Y al reves: cuantos ejes dice un tipo del fichero.
    pub fn ejes_del_tipo(tipo: &str) -> Option<u8> {
        match tipo {
            "pixpin-number-line" => Some(1),
            "pixpin-axes" => Some(2),
            "pixpin-space" => Some(3),
            _ => None,
        }
    }

    /// La unidad de verdad (`unidadDelPlano`): nunca menos de medio pixel.
    pub fn unidad_util(&self) -> f64 {
        self.unidad
            .filter(|u| u.is_finite())
            .unwrap_or(UNIDAD_POR_DEFECTO)
            .max(0.5)
    }

    fn paso_de_numeros(&self) -> f64 {
        self.paso_numeros
            .filter(|p| p.is_finite())
            .unwrap_or(PASO_POR_DEFECTO)
            .max(1e-6)
    }

    fn paso_de_cuadros(&self) -> f64 {
        self.paso_cuadros
            .filter(|p| p.is_finite())
            .unwrap_or(PASO_POR_DEFECTO)
            .max(1e-6)
    }

    fn azimut_util(&self) -> f64 {
        self.azimut.filter(|a| a.is_finite()).unwrap_or(0.0)
    }

    fn elevacion_util(&self) -> f64 {
        self.elevacion
            .filter(|a| a.is_finite())
            .unwrap_or(ELEVACION_POR_DEFECTO)
            .clamp(0.0, 89.0)
    }
}

/// El instrumento de `e`, si lo es.
pub fn de(e: &Elemento) -> Option<&Instrumento> {
    match &e.figura {
        Figura::Instrumento(i) => Some(i),
        _ => None,
    }
}

/// Una raya del instrumento, **relativa a `x`/`y`** del elemento. `eje`
/// separa los ejes (y sus marcas y puntas) de la rejilla.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Raya {
    pub a: (f64, f64),
    pub b: (f64, f64),
    pub eje: bool,
}

/// Una cifra (o la letra de un eje), con su ancla relativa a `x`/`y`.
/// `horizontal`: cuelga centrada bajo su marca; si no, va a la izquierda.
#[derive(Debug, Clone, PartialEq)]
pub struct Cifra {
    pub donde: (f64, f64),
    pub texto: String,
    pub horizontal: bool,
}

/// Hacia donde apunta cada eje en el papel, por unidad de escena: el de la
/// recta y el plano son fijos; el del espacio sale de su vista.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Proyeccion {
    pub ex: (f64, f64),
    pub ey: (f64, f64),
    pub ez: (f64, f64),
}

impl Proyeccion {
    fn de(&self, x: f64, y: f64, z: f64) -> (f64, f64) {
        (
            self.ex.0 * x + self.ey.0 * y + self.ez.0 * z,
            self.ex.1 * x + self.ey.1 * y + self.ez.1 * z,
        )
    }
}

/// La proyeccion del instrumento (`proyeccionDelEspacio`): primero se gira
/// alrededor de la vertical y luego se inclina la mirada. Con uno o dos ejes
/// es la del papel: la x a la derecha y la y hacia arriba.
pub fn proyeccion(i: &Instrumento) -> Proyeccion {
    if i.ejes != 3 {
        return Proyeccion {
            ex: (1.0, 0.0),
            ey: (0.0, -1.0),
            ez: (0.0, 0.0),
        };
    }
    let a = (AZIMUT_BASE + i.azimut_util()).to_radians();
    let b = i.elevacion_util().to_radians();
    let p = |x: f64, y: f64, z: f64| {
        let derecha = x * a.cos() - y * a.sin();
        let lejos = x * a.sin() + y * a.cos();
        let arriba = lejos * b.sin() + z * b.cos();
        (derecha, -arriba)
    };
    Proyeccion {
        ex: p(1.0, 0.0, 0.0),
        ey: p(0.0, 1.0, 0.0),
        ez: p(0.0, 0.0, 1.0),
    }
}

/// Lo que mide la punta de flecha (`largoDeLaPunta`).
fn largo_de_la_punta(e: &Elemento) -> f64 {
    (PUNTA_BASE + e.grosor as f64 * 1.5)
        .min((e.ancho.min(e.alto) as f64) / 5.0)
        .max(2.0)
}

/// Las dos alas de una punta en `punta` hacia `(dx, dy)` (`puntaDeFlecha`).
fn punta_de_flecha(punta: (f64, f64), dx: f64, dy: f64, largo: f64) -> [Raya; 2] {
    let n = dx.hypot(dy);
    if n == 0.0 || largo <= 0.0 {
        let nada = Raya {
            a: punta,
            b: punta,
            eje: true,
        };
        return [nada, nada];
    }
    let (ux, uy) = (dx / n, dy / n);
    let ala = largo * ANGULO_DE_LA_PUNTA.to_radians().tan();
    let base = (punta.0 - ux * largo, punta.1 - uy * largo);
    [
        Raya {
            a: (base.0 - uy * ala, base.1 + ux * ala),
            b: punta,
            eje: true,
        },
        Raya {
            a: (base.0 + uy * ala, base.1 - ux * ala),
            b: punta,
            eje: true,
        },
    ]
}

/// El paso ensanchado si el pedido no cabe (`pasoUtil`): doblandolo, las
/// rayas que quedan son un subconjunto de las que se pidieron.
pub fn paso_util(paso: f64, unidad: f64, minimo: f64) -> f64 {
    if paso <= 0.0 || unidad <= 0.0 {
        return paso;
    }
    let mut p = paso;
    let mut vueltas = 0;
    while p * unidad < minimo && vueltas < 16 {
        p *= 2.0;
        vueltas += 1;
    }
    p
}

/// Los multiplos que caben a cada lado del origen (`enteros`).
pub fn enteros(alcance: f64, paso: f64) -> std::ops::RangeInclusive<i64> {
    if paso <= 0.0 || alcance <= 0.0 || !alcance.is_finite() {
        #[allow(clippy::reversed_empty_ranges)]
        return 1..=0;
    }
    let cuantos = ((alcance / paso).floor() as i64).min(MAXIMAS);
    if cuantos <= 0 {
        return 0..=0;
    }
    -cuantos..=cuantos
}

/// Un numero para la regla: sin decimales si no hacen falta y con coma
/// (`escrito`). Dos decimales como mucho.
pub fn escrito(valor: f64) -> String {
    if (valor - valor.round()).abs() < 1e-9 {
        return format!("{}", valor.round() as i64);
    }
    let redondo = (valor * 100.0).round() / 100.0;
    let t = format!("{redondo}");
    let t = if t.contains('.') {
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        t
    };
    t.replace('.', ",")
}

/// El origen: el centro de la caja (`origenDelPlano`).
fn origen(e: &Elemento) -> (f64, f64) {
    (e.ancho as f64 / 2.0, e.alto as f64 / 2.0)
}

/// Hasta donde llega cada eje, en unidades. Con uno o dos ejes, media caja
/// (`alcanceDelPlano`); en el espacio, lo que deja caber cada eje proyectado
/// sin salirse de la caja (`alcanceDelEspacio`).
pub fn alcance(e: &Elemento, i: &Instrumento) -> (f64, f64, f64) {
    let u = i.unidad_util();
    let (w, h) = (e.ancho as f64, e.alto as f64);
    if i.ejes != 3 {
        return (w / 2.0 / u, h / 2.0 / u, 0.0);
    }
    let pr = proyeccion(i);
    let medio_ancho = (w / 2.0 - MARGEN_DE_LETRAS).max(1.0);
    let medio_alto = (h / 2.0 - MARGEN_DE_LETRAS).max(1.0);
    let cabe = |v: (f64, f64)| {
        let mut r = f64::MAX;
        if v.0.abs() > 1e-6 {
            r = r.min(medio_ancho / v.0.abs());
        }
        if v.1.abs() > 1e-6 {
            r = r.min(medio_alto / v.1.abs());
        }
        if r == f64::MAX { 0.0 } else { r / u }
    };
    (cabe(pr.ex), cabe(pr.ey), cabe(pr.ez))
}

/// **Todas las rayas del instrumento**, en orden de pintado: la rejilla
/// primero y los ejes encima (`trazosDelInstrumento`).
pub fn rayas(e: &Elemento, i: &Instrumento) -> Vec<Raya> {
    if e.ancho <= 0.0 || e.alto <= 0.0 {
        return Vec::new();
    }
    let u = i.unidad_util();
    let o = origen(e);
    let (w, h) = (e.ancho as f64, e.alto as f64);
    let punta = largo_de_la_punta(e);
    let mut out = Vec::new();
    match i.ejes {
        1 => {
            let (alc, _, _) = alcance(e, i);
            let paso = paso_util(i.paso_de_cuadros(), u, MINIMO_CUADRO);
            let marca = 6.0f64.min(h / 4.0);
            for n in enteros(alc, paso) {
                let x = o.0 + n as f64 * paso * u;
                out.push(Raya {
                    a: (x, o.1 - marca),
                    b: (x, o.1 + marca),
                    eje: true,
                });
            }
            out.push(Raya {
                a: (0.0, o.1),
                b: (w, o.1),
                eje: true,
            });
            out.extend(punta_de_flecha((w, o.1), 1.0, 0.0, punta));
            out.extend(punta_de_flecha((0.0, o.1), -1.0, 0.0, punta));
        }
        2 => {
            let (ax, ay, _) = alcance(e, i);
            let paso = paso_util(i.paso_de_cuadros(), u, MINIMO_CUADRO);
            for n in enteros(ax, paso) {
                let x = o.0 + n as f64 * paso * u;
                out.push(Raya {
                    a: (x, 0.0),
                    b: (x, h),
                    eje: false,
                });
            }
            for n in enteros(ay, paso) {
                let y = o.1 + n as f64 * paso * u;
                out.push(Raya {
                    a: (0.0, y),
                    b: (w, y),
                    eje: false,
                });
            }
            out.push(Raya {
                a: (0.0, o.1),
                b: (w, o.1),
                eje: true,
            });
            out.push(Raya {
                a: (o.0, 0.0),
                b: (o.0, h),
                eje: true,
            });
            out.extend(punta_de_flecha((w, o.1), 1.0, 0.0, punta));
            out.extend(punta_de_flecha((0.0, o.1), -1.0, 0.0, punta));
            out.extend(punta_de_flecha((o.0, 0.0), 0.0, -1.0, punta));
            out.extend(punta_de_flecha((o.0, h), 0.0, 1.0, punta));
        }
        _ => rayas_del_espacio(e, i, &mut out),
    }
    out
}

fn rayas_del_espacio(e: &Elemento, i: &Instrumento, out: &mut Vec<Raya>) {
    let u = i.unidad_util();
    let o = origen(e);
    let pr = proyeccion(i);
    let (ax, ay, az) = alcance(e, i);
    let en = |x: f64, y: f64, z: f64| {
        let p = pr.de(x * u, y * u, z * u);
        (o.0 + p.0, o.1 + p.1)
    };
    // El suelo pautado, encogido lo justo para que sus esquinas quepan.
    let paso = paso_util(
        i.paso_de_cuadros(),
        u * pr.ex.0.hypot(pr.ex.1).max(0.2),
        MINIMO_CUADRO_DEL_ESPACIO,
    );
    let medio_ancho = (e.ancho as f64 / 2.0 - MARGEN_DE_LETRAS).max(1.0);
    let medio_alto = (e.alto as f64 / 2.0 - MARGEN_DE_LETRAS).max(1.0);
    let ancho_suelo = (pr.ex.0.abs() * ax + pr.ey.0.abs() * ay) * u;
    let alto_suelo = (pr.ex.1.abs() * ax + pr.ey.1.abs() * ay) * u;
    let encoge = 1.0f64
        .min(if ancho_suelo > 1e-9 {
            medio_ancho / ancho_suelo
        } else {
            1.0
        })
        .min(if alto_suelo > 1e-9 {
            medio_alto / alto_suelo
        } else {
            1.0
        });
    let nx = ((ax * encoge / paso).floor() as i64).clamp(0, MAXIMAS_DEL_ESPACIO);
    let ny = ((ay * encoge / paso).floor() as i64).clamp(0, MAXIMAS_DEL_ESPACIO);
    if nx > 0 && ny > 0 {
        let (lx, ly) = (nx as f64 * paso, ny as f64 * paso);
        for k in -nx..=nx {
            let x = k as f64 * paso;
            out.push(Raya {
                a: en(x, -ly, 0.0),
                b: en(x, ly, 0.0),
                eje: false,
            });
        }
        for k in -ny..=ny {
            let y = k as f64 * paso;
            out.push(Raya {
                a: en(-lx, y, 0.0),
                b: en(lx, y, 0.0),
                eje: false,
            });
        }
    }
    // Los ejes, de punta a punta.
    out.push(Raya {
        a: en(-ax, 0.0, 0.0),
        b: en(ax, 0.0, 0.0),
        eje: true,
    });
    out.push(Raya {
        a: en(0.0, -ay, 0.0),
        b: en(0.0, ay, 0.0),
        eje: true,
    });
    out.push(Raya {
        a: en(0.0, 0.0, -az),
        b: en(0.0, 0.0, az),
        eje: true,
    });
    // Las marcas, cruzadas segun otro eje: las de la x segun la y, las de la
    // y y la z segun la x.
    let mut marcas = |alc: f64, sitio: &dyn Fn(f64) -> (f64, f64), cruce: (f64, f64)| {
        let largo = cruce.0.hypot(cruce.1);
        if largo < 1e-6 {
            return;
        }
        let (cx, cy) = (
            cruce.0 / largo * MARCA_DEL_ESPACIO,
            cruce.1 / largo * MARCA_DEL_ESPACIO,
        );
        for n in enteros(alc, paso) {
            if n == 0 {
                continue;
            }
            let c = sitio(n as f64 * paso);
            out.push(Raya {
                a: (c.0 - cx, c.1 - cy),
                b: (c.0 + cx, c.1 + cy),
                eje: true,
            });
        }
    };
    marcas(ax, &|t| en(t, 0.0, 0.0), pr.ey);
    marcas(ay, &|t| en(0.0, t, 0.0), pr.ex);
    marcas(az, &|t| en(0.0, 0.0, t), pr.ex);
    let punta = largo_de_la_punta(e);
    out.extend(punta_de_flecha(en(ax, 0.0, 0.0), pr.ex.0, pr.ex.1, punta));
    out.extend(punta_de_flecha(en(0.0, ay, 0.0), pr.ey.0, pr.ey.1, punta));
    out.extend(punta_de_flecha(en(0.0, 0.0, az), pr.ez.0, pr.ez.1, punta));
}

/// El tamano de las cifras de `e`: el de su `fontSize` o el de fabrica.
pub fn tam_de_las_cifras(e: &Elemento) -> f32 {
    e.extras
        .tam_letra
        .filter(|t| t.is_finite())
        .unwrap_or(TAM_DE_LA_CIFRA)
        .max(1.0)
}

/// **Las cifras del instrumento** (`numerosDelInstrumento`). El cero va una
/// sola vez; en el espacio, ademas, las letras x, y, z pasada cada punta.
pub fn cifras(e: &Elemento, i: &Instrumento) -> Vec<Cifra> {
    if e.ancho <= 0.0 || e.alto <= 0.0 {
        return Vec::new();
    }
    let u = i.unidad_util();
    let o = origen(e);
    let mut out = Vec::new();
    match i.ejes {
        1 => {
            let (alc, _, _) = alcance(e, i);
            let paso = paso_util(i.paso_de_numeros(), u, MINIMO_ENTRE_NUMEROS);
            for n in enteros(alc, paso) {
                out.push(Cifra {
                    donde: (o.0 + n as f64 * paso * u, o.1),
                    texto: escrito(n as f64 * paso),
                    horizontal: true,
                });
            }
        }
        2 => {
            let (ax, ay, _) = alcance(e, i);
            let paso = paso_util(i.paso_de_numeros(), u, MINIMO_ENTRE_NUMEROS);
            for n in enteros(ax, paso) {
                if n == 0 {
                    continue;
                }
                let v = n as f64 * paso;
                out.push(Cifra {
                    donde: (o.0 + v * u, o.1),
                    texto: escrito(v),
                    horizontal: true,
                });
            }
            for n in enteros(ay, paso) {
                if n == 0 {
                    continue;
                }
                let v = n as f64 * paso;
                // La y del papel crece hacia abajo: debajo del origen va el negativo.
                out.push(Cifra {
                    donde: (o.0, o.1 + v * u),
                    texto: escrito(-v),
                    horizontal: false,
                });
            }
            out.push(Cifra {
                donde: o,
                texto: "0".into(),
                horizontal: true,
            });
        }
        _ => {
            let pr = proyeccion(i);
            let (ax, ay, az) = alcance(e, i);
            let en = |x: f64, y: f64, z: f64| {
                let p = pr.de(x * u, y * u, z * u);
                (o.0 + p.0, o.1 + p.1)
            };
            let mut de_un_eje = |alc: f64, v: (f64, f64), sitio: &dyn Fn(f64) -> (f64, f64), h| {
                let paso = paso_util(
                    i.paso_de_numeros(),
                    u * v.0.hypot(v.1).max(0.2),
                    MINIMO_ENTRE_NUMEROS,
                );
                for n in enteros(alc, paso) {
                    if n == 0 {
                        continue;
                    }
                    out.push(Cifra {
                        donde: sitio(n as f64 * paso),
                        texto: escrito(n as f64 * paso),
                        horizontal: h,
                    });
                }
            };
            de_un_eje(ax, pr.ex, &|t| en(t, 0.0, 0.0), true);
            de_un_eje(ay, pr.ey, &|t| en(0.0, t, 0.0), true);
            de_un_eje(az, pr.ez, &|t| en(0.0, 0.0, t), false);
            out.push(Cifra {
                donde: o,
                texto: "0".into(),
                horizontal: true,
            });
            let tam = tam_de_las_cifras(e) as f64;
            let punta = largo_de_la_punta(e);
            let mut letra = |texto: &str, alc: f64, v: (f64, f64), tope: (f64, f64)| {
                let largo = v.0.hypot(v.1);
                if largo < 1e-6 || alc <= 0.0 {
                    return;
                }
                let dx = v.0 / largo * (punta + tam * 0.6);
                let dy = v.1 / largo * (punta + tam * 0.6);
                out.push(Cifra {
                    donde: (tope.0 + dx, tope.1 + dy - tam * 0.75),
                    texto: texto.into(),
                    horizontal: true,
                });
            };
            letra("x", ax, pr.ex, en(ax, 0.0, 0.0));
            letra("y", ay, pr.ey, en(0.0, ay, 0.0));
            letra("z", az, pr.ez, en(0.0, 0.0, az));
        }
    }
    out
}

/// **Donde cae el punto `(x, y, z)` del instrumento en la escena**
/// (`puntoDelPlano`, `puntoDelEspacio`): lo que lo hace instrumento. Con un
/// eje solo cuenta `x`; con dos, `x` e `y`.
pub fn punto(e: &Elemento, i: &Instrumento, x: f64, y: f64, z: f64) -> Punto2 {
    let u = i.unidad_util();
    let o = origen(e);
    let (y, z) = match i.ejes {
        1 => (0.0, 0.0),
        2 => (y, 0.0),
        _ => (y, z),
    };
    let p = proyeccion(i).de(x * u, y * u, z * u);
    Punto2::nuevo(
        (e.x as f64 + o.0 + p.0) as f32,
        (e.y as f64 + o.1 + p.1) as f32,
    )
}

/// **Los ejes como segmentos de escena**: a lo que se pega el iman y por
/// donde pasan sus cruces (`Perimetros.kt`: «los instrumentos dan sus ejes,
/// no su caja»).
pub fn ejes_en_escena(e: &Elemento, i: &Instrumento) -> Vec<[Punto2; 2]> {
    rayas(e, i)
        .into_iter()
        .filter(|r| r.eje)
        .map(|r| {
            [
                Punto2::nuevo(e.x + r.a.0 as f32, e.y + r.a.1 as f32),
                Punto2::nuevo(e.x + r.b.0 as f32, e.y + r.b.1 as f32),
            ]
        })
        .collect()
}

/// Donde cae la linea base de un renglon de `familia`, en veces su tamano
/// desde arriba de la caja del texto: es lo que hace falta para colocar una
/// cifra por su base, como `drawText` del movil.
fn linea_base(familia: &str) -> f32 {
    // Segoe UI (la de los rotulos de fabrica) no esta en el catalogo: su base
    // cae a un tamano de letra de arriba, como la deja DirectWrite.
    crate::texto::fuente_por_nombre(familia).map_or(1.0, |f| f.linea_base())
}

/// **Lo que se pinta del instrumento**, como ordenes: un solo pintado para
/// los tres (`drawPlano` del movil). La rejilla, translucida y fina; los
/// ejes con el grosor del pincel; las cifras con su letra.
pub fn ordenes(e: &Elemento, i: &Instrumento, color: ColorRgba, familia: String) -> Vec<Orden> {
    let mut v = Vec::new();
    let grosor = e.grosor.max(0.5);
    let rejilla = ColorRgba {
        a: color.a * ALFA_DE_LA_REJILLA,
        ..color
    };
    let (x, y) = (e.x as f64, e.y as f64);
    for r in rayas(e, i) {
        if r.a == r.b {
            continue;
        }
        v.push(Orden::Polilinea {
            puntos: vec![
                Punto2::nuevo((x + r.a.0) as f32, (y + r.a.1) as f32),
                Punto2::nuevo((x + r.b.0) as f32, (y + r.b.1) as f32),
            ],
            color: if r.eje { color } else { rejilla },
            grosor: if r.eje { grosor } else { GROSOR_DE_LA_REJILLA },
            estilo: EstiloTrazo::Solido,
        });
    }
    let tam = tam_de_las_cifras(e);
    let base = linea_base(&familia) * tam;
    let estilo = crate::texto::EstiloDeTexto::default();
    for c in cifras(e, i) {
        let ancho = crate::texto::ancho_real(&c.texto, tam, &familia, estilo);
        let (cx, cy) = ((x + c.donde.0) as f32, (y + c.donde.1) as f32);
        // Colgando bajo su marca y centrada, o a la izquierda del eje.
        let (izq, linea) = if c.horizontal {
            (cx - ancho / 2.0, cy + tam * 1.15)
        } else {
            (cx - tam * 0.3 - ancho, cy + tam * 0.35)
        };
        v.push(Orden::Texto {
            texto: c.texto,
            x: izq,
            y: linea - base,
            tam,
            familia: familia.clone(),
            color,
            ancho_max: crate::texto::SIN_PARTIR,
            negrita: false,
            cursiva: false,
        });
    }
    v
}

/// Un instrumento nuevo de `ejes` ejes, como lo pone el panel de figuras:
/// la caja de fabrica (ocho unidades de lado; la recta, de alto lo justo
/// para sus marcas y cifras), a tiralineas y con el pincel de ahora.
pub fn elemento_de_fabrica(ejes: u8, trazo: ColorRgba, grosor: f32, opacidad: f32) -> Elemento {
    let i = Instrumento::de_fabrica(ejes);
    let lado = (UNIDAD_POR_DEFECTO * 8.0) as f32;
    Elemento {
        figura: Figura::Instrumento(i),
        ancho: lado,
        alto: if i.ejes == 1 { ALTO_DE_LA_RECTA } else { lado },
        trazo,
        grosor,
        opacidad,
        rugosidad: 0.0,
        ..Default::default()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn con(ejes: u8, ancho: f32, alto: f32) -> (Elemento, Instrumento) {
        let mut e = elemento_de_fabrica(ejes, ColorRgba::opaco(0.0, 0.0, 0.0), 2.0, 1.0);
        e.ancho = ancho;
        e.alto = alto;
        let i = *de(&e).unwrap();
        (e, i)
    }

    #[test]
    fn el_origen_esta_en_el_centro_y_el_punto_3_menos_2_cae_donde_toca() {
        // `PlanoCartesianoTest`: con 40 px la unidad, (3, -2) cae a 120 a la
        // derecha y 80 por debajo del centro.
        let (e, i) = con(2, 320.0, 320.0);
        let p = punto(&e, &i, 3.0, -2.0, 0.0);
        assert_eq!((p.x, p.y), (160.0 + 120.0, 160.0 + 80.0));
    }

    #[test]
    fn el_cero_se_escribe_una_sola_vez_en_el_plano() {
        let (e, i) = con(2, 320.0, 320.0);
        let ceros = cifras(&e, &i).iter().filter(|c| c.texto == "0").count();
        assert_eq!(ceros, 1);
    }

    #[test]
    fn la_recta_pone_cifras_de_menos_cuatro_a_cuatro() {
        let (e, i) = con(1, 320.0, 56.0);
        let t: Vec<String> = cifras(&e, &i).into_iter().map(|c| c.texto).collect();
        assert_eq!(t, ["-4", "-3", "-2", "-1", "0", "1", "2", "3", "4"]);
    }

    #[test]
    fn estirar_la_caja_sin_tocar_la_unidad_ensena_mas_numeros() {
        let (e, i) = con(1, 320.0, 56.0);
        let (mut f, _) = con(1, 640.0, 56.0);
        f.figura = e.figura.clone();
        assert!(cifras(&f, &i).len() > cifras(&e, &i).len());
    }

    #[test]
    fn el_paso_que_no_cabe_se_dobla_y_sigue_siendo_de_los_pedidos() {
        assert_eq!(paso_util(1.0, 10.0, 34.0), 4.0);
        assert_eq!(paso_util(1.0, 40.0, 34.0), 1.0);
        // Caso negativo: un paso imposible no entra en un bucle sin fin.
        assert_eq!(paso_util(0.0, 40.0, 34.0), 0.0);
    }

    #[test]
    fn las_cifras_se_escriben_con_coma_y_sin_decimales_de_mas() {
        assert_eq!(escrito(2.0), "2");
        assert_eq!(escrito(-0.5), "-0,5");
        assert_eq!(escrito(1.25), "1,25");
        assert_eq!(escrito(0.333333), "0,33");
    }

    #[test]
    fn una_caja_sin_tamano_no_pinta_nada() {
        let (e, i) = con(3, 0.0, 300.0);
        assert!(rayas(&e, &i).is_empty());
        assert!(cifras(&e, &i).is_empty());
    }

    #[test]
    fn el_espacio_cabe_en_su_caja_y_girarlo_cambia_donde_cae_la_x() {
        let (e, i) = con(3, 320.0, 320.0);
        for r in rayas(&e, &i) {
            for (x, y) in [r.a, r.b] {
                assert!(
                    (-1.0..=321.0).contains(&x) && (-1.0..=321.0).contains(&y),
                    "se sale: ({x}, {y})"
                );
            }
        }
        let antes = punto(&e, &i, 1.0, 0.0, 0.0);
        let girado = Instrumento {
            azimut: Some(90.0),
            ..i
        };
        let despues = punto(&e, &girado, 1.0, 0.0, 0.0);
        assert!((antes.x - despues.x).abs() > 1.0 || (antes.y - despues.y).abs() > 1.0);
        // La z sube siempre.
        assert!(punto(&e, &i, 0.0, 0.0, 1.0).y < 160.0);
    }

    #[test]
    fn el_espacio_pone_sus_tres_letras() {
        let (e, i) = con(3, 320.0, 320.0);
        let c = cifras(&e, &i);
        for l in ["x", "y", "z"] {
            assert!(c.iter().any(|c| c.texto == l), "falta la {l}");
        }
    }

    #[test]
    fn el_mismo_pintado_vale_para_los_tres_y_la_rejilla_es_translucida() {
        for ejes in 1..=3 {
            let (e, i) = con(ejes, 320.0, if ejes == 1 { 56.0 } else { 320.0 });
            let o = ordenes(&e, &i, e.trazo, "Segoe UI".into());
            assert!(o.iter().any(|o| matches!(o, Orden::Polilinea { .. })));
            assert!(o.iter().any(|o| matches!(o, Orden::Texto { .. })));
            if ejes > 1 {
                assert!(
                    o.iter().any(|o| matches!(o,
                    Orden::Polilinea { color, .. } if (color.a - ALFA_DE_LA_REJILLA).abs() < 1e-6))
                );
            }
        }
    }
}
