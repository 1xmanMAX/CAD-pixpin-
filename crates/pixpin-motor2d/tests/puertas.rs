//! Las puertas de rendimiento de S3-A (§6 de la spec).
//!
//! Se ejecutan en release, que es donde vive el usuario. En depuracion el
//! coma flotante de Rust va varias veces mas lento y los numeros no dicen
//! nada, asi que los topes se relajan segun el perfil: lo que se comprueba
//! siempre es que el orden de magnitud es el correcto, no que la maquina de
//! CI sea rapida.

use std::time::Instant;

use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::{Escena, Punto2, ordenes, ordenes_de_escena};

/// Los topes de la spec valen para release; en depuracion se multiplican.
const FACTOR: u32 = if cfg!(debug_assertions) { 20 } else { 1 };

fn trazo_largo(n: usize) -> Vec<Punto2> {
    (0..n)
        .map(|i| {
            let t = i as f32 * 0.1;
            Punto2::nuevo(t * 8.0, 300.0 + (t.sin() * 120.0))
        })
        .collect()
}

fn elemento(i: u64) -> Elemento {
    Elemento {
        id: i,
        figura: Figura::Rectangulo,
        x: (i % 40) as f32 * 30.0,
        y: (i / 40) as f32 * 30.0,
        ancho: 120.0,
        alto: 80.0,
        angulo: 0.0,
        trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
        estilo_relleno: Default::default(),
        relleno: None,
        grosor: 2.0,
        estilo: EstiloTrazo::Solido,
        rugosidad: 1.0,
        opacidad: 1.0,
        semilla: (i as u32 * 7919) | 1,
        version: 0,
        borrado: false,
        grupos: Vec::new(),
        bloqueado: false,
        enlace: None,
        redondo: false,
        material: Default::default(),
        extras: Default::default(),
    }
}

#[test]
fn un_trazo_de_cinco_mil_puntos_se_calcula_en_menos_de_dos_milisegundos() {
    // Spec E1 §4: mientras se dibuja se recalcula el trazo entero en cada
    // fotograma (como Excalidraw). Si esto se pone rojo, el arreglo es
    // recalcular solo la cola, no subir el tope.
    let trazo = trazo_largo(5_000);
    let o = Some(pixpin_motor2d::tinta::OpcionesTinta::default());
    // Calentar: la primera vuelta paga paginas y cache del procesador.
    let _ = pixpin_motor2d::tinta::contorno_de_lapiz(&trazo, &[], 1.0, o);
    let mut mejor = std::time::Duration::MAX;
    for _ in 0..5 {
        let t = Instant::now();
        let c = pixpin_motor2d::tinta::contorno_de_lapiz(&trazo, &[], 1.0, o);
        mejor = mejor.min(t.elapsed());
        assert!(!c.is_empty());
    }
    assert!(
        mejor.as_micros() < 2_000 * FACTOR as u128,
        "5.000 puntos en {mejor:?}"
    );
    println!("trazo de 5.000 puntos: {mejor:?}");
}

#[test]
fn un_trazo_de_500_puntos_se_convierte_en_contorno_en_menos_de_2_ms() {
    let puntos = trazo_largo(500);
    let contorno =
        || pixpin_motor2d::tinta::contorno_de_lapiz(&puntos, &[], 1.0, Some(Default::default()));
    // Una pasada en frio para que las paginas de memoria ya esten tocadas:
    // se mide el algoritmo, no el primer fallo de pagina.
    let _ = contorno();

    let t = Instant::now();
    let repeticiones = 20;
    for _ in 0..repeticiones {
        let p = contorno();
        assert!(!p.is_empty());
    }
    let micros = t.elapsed().as_micros() / repeticiones;
    let tope = 2_000 * FACTOR as u128;
    assert!(
        micros <= tope,
        "el trazo de 500 puntos tarda {micros} us y el tope es {tope}"
    );
    println!("trazo de 500 puntos: {micros} us");
}

#[test]
fn mil_elementos_ocupan_menos_de_20_mb() {
    // Medida por estructura, no por asignador: lo que se quiere acotar es el
    // tamano del modelo, que es lo que crece con el documento.
    let mut e = Escena::nueva();
    for i in 0..1000 {
        e.anadir(elemento(i));
    }
    let por_elemento = size_of::<Elemento>();
    let total = por_elemento * e.elementos.len();
    assert!(
        total < 20 * 1024 * 1024,
        "mil elementos ocupan {total} bytes ({por_elemento} cada uno)"
    );
    println!("mil elementos: {total} bytes, {por_elemento} por elemento");
}

