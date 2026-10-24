//! **La cota pide su medida nada mas trazarla** (`pedirLaMedida` y
//! `aplicarCota` del movil), contra la API publica del gesto.

use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta, Peticion};
use pixpin_motor2d::medida::{angulo_de, longitud_de};
use pixpin_motor2d::vector::Punto2;

fn trazar(g: &mut Gesto, e: &mut Escena, de: (f32, f32), a: (f32, f32)) -> Option<Peticion> {
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
    g.evento(
        EventoGesto::Mover {
            p: Punto2::nuevo(a.0, a.1),
            shift: false,
            alt: false,
            presion: None,
        },
        e,
        1.0,
    );
    g.evento(
        EventoGesto::Soltar {
            p: Punto2::nuevo(a.0, a.1),
        },
        e,
        1.0,
    )
    .pide
}

fn con_cota() -> Gesto {
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Cota);
    // Sin iman: se mira lo que se dicta, no donde se pega el cursor.
    g.enganche.activo = false;
    g
}

#[test]
fn trazar_una_cota_pide_su_medida_y_lo_dictado_la_deja_anclada_por_su_principio() {
    let mut escena = Escena::nueva();
    let mut g = con_cota();
    let pide = trazar(&mut g, &mut escena, (100.0, 100.0), (187.0, 131.0));
    let Some(Peticion::DictarCota { id }) = pide else {
        panic!("no pidio la medida: {pide:?}");
    };
    assert!(Gesto::dictar_cota(&mut escena, id, 120.0, 30.0));
    let c = escena.buscar(id).unwrap();
    assert!((longitud_de(c) - 120.0).abs() < 0.01);
    assert!(
        (angulo_de(c) - 30.0).abs() < 0.01,
        "30 grados es hacia arriba"
    );
    assert_eq!(c.puntos().unwrap()[0], Punto2::nuevo(100.0, 100.0));
    // Un paso propio: deshacer vuelve a la raya trazada, no la borra.
    assert!(escena.deshacer());
    assert!(escena.buscar(id).is_some_and(|c| !c.borrado));
    assert!((longitud_de(escena.buscar(id).unwrap()) - 92.5).abs() < 0.5);
}

#[test]
fn con_el_interruptor_apagado_la_cota_mide_lo_que_hay_y_no_pregunta() {
    let mut escena = Escena::nueva();
    let mut g = con_cota();
    g.pedir_la_medida = false;
    assert_eq!(trazar(&mut g, &mut escena, (0.0, 0.0), (80.0, 0.0)), None);
}

#[test]
fn un_clic_con_la_cota_no_pide_nada_ni_otra_herramienta_tampoco() {
    let mut escena = Escena::nueva();
    let mut g = con_cota();
    assert_eq!(trazar(&mut g, &mut escena, (5.0, 5.0), (5.5, 5.0)), None);
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Linea);
    assert_eq!(trazar(&mut g, &mut escena, (0.0, 0.0), (80.0, 0.0)), None);
}

#[test]
fn dictar_numeros_imposibles_no_cambia_la_cota_ni_deja_paso() {
    let mut escena = Escena::nueva();
    let mut g = con_cota();
    let Some(Peticion::DictarCota { id }) = trazar(&mut g, &mut escena, (0.0, 0.0), (80.0, 0.0))
    else {
        panic!("no pidio la medida");
    };
    let pasos = escena.pasos_cerrados();
    assert!(!Gesto::dictar_cota(&mut escena, id, 0.0, 10.0));
    assert!(!Gesto::dictar_cota(&mut escena, 9999, 10.0, 10.0));
    assert_eq!(escena.pasos_cerrados(), pasos);
}
