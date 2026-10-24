//! **La herramienta Grafito, contra la API publica del gesto**: pulsar, mover,
//! soltar, como el usuario.
//!
//! En el movil (v0.75-v0.80.2) el grafito es una herramienta y no un material
//! del panel: el trazo que deja es un `freedraw` con `material:"cuadritos"`,
//! salta con el el gesto de pararse (la figura sale de grafito), se pega al
//! iman como cualquier lapiz, y mientras se tiene en la mano las figuras
//! nuevas tambien salen de grafito. Todo eso se mira aqui.

use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::excalidraw;
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

/// Un gesto con la herramienta cogida como la coge la barra.
fn con(h: Herramienta) -> Gesto {
    let mut g = Gesto::nuevo();
    g.enganche.activo = false;
    g.tomar_herramienta(h);
    g
}

/// Arrastra de `a` a `b` en `pasos` y devuelve lo ultimo que nacio.
fn arrastrar(
    g: &mut Gesto,
    e: &mut Escena,
    a: (f32, f32),
    b: (f32, f32),
    pasos: usize,
) -> Elemento {
    ev(g, e, pulsar(a.0, a.1));
    for k in 1..=pasos {
        let t = k as f32 / pasos as f32;
        ev(g, e, mover(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
    }
    ev(g, e, soltar(b.0, b.1));
    e.elementos.last().expect("nacio algo").clone()
}

#[test]
fn dibujar_con_el_grafito_deja_un_lapiz_de_cuadritos_con_el_grosor_del_lapiz() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    let e = arrastrar(&mut g, &mut escena, (10.0, 10.0), (200.0, 60.0), 20);
    assert!(matches!(e.figura, Figura::Lapiz { .. }), "{:?}", e.figura);
    assert_eq!(e.material, MaterialTinta::Cuadritos);
    // El grosor es el del lapiz (las teclas 1, 2 y 3), no el de las figuras.
    assert_eq!(e.grosor, g.grosor_tinta);
}

#[test]
fn el_lapiz_despues_del_grafito_vuelve_a_la_tinta_del_panel() {
    // Caso negativo: el grafito no se queda pegado al lapiz de tinta.
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    arrastrar(&mut g, &mut escena, (10.0, 10.0), (100.0, 10.0), 10);
    g.tomar_herramienta(Herramienta::Lapiz);
    let e = arrastrar(&mut g, &mut escena, (10.0, 50.0), (100.0, 50.0), 10);
    assert_eq!(e.material, MaterialTinta::Lisa);
    assert!(!g.grafito_en_la_mano);
}

#[test]
fn con_el_grafito_en_la_mano_el_rectangulo_sale_de_grafito_hasta_coger_el_lapiz() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    // Ir a la mano a mover algo y volver no lo apaga.
    g.tomar_herramienta(Herramienta::Mano);
    g.tomar_herramienta(Herramienta::Rectangulo);
    let r = arrastrar(&mut g, &mut escena, (10.0, 10.0), (120.0, 90.0), 5);
    assert!(matches!(r.figura, Figura::Rectangulo));
    assert_eq!(r.material, MaterialTinta::Cuadritos);
    g.tomar_herramienta(Herramienta::Flecha);
    let f = arrastrar(&mut g, &mut escena, (200.0, 10.0), (300.0, 90.0), 5);
    assert_eq!(f.material, MaterialTinta::Cuadritos);
    // El resaltador lo deja, como en el movil.
    g.tomar_herramienta(Herramienta::Resaltador);
    g.tomar_herramienta(Herramienta::Rectangulo);
    let r2 = arrastrar(&mut g, &mut escena, (10.0, 200.0), (120.0, 290.0), 5);
    assert_eq!(r2.material, MaterialTinta::Lisa);
}

#[test]
fn sin_haber_cogido_el_grafito_las_figuras_salen_de_la_tinta_del_panel() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Rectangulo);
    let r = arrastrar(&mut g, &mut escena, (10.0, 10.0), (120.0, 90.0), 5);
    assert_eq!(r.material, MaterialTinta::Lisa);
    // Y el texto, aun con el grafito en la mano, no es una de sus figuras.
    assert!(!pixpin_motor2d::gesto::FIGURAS_DE_GRAFITO.contains(&Herramienta::Texto));
}