#[test]
fn generar_las_ordenes_de_100_elementos_cabe_en_un_fotograma() {
    // El caso real de un dibujo cargado: 100 figuras a mano. El cache que
    // pide la spec (D43) evita rehacerlo cada fotograma, pero incluso SIN
    // cache tiene que caber, porque es lo que pasa al cargar el documento.
    let mut e = Escena::nueva();
    for i in 0..100 {
        e.anadir(elemento(i));
    }
    let _ = ordenes_de_escena(&e);

    let t = Instant::now();
    let ordenes = ordenes_de_escena(&e);
    let micros = t.elapsed().as_micros();
    assert!(!ordenes.is_empty());
    // Un fotograma a 60 Hz son 16 666 us.
    let tope = 16_666 * FACTOR as u128;
    assert!(
        micros <= tope,
        "100 elementos tardan {micros} us y el tope es {tope}"
    );
    println!("100 elementos: {micros} us, {} ordenes", ordenes.len());
}

#[test]
fn abrir_un_dibujo_de_mil_elementos_tarda_menos_de_150_ms() {
    let dir = std::env::temp_dir().join("pixpin-motor2d-puerta-abrir");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("grande.pixpin2d");

    let mut e = Escena::nueva();
    for i in 0..1000 {
        e.anadir(elemento(i));
    }
    pixpin_motor2d::guardar(&ruta, &e).unwrap();

    let t = Instant::now();
    let leida = pixpin_motor2d::cargar(&ruta).unwrap();
    let ms = t.elapsed().as_millis();

    assert_eq!(leida.elementos.len(), 1000);
    let tope = 150 * FACTOR as u128;
    assert!(ms <= tope, "abrir mil elementos tarda {ms} ms, tope {tope}");
    println!("abrir mil elementos: {ms} ms");
}

#[test]
fn el_dibujo_es_identico_al_reabrirlo() {
    // La puerta que de verdad importa: no es velocidad, es que el documento
    // del usuario se vea igual manana. Se guarda, se lee y se comparan las
    // ordenes de dibujo punto por punto.
    let dir = std::env::temp_dir().join("pixpin-motor2d-puerta-identico");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("d.pixpin2d");

    let mut e = Escena::nueva();
    for i in 0..30 {
        e.anadir(elemento(i));
    }
    e.anadir(Elemento {
        figura: Figura::Lapiz {
            puntos: trazo_largo(80),
            presiones: vec![],
            opciones: None,
        },
        ..elemento(99)
    });

    let antes = ordenes_de_escena(&e);
    pixpin_motor2d::guardar(&ruta, &e).unwrap();
    let despues = ordenes_de_escena(&pixpin_motor2d::cargar(&ruta).unwrap());

    assert_eq!(
        antes, despues,
        "el dibujo cambia al reabrirlo: la semilla no esta haciendo su trabajo"
    );
}

/// Un dibujo con la forma del que el usuario trajo de su movil: 194
/// elementos, 8.003 puntos, extendido sobre 1013 x 1920.
///
/// No es un numero inventado para que la prueba salga bien; es lo que se
/// midio de `Proyecto 2.pixpin`, y por eso es la carga con la que tiene que
/// ir fluido.
fn dibujo_como_el_del_movil() -> Escena {
    let mut escena = Escena::nueva();
    for i in 0..194u64 {
        let fila = (i / 14) as f32;
        let col = (i % 14) as f32;
        let base = Punto2::nuevo(165.0 + col * 72.0, 228.0 + fila * 137.0);
        let puntos: Vec<Punto2> = (0..41)
            .map(|k| {
                let t = k as f32 * 0.25;
                Punto2::nuevo(base.x + t * 6.0, base.y + t.sin() * 18.0)
            })
            .collect();
        let mut e = elemento(i);
        e.x = base.x;
        e.y = base.y;
        e.figura = Figura::Lapiz {
            puntos,
            presiones: Vec::new(),
            opciones: None,
        };
        escena.anadir(e);
    }
    escena
}

/// Cuantos puntos de geometria hay que fabricar para pintar esto.
///
/// Es la medida que importa de verdad: cada punto se calcula en el procesador
/// antes de que el GPU vea nada, y es lo unico que crece con el dibujo.
fn puntos_de(v: &[pixpin_motor2d::Orden]) -> usize {
    v.iter()
        .map(|o| match o {
            pixpin_motor2d::Orden::Poligono { puntos, .. }
            | pixpin_motor2d::Orden::Tinta {
                contorno: puntos, ..
            }
            | pixpin_motor2d::Orden::Polilinea { puntos, .. }
            | pixpin_motor2d::Orden::Relleno { puntos, .. } => puntos.len(),
            _ => 0,
        })
        .sum()
}

