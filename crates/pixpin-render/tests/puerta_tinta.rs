//! Puertas de pintado de E1 (spec §5). Necesitan GPU: `--ignored`.
//!
//! Hay dos escenarios y cada uno tiene su propio tope:
//! - **Mientras se dibuja** (D120, ver `apps/pixpin/src/ventana_editor.rs`):
//!   la escena de 1.000 trazos ya esta congelada en `CapaEstatica`, asi que
//!   el fotograma es un solo volcado (blit) mas pintar SOLO el trazo en
//!   curso (sin cache, porque cambia en cada fotograma). Ese es el que la
//!   spec acota en menos de 3 ms.
//! - **Repintar los 1.000 trazos completos** (p.ej. al reconstruir la capa
//!   estatica, o sin ella en un nivel de GPU mas modesto): no tiene un tope
//!   de 3 ms en la spec, pero si tiene que caber en un fotograma de 60 Hz.

use std::time::Instant;

use pixpin_render::{CacheTinta, CapaEstatica, Color, Estampa, MotorRender};

fn contorno(i: usize) -> Vec<(f32, f32)> {
    let (bx, by) = ((i % 40) as f32 * 45.0, (i / 40) as f32 * 40.0);
    (0..60)
        .map(|k| {
            let a = k as f32 / 60.0 * std::f32::consts::TAU;
            (bx + 20.0 + a.cos() * 15.0, by + 20.0 + a.sin() * 8.0)
        })
        .collect()
}

/// El trazo que el usuario todavia esta dibujando: no esta en la capa
/// estatica (D120), asi que se pinta sin cache en cada fotograma. Una banda
/// ondulada cerrada de unos 1.000 vertices, del mismo orden que un contorno
/// de tinta real de `contorno_de_lapiz`.
fn trazo_en_curso() -> Vec<(f32, f32)> {
    (0..1_000)
        .map(|i| {
            let t = i as f32 / 1_000.0 * std::f32::consts::TAU * 3.0;
            let ondulacion = (t * 5.0).sin() * 20.0;
            (
                960.0 + t.cos() * (400.0 + ondulacion),
                540.0 + t.sin() * (300.0 + ondulacion),
            )
        })
        .collect()
}

/// Las mismas lineas que `motor_y_destino_de_prueba` de `src/tinta.rs`
/// (Tarea 10): una prueba de integracion no ve los ayudantes `#[cfg(test)]`
/// del crate, asi que se repiten aqui.
fn motor_y_destino(
    ancho: u32,
    alto: u32,
) -> (
    MotorRender,
    windows::Win32::Graphics::Direct2D::ID2D1Bitmap1,
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
    (motor, destino)
}

/// El escenario que NO es el de la spec §5: repintar los 1.000 trazos
/// completos (por ejemplo al reconstruir la capa estatica) en vez de un solo
/// volcado mas el trazo en curso. La spec no le pone 3 ms, pero si tiene que
/// caber en un fotograma de 60 Hz para no notarse como una pausa.
#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn repintar_mil_trazos_realizados_cabe_en_un_fotograma_de_60_hz() {
    let (motor, destino) = motor_y_destino(1920, 1080);
    let contornos: Vec<_> = (0..1_000).map(contorno).collect();
    let mut cache = CacheTinta::nueva();
    let negro = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let fotograma = |cache: &mut CacheTinta| {
        motor
            .dibujar(&destino, |p| {
                for (i, c) in contornos.iter().enumerate() {
                    p.tinta_cacheada(cache, (i as u64, 1, 0), c, negro);
                }
            })
            .unwrap();
    };
    fotograma(&mut cache); // realiza
    let mut mejor = std::time::Duration::MAX;
    for _ in 0..10 {
        let t = Instant::now();
        fotograma(&mut cache);
        mejor = mejor.min(t.elapsed());
    }
    // Un fotograma a 60 Hz son 16 666 us; no es el tope de 3 ms de la spec
    // §5, que es para el fotograma MIENTRAS SE DIBUJA (ver la otra puerta).
    println!("repintar 1.000 trazos: {mejor:?}");
    assert!(
        mejor.as_micros() < 16_666,
        "repintar 1.000 trazos en {mejor:?}"
    );
}

/// El escenario de la spec §5 de verdad: mientras se dibuja, los 1.000
/// trazos ya realizados viven en `CapaEstatica` (D120) y el fotograma solo
/// vuelca ese bitmap y pinta, sin cache, el trazo que esta en curso.
#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn un_fotograma_dibujando_con_mil_trazos_en_escena_cuesta_menos_de_tres_milisegundos() {
    let (mut motor, destino) = motor_y_destino(1920, 1080);
    let contornos: Vec<_> = (0..1_000).map(contorno).collect();
    let en_curso = trazo_en_curso();
    let negro = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let blanco = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    let mut capa = CapaEstatica::nueva();
    let estampa = Estampa {
        camara: (0.0, 0.0, 1.0),
        tamano: (1920, 1080),
        // El elemento 1000 no existe entre los 0..1000 de arriba: es el
        // trazo en curso, que por eso no esta en la capa congelada.
        excluidos: vec![1000],
    };
    capa.preparar(&mut motor, estampa.clone(), |p| {
        p.limpiar(blanco);
        for c in &contornos {
            p.tinta(c, negro);
        }
    })
    .expect("la capa estatica se prepara una vez, antes de medir");

    let fotograma = |motor: &mut MotorRender| {
        let vale = capa.volcar(motor, &destino, &estampa);
        assert!(
            vale,
            "la estampa no cambia entre fotogramas: tiene que volcar"
        );
        motor
            .dibujar(&destino, |p| p.tinta(&en_curso, negro))
            .unwrap();
    };
    fotograma(&mut motor); // en frio
    let mut mejor = std::time::Duration::MAX;
    for _ in 0..10 {
        let t = Instant::now();
        fotograma(&mut motor);
        mejor = mejor.min(t.elapsed());
    }
    println!("un fotograma dibujando con 1.000 trazos en escena: {mejor:?}");
    assert!(
        mejor.as_micros() < 3_000,
        "un fotograma dibujando con 1.000 trazos en escena tarda {mejor:?}"
    );
}
