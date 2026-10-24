//! **La pasada de tapado del editor**: donde el mosaico deja de ser una
//! figura y pasa a ser pixeles.
//!
//! Es deliberadamente corto. Toda la geometria —que caja, que cuadro, en que
//! orden— la decide `pixpin_motor2d::mosaico`, que es puro y se prueba sin
//! GPU; y todo el Direct2D lo hace `pixpin_render::capa_estatica::tapar`.
//! Aqui solo se juntan los dos, que es lo unico que no puede vivir en
//! ninguno de ellos.
//!
//! # Como se engancha, y por que asi
//!
//! El mosaico tapa **lo que hay debajo**, y debajo es lo que se pinto antes
//! que el. Asi que no vale con pasar al final y tapar: una flecha dibujada
//! despues, senalando el dato tapado, acabaria pixelada tambien.
//!
//! El orden del fotograma queda asi:
//!
//! 1. Se pinta la escena **saltandose los mosaicos** ([`es_mosaico`]).
//! 2. Se cierra el fotograma, porque leer lo ya pintado necesita que el
//!    destino este escrito de verdad.
//! 3. Se llama a [`pasar`] una vez, que tapa cada mosaico con lo que hay
//!    debajo de el.
//!
//! El paso 1 es el que hay que recordar: si el mosaico se pintara ademas con
//! su banda opaca de `pintado.rs`, lo que la pasada leeria seria la banda, y
//! el resultado seria un rectangulo gris uniforme —lo de antes, con mas
//! trabajo—.

use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::mosaico;
use pixpin_render::MotorRender;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// Si este elemento lo pinta la pasada de tapado y no el camino normal.
pub fn es_mosaico(e: &Elemento) -> bool {
    matches!(e.figura, Figura::Mosaico { .. })
}

