//! Imprimir con `ID2D1PrintControl`, sin papel de por medio.
//!
//! La prueba de verdad manda un lienzo a «Microsoft Print to PDF» **con la
//! salida a un fichero**: el camino entero (lista de ordenes, XPS, cola) sin
//! tocar una impresora. Va `#[ignore]` porque depende del servicio de cola de
//! impresion, que en un CI o en un Windows recortado puede no estar; se
//! lanza a mano con `cargo test -p pixpin-render --test imprimir -- --ignored`.
//! `PIXPIN_MUESTRA_IMPRESION` dice donde dejar el fichero para mirarlo.

use pixpin_render::imprimir::{Trabajo, imprimir};
use pixpin_render::{Color, MotorRender, RectF};

fn motor() -> MotorRender {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
    };
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
    MotorRender::nuevo(&d3d.expect("dispositivo")).expect("motor")
}

#[test]
fn una_impresora_que_no_existe_da_un_error_y_no_cuelga() {
    let m = motor();
    let mut pintadas = 0;
    let r = imprimir(
        &m,
        &Trabajo {
            impresora: "PixPin impresora que no existe 12345",
            nombre: "prueba",
            devmode: None,
            paginas: &[(793.7, 1122.5)],
            fichero: None,
        },
        &mut |_, _| pintadas += 1,
    );
    assert!(r.is_err());
    assert_eq!(pintadas, 0, "sin impresora no se pinta ni una pagina");
}

#[test]
#[ignore = "usa la cola de impresion de Windows (Microsoft Print to PDF)"]
fn imprimir_a_microsoft_print_to_pdf_deja_un_pdf_en_el_fichero() {
    let m = motor();
    let ruta = std::env::var_os("PIXPIN_MUESTRA_IMPRESION")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("pixpin-impresion-prueba.pdf"));
    let _ = std::fs::remove_file(&ruta);
    // Dos paginas A4 en DIP: una por marco, como imprime el movil.
    let paginas = [(793.7, 1122.5), (1122.5, 793.7)];
    let mut vistas = Vec::new();
    imprimir(
        &m,
        &Trabajo {
            impresora: "Microsoft Print to PDF",
            nombre: "PixPin prueba",
            devmode: None,
            paginas: &paginas,
            fichero: Some(&ruta),
        },
        &mut |i, p| {
            vistas.push(i);
            p.rellenar(
                RectF {
                    x: 100.0,
                    y: 100.0,
                    ancho: 300.0,
                    alto: 200.0,
                },
                Color {
                    r: 0.1,
                    g: 0.3,
                    b: 0.9,
                    a: 1.0,
                },
            );
            p.texto_ajustado(
                &format!("Hoja {}", i + 1),
                100.0,
                340.0,
                32.0,
                600.0,
                Color::NEGRO,
            );
        },
    )
    .expect("imprime");
    assert_eq!(vistas, [0, 1]);
    let bytes = std::fs::read(&ruta).expect("hay fichero");
    assert!(bytes.len() > 100, "{} bytes", bytes.len());
    eprintln!(
        "salida: {} bytes, empieza por {:?}",
        bytes.len(),
        &bytes[..8.min(bytes.len())]
    );
}
