//! **Soldar vertices con el raton** (`Tool.NUDO` del movil, `NudosTest.kt`):
//! lo clavado se comporta como tal cuando se arrastra con la mano, contra la
//! API publica del gesto, como lo usa la ventana.

use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::nudos;
use pixpin_motor2d::vector::Punto2;

fn raya(a: (f32, f32), b: (f32, f32)) -> Elemento {
    Elemento {
        figura: Figura::Linea {
            puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
        },
        x: a.0.min(b.0),
        y: a.1.min(b.1),
        ancho: (b.0 - a.0).abs(),
        alto: (b.1 - a.1).abs(),
        trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
        grosor: 2.0,
        ..Default::default()
    }
}

fn arrastrar(g: &mut Gesto, e: &mut Escena, de: (f32, f32), a: (f32, f32)) {
    g.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(de.0, de.1),
            shift: false,
            alt: false,
            presion: None,
        },
        e,
        1.0,
    );
    // En varios avisos, como llega de verdad el raton.
    for i in 1..=10 {
        let t = i as f32 / 10.0;
        g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(de.0 + (a.0 - de.0) * t, de.1 + (a.1 - de.1) * t),
                shift: false,
                alt: false,
                presion: None,
            },
            e,
            1.0,
        );
    }
    g.evento(
        EventoGesto::Soltar {
            p: Punto2::nuevo(a.0, a.1),
        },
        e,
        1.0,
    );
}

fn puntas(e: &Escena, id: u64) -> Vec<Punto2> {
    nudos::vertices_en_el_mundo(e.buscar(id).unwrap())
}

/// Un angulo en L: la base y el lado clavados por la esquina.
fn escuadra_clavada() -> (Escena, u64, u64) {
    let mut escena = Escena::nueva();
    let base = escena.anadir(raya((0.0, 100.0), (100.0, 100.0)));
    let lado = escena.anadir(raya((100.0, 100.0), (100.0, 0.0)));
    assert!(nudos::soldar(
        &mut escena,
        Punto2::nuevo(100.0, 100.0),
        16.0
    ));
    (escena, base, lado)
}

#[test]
fn arrastrar_con_la_mano_una_raya_clavada_la_hace_girar_sobre_el_clavo() {
    let (mut escena, base, lado) = escuadra_clavada();
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Mano);
    arrastrar(&mut g, &mut escena, (30.0, 100.0), (30.0, 160.0));
    let b = puntas(&escena, base);
    assert!(
        b[1].distancia(Punto2::nuevo(100.0, 100.0)) < 0.5,
        "se despego del clavo: {b:?}"
    );
    assert!(b[0].y > 120.0, "no giro: {b:?}");
    assert!(
        (b[0].distancia(b[1]) - 100.0).abs() < 0.5,
        "se estiro al girar"
    );
    assert_eq!(
        puntas(&escena, lado)[1],
        Punto2::nuevo(100.0, 0.0),
        "el lado se movio"
    );
    // Un paso de deshacer, y vuelve entera.
    assert!(escena.deshacer());
    assert_eq!(puntas(&escena, base)[0], Punto2::nuevo(0.0, 100.0));
}

#[test]
fn coger_el_clavo_con_la_mano_se_lleva_la_esquina_de_las_dos() {
    let (mut escena, base, lado) = escuadra_clavada();
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Mano);
    arrastrar(&mut g, &mut escena, (101.0, 99.0), (150.0, 140.0));
    let esquina = Punto2::nuevo(150.0, 140.0);
    assert!(
        puntas(&escena, base)[1].distancia(esquina) < 1.5,
        "{:?}",
        puntas(&escena, base)
    );
    assert!(
        puntas(&escena, lado)[0].distancia(esquina) < 1.5,
        "{:?}",
        puntas(&escena, lado)
    );
    // Las otras puntas, quietas: es un clavo, no un arrastre de todo.
    assert_eq!(puntas(&escena, base)[0], Punto2::nuevo(0.0, 100.0));
    assert_eq!(escena.alfileres.len(), 1);
    assert!(escena.alfileres[0].punto.distancia(esquina) < 1.5);
}

#[test]
fn con_clavos_en_la_escena_lo_elegido_no_se_mueve_como_una_capa_quieta() {
    // La ventana mueve lo elegido como una capa trasladada si `moviendo()`
    // dice que si. Con clavos puede girar, y esa capa mentiria.
    let (mut escena, _, _) = escuadra_clavada();
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Mano);
    g.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(30.0, 100.0),
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );
    assert!(!g.moviendo());
}

#[test]
fn sin_clavos_arrastrar_sigue_trasladando_y_moviendo_como_capa() {
    // Caso negativo: nada de esto cambia un dibujo sin clavos.
    let mut escena = Escena::nueva();
    let base = escena.anadir(raya((0.0, 100.0), (100.0, 100.0)));
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Mano);
    g.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(30.0, 100.0),
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );
    assert!(g.moviendo());
    g.evento(
        EventoGesto::Soltar {
            p: Punto2::nuevo(30.0, 100.0),
        },
        &mut escena,
        1.0,
    );
    arrastrar(&mut g, &mut escena, (30.0, 100.0), (30.0, 160.0));
    assert_eq!(puntas(&escena, base)[0], Punto2::nuevo(0.0, 160.0));
}

#[test]
fn con_el_lapiz_en_la_mano_pulsar_en_un_clavo_dibuja_y_no_lo_coge() {
    // Caso negativo: coger el clavo es solo de la mano, como mover.
    let (mut escena, _, _) = escuadra_clavada();
    let antes = escena.alfileres.clone();
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Lapiz);
    arrastrar(&mut g, &mut escena, (100.0, 100.0), (150.0, 140.0));
    assert_eq!(escena.alfileres, antes);
    assert_eq!(escena.visibles().count(), 3, "no nacio el trazo");
}

#[test]
fn un_clavo_que_no_sujeta_lo_elegido_no_le_quita_moverse_como_capa() {
    // La queja «todo se mueve lento»: un solo clavo en cualquier sitio del
    // dibujo apagaba la capa trasladada de TODO arrastre, y cada aviso del
    // raton rehacia la escena entera. Solo cuenta si sujeta lo que se mueve.
    let (mut escena, _, _) = escuadra_clavada();
    let suelta = escena.anadir(raya((300.0, 300.0), (400.0, 300.0)));
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Mano);
    g.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(330.0, 300.0),
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );
    assert!(g.moviendo(), "la raya suelta no esta clavada");
    g.evento(
        EventoGesto::Soltar {
            p: Punto2::nuevo(330.0, 300.0),
        },
        &mut escena,
        1.0,
    );
    let antes = escena.alfileres.clone();
    arrastrar(&mut g, &mut escena, (330.0, 300.0), (330.0, 360.0));
    assert_eq!(
        puntas(&escena, suelta)[0],
        Punto2::nuevo(300.0, 360.0),
        "se traslada entera"
    );
    assert_eq!(
        escena.alfileres, antes,
        "el clavo de la escuadra no se entera"
    );
}
