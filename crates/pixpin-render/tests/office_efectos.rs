//! **Ver los efectos de DrawingML pintados**: maqueta los `.pptx` y `.docx`
//! de una carpeta y escribe cada pagina en PNG al lado, para mirarla.
//!
//! ```text
//! set PIXPIN_OFFICE_MUESTRAS=C:\carpeta\con\muestras
//! cargo test -p pixpin-render --test office_efectos -- --ignored
//! ```

use std::path::PathBuf;

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
    // SAFETY: salidas locales y constantes documentadas, como en el ejemplo
    // `office_a_png`.
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

#[test]
#[ignore = "pinta a PNG para mirarlo: necesita GPU y PIXPIN_OFFICE_MUESTRAS"]
fn pintar_muestras_con_efectos() {
    let carpeta =
        PathBuf::from(std::env::var("PIXPIN_OFFICE_MUESTRAS").expect("PIXPIN_OFFICE_MUESTRAS"));
    // SAFETY: iniciar COM en este hilo para WIC.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let dispositivo = d3d();
    let motor = MotorRender::nuevo(&dispositivo).expect("motor");
    let pintor = PintorOffice::nuevo(&motor).expect("letras del sistema");
    let mut ficheros: Vec<PathBuf> = std::fs::read_dir(&carpeta)
        .expect("carpeta")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "pptx" || x == "docx"))
        .collect();
    ficheros.sort();
    assert!(!ficheros.is_empty(), "no hay muestras en {carpeta:?}");
    let escala = 2.0;
    for ruta in ficheros {
        let mut doc = Documento::abrir(&ruta).expect("abrir");
        let recursos = doc.recursos();
        let mut n = 0;
        while let Some(pagina) = doc.pagina(n, &pintor.letras) {
            let (w, h) = (
                (pagina.ancho * escala).ceil() as u32,
                (pagina.alto * escala).ceil() as u32,
            );
            let destino =
                FueraDePantalla::nuevo(&motor, &dispositivo, w.max(1), h.max(1)).expect("destino");
            let total = doc.total() as u32;
            let t = std::time::Instant::now();
            motor
                .dibujar(&destino.destino, |p| {
                    p.limpiar(Color::BLANCO);
                    pintor.pintar(p, &pagina, &recursos, escala, (0.0, 0.0), total);
                })
                .expect("dibujar");
            destino.esperar_gpu().expect("gpu");
            println!(
                "{} pagina {}: {:.1} ms",
                ruta.display(),
                n + 1,
                t.elapsed().as_secs_f64() * 1000.0
            );
            let (aw, ah, px) = destino.leer_rgba().expect("leer");
            let nombre = carpeta.join(format!(
                "{}-{:02}.png",
                ruta.file_name().unwrap_or_default().to_string_lossy(),
                n + 1
            ));
            guardar(
                &ImagenRgba {
                    ancho: aw,
                    alto: ah,
                    pixeles: px,
                },
                &nombre,
                FormatoImagen::Png,
            )
            .expect("png");
            n += 1;
            if n > 20 {
                break;
            }
        }
    }
}
