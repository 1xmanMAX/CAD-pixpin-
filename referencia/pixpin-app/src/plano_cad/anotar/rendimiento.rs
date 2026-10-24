//! **Cuanto cuesta acercar un plano anotado** (pedido del 13-oct-2026: «en
//! los CAD con anotaciones el zoom va lagueado»). Se mide con la tarjeta de
//! verdad, el plano de verdad y una capa con cientos de trazos del lapiz del
//! motor, fotograma a fotograma, con y sin la capa, esperando a la GPU al
//! final de cada uno (asi lo que se mide es lo que se ve, no lo que se
//! encola).
//!
//! `PIXPIN_CAD_PLANO=<copia de un dwg> cargo test -p pixpin --bin pixpinmax
//! medir_zoom_del_plano_anotado -- --ignored --nocapture --test-threads=1`

use std::time::{Duration, Instant};

use pixpin_cad::anotado::Anotador;
use pixpin_cad::gpu::{Gpu, Vista};

use super::pruebas::carpeta;
use super::*;

/// Un azar fijo (las mismas capas en cada vuelta).
struct Azar(u64);
impl Azar {
    fn sig(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Lo que mira la ventana del plano: el centro y las unidades por pixel.
#[derive(Clone, Copy)]
struct Cam {
    centro: [f64; 2],
    px: f64,
}

const W: u32 = 1200;
const H: u32 = 800;

fn vista_motor(c: Cam, unidad: f64) -> VistaPlano {
    VistaPlano {
        centro: c.centro,
        px: c.px,
        ancho: W,
        alto: H,
        origen_pantalla: (0, 0),
        escala_por_cien: 100,
        unidad,
        claro: true,
    }
}

/// **Una capa como la de un plano de verdad**: `n` trazos del lapiz del
/// motor, hechos con el raton (sus eventos, sin tocar el escritorio) en
/// ocho sitios del plano y a tres aumentos (como quien anota acercandose a
/// cada detalle), cada uno de ~120 muestras de un pulso de mano.
fn anotar_mucho(motor: &mut MotorDelPlano, m: &Modelo, n: usize) {
    let unidad = pixpin_cad::anotado::unidad_de_la_capa(m);
    let ancho = (m.caja[2] - m.caja[0]) as f64;
    let alto = (m.caja[3] - m.caja[1]) as f64;
    let mut az = Azar(7);
    motor
        .tinta
        .gesto
        .tomar_herramienta(pixpin_motor2d::gesto::Herramienta::Lapiz);
    for i in 0..n {
        // Cada 50 trazos, otro sitio y otro aumento.
        let sitio = i / 50;
        let mut az_s = Azar(1000 + sitio as u64);
        let centro = [
            m.caja[0] as f64 + ancho * (0.2 + 0.6 * az_s.sig()),
            m.caja[1] as f64 + alto * (0.2 + 0.6 * az_s.sig()),
        ];
        let k = [1.0, 4.0, 16.0][sitio % 3];
        let cam = Cam {
            centro,
            px: ancho / W as f64 / k,
        };
        let v = vista_motor(cam, unidad);
        let (mut x, mut y) = (250.0 + 700.0 * az.sig(), 250.0 + 300.0 * az.sig());
        let mut ang = az.sig() * std::f64::consts::TAU;
        motor.evento(EventoPlano::Mover(x as i32, y as i32), &v);
        motor.evento(EventoPlano::Bajar(x as i32, y as i32), &v);
        for _ in 0..120 {
            ang += (az.sig() - 0.5) * 0.6;
            x = (x + 3.0 * ang.cos()).clamp(150.0, 1050.0);
            y = (y + 3.0 * ang.sin()).clamp(150.0, 650.0);
            motor.evento(EventoPlano::Mover(x as i32, y as i32), &v);
        }
        motor.evento(EventoPlano::Subir(x as i32, y as i32), &v);
    }
}

#[derive(Default, Clone, Copy)]
struct Fotograma {
    total: f64,
    capa: f64,
    presentar: f64,
    gpu: f64,
    realizaciones: u64,
    modo: Option<Modo>,
    foto: f64,
    zoom: f64,
}

fn resumen(nombre: &str, f: &[Fotograma]) -> (f64, f64) {
    let mut t: Vec<f64> = f.iter().map(|x| x.total).collect();
    t.sort_by(f64::total_cmp);
    let media = t.iter().sum::<f64>() / t.len() as f64;
    let p95 = t[(t.len() * 95 / 100).min(t.len() - 1)];
    let max = *t.last().unwrap();
    let capa = f.iter().map(|x| x.capa).sum::<f64>() / f.len() as f64;
    let capa_max = f.iter().map(|x| x.capa).fold(0.0, f64::max);
    let pres = f.iter().map(|x| x.presentar).sum::<f64>() / f.len() as f64;
    let gpu = f.iter().map(|x| x.gpu).sum::<f64>() / f.len() as f64;
    let real: u64 = f.iter().map(|x| x.realizaciones).sum();
    let lentos = t.iter().filter(|x| **x > 8.0).count();
    let quietos: Vec<f64> = f
        .iter()
        .filter(|x| x.realizaciones == 0)
        .map(|x| x.capa)
        .collect();
    let capa_sin_teselar = quietos.iter().sum::<f64>() / quietos.len().max(1) as f64;
    println!(
        "MEDIDA {nombre}: {} fotogramas  media {media:.2} ms  mediana {:.2}  p95 {p95:.2}  max {max:.2}  (>8 ms: {lentos})  | capa media {capa:.2} max {capa_max:.2}  presentar {pres:.2}  esperar gpu {gpu:.2}  teselados {real}  capa sin teselar {capa_sin_teselar:.2}",
        f.len(),
        t[t.len() / 2]
    );
    let cuenta = |m: Modo| f.iter().filter(|x| x.modo == Some(m)).count();
    if f.iter().any(|x| x.modo.is_some()) {
        println!(
            "  modos: directo {} foto {} movida {}",
            cuenta(Modo::Directo),
            cuenta(Modo::Foto),
            cuenta(Modo::Movida)
        );
        for m in [Modo::Foto, Modo::Movida] {
            let mut t: Vec<f64> = f
                .iter()
                .filter(|x| x.modo == Some(m))
                .map(|x| x.total)
                .collect();
            let mut c: Vec<f64> = f
                .iter()
                .filter(|x| x.modo == Some(m))
                .map(|x| x.capa)
                .collect();
            if t.is_empty() {
                continue;
            }
            t.sort_by(f64::total_cmp);
            c.sort_by(f64::total_cmp);
            let med = |v: &[f64]| v[v.len() / 2];
            let p95 = |v: &[f64]| v[(v.len() * 95 / 100).min(v.len() - 1)];
            println!(
                "  {m:?}: fotograma mediana {:.2} ms p95 {:.2} | capa mediana {:.2} ms p95 {:.2}",
                med(&t),
                p95(&t),
                med(&c),
                p95(&c)
            );
        }
    }
    (p95, max)
}

/// El recorrido de la camara de la ventana: la rueda a 1,25 por muesca, una
/// muesca cada tres fotogramas, y la camara que va hacia su destino un 38 %
/// por fotograma (`ventana::ver`). Diez muescas hacia dentro sobre un punto
/// con trazos, quieto, diez hacia fuera y un arrastre.
fn recorrido(inicio: Cam, punto: [f64; 2]) -> Vec<Cam> {
    let mut cams = Vec::new();
    let (mut cam, mut destino) = (inicio, inicio);
    let mut muescas: Vec<f64> = Vec::new();
    muescas.extend(std::iter::repeat_n(1.0 / 1.25, 10));
    muescas.extend(std::iter::repeat_n(f64::NAN, 1)); // quieto
    muescas.extend(std::iter::repeat_n(1.25, 10));
    for f in muescas {
        let pasos = if f.is_nan() { 20 } else { 3 };
        if !f.is_nan() {
            // Dejar quieto el punto bajo el cursor.
            let px = destino.px * f;
            destino = Cam {
                centro: [
                    punto[0] + (destino.centro[0] - punto[0]) * f,
                    punto[1] + (destino.centro[1] - punto[1]) * f,
                ],
                px,
            };
        }
        for _ in 0..pasos {
            let k = 0.38;
            cam = Cam {
                px: (cam.px.ln() + (destino.px.ln() - cam.px.ln()) * k).exp(),
                centro: [
                    cam.centro[0] + (destino.centro[0] - cam.centro[0]) * k,
                    cam.centro[1] + (destino.centro[1] - cam.centro[1]) * k,
                ],
            };
            // Y llega, como en la ventana: casi en el destino, el destino.
            if (cam.px - destino.px).abs() < destino.px * 1e-4
                && (cam.centro[0] - destino.centro[0]).abs() < destino.px * 0.05
                && (cam.centro[1] - destino.centro[1]).abs() < destino.px * 0.05
            {
                cam = destino;
            }
            cams.push(cam);
        }
    }
    // Un arrastre de lado a lado.
    for i in 0..30 {
        cams.push(Cam {
            centro: [cam.centro[0] + cam.px * 20.0 * i as f64, cam.centro[1]],
            px: cam.px,
        });
    }
    cams
}

fn vista_gpu(c: Cam) -> Vista {
    Vista {
        escala: [
            (2.0 / (W as f64 * c.px)) as f32,
            (2.0 / (H as f64 * c.px)) as f32,
        ],
        centro: [c.centro[0] as f32, c.centro[1] as f32],
        color7: [0.0, 0.0, 0.0, 1.0],
        px: c.px as f32,
        modo: 2.0,
        relleno: [0.0; 2],
    }
}

/// Pasa el recorrido; con `motor`, con la capa encima (como la ventana).
fn pasar(
    gpu: &mut Gpu,
    plano: &pixpin_cad::gpu::PlanoGpu,
    cams: &[Cam],
    unidad: f64,
    mut motor: Option<&mut MotorDelPlano>,
) -> Vec<Fotograma> {
    let fondo = [0.99, 0.988, 0.976, 1.0];
    let mut fs = Vec::new();
    // Despues del recorrido, como la ventana: otro fotograma mientras lo
    // anotado diga que no quedo nitido (y se cuentan).
    let mut i = 0;
    loop {
        let c = match cams.get(i) {
            Some(c) => c,
            None if motor.as_ref().is_some_and(|m| m.falta_pintar()) && i < cams.len() + 300 => {
                cams.last().unwrap()
            }
            None => break,
        };
        i += 1;
        let t0 = Instant::now();
        let antes = motor.as_ref().map_or(0, |m| m.tinta.realizaciones());
        let mut capa = (t0, t0);
        let v = vista_motor(*c, unidad);
        let capas = [(plano, vista_gpu(*c))];
        gpu.dibujar_con(fondo, &capas, &mut |d, t| {
            capa.0 = Instant::now();
            if let Some(m) = motor.as_deref_mut() {
                m.pintar(d, t, &v, false);
            }
            capa.1 = Instant::now();
        })
        .unwrap();
        let t1 = Instant::now();
        gpu.esperar();
        let t2 = Instant::now();
        let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1000.0;
        fs.push(Fotograma {
            total: ms(t0, t2),
            capa: ms(capa.0, capa.1),
            presentar: ms(capa.1, t1),
            gpu: ms(t1, t2),
            realizaciones: motor
                .as_ref()
                .map_or(0, |m| m.tinta.realizaciones())
                .saturating_sub(antes),
            modo: motor.as_ref().map(|m| m.medida.0),
            foto: motor
                .as_ref()
                .map_or(0.0, |m| m.medida.1.as_secs_f64() * 1000.0),
            zoom: unidad / c.px,
        });
    }
    fs
}

#[test]
#[ignore = "necesita GPU y PIXPIN_CAD_PLANO (una copia de un dwg); mide el zoom con la capa"]
fn medir_zoom_del_plano_anotado() {
    let ruta = std::env::var("PIXPIN_CAD_PLANO").expect("PIXPIN_CAD_PLANO");
    let n: usize = std::env::var("PIXPIN_CAD_TRAZOS")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(400);
    let (m, _) = pixpin_cad::convertir::convertir_fichero(Path::new(&ruta)).unwrap();
    let unidad = pixpin_cad::anotado::unidad_de_la_capa(&m);
    println!("plano caja {:?} unidad {unidad}", m.caja);

    // La capa: se anota, se guarda (`anot-<uid>.excalidraw.gz` y su `.hoja`)
    // y se vuelve a abrir, como al abrir un plano ya anotado.
    let d = carpeta("rendimiento");
    let rutas = Rutas {
        tinta: d.join("anot-prueba.excalidraw.gz"),
        hoja: d.join("anot-prueba.hoja"),
    };
    let mut motor = MotorDelPlano::nuevo(rutas.clone(), &m);
    let t = Instant::now();
    anotar_mucho(&mut motor, &m, n);
    let hechos = motor.capa.escena.visibles().count();
    motor.terminar();
    println!(
        "anotados {n} trazos ({hechos} en la capa) en {:?}",
        t.elapsed()
    );
    let mut motor = MotorDelPlano::nuevo(rutas.clone(), &m);
    let leidos = motor.capa.escena.visibles().count();
    assert!(
        leidos == hechos && hechos * 10 >= n * 9,
        "la capa se leyo entera: {leidos} de {hechos}"
    );

    let mut gpu = Gpu::para_medir(W, H).unwrap();
    let plano = gpu.subir(&m).unwrap();

    let ancho = (m.caja[2] - m.caja[0]) as f64;
    let alto = (m.caja[3] - m.caja[1]) as f64;
    let px0 = (ancho / (W as f64 * 0.92)).max(alto / (H as f64 * 0.92));
    let inicio = Cam {
        centro: [
            (m.caja[0] + m.caja[2]) as f64 / 2.0,
            (m.caja[1] + m.caja[3]) as f64 / 2.0,
        ],
        px: px0,
    };
    // Hacia el primer sitio anotado.
    let mut az_s = Azar(1000);
    let punto = [
        m.caja[0] as f64 + ancho * (0.2 + 0.6 * az_s.sig()),
        m.caja[1] as f64 + alto * (0.2 + 0.6 * az_s.sig()),
    ];
    let cams = recorrido(inicio, punto);

    // Lo que cuesta en la CPU el contorno de todos los trazos (lo que se
    // rehace al cambiar de octava de zoom).
    let t = Instant::now();
    let mut nc = 0;
    for e in motor.capa.escena.visibles() {
        nc += pixpin_motor2d::pintado::ordenes_a_distancia(e, (unidad / px0) as f32).len();
    }
    println!(
        "MEDIDA contornos de toda la capa en la CPU: {:.2} ms ({nc} ordenes)",
        t.elapsed().as_secs_f64() * 1000.0
    );

    // Calentar: todo realizado al encuadre, como tras abrir.
    pasar(&mut gpu, &plano, &[inicio; 20], unidad, Some(&mut motor));
    pasar(&mut gpu, &plano, &[inicio; 20], unidad, None);

    for k in [1.0, 4.0, 16.0] {
        desglose(
            &mut gpu,
            &plano,
            &mut motor,
            Cam {
                centro: punto,
                px: px0 / k,
            },
            unidad,
        );
    }
    for k in [1.0f32, 4.0, 9.0] {
        estirado(&mut gpu, &plano, &mut motor, inicio, unidad, k);
    }
    motor.motor = None;
    // Otra vez a punto (cambiar de Direct2D tira lo teselado), como tras abrir.
    pasar(&mut gpu, &plano, &[inicio; 20], unidad, Some(&mut motor));

    let mut sin = Vec::new();
    let mut con = Vec::new();
    let mut antes = Vec::new();
    for _ in 0..3 {
        sin.extend(pasar(&mut gpu, &plano, &cams, unidad, None));
        // El antes: la capa trazo a trazo en cada fotograma y, al pasar de
        // vez y media de zoom, todo teselado de golpe.
        motor.como_antes = true;
        antes.extend(pasar(&mut gpu, &plano, &cams, unidad, Some(&mut motor)));
        motor.como_antes = false;
        con.extend(pasar(&mut gpu, &plano, &cams, unidad, Some(&mut motor)));
    }
    let (p95_sin, _) = resumen("zoom SIN capa", &sin);
    resumen(&format!("ANTES zoom CON capa de {n} trazos"), &antes);
    let (p95_con, max_con) = resumen(&format!("AHORA zoom CON capa de {n} trazos"), &con);
    // Los fotogramas mas caros con la capa, para ver de que son.
    let mut peores: Vec<(usize, Fotograma)> = con.iter().copied().enumerate().collect();
    peores.sort_by(|a, b| b.1.total.total_cmp(&a.1.total));
    for (i, f) in peores.iter().take(8) {
        println!(
            "  peor #{i}: {:.2} ms (capa {:.2}, de ella foto {:.2}, presentar {:.2}, gpu {:.2}) teselados {} zoom {:.3} modo {:?}",
            f.total, f.capa, f.foto, f.presentar, f.gpu, f.realizaciones, f.zoom, f.modo
        );
    }
    std::thread::sleep(Duration::from_millis(10));
    let _ = std::fs::remove_dir_all(&d);
    if std::env::var("PIXPIN_CAD_EXIGIR").is_ok() {
        assert!(
            p95_con < 8.0 && p95_con < p95_sin + 4.0,
            "p95 con capa {p95_con:.2} ms (sin {p95_sin:.2}), max {max_con:.2}"
        );
    }
}

/// El desglose de la capa en un fotograma sin cambios de zoom: envolver el
/// bufer, el BeginDraw/EndDraw vacio y lo pintado.
fn desglose(
    gpu: &mut Gpu,
    plano: &pixpin_cad::gpu::PlanoGpu,
    motor: &mut MotorDelPlano,
    c: Cam,
    unidad: f64,
) {
    let v = vista_motor(c, unidad);
    let capas = [(plano, vista_gpu(c))];
    let (mut envolver, mut vacio, mut pintar, mut n) = (0.0, 0.0, 0.0, 0);
    let mut contadores = pixpin_render::motor::Contadores::default();
    for i in 0..45 {
        if i == 5 {
            (envolver, vacio, pintar, n) = (0.0, 0.0, 0.0, 0);
        }
        gpu.dibujar_con([1.0; 4], &capas, &mut |d, t| {
            if motor.motor.is_none() {
                motor.motor = Some((0, MotorRender::nuevo(d).unwrap()));
            }
            let (_, r) = motor.motor.take().unwrap();
            let t0 = Instant::now();
            let b = r.destino_backbuffer(t).unwrap();
            let t1 = Instant::now();
            r.dibujar(&b, |_| {}).unwrap();
            let t2 = Instant::now();
            r.dibujar(&b, |p| motor.pintar_con(p, &v, false, Modo::Directo))
                .unwrap();
            let t3 = Instant::now();
            contadores = r.contadores();
            motor.motor = Some((0, r));
            envolver += (t1 - t0).as_secs_f64() * 1000.0;
            vacio += (t2 - t1).as_secs_f64() * 1000.0;
            pintar += (t3 - t2).as_secs_f64() * 1000.0;
            n += 1;
        })
        .unwrap();
        gpu.esperar();
    }
    let n = n as f64;
    println!(
        "MEDIDA desglose zoom {:.3}: envolver {:.3} ms  dibujar vacio {:.3}  pintar capa {:.3}  creados {:?}",
        unidad / c.px,
        envolver / n,
        vacio / n,
        pintar / n,
        contadores
    );
}

/// Lo que cuesta pintar la capa al encuadre con todo teselado a otra escala
/// (`k` veces mas cerca) frente a teselado a la suya.
fn estirado(
    gpu: &mut Gpu,
    plano: &pixpin_cad::gpu::PlanoGpu,
    motor: &mut MotorDelPlano,
    c: Cam,
    unidad: f64,
    k: f32,
) {
    let v = vista_motor(c, unidad);
    let capas = [(plano, vista_gpu(c))];
    let cam = camara_de(&v);
    let vista = (
        cam.x,
        cam.y,
        cam.x + v.ancho as f32 / cam.zoom,
        cam.y + v.alto as f32 / cam.zoom,
    );
    let mut medir = |motor: &mut MotorDelPlano,
                     zoom_teselado: f32,
                     presupuesto: Option<Duration>|
     -> (f64, f64) {
        let mut t_cpu = 0.0;
        let mut t_total = 0.0;
        for _ in 0..10 {
            let t0 = Instant::now();
            gpu.dibujar_con([1.0; 4], &capas, &mut |d, t| {
                if motor.motor.is_none() {
                    motor.motor = Some((0, MotorRender::nuevo(d).unwrap()));
                }
                let (_, r) = motor.motor.take().unwrap();
                let b = r.destino_backbuffer(t).unwrap();
                let t1 = Instant::now();
                motor.tinta.presupuesto = presupuesto;
                r.dibujar(&b, |p| {
                    p.poner_vista((0.0, 0.0), cam.zoom, (-cam.x * cam.zoom, -cam.y * cam.zoom));
                    // `zoom_teselado` fija la escala de la cache (y la octava).
                    motor.tinta.pintar_capa_parte(
                        p,
                        0,
                        &motor.capa,
                        vista,
                        zoom_teselado,
                        papel(true),
                        false,
                        Parte::Dibujado,
                    );
                })
                .unwrap();
                t_cpu += t1.elapsed().as_secs_f64() * 1000.0;
                motor.motor = Some((0, r));
            })
            .unwrap();
            gpu.esperar();
            t_total += t0.elapsed().as_secs_f64() * 1000.0;
        }
        (t_cpu / 10.0, t_total / 10.0)
    };
    // Todo teselado `k` veces mas cerca (sin tope: de golpe).
    medir(motor, cam.zoom * k, None);
    // Pintado al encuadre sin tiempo de rehacer: todo de la otra escala.
    let (cpu_e, tot_e) = medir(motor, cam.zoom, Some(Duration::ZERO));
    // Y a su escala.
    medir(motor, cam.zoom, None);
    let (cpu_n, tot_n) = medir(motor, cam.zoom, None);
    println!(
        "MEDIDA teselado {k}x mas cerca y pintado al encuadre: cpu {cpu_e:.2} ms total {tot_e:.2}  |  a su escala: cpu {cpu_n:.2} total {tot_n:.2}"
    );
    motor.tinta.presupuesto = Some(PRESUPUESTO_DE_TESELAR);
}
