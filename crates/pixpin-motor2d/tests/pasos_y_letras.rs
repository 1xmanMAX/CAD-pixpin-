//! **Los pasos numerados y la letra de todo lo que tiene letra**, contra la
//! API publica, como el usuario («solo se pone el 1 y tengo que agrandarlo
//! como un circulo»; «poder elegir el tipo de letra en todo lo que tenga
//! letra»).

use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::estilo::{CambioForma, aplicar_forma};
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::seleccion::Seleccion;
use pixpin_motor2d::texto::{FUENTE_CAVEAT, FUENTE_NUNITO};
use pixpin_motor2d::vector::Punto2;

fn clic(g: &mut Gesto, e: &mut Escena, x: f32, y: f32) {
    let p = Punto2::nuevo(x, y);
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
    g.evento(EventoGesto::Soltar { p }, e, 1.0);
}

fn con(h: Herramienta) -> Gesto {
    let mut g = Gesto::nuevo();
    g.enganche.activo = false;
    g.tomar_herramienta(h);
    g
}

fn numeros(e: &Escena) -> Vec<u32> {
    e.visibles()
        .filter_map(|e| match e.figura {
            Figura::Serie { numero } => Some(numero),
            _ => None,
        })
        .collect()
}

#[test]
fn cada_clic_con_pasos_pone_el_siguiente_numero_ya_hecho_y_centrado() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Serie);
    clic(&mut g, &mut escena, 100.0, 100.0);
    clic(&mut g, &mut escena, 200.0, 100.0);
    clic(&mut g, &mut escena, 300.0, 100.0);
    assert_eq!(numeros(&escena), [1, 2, 3]);
    // Del tamano del movil sin arrastrar: radio = letra (20) x 0,9.
    let primero = escena.visibles().next().unwrap();
    assert_eq!((primero.ancho, primero.alto), (36.0, 36.0));
    assert_eq!((primero.x + 18.0, primero.y + 18.0), (100.0, 100.0));
    // Un paso de deshacer por numero, y el siguiente vuelve a ser ese.
    assert!(escena.deshacer());
    assert_eq!(numeros(&escena), [1, 2]);
    clic(&mut g, &mut escena, 300.0, 100.0);
    assert_eq!(numeros(&escena), [1, 2, 3]);
}

#[test]
fn arrastrar_con_pasos_no_estira_el_circulo() {
    // Caso negativo: antes el arrastre era lo unico que le daba tamano.
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Serie);
    let p = |x: f32, y: f32| Punto2::nuevo(x, y);
    g.evento(
        EventoGesto::Pulsar {
            p: p(50.0, 50.0),
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );
    g.evento(
        EventoGesto::Mover {
            p: p(250.0, 250.0),
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );
    g.evento(EventoGesto::Soltar { p: p(250.0, 250.0) }, &mut escena, 1.0);
    let e = escena.visibles().next().unwrap();
    assert_eq!((e.ancho, e.alto), (36.0, 36.0));
}

#[test]
fn el_numero_nace_con_la_letra_y_el_tamano_del_pincel() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Serie);
    g.estilo.aplicar_forma(CambioForma::Familia(FUENTE_CAVEAT));
    g.estilo.aplicar_forma(CambioForma::TamanoLetra(28.0));
    clic(&mut g, &mut escena, 0.0, 0.0);
    let e = escena.visibles().next().unwrap();
    assert_eq!(e.extras.familia.as_deref(), Some("Caveat"));
    assert!((e.ancho - 28.0 * 0.9 * 2.0).abs() < 0.01, "{}", e.ancho);
}

fn elegido(escena: &mut Escena, e: Elemento) -> Seleccion {
    let id = escena.anadir(e);
    let mut s = Seleccion::default();
    s.poner(id);
    s
}

