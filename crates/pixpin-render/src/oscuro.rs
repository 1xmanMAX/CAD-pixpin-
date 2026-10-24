//! **El modo oscuro de los visores** (12-oct-2026, el usuario: «todos los
//! visores con modo oscuro, con un boton luna/sol en la barra del pin»).
//!
//! El criterio de colores es el de PixPin Android (`LectorPdfActivity.kt`,
//! `MATRIZ_INVERTIDA`, que a su vez copio la «N» del PC): **invertir la luz
//! y girar el matiz media vuelta** (el `invert(1) hue-rotate(180deg)` de
//! CSS, el «invertir inteligente» del telefono). Lo blanco se oscurece, lo
//! negro se aclara y **el tono se queda**: una raya roja sigue siendo roja,
//! una azul sigue azul, y una foto no sale en negativo de color.
//!
//! Sobre eso, dos retoques nuestros: lo blanco no va a negro puro sino a un
//! gris muy oscuro ([`PAPEL`], el de la barra de los pines) y lo negro no va
//! a blanco puro sino a un gris muy claro ([`TINTA`]): se lee mejor de noche
//! y no deslumbra.
//!
//! Hay dos maneras de usarlo:
//! - **Por color** ([`invertir_luz`], [`legible_sobre_oscuro`]): los
//!   pintores en vectores (Excel, Word, PowerPoint) cambian solo lo que es
//!   del papel o «automatico» (el fondo, la rejilla, el texto oscuro sin
//!   relleno debajo); los rellenos, las imagenes y los graficos se quedan
//!   con sus colores, como el modo oscuro de Word.
//! - **Por pixel** ([`matriz_d2d`]), para lo que llega ya rasterizado (la
//!   hoja de un PDF): la misma cuenta como un efecto de matriz de color de
//!   Direct2D al pintar, en la GPU, sin tocar ni duplicar el bitmap guardado.

use crate::motor::Color;

/// Lo blanco del documento, en oscuro: el gris de la barra de los pines.
pub const PAPEL: Color = Color {
    r: 0x1F as f32 / 255.0,
    g: 0x1F as f32 / 255.0,
    b: 0x22 as f32 / 255.0,
    a: 1.0,
};
/// Lo negro del documento, en oscuro.
pub const TINTA: Color = Color {
    r: 0xE6 as f32 / 255.0,
    g: 0xE6 as f32 / 255.0,
    b: 0xE8 as f32 / 255.0,
    a: 1.0,
};

/// El giro de matiz de 180 grados de Android (fila = canal de salida), que
/// deja los grises como estan: cada fila suma 1.
const GIRO: [[f32; 3]; 3] = [
    [-0.574, 1.430, 0.144],
    [0.426, 0.430, 0.144],
    [0.426, 1.430, -0.856],
];

/// La luz (luma) de un color, de 0 a 1: lo que decide si un texto se lee
/// sobre el papel oscuro.
pub fn luz(c: Color) -> f32 {
    0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
}

/// **Un color del documento, en oscuro**: la luz invertida y el tono
/// conservado, y lo que sale, entre [`PAPEL`] y [`TINTA`]. El alfa se queda.
pub fn invertir_luz(c: Color) -> Color {
    let inv = [1.0 - c.r, 1.0 - c.g, 1.0 - c.b];
    let canal = |k: usize| GIRO[k][0] * inv[0] + GIRO[k][1] * inv[1] + GIRO[k][2] * inv[2];
    let (lo, hi) = ([PAPEL.r, PAPEL.g, PAPEL.b], [TINTA.r, TINTA.g, TINTA.b]);
    // Se recorta al final, como la matriz de Android y la de Direct2D.
    let a = |k: usize| (lo[k] + (hi[k] - lo[k]) * canal(k)).clamp(0.0, 1.0);
    Color {
        r: a(0),
        g: a(1),
        b: a(2),
        a: c.a,
    }
}

