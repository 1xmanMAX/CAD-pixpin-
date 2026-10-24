//! Porte fiel de perfect-freehand 1.2.0 (MIT, Copyright (c) 2021 Stephen
//! Ruiz Ltd): `getStrokePoints` y `getStrokeOutlinePoints`.
//!
//! Origen: npm `perfect-freehand@1.2.0`, la version que fija Excalidraw
//! `afa3a65` en `packages/excalidraw/package.json`. No la del repositorio
//! de perfect-freehand, que va por delante.
//!
//! # Por que f64 y bucles que suman el paso
//!
//! La referencia es JavaScript: todo es f64 y los arcos se trazan con
//! `for (t = 0; t <= 1; t += 1/13)`. Sumar 1/13 trece veces no da 1 exacto,
//! y de eso depende si sale la ultima vuelta del bucle. Un porte "limpio"
//! con `0..=13` daria un punto de mas o de menos en algunas tapas y el
//! oraculo lo cazaria. Asi que se copia el bucle tal cual, en f64.
//!
//! # Que NO se porta
//!
//! El afilado (`taper`) y las tapas planas: Excalidraw no los usa. Con
//! `taper = 0` y `cap = true` sus ramas son codigo muerto.

pub type V = [f64; 2];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entrada {
    pub x: f64,
    pub y: f64,
    /// `None` equivale al `undefined` de JavaScript: se usa la presion por
    /// defecto (0,25 el primer punto, 0,5 el resto).
    pub presion: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Opciones {
    pub size: f64,
    pub thinning: f64,
    pub smoothing: f64,
    pub streamline: f64,
    pub simular_presion: bool,
    pub last: bool,
    pub easing: fn(f64) -> f64,
}

/// easeOutSine, el easing que pasa Excalidraw.
pub fn seno(t: f64) -> f64 {
    (t * std::f64::consts::PI / 2.0).sin()
}

const RATE_OF_PRESSURE_CHANGE: f64 = 0.275;
const FIXED_PI: f64 = std::f64::consts::PI + 0.0001;
const START_CAP_SEGMENTS: f64 = 13.0;
const END_CAP_SEGMENTS: f64 = 29.0;
const CORNER_CAP_SEGMENTS: f64 = 13.0;
const END_NOISE_THRESHOLD: f64 = 3.0;
const MIN_STREAMLINE_T: f64 = 0.15;
const STREAMLINE_T_RANGE: f64 = 0.85;
const MIN_RADIUS: f64 = 0.01;
const DEFAULT_FIRST_PRESSURE: f64 = 0.25;
const DEFAULT_PRESSURE: f64 = 0.5;

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1]]
}
fn mul(a: V, n: f64) -> V {
    [a[0] * n, a[1] * n]
}
fn neg(a: V) -> V {
    [-a[0], -a[1]]
}
fn per(a: V) -> V {
    [a[1], -a[0]]
}
fn dpr(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn uni(a: V) -> V {
    let l = a[0].hypot(a[1]);
    [a[0] / l, a[1] / l]
}
fn dist(a: V, b: V) -> f64 {
    (a[1] - b[1]).hypot(a[0] - b[0])
}
fn dist2(a: V, b: V) -> f64 {
    let d = sub(a, b);
    d[0] * d[0] + d[1] * d[1]
}
fn lrp(a: V, b: V, t: f64) -> V {
    add(a, mul(sub(b, a), t))
}
fn prj(a: V, b: V, c: f64) -> V {
    add(a, mul(b, c))
}
fn rot_around(a: V, c: V, r: f64) -> V {
    let (s, co) = (r.sin(), r.cos());
    let (px, py) = (a[0] - c[0], a[1] - c[1]);
    [px * co - py * s + c[0], px * s + py * co + c[1]]
}

#[derive(Debug, Clone, Copy)]
struct PuntoTrazo {
    point: V,
    pressure: f64,
    vector: V,
    distance: f64,
    running_length: f64,
}

/// `presion >= 0` en JavaScript: `undefined` y `NaN` no valen.
fn valida(p: Option<f64>) -> Option<f64> {
    p.filter(|v| *v >= 0.0)
}

fn puntos_del_trazo(entrada: &[Entrada], o: &Opciones) -> Vec<PuntoTrazo> {
    if entrada.is_empty() {
        return Vec::new();
    }
    let t = MIN_STREAMLINE_T + (1.0 - o.streamline) * STREAMLINE_T_RANGE;
    let mut pts: Vec<Entrada> = entrada.to_vec();

    // Dos puntos se convierten en cinco. `lrp` del original devuelve solo
    // x,y, asi que los interpolados PIERDEN la presion: se copia eso.
    if pts.len() == 2 {
        let ultimo = pts[1];
        pts.truncate(1);
        for i in 1..5 {
            let q = lrp([pts[0].x, pts[0].y], [ultimo.x, ultimo.y], i as f64 / 4.0);
            pts.push(Entrada {
                x: q[0],
                y: q[1],
                presion: None,
            });
        }
    }
    // Uno se duplica desplazado (1,1), este SI conserva la presion.
    if pts.len() == 1 {
        let p = pts[0];
        pts.push(Entrada {
            x: p.x + 1.0,
            y: p.y + 1.0,
            presion: p.presion,
        });
    }

    let mut salida = vec![PuntoTrazo {
        point: [pts[0].x, pts[0].y],
        pressure: valida(pts[0].presion).unwrap_or(DEFAULT_FIRST_PRESSURE),
        vector: [1.0, 1.0],
        distance: 0.0,
        running_length: 0.0,
    }];
    let mut alcanzado = false;
    let mut recorrido = 0.0;
    let mut prev = salida[0];
    let max = pts.len() - 1;

    for (i, p) in pts.iter().enumerate().skip(1) {
        let point = if o.last && i == max {
            [p.x, p.y]
        } else {
            lrp(prev.point, [p.x, p.y], t)
        };
        if prev.point == point {
            continue;
        }
        let distance = dist(point, prev.point);
        // El recorrido suma aunque el punto se descarte abajo: asi lo hace
        // el original, y cambia cuando se alcanza el largo minimo.
        recorrido += distance;
        if i < max && !alcanzado {
            if recorrido < o.size {
                continue;
            }
            alcanzado = true;
        }
        prev = PuntoTrazo {
            point,
            pressure: valida(p.presion).unwrap_or(DEFAULT_PRESSURE),
            vector: uni(sub(prev.point, point)),
            distance,
            running_length: recorrido,
        };
        salida.push(prev);
    }
    salida[0].vector = salida.get(1).map_or([0.0, 0.0], |s| s.vector);
    salida
}

fn simular_presion(anterior: f64, distancia: f64, size: f64) -> f64 {
    let sp = (distancia / size).min(1.0);
    let rp = (1.0 - sp).min(1.0);
    (anterior + (rp - anterior) * (sp * RATE_OF_PRESSURE_CHANGE)).min(1.0)
}

fn radio(size: f64, thinning: f64, presion: f64, easing: fn(f64) -> f64) -> f64 {
    size * easing(0.5 - thinning * (0.5 - presion))
}

fn contorno_de_puntos(puntos: &[PuntoTrazo], o: &Opciones) -> Vec<V> {
    if puntos.is_empty() || o.size <= 0.0 {
        return Vec::new();
    }
    let n = puntos.len();
    let total = puntos[n - 1].running_length;
    let min_distance = (o.size * o.smoothing).powi(2);
    let mut izquierda: Vec<V> = Vec::new();
    let mut derecha: Vec<V> = Vec::new();

    let mut prev_pressure = puntos.iter().take(10).fold(puntos[0].pressure, |acc, p| {
        let presion = if o.simular_presion {
            simular_presion(acc, p.distance, o.size)
        } else {
            p.pressure
        };
        (acc + presion) / 2.0
    });
    let mut radius = radio(o.size, o.thinning, puntos[n - 1].pressure, o.easing);
    let mut first_radius: Option<f64> = None;
    let mut prev_vector = puntos[0].vector;
    let mut prev_left = puntos[0].point;
    let mut prev_right = prev_left;
    let mut temp_left = prev_left;
    let mut temp_right = prev_right;
    let mut is_prev_sharp = false;

    for i in 0..n {
        let PuntoTrazo {
            point,
            vector,
            distance,
            running_length,
            mut pressure,
        } = puntos[i];
        let is_last = i == n - 1;
        if !is_last && total - running_length < END_NOISE_THRESHOLD {
            continue;
        }
        if o.thinning != 0.0 {
            if o.simular_presion {
                pressure = simular_presion(prev_pressure, distance, o.size);
            }
            radius = radio(o.size, o.thinning, pressure, o.easing);
        } else {
            radius = o.size / 2.0;
        }
        if first_radius.is_none() {
            first_radius = Some(radius);
        }
        // Sin afilado las dos fuerzas valen 1: queda solo el minimo.
        radius = radius.max(MIN_RADIUS);

        let next_vector = if is_last {
            vector
        } else {
            puntos[i + 1].vector
        };
        let next_dpr = if is_last {
            1.0
        } else {
            dpr(vector, next_vector)
        };
        let prev_dpr = dpr(vector, prev_vector);
        let is_point_sharp = prev_dpr < 0.0 && !is_prev_sharp;
        let is_next_sharp = next_dpr < 0.0;

        if is_point_sharp || is_next_sharp {
            let offset = mul(per(prev_vector), radius);
            let step = 1.0 / CORNER_CAP_SEGMENTS;
            let mut t = 0.0;
            while t <= 1.0 {
                temp_left = rot_around(sub(point, offset), point, FIXED_PI * t);
                izquierda.push(temp_left);
                temp_right = rot_around(add(point, offset), point, FIXED_PI * -t);
                derecha.push(temp_right);
                t += step;
            }
            prev_left = temp_left;
            prev_right = temp_right;
            if is_next_sharp {
                is_prev_sharp = true;
            }
            continue;
        }
        is_prev_sharp = false;

        if is_last {
            let offset = mul(per(vector), radius);
            izquierda.push(sub(point, offset));
            derecha.push(add(point, offset));
            continue;
        }

        let offset = mul(per(lrp(next_vector, vector, next_dpr)), radius);
        temp_left = sub(point, offset);
        if i <= 1 || dist2(prev_left, temp_left) > min_distance {
            izquierda.push(temp_left);
            prev_left = temp_left;
        }
        temp_right = add(point, offset);
        if i <= 1 || dist2(prev_right, temp_right) > min_distance {
            derecha.push(temp_right);
            prev_right = temp_right;
        }
        prev_pressure = pressure;
        prev_vector = vector;
    }

    let first_point = puntos[0].point;
    let last_point = if n > 1 {
        puntos[n - 1].point
    } else {
        add(puntos[0].point, [1.0, 1.0])
    };

    // Un solo punto: un circulo. `(firstRadius || radius)` en JS: un cero
    // cuenta como ausente.
    if n == 1 {
        let r = match first_radius {
            Some(r) if r != 0.0 => r,
            _ => radius,
        };
        let start = prj(first_point, uni(per(sub(first_point, last_point))), -r);
        let step = 1.0 / START_CAP_SEGMENTS;
        let mut punto = Vec::new();
        let mut t = step;
        while t <= 1.0 {
            punto.push(rot_around(start, first_point, FIXED_PI * 2.0 * t));
            t += step;
        }
        return punto;
    }

    let mut tapa_inicio = Vec::new();
    let step = 1.0 / START_CAP_SEGMENTS;
    let mut t = step;
    while t <= 1.0 {
        tapa_inicio.push(rot_around(derecha[0], first_point, FIXED_PI * t));
        t += step;
    }

    let direction = per(neg(puntos[n - 1].vector));
    let start = prj(last_point, direction, radius);
    let mut tapa_fin = Vec::new();
    let step = 1.0 / END_CAP_SEGMENTS;
    let mut t = step;
    while t < 1.0 {
        tapa_fin.push(rot_around(start, last_point, FIXED_PI * 3.0 * t));
        t += step;
    }

    izquierda.extend(tapa_fin);
    izquierda.extend(derecha.into_iter().rev());
    izquierda.extend(tapa_inicio);
    izquierda
}

/// Cuantas veces se repasa lo rapido. Las del movil.
const PASADAS_DE_ASIENTO: usize = 3;
/// Por debajo de este paso medio entre muestras no se toca nada: es escritura.
const PASO_LENTO: f64 = 5.0;
/// Desde este paso se asienta del todo (la mitad hacia el medio de los vecinos).
const PASO_RAPIDO: f64 = 16.0;

/// **Una raya tirada deprisa sale derecha** (F4, `asentarLoRapido` de
/// `Freehand.kt`, v0.76 del movil).
///
/// No es de perfect-freehand: es un paso previo del PixPin. Una linea rapida
/// salia «medio ondulada, levemente, pero se nota». No es el pulso —un gesto
/// rapido es recto por naturaleza—: es el **digitalizador**, que entrega cada
/// muestra con un error de un pixel o dos, y a esa velocidad las muestras caen
/// tan separadas que el `streamline` ya no las junta y el error se queda
/// dibujado como una onda.
///
/// Se asienta **solo lo rapido**: cada punto se acerca al medio de sus dos
/// vecinos tanto mas cuanto mas separados estan. Escribiendo —muestras a dos o
/// tres unidades— no se toca nada y la letra no se redondea; en una raya
/// lanzada —a quince o treinta— el error desaparece. Las puntas no se mueven
/// ni cambia el numero de puntos, para que las presiones sigan casando una a
/// una.
///
/// En el sitio y sin asignar: cada pasada guarda el valor viejo del vecino
/// anterior en una variable, que es lo unico que la version con lista nueva
/// necesitaba de la copia.
pub fn asentar_lo_rapido(puntos: &mut [V]) {
    asentar_con(puntos, |p| *p, |p, v| *p = v);
}

/// [`asentar_lo_rapido`] sobre la entrada del trazo, sin tocar las presiones.
pub fn asentar_entrada(entrada: &mut [Entrada]) {
    asentar_con(
        entrada,
        |e| [e.x, e.y],
        |e, v| {
            e.x = v[0];
            e.y = v[1];
        },
    );
}

fn asentar_con<T>(puntos: &mut [T], leer: impl Fn(&T) -> V, poner: impl Fn(&mut T, V)) {
    let n = puntos.len();
    if n < 4 {
        return;
    }
    // Se pinta en cada fotograma del trazo entero (puerta de 5.000 puntos en
    // dos milisegundos), asi que una raiz por punto y pasada y no dos: el
    // tramo q-p de un punto es el p-r del anterior, ambos con valores viejos.
    // Y la escritura, que es casi todo lo que se pinta, ni llega a la raiz:
    // con los dos tramos por debajo de PASO_LENTO la media tambien lo esta.
    let cuadrado = |a: V, b: V| (a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]);
    const LENTO2: f64 = PASO_LENTO * PASO_LENTO;
    for _ in 0..PASADAS_DE_ASIENTO {
        let mut q = leer(&puntos[0]);
        let mut izquierda = cuadrado(leer(&puntos[1]), q);
        for i in 1..n - 1 {
            let p = leer(&puntos[i]);
            let r = leer(&puntos[i + 1]);
            let derecha = cuadrado(r, p);
            let lento = izquierda < LENTO2 && derecha < LENTO2;
            let paso = if lento {
                0.0
            } else {
                (izquierda.sqrt() + derecha.sqrt()) / 2.0
            };
            izquierda = derecha;
            let cuanto = ((paso - PASO_LENTO) / (PASO_RAPIDO - PASO_LENTO)).clamp(0.0, 1.0) * 0.5;
            if cuanto > 0.0 {
                let medio = [(q[0] + r[0]) / 2.0, (q[1] + r[1]) / 2.0];
                poner(
                    &mut puntos[i],
                    [
                        p[0] + (medio[0] - p[0]) * cuanto,
                        p[1] + (medio[1] - p[1]) * cuanto,
                    ],
                );
            }
            // El vecino de la izquierda del siguiente es el valor de ANTES de
            // esta pasada, como en el movil, que escribia en una lista nueva.
            q = p;
        }
    }
}

