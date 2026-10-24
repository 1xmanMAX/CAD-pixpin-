//! Cuantas veces pide memoria el camino caliente.
//!
//! El documento de rendimiento pone «asignaciones en el camino caliente: 0»
//! en su tabla de presupuesto, y hasta ahora no habia nada que lo midiera.
//!
//! El camino caliente es uno solo: **mover el raton mientras se dibuja**.
//! Tiene el unico plazo sagrado del editor —un fotograma del refresco real,
//! 16 ms a 60 Hz— y es donde una asignacion se nota, porque el asignador
//! puede irse al sistema operativo en el peor momento.
//!
//! Esto vive en `tests/` y no en `src/` porque un `#[global_allocator]`
//! afecta a todo el binario: no queremos uno en el programa de verdad.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static VECES: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// **Solo cuenta el hilo que mide** (24-oct-2026): con un contador para
    /// todo el proceso, lo que pedia a la vez otro hilo —otra prueba en
    /// paralelo, o el propio arnes de pruebas escribiendo su salida— caia en
    /// la cuenta y la prueba fallaba a veces sin que el camino caliente
    /// asignara nada. Constante y sin destructor: leerlo no asigna.
    static CONTANDO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Si este hilo esta midiendo. Fuera de un hilo vivo (al morir) no cuenta.
fn contando_aqui() -> bool {
    CONTANDO.try_with(std::cell::Cell::get).unwrap_or(false)
}

struct Contador;

// SAFETY: se delega todo en `System`, que cumple el contrato de
// `GlobalAlloc`. Lo unico anadido es un contador atomico, que no toca la
// memoria devuelta ni cambia el puntero.
unsafe impl GlobalAlloc for Contador {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if contando_aqui() {
            VECES.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: mismo `Layout` que nos han dado.
        unsafe { System.alloc(l) }
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: el puntero y el layout vienen de nuestro `alloc`.
        unsafe { System.dealloc(p, l) }
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, nuevo: usize) -> *mut u8 {
        // Reasignar tambien cuenta: es lo que hace un `Vec` al crecer, y es
        // justo lo que esta prueba viene a cazar.
        if contando_aqui() {
            VECES.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: el puntero y el layout vienen de nuestro `alloc`.
        unsafe { System.realloc(p, l, nuevo) }
    }
}

#[global_allocator]
static ASIGNADOR: Contador = Contador;

/// Cuenta las asignaciones que hace `f` en este hilo. Una medida a la vez
/// (`VECES` es una), asi que las pruebas que miden van en fila: [`EN_FILA`].
fn contando<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let _fila = EN_FILA.lock().unwrap_or_else(|e| e.into_inner());
    VECES.store(0, Ordering::Relaxed);
    CONTANDO.with(|c| c.set(true));
    let r = f();
    CONTANDO.with(|c| c.set(false));
    (r, VECES.load(Ordering::Relaxed))
}

/// Las medidas, de una en una aunque las pruebas corran en paralelo.
static EN_FILA: std::sync::Mutex<()> = std::sync::Mutex::new(());

use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::vector::Punto2;

#[test]
fn mover_el_raton_dibujando_no_asigna_memoria() {
    // La puerta que faltaba. Si esto falla, hay un `Vec` creciendo o un
    // `to_vec()` colado en el camino caliente.
    let mut escena = Escena::nueva();
    let mut gesto = Gesto::nuevo();
    gesto.herramienta = Herramienta::Lapiz;

    // Empezar el trazo SI asigna: crea el elemento y reserva sus puntos.
    // Eso pasa una vez por trazo, no una vez por aviso del raton.
    gesto.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(0.0, 0.0),
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );

    let (_, veces) = contando(|| {
        for i in 1..400 {
            gesto.evento(
                EventoGesto::Mover {
                    p: Punto2::nuevo(i as f32, (i % 7) as f32),
                    shift: false,
                    alt: false,
                    presion: None,
                },
                &mut escena,
                1.0,
            );
        }
    });

    assert_eq!(veces, 0, "el camino caliente asigno {veces} veces");
}

#[test]
fn mover_el_raton_dibujando_con_presion_tampoco_asigna_memoria() {
    // Igual que la prueba de arriba, pero con un lapiz de verdad mandando
    // presion en cada muestra: `anadir_a_lapiz` tambien tiene que vivir sin
    // pedir memoria en el camino caliente.
    let mut escena = Escena::nueva();
    let mut gesto = Gesto::nuevo();
    gesto.herramienta = Herramienta::Lapiz;

    gesto.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(0.0, 0.0),
            shift: false,
            alt: false,
            presion: Some(0.5),
        },
        &mut escena,
        1.0,
    );

    let (_, veces) = contando(|| {
        for i in 1..400 {
            gesto.evento(
                EventoGesto::Mover {
                    p: Punto2::nuevo(i as f32, (i % 7) as f32),
                    shift: false,
                    alt: false,
                    presion: Some(0.5),
                },
                &mut escena,
                1.0,
            );
        }
    });

    assert_eq!(
        veces, 0,
        "el camino caliente con presion asigno {veces} veces"
    );
}

