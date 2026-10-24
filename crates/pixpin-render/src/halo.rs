//! **El halo del numero de una cota, en un mapa hecho una vez.**
//!
//! El halo son 24 copias del renglon en dos anillos alrededor de la letra
//! (`Pintor::texto_con_halo`). Derecho, cada copia es barata (el atlas de
//! letras de Direct2D); pero el numero de una cota va **girado** con su raya,
//! y girado Direct2D no usa ese atlas: medido con veinte cotas en un i7
//! (`medir_el_arrastre_de_cotas_y_marcos`), 7 de los 12 ms de pintarlas eran
//! las copias del halo, fotograma a fotograma.
//!
//! Aqui las 24 copias se pintan UNA vez, derechas, en un mapa del tamano del
//! renglon a la escala a la que se ve, y cada fotograma pinta ese mapa girado
//! (un `DrawBitmap`) y las letras encima como siempre. Las letras no cambian
//! ni un pixel; el halo, solo el remuestreo de su borde.
//!
//! Se probo tambien el contorno de las letras (`GetGlyphRunOutline`) con un
//! trazo ancho de uniones redondas, que es literalmente lo que hace el movil
//! (`Paint.Style.STROKE`): teselarlo con `CreateStrokedGeometryRealization`
//! costo **200 ms por numero** en el i7. Descartado.

use windows::Win32::Graphics::Direct2D::Common::{D2D_SIZE_F, D2D_SIZE_U};
use windows::Win32::Graphics::Direct2D::{
    D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_NONE,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, ID2D1Bitmap,
};
use windows::Win32::Graphics::DirectWrite::IDWriteTextLayout;
use windows_numerics::{Matrix3x2, Vector2};

use crate::motor::{Color, MotorRender};

/// Lado maximo del mapa, en pixeles: un numero enorme a mucho aumento se
/// pinta con las copias de siempre antes que pedir una textura gigante.
const LADO_MAXIMO: f32 = 2048.0;

/// El mapa del halo y su caja en las unidades del renglon, con la esquina
/// del renglon en el origen: se pinta en `(x + caja.0, y + caja.1)` con
/// `caja.2 x caja.3` de lado.
#[derive(Clone)]
pub(crate) struct MapaDelHalo {
    pub(crate) mapa: ID2D1Bitmap,
    pub(crate) caja: (f32, f32, f32, f32),
}

/// La escala a la que se hace el mapa: la de la vista redondeada hacia
/// ARRIBA a octavos de octava, para que al pintarlo solo se reduzca (un 9 %
/// como mucho) y el mismo mapa valga mientras no se acerque o aleje. Es la
/// clave de la cache junto al grosor y el color.
pub(crate) fn nivel_de(escala: f32) -> Option<i32> {
    (escala.is_finite() && escala > 0.0)
        .then(|| (escala.log2() * 8.0).ceil().clamp(-64.0, 64.0) as i32)
}

fn escala_del_nivel(nivel: i32) -> f32 {
    2f32.powf(nivel as f32 / 8.0)
}

