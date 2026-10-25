//! **Lo que tarda el nucleo con un plano grande** (200 000 lineas y algunos
//! circulos y arcos). Las pruebas de tiempo van con `#[ignore]`: se corren con
//!
//! ```text
//! cargo test -p cad-nucleo --release -- --ignored rendimiento --nocapture
//! ```
//!
//! y escriben lo que tarda cada cosa. Los topes se comprueban en proporcion
//! (lo incremental frente a rehacerlo todo) siempre, y en milisegundos solo
//! si se compilo optimizado (en depuracion todo va diez veces mas lento).

use std::time::{Duration, Instant};

use cad_nucleo::dibujo::Indice;
use cad_nucleo::enganche::{self, Modos};
use cad_nucleo::especialidad::{arco, circulo, linea};
use cad_nucleo::geo::P;
use cad_nucleo::opencadcodec::Handle;
use cad_nucleo::{Dibujo, Sesion, seleccion};

/// Cuantas lineas tiene el plano de prueba.
const LINEAS: usize = 200_000;
/// Cuantos circulos y cuantos arcos, ademas.
const CURVAS: usize = 10_000;
/// Columnas de la cuadricula en que se reparten (a 100 unidades).
const COLUMNAS: usize = 500;

/// Un generador pseudoaleatorio sencillo (siempre la misma serie).
struct Azar(u64);