#[test]
fn la_letra_se_cambia_en_la_cota_el_numero_y_el_punto() {
    for figura in [
        Figura::Serie { numero: 1 },
        Figura::Cota {
            puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(50.0, 0.0)],
        },
        Figura::Punto {
            letra: "A".into(),
            angulo: 0.0,
            radio: 14.0,
        },
    ] {
        let mut escena = Escena::nueva();
        let sel = elegido(
            &mut escena,
            Elemento {
                figura: figura.clone(),
                ancho: 40.0,
                alto: 40.0,
                ..Default::default()
            },
        );
        assert!(
            aplicar_forma(&mut escena, &sel, CambioForma::Familia(FUENTE_NUNITO)),
            "{figura:?}"
        );
        let e = escena.visibles().next().unwrap();
        assert_eq!(e.extras.familia.as_deref(), Some("Nunito"), "{figura:?}");
        // Pulsar la que ya tiene no abre un paso vacio.
        assert!(!aplicar_forma(
            &mut escena,
            &sel,
            CambioForma::Familia(FUENTE_NUNITO)
        ));
    }
    // Caso negativo: un rectangulo no tiene letra que cambiar.
    let mut escena = Escena::nueva();
    let sel = elegido(
        &mut escena,
        Elemento {
            figura: Figura::Rectangulo,
            ancho: 40.0,
            alto: 40.0,
            ..Default::default()
        },
    );
    assert!(!aplicar_forma(
        &mut escena,
        &sel,
        CambioForma::Familia(FUENTE_NUNITO)
    ));
    assert_eq!(escena.visibles().next().unwrap().extras.familia, None);
}

#[test]
fn cambiar_el_tamano_de_un_numero_agranda_su_circulo_desde_el_centro() {
    let mut escena = Escena::nueva();
    let sel = elegido(
        &mut escena,
        Elemento {
            figura: Figura::Serie { numero: 4 },
            x: 82.0,
            y: 82.0,
            ancho: 36.0,
            alto: 36.0,
            ..Default::default()
        },
    );
    assert!(aplicar_forma(
        &mut escena,
        &sel,
        CambioForma::TamanoLetra(36.0)
    ));
    let e = escena.visibles().next().unwrap();
    assert!(
        (e.ancho - 64.8).abs() < 0.01 && (e.alto - 64.8).abs() < 0.01,
        "{}",
        e.ancho
    );
    assert!(
        (e.x + e.ancho / 2.0 - 100.0).abs() < 0.01,
        "sigue centrado en 100"
    );
}

#[test]
fn el_foco_elegido_cambia_cuanto_oscurece_y_su_zona_sin_mover_el_marco() {
    use pixpin_motor2d::lupa_elemento::{self as l, Cristal};
    let mut escena = Escena::nueva();
    let sel = elegido(
        &mut escena,
        Elemento {
            figura: Figura::Foco {
                cristal: Cristal {
                    foco: Some(Punto2::nuevo(100.0, 50.0)),
                    foco_ancho: Some(100.0),
                    foco_alto: Some(50.0),
                    ..Default::default()
                },
            },
            x: 10.0,
            y: 5.0,
            ancho: 180.0,
            alto: 90.0,
            ..Default::default()
        },
    );
    assert!(aplicar_forma(&mut escena, &sel, CambioForma::Oscurecer(65)));
    assert!(aplicar_forma(
        &mut escena,
        &sel,
        CambioForma::ZonaFoco(0.75)
    ));
    let e = escena.visibles().next().unwrap();
    let Figura::Foco { cristal } = &e.figura else {
        panic!()
    };
    assert_eq!(l::oscurecimiento_de(cristal), 65);
    assert!((l::zona_de(cristal, l::caja_de(e)) - 0.75).abs() < 1e-4);
    assert_eq!(
        (e.x, e.y, e.ancho, e.alto),
        (10.0, 5.0, 180.0, 90.0),
        "el marco no se mueve"
    );
    // Caso negativo: a una lupa no se le cambia el oscurecer.
    let mut escena = Escena::nueva();
    let sel = elegido(
        &mut escena,
        Elemento {
            figura: Figura::Lupa {
                cristal: Cristal::default(),
            },
            ancho: 50.0,
            alto: 50.0,
            ..Default::default()
        },
    );
    assert!(!aplicar_forma(
        &mut escena,
        &sel,
        CambioForma::Oscurecer(65)
    ));
}
