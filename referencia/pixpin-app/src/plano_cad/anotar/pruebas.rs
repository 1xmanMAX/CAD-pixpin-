//! Pruebas de anotar sobre un plano: el tamano del lapiz (12-oct-2026) y lo
//! anotado que viaja con el movil.

use super::*;
use pixpin_motor2d::estilo::NivelGrosor;

fn raya(id: u64, grosor: f32) -> pixpin_motor2d::Elemento {
    let mut e = pixpin_motor2d::zona::marca((0.0, 0.0, 10.0, 10.0), "x");
    e.id = id;
    e.grosor = grosor;
    e
}

#[test]
fn lo_elegido_en_el_panel_se_pone_a_la_escala_del_plano() {
    let k = 0.01;
    let mut visto = std::collections::HashMap::new();
    // Nace con el grosor «grueso» del panel: a la escala del plano.
    let grueso = NivelGrosor::Grueso.de_tinta();
    let mut v = vec![raya(1, grueso)];
    poner_a_escala(&mut v, &mut visto, k, 20.0);
    assert!((v[0].grosor - grueso * k).abs() < 1e-6);
    // Una vuelta sin cambios no lo vuelve a achicar.
    poner_a_escala(&mut v, &mut visto, k, 20.0);
    assert!((v[0].grosor - grueso * k).abs() < 1e-6);
    // Se elige «fino» en el panel con la raya elegida: tambien a escala
    // (antes se quedaba con el valor de una hoja, enorme en el plano).
    v[0].grosor = NivelGrosor::Fino.de_tinta();
    poner_a_escala(&mut v, &mut visto, k, 20.0);
    assert!((v[0].grosor - NivelGrosor::Fino.de_tinta() * k).abs() < 1e-6);
    // Un texto con la letra de 36 del panel.
    let mut t = raya(2, NivelGrosor::Fino.de_forma());
    t.figura = pixpin_motor2d::Figura::Texto {
        texto: "a".into(),
        tam: 36.0,
        familia: "Segoe UI".into(),
    };
    v.push(t);
    poner_a_escala(&mut v, &mut visto, k, 20.0);
    let pixpin_motor2d::Figura::Texto { tam, .. } = &v[1].figura else {
        panic!()
    };
    assert!((tam - 0.36).abs() < 1e-5, "{tam}");
    // Caso negativo: un cambio que no viene del panel (estirar el
    // texto, pegar algo ya a escala) se deja como esta.
    if let pixpin_motor2d::Figura::Texto { tam, .. } = &mut v[1].figura {
        *tam = 0.5;
    }
    v.push(raya(3, 0.0123));
    poner_a_escala(&mut v, &mut visto, k, 20.0);
    let pixpin_motor2d::Figura::Texto { tam, .. } = &v[1].figura else {
        panic!()
    };
    assert!((tam - 0.5).abs() < 1e-6);
    assert!((v[2].grosor - 0.0123).abs() < 1e-7);
}

// ------------------------------------------------ el lapiz, a su tamano

/// Una carpeta de pruebas propia y vacia.
pub(super) fn carpeta(etiqueta: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "pixpin-plano-anotar-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub(super) fn rutas_en(d: &Path) -> Rutas {
    Rutas {
        tinta: d.join("capa.excalidraw"),
        hoja: d.join("capa.hoja"),
    }
}

/// Un plano de 100 x 50 m dibujado en milimetros con letras de 2,5 mm (el
/// tramo de 2 a 4), como los de las muestras del usuario.
pub(super) fn plano_de_obra() -> Modelo {
    Modelo {
        caja: [-50_000.0, -25_000.0, 50_000.0, 25_000.0],
        tramos_letras: vec![
            pixpin_cad::modelo::Tramo {
                desde: 0,
                cuantos: 4000,
                caja: [0.0; 4],
                tamano: 2.0,
                clase: 0,
            },
            pixpin_cad::modelo::Tramo {
                desde: 0,
                cuantos: 300,
                caja: [0.0; 4],
                tamano: 64.0,
                clase: 0,
            },
        ],
        ..Default::default()
    }
}

