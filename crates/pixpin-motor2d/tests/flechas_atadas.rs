//! **Las flechas atadas, como en Excalidraw**: se atan al dibujarlas, se
//! resalta a que se van a atar, siguen a la caja EN CADA AVISO del raton y
//! se sueltan si su punta se lleva lejos.
//!
//! Van aqui, contra la API publica del gesto, porque es lo que hace el
//! usuario: pulsar, mover, soltar. Una prueba de `enlace` a secas no habria
//! visto que la flecha solo se recolocaba al soltar, ni que un dibujo del
//! movil abierto con `a_escena` no encontraba nunca a sus cajas.

use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura, ModoEnganche};
use pixpin_motor2d::enlace;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::excalidraw;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::organizar::{self, Alineacion, Reparto};
use pixpin_motor2d::seleccion::Seleccion;
use pixpin_motor2d::transformar::{self, EjeVolteo};
use pixpin_motor2d::vector::Punto2;

fn caja(x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
    Elemento {
        figura: Figura::Rectangulo,
        x,
        y,
        ancho,
        alto,
        trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
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

fn soltar(g: &mut Gesto, e: &mut Escena, x: f32, y: f32) {
    g.evento(
        EventoGesto::Soltar {
            p: Punto2::nuevo(x, y),
        },
        e,
        1.0,
    );
}

/// Un gesto sin iman: las pruebas miden la atadura, y el iman moveria la
/// punta a la esquina o al medio mas cercano antes de que llegue a ella.
fn gesto(h: Herramienta) -> Gesto {
    let mut g = Gesto::nuevo();
    g.enganche.activo = false;
    g.herramienta = h;
    g
}

fn puntos(escena: &Escena, id: u64) -> Vec<Punto2> {
    match &escena.buscar(id).expect("sigue ahi").figura {
        Figura::Flecha { puntos, .. } => puntos.clone(),
        otra => panic!("no es una flecha: {otra:?}"),
    }
}

/// La ultima flecha de la escena: la que se acaba de dibujar.
fn ultima_flecha(escena: &Escena) -> u64 {
    escena
        .elementos
        .iter()
        .rev()
        .find(|e| matches!(e.figura, Figura::Flecha { .. }) && !e.borrado)
        .expect("se dibujo una flecha")
        .id
}

/// Dibuja una flecha de `a` a `b` con la herramienta de flecha.
fn dibujar_flecha(escena: &mut Escena, a: (f32, f32), b: (f32, f32)) -> u64 {
    let mut g = gesto(Herramienta::Flecha);
    pulsar(&mut g, escena, a.0, a.1);
    mover(&mut g, escena, (a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    mover(&mut g, escena, b.0, b.1);
    soltar(&mut g, escena, b.0, b.1);
    ultima_flecha(escena)
}

fn atados_de(escena: &Escena, id: u64) -> Vec<String> {
    escena
        .buscar(id)
        .unwrap()
        .extras
        .atados
        .iter()
        .filter(|a| a.tipo == "arrow")
        .map(|a| a.id.clone())
        .collect()
}

#[test]
fn dibujar_una_flecha_que_acaba_en_una_caja_la_ata_y_la_caja_la_apunta() {
    // Lo que el usuario echaba de menos: soltar la punta sobre la caja y que
    // quede UNIDA, con las dos mitades de la atadura escritas —la flecha
    // dice a quien se ata y la caja dice quien le cuelga—.
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));

    let flecha = escena.buscar(f).unwrap();
    let fin = flecha
        .extras
        .enganche_fin
        .as_ref()
        .expect("la punta tiene que quedar atada");
    let objetivo = escena.buscar(c).unwrap();
    assert_eq!(fin.elemento, enlace::id_de_texto_de(objetivo));
    assert!(
        flecha.extras.enganche_inicio.is_none(),
        "la salida cae en el vacio"
    );
    assert_eq!(
        atados_de(&escena, c),
        vec![enlace::id_de_texto_de(flecha)],
        "la caja no apunta la flecha en su boundElements"
    );
    // Y la punta se queda DONDE SE SOLTO (lo pidio el usuario el
    // 2026-09-23), no salta al borde como en Excalidraw.
    let punta = *puntos(&escena, f).last().unwrap();
    assert!(
        casi(punta, Punto2::nuevo(230.0, 50.0)),
        "la punta salto: {punta:?}"
    );
}

#[test]
fn una_flecha_que_sale_de_una_caja_y_llega_a_otra_se_ata_por_los_dos_lados() {
    let mut escena = Escena::nueva();
    let a = escena.anadir(caja(0.0, 0.0, 100.0, 100.0));
    let b = escena.anadir(caja(300.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (98.0, 50.0), (303.0, 50.0));
    let flecha = escena.buscar(f).unwrap();
    assert!(flecha.extras.enganche_inicio.is_some() && flecha.extras.enganche_fin.is_some());
    assert_eq!(atados_de(&escena, a).len(), 1);
    assert_eq!(atados_de(&escena, b).len(), 1);
}

#[test]
fn una_flecha_que_acaba_lejos_de_toda_caja_no_se_ata_a_nada() {
    // Caso negativo: soltar a una distancia razonable deja la punta donde
    // se solto y no toca la caja de al lado.
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let version_caja = escena.buscar(c).unwrap().version;
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (150.0, 50.0));
    let flecha = escena.buscar(f).unwrap();
    assert!(flecha.extras.enganche_inicio.is_none() && flecha.extras.enganche_fin.is_none());
    assert!(atados_de(&escena, c).is_empty());
    assert_eq!(
        escena.buscar(c).unwrap().version,
        version_caja,
        "se toco una caja a la que no se ato nada"
    );
    assert_eq!(
        *puntos(&escena, f).last().unwrap(),
        Punto2::nuevo(150.0, 50.0)
    );
}

#[test]
fn una_raya_no_se_ata_aunque_acabe_encima_de_una_caja() {
    // Caso negativo: en Excalidraw solo las flechas se atan. Una raya que
    // cruza una caja es un dibujo, no un conector.
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let mut g = gesto(Herramienta::Linea);
    pulsar(&mut g, &mut escena, 0.0, 50.0);
    mover(&mut g, &mut escena, 230.0, 50.0);
    assert!(
        g.candidatas.iter().all(Option::is_none),
        "una raya no resalta nada"
    );
    soltar(&mut g, &mut escena, 230.0, 50.0);
    assert!(atados_de(&escena, c).is_empty());
}

#[test]
fn mientras_se_dibuja_se_resalta_la_caja_a_la_que_se_va_a_atar() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let mut g = gesto(Herramienta::Flecha);
    pulsar(&mut g, &mut escena, 0.0, 50.0);
    assert!(
        g.resaltado_de_union(&escena, 1.0).is_empty(),
        "sale del vacio"
    );

    mover(&mut g, &mut escena, 230.0, 50.0);
    assert_eq!(g.candidatas[1], Some(c));
    assert!(
        !g.resaltado_de_union(&escena, 1.0).is_empty(),
        "la caja candidata no se resalta"
    );

    // Se aleja: el resaltado se apaga antes de soltar, que es cuando sirve.
    mover(&mut g, &mut escena, 150.0, 50.0);
    assert_eq!(g.candidatas[1], None);
    assert!(g.resaltado_de_union(&escena, 1.0).is_empty());

    mover(&mut g, &mut escena, 230.0, 50.0);
    soltar(&mut g, &mut escena, 230.0, 50.0);
    assert!(
        g.resaltado_de_union(&escena, 1.0).is_empty(),
        "el resaltado sobrevivio al gesto"
    );
}

#[test]
fn al_mover_la_caja_la_flecha_la_sigue_en_cada_aviso_y_no_solo_al_soltar() {
    // El fallo de fondo: `enlace::seguir` solo se llamaba al soltar, asi que
    // mientras se arrastraba la caja la flecha se quedaba apuntando al
    // vacio y saltaba al final.
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    let antes = puntos(&escena, f);

    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(c);
    pulsar(&mut g, &mut escena, 200.0, 20.0);
    assert_eq!(
        g.flechas_que_siguen(),
        &[f],
        "la flecha no se apunto al pulsar"
    );
    for paso in 1..=10 {
        mover(&mut g, &mut escena, 200.0, 50.0 + paso as f32 * 20.0);
        let ahora = puntos(&escena, f);
        let caja_y = escena.buscar(c).unwrap().y;
        assert!(
            ahora.last().unwrap().y > antes.last().unwrap().y,
            "aviso {paso}: la punta no siguio a la caja (caja en y={caja_y}, punta {:?})",
            ahora.last()
        );
        // La salida, que no esta atada, no se mueve.
        assert_eq!(ahora[0], antes[0], "aviso {paso}: la salida se movio");
    }
    soltar(&mut g, &mut escena, 200.0, 250.0);
    assert!(
        g.flechas_que_siguen().is_empty(),
        "la lista sobrevivio al gesto"
    );
    // Sigue atada despues de soltar.
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_some());

    // Y un solo Ctrl+Z devuelve caja y flecha a la vez.
    assert!(escena.deshacer());
    assert_eq!(escena.buscar(c).unwrap().y, 0.0);
    assert_eq!(puntos(&escena, f), antes, "un Ctrl+Z no devolvio la flecha");
}