#[test]
fn arrastrar_una_seleccion_tampoco_asigna() {
    // Arrastrar es tan camino caliente como dibujar, y es donde un
    // `.to_vec()` sobre los ids de la seleccion se cuela con mas facilidad:
    // parece necesario por el prestamo y no lo es.
    use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};

    let mut escena = Escena::nueva();
    let mut ids = Vec::new();
    for i in 0..20 {
        ids.push(escena.anadir(Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x: i as f32 * 5.0,
            y: 0.0,
            ancho: 40.0,
            alto: 40.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }));
    }

    let mut gesto = Gesto::nuevo();
    gesto.herramienta = Herramienta::Mano;
    gesto.seleccion.poner_todos(ids);

    // Pulsar SI asigna: abre el paso y guarda una instantanea por elemento.
    // Eso pasa una vez por gesto.
    gesto.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(10.0, 10.0),
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );

    let (_, veces) = contando(|| {
        for i in 1..400 {
            gesto.evento(
                EventoGesto::Mover {
                    p: Punto2::nuevo(10.0 + i as f32, 10.0),
                    shift: false,
                    alt: false,
                    presion: None,
                },
                &mut escena,
                1.0,
            );
        }
    });

    assert_eq!(veces, 0, "arrastrar asigno {veces} veces");
}

#[test]
fn mover_el_raton_en_reposo_tampoco_asigna() {
    // Pasar el raton por encima calcula el cursor, que consulta tiradores y
    // picado. Es casi tan frecuente como dibujar.
    let mut escena = Escena::nueva();
    let mut gesto = Gesto::nuevo();
    gesto.herramienta = Herramienta::Mano;

    let (_, veces) = contando(|| {
        for i in 0..400 {
            gesto.evento(
                EventoGesto::Mover {
                    p: Punto2::nuevo(i as f32, 0.0),
                    shift: false,
                    alt: false,
                    presion: None,
                },
                &mut escena,
                1.0,
            );
        }
    });

    assert_eq!(veces, 0, "pasar el raton asigno {veces} veces");
}

#[test]
fn encuadrar_sesenta_fotogramas_solo_calcula_la_geometria_una_vez() {
    // No es una prueba de tiempo —eso depende de la maquina— sino de
    // comportamiento: encuadrar no cambia el aumento, asi que no debe
    // invalidar nada. Es lo que hace que arrastrar el lienzo con ocho mil
    // elementos no cueste nada.
    use pixpin_motor2d::cache::Cache;
    use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};

    const CUANTOS: u64 = 500;
    const FOTOGRAMAS: u64 = 60;

    let mut escena = Escena::nueva();
    for i in 0..CUANTOS {
        escena.anadir(Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x: i as f32 * 30.0,
            y: 0.0,
            ancho: 20.0,
            alto: 20.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: (i + 1) as u32,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        });
    }

    let mut cache = Cache::nueva();
    for _ in 0..FOTOGRAMAS {
        // Encuadrar mueve la camara pero NO el aumento: el zoom que se le
        // pasa a la cache es el mismo en los sesenta fotogramas.
        for e in escena.visibles() {
            cache.ordenes(e, 1.0);
        }
    }

    assert_eq!(cache.fallos(), CUANTOS, "solo el primer fotograma calcula");
    assert_eq!(cache.aciertos(), CUANTOS * (FOTOGRAMAS - 1));
}

#[test]
fn el_iman_no_asigna_en_el_camino_caliente() {
    // El iman corre en cada movimiento del raton. Si monta una lista de
    // anclajes por elemento -que es lo que hace el original en Kotlin- el
    // presupuesto de cero asignaciones se va al suelo con un plano lleno.
    use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};

    let mut escena = Escena::nueva();
    let mut gesto = Gesto::nuevo();
    gesto.herramienta = Herramienta::Rectangulo;

    // Cien rectangulos por los que pasar por encima. `Elemento` no tiene
    // constructora: se monta con el literal, igual que hace
    // `arrastrar_una_seleccion_tampoco_asigna` mas arriba en este fichero.
    for i in 0..100u64 {
        let mut e = Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 40.0,
            alto: 40.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        };
        e.id = i + 1;
        e.x = (i % 10) as f32 * 40.0;
        e.y = (i / 10) as f32 * 40.0;
        e.ancho = 30.0;
        e.alto = 30.0;
        escena.elementos.push(e);
    }

    gesto.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(500.0, 500.0),
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );

    let (_, veces) = contando(|| {
        for i in 1..400 {
            gesto.evento(
                EventoGesto::Mover {
                    p: Punto2::nuevo((i % 400) as f32, (i % 400) as f32),
                    shift: false,
                    alt: false,
                    presion: None,
                },
                &mut escena,
                1.0,
            );
        }
    });

    assert_eq!(veces, 0, "el iman asigno {veces} veces");
}