#[test]
fn el_trazo_de_grafito_en_curso_es_trazo_a_mano_para_la_tinta_viva() {
    // Es lo que lo excluye de la capa congelada y lo pinta en la capa de la
    // tinta mientras se dibuja, como al lapiz.
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    ev(&mut g, &mut escena, pulsar(10.0, 10.0));
    ev(&mut g, &mut escena, mover(40.0, 12.0));
    assert!(g.trazo_en_curso().is_some());
    ev(&mut g, &mut escena, soltar(40.0, 12.0));
    assert!(g.trazo_en_curso().is_none());
}

#[test]
fn el_gesto_de_pararse_con_grafito_da_una_recta_de_grafito() {
    // v0.80.2: «el grafito es un lapiz mas: gesto de pararse». Sin esto, con
    // el grafito en la mano no saltaba la recta.
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    ev(&mut g, &mut escena, pulsar(10.0, 10.0));
    for k in 1..=30 {
        ev(
            &mut g,
            &mut escena,
            mover(10.0 + k as f32 * 6.0, 10.0 + (k % 2) as f32 * 0.5),
        );
    }
    let r = g.convertir_en_forma(&mut escena, Punto2::nuevo(190.0, 10.0), 1.0);
    assert!(r.is_some(), "no salto la forma");
    let e = escena.elementos.last().unwrap();
    assert!(matches!(e.figura, Figura::Linea { .. }), "{:?}", e.figura);
    assert_eq!(
        e.material,
        MaterialTinta::Cuadritos,
        "la recta perdio el grafito"
    );
}

#[test]
fn el_gesto_de_pararse_tambien_salta_con_el_resaltador_y_no_le_quita_el_material() {
    // En el movil el resaltador es un FREEDRAW como el lapiz, y `latido` solo
    // mira `isFreeDraw`: el gesto salta con los tres. Antes el PC lo negaba
    // con el resaltador. Y la recta que sale de un lapiz de tinta sigue sin
    // ser de grafito.
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Resaltador);
    ev(&mut g, &mut escena, pulsar(10.0, 10.0));
    for k in 1..=30 {
        ev(&mut g, &mut escena, mover(10.0 + k as f32 * 6.0, 10.0));
    }
    assert!(
        g.convertir_en_forma(&mut escena, Punto2::nuevo(190.0, 10.0), 1.0)
            .is_some()
    );
    let e = escena.elementos.last().unwrap();
    assert!(matches!(e.figura, Figura::Linea { .. }), "{:?}", e.figura);
    assert!(e.opacidad < 0.5, "la raya del resaltador sigue translucida");
    assert_ne!(e.material, MaterialTinta::Cuadritos);
}

#[test]
fn el_iman_trata_al_grafito_igual_que_al_lapiz() {
    // v0.80.2: «iman». Con el iman encendido y una caja al lado, el primer
    // punto del grafito cae exactamente donde cae el del lapiz.
    let caja = Elemento {
        id: 7,
        figura: Figura::Rectangulo,
        x: 100.0,
        y: 100.0,
        ancho: 80.0,
        alto: 60.0,
        trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
        grosor: 2.0,
        ..Default::default()
    };
    let primer_punto = |h: Herramienta| {
        let mut escena = Escena::nueva();
        escena.elementos.push(caja.clone());
        let mut g = Gesto::nuevo();
        g.enganche.activo = true;
        g.tomar_herramienta(h);
        let e = arrastrar(&mut g, &mut escena, (103.0, 97.0), (150.0, 40.0), 6);
        e.puntos().expect("tiene puntos")[0]
    };
    assert_eq!(
        primer_punto(Herramienta::Grafito),
        primer_punto(Herramienta::Lapiz)
    );
    // Y el iman estaba encendido y la esquina a su alcance: la linea si se
    // pega. Que el trazo a mano no salte al vertice es la regla del PC para
    // los dos lapices (`enganche::Faena::AMano`: «un trazo que salta a un
    // vertice no se corrige, se rompe»); lo que se prueba es que el grafito
    // la sigue igual que el lapiz, no que se invente otra.
    assert_ne!(
        primer_punto(Herramienta::Linea),
        Punto2::nuevo(103.0, 97.0),
        "el iman no estaba encendido: la prueba no probaria nada"
    );
}