#[test]
fn con_alt_al_estirar_la_caja_la_flecha_sigue_a_su_borde() {
    // Con Alt la punta va al borde (orbita) y lo sigue al estirar.
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha_con_alt(&mut escena, (0.0, 50.0), (230.0, 50.0));
    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(c);
    // El tirador del oeste, en el medio del lado izquierdo.
    let ts = g.tiradores(&escena, 1.0).unwrap();
    let oeste = ts
        .tamano
        .iter()
        .find(|(t, _)| *t == pixpin_geom::Tirador::OesteBorde)
        .map(|(_, p)| *p)
        .unwrap();
    pulsar(&mut g, &mut escena, oeste.x, oeste.y);
    mover(&mut g, &mut escena, 120.0, oeste.y);
    let punta = *puntos(&escena, f).last().unwrap();
    assert!(
        punta.x < 120.0 && punta.x > 110.0,
        "la punta no siguio al borde estirado hasta x=120: {punta:?}"
    );
    soltar(&mut g, &mut escena, 120.0, oeste.y);
}

#[test]
fn una_flecha_que_viaja_con_su_caja_no_se_re_traza() {
    // Caso negativo: caja y flecha elegidas juntas viajan enteras. Re-trazar
    // la flecha ademas la deformaria (`simultaneouslyUpdated`).
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    let antes = puntos(&escena, f);
    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner_todos(vec![c, f]);
    pulsar(&mut g, &mut escena, 200.0, 20.0);
    assert!(g.flechas_que_siguen().is_empty());
    mover(&mut g, &mut escena, 200.0, 120.0);
    let ahora = puntos(&escena, f);
    for (a, b) in antes.iter().zip(&ahora) {
        assert!((b.y - a.y - 100.0).abs() < 0.01 && (b.x - a.x).abs() < 0.01);
    }
    soltar(&mut g, &mut escena, 200.0, 120.0);
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_some());
}