/// **Un texto o una raya sobre el papel oscuro**: si es oscuro (el negro
/// «automatico», un azul marino) se aclara conservando su tono; si ya se
/// lee (blanco, amarillo, un verde claro) se queda como esta.
pub fn legible_sobre_oscuro(c: Color) -> Color {
    if luz(c) < 0.45 { invertir_luz(c) } else { c }
}

/// El color de un texto en oscuro segun lo que tenga debajo: con un relleno
/// propio (`con_relleno`) el texto se queda como lo puso su autor, que ya
/// se lee sobre ese relleno; sin el, el papel es oscuro y se aclara.
pub fn texto_en_oscuro(c: Color, con_relleno: bool) -> Color {
    if con_relleno {
        c
    } else {
        legible_sobre_oscuro(c)
    }
}

/// La cuenta de [`invertir_luz`] como la **matriz 5x4** del efecto
/// `ColorMatrix` de Direct2D (fila = canal de entrada R, G, B, A y la
/// quinta, lo que se suma; columna = canal de salida). Sale de
/// `PAPEL + (TINTA - PAPEL) * GIRO * (1 - c)`, y como cada fila de `GIRO`
/// suma 1, es `TINTA - (TINTA - PAPEL) * GIRO * c`.
pub fn matriz_d2d() -> [[f32; 4]; 5] {
    let lo = [PAPEL.r, PAPEL.g, PAPEL.b];
    let hi = [TINTA.r, TINTA.g, TINTA.b];
    let mut m = [[0.0f32; 4]; 5];
    for (j, (l, h)) in lo.iter().zip(hi).enumerate() {
        let s = h - l;
        for (i, fila) in m.iter_mut().take(3).enumerate() {
            fila[j] = -s * GIRO[j][i];
        }
        m[4][j] = h;
    }
    m[3][3] = 1.0;
    m
}

/// Aplica la matriz a un color como lo haria Direct2D (para las pruebas y
/// para quien necesite el color exacto de un pixel ya oscurecido).
pub fn aplicar_matriz(m: &[[f32; 4]; 5], c: Color) -> Color {
    let v = [c.r, c.g, c.b, c.a];
    let col = |j: usize| (0..4).map(|i| v[i] * m[i][j]).sum::<f32>() + m[4][j];
    Color {
        r: col(0).clamp(0.0, 1.0),
        g: col(1).clamp(0.0, 1.0),
        b: col(2).clamp(0.0, 1.0),
        a: col(3).clamp(0.0, 1.0),
    }
}

/// Un color de la interfaz de un visor (barras, marcos, rotulos): tal cual
/// en claro y con la luz invertida en oscuro.
pub fn si(oscuro: bool, c: Color) -> Color {
    if oscuro { invertir_luz(c) } else { c }
}

/// Lo mismo que [`invertir_luz`] para un color de 8 bits `(r, g, b)`.
pub fn invertir_luz_u8(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let c = invertir_luz(Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    });
    let q = |x: f32| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    (q(c.r), q(c.g), q(c.b))
}

/// Lo mismo que [`legible_sobre_oscuro`] para un color de 8 bits.
pub fn legible_u8(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let c = Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    };
    if luz(c) < 0.45 {
        invertir_luz_u8(r, g, b)
    } else {
        (r, g, b)
    }
}