#[test]
fn el_json_del_grafito_es_el_del_movil_freedraw_con_material_cuadritos() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    arrastrar(&mut g, &mut escena, (10.0, 10.0), (200.0, 60.0), 8);
    let lienzo = excalidraw::con_escena(&excalidraw::leer("{\"elements\":[]}").unwrap(), &escena);
    let texto = excalidraw::escribir(&lienzo);
    let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
    let e = &v["elements"][0];
    assert_eq!(e["type"], "freedraw");
    assert_eq!(e["material"], "cuadritos");
    assert!(e["points"].is_array());
    // Y vuelve a leerse como grafito, no como tinta lisa.
    let vuelta = excalidraw::a_escena(&excalidraw::leer(&texto).unwrap());
    assert_eq!(vuelta.elementos[0].material, MaterialTinta::Cuadritos);
}

#[test]
fn un_grafito_del_movil_se_lee_como_grafito_y_se_devuelve_como_cuadritos() {
    // Lo que manda el telefono hoy: antes se leia como material desconocido
    // y se pintaba liso.
    let json = r##"{"type":"excalidraw","version":2,"elements":[
        {"id":"g1","type":"freedraw","x":10,"y":20,"width":50,"height":5,"angle":0,
         "strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid",
         "strokeWidth":1,"strokeStyle":"solid","roughness":1,"opacity":100,
         "seed":3,"version":4,"isDeleted":false,"groupIds":[],
         "points":[{"x":0,"y":0},{"x":25,"y":3},{"x":50,"y":5}],
         "pressures":[],"simulatePressure":true,"material":"cuadritos"}]}"##;
    let l = excalidraw::leer(json).unwrap();
    assert_eq!(l.elementos()[0].material, MaterialTinta::Cuadritos);
    let vuelta: serde_json::Value = serde_json::from_str(&excalidraw::escribir(&l)).unwrap();
    assert_eq!(vuelta["elements"][0]["material"], "cuadritos");
    // Y sus puntos siguen como objetos {x, y}, que es como los escribe el
    // movil (kotlinx): el PC edita sobre su JSON original.
    assert!(
        vuelta["elements"][0]["points"][1]["x"].is_number(),
        "{}",
        vuelta["elements"][0]["points"]
    );
}

/// Los topes valen para release; en depuracion se multiplican, como en
/// `puertas.rs`.
const FACTOR: u128 = if cfg!(debug_assertions) { 20 } else { 1 };

#[test]
fn arrastrar_una_figura_de_grafito_cuesta_menos_de_cuatro_milisegundos_por_fotograma() {
    // Mientras se arrastra para crearla, la figura cambia en cada fotograma y
    // se cuece entera cada vez (`pintar_copia_predicha` del editor). Esto es
    // lo que la deja verse de grafito en vivo sin que el lienzo vaya a
    // tirones en un i3 de tercera generacion, que va tres o cuatro veces mas
    // despacio que el equipo en el que se fijo el tope.
    use pixpin_motor2d::tinta::grafito;
    let caja = |ancho: f32, alto: f32| Elemento {
        id: 77,
        figura: Figura::Rectangulo,
        x: 20.0,
        y: 20.0,
        ancho,
        alto,
        grosor: 2.0,
        trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
        material: MaterialTinta::Cuadritos,
        ..Default::default()
    };
    let _ = grafito::cocer_sin_horno(&caja(100.0, 80.0));
    let mut mejor = std::time::Duration::MAX;
    for k in 0..8 {
        // Cada vuelta un poco mas grande, como al arrastrar.
        let e = caja(600.0 + k as f32 * 3.0, 400.0 + k as f32 * 2.0);
        let t = std::time::Instant::now();
        let c = grafito::cocer_sin_horno(&e).expect("se cuece");
        mejor = mejor.min(t.elapsed());
        assert!(c.rgba.iter().skip(3).step_by(4).any(|a| *a > 0));
    }
    println!("rectangulo de grafito de 600x400 en curso: {mejor:?}");
    assert!(
        mejor.as_micros() < 4_000 * FACTOR,
        "cocer la figura en curso cuesta {mejor:?}"
    );
}