#[test]
fn arrastrar_la_punta_fuera_de_la_caja_suelta_la_union_y_la_caja_la_olvida() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    let punta = *puntos(&escena, f).last().unwrap();

    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(f);
    assert!(
        g.tiradores_de_punta(&escena, 1.0).is_some(),
        "sin puntos que coger"
    );
    pulsar(&mut g, &mut escena, punta.x, punta.y);
    mover(&mut g, &mut escena, 120.0, 200.0);
    soltar(&mut g, &mut escena, 120.0, 200.0);

    let flecha = escena.buscar(f).unwrap();
    assert!(
        flecha.extras.enganche_fin.is_none(),
        "la punta sigue atada lejos de la caja"
    );
    assert_eq!(
        *puntos(&escena, f).last().unwrap(),
        Punto2::nuevo(120.0, 200.0)
    );
    assert!(
        atados_de(&escena, c).is_empty(),
        "la caja sigue creyendo que la flecha es suya"
    );

    // Y mover la caja ya no arrastra la flecha.
    let antes = puntos(&escena, f);
    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(c);
    pulsar(&mut g, &mut escena, 200.0, 20.0);
    mover(&mut g, &mut escena, 200.0, 300.0);
    soltar(&mut g, &mut escena, 200.0, 300.0);
    assert_eq!(puntos(&escena, f), antes);
}

#[test]
fn arrastrar_la_punta_de_una_flecha_suelta_hasta_una_caja_la_ata() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (120.0, 50.0));
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_none());

    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(f);
    pulsar(&mut g, &mut escena, 120.0, 50.0);
    mover(&mut g, &mut escena, 240.0, 40.0);
    assert_eq!(
        g.candidatas[1],
        Some(c),
        "al arrastrar la punta no se resalta la caja"
    );
    soltar(&mut g, &mut escena, 240.0, 40.0);
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_some());
    assert_eq!(atados_de(&escena, c).len(), 1);
}

#[test]
fn deshacer_la_flecha_atada_deja_la_caja_como_estaba() {
    // Si la flecha se deshace y la caja sigue apuntandola, el fichero dice
    // que le cuelga una flecha que no existe.
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let caja_antes = escena.buscar(c).unwrap().clone();
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    assert_eq!(atados_de(&escena, c).len(), 1);
    assert!(escena.deshacer(), "dibujar la flecha no dejo paso");
    assert!(escena.buscar(f).is_none_or(|e| e.borrado));
    assert_eq!(
        escena.buscar(c).unwrap(),
        &caja_antes,
        "un Ctrl+Z dejo la caja atada"
    );
    // Y rehacer lo devuelve todo.
    assert!(escena.rehacer());
    assert_eq!(atados_de(&escena, c).len(), 1);
}

#[test]
fn mover_la_flecha_sola_lejos_la_suelta_y_poco_la_deja_atada() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));

    // Poco: la punta sigue encima de la caja.
    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(f);
    pulsar(&mut g, &mut escena, 60.0, 50.0);
    mover(&mut g, &mut escena, 64.0, 60.0);
    soltar(&mut g, &mut escena, 64.0, 60.0);
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_some());
    assert_eq!(atados_de(&escena, c).len(), 1);

    // Lejos: se suelta, y la caja la olvida.
    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(f);
    pulsar(&mut g, &mut escena, 64.0, 60.0);
    mover(&mut g, &mut escena, 64.0, 400.0);
    soltar(&mut g, &mut escena, 64.0, 400.0);
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_none());
    assert!(atados_de(&escena, c).is_empty());
}

