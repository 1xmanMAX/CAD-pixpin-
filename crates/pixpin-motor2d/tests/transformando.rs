//! **Si el gesto solo esta cambiando lo elegido** (estirarlo o girarlo por
//! sus tiradores): la ventana rehace entonces solo el trozo de lo elegido
//! sobre la capa congelada, y no la escena entera en cada aviso del raton.

use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::vector::Punto2;

fn pulsar(g: &mut Gesto, e: &mut Escena, p: Punto2) {
    g.evento(
        EventoGesto::Pulsar {
            p,
            shift: false,
            alt: false,
            presion: None,
        },
        e,
        1.0,
    );
}

fn con_una_caja_elegida() -> (Escena, Gesto) {
    let mut escena = Escena::nueva();
    let id = escena.anadir(Elemento {
        figura: Figura::Rectangulo,
        x: 100.0,
        y: 100.0,
        ancho: 200.0,
        alto: 100.0,
        grosor: 2.0,
        ..Default::default()
    });
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Mano);
    g.seleccion.poner(id);
    (escena, g)
}

#[test]
fn estirar_por_un_tirador_es_transformar_y_no_mover() {
    let (mut escena, mut g) = con_una_caja_elegida();
    let q = g.tiradores(&escena, 1.0).expect("tiradores").tamano[4].1;
    pulsar(&mut g, &mut escena, q);
    assert!(g.transformando());
    assert!(!g.moviendo());
}

#[test]
fn girar_por_su_tirador_tambien_es_transformar() {
    let (mut escena, mut g) = con_una_caja_elegida();
    let q = g.tiradores(&escena, 1.0).expect("tiradores").giro;
    pulsar(&mut g, &mut escena, q);
    assert!(g.transformando());
}

#[test]
fn mover_lo_elegido_o_estar_quieto_no_es_transformar() {
    let (mut escena, mut g) = con_una_caja_elegida();
    assert!(!g.transformando(), "en reposo");
    // Por el borde de la caja, lejos de los tiradores (sin relleno se coge
    // por su raya).
    pulsar(&mut g, &mut escena, Punto2::nuevo(150.0, 100.0));
    assert!(g.moviendo());
    assert!(!g.transformando());
}