/// **Una raya** (un borde, un eje, una linea de un grafico) sobre el papel
/// oscuro: solo se aclara la casi negra (los bordes de una tabla, el eje
/// negro); las de color, aunque sean oscuras, son del grafico y se quedan.
pub fn raya_legible_u8(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let c = Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    };
    if luz(c) < 0.2 {
        invertir_luz_u8(r, g, b)
    } else {
        (r, g, b)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(r: f32, g: f32, b: f32) -> Color {
        Color { r, g, b, a: 1.0 }
    }

    fn cerca(a: Color, b: Color) -> bool {
        (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
    }

    #[test]
    fn el_blanco_pasa_a_papel_oscuro_y_el_negro_a_tinta_clara() {
        assert!(cerca(invertir_luz(Color::BLANCO), PAPEL));
        assert!(cerca(invertir_luz(Color::NEGRO), TINTA));
        // Ni negro puro ni blanco puro: no deslumbra.
        const { assert!(PAPEL.r > 0.05 && TINTA.r < 0.95) };
        // Un gris sigue gris (los tres canales casi iguales).
        let g = invertir_luz(c(0.4, 0.4, 0.4));
        assert!(
            (g.r - g.g).abs() < 0.02 && (g.g - g.b).abs() < 0.02,
            "{g:?}"
        );
    }

    #[test]
    fn un_color_saturado_conserva_su_tono() {
        // Rojo oscuro: sale claro y sigue rojo.
        let r = invertir_luz(c(0.86, 0.12, 0.12));
        assert!(r.r > r.g + 0.2 && r.r > r.b + 0.2, "{r:?}");
        // Azul marino: sale azul claro.
        let a = invertir_luz(c(0.0, 0.1, 0.5));
        assert!(a.b > a.r && luz(a) > 0.45, "{a:?}");
        // Caso negativo: invertir a secas dejaria el rojo turquesa.
        let ingenuo = Color::BLANCO.invertido();
        assert!(cerca(ingenuo, Color::NEGRO));
        let turquesa = c(0.86, 0.12, 0.12).invertido();
        assert!(turquesa.g > turquesa.r);
    }

    #[test]
    fn lo_que_ya_se_lee_sobre_oscuro_no_cambia() {
        // El negro automatico se aclara.
        assert!(luz(legible_sobre_oscuro(Color::NEGRO)) > 0.8);
        // Un amarillo o el blanco ya se leen: igual.
        let amarillo = c(1.0, 0.9, 0.2);
        assert!(cerca(legible_sobre_oscuro(amarillo), amarillo));
        assert!(cerca(legible_sobre_oscuro(Color::BLANCO), Color::BLANCO));
    }

    #[test]
    fn el_texto_sobre_un_relleno_explicito_se_queda_como_esta() {
        // Negro sobre un relleno amarillo: lo puso su autor y se lee.
        assert!(cerca(texto_en_oscuro(Color::NEGRO, true), Color::NEGRO));
        // El mismo negro sin relleno debajo: claro.
        assert!(luz(texto_en_oscuro(Color::NEGRO, false)) > 0.8);
    }

    #[test]
    fn la_matriz_de_direct2d_da_lo_mismo_que_la_cuenta_por_color() {
        let m = matriz_d2d();
        for col in [
            Color::BLANCO,
            Color::NEGRO,
            c(0.86, 0.12, 0.12),
            c(0.2, 0.6, 1.0),
            c(0.5, 0.5, 0.5),
        ] {
            let a = aplicar_matriz(&m, col);
            let b = invertir_luz(col);
            assert!(cerca(a, b), "{col:?}: {a:?} != {b:?}");
            // El alfa no se toca.
            assert!((a.a - 1.0).abs() < 1e-6);
        }
        assert_eq!(invertir_luz_u8(255, 255, 255), (0x1F, 0x1F, 0x22));
        assert_eq!(legible_u8(255, 255, 0), (255, 255, 0));
        // Las rayas: la negra se aclara; la azul de un grafico se queda.
        assert!(raya_legible_u8(0, 0, 0).0 > 200);
        assert_eq!(raya_legible_u8(0x44, 0x72, 0xC4), (0x44, 0x72, 0xC4));
    }
}

/// **Un Excel en oscuro, a PNG** para mirarlo: la misma hoja con sus
/// encabezados y sus pestanas en claro y en oscuro, una al lado de otra.
/// `PIXPIN_XLSX` es el libro (una copia, nunca el original) y
/// `PIXPIN_MUESTRAS` la carpeta del PNG:
/// `cargo test -p pixpin-render --lib muestra_de_excel_en_oscuro -- --ignored --nocapture`.
#[cfg(test)]
mod muestras {
    use crate::MotorRender;
    use crate::fuera_de_pantalla::FueraDePantalla;
    use crate::hoja_office::{self, CacheHoja, OpcionesHoja};
    use crate::lienzo::RectF;
    use pixpin_hojas::{Libro, Regional, Rejilla, Vista};
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    };

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

    #[test]
    #[ignore = "necesita GPU y un libro de Excel"]
    fn muestra_de_excel_en_oscuro() {
        let Some(ruta) = std::env::var_os("PIXPIN_XLSX").map(std::path::PathBuf::from) else {
            println!("sin PIXPIN_XLSX");
            return;
        };
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        // SAFETY: COM en este hilo, para WIC (las imagenes del libro).
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            )
        };
        let libro = Libro::abrir(&ruta, Regional::es_pe()).expect("abrir el libro");
        let indice = std::env::var("PIXPIN_HOJA")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(libro.hoja_activa);
        let mut hoja = libro.abrir_hoja(indice);
        hoja.cargar_todo();
        let disp = d3d();
        let motor = MotorRender::nuevo(&disp).expect("motor");
        let mut cache = CacheHoja::nueva();
        let digito = cache.ancho_digito(&motor, &libro.estilos.fuente_normal, 96.0);
        let mut rejilla = Rejilla::nueva_en(&hoja, &libro.estilos.fuente_normal, digito, 96.0);
        let (ancho, alto) = (900u32, 560u32);
        let tira = 30.0f32;
        let vista = Vista {
            ancho: ancho as f64,
            alto: alto as f64 - tira as f64,
            escala: 1.0,
            x: 0.0,
            y: 0.0,
        };
        let ultima = rejilla.fila_en(alto as f64) + 1;
        hoja_office::autoajustar_filas(&mut cache, &motor, &libro, &hoja, &mut rejilla, 0, ultima);
        let fuera = FueraDePantalla::nuevo(&motor, &disp, ancho * 2, alto).expect("superficie");
        motor
            .dibujar(&fuera.destino, |p| {
                for (k, oscuro) in [false, true].into_iter().enumerate() {
                    let x = k as f32 * ancho as f32;
                    let caja = RectF {
                        x,
                        y: 0.0,
                        ancho: ancho as f32,
                        alto: alto as f32 - tira,
                    };
                    let op = OpcionesHoja {
                        encabezados: true,
                        activa: Some((1, 1)),
                        oscuro,
                        ..Default::default()
                    };
                    p.empujar_recorte(caja);
                    hoja_office::pintar_hoja(
                        p, &mut cache, &libro, &hoja, &rejilla, &vista, caja, &op,
                    );
                    p.soltar_recorte();
                    let abajo = RectF {
                        x,
                        y: alto as f32 - tira,
                        ancho: ancho as f32,
                        alto: tira,
                    };
                    hoja_office::pintar_pestanas(p, &mut cache, &libro, indice, 0, abajo, 1.0);
                }
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("gpu");
        let (w, h, pixeles) = fuera.leer_rgba().expect("leer");
        // El papel de la mitad oscura es el de `PAPEL` y el de la clara, blanco
        // (un pixel de la ultima fila visible, lejos de los textos).
        let px = |x: u32, y: u32| {
            let i = ((y * w + x) * 4) as usize;
            (pixeles[i], pixeles[i + 1], pixeles[i + 2])
        };
        println!(
            "esquina de celdas: claro {:?} / oscuro {:?}",
            px(ancho - 5, alto - 40),
            px(2 * ancho - 5, alto - 40)
        );
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho: w,
            alto: h,
            pixeles,
        })
        .expect("png");
        let salida = carpeta.join("excel-claro-y-oscuro.png");
        std::fs::write(&salida, png).expect("guardar");
        println!("{}", salida.display());
    }
}