#[test]
fn un_clic_sobre_la_flecha_atada_sin_arrastrar_no_deja_paso_de_deshacer() {
    // Caso negativo: re-atar al soltar solo si hubo arrastre, o cada clic
    // de seleccion dejaria un Ctrl+Z que no deshace nada visible.
    let mut escena = Escena::nueva();
    escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    let pasos = escena.pasos_cerrados();
    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(f);
    pulsar(&mut g, &mut escena, 60.0, 50.0);
    soltar(&mut g, &mut escena, 60.0, 50.0);
    assert_eq!(escena.pasos_cerrados(), pasos);
}

#[test]
fn escape_a_mitad_de_arrastre_devuelve_la_flecha_que_seguia() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    let antes = puntos(&escena, f);
    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(c);
    pulsar(&mut g, &mut escena, 200.0, 20.0);
    mover(&mut g, &mut escena, 200.0, 250.0);
    g.evento(EventoGesto::Escape, &mut escena, 1.0);
    assert_eq!(puntos(&escena, f), antes);
    assert!(g.flechas_que_siguen().is_empty());
}

/// Un organigrama del movil: dos cajas y una flecha entre ellas, con los
/// puntos como objetos y las ataduras escritas como las escribe kotlinx.
const DEL_MOVIL: &str = r##"{"type":"excalidraw","elements":[
  {"id":"caja-a","type":"rectangle","x":0,"y":0,"width":100,"height":60,
   "strokeColor":"#000000","strokeWidth":2,"seed":1,
   "boundElements":[{"id":"flecha-1","type":"arrow"}]},
  {"id":"caja-b","type":"rectangle","x":300,"y":0,"width":100,"height":60,
   "strokeColor":"#000000","strokeWidth":2,"seed":2,
   "boundElements":[{"id":"flecha-1","type":"arrow"}]},
  {"id":"flecha-1","type":"arrow","x":106,"y":30,"width":188,"height":0,
   "strokeColor":"#000000","strokeWidth":2,"seed":3,
   "points":[{"x":0,"y":0},{"x":188,"y":0}],
   "startArrowhead":null,"endArrowhead":"arrow",
   "startBinding":{"elementId":"caja-a","focus":0,"gap":6,"fixedPoint":null,"mode":"orbit"},
   "endBinding":{"elementId":"caja-b","focus":0,"gap":6,"fixedPoint":null,"mode":"orbit"}}
]}"##;

fn por_id<'a>(v: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no esta {id}"))
}

#[test]
fn en_un_dibujo_del_movil_la_flecha_sigue_a_la_caja_que_se_mueve() {
    // **La causa de fondo de «las flechas no se sujetan».** Un dibujo del
    // movil se abre para editar con `a_escena`, que numera los elementos
    // 1, 2, 3...; el enganche se buscaba por el FNV de su texto, que no es
    // ninguno de esos numeros, y la flecha no encontraba nunca a su caja.
    let lienzo = excalidraw::leer(DEL_MOVIL).unwrap();
    let mut escena = excalidraw::a_escena(&lienzo);
    let caja_b = escena
        .elementos
        .iter()
        .find(|e| e.extras.id_de_fichero.as_deref() == Some("caja-b"))
        .unwrap()
        .id;
    let flecha = ultima_flecha(&escena);
    let antes = puntos(&escena, flecha);

    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(caja_b);
    pulsar(&mut g, &mut escena, 325.0, 0.0);
    mover(&mut g, &mut escena, 325.0, 200.0);
    assert!(
        puntos(&escena, flecha).last().unwrap().y > antes.last().unwrap().y + 100.0,
        "la flecha del movil no siguio a su caja"
    );
    soltar(&mut g, &mut escena, 325.0, 200.0);

    // Y vuelve al movil con los nombres y las formas de siempre: puntos
    // como objetos, ataduras por el id de texto, `boundElements` intacto.
    let json: serde_json::Value = serde_json::from_str(&excalidraw::escribir(
        &excalidraw::con_escena(&lienzo, &escena),
    ))
    .unwrap();
    let f = por_id(&json, "flecha-1");
    assert!(
        f["points"][0].is_object(),
        "los puntos del movil dejaron de ser objetos"
    );
    assert_eq!(f["startBinding"]["elementId"], "caja-a");
    assert_eq!(f["endBinding"]["elementId"], "caja-b");
    assert_eq!(f["endBinding"]["mode"], "orbit");
    assert!(f["endBinding"]["focus"].is_number() && f["endBinding"]["gap"].is_number());
    assert_eq!(
        por_id(&json, "caja-b")["boundElements"],
        serde_json::json!([{"id": "flecha-1", "type": "arrow"}])
    );
}

