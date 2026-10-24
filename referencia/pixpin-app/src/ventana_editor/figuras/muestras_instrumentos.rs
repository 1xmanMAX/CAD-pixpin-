//! **Muestras del instrumento de graficar y de la tabla de coordenadas**
//! (Tanda 4): la recta, el plano y el espacio con una serie tecleada encima,
//! el plano estirado por un lado y por una esquina, el espacio girado, la
//! pantalla con el eje y el cajetin de la tabla.
//! `cargo test -p pixpin --bin pixpinmax muestras_de_los_instrumentos --
//! --ignored --nocapture`. Necesita GPU; deja los PNG en `PIXPIN_MUESTRAS`.

use super::muestras_figuras::{cajetin_a_png, foto_de, guardar};
use super::*;
use pixpin_motor2d::instrumento::tabla::{PuntoDeTabla, TablaDeCoordenadas};

fn instrumento(ejes: u8, x: f32, y: f32) -> Elemento {
    let mut e = pixpin_motor2d::instrumento::elemento_de_fabrica(
        ejes,
        ColorRgba::opaco(0.12, 0.12, 0.12),
        2.0,
        1.0,
    );
    e.x = x;
    e.y = y;
    e
}

/// La escena de muestra: los tres instrumentos y dos series, con la escala
/// puesta para que una unidad de la tabla caiga en un cuadro del plano.
fn escena_de_muestra() -> Escena {
    let mut escena = Escena::nueva();
    escena.anadir(instrumento(1, 20.0, 20.0));
    escena.anadir(instrumento(2, 20.0, 100.0));
    let mut esp = instrumento(3, 380.0, 100.0);
    if let pixpin_motor2d::Figura::Instrumento(i) = &mut esp.figura {
        i.azimut = Some(35.0);
        i.elevacion = Some(20.0);
    }
    escena.anadir(esp);
    escena.escala = pixpin_motor2d::Escala::calibrando(40.0, 1.0, "u", 2);
    let serie = |id: &str, color: &str, puntos: &[(f64, f64)]| TablaDeCoordenadas {
        id: id.into(),
        nombre: String::new(),
        color: color.into(),
        puntos: puntos.iter().map(|&(x, y)| PuntoDeTabla { x, y }).collect(),
        visible: true,
    };
    escena.tablas = vec![
        serie(
            "a",
            "#e03131",
            &[(-3.0, -2.0), (-1.0, 1.0), (1.0, 2.0), (3.0, -1.5)],
        ),
        serie("b", "#1971c2", &[(0.0, 3.0), (2.0, 0.5)]),
    ];
    // El eje comun, en el origen del plano.
    escena.origen_coordenadas = Some(Punto2::nuevo(180.0, 260.0));
    escena
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestras_de_los_instrumentos() {
    let d = pixpin_capture::Dispositivo::nuevo().unwrap();
    let mut motor = MotorRender::nuevo(d.d3d()).unwrap();
    pixpin_motor2d::texto::instalar_medidor(crate::dibujo::pintar::medir_para_el_motor);

    // 1. Los tres y las series, como salen al exportar.
    let escena = escena_de_muestra();
    guardar("instrumentos", &foto_de(&escena));

    // 2. El plano estirado por el lado este (mas numeros) y por la esquina
    //    sureste (los mismos, mas grandes), y el espacio girado.
    let mut lado = escena_de_muestra();
    let id = lado.elementos[1].id;
    let e = lado.buscar_mut(id).unwrap();
    pixpin_motor2d::transformar::escalar(
        e,
        pixpin_geom::Tirador::EsteBorde,
        Punto2::nuevo(500.0, 260.0),
        false,
        false,
    );
    lado.tablas.clear();
    lado.elementos.retain(|e| e.id == id);
    guardar("instrumento-plano-por-un-lado", &foto_de(&lado));
    let mut esquina = escena_de_muestra();
    let e = esquina.buscar_mut(id).unwrap();
    pixpin_motor2d::transformar::escalar(
        e,
        pixpin_geom::Tirador::SuresteEsquina,
        Punto2::nuevo(500.0, 580.0),
        false,
        false,
    );
    esquina.tablas.clear();
    esquina.elementos.retain(|e| e.id == id);
    guardar("instrumento-plano-por-una-esquina", &foto_de(&esquina));
    let mut girado = escena_de_muestra();
    let id3 = girado.elementos[2].id;
    let e = girado.buscar_mut(id3).unwrap();
    pixpin_motor2d::transformar::girar_el_espacio(e, Punto2::nuevo(640.0, 260.0), false);
    girado.tablas.clear();
    girado.elementos.retain(|e| e.id == id3);
    guardar("instrumento-espacio-girado", &foto_de(&girado));

    // 3. La pantalla: lo dibujado y, encima, el eje a trazos y los puntos
    //    del tamano de pantalla, con las mismas funciones que el lienzo.
    let (w, h) = (740u32, 460u32);
    let destino =
        pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h).unwrap();
    let sin_imagenes = ImagenesLienzo::nuevo(1);
    let vista = (0.0, 0.0, w as f32, h as f32);
    motor
        .dibujar(&destino.destino, |p| {
            p.limpiar(Color::BLANCO);
            for e in escena.a_la_vista() {
                for o in pixpin_motor2d::pintado::ordenes(e) {
                    dibujar_orden(p, &o, vista, None, &sin_imagenes, 1.0, None);
                }
            }
            for o in crate::dibujo::pintar::ordenes_de_las_tablas(&escena, 1.0, Some(vista)) {
                dibujar_orden(p, &o, vista, None, &sin_imagenes, 1.0, None);
            }
        })
        .unwrap();
    let (ancho, alto, pixeles) = destino.leer_rgba().unwrap();
    guardar(
        "instrumentos-en-pantalla",
        &pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles,
        },
    );

    // 4. El cajetin de la serie roja.
    let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
    let mut f = formulario_de_coordenadas(&t, &escena.tablas[0], 0, "u");
    f.activo = 2;
    cajetin_a_png(&mut motor, &d, "cajetin-coordenadas", &mut f);
}