/// **Pinta las 24 copias en un mapa nuevo.** `ancho` y `alto` son los de la
/// disposicion; `radio`, medio grosor del halo. `None` si el mapa saldria
/// demasiado grande o Direct2D no puede; quien llama pinta entonces las
/// copias a pelo.
pub(crate) fn hacer_mapa(
    motor: &MotorRender,
    disposicion: &IDWriteTextLayout,
    ancho: f32,
    alto: f32,
    radio: f32,
    nivel: i32,
    color: Color,
) -> Option<MapaDelHalo> {
    let escala = escala_del_nivel(nivel);
    // Lo que la tinta sale de la caja del renglon (el rabo de una letra a
    // mano), mas el radio del halo y un pixel por el suavizado.
    // SAFETY: consulta de solo lectura sobre una disposicion viva.
    let sale = unsafe { disposicion.GetOverhangMetrics() }.ok()?;
    let aire = radio + 1.0 / escala + 1.0;
    let izq = aire + sale.left.max(0.0);
    let arr = aire + sale.top.max(0.0);
    let der = aire + sale.right.max(0.0);
    let abj = aire + sale.bottom.max(0.0);
    let (px_ancho, px_alto) = (
        ((ancho + izq + der) * escala).ceil(),
        ((alto + arr + abj) * escala).ceil(),
    );
    if !(1.0..=LADO_MAXIMO).contains(&px_ancho) || !(1.0..=LADO_MAXIMO).contains(&px_alto) {
        return None;
    }
    // Los DIP son `pixeles / escala`: el mapa se dibuja en unidades del
    // renglon y cada unidad cae en `escala` pixeles, como en la pantalla.
    let dip = D2D_SIZE_F {
        width: px_ancho / escala,
        height: px_alto / escala,
    };
    let px = D2D_SIZE_U {
        width: px_ancho as u32,
        height: px_alto as u32,
    };
    let pincel = motor.pincel(color)?;
    // SAFETY: el destino compatible tiene su propio BeginDraw/EndDraw y
    // comparte los recursos del contexto (la disposicion y el pincel), asi
    // que se puede usar con el fotograma del contexto abierto.
    unsafe {
        let destino = motor
            .contexto()
            .CreateCompatibleRenderTarget(
                Some(&dip),
                Some(&px),
                None,
                D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
            )
            .ok()?;
        destino.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        destino.BeginDraw();
        destino.SetTransform(&Matrix3x2::identity());
        destino.Clear(Some(
            &Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }
            .a_d2d(),
        ));
        for (n, r) in [(16, radio), (8, radio / 2.0)] {
            for i in 0..n {
                let a = i as f32 * std::f32::consts::TAU / n as f32;
                destino.DrawTextLayout(
                    Vector2 {
                        X: izq + r * a.cos(),
                        Y: arr + r * a.sin(),
                    },
                    disposicion,
                    &pincel,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }
        }
        destino.EndDraw(None, None).ok()?;
        motor.conto(|c| c.bitmaps += 1);
        Some(MapaDelHalo {
            mapa: destino.GetBitmap().ok()?,
            caja: (-izq, -arr, dip.width, dip.height),
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::time::Instant;

    #[test]
    fn el_nivel_redondea_hacia_arriba_y_no_admite_escalas_absurdas() {
        assert_eq!(nivel_de(1.0), Some(0));
        assert!(escala_del_nivel(nivel_de(0.6).unwrap()) >= 0.6);
        assert!(escala_del_nivel(nivel_de(0.6).unwrap()) < 0.6 * 1.1);
        assert_eq!(nivel_de(0.0), None);
        assert_eq!(nivel_de(-1.0), None);
        assert_eq!(nivel_de(f32::NAN), None);
    }

    /// Lo que cuesta cada forma de pintar el halo de veinte numeros girados.
    /// `cargo test --release -p pixpin-render medir_el_halo -- --ignored --nocapture`
    #[test]
    #[ignore = "necesita GPU; mide"]
    fn medir_el_halo() {
        let (motor, destino) = crate::tinta::pruebas::motor_y_destino_de_prueba(800, 600);
        let letra = crate::letras::Letra {
            familia: "Excalifont",
            negrita: false,
            cursiva: false,
            interlineado: None,
        };
        let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
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
        let pasadas = |que: &str, f: &dyn Fn(&crate::lienzo::Pintor)| {
            let t = Instant::now();
            for _ in 0..30 {
                motor
                    .dibujar(&destino, |p| {
                        p.limpiar(blanco);
                        p.poner_vista((0.0, 0.0), 0.6, (0.0, 0.0));
                        for i in 0..20 {
                            let (cx, cy) = (
                                100.0 + (i % 5) as f32 * 220.0,
                                100.0 + (i / 5) as f32 * 180.0,
                            );
                            p.girado((cx, cy), 0.3, |p| f(p));
                        }
                    })
                    .unwrap();
            }
            println!("{que}: {:.3} ms por fotograma de 20 girados", ms(t) / 30.0);
        };
        pasadas("24 copias + letra", &|p| {
            p.texto_con_halo_de_copias_para_medir(
                "3,48 cm · -17°",
                300.0,
                200.0,
                20.0,
                &letra,
                negro,
                4.4,
            )
        });
        pasadas("mapa + letra", &|p| {
            p.texto_con_halo(
                "3,48 cm · -17°",
                300.0,
                200.0,
                20.0,
                &letra,
                negro,
                blanco,
                4.4,
            )
        });
        let (d, w, h) = crate::letras::disposicion(
            motor.dwrite(),
            "3,48 cm · -17°",
            20.0,
            crate::letras::SIN_PARTIR,
            &letra,
        )
        .unwrap();
        let _ = motor.dibujar(&destino, |_| {
            let t = Instant::now();
            for _ in 0..20 {
                std::hint::black_box(hacer_mapa(&motor, &d, w, h, 2.2, 0, blanco));
            }
            println!("hacer un mapa: {:.3} ms", ms(t) / 20.0);
        });
    }
}