#[test]
fn una_flecha_nueva_atada_a_una_caja_del_movil_viaja_con_los_ids_del_fichero() {
    let lienzo = excalidraw::leer(DEL_MOVIL).unwrap();
    let mut escena = excalidraw::a_escena(&lienzo);
    // Debajo de la caja A, hasta su borde de abajo.
    let nueva = dibujar_flecha(&mut escena, (50.0, 300.0), (50.0, 58.0));
    assert!(escena.buscar(nueva).unwrap().extras.enganche_fin.is_some());

    let json: serde_json::Value = serde_json::from_str(&excalidraw::escribir(
        &excalidraw::con_escena(&lienzo, &escena),
    ))
    .unwrap();
    let texto_nueva = enlace::id_de_texto(nueva);
    let f = por_id(&json, &texto_nueva);
    assert_eq!(f["type"], "arrow");
    assert_eq!(
        f["endBinding"]["elementId"], "caja-a",
        "ata al id del fichero"
    );
    assert!(f["startBinding"].is_null());
    assert!(
        f["points"][0].is_object(),
        "en un lienzo del movil los puntos van como objetos"
    );
    let atados = por_id(&json, "caja-a")["boundElements"]
        .as_array()
        .unwrap()
        .clone();
    assert!(atados.contains(&serde_json::json!({"id": "flecha-1", "type": "arrow"})));
    assert!(
        atados.contains(&serde_json::json!({"id": texto_nueva, "type": "arrow"})),
        "la caja del movil no apunta la flecha nueva: {atados:?}"
    );
    // La caja B no tiene nada que ver: sale tal cual entro.
    assert_eq!(
        por_id(&json, "caja-b"),
        por_id(&serde_json::from_str(DEL_MOVIL).unwrap(), "caja-b")
    );
}

// ---------------------------------------------------------------------------
// Alt: atar DENTRO (`bindMode: "inside"` de Excalidraw)
// ---------------------------------------------------------------------------

fn mover_con_alt(g: &mut Gesto, e: &mut Escena, x: f32, y: f32) {
    g.evento(
        EventoGesto::Mover {
            p: Punto2::nuevo(x, y),
            shift: false,
            alt: true,
            presion: None,
        },
        e,
        1.0,
    );
}

/// Dibuja una flecha de `a` a `b` manteniendo Alt al llegar.
fn dibujar_flecha_con_alt(escena: &mut Escena, a: (f32, f32), b: (f32, f32)) -> u64 {
    let mut g = gesto(Herramienta::Flecha);
    pulsar(&mut g, escena, a.0, a.1);
    mover(&mut g, escena, (a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    mover_con_alt(&mut g, escena, b.0, b.1);
    soltar(&mut g, escena, b.0, b.1);
    ultima_flecha(escena)
}

/// Mueve `id` con la mano, cogiendolo por `agarre`.
fn arrastrar(escena: &mut Escena, id: u64, agarre: (f32, f32), d: (f32, f32)) {
    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(id);
    pulsar(&mut g, escena, agarre.0, agarre.1);
    mover(&mut g, escena, agarre.0 + d.0, agarre.1 + d.1);
    soltar(&mut g, escena, agarre.0 + d.0, agarre.1 + d.1);
}

fn casi(a: Punto2, b: Punto2) -> bool {
    a.distancia(b) < 0.5
}

#[test]
fn la_punta_se_ata_dentro_justo_donde_se_solto() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (240.0, 30.0));

    let fin = escena
        .buscar(f)
        .unwrap()
        .extras
        .enganche_fin
        .clone()
        .expect("atada");
    assert_eq!(fin.modo, ModoEnganche::Dentro);
    let (fx, fy) = fin.punto_fijo.expect("dentro guarda el punto agarrado");
    assert!(
        (fx - 0.4).abs() < 1e-3 && (fy - 0.3).abs() < 1e-3,
        "{fx},{fy}"
    );
    // No salta al borde: se queda donde se solto.
    let punta = *puntos(&escena, f).last().unwrap();
    assert!(
        casi(punta, Punto2::nuevo(240.0, 30.0)),
        "la punta salto: {punta:?}"
    );
    assert_eq!(atados_de(&escena, c).len(), 1, "la caja no la apunta");
}

#[test]
fn la_punta_atada_dentro_sigue_al_mismo_sitio_de_la_caja_al_moverla() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (240.0, 30.0));
    arrastrar(&mut escena, c, (200.0, 70.0), (50.0, 100.0));
    let punta = *puntos(&escena, f).last().unwrap();
    assert!(
        casi(punta, Punto2::nuevo(290.0, 130.0)),
        "no siguio su sitio: {punta:?}"
    );
}

