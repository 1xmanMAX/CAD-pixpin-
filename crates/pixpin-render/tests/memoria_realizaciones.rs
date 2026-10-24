//! **De donde sale la memoria que crece al mover trazos.**
//!
//! El banco del editor (`de_donde_sale_la_memoria_que_crece_al_dibujar`)
//! midio que 300 escenas enteras de 400 trazos, moviendo UNO en cada una,
//! hacian crecer el proceso y la memoria de video en mas de un giga, y que
//! ni vaciar las caches ni `Trim` lo devolvian; teselar los 400 de nuevo en
//! cada escena, en cambio, apenas crecia. Aqui se reproduce con lo minimo
//! -realizaciones de `CacheTinta` sobre una textura, sin el editor- para
//! ver que paso del ciclo es el que retiene. Necesita GPU: `--ignored`.
//!
//! Lo medido (2026-09-24, en el equipo de desarrollo): crece por cada
//! fotograma que pinta MUCHAS realizaciones despues de haberse creado una
//! nueva -en ese fotograma o antes-, en proporcion a cuantas pinta (unos
//! 10 KB por realizacion pintada; con 400, 5 MB por cada vez). No es la
//! geometria (identica y corrida crece igual), ni el orden dentro del
//! fotograma, ni el lote (`Flush` cada 8 no lo cambia), ni la memoria de
//! texturas de Direct2D (con su tope a 32 MB crece igual), ni lo que queda
//! en vuelo (esperando a la GPU crece igual). Pintar lo nuevo a pelo, sin
//! realizarlo, no crece; juntar las creaciones (diez de golpe) crece una
//! decima parte. Lo que se puede evitar es crear: `reusar_trasladada`
//! pinta lo que solo se movio con su realizacion de antes.
//!
//! ```text
//! cargo test --release -p pixpin-render --test memoria_realizaciones -- --ignored --nocapture --test-threads=1
//! ```

use pixpin_render::{CacheTinta, Color, MotorRender};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

fn motor_y_destino(
    ancho: u32,
    alto: u32,
) -> (
    MotorRender,
    windows::Win32::Graphics::Direct2D::ID2D1Bitmap1,
    ID3D11Device,
) {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11CreateDevice,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
    let mut d3d = None;
    // SAFETY: salidas locales; sin adaptador concreto ni capas de depuracion.
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut d3d),
            None,
            None,
        )
        .expect("sin D3D11 hardware");
    }
    let d3d = d3d.expect("dispositivo");
    let motor = MotorRender::nuevo(&d3d).expect("motor");
    let desc = D3D11_TEXTURE2D_DESC {
        Width: ancho,
        Height: alto,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        ..Default::default()
    };
    let mut textura = None;
    // SAFETY: descripcion local valida; salida local.
    unsafe {
        d3d.CreateTexture2D(&desc, None, Some(&mut textura))
            .expect("textura")
    };
    let destino = motor
        .destino_desde_textura(&textura.expect("textura"))
        .expect("destino");
    (motor, destino, d3d)
}

/// Memoria privada del proceso y de video (local + no local), en megas.
fn memoria(d3d: &ID3D11Device) -> (f64, f64) {
    let pid = std::process::id();
    let orden = format!("(Get-Process -Id {pid}).PrivateMemorySize64");
    let privada = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &orden])
        .output()
        .ok()
        .and_then(|s| {
            String::from_utf8_lossy(&s.stdout)
                .trim()
                .parse::<f64>()
                .ok()
        })
        .unwrap_or(0.0)
        / (1024.0 * 1024.0);
    let video = pixpin_render::fuera_de_pantalla::memoria_de_video(d3d)
        .map_or(0.0, |(l, n)| (l + n) as f64 / (1024.0 * 1024.0));
    (privada, video)
}

