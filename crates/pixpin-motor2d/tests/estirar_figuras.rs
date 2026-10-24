//! **Estirar una figura hecha de muchas piezas** (grafica, tabla, figura de
//! la biblioteca): el bloque se estira como un todo, como en el movil
//! (`resizeMultipleElements`), y no cada pieza hacia el cursor.

use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
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

fn caja(x: f32, y: f32, w: f32, h: f32) -> Elemento {
    Elemento {
        figura: Figura::Rectangulo,
        x,
        y,
        ancho: w,
        alto: h,
        grosor: 1.0,
        ..Default::default()
    }
}

fn texto(x: f32, y: f32) -> Elemento {
    Elemento {
        figura: Figura::Texto {
            texto: "12".into(),
            tam: 20.0,
            familia: "Excalifont".into(),
        },
        x,
        y,
        ancho: 24.0,
        alto: 25.0,
        ..Default::default()
    }
}

/// Una escena con lo dado elegido y la mano puesta.
fn con(v: Vec<Elemento>) -> (Escena, Gesto, Vec<u64>) {
    let mut escena = Escena::nueva();
    let ids: Vec<u64> = v.into_iter().map(|e| escena.anadir(e)).collect();
    let mut g = Gesto::nuevo();
    g.enganche.activo = false;
    g.tomar_herramienta(Herramienta::Mano);
    g.seleccion.poner_todos(ids.clone());
    (escena, g, ids)
}

fn arrastrar(g: &mut Gesto, escena: &mut Escena, desde: (f32, f32), hasta: (f32, f32)) {
    ev(g, escena, pulsar(desde.0, desde.1));
    ev(
        g,
        escena,
        mover((desde.0 + hasta.0) / 2.0, (desde.1 + hasta.1) / 2.0),
    );
    ev(g, escena, mover(hasta.0, hasta.1));
    ev(g, escena, soltar(hasta.0, hasta.1));
}

fn geo(e: &Escena, id: u64) -> (f32, f32, f32, f32) {
    let x = e.buscar(id).unwrap();
    (x.x, x.y, x.ancho, x.alto)
}

#[test]
fn por_la_esquina_dos_cajas_crecen_juntas_y_en_proporcion() {
    // Lo que pasaba: cada caja se estiraba hasta el cursor y la segunda
    // acababa encima de la primera (a 0,0,400,200 y b 100,0,300,200).
    let (mut escena, mut g, ids) = con(vec![
        caja(0.0, 0.0, 100.0, 100.0),
        caja(100.0, 0.0, 100.0, 100.0),
    ]);
    let (_, _, x1, y1) = g.seleccion.caja(&escena).unwrap();
    // Se tira hacia la derecha y un poco abajo: manda el eje que mas cambio.
    arrastrar(&mut g, &mut escena, (x1, y1), (x1 * 2.0, y1 + 10.0));
    let (ax, ay, aw, ah) = geo(&escena, ids[0]);
    let (bx, by, bw, bh) = geo(&escena, ids[1]);
    assert!(
        ax.abs() < 2.0 && ay.abs() < 2.0,
        "el ancla se movio: {ax},{ay}"
    );
    assert!(
        (aw - ah).abs() < 0.5 && (bw - bh).abs() < 0.5,
        "se deformaron: {aw}x{ah} {bw}x{bh}"
    );
    assert!(aw > 150.0, "no crecio: {aw}");
    assert!(
        bx >= ax + aw - 3.0,
        "b se monto sobre a: a {ax}+{aw}, b {bx}"
    );
    assert!((by - ay).abs() < 0.5);
}

#[test]
fn por_un_lado_solo_se_estira_ese_eje_y_la_letra_no_se_deforma() {
    let (mut escena, mut g, ids) = con(vec![caja(0.0, 0.0, 200.0, 100.0), texto(20.0, 30.0)]);
    let (_, y0, x1, y1) = g.seleccion.caja(&escena).unwrap();
    let medio = (y0 + y1) / 2.0;
    arrastrar(&mut g, &mut escena, (x1, medio), (x1 + x1, medio + 40.0));
    let (_, _, w, h) = geo(&escena, ids[0]);
    assert!(w > 350.0, "no se ensancho: {w}");
    assert!(
        (h - 100.0).abs() < 1.5,
        "el alto cambio tirando de un lado: {h}"
    );
    let Figura::Texto { tam, .. } = &escena.buscar(ids[1]).unwrap().figura else {
        panic!()
    };
    assert_eq!(*tam, 20.0, "la letra crecio con un solo eje");
}