#[test]
fn con_alt_la_misma_suelta_se_ata_en_orbita_al_borde() {
    // Con Alt, lo de Excalidraw: al borde, aunque se suelte bien adentro.
    let mut escena = Escena::nueva();
    escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha_con_alt(&mut escena, (0.0, 50.0), (240.0, 30.0));
    let fin = escena
        .buscar(f)
        .unwrap()
        .extras
        .enganche_fin
        .clone()
        .expect("atada");
    assert_eq!(fin.modo, ModoEnganche::Orbita);
    let punta = *puntos(&escena, f).last().unwrap();
    assert!(punta.x < 200.0, "en orbita la punta va al borde: {punta:?}");
}

#[test]
fn con_alt_en_el_vacio_no_se_ata_a_nada() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha_con_alt(&mut escena, (0.0, 50.0), (120.0, 200.0));
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_none());
    assert!(atados_de(&escena, c).is_empty());
}

#[test]
fn arrastrar_la_punta_de_una_flecha_suelta_la_ata_dentro_donde_se_suelta() {
    let mut escena = Escena::nueva();
    escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (120.0, 50.0));
    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner(f);
    pulsar(&mut g, &mut escena, 120.0, 50.0);
    mover(&mut g, &mut escena, 260.0, 80.0);
    soltar(&mut g, &mut escena, 260.0, 80.0);
    let fin = escena
        .buscar(f)
        .unwrap()
        .extras
        .enganche_fin
        .clone()
        .expect("atada");
    assert_eq!(fin.modo, ModoEnganche::Dentro);
    assert!(casi(
        *puntos(&escena, f).last().unwrap(),
        Punto2::nuevo(260.0, 80.0)
    ));
}

#[test]
fn la_punta_dentro_viaja_al_movil_como_inside_con_su_fixed_point() {
    let lienzo = excalidraw::leer(DEL_MOVIL).unwrap();
    let mut escena = excalidraw::a_escena(&lienzo);
    // Dentro de la caja B (300..400 x 0..60).
    let nueva = dibujar_flecha(&mut escena, (350.0, 300.0), (325.0, 30.0));
    let json: serde_json::Value = serde_json::from_str(&excalidraw::escribir(
        &excalidraw::con_escena(&lienzo, &escena),
    ))
    .unwrap();
    let f = por_id(&json, &enlace::id_de_texto(nueva));
    assert_eq!(f["endBinding"]["elementId"], "caja-b");
    assert_eq!(f["endBinding"]["mode"], "inside");
    let fijo = f["endBinding"]["fixedPoint"]
        .as_array()
        .expect("fixedPoint [x, y]");
    assert!((fijo[0].as_f64().unwrap() - 0.25).abs() < 1e-3);
    assert!((fijo[1].as_f64().unwrap() - 0.5).abs() < 1e-3);
}

// ---------------------------------------------------------------------------
// Re-trazar en todo lo que mueve, no solo en el arrastre
// ---------------------------------------------------------------------------

fn seleccion(ids: &[u64]) -> Seleccion {
    let mut s = Seleccion::nueva();
    s.poner_todos(ids.iter().copied());
    s
}

#[test]
fn alinear_la_caja_se_lleva_su_flecha_y_un_ctrl_z_devuelve_las_dos() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let d = escena.anadir(caja(400.0, 200.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    let antes = puntos(&escena, f);

    organizar::alinear(&mut escena, &seleccion(&[c, d]), Alineacion::Abajo);
    let punta = *puntos(&escena, f).last().unwrap();
    assert!(
        punta.y > 150.0,
        "la flecha no siguio a la caja alineada: {punta:?}"
    );
    assert_eq!(
        puntos(&escena, f)[0],
        antes[0],
        "la salida suelta no se mueve"
    );

    assert!(escena.deshacer());
    assert_eq!(
        puntos(&escena, f),
        antes,
        "hizo falta un segundo Ctrl+Z para la flecha"
    );
    assert_eq!(escena.buscar(c).unwrap().y, 0.0);
}

#[test]
fn repartir_tambien_se_lleva_las_flechas_atadas() {
    let mut escena = Escena::nueva();
    let a = escena.anadir(caja(0.0, 300.0, 50.0, 50.0));
    let b = escena.anadir(caja(100.0, 300.0, 50.0, 50.0));
    let c = escena.anadir(caja(400.0, 300.0, 50.0, 50.0));
    // Flecha desde arriba hasta la de en medio.
    let f = dibujar_flecha(&mut escena, (125.0, 100.0), (125.0, 320.0));
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_some());

    organizar::repartir(&mut escena, &seleccion(&[a, b, c]), Reparto::Horizontal);
    // B pasa a estar centrada en 225: la punta va con ella.
    let punta = *puntos(&escena, f).last().unwrap();
    assert!(
        punta.x > 180.0,
        "la flecha se quedo donde estaba B: {punta:?}"
    );
}