impl Azar {
    fn sig(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// **El plano grande**: lineas que se cruzan con sus vecinas, y curvas.
fn plano() -> (Dibujo, Vec<Handle>) {
    let mut d = Dibujo::nuevo();
    let mut az = Azar(7);
    let mut hs = Vec::with_capacity(LINEAS + 2 * CURVAS);
    d.editar("PLANO", |t| {
        for i in 0..LINEAS {
            let (x, y) = ((i % COLUMNAS) as f64 * 100.0, (i / COLUMNAS) as f64 * 100.0);
            let a = [x, y];
            let b = [x + 60.0 + az.sig() * 80.0, y + az.sig() * 120.0 - 20.0];
            hs.push(t.anadir(linea(a, b)).unwrap());
        }
        for i in 0..CURVAS {
            let k = i * (LINEAS / CURVAS);
            let c = [
                (k % COLUMNAS) as f64 * 100.0 + 50.0,
                (k / COLUMNAS) as f64 * 100.0,
            ];
            hs.push(t.anadir(circulo(c, 30.0)).unwrap());
            hs.push(t.anadir(arco(c, 45.0, 0.3, 2.5)).unwrap());
        }
    });
    (d, hs)
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Lo que tarda `f`, en milisegundos (la media de `veces`).
fn medir(veces: usize, mut f: impl FnMut(usize)) -> f64 {
    let t = Instant::now();
    for i in 0..veces {
        f(i);
    }
    ms(t.elapsed()) / veces as f64
}

fn optimizado() -> bool {
    !cfg!(debug_assertions)
}

/// Puntos al azar dentro del plano.
fn puntos(n: usize) -> Vec<P> {
    let mut az = Azar(99);
    let filas = (LINEAS / COLUMNAS) as f64;
    (0..n)
        .map(|_| [az.sig() * COLUMNAS as f64 * 100.0, az.sig() * filas * 100.0])
        .collect()
}

#[test]
#[ignore]
fn rendimiento_indice_completo_e_incremental() {
    let t = Instant::now();
    let (mut d, hs) = plano();
    println!(
        "construir {} entidades: {:.1} ms",
        hs.len(),
        ms(t.elapsed())
    );
    let t = Instant::now();
    let n = d.indice().len();
    let primero = ms(t.elapsed());
    println!("primer indice ({n} entradas): {primero:.1} ms");
    assert_eq!(n, hs.len());
    let completo = medir(3, |_| {
        let _ = Indice::completo(&d.doc);
    });
    println!("indice rehecho entero: {completo:.1} ms");

    // Mover una linea y volver a pedir el indice.
    let mover = medir(20, |i| {
        let h = hs[i * 997];
        d.editar("MOVER", |t| {
            t.cambiar(h, |e| {
                e.translate(cad_nucleo::opencadcodec::Vector3::new(5.0, 5.0, 0.0))
            })
        });
        let _ = d.indice();
    });
    println!("mover 1 linea + indice: {mover:.3} ms");
    let deshacer = medir(20, |_| {
        d.deshacer();
        let _ = d.indice();
    });
    println!("deshacer + indice: {deshacer:.3} ms");
    let rehacer = medir(20, |_| {
        d.rehacer();
        let _ = d.indice();
    });
    println!("rehacer + indice: {rehacer:.3} ms");
    // Mover mil a la vez.
    let mil = medir(5, |i| {
        d.editar("MOVER", |t| {
            for h in &hs[i * 1000..(i + 1) * 1000] {
                t.cambiar(*h, |e| {
                    e.translate(cad_nucleo::opencadcodec::Vector3::new(1.0, 0.0, 0.0))
                });
            }
        });
        let _ = d.indice();
    });
    println!("mover 1000 lineas + indice: {mil:.3} ms");
    assert!(
        mover * 20.0 < completo,
        "lo incremental tiene que ser mucho mas rapido: {mover} frente a {completo}"
    );
    if optimizado() {
        assert!(mover < 5.0, "mover una linea: {mover} ms");
        assert!(deshacer < 5.0 && rehacer < 5.0);
        assert!(mil < 50.0, "mover mil lineas: {mil} ms");
    }
}

#[test]
#[ignore]
fn rendimiento_enganches_y_elegir() {
    let (d, _) = plano();
    let ind = d.indice();
    let pts = puntos(10_000);
    let m = Modos(0xff);
    let mut hallados = 0;
    let buscar = medir(pts.len(), |i| {
        if enganche::buscar(&ind, pts[i], 10.0, m, Some([0.0, 0.0])).is_some() {
            hallados += 1;
        }
    });
    println!(
        "enganche (radio 10): {:.1} us por busqueda ({hallados} con enganche)",
        buscar * 1000.0
    );
    // Con el plano entero en pantalla, unos pixeles son muchas unidades.
    let lejos = medir(500, |i| {
        let _ = enganche::buscar(&ind, pts[i], 400.0, m, Some([0.0, 0.0]));
    });
    println!(
        "enganche (radio 400): {:.1} us por busqueda",
        lejos * 1000.0
    );
    let mut elegidos = 0;
    let elegir = medir(pts.len(), |i| {
        if seleccion::elegir(&ind, pts[i], 10.0).is_some() {
            elegidos += 1;
        }
    });
    println!(
        "elegir con clic: {:.1} us por clic ({elegidos} con algo)",
        elegir * 1000.0
    );
    assert!(hallados > 0 && elegidos > 0);
    if optimizado() {
        assert!(buscar < 0.2, "enganche: {buscar} ms");
        assert!(lejos < 5.0, "enganche lejos: {lejos} ms");
        assert!(elegir < 0.1, "elegir: {elegir} ms");
    }
}

#[test]
#[ignore]
fn rendimiento_recuadros() {
    let (d, hs) = plano();
    let ind = d.indice();
    let todo = [-1000.0, -1000.0, 1e6, 1e6];
    let mut n = 0;
    let ventana = medir(5, |_| {
        n = seleccion::por_recuadro(&ind, [todo[0], todo[1]], [todo[2], todo[3]]).len();
    });
    println!("ventana sobre todo: {ventana:.1} ms ({n})");
    assert_eq!(n, hs.len());
    let captura = medir(5, |_| {
        n = seleccion::por_recuadro(&ind, [todo[2], todo[3]], [todo[0], todo[1]]).len();
    });
    println!("captura sobre todo: {captura:.1} ms ({n})");
    assert_eq!(n, hs.len());
    // Una captura que corta muchas: la mitad del plano.
    let mitad = medir(5, |_| {
        n = seleccion::por_recuadro(&ind, [25_013.0, 20_007.0], [3.0, 7.0]).len();
    });
    println!("captura sobre un cuarto del plano: {mitad:.1} ms ({n})");
    // Una franja fina que corta casi todo lo que toca (lo caro: forma a forma).
    let franja = medir(5, |_| {
        n = seleccion::por_recuadro(&ind, [50_000.0, 10_031.0], [0.0, 9_969.0]).len();
    });
    println!("captura en franja: {franja:.1} ms ({n})");
    assert!(n > 0);
    if optimizado() {
        assert!(franja < 20.0, "captura en franja: {franja} ms");
        assert!(ventana < 60.0, "ventana: {ventana} ms");
        assert!(captura < 60.0, "captura: {captura} ms");
        assert!(mitad < 60.0, "captura media: {mitad} ms");
    }
}

#[test]
#[ignore]
fn rendimiento_previa_de_mover_todo() {
    let (d, hs) = plano();
    let mut s = Sesion::nueva(d);
    s.elegir_todo();
    assert_eq!(s.seleccion.len(), hs.len());
    for orden in ["m", "gi", "es", "si"] {
        s.escape();
        s.elegir_todo();
        s.escribir(orden);
        s.clic([0.0, 0.0], 1.0);
        let mut formas = 0;
        let previa = medir(10, |i| {
            formas = s.previa([100.0 + i as f64, 50.0]).formas.len();
        });
        println!(
            "previa de {orden} con {} elegidas: {previa:.2} ms ({formas} formas)",
            hs.len()
        );
        assert!(formas > 0);
        if optimizado() {
            assert!(previa < 30.0, "previa de {orden}: {previa} ms");
        }
    }
}

#[test]
#[ignore]
fn rendimiento_mover_borrar_y_deshacer_todo() {
    let (d, hs) = plano();
    let mut s = Sesion::nueva(d);
    s.elegir_todo();
    s.escribir("m");
    s.clic([0.0, 0.0], 1.0);
    let t = Instant::now();
    s.clic([100.0, 50.0], 1.0);
    let mover = ms(t.elapsed());
    let t = Instant::now();
    let _ = s.dibujo.indice();
    let indice = ms(t.elapsed());
    let t = Instant::now();
    s.escribir("u");
    let _ = s.dibujo.indice();
    let deshacer = ms(t.elapsed());
    s.elegir_todo();
    let t = Instant::now();
    s.borrar_seleccion();
    let _ = s.dibujo.indice();
    let borrar = ms(t.elapsed());
    println!(
        "con {} objetos: mover {mover:.0} ms (+ indice {indice:.0} ms), deshacer {deshacer:.0} ms, borrar {borrar:.0} ms",
        hs.len()
    );
    assert_eq!(s.dibujo.doc.model_space_entities().count(), 0);
    if optimizado() {
        assert!(mover + indice < 3000.0, "mover: {mover} + {indice} ms");
        assert!(deshacer < 3000.0, "deshacer: {deshacer} ms");
        assert!(borrar < 3000.0, "borrar: {borrar} ms");
    }
}
