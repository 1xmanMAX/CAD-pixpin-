//! **Se para y sale limpio: el compas y el rectangulo de la «L»**, contra la
//! API publica del gesto, como el usuario.
//!
//! Porte de `DedoParadoTest.kt` del movil (v0.80.2). La ventana llama a
//! `convertir_en_forma` cuando el cursor lleva `ESPERA_PARA_LA_FORMA_MS`
//! quieto con el boton pulsado; aqui se llama directamente, que es lo mismo
//! sin reloj. Lo que no se llama es lo que «no se sostiene».

use pixpin_motor2d::elemento::Figura;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::tinta::MaterialTinta;
use pixpin_motor2d::vector::Punto2;

fn ev(g: &mut Gesto, e: &mut Escena, evento: EventoGesto) {
    g.evento(evento, e, 1.0);
}

fn pulsar(x: f32, y: f32) -> EventoGesto {
    EventoGesto::Pulsar {
        p: Punto2::nuevo(x, y),
        shift: false,
        alt: false,
        presion: None,
    }
}

fn mover(x: f32, y: f32) -> EventoGesto {
    EventoGesto::Mover {
        p: Punto2::nuevo(x, y),
        shift: false,
        alt: false,
        presion: None,
    }
}

fn soltar(x: f32, y: f32) -> EventoGesto {
    EventoGesto::Soltar {
        p: Punto2::nuevo(x, y),
    }
}

fn con(h: Herramienta) -> Gesto {
    let mut g = Gesto::nuevo();
    g.enganche.activo = false;
    g.tomar_herramienta(h);
    g
}

/// Una «L» a mano: baja en vertical de (0,0) a (0,100) con un temblor, y
/// luego va en horizontal hasta (150,100).
fn trazar_ele(g: &mut Gesto, e: &mut Escena) {
    ev(g, e, pulsar(0.0, 0.0));
    for k in 1..=20 {
        let temblor = if k % 2 == 0 { 1.0 } else { -1.0 };
        ev(g, e, mover(temblor, k as f32 * 5.0));
    }
    for k in 1..=30 {
        ev(
            g,
            e,
            mover(k as f32 * 5.0, 100.0 + if k % 3 == 0 { 1.0 } else { 0.0 }),
        );
    }
}

fn ultimo(e: &Escena) -> &pixpin_motor2d::elemento::Elemento {
    e.elementos.last().expect("nacio algo")
}

#[test]
fn una_raya_vertical_y_luego_horizontal_quieta_da_un_rectangulo_que_sigue_al_cursor() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Lapiz);
    trazar_ele(&mut g, &mut escena);
    assert!(
        g.convertir_en_forma(&mut escena, Punto2::nuevo(150.0, 100.0), 1.0)
            .is_some()
    );
    let r = ultimo(&escena);
    assert!(matches!(r.figura, Figura::Rectangulo), "{:?}", r.figura);
    // Una esquina donde empezo y la otra en el cursor.
    assert_eq!((r.x, r.y, r.ancho, r.alto), (0.0, 0.0, 150.0, 100.0));
    // Se sigue tirando de la esquina del cursor; la de salida se queda.
    ev(&mut g, &mut escena, mover(120.0, 120.0));
    let r = ultimo(&escena);
    assert_eq!((r.x, r.y, r.ancho, r.alto), (0.0, 0.0, 120.0, 120.0));
    ev(&mut g, &mut escena, soltar(120.0, 120.0));
    assert!(matches!(ultimo(&escena).figura, Figura::Rectangulo));
}

#[test]
fn una_ele_que_no_se_sostiene_no_hace_rectangulo() {
    // Caso negativo: se traza la L y se suelta sin pararse. La ventana no
    // llega a llamar al gesto, y el trazo se queda a mano.
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Lapiz);
    trazar_ele(&mut g, &mut escena);
    ev(&mut g, &mut escena, soltar(150.0, 100.0));
    assert!(matches!(ultimo(&escena).figura, Figura::Lapiz { .. }));
    // Y despues de soltar ya no hay nada que convertir.
    assert!(
        g.convertir_en_forma(&mut escena, Punto2::nuevo(150.0, 100.0), 1.0)
            .is_none()
    );
}