#[test]
fn alinear_una_flecha_lejos_de_su_caja_la_suelta() {
    // La otra mitad: lo que se mueve es la flecha y no su caja. Como al
    // soltarla tras arrastrarla, si su punta ya no cae encima se suelta; si
    // no, la proxima vez que se moviera la caja volveria sola a ella.
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let d = escena.anadir(caja(0.0, 500.0, 50.0, 50.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    organizar::alinear(&mut escena, &seleccion(&[f, d]), Alineacion::Abajo);
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_none());
    assert!(
        atados_de(&escena, c).is_empty(),
        "la caja sigue apuntando la flecha"
    );
}

#[test]
fn alinear_lo_que_no_tiene_flechas_no_toca_las_flechas_de_otros() {
    // Caso negativo: una flecha atada a una caja que no se mueve no cambia
    // ni de version, que es lo que el movil mira para saber que cambio.
    let mut escena = Escena::nueva();
    escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    let a = escena.anadir(caja(0.0, 300.0, 50.0, 50.0));
    let b = escena.anadir(caja(100.0, 400.0, 50.0, 50.0));
    let flecha_antes = escena.buscar(f).unwrap().clone();
    organizar::alinear(&mut escena, &seleccion(&[a, b]), Alineacion::Izquierda);
    assert_eq!(escena.buscar(f).unwrap(), &flecha_antes);
}

/// Lo mismo que hace la ventana al voltear (`volteando`): sacar, voltear,
/// devolver y atar, en un paso.
fn voltear_en(escena: &mut Escena, ids: &[u64]) {
    escena.abrir_paso();
    for &id in ids {
        escena.apuntar_edicion(id);
    }
    let sitios: Vec<usize> = escena
        .elementos
        .iter()
        .enumerate()
        .filter(|(_, e)| ids.contains(&e.id))
        .map(|(i, _)| i)
        .collect();
    let mut copia: Vec<Elemento> = sitios
        .iter()
        .map(|&i| escena.elementos[i].clone())
        .collect();
    transformar::voltear(&mut copia, EjeVolteo::Horizontal);
    for (&i, e) in sitios.iter().zip(copia) {
        escena.elementos[i] = e;
    }
    transformar::atar_tras_voltear(escena, ids);
    escena.cerrar_paso();
}

#[test]
fn voltear_las_cajas_se_lleva_la_flecha_que_cuelga_de_ellas() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let d = escena.anadir(caja(500.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (100.0, 300.0), (230.0, 90.0));
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_some());
    voltear_en(&mut escena, &[c, d]);
    // C paso a 400..500: la punta va con ella.
    let punta = *puntos(&escena, f).last().unwrap();
    assert!(
        punta.x > 380.0,
        "la flecha se quedo en el sitio viejo de C: {punta:?}"
    );
    assert!(escena.deshacer());
    assert_eq!(escena.buscar(c).unwrap().x, 200.0);
    assert!(
        puntos(&escena, f).last().unwrap().x < 300.0,
        "Ctrl+Z dejo la flecha volteada"
    );
}

#[test]
fn voltear_la_flecha_con_su_caja_la_deja_atada_al_lado_nuevo() {
    // Atada dentro a un 40 % de la caja; volteado todo, ese sitio es el
    // 60 %. Si el enganche se quedara con el 40 %, mover la caja despues
    // haria saltar la punta al otro lado.
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (240.0, 30.0));
    voltear_en(&mut escena, &[c, f]);
    let fin = escena
        .buscar(f)
        .unwrap()
        .extras
        .enganche_fin
        .clone()
        .expect("sigue atada");
    let (fx, _) = fin.punto_fijo.unwrap();
    assert!((fx - 0.6).abs() < 1e-3, "el punto fijo no se volteo: {fx}");
    let (x0, ..) = escena.buscar(c).unwrap().caja();
    arrastrar(&mut escena, c, (x0, 70.0), (0.0, 100.0));
    let punta = *puntos(&escena, f).last().unwrap();
    assert!(
        casi(punta, Punto2::nuevo(x0 + 60.0, 130.0)),
        "la punta salto: {punta:?}"
    );
}

// ---------------------------------------------------------------------------
// Borrar suelta las ataduras
// ---------------------------------------------------------------------------

fn suprimir(escena: &mut Escena, ids: &[u64]) {
    let mut g = gesto(Herramienta::Mano);
    g.seleccion.poner_todos(ids.iter().copied());
    g.evento(EventoGesto::Suprimir, escena, 1.0);
}