#[test]
fn de_lejos_nunca_cuesta_mas_que_el_dibujo_entero() {
    // D115: la tinta solo se sustituye por debajo de 2 px. Esta puerta ya
    // no promete una decima parte; promete que alejarse no fabrica MAS
    // geometria, y que al 5 % algo se sigue viendo.
    let escena = dibujo_como_el_del_movil();
    let entero = puntos_de(&ordenes_de_escena(&escena));
    let c = pixpin_motor2d::Camara {
        x: 100.0,
        y: 150.0,
        zoom: 0.05,
    };
    let lejos = puntos_de(&pixpin_motor2d::ordenes_de_escena_vista(
        &escena, &c, 1920.0, 1080.0,
    ));
    assert!(lejos > 0, "al 5 % no se pinto nada");
    assert!(
        lejos <= entero,
        "de lejos no puede costar mas que entero: {lejos} > {entero}"
    );
}

#[test]
fn de_cerca_se_dibuja_la_tinta_entera() {
    // La otra mitad del trato, y la que importa mas: a tamano natural no se
    // puede notar nada. Un trazo grande tiene que seguir saliendo como tinta
    // —un poligono relleno, con su afilado y sus tapas— y no como una raya.
    let escena = dibujo_como_el_del_movil();
    let e = escena.visibles().next().expect("el dibujo tiene elementos");
    let cerca = pixpin_motor2d::ordenes_a_distancia(e, 4.0);
    assert_eq!(
        cerca,
        ordenes(e),
        "de cerca el trazo tiene que salir identico a como se guardo"
    );
    assert!(
        matches!(cerca.first(), Some(pixpin_motor2d::Orden::Tinta { .. })),
        "de cerca un lapiz es tinta, no una linea: {:?}",
        cerca.first()
    );
}

#[test]
fn solo_lo_que_mide_menos_de_dos_pixeles_en_pantalla_se_dibuja_como_raya() {
    let escena = dibujo_como_el_del_movil();
    let e = escena.visibles().next().expect("el dibujo tiene elementos");
    let (x0, y0, x1, y1) = e.caja();
    let lado = (x1 - x0).max(y1 - y0);
    assert!(matches!(
        pixpin_motor2d::ordenes_a_distancia(e, 1.5 / lado).first(),
        Some(pixpin_motor2d::Orden::Polilinea { .. })
    ));
    assert!(matches!(
        pixpin_motor2d::ordenes_a_distancia(e, 10.0 / lado).first(),
        Some(pixpin_motor2d::Orden::Tinta { .. })
    ));
}

#[test]
fn lo_que_esta_fuera_de_la_pantalla_ni_se_calcula() {
    // El recorte es lo que hace que un lienzo infinito no se vuelva mas lento
    // cuanto mas se dibuja: mirando a un rincon vacio no cuesta nada, tenga
    // el dibujo lo que tenga.
    let escena = dibujo_como_el_del_movil();
    let vacio = pixpin_motor2d::Camara {
        x: 900_000.0,
        y: 900_000.0,
        zoom: 1.0,
    };
    let ahora = Instant::now();
    let salida = pixpin_motor2d::ordenes_de_escena_vista(&escena, &vacio, 1920.0, 1080.0);
    let tardo = ahora.elapsed();
    assert!(
        salida.is_empty(),
        "pinto {} cosas de un sitio vacio",
        salida.len()
    );
    assert!(
        tardo.as_millis() < (2 * FACTOR) as u128,
        "mirar a un sitio vacio tardo {tardo:?}"
    );
}

#[test]
fn sin_aumento_no_se_simplifica_nada() {
    // Caso negativo: exportar a un fichero pide el dibujo entero, y pedirlo
    // con un cero no puede devolver una version simplificada en silencio.
    let escena = dibujo_como_el_del_movil();
    let e = escena.visibles().next().expect("el dibujo tiene elementos");
    assert_eq!(pixpin_motor2d::ordenes_a_distancia(e, 0.0), ordenes(e));
    assert_eq!(pixpin_motor2d::ordenes_a_distancia(e, -1.0), ordenes(e));
}

/// Mover una seleccion grande: lo que cuesta CADA aviso del raton mientras
/// se arrastra, sin contar el pintado. Excalidraw busca por id en un mapa;
/// aqui la seleccion y la escena eran listas, y mover mil de dos mil
/// recorria las dos por cada elemento en cada movimiento.
#[test]
fn arrastrar_mil_seleccionados_de_dos_mil_cuesta_menos_de_cuatrocientos_microsegundos_por_aviso() {
    use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
    let mut escena = Escena::nueva();
    let ids: Vec<u64> = (0..2_000).map(|i| escena.anadir(elemento(i))).collect();
    let mut g = Gesto::nuevo();
    g.herramienta = Herramienta::Mano;
    g.seleccion.poner_todos(ids.iter().copied().step_by(2));
    // Se pulsa sobre el borde de un elegido (el de 0,0): un rectangulo sin
    // relleno solo se coge por el borde.
    g.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(0.0, 40.0),
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );
    assert!(
        g.marquesina().is_none() && !g.en_reposo(),
        "la pulsacion tiene que coger la seleccion"
    );
    let avisos = 200;
    let ahora = Instant::now();
    for i in 0..avisos {
        g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(10.0 + i as f32, 10.0 + i as f32),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
    }
    let por_aviso = ahora.elapsed() / avisos;
    eprintln!("mover 1000 de 2000: {por_aviso:?} por aviso");
    assert!(
        por_aviso.as_micros() < (400 * FACTOR) as u128,
        "cada aviso del raton tardo {por_aviso:?}"
    );
}