/// **Tapa todos los mosaicos que se ven.**
///
/// Devuelve cuantos tapo, que es lo que una prueba de humo puede mirar sin
/// GPU. Un fallo de uno no detiene a los demas: que un mosaico no llegue a
/// taparse es malo, pero que por su culpa no se tapen los otros es peor.
pub fn pasar(
    motor: &mut MotorRender,
    destino: &ID2D1Bitmap1,
    elementos: &[Elemento],
    camara: &Camara,
    ancho_px: u32,
    alto_px: u32,
) -> usize {
    let mut tapados = 0;
    for m in mosaico::plan_en_pantalla(elementos, camara, ancho_px, alto_px) {
        match pixpin_render::capa_estatica::tapar(motor, destino, m.zona, m.lado, m.desenfoque) {
            Ok(()) => tapados += 1,
            Err(e) => tracing::warn!(?e, id = m.id, "no se pudo tapar un mosaico"),
        }
    }
    tapados
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn mosaico_en(x: f32) -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Mosaico { desenfoque: false },
            x,
            y: 0.0,
            ancho: 40.0,
            alto: 20.0,
            grosor: 2.0,
            ..Default::default()
        }
    }

    #[test]
    fn solo_los_mosaicos_se_saltan_del_pintado_normal() {
        assert!(es_mosaico(&mosaico_en(0.0)));
        assert!(!es_mosaico(&Elemento::default()), "un rectangulo se pinta");
    }

    /// El caso normal y el que mas veces ocurre: ni un mosaico. La pasada no
    /// tiene entonces nada que hacer, y sobre todo no puede inventarse una
    /// zona: tapar de mas es tapar el dibujo.
    #[test]
    fn sin_mosaicos_a_la_vista_la_pasada_no_tiene_nada_que_hacer() {
        let c = Camara::nueva();
        let vacio = |es: &[Elemento]| mosaico::plan_en_pantalla(es, &c, 800, 600).is_empty();
        assert!(vacio(&[]));
        assert!(vacio(&[Elemento::default()]), "un rectangulo no es mosaico");
        assert!(
            vacio(&[mosaico_en(10_000.0)]),
            "uno fuera de la pantalla tampoco cuenta"
        );
        assert!(!vacio(&[mosaico_en(10.0)]), "uno a la vista si");
    }

    /// **Pixelar y desenfocar, pintados de verdad** (F18): el mismo texto
    /// tapado con los dos modos del panel. Pixelado: cada cuadro de 16 (el
    /// grano del grosor medio) es de un solo color, asi que no queda el trazo
    /// de ninguna letra. Desenfocado: no queda ningun canto vivo.
    /// Se deja `mosaico-pixelar-desenfocar.png`.
    #[test]
    fn pixelar_y_desenfocar_tapan_el_texto_y_la_muestra_se_deja_en_png() {
        use crate::dibujo::pintar::{a_color, dibujar_orden, por_cada_orden};
        let texto = |y: f32| Elemento {
            id: 0,
            figura: Figura::Texto {
                texto: "CUENTA 1234 5678".into(),
                tam: 28.0,
                familia: "Segoe UI".into(),
            },
            x: 20.0,
            y,
            ancho: 300.0,
            alto: 36.0,
            trazo: pixpin_motor2d::ColorRgba::opaco(0.0, 0.0, 0.0),
            ..Default::default()
        };
        let tapa = |y: f32, desenfoque: bool| Elemento {
            id: 0,
            figura: Figura::Mosaico { desenfoque },
            x: 20.0,
            y,
            ancho: 300.0,
            alto: 40.0,
            grosor: 2.0,
            ..Default::default()
        };
        let mut escena = pixpin_motor2d::Escena::nueva();
        escena.anadir(texto(20.0));
        escena.anadir(texto(90.0));
        escena.anadir(tapa(18.0, false));
        escena.anadir(tapa(88.0, true));
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let mut motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let (w, h) = (360u32, 150u32);
        let destino =
            pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h)
                .expect("textura");
        let imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(4096);
        let camara = Camara::nueva();
        let vista = camara.ventana(w as f32, h as f32);
        motor
            .dibujar(&destino.destino, |p| {
                p.limpiar(a_color(escena.fondo));
                p.poner_vista((0.0, 0.0), 1.0, (0.0, 0.0));
                let mut cache = pixpin_motor2d::cache::Cache::nueva();
                for e in escena.visibles().filter(|e| !es_mosaico(e)) {
                    por_cada_orden(&mut cache, e, 1.0, None, |o| {
                        dibujar_orden(p, o, vista, None, &imagenes, 1.0, None);
                    });
                }
            })
            .expect("pinta");
        assert_eq!(
            pasar(
                &mut motor,
                &destino.destino,
                &escena.elementos,
                &camara,
                w,
                h
            ),
            2
        );
        let (ancho, alto, px) = destino.leer_rgba().expect("lee");
        let g = |x: u32, y: u32| px[((y * ancho + x) * 4) as usize] as i32;
        // Pixelado: en cada fila, cuantas veces cambia el color. Con cuadros
        // de 16 no puede cambiar mas de una vez cada 16 pixeles; un texto
        // cambia en cada canto de cada letra.
        let cambios = |y: u32| (22..318u32).filter(|&x| g(x + 1, y) != g(x, y)).count();
        let pix = (22..54u32).map(cambios).max().unwrap_or(0);
        let crudo = cambios(8);
        // El salto mas grande entre dos vecinos del desenfocado.
        let mut borr = 0;
        for y in 92..124u32 {
            for x in 24..316u32 {
                borr = borr.max((g(x + 1, y) - g(x, y)).abs());
            }
        }
        let dir = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&dir).expect("carpeta");
        let png = pixpin_codec::codificar_png(&pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles: px,
        })
        .expect("png");
        std::fs::write(dir.join("mosaico-pixelar-desenfocar.png"), png).expect("escribe");
        assert!(
            pix <= 300 / 16 + 2,
            "pixelado: una fila con detalle ({pix} cambios)"
        );
        assert!(borr < 60, "desenfocado: queda un canto vivo ({borr})");
        // Caso negativo: encima del mosaico (papel liso) no cambia nada, y
        // la cuenta no es trivial: el pixelado si tiene sus cuadros.
        assert_eq!(crudo, 0);
        assert!(pix > 0, "el pixelado salio liso: no tapo nada");
    }
}