/// La vista de una ventana de 1200 x 800 en el escritorio (0, 0) que mira
/// el centro del plano con `px` unidades del plano por pixel y la capa en
/// unidades de `unidad`.
pub(super) fn vista(px: f64, unidad: f64) -> VistaPlano {
    VistaPlano {
        centro: [0.0, 0.0],
        px,
        ancho: 1200,
        alto: 800,
        origen_pantalla: (0, 0),
        escala_por_cien: 100,
        unidad,
        claro: true,
    }
}

/// **Un trazo del raton** de (300, 500) a (900, 500), muestra a muestra,
/// con el lapiz del motor sobre el plano; devuelve el trazo que nacio.
pub(super) fn trazar(motor: &mut MotorDelPlano, v: &VistaPlano) -> Elemento {
    motor
        .tinta
        .gesto
        .tomar_herramienta(pixpin_motor2d::gesto::Herramienta::Lapiz);
    motor.evento(EventoPlano::Mover(300, 500), v);
    motor.evento(EventoPlano::Bajar(300, 500), v);
    for x in (303..=900).step_by(3) {
        // Un pulso de mano de verdad: medio pixel arriba y abajo.
        let y = 500 + (x / 3 % 2);
        motor.evento(EventoPlano::Mover(x, y), v);
    }
    motor.evento(EventoPlano::Subir(900, 500), v);
    motor
        .capa
        .escena
        .visibles()
        .filter(|e| matches!(e.figura, Figura::Lapiz { .. }))
        .last()
        .cloned()
        .expect("el lapiz dejo un trazo")
}

/// Lo gordo que se ve el trazo en la pantalla, en pixeles: el alto de su
/// contorno en el tramo del medio (lejos de las puntas).
fn gordo_en_pantalla(e: &Elemento, v: &VistaPlano) -> f32 {
    let Figura::Lapiz {
        puntos,
        presiones,
        opciones,
    } = &e.figura
    else {
        panic!()
    };
    let contorno = pixpin_motor2d::tinta::contorno_de_lapiz_firme(
        puntos,
        presiones,
        e.grosor,
        *opciones,
        e.extras.presion_firme,
    );
    let (x, y, zoom) = v.camara();
    let (mut arriba, mut abajo) = (f32::MAX, f32::MIN);
    for q in &contorno {
        let (px, py) = (
            ((q.x as f64 - x) * zoom) as f32,
            ((q.y as f64 - y) * zoom) as f32,
        );
        if (500.0..=700.0).contains(&px) {
            arriba = arriba.min(py);
            abajo = abajo.max(py);
        }
    }
    abajo - arriba
}

