//! **La «hoja de muestras» de las formas predefinidas de Office**: todas, en
//! rejilla, cada una con su nombre debajo, pintadas con el mismo pintor que
//! Word y PowerPoint (`pagina_office`).
//!
//! ```text
//! cargo run -p pixpin-render --example formas_a_png -- <carpeta> [escala]
//! ```
//!
//! Escribe `formas-1.png`, `formas-2.png`... (unas 48 formas por hoja).

use std::path::PathBuf;

use pixpin_codec::{FormatoImagen, ImagenRgba, guardar};
use pixpin_office::geometria;
use pixpin_office::pagina::{
    Color as ColorO, Elemento, Letra, Pagina, Recursos, Relleno, Texto, Trazo,
};
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(carpeta) = args.first() else {
        eprintln!("uso: formas_a_png <carpeta> [escala]");
        std::process::exit(2);
    };
    let carpeta = PathBuf::from(carpeta);
    let escala: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1.4);
    std::fs::create_dir_all(&carpeta).expect("carpeta de salida");
    // SAFETY: iniciar COM en este hilo, como en `office_a_png`.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let dispositivo = d3d();
    let motor = MotorRender::nuevo(&dispositivo).expect("motor");
    let pintor = PintorOffice::nuevo(&motor).expect("letras del sistema");

    let nombres = geometria::conocidas();
    let (columnas, filas) = (8usize, 6usize);
    let (celda_w, celda_h) = (150.0f32, 118.0f32);
    let (forma_w, forma_h) = (96.0f32, 64.0f32);
    let por_hoja = columnas * filas;
    let relleno = Relleno::Solido(ColorO::rgb(68, 114, 196));
    let borde = Trazo::solido(ColorO::rgb(47, 82, 143), 1.0);
    let mut hoja_n = 0;
    for grupo in nombres.chunks(por_hoja) {
        hoja_n += 1;
        let (ancho, alto) = (columnas as f32 * celda_w, filas as f32 * celda_h);
        let mut elementos = Vec::new();
        for (i, nombre) in grupo.iter().enumerate() {
            let (c, f) = ((i % columnas) as f32, (i / columnas) as f32);
            let (x, y) = (c * celda_w + (celda_w - forma_w) / 2.0, f * celda_h + 14.0);
            let figuras = geometria::predefinida(nombre, forma_w, forma_h, &Default::default());
            let mut e = Elemento::Trayecto {
                figuras,
                relleno: Some(relleno.clone()),
                borde: Some(borde),
            };
            e.trasladar(x, y);
            elementos.push(e);
            let texto = nombre.clone();
            elementos.push(Elemento::Texto(Texto {
                x: c * celda_w + 4.0,
                y: f * celda_h + 14.0 + forma_h + 22.0,
                letra: Letra::nueva("Segoe UI", 8.5),
                color: ColorO::rgb(40, 40, 40),
                ancho: texto.len() as f32 * 4.6,
                texto,
                subrayado: None,
                tachado: false,
                espaciado: 0.0,
                extra_espacio: 0.0,
                campo: None,
                enlace: None,
            }));
        }
        let pagina = Pagina {
            ancho,
            alto,
            fondo: None,
            elementos,
            numero: hoja_n,
        };
        let (w, h) = (
            (ancho * escala).ceil() as u32,
            (alto * escala).ceil() as u32,
        );
        let destino = FueraDePantalla::nuevo(&motor, &dispositivo, w, h).expect("destino");
        let recursos = Recursos::nuevo();
        motor
            .dibujar(&destino.destino, |p| {
                p.limpiar(Color::BLANCO);
                pintor.pintar(p, &pagina, &recursos, escala, (0.0, 0.0), 1);
            })
            .expect("dibujar");
        destino.esperar_gpu().expect("gpu");
        let (aw, ah, px) = destino.leer_rgba().expect("leer");
        let ruta = carpeta.join(format!("formas-{hoja_n}.png"));
        guardar(
            &ImagenRgba {
                ancho: aw,
                alto: ah,
                pixeles: px,
            },
            &ruta,
            FormatoImagen::Png,
        )
        .expect("png");
        println!("{}", ruta.display());
    }
}