#[test]
fn clavar_el_cursor_y_arrastrar_abre_un_circulo_desde_ese_centro() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Lapiz);
    ev(&mut g, &mut escena, pulsar(50.0, 50.0));
    // Un temblor de nada: sigue siendo un toque, no un recorrido.
    ev(&mut g, &mut escena, mover(52.0, 51.0));
    assert!(
        g.convertir_en_forma(&mut escena, Punto2::nuevo(52.0, 51.0), 1.0)
            .is_some()
    );
    // Un ovalo de verdad, el de la herramienta de elipse, no un poligono.
    assert!(matches!(ultimo(&escena).figura, Figura::Elipse));
    ev(&mut g, &mut escena, mover(50.0, 150.0));
    let c = ultimo(&escena);
    assert_eq!((c.x, c.y, c.ancho, c.alto), (-50.0, -50.0, 200.0, 200.0));
    // Crece mientras se arrastra, clavado en su centro.
    ev(&mut g, &mut escena, mover(130.0, 50.0));
    let c = ultimo(&escena);
    assert_eq!((c.ancho, c.alto), (160.0, 160.0));
    assert_eq!((c.x + c.ancho / 2.0, c.y + c.alto / 2.0), (50.0, 50.0));
    ev(&mut g, &mut escena, soltar(130.0, 50.0));
    assert!(matches!(ultimo(&escena).figura, Figura::Elipse));
    assert_eq!(escena.cuantos_visibles(), 1);
}

#[test]
fn un_clic_quieto_sin_arrastrar_no_deja_circulo_ni_gasta_un_deshacer() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Lapiz);
    ev(&mut g, &mut escena, pulsar(50.0, 50.0));
    assert!(
        g.convertir_en_forma(&mut escena, Punto2::nuevo(50.0, 50.0), 1.0)
            .is_some()
    );
    ev(&mut g, &mut escena, soltar(50.0, 50.0));
    assert_eq!(escena.cuantos_visibles(), 0, "quedo un circulo de nada");
    assert!(!escena.deshacer(), "un clic no puede gastar un Ctrl+Z");
}

#[test]
fn un_trazo_normal_que_se_para_no_es_compas_ni_rectangulo() {
    // Caso negativo: una curva suave abierta (media luna) se para. No es un
    // punto, ni una L, ni una linea, ni esta cerrada: se queda como esta.
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Lapiz);
    ev(&mut g, &mut escena, pulsar(100.0, 0.0));
    for k in 1..=30 {
        let t = std::f32::consts::PI * k as f32 / 30.0;
        ev(&mut g, &mut escena, mover(100.0 * t.cos(), 100.0 * t.sin()));
    }
    assert!(
        g.convertir_en_forma(&mut escena, Punto2::nuevo(-100.0, 0.0), 1.0)
            .is_none()
    );
    assert!(matches!(ultimo(&escena).figura, Figura::Lapiz { .. }));
}

#[test]
fn el_compas_se_mide_en_pantalla_y_no_en_la_escena() {
    // 10 unidades de escena con zoom 3 (escala 1/3) son 30 px de mano: eso
    // ya es dibujar, no clavar la punta.
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Lapiz);
    ev(&mut g, &mut escena, pulsar(0.0, 0.0));
    ev(&mut g, &mut escena, mover(10.0, 0.0));
    let r = g.convertir_en_forma(&mut escena, Punto2::nuevo(10.0, 0.0), 1.0 / 3.0);
    assert!(
        r.is_none() || !matches!(ultimo(&escena).figura, Figura::Elipse),
        "abrio el compas con 30 px de recorrido"
    );
}

#[test]
fn el_compas_de_grafito_sale_de_grafito_y_el_del_resaltador_translucido() {
    for (h, grafito) in [
        (Herramienta::Grafito, true),
        (Herramienta::Resaltador, false),
    ] {
        let mut escena = Escena::nueva();
        let mut g = con(h);
        ev(&mut g, &mut escena, pulsar(0.0, 0.0));
        assert!(
            g.convertir_en_forma(&mut escena, Punto2::nuevo(0.0, 0.0), 1.0)
                .is_some(),
            "{h:?}"
        );
        ev(&mut g, &mut escena, mover(40.0, 0.0));
        let c = ultimo(&escena);
        assert!(matches!(c.figura, Figura::Elipse));
        assert_eq!(c.material == MaterialTinta::Cuadritos, grafito, "{h:?}");
        if h == Herramienta::Resaltador {
            assert!(c.opacidad < 0.5);
        }
    }
}