#[test]
fn estrechar_y_volver_a_ensanchar_deja_la_letra_como_estaba() {
    // Se calcula desde lo de al pulsar: paso a paso, la letra encogida al
    // estrechar no volveria a crecer.
    let (mut escena, mut g, ids) = con(vec![caja(0.0, 0.0, 200.0, 100.0), texto(20.0, 30.0)]);
    let (_, y0, x1, y1) = g.seleccion.caja(&escena).unwrap();
    let medio = (y0 + y1) / 2.0;
    ev(&mut g, &mut escena, pulsar(x1, medio));
    ev(&mut g, &mut escena, mover(x1 / 2.0, medio));
    let Figura::Texto { tam, .. } = &escena.buscar(ids[1]).unwrap().figura else {
        panic!()
    };
    assert!(*tam < 20.0, "estrechar no encogio la letra: {tam}");
    ev(&mut g, &mut escena, mover(x1, medio));
    ev(&mut g, &mut escena, soltar(x1, medio));
    let Figura::Texto { tam, .. } = &escena.buscar(ids[1]).unwrap().figura else {
        panic!()
    };
    assert!((*tam - 20.0).abs() < 0.01, "la letra no volvio: {tam}");
}

#[test]
fn una_raya_del_bloque_estira_sus_puntos_con_el() {
    let raya = Elemento {
        figura: Figura::Linea {
            puntos: vec![Punto2::nuevo(0.0, 50.0), Punto2::nuevo(200.0, 50.0)],
        },
        grosor: 1.0,
        ..Default::default()
    };
    let (mut escena, mut g, ids) = con(vec![caja(0.0, 0.0, 200.0, 100.0), raya]);
    let (x0, y0, x1, y1) = g.seleccion.caja(&escena).unwrap();
    arrastrar(
        &mut g,
        &mut escena,
        (x1, (y0 + y1) / 2.0),
        (x0 + (x1 - x0) * 2.0, (y0 + y1) / 2.0),
    );
    let Some(pts) = escena.buscar(ids[1]).unwrap().puntos() else {
        panic!()
    };
    assert!(pts[1].x > 380.0, "la raya no se estiro: {:?}", pts);
    assert!((pts[0].y - pts[1].y).abs() < 0.01);
}

#[test]
fn un_elemento_solo_se_sigue_estirando_como_siempre() {
    // Caso negativo: con uno solo no hay bloque; el rectangulo va por
    // `transformar::escalar` y por su esquina NO se fuerza la proporcion.
    let (mut escena, mut g, ids) = con(vec![caja(0.0, 0.0, 100.0, 100.0)]);
    let (_, _, x1, y1) = g.seleccion.caja(&escena).unwrap();
    arrastrar(&mut g, &mut escena, (x1, y1), (x1 + 100.0, y1));
    let (_, _, w, h) = geo(&escena, ids[0]);
    assert!(w > 190.0 && (h - y1).abs() < 2.0, "{w}x{h}");
}

#[test]
fn lo_bloqueado_del_bloque_no_se_estira() {
    let mut quieta = caja(100.0, 0.0, 100.0, 100.0);
    quieta.bloqueado = true;
    let originales = vec![caja(0.0, 0.0, 100.0, 100.0), quieta.clone()];
    let v = pixpin_motor2d::estirar_bloque::estirar(
        &originales,
        pixpin_geom::Tirador::SuresteEsquina,
        Punto2::nuevo(400.0, 200.0),
        None,
    );
    assert_eq!(
        (v[1].x, v[1].y, v[1].ancho, v[1].alto),
        (quieta.x, quieta.y, quieta.ancho, quieta.alto)
    );
    assert!(v[0].ancho > 150.0);
}
