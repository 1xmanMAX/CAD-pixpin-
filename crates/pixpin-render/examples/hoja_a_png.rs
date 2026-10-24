//! Pinta una hoja de un `.xlsx` en un PNG y mide cuanto tarda cada paso.
//!
//! ```text
//! cargo run --release -p pixpin-render --example hoja_a_png -- <libro.xlsx> <salida.png> [hoja] [ancho alto] [opciones]
//! ```
//!
//! `hoja`: numero (desde 0) o nombre; por defecto, la activa. `ancho alto`:
//! tamano de la ventana (por defecto 1920 x 1080); `auto` en el ancho pinta
//! todo lo usado (sin encabezados, para comparar con el PDF de Excel).
//! Opciones: `--escala 1.25` (zoom), `--ppp 144` (pantalla), `--sin-encabezados`, `--es` / `--en`
//! (separadores y nombres), `--desde FILA` (desplazar hasta esa fila).

use std::time::Instant;

use pixpin_hojas::{Libro, Regional, Rejilla, Vista};
use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::hoja_office::{self, CacheHoja, OpcionesHoja};
use pixpin_render::lienzo::RectF;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn d3d() -> ID3D11Device {
    let mut d = None;
    // SAFETY: salidas locales y constantes documentadas.
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut d),
            None,
            None,
        )
        .expect("GPU");
    }
    d.expect("dispositivo")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut libres = Vec::new();
    let mut escala = 1.0f64;
    let mut ppp = 96.0f64;
    let mut encabezados = true;
    let mut regional = Regional::es_pe();
    let mut desde_fila = 0u32;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--escala" => {
                escala = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(1.0);
                i += 1;
            }
            "--desde" => {
                desde_fila = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                i += 1;
            }
            "--ppp" => {
                ppp = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(96.0);
                i += 1;
            }
            "--sin-encabezados" => encabezados = false,
            "--en" => regional = Regional::en_us(),
            "--es" => regional = Regional::es_pe(),
            "--sep-fecha" => {
                regional.separador_fecha = args
                    .get(i + 1)
                    .and_then(|s| s.chars().next())
                    .unwrap_or('/');
                i += 1;
            }
            a => libres.push(a.to_string()),
        }
        i += 1;
    }
    if libres.len() < 2 {
        eprintln!(
            "uso: hoja_a_png <libro.xlsx> <salida.png> [hoja] [ancho alto] [--escala N] [--sin-encabezados] [--en] [--desde FILA]"
        );
        std::process::exit(2);
    }
    // SAFETY: iniciar COM en este hilo (WIC lo necesita para las imagenes).
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    let t_total = Instant::now();
    let t = Instant::now();
    let libro = Libro::abrir(libres[0].as_ref(), regional).expect("abrir el libro");
    let t_abrir = ms(t);
    let indice = match libres.get(2) {
        Some(s) => s
            .parse::<usize>()
            .ok()
            .or_else(|| libro.hojas.iter().position(|h| &h.nombre == s))
            .unwrap_or(libro.hoja_activa),
        None => libro.hoja_activa,
    };
    let auto = libres.get(3).is_some_and(|s| s == "auto");
    let (mut ancho, mut alto) = (
        libres
            .get(3)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1920),
        libres
            .get(4)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1080),
    );
    if auto {
        encabezados = false;
    }

    // El dispositivo y el motor ya existen en la aplicacion: fuera de la cuenta.
    let disp = d3d();
    let motor = MotorRender::nuevo(&disp).expect("motor");
    let mut cache = CacheHoja::nueva();

    // Primera pantalla: la cabecera y las filas que caben.
    let t = Instant::now();
    let mut hoja = libro.abrir_hoja(indice);
    let filas_pantalla = desde_fila + (alto as f64 / (15.0 * escala)).ceil() as u32 + 2;
    hoja.cargar_hasta_fila(filas_pantalla);
    let digito = cache.ancho_digito(&motor, &libro.estilos.fuente_normal, ppp);
    let mut rejilla = Rejilla::nueva_en(&hoja, &libro.estilos.fuente_normal, digito, ppp);
    let t_primera = ms(t);
    println!(
        "fuente normal: {} {} pt, digito {:?} px (estimado {})",
        libro.estilos.fuente_normal.nombre,
        libro.estilos.fuente_normal.tam,
        digito,
        pixpin_hojas::rejilla::ancho_digito_en(&libro.estilos.fuente_normal, ppp)
    );

    // Pintar la primera pantalla (sin el resto cargado).
    let mut vista = Vista {
        ancho: ancho as f64,
        alto: alto as f64,
        escala,
        x: 0.0,
        y: rejilla.y(desde_fila),
    };
    let op = OpcionesHoja {
        encabezados,
        seleccion: None,
        activa: std::env::var("HOJA_ACTIVA")
            .ok()
            .and_then(|s| pixpin_hojas::hoja::Rango::leer(&s))
            .map(|r| (r.fila0, r.col0)),
        ..Default::default()
    };
    let t_pintar_primera;
    {
        let fuera =
            FueraDePantalla::nuevo(&motor, &disp, ancho.max(1), alto.max(1)).expect("superficie");
        let t = Instant::now();
        let (f0, f1) = (
            rejilla.fila_en(vista.y),
            rejilla.fila_en(vista.y + alto as f64 / escala) + 1,
        );
        hoja_office::autoajustar_filas(&mut cache, &motor, &libro, &hoja, &mut rejilla, f0, f1);
        motor
            .dibujar(&fuera.destino, |p| {
                hoja_office::pintar_hoja(
                    p,
                    &mut cache,
                    &libro,
                    &hoja,
                    &rejilla,
                    &vista,
                    RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: ancho as f32,
                        alto: alto as f32,
                    },
                    &op,
                )
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("gpu");
        t_pintar_primera = ms(t);
    }

    // El resto.
    let t = Instant::now();
    hoja.cargar_todo();
    rejilla.actualizar(&hoja);
    let t_todo = ms(t);
    if auto {
        hoja_office::autoajustar_filas(
            &mut cache,
            &motor,
            &libro,
            &hoja,
            &mut rejilla,
            0,
            hoja.max_fila(),
        );
        let (w, h) = rejilla.tamano_usado(&hoja);
        ancho = ((w + 2.0) * escala).ceil().clamp(1.0, 8000.0) as u32;
        alto = ((h + 2.0) * escala).ceil().clamp(1.0, 8000.0) as u32;
        vista = Vista {
            ancho: ancho as f64,
            alto: alto as f64,
            escala,
            x: 0.0,
            y: 0.0,
        };
        // Sin paneles: la hoja entera de una pieza.
        rejilla.filas_fijas = 0;
        rejilla.columnas_fijas = 0;
    }
    {
        let (f0, f1) = (
            rejilla.fila_en(vista.y),
            rejilla.fila_en(vista.y + alto as f64 / escala) + 1,
        );
        hoja_office::autoajustar_filas(&mut cache, &motor, &libro, &hoja, &mut rejilla, f0, f1);
    }
    let fuera =
        FueraDePantalla::nuevo(&motor, &disp, ancho.max(1), alto.max(1)).expect("superficie");
    let caja = RectF {
        x: 0.0,
        y: 0.0,
        ancho: ancho as f32,
        alto: alto as f32,
    };
    // Primero en frio (con la cache de textos de la primera pantalla) y
    // despues varias en caliente.
    let t = Instant::now();
    motor
        .dibujar(&fuera.destino, |p| {
            hoja_office::pintar_hoja(p, &mut cache, &libro, &hoja, &rejilla, &vista, caja, &op)
        })
        .expect("pintar");
    fuera.esperar_gpu().expect("gpu");
    let t_frio = ms(t);
    let medidas = cache.medidas;
    let mut tiempos = Vec::new();
    for _ in 0..10 {
        let t = Instant::now();
        motor
            .dibujar(&fuera.destino, |p| {
                hoja_office::pintar_hoja(p, &mut cache, &libro, &hoja, &rejilla, &vista, caja, &op)
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("gpu");
        tiempos.push(ms(t));
    }
    tiempos.sort_by(|a, b| a.total_cmp(b));
    // Pestanas: solo para ver que se pintan (no van al PNG).
    let (w, h, rgba) = fuera.leer_rgba().expect("leer");
    // Depurar: `PIXELES=x,y;x,y` imprime el color de esos pixeles.
    if let Ok(px) = std::env::var("PIXELES") {
        for par in px.split(';') {
            if let Some((x, y)) = par.split_once(',') {
                let (x, y): (u32, u32) =
                    (x.trim().parse().unwrap_or(0), y.trim().parse().unwrap_or(0));
                if x < w && y < h {
                    let i = ((y * w + x) * 4) as usize;
                    println!("pixel ({x},{y}): {:?}", &rgba[i..i + 4]);
                }
            }
        }
    }
    image::save_buffer(&libres[1], &rgba, w, h, image::ColorType::Rgba8).expect("guardar png");

    let celdas = hoja.celdas.len();
    println!(
        "libro: {} hojas, {} textos compartidos, {} estilos",
        libro.hojas.len(),
        libro.textos.len(),
        libro.estilos.xfs.len()
    );
    println!(
        "abrir: {:.1} ms (zip {:.1}, libro {:.1}, estilos {:.1}, textos {:.1})",
        t_abrir,
        libro.tiempos.zip,
        libro.tiempos.libro,
        libro.tiempos.estilos,
        libro.tiempos.textos
    );
    println!(
        "hoja '{}' ({}): primera pantalla lista en {:.1} ms (filas {}), pintada en {:.1} ms",
        hoja.nombre, indice, t_primera, filas_pantalla, t_pintar_primera
    );
    println!(
        "primera pantalla desde el doble clic: {:.1} ms",
        t_abrir + t_primera + t_pintar_primera
    );
    println!(
        "carga completa: {:.1} ms mas ({} filas, {} celdas, ~{} MB de celdas)",
        t_todo,
        hoja.filas.len(),
        celdas,
        celdas * 24 / (1024 * 1024)
    );
    println!(
        "pintar {}x{}: frio {:.2} ms, caliente mediana {:.2} ms (min {:.2}, max {:.2}); celdas miradas {}, textos {}, bordes {}",
        w,
        h,
        t_frio,
        tiempos[tiempos.len() / 2],
        tiempos[0],
        tiempos[tiempos.len() - 1],
        medidas.celdas,
        medidas.textos,
        medidas.bordes
    );
    let m2 = cache.medidas;
    println!(
        "  en caliente: preparar textos {:.2} ms, encargar textos {:.2} ms, disposiciones nuevas {}",
        m2.us_preparar_textos as f64 / 1000.0,
        m2.us_dibujar_textos as f64 / 1000.0,
        m2.disposiciones_nuevas
    );
    println!("total: {:.1} ms", ms(t_total));
}