/// `getStroke`: puntos de entrada a contorno cerrado, listo para rellenar.
pub fn contorno(entrada: &[Entrada], o: &Opciones) -> Vec<V> {
    contorno_de_puntos(&puntos_del_trazo(entrada, o), o)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn opciones() -> Opciones {
        Opciones {
            size: 4.25,
            thinning: 0.6,
            smoothing: 0.5,
            streamline: 0.5,
            simular_presion: true,
            last: true,
            easing: seno,
        }
    }

    fn e(x: f64, y: f64) -> Entrada {
        Entrada {
            x,
            y,
            presion: None,
        }
    }

    #[test]
    fn lo_rapido_se_asienta_y_lo_lento_no_se_toca() {
        // Una recta tirada deprisa, con el error del digitalizador: +-1,5
        // alternando. La prueba del movil, tal cual.
        let original: Vec<V> = (0..30)
            .map(|i| [i as f64 * 20.0, if i % 2 == 0 { 1.5 } else { -1.5 }])
            .collect();
        let mut rapida = original.clone();
        asentar_lo_rapido(&mut rapida);
        assert_eq!(rapida.len(), original.len());
        assert_eq!(rapida[0], original[0]);
        assert_eq!(rapida[29], original[29]);
        let peor = rapida[3..27].iter().map(|p| p[1].abs()).fold(0.0, f64::max);
        assert!(peor < 0.25, "la onda sigue ahi: {peor}");

        // Letra: muestras a dos unidades. Ni se mueve.
        let lenta: Vec<V> = (0..30)
            .map(|i| [i as f64 * 2.0, if i % 2 == 0 { 1.0 } else { -1.0 }])
            .collect();
        let mut tocada = lenta.clone();
        asentar_lo_rapido(&mut tocada);
        assert_eq!(tocada, lenta);
    }

    #[test]
    fn con_tres_puntos_o_menos_no_hay_nada_que_asentar() {
        let mut tres: Vec<V> = vec![[0.0, 0.0], [50.0, 9.0], [100.0, 0.0]];
        let antes = tres.clone();
        asentar_lo_rapido(&mut tres);
        assert_eq!(tres, antes);
    }

    #[test]
    fn asentar_la_entrada_no_toca_las_presiones() {
        let mut entrada: Vec<Entrada> = (0..10)
            .map(|i| Entrada {
                x: i as f64 * 25.0,
                y: if i % 2 == 0 { 2.0 } else { -2.0 },
                presion: Some(0.1 * i as f64),
            })
            .collect();
        asentar_entrada(&mut entrada);
        for (i, e) in entrada.iter().enumerate() {
            assert_eq!(e.presion, Some(0.1 * i as f64));
        }
        assert!(entrada[4].y.abs() < 2.0, "lo rapido tenia que moverse");
    }

    #[test]
    fn una_lista_vacia_da_un_contorno_vacio_sin_panico() {
        assert!(contorno(&[], &opciones()).is_empty());
    }

    // El original de la brief asumia un circulo de trece puntos, pero un
    // solo punto se duplica en `puntos_del_trazo` (desplazado (1,1)) antes
    // de llegar aqui: el contorno sale con tapas, no como el circulo de un
    // solo punto de `contorno_de_puntos`. El oraculo es la autoridad; esta
    // prueba solo verifica que el contorno quede cerca del clic.
    #[test]
    fn un_clic_sin_arrastrar_da_un_contorno_cerrado_alrededor_del_punto() {
        let size = opciones().size;
        let c = contorno(&[e(10.0, 10.0)], &opciones());
        assert!(!c.is_empty());
        for p in &c {
            assert!(p[0].is_finite() && p[1].is_finite());
            let r = (p[0] - 10.5).hypot(p[1] - 10.5);
            assert!(r <= size, "punto fuera del rango esperado: {p:?} r={r}");
        }
    }

    #[test]
    fn un_tamano_de_cero_no_dibuja_nada() {
        let o = Opciones {
            size: 0.0,
            ..opciones()
        };
        assert!(contorno(&[e(0.0, 0.0), e(10.0, 0.0)], &o).is_empty());
    }

    #[test]
    fn ningun_punto_del_contorno_es_nan_ni_infinito() {
        let entrada: Vec<Entrada> = (0..200)
            .map(|i| e(i as f64 * 3.0, ((i as f64) / 5.0).sin() * 30.0))
            .collect();
        assert!(
            contorno(&entrada, &opciones())
                .iter()
                .all(|p| p[0].is_finite() && p[1].is_finite())
        );
    }

    #[test]
    fn con_presion_real_apretar_engorda_el_trazo() {
        let recta = |p: f64| -> Vec<Entrada> {
            (0..30)
                .map(|i| Entrada {
                    x: i as f64 * 4.0,
                    y: 0.0,
                    presion: Some(p),
                })
                .collect()
        };
        let o = Opciones {
            simular_presion: false,
            ..opciones()
        };
        let alto = |c: Vec<V>| {
            let (mn, mx) = c
                .iter()
                .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
            mx - mn
        };
        assert!(alto(contorno(&recta(1.0), &o)) > alto(contorno(&recta(0.1), &o)));
    }
}