/// Pasar el raton por encima con la seleccion puesta, sin pulsar: en cada
/// aviso se mira que hay debajo para poner el cursor. Es lo que mas avisos
/// recibe de todo el editor.
#[test]
fn pasar_el_raton_sobre_dos_mil_elementos_cuesta_menos_de_cien_microsegundos_por_aviso() {
    use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
    let mut escena = Escena::nueva();
    for i in 0..2_000 {
        escena.anadir(elemento(i));
    }
    let mut g = Gesto::nuevo();
    g.herramienta = Herramienta::Mano;
    let avisos = 400;
    let ahora = Instant::now();
    for i in 0..avisos {
        let t = i as f32;
        g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(5.0 + t * 3.0, 7.0 + t * 1.5),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
    }
    let por_aviso = ahora.elapsed() / avisos;
    eprintln!("pasar el raton sobre 2000: {por_aviso:?} por aviso");
    assert!(
        por_aviso.as_micros() < (100 * FACTOR) as u128,
        "cada aviso del raton tardo {por_aviso:?}"
    );
}

/// Lo mismo, con **cien flechas atadas** a los elegidos y fuera de la
/// seleccion: cada aviso las re-traza (siguen a su caja en vivo, como en
/// Excalidraw). Las flechas se buscan UNA vez al pulsar y se apuntan por su
/// posicion; si se buscaran en cada aviso, el coste creceria con elegidos por
/// flechas y esto no cabria.
#[test]
fn arrastrar_mil_de_dos_mil_con_cien_flechas_atadas_cuesta_menos_de_seiscientos_microsegundos_por_aviso()
 {
    use pixpin_motor2d::elemento::{Enganche, Extras};
    use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
    let mut escena = Escena::nueva();
    let ids: Vec<u64> = (0..2_000).map(|i| escena.anadir(elemento(i))).collect();
    // Cada flecha sale del vacio y acaba atada a un elegido distinto.
    for k in 0..100usize {
        let objetivo = ids[k * 20];
        let caja = escena.buscar(objetivo).unwrap().clone();
        let fin = Punto2::nuevo(caja.x - 5.0, caja.y + caja.alto / 2.0);
        let inicio = Punto2::nuevo(fin.x - 300.0, fin.y);
        escena.anadir(Elemento {
            figura: Figura::Flecha {
                puntos: vec![inicio, fin],
                punta_inicio: pixpin_motor2d::TipoPunta::Ninguna,
                punta_fin: pixpin_motor2d::TipoPunta::Flecha,
                codos: false,
            },
            x: inicio.x,
            y: inicio.y,
            extras: Extras {
                enganche_fin: Some(Enganche {
                    elemento: pixpin_motor2d::enlace::id_de_texto(objetivo),
                    foco: 0.0,
                    hueco: 6.0,
                    punto_fijo: None,
                    modo: Default::default(),
                }),
                ..Default::default()
            },
            ..elemento(0)
        });
    }
    let mut g = Gesto::nuevo();
    g.herramienta = Herramienta::Mano;
    g.seleccion.poner_todos(ids.iter().copied().step_by(2));
    g.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(0.0, 40.0),
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );
    assert_eq!(
        g.flechas_que_siguen().len(),
        100,
        "no se apuntaron las cien flechas"
    );
    let avisos = 200;
    let ahora = Instant::now();
    for i in 0..avisos {
        g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(10.0 + i as f32, 10.0 + i as f32),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
    }
    let por_aviso = ahora.elapsed() / avisos;
    eprintln!("mover 1000 de 2000 con 100 flechas atadas: {por_aviso:?} por aviso");
    // El tope es el del arrastre sin flechas (400) mas dos microsegundos por
    // flecha: re-trazar una cuesta ~1,4 µs medidos (cortar su rayo con el
    // borde de la caja). Lo que se vigila es que crezca con las FLECHAS y no
    // con la escena ni con lo elegido.
    assert!(
        por_aviso.as_micros() < (600 * FACTOR) as u128,
        "cada aviso del raton tardo {por_aviso:?}"
    );
}