/// **El fallo del 12-oct-2026** («cuando pinto con el lapiz en el archivo
/// CAD esta desproporcionado»), reproducido: el mismo gesto del raton,
/// sobre el plano mirado a un tamano en que se leen sus letras (de 2,5 mm a
/// unos 20 pixeles), contra el mismo gesto en un lienzo. Con la unidad de
/// antes (la milesima del lado del plano: 100 mm) cada unidad de la capa son
/// 800 pixeles, y las medidas fijas del motor (el radio minimo del trazo,
/// 0,01 unidades) lo engordaban; con la de la letra (Android v0.115) se ve
/// como en el lienzo, a cualquier zoom.
#[test]
fn el_lapiz_sobre_el_plano_se_ve_como_en_un_lienzo_a_cualquier_zoom() {
    let d = carpeta("lapiz");
    let m = plano_de_obra();
    // El lienzo: una unidad, un pixel.
    let mut lienzo = MotorDelPlano::nuevo(rutas_en(&d.join("lienzo")), &m);
    let v = vista(1.0, 1.0);
    let referencia = gordo_en_pantalla(&trazar(&mut lienzo, &v), &v);
    assert!(
        referencia > 1.0 && referencia < 12.0,
        "el lapiz medio de un lienzo: {referencia}"
    );

    let letra = 2.5;
    let u = pixpin_cad::anotado::unidad_de_la_capa(&m);
    for (i, px) in [letra / 20.0 * 8.0, letra / 20.0, letra / 20.0 / 8.0]
        .into_iter()
        .enumerate()
    {
        let mut motor = MotorDelPlano::nuevo(rutas_en(&d.join(format!("ahora-{i}"))), &m);
        let v = vista(px, u);
        let gordo = gordo_en_pantalla(&trazar(&mut motor, &v), &v);
        println!("MEDIDA lapiz ahora px={px} gordo={gordo:.2} referencia={referencia:.2}");
        assert!(
            (gordo / referencia - 1.0).abs() < 0.2,
            "zoom {px}: {gordo} px contra {referencia} px en el lienzo"
        );
    }

    // Caso negativo, el fallo: con la unidad de antes, mirando a leer, el
    // mismo trazo sale mucho mas gordo que en el lienzo.
    let antes = pixpin_cad::anotado::unidad_antigua(&m);
    let mut motor = MotorDelPlano::nuevo(rutas_en(&d.join("antes")), &m);
    let v = vista(letra / 20.0, antes);
    let gordo = gordo_en_pantalla(&trazar(&mut motor, &v), &v);
    println!("MEDIDA lapiz antes gordo={gordo:.2} referencia={referencia:.2}");
    assert!(
        gordo > referencia * 2.0,
        "con la unidad de antes: {gordo} px contra {referencia} px"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn lo_anotado_se_guarda_con_su_hoja_y_lo_de_antes_se_lee_con_la_unidad_de_antes() {
    let d = carpeta("hoja");
    let m = plano_de_obra();
    let r = rutas_en(&d);
    // Abrir un plano y no hacer nada no deja ficheros.
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m);
    motor.terminar();
    assert!(!r.tinta.exists() && !r.hoja.exists());

    // Se dibuja: la capa y su hoja (la caja del plano en la unidad de la letra).
    let u = pixpin_cad::anotado::unidad_de_la_capa(&m);
    let v = vista(2.5 / 20.0, u);
    let trazo = trazar(&mut motor, &v);
    motor.terminar();
    let hoja = leer_hoja(&r.hoja).expect("hoja");
    let e = Encaje::de_la_hoja(hoja, m.caja).unwrap();
    assert!(
        (e.u - u).abs() / u < 1e-4 && e.cero_x.abs() < 1.0 && e.cero_y.abs() < 1.0,
        "{e:?}"
    );
    // Y al volver a abrir, el trazo esta donde estaba.
    let otra = MotorDelPlano::nuevo(r.clone(), &m);
    let leido = otra
        .capa
        .escena
        .visibles()
        .find(|x| matches!(x.figura, Figura::Lapiz { .. }))
        .unwrap();
    assert!((leido.x - trazo.x).abs() < 1e-3 && (leido.grosor - trazo.grosor).abs() < 1e-6);

    // Lo anotado antes sin hoja (en milesimas del lado del plano): cae en su
    // sitio y a su tamano; al guardarlo lleva su hoja, en sus unidades.
    let antigua = pixpin_cad::anotado::unidad_antigua(&m);
    std::fs::remove_file(&r.hoja).unwrap();
    let mut viejo = Capa::default();
    let mut raya = pixpin_motor2d::Elemento {
        x: 10.0,
        y: 20.0,
        ancho: 5.0,
        grosor: 0.5,
        figura: Figura::Linea {
            puntos: vec![Punto2::nuevo(10.0, 20.0), Punto2::nuevo(15.0, 20.0)],
        },
        ..Default::default()
    };
    raya.version = 1;
    viejo.escena.anadir(raya);
    viejo.sucia = true;
    std::fs::remove_file(&r.tinta).unwrap();
    viejo.guardar(&r.tinta).unwrap();
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m);
    let linea = motor
        .capa
        .escena
        .visibles()
        .find(|x| matches!(x.figura, Figura::Linea { .. }))
        .unwrap()
        .clone();
    let p = motor.memoria.a_plano([linea.x as f64, linea.y as f64]);
    assert!(
        (p[0] - 10.0 * antigua).abs() < 1e-2 && (p[1] + 20.0 * antigua).abs() < 1e-2,
        "{p:?}"
    );
    assert!((linea.grosor as f64 * u - 0.5 * antigua).abs() < 1e-3);
    motor.poner_cotas_y_marcos(&[vec![[0.0, 0.0], [1000.0, 0.0]]], &[]);
    let e = Encaje::de_la_hoja(leer_hoja(&r.hoja).unwrap(), m.caja).unwrap();
    assert!(
        (e.u - antigua).abs() / antigua < 1e-4,
        "la hoja dice las unidades del fichero: {e:?}"
    );
    // Lo que no se toco vuelve al fichero con sus numeros exactos.
    let texto = std::fs::read_to_string(&r.tinta).unwrap();
    assert!(
        texto.contains("\"x\": 10.0") || texto.contains("\"x\": 10"),
        "{texto}"
    );

    // Casos negativos: una hoja danada se ignora (regla de antes) y una capa
    // ilegible se lee vacia, sin romper nada.
    std::fs::write(&r.hoja, "esto,no,es\nv9\n").unwrap();
    let (c, _, f) = leer_capa(&r, &m);
    assert!((f.u - antigua).abs() < 1e-9 && !c.vacia());
    std::fs::write(&r.tinta, "{ roto").unwrap();
    let (c, _, _) = leer_capa(&r, &m);
    assert!(c.vacia());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn las_cotas_y_los_marcos_van_en_la_capa_como_en_android() {
    let d = carpeta("cotas");
    let m = plano_de_obra();
    let r = rutas_en(&d);
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m);
    let cota = vec![[100.0, 200.0], [1100.0, 200.0], [1100.0, 900.0]];
    let marco = [-2000.0, -1000.0, 3000.0, 1500.0];
    motor.poner_cotas_y_marcos(std::slice::from_ref(&cota), &[marco]);
    let (cs, ms) = motor.cotas_y_marcos();
    assert_eq!(cs.len(), 1);
    for (a, b) in cs[0].iter().zip(&cota) {
        assert!(
            (a[0] - b[0]).abs() < 1e-2 && (a[1] - b[1]).abs() < 1e-2,
            "{a:?} {b:?}"
        );
    }
    assert!(
        ms[0].iter().zip(marco).all(|(a, b)| (a - b).abs() < 1e-2),
        "{ms:?}"
    );
    // En el fichero: una raya y un marco con su grupo y el id de su forma.
    let texto = std::fs::read_to_string(&r.tinta).unwrap();
    let id_cota =
        pixpin_cad::anotado::huella(GRUPO_COTA, &[100.0, 200.0, 1100.0, 200.0, 1100.0, 900.0]);
    assert!(
        texto.contains(&id_cota) && texto.contains("plano-marco") && texto.contains("\"frame\""),
        "{texto}"
    );
    // Lo mismo otra vez no cambia nada (ni escribe).
    let antes = std::fs::metadata(&r.tinta).unwrap().modified().unwrap();
    motor.poner_cotas_y_marcos(std::slice::from_ref(&cota), &[marco]);
    assert_eq!(
        std::fs::metadata(&r.tinta).unwrap().modified().unwrap(),
        antes
    );
    // Quitar el marco lo deja borrado (viaja como borrado); volver a ponerlo, revive.
    motor.poner_cotas_y_marcos(std::slice::from_ref(&cota), &[]);
    let texto = std::fs::read_to_string(&r.tinta).unwrap();
    assert!(texto.contains("\"isDeleted\": true"), "{texto}");
    assert_eq!(
        MotorDelPlano::nuevo(r.clone(), &m).cotas_y_marcos().1.len(),
        0
    );
    motor.poner_cotas_y_marcos(&[cota], &[marco]);
    assert_eq!(
        MotorDelPlano::nuevo(r.clone(), &m).cotas_y_marcos().1.len(),
        1
    );
    // Caso negativo: lo anotado a mano no se toma por cota.
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m);
    let v = vista(2.5 / 20.0, pixpin_cad::anotado::unidad_de_la_capa(&m));
    trazar(&mut motor, &v);
    assert_eq!(motor.cotas_y_marcos().0.len(), 1);
    let _ = std::fs::remove_dir_all(&d);
}
