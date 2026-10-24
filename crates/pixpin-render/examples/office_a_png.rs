//! **Un `.docx` o `.pptx` a PNG, pagina a pagina**, midiendo cuanto tarda.
//!
//! ```text
//! cargo run --release -p pixpin-render --example office_a_png -- <fichero> <carpeta> [ppp] [--ref referencia.pdf] [--max N]
//! ```
//!
//! Escribe `pagina-NNN.png` por pagina y, con `--ref`, tambien
//! `lado-NNN.png`: la nuestra a la izquierda y la de Office (el PDF que
//! exporto Word o PowerPoint) a la derecha, a la misma escala.

use std::path::{Path, PathBuf};
use std::time::Instant;

use pixpin_codec::{FormatoImagen, ImagenRgba, guardar};
use pixpin_office::Documento;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::pagina_office::PintorOffice;
use pixpin_render::{Color, MotorRender};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

fn d3d() -> ID3D11Device {
    let mut d = None;
    // SAFETY: salidas locales y constantes documentadas, como en las pruebas
    // de dibujo del resto del proyecto.
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
        .expect("GPU real");
    }
    d.expect("el dispositivo sale cuando la llamada va bien")
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Pone dos imagenes una al lado de la otra (alto el de la mayor).
fn lado_a_lado(a: &ImagenRgba, b: &ImagenRgba) -> ImagenRgba {
    let sep = 8;
    let ancho = a.ancho + sep + b.ancho;
    let alto = a.alto.max(b.alto);
    let mut px = vec![200u8; (ancho * alto * 4) as usize];
    for (img, x0) in [(a, 0u32), (b, a.ancho + sep)] {
        for y in 0..img.alto {
            let o = ((y * img.ancho) * 4) as usize;
            let d = ((y * ancho + x0) * 4) as usize;
            px[d..d + (img.ancho * 4) as usize]
                .copy_from_slice(&img.pixeles[o..o + (img.ancho * 4) as usize]);
        }
    }
    ImagenRgba {
        ancho,
        alto,
        pixeles: px,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!(
            "uso: office_a_png <fichero.docx|pptx> <carpeta> [ppp] [--ref ref.pdf] [--max N]"
        );
        std::process::exit(2);
    }
    let ruta = PathBuf::from(&args[0]);
    let salida = PathBuf::from(&args[1]);
    let ppp: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(96.0);
    let referencia = args
        .iter()
        .position(|a| a == "--ref")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let volcar: Option<usize> = args
        .iter()
        .position(|a| a == "--volcar")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok());
    let maximo: usize = args
        .iter()
        .position(|a| a == "--max")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(usize::MAX);
    std::fs::create_dir_all(&salida).expect("carpeta de salida");
    // SAFETY: iniciar COM en este hilo para WIC y Windows.Data.Pdf.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };

    let dispositivo = d3d();
    let motor = MotorRender::nuevo(&dispositivo).expect("motor");
    let pintor = PintorOffice::nuevo(&motor).expect("letras del sistema");

    let t0 = Instant::now();
    let mut doc = Documento::abrir(&ruta).expect("abrir el documento");
    let abrir = ms(t0);
    let t1 = Instant::now();
    let primera = doc.pagina(0, &pintor.letras).expect("al menos una pagina");
    let maquetar_primera = ms(t1);
    let recursos = doc.recursos();
    if args.iter().any(|a| a == "--comparar") {
        // El principio de cada pagina, la nuestra y la de Office, para ver
        // donde empiezan a separarse.
        doc.maquetar_todo(&pintor.letras);
        let ref_txt = referencia
            .as_deref()
            .and_then(|r| pixpin_pdf::texto::de_fichero(r).ok())
            .unwrap_or_default();
        let saltar: usize = args
            .iter()
            .position(|a| a == "--saltar")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let limpiar = |s: &str| -> String {
            s.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .skip(saltar)
                .take(70)
                .collect()
        };
        for i in 0..doc.paginas_listas().max(ref_txt.len()) {
            let nuestra = doc
                .pagina(i, &pintor.letras)
                .map(|p| {
                    p.textos()
                        .iter()
                        .map(|(t, _)| t.texto.clone())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            let suya = ref_txt.get(i).map(|p| p.texto.clone()).unwrap_or_default();
            println!("{:3} N: {}", i + 1, limpiar(&nuestra));
            println!("    O: {}", limpiar(&suya));
        }
        return;
    }
    if args.iter().any(|a| a == "--contar") {
        // Solo maquetar todo y decir cuantas paginas salen y cuanto tarda.
        let t = Instant::now();
        doc.maquetar_todo(&pintor.letras);
        let todo = abrir + maquetar_primera + ms(t);
        let oficina = referencia
            .as_deref()
            .and_then(|r| pixpin_pdf::Documento::abrir(r).ok())
            .map(|p| p.paginas());
        println!(
            "{} | {} (Office: {}) | abrir {abrir:.1} ms | primera {:.1} ms | todo {:.1} ms",
            nombre_de(&ruta),
            doc.paginas_listas(),
            oficina.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
            abrir + maquetar_primera,
            todo
        );
        return;
    }

    let pdf = referencia
        .as_deref()
        .map(|r| pixpin_pdf::Documento::abrir(r).expect("abrir el PDF de referencia"));
    let escala = ppp / 72.0;
    let mut n = 0;
    let mut tiempos_pintar = Vec::new();
    let mut tiempos_maquetar = vec![maquetar_primera];
    let mut primera = Some(primera);
    let t_total = Instant::now();
    loop {
        if n >= maximo {
            break;
        }
        let t = Instant::now();
        let pagina = match primera.take() {
            Some(p) => p,
            None => match doc.pagina(n, &pintor.letras) {
                Some(p) => p,
                None => break,
            },
        };
        if n > 0 {
            tiempos_maquetar.push(ms(t));
        }
        if volcar == Some(n + 1) && args.iter().any(|a| a == "--formas") {
            fn bajar(els: &[pixpin_office::Elemento], nivel: usize) {
                for e in els {
                    let s = match e {
                        pixpin_office::Elemento::Texto(t) => format!("Texto {:?}", t.texto),
                        pixpin_office::Elemento::Rect { rect, .. } => format!("Rect {rect:?}"),
                        pixpin_office::Elemento::Linea { desde, hasta, .. } => {
                            format!("Linea {desde:?}-{hasta:?}")
                        }
                        pixpin_office::Elemento::Imagen { id, rect, .. } => {
                            format!("Imagen {id} {rect:?}")
                        }
                        pixpin_office::Elemento::Trayecto {
                            figuras, relleno, ..
                        } => {
                            format!(
                                "Trayecto {} fig, inicio {:?} relleno {}",
                                figuras.len(),
                                figuras.first().map(|f| f.inicio),
                                relleno.is_some()
                            )
                        }
                        pixpin_office::Elemento::Grupo {
                            transformada,
                            elementos,
                            ..
                        } => {
                            println!("{}Grupo {:?}", "  ".repeat(nivel), transformada);
                            bajar(elementos, nivel + 1);
                            continue;
                        }
                        pixpin_office::Elemento::Efecto { efectos, elementos } => {
                            println!("{}Efecto {:?}", "  ".repeat(nivel), efectos);
                            bajar(elementos, nivel + 1);
                            continue;
                        }
                    };
                    println!("{}{s}", "  ".repeat(nivel));
                }
            }
            bajar(&pagina.elementos, 0);
        }
        if volcar == Some(n + 1) {
            for (t, r) in pagina.textos() {
                println!(
                    "{:7.2}-{:7.2} {:7.2} {:5.1} {:?} {} #{:02X}{:02X}{:02X}",
                    r.x,
                    r.x + t.ancho,
                    t.y,
                    t.letra.tam,
                    t.letra.familia,
                    t.texto,
                    t.color.r,
                    t.color.g,
                    t.color.b
                );
            }
            if let Some(r) = &referencia
                && let Ok(paginas) = pixpin_pdf::texto::de_fichero(r)
                && let Some(pt) = paginas.get(n)
            {
                // Los renglones de Office: letras seguidas a la misma altura.
                let chars: Vec<char> = pt.texto.chars().collect();
                let mut renglon = String::new();
                let mut caja: Option<[f32; 4]> = None;
                let volcar_renglon = |s: &mut String, c: &Option<[f32; 4]>| {
                    if let Some(c) = c
                        && !s.trim().is_empty()
                    {
                        println!(
                            "REF {:7.2}-{:7.2} {:7.2}..{:7.2} {}",
                            c[0] * pagina.ancho,
                            c[2] * pagina.ancho,
                            c[1] * pagina.alto,
                            c[3] * pagina.alto,
                            s.trim()
                        );
                    }
                    s.clear();
                };
                for (k, ch) in chars.iter().enumerate() {
                    let Some(Some(b)) = pt.cajas.get(k) else {
                        renglon.push(*ch);
                        continue;
                    };
                    if let Some(u) = caja.as_mut() {
                        let alto = (u[3] - u[1]).max(b[3] - b[1]);
                        if (b[1] - u[1]).abs() < alto * 0.5 && b[0] >= u[0] - alto {
                            u[2] = u[2].max(b[2]);
                            renglon.push(*ch);
                            continue;
                        }
                        let anterior = caja;
                        volcar_renglon(&mut renglon, &anterior);
                    }
                    caja = Some(*b);
                    renglon.push(*ch);
                }
                volcar_renglon(&mut renglon, &caja);
            }
        }
        let (w, h) = (
            (pagina.ancho * escala).ceil() as u32,
            (pagina.alto * escala).ceil() as u32,
        );
        let destino =
            FueraDePantalla::nuevo(&motor, &dispositivo, w.max(1), h.max(1)).expect("destino");
        let tp = Instant::now();
        let total = doc.total() as u32;
        motor
            .dibujar(&destino.destino, |p| {
                p.limpiar(Color::BLANCO);
                pintor.pintar(p, &pagina, &recursos, escala, (0.0, 0.0), total);
            })
            .expect("dibujar");
        destino.esperar_gpu().expect("gpu");
        tiempos_pintar.push(ms(tp));
        let (aw, ah, px) = destino.leer_rgba().expect("leer");
        let nuestra = ImagenRgba {
            ancho: aw,
            alto: ah,
            pixeles: px,
        };
        let nombre = salida.join(format!("pagina-{:03}.png", n + 1));
        guardar(&nuestra, &nombre, FormatoImagen::Png).expect("png");
        if let Some(pdf) = &pdf
            && (n as u32) < pdf.paginas()
            && let Ok(ref_img) = pdf.renderizar(n as u32, aw)
        {
            let lado = lado_a_lado(&nuestra, &ref_img);
            guardar(
                &lado,
                &salida.join(format!("lado-{:03}.png", n + 1)),
                FormatoImagen::Png,
            )
            .expect("png");
        }
        n += 1;
    }
    let total = ms(t_total) + maquetar_primera;
    let suma_pintar: f64 = tiempos_pintar.iter().sum();
    let suma_maquetar: f64 = tiempos_maquetar.iter().sum();
    println!("fichero: {}", nombre_de(&ruta));
    println!(
        "paginas: {n} (Office: {})",
        pdf.as_ref()
            .map(|p| p.paginas().to_string())
            .unwrap_or_else(|| "?".into())
    );
    println!("abrir+leer: {abrir:.1} ms");
    println!(
        "maquetar la primera: {maquetar_primera:.1} ms  (primera lista: {:.1} ms)",
        abrir + maquetar_primera
    );
    println!("maquetar todas: {suma_maquetar:.1} ms");
    println!(
        "pintar: total {suma_pintar:.1} ms, media {:.2} ms, maxima {:.2} ms",
        suma_pintar / n.max(1) as f64,
        tiempos_pintar.iter().copied().fold(0.0, f64::max)
    );
    println!("todo (con PNG): {total:.1} ms");
}

fn nombre_de(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}
