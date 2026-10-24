//! **F7: con la herramienta de texto, tocar una figura ya no escribe dentro**
//! (v0.67 y v0.72 del movil, `DrawController.plantarTexto` con
//! `textoDentroDeFiguras` apagado de fabrica).
//!
//! El usuario lo pidio el 19-sep-2026: prefiere el texto SUELTO —«anotar el
//! texto por separado, la figura por separado, seleccionarlos y unirlos»—,
//! porque dentro de la figura el texto queda obligado a su zona. Lo que ya
//! estaba escrito dentro de una figura (o llega asi de Excalidraw) se sigue
//! editando tocando el propio texto. Aqui se comprueban las dos cosas contra
//! la API publica del gesto, como el usuario.

use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::vector::Punto2;

fn pulsar(g: &mut Gesto, e: &mut Escena, x: f32, y: f32) {
    g.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(x, y),
            shift: false,
            alt: false,
            presion: None,
        },
        e,
        1.0,
    );
    g.evento(
        EventoGesto::Soltar {
            p: Punto2::nuevo(x, y),
        },
        e,
        1.0,
    );
}

fn caja() -> Elemento {
    Elemento {
        figura: Figura::Rectangulo,
        x: 0.0,
        y: 0.0,
        ancho: 200.0,
        alto: 100.0,
        ..Default::default()
    }
}

#[test]
fn con_el_texto_en_la_mano_tocar_una_caja_planta_un_texto_suelto_encima() {
    let mut escena = Escena::nueva();
    let id_caja = escena.anadir(caja());
    let antes = escena.buscar(id_caja).cloned().unwrap();
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Texto);
    pulsar(&mut g, &mut escena, 100.0, 50.0);
    for c in "hola".chars() {
        g.escribir(c, &mut escena);
    }
    let (id_texto, _) = g.escribiendo.clone().expect("se esta escribiendo");
    let texto = escena.buscar(id_texto).unwrap();
    assert!(matches!(&texto.figura, Figura::Texto { texto, .. } if texto == "hola"));
    // Suelto: sin contenedor, donde se toco, y la caja sin tocar (no se ata).
    assert_eq!(texto.extras.contenedor, None);
    assert_eq!((texto.x, texto.y), (100.0, 50.0));
    assert_eq!(escena.buscar(id_caja).unwrap(), &antes);
}

#[test]
fn un_texto_que_ya_estaba_dentro_de_una_caja_se_sigue_editando_tocandolo() {
    // Caso negativo del anterior: tocar el texto de dentro no planta otro
    // encima, lo abre.
    let mut escena = Escena::nueva();
    escena.anadir(caja());
    let id = escena.anadir(Elemento {
        figura: Figura::Texto {
            texto: "dentro".into(),
            tam: 20.0,
            familia: "Segoe UI".into(),
        },
        x: 60.0,
        y: 38.0,
        ancho: 90.0,
        alto: 26.0,
        ..Default::default()
    });
    let cuantos = escena.visibles().count();
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Texto);
    pulsar(&mut g, &mut escena, 80.0, 50.0);
    assert_eq!(g.escribiendo.as_ref().map(|(i, _)| *i), Some(id));
    assert_eq!(escena.visibles().count(), cuantos, "no nacio otro texto");
}