/// Un contorno cerrado de trazo, de unos 300 vertices, corrido `dx`.
fn contorno(i: usize, dx: f32) -> Vec<(f32, f32)> {
    let (bx, by) = ((i % 20) as f32 * 90.0 + dx, (i / 20) as f32 * 50.0);
    (0..300)
        .map(|k| {
            let a = k as f32 / 300.0 * std::f32::consts::TAU;
            (
                bx + 40.0 + a.cos() * 35.0 + (a * 7.0).sin() * 3.0,
                by + 20.0 + a.sin() * 15.0,
            )
        })
        .collect()
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio; medicion"]
fn mover_un_trazo_por_fotograma_no_retiene_memoria() {
    let (motor, destino, d3d) = motor_y_destino(1920, 1080);
    let negro = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let n = 400usize;
    let mut versiones = vec![1u32; n];
    let mut corrimiento = vec![0.0f32; n];
    let mut cache = CacheTinta::nueva();
    let fotograma = |cache: &mut CacheTinta, versiones: &[u32], corrimiento: &[f32]| {
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color::BLANCO);
                for i in 0..n {
                    let clave = (i as u64, versiones[i], 0);
                    if !p.pintar_realizada(cache, clave, negro) {
                        p.tinta_cacheada(cache, clave, &contorno(i, corrimiento[i]), negro);
                    }
                }
            })
            .expect("fotograma");
    };
    fotograma(&mut cache, &versiones, &corrimiento);
    let m0 = memoria(&d3d);
    for _ in 0..300 {
        fotograma(&mut cache, &versiones, &corrimiento);
    }
    let m1 = memoria(&d3d);
    println!(
        "300 sin mover:            priv {:+.1} MB, video {:+.1} MB",
        m1.0 - m0.0,
        m1.1 - m0.1
    );
    for k in 0..300 {
        let i = k % n;
        versiones[i] += 1;
        corrimiento[i] += 1.0;
        fotograma(&mut cache, &versiones, &corrimiento);
    }
    let m2 = memoria(&d3d);
    println!(
        "300 moviendo uno:         priv {:+.1} MB, video {:+.1} MB",
        m2.0 - m1.0,
        m2.1 - m1.1
    );
    // La puerta: mover no crea realizaciones (`reusar_trasladada`), y sin
    // crearlas no hay nada que crezca. Antes eran +360 MB.
    assert_eq!(
        cache.trasladadas(),
        300,
        "cada movimiento tenia que reusar su realizacion"
    );
    assert!(
        m2.1 - m1.1 < 40.0,
        "mover un trazo por fotograma retiene memoria de video: {:+.1} MB",
        m2.1 - m1.1
    );
    for k in 0..300 {
        let i = k % n;
        versiones[i] += 1;
        fotograma(&mut cache, &versiones, &corrimiento);
    }
    let m3 = memoria(&d3d);
    println!(
        "300 version nueva, quieto: priv {:+.1} MB, video {:+.1} MB",
        m3.0 - m2.0,
        m3.1 - m2.1
    );
    cache.vaciar();
    motor.devolver_memoria(&d3d);
    let m4 = memoria(&d3d);
    println!(
        "vaciar y Trim:            priv {:+.1} MB, video {:+.1} MB",
        m4.0 - m3.0,
        m4.1 - m3.1
    );
}

/// Cada variante en su propio proceso (`PIXPIN_VARIANTE`), porque lo que
/// crece se reutiliza despues y una variante enmascararia a la siguiente:
/// 0 = 400 realizadas pintadas y una nueva por fotograma; 1 = 400 realizadas
/// pero solo 50 pintadas; 2 = las mismas 400 a pelo, sin realizar; 3 = 400
/// realizadas, una nueva pintada a pelo (sin realizar) por fotograma; 6 = como
/// 0, pero los nuevos entran de diez en diez.
#[test]
#[ignore = "necesita GPU y sesion de escritorio; medicion"]
fn variante_de_trazos_nuevos() {
    let variante: u32 = std::env::var("PIXPIN_VARIANTE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let (motor, _, d3d) = motor_y_destino(16, 16);
    let fuera = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, &d3d, 1920, 1080)
        .expect("destino");
    let negro = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let n = 400usize;
    let mut cache = CacheTinta::nueva();
    let mut elementos: Vec<(u64, u32, f32)> = (0..n).map(|i| (i as u64, 1, 0.0)).collect();
    let fotograma = |elementos: &[(u64, u32, f32)], cache: &mut CacheTinta| {
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(Color::BLANCO);
                let ultimo = elementos.len() - 1;
                for (i, (id, v, dx)) in elementos.iter().enumerate() {
                    if variante == 1 && i >= 50 && i < ultimo {
                        continue;
                    }
                    let c = contorno(i % n, *dx);
                    if variante == 2 || (variante == 3 && i == ultimo && i >= n) {
                        p.tinta(&c, negro);
                        continue;
                    }
                    let clave = (*id, *v, 0);
                    if !p.pintar_realizada(cache, clave, negro) {
                        p.tinta_cacheada(cache, clave, &c, negro);
                    }
                }
            })
            .expect("fotograma");
        fuera.esperar_gpu().expect("GPU");
    };
    fotograma(&elementos, &mut cache);
    let m0 = memoria(&d3d);
    for k in 0..150 {
        if variante == 3 {
            // Uno nuevo cada vez, que sustituye al anterior nuevo.
            elementos.truncate(n);
        }
        elementos.push((10_000 + k as u64, 1, 0.37 * k as f32));
        // 6: los nuevos se juntan y entran de diez en diez.
        if variante == 6 && k % 10 != 9 {
            continue;
        }
        fotograma(&elementos, &mut cache);
    }
    let m1 = memoria(&d3d);
    println!(
        "variante {variante}: 150 fotogramas con un trazo nuevo: priv {:+.1} MB, video {:+.1} MB",
        m1.0 - m0.0,
        m1.1 - m0.1
    );
}
