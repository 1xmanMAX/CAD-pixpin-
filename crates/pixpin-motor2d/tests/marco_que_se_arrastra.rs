//! **Arrastrar un marco se lleva lo que encierra**, y quien pinta tiene que
//! saber que es eso para no dejarlo quieto en la capa congelada (queja del
//! 28-sep-2026: «al poner los frames pasa lo mismo», lento).

use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::vector::Punto2;

fn caja(figura: Figura, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
    Elemento {
        figura,
        x,
        y,
        ancho,
        alto,
        grosor: 2.0,
        ..Default::default()
    }
}

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
}

fn mover(g: &mut Gesto, e: &mut Escena, x: f32, y: f32) {
    g.evento(
        EventoGesto::Mover {
            p: Punto2::nuevo(x, y),
            shift: false,
            alt: false,
            presion: None,
        },
        e,
        1.0,
    );
}

/// Un marco de 0..400 x 0..300 con un rectangulo dentro, y otro rectangulo
/// fuera, a la derecha, en el camino del arrastre.
fn lamina() -> (Escena, u64, u64, u64) {
    let mut e = Escena::nueva();
    let marco = e.anadir(caja(
        Figura::Marco {
            nombre: "Lamina".into(),
        },
        0.0,
        0.0,
        400.0,
        300.0,
    ));
    let dentro = e.anadir(caja(Figura::Rectangulo, 100.0, 100.0, 50.0, 50.0));
    let en_el_camino = e.anadir(caja(Figura::Rectangulo, 500.0, 100.0, 50.0, 50.0));
    (e, marco, dentro, en_el_camino)
}

fn agarrar_el_marco(e: &mut Escena, marco: u64) -> Gesto {
    let mut g = Gesto::nuevo();
    g.herramienta = Herramienta::Mano;
    g.seleccion.poner_todos([marco]);
    // Por el borde de arriba, lejos de los tiradores: el marco se coge por
    // su raya.
    pulsar(&mut g, e, 100.0, 0.0);
    assert!(g.moviendo(), "cogido para mover");
    g
}

#[test]
fn mientras_se_arrastra_un_marco_dice_que_lleva_lo_de_dentro() {
    let (mut e, marco, dentro, fuera) = lamina();
    let mut g = agarrar_el_marco(&mut e, marco);
    mover(&mut g, &mut e, 130.0, 10.0);
    assert!(g.lleva_el_marco(dentro), "lo de dentro va con el marco");
    assert!(!g.lleva_el_marco(fuera), "lo de fuera no");
    assert!(
        !g.lleva_el_marco(marco),
        "el marco va por la seleccion, no por aqui"
    );
    let d = e.buscar(dentro).unwrap();
    assert_eq!((d.x, d.y), (130.0, 110.0), "y de verdad se ha movido");
}

#[test]
fn un_marco_no_recoge_lo_que_pisa_por_el_camino() {
    // Lo de dentro se decide al coger el marco, como el `frameId` de
    // Excalidraw: pasar por encima de algo no lo mete en el marco.
    let (mut e, marco, _, fuera) = lamina();
    let mut g = agarrar_el_marco(&mut e, marco);
    for i in 1..=20 {
        mover(&mut g, &mut e, 100.0 + i as f32 * 20.0, 0.0);
    }
    let f = e.buscar(fuera).unwrap();
    assert_eq!(
        (f.x, f.y),
        (500.0, 100.0),
        "lo que estaba fuera sigue en su sitio"
    );
    assert!(!g.lleva_el_marco(fuera));
}

#[test]
fn sin_arrastrar_no_lleva_nada() {
    let (mut e, marco, dentro, _) = lamina();
    let mut g = agarrar_el_marco(&mut e, marco);
    mover(&mut g, &mut e, 130.0, 10.0);
    g.evento(
        EventoGesto::Soltar {
            p: Punto2::nuevo(130.0, 10.0),
        },
        &mut e,
        1.0,
    );
    assert!(!g.lleva_el_marco(dentro), "soltado, ya no se arrastra nada");
    // Y un rectangulo elegido no es un marco: no lleva nada.
    let mut g = Gesto::nuevo();
    g.herramienta = Herramienta::Mano;
    g.seleccion.poner_todos([dentro]);
    pulsar(&mut g, &mut e, 155.0, 135.0);
    assert!(g.lo_que_lleva_el_marco().is_empty());
}