#[test]
fn deshacer_quita_el_circulo_del_compas_de_una_vez() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Lapiz);
    ev(&mut g, &mut escena, pulsar(0.0, 0.0));
    g.convertir_en_forma(&mut escena, Punto2::nuevo(0.0, 0.0), 1.0);
    ev(&mut g, &mut escena, mover(60.0, 0.0));
    ev(&mut g, &mut escena, soltar(60.0, 0.0));
    assert_eq!(escena.cuantos_visibles(), 1);
    assert!(escena.deshacer());
    assert_eq!(escena.cuantos_visibles(), 0);
}

/// Lo ancho que sale pintado un elemento: el alto del contorno de su tinta
/// o el grosor de sus rayas, y cuantas pasadas lleva.
fn ancho_pintado(e: &pixpin_motor2d::elemento::Elemento) -> (f32, usize) {
    use pixpin_motor2d::pintado::{Orden, ordenes};
    let mut ancho: f32 = 0.0;
    let mut pasadas = 0;
    for o in ordenes(e) {
        match o {
            Orden::Tinta { contorno, .. } => {
                // Solo la franja del medio: las tapas redondas no cuentan.
                let (mn, mx) = contorno
                    .iter()
                    .filter(|p| p.x > 60.0 && p.x < 140.0)
                    .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
                ancho = ancho.max(mx - mn);
                pasadas += 1;
            }
            Orden::Polilinea { grosor, .. } => {
                ancho = ancho.max(grosor);
                pasadas += 1;
            }
            _ => {}
        }
    }
    (ancho, pasadas)
}

#[test]
fn la_raya_que_sale_del_resaltador_al_pararse_es_tan_ancha_como_su_tinta() {
    // Lo que vio el usuario: con el resaltador grueso una raya a mano se veia
    // gorda y, al pararse y volverse recta, se quedaba en el grosor de una
    // linea (1/2/4) -«delgadisima»-. Tiene que seguir midiendo lo mismo.
    use pixpin_motor2d::estilo::NivelGrosor;
    for nivel in [NivelGrosor::Fino, NivelGrosor::Medio, NivelGrosor::Grueso] {
        let mut escena = Escena::nueva();
        let mut g = con(Herramienta::Resaltador);
        g.estilo.grosor = nivel;
        ev(&mut g, &mut escena, pulsar(0.0, 0.0));
        for k in 1..=40 {
            ev(&mut g, &mut escena, mover(k as f32 * 5.0, 0.0));
        }
        let (a_mano, _) = ancho_pintado(ultimo(&escena));
        // La tinta del movil adelgaza y engorda con la velocidad: a mano
        // mide del orden de su ancho tipico, no exactamente.
        let esperado = pixpin_motor2d::tinta::ancho_del_resaltador(nivel.de_resaltador());
        assert!(
            a_mano > esperado * 0.7 && a_mano < esperado * 1.5,
            "{nivel:?}: a mano {a_mano}, tipico {esperado}"
        );
        assert!(
            g.convertir_en_forma(&mut escena, Punto2::nuevo(200.0, 0.0), 1.0)
                .is_some()
        );
        ev(&mut g, &mut escena, mover(200.0, 0.0));
        ev(&mut g, &mut escena, soltar(200.0, 0.0));
        let raya = ultimo(&escena);
        assert!(
            matches!(raya.figura, Figura::Linea { .. }),
            "{:?}",
            raya.figura
        );
        let (recta, pasadas) = ancho_pintado(raya);
        assert!(
            (recta - esperado).abs() < 0.5,
            "{nivel:?}: la raya mide {recta} y la tinta {esperado}"
        );
        // Translucida y de UNA pasada: con dos pasadas a mano alzada el
        // cruce de las dos se veria mas oscuro que el resto de la raya.
        assert_eq!(pasadas, 1, "{nivel:?}");
        assert!(raya.opacidad < 0.5);
    }
}

#[test]
fn la_raya_que_sale_del_lapiz_al_pararse_sigue_con_el_grosor_de_las_lineas() {
    // Caso negativo: el arreglo es del resaltador. El lapiz se vuelve la
    // raya de la herramienta de linea, con su grosor, como en el movil.
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Lapiz);
    ev(&mut g, &mut escena, pulsar(0.0, 0.0));
    for k in 1..=40 {
        ev(&mut g, &mut escena, mover(k as f32 * 5.0, 0.0));
    }
    g.convertir_en_forma(&mut escena, Punto2::nuevo(200.0, 0.0), 1.0)
        .unwrap();
    let raya = ultimo(&escena);
    assert!(matches!(raya.figura, Figura::Linea { .. }));
    assert!(raya.grosor <= 4.0, "{}", raya.grosor);
    assert_eq!(raya.opacidad, 1.0);
}