#[test]
fn borrar_la_caja_suelta_la_flecha_y_un_ctrl_z_la_vuelve_a_atar() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    let enganche_antes = escena.buscar(f).unwrap().extras.enganche_fin.clone();
    let puntos_antes = puntos(&escena, f);

    suprimir(&mut escena, &[c]);
    assert!(
        escena.buscar(f).unwrap().extras.enganche_fin.is_none(),
        "atada a una caja borrada"
    );
    assert_eq!(puntos(&escena, f), puntos_antes, "soltarla no la mueve");

    assert!(escena.deshacer(), "no quedo paso");
    assert_eq!(
        escena.buscar(f).unwrap().extras.enganche_fin,
        enganche_antes
    );
    assert!(!escena.buscar(c).unwrap().borrado);
}

#[test]
fn borrar_la_flecha_la_quita_de_la_lista_de_su_caja() {
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    suprimir(&mut escena, &[f]);
    assert!(
        atados_de(&escena, c).is_empty(),
        "la caja apunta una flecha borrada"
    );
    assert!(escena.deshacer());
    assert_eq!(
        atados_de(&escena, c).len(),
        1,
        "Ctrl+Z no devolvio la atadura"
    );
}

#[test]
fn borrar_sin_paso_abierto_tambien_suelta_y_se_deshace_de_una_vez() {
    // Los pines y la capa borran sin abrir paso: tiene que quedar uno solo,
    // con la caja y la flecha dentro.
    let mut escena = Escena::nueva();
    let c = escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    assert!(escena.borrar_apuntando(c));
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_none());
    assert!(escena.deshacer());
    assert!(!escena.buscar(c).unwrap().borrado);
    assert!(escena.buscar(f).unwrap().extras.enganche_fin.is_some());
}

#[test]
fn borrar_una_caja_sin_flechas_no_toca_las_flechas_de_otras() {
    let mut escena = Escena::nueva();
    escena.anadir(caja(200.0, 0.0, 100.0, 100.0));
    let f = dibujar_flecha(&mut escena, (0.0, 50.0), (230.0, 50.0));
    let otra = escena.anadir(caja(0.0, 400.0, 50.0, 50.0));
    let flecha_antes = escena.buscar(f).unwrap().clone();
    suprimir(&mut escena, &[otra]);
    assert_eq!(escena.buscar(f).unwrap(), &flecha_antes);
}

#[test]
fn borrar_una_caja_del_movil_deja_su_flecha_sin_esa_union_en_el_fichero() {
    let lienzo = excalidraw::leer(DEL_MOVIL).unwrap();
    let mut escena = excalidraw::a_escena(&lienzo);
    let caja_a = escena
        .elementos
        .iter()
        .find(|e| e.extras.id_de_fichero.as_deref() == Some("caja-a"))
        .unwrap()
        .id;
    suprimir(&mut escena, &[caja_a]);
    let json: serde_json::Value = serde_json::from_str(&excalidraw::escribir(
        &excalidraw::con_escena(&lienzo, &escena),
    ))
    .unwrap();
    let f = por_id(&json, "flecha-1");
    assert!(f["startBinding"].is_null(), "sigue atada a la caja borrada");
    assert_eq!(
        f["endBinding"]["elementId"], "caja-b",
        "la otra punta no se toca"
    );
}

/// El caso del usuario, tal cual: «si inicie al centro de una figura ahi se
/// sujeta, y si lo solte en la esquina de una figura ahi se queda».
#[test]
fn la_flecha_se_queda_en_el_centro_y_en_la_esquina_donde_se_puso_al_mover_las_cajas() {
    let mut escena = Escena::nueva();
    let a = escena.anadir(caja(0.0, 0.0, 100.0, 100.0));
    let b = escena.anadir(caja(300.0, 200.0, 100.0, 100.0));
    // Del centro de A a la esquina de arriba a la izquierda de B (un poco
    // dentro, que es donde cae el raton al apuntar a una esquina).
    let f = dibujar_flecha(&mut escena, (50.0, 50.0), (303.0, 203.0));
    let flecha = escena.buscar(f).unwrap();
    assert!(
        flecha.extras.enganche_inicio.is_some(),
        "la salida no se sujeto al centro"
    );
    assert!(
        flecha.extras.enganche_fin.is_some(),
        "la punta no se sujeto a la esquina"
    );

    // Se cogen por el borde, lejos de los tiradores: por el del medio de
    // un lado se estirarian en vez de moverse.
    arrastrar(&mut escena, a, (0.0, 20.0), (40.0, -30.0));
    arrastrar(&mut escena, b, (400.0, 220.0), (-60.0, 80.0));
    let p = puntos(&escena, f);
    assert!(
        casi(p[0], Punto2::nuevo(90.0, 20.0)),
        "la salida no sigue al centro: {:?}",
        p[0]
    );
    let fin = *p.last().unwrap();
    assert!(
        casi(fin, Punto2::nuevo(243.0, 283.0)),
        "la punta no sigue a la esquina: {fin:?}"
    );
}
