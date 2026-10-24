//! **Muestras de las figuras de F12/F14**, antes y despues de arreglarlas:
//! la grafica entera y estirada, la tabla y los cajetines.
//! `cargo test -p pixpin --bin pixpinmax muestras_de_las_figuras -- --ignored
//! --nocapture`. Necesita GPU; deja los PNG en `PIXPIN_MUESTRAS`, con el
//! prefijo de `PIXPIN_MUESTRAS_PREFIJO` (por ejemplo «antes-»).

use super::*;
use pixpin_motor2d::gesto::{EventoGesto, Herramienta};

pub(super) fn carpeta() -> PathBuf {
    let c = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-lienzo")
        });
    std::fs::create_dir_all(&c).unwrap();
    c
}

pub(super) fn guardar(nombre: &str, img: &pixpin_codec::ImagenRgba) {
    let prefijo = std::env::var("PIXPIN_MUESTRAS_PREFIJO").unwrap_or_default();
    let png = pixpin_codec::imagen::codificar_png(img).unwrap();
    let ruta = carpeta().join(format!("{prefijo}{nombre}.png"));
    std::fs::write(&ruta, png).unwrap();
    println!("{nombre}: {}", ruta.display());
}

pub(super) fn foto_de(escena: &Escena) -> pixpin_codec::ImagenRgba {
    let fotos = |_: u64| None;
    let lienzo = super::super::exportar::Lienzo {
        escena,
        seleccion: &[],
        papel: None,
        fotos: &fotos,
        nombre: "muestra".into(),
    };
    let hojas =
        pixpin_motor2d::exportar::hojas(escena, pixpin_motor2d::exportar::Alcance::Todo, &[], None);
    super::super::exportar::a_imagen(&hojas[0], 1.0, Some(escena.fondo), &lienzo).unwrap()
}

/// Estira lo elegido tirando de la esquina sureste `dx, dy` pixeles, con el
/// gesto de verdad.
fn estirar(escena: &mut Escena, gesto: &mut Gesto, dx: f32, dy: f32) {
    gesto.tomar_herramienta(Herramienta::Mano);
    let (_, _, x1, y1) = gesto.seleccion.caja(escena).unwrap();
    let ev = |g: &mut Gesto, e: &mut Escena, x: f32, y: f32, fase: u8| {
        let p = Punto2::nuevo(x, y);
        let evento = match fase {
            0 => EventoGesto::Pulsar {
                p,
                shift: false,
                alt: false,
                presion: None,
            },
            1 => EventoGesto::Mover {
                p,
                shift: false,
                alt: false,
                presion: None,
            },
            _ => EventoGesto::Soltar { p },
        };
        g.evento(evento, e, 1.0);
    };
    ev(gesto, escena, x1, y1, 0);
    ev(gesto, escena, x1 + dx / 2.0, y1 + dy / 2.0, 1);
    ev(gesto, escena, x1 + dx, y1 + dy, 1);
    ev(gesto, escena, x1 + dx, y1 + dy, 2);
}

fn peticion() -> Peticion {
    Peticion {
        formulas: vec!["sin(x)".into(), "x^2/4 - 1".into()],
        x_desde: -5.0,
        x_hasta: 5.0,
        y_desde: -3.0,
        y_hasta: 3.0,
        escala: 40.0,
    }
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestras_de_las_figuras() {
    let d = pixpin_capture::Dispositivo::nuevo().unwrap();
    let mut motor = MotorRender::nuevo(d.d3d()).unwrap();
    pixpin_motor2d::texto::instalar_medidor(crate::dibujo::pintar::medir_para_el_motor);
    let gesto0 = Gesto::nuevo();
    let estilo = estilo_del_pincel(&gesto0);

    // 1. La grafica, entera y estirada por la esquina el doble de ancha.
    {
        let medir = medidor(&motor, &estilo.familia);
        let v = grafica::elementos(&peticion(), &estilo, &medir).unwrap();
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        estampar_en_la_vista(
            &mut escena,
            &mut gesto,
            Punto2::nuevo(300.0, 200.0),
            "grafica",
            &v,
        );
        guardar("grafica", &foto_de(&escena));
        estirar(&mut escena, &mut gesto, 300.0, 60.0);
        guardar("grafica-estirada", &foto_de(&escena));
    }

    // 2. Las tablas: la pegada de Excel y la en blanco, y la pegada estirada.
    {
        let medir = medidor(&motor, &estilo.familia);
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        pegar_tabla(
            None,
            "Material\tCantidad\tPrecio\nCemento\t12\t8,50\nArena fina\t3\t22,00\nLadrillo\t1200\t0,35\n",
            &mut escena,
            &mut gesto,
            Punto2::nuevo(200.0, 100.0),
            &medir,
        );
        let fb = formulario_de_tabla(
            &Catalogo::nuevo(pixpin_store::Idioma::Espanol),
            &vec![vec![String::new(); 3]; 4],
            true,
            "tabla-blanco-titulo",
            "tabla-insertar",
        );
        let v = tabla_de_celdas(&fb, &estilo, &medir);
        let mut g2 = Gesto::nuevo();
        estampar_en_la_vista(
            &mut escena,
            &mut g2,
            Punto2::nuevo(200.0, 320.0),
            "tabla",
            &v,
        );
        guardar("tablas", &foto_de(&escena));
        estirar(&mut escena, &mut gesto, 200.0, 0.0);
        guardar("tabla-estirada", &foto_de(&escena));
    }

    // 3. Los cajetines, a 100 %, tal como se pintan encima del lienzo.
    let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
    let mut f = formulario_de_grafica(&t);
    f.campos[0].texto = "sin(x)\nx^2/4 - 1".into();
    cajetin_a_png(&mut motor, &d, "cajetin-grafica", &mut f);
    let filas = vec![
        vec!["Material".to_string(), "Cantidad".into(), "Precio".into()],
        vec!["Cemento".into(), "12".into(), "8,50".into()],
        vec!["Arena fina".into(), "3".into(), "22,00".into()],
    ];
    let mut tb = formulario_de_tabla(&t, &filas, true, "tabla-pegada-titulo", "tabla-insertar");
    tb.activo = 4;
    cajetin_a_png(&mut motor, &d, "cajetin-tabla", &mut tb);
}

/// Pinta el cajetin `f` sobre un fondo gris claro, como si fuera el lienzo.
pub(super) fn cajetin_a_png(
    motor: &mut MotorRender,
    d: &pixpin_capture::Dispositivo,
    nombre: &str,
    f: &mut Formulario,
) {
    let (w, h) = (1100u32, 800u32);
    let previa = vista_previa(
        f,
        &medidor(motor, pixpin_motor2d::texto::FAMILIA_DEL_SISTEMA),
        100,
    );
    let destino =
        pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(motor, d.d3d(), w, h).unwrap();
    let sin_imagenes = ImagenesLienzo::nuevo(1);
    motor
        .dibujar(&destino.destino, |p| {
            p.limpiar(Color {
                r: 0.93,
                g: 0.93,
                b: 0.93,
                a: 1.0,
            });
            dibujar(
                p,
                (0.0, 0.0),
                w as f32,
                h as f32,
                100,
                f,
                &previa,
                &sin_imagenes,
                true,
            );
        })
        .unwrap();
    let (ancho, alto, pixeles) = destino.leer_rgba().unwrap();
    guardar(
        nombre,
        &pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles,
        },
    );
}
