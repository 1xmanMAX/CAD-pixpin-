//! **El grafito con durezas y el modo guia** (Tanda 3 de
//! `docs/investigacion/2026-10-13-android-vs-pc.md`), contra la API publica.
//!
//! Lo que se mira, con la logica del movil:
//! - la curva del trazo pasa por cada muestra sin moverla (los casos de
//!   `CurvaDelTrazoTest.kt`, con sus mismos numeros);
//! - las siete durezas cambian el grueso, la carga y el grano como en
//!   `Element.kt`, y HB es exactamente lo de antes de haberlas;
//! - `dureza`, `reference` y `referenciasVisibles` van y vuelven por el
//!   fichero sin cambiar un byte de lo que nadie toco;
//! - los gestos del grafito (`GrafitoLibreTest.kt`): una letra no se vuelve
//!   recta, una raya grande si, clavar la punta abre el compas;
//! - los dos mundos del modo guia: fuera no se elige una guia, dentro solo
//!   guias, y una guia escondida no se ve ni se toca.

use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::excalidraw;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::impacto;
use pixpin_motor2d::tinta::MaterialTinta;
use pixpin_motor2d::tinta::grafito::{self, Dureza, curva_del_trazo};
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

fn arrastrar(g: &mut Gesto, e: &mut Escena, a: (f32, f32), b: (f32, f32)) -> Elemento {
    ev(g, e, pulsar(a.0, a.1));
    for k in 1..=10 {
        let t = k as f32 / 10.0;
        ev(g, e, mover(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
    }
    ev(g, e, soltar(b.0, b.1));
    e.elementos.last().expect("nacio algo").clone()
}

/// Un trazo de grafito en curva (media onda) con la dureza `d`.
fn trazo(d: Option<Dureza>, dx: f32) -> Elemento {
    let puntos: Vec<Punto2> = (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            Punto2::nuevo(
                dx + 20.0 + 160.0 * t,
                60.0 - 40.0 * (t * std::f32::consts::PI).sin(),
            )
        })
        .collect();
    let mut e = Elemento {
        id: 7,
        figura: Figura::Lapiz {
            puntos,
            presiones: Vec::new(),
            opciones: Some(Default::default()),
        },
        grosor: 2.0,
        material: MaterialTinta::Cuadritos,
        trazo: ColorRgba::opaco(0.12, 0.12, 0.12),
        ..Default::default()
    };
    e.extras.dureza = d;
    e
}

/// Lo que carga un mapa: la suma del alfa, y cuantas casillas toca.
fn carga(c: &grafito::Cocido) -> (u64, usize) {
    let a = c.rgba.chunks(4).map(|p| p[3] as u64).sum();
    let n = c.rgba.chunks(4).filter(|p| p[3] > 0).count();
    (a, n)
}

// ---------------------------------------------------------------------------
// La curva (CurvaDelTrazoTest.kt)
// ---------------------------------------------------------------------------

#[test]
fn la_curva_pasa_por_todas_las_muestras_sin_mover_ninguna() {
    let muestras = [
        [0.0, 0.0],
        [10.0, 3.0],
        [18.0, 15.0],
        [20.0, 30.0],
        [12.0, 41.0],
    ];
    let (curva, _, indices) = curva_del_trazo(&muestras, None, 1.0);
    for (k, m) in muestras.iter().enumerate() {
        assert_eq!(curva[indices[k]], *m, "la muestra {k} se movio");
    }
    assert_eq!(curva.first(), muestras.first());
    assert_eq!(curva.last(), muestras.last());
}

#[test]
fn una_circunferencia_con_pocas_muestras_sale_redonda_y_no_un_poligono() {
    let r = 100.0f64;
    let muestras: Vec<[f64; 2]> = (0..=12)
        .map(|i| {
            let a = i as f64 * std::f64::consts::PI / 6.0;
            [r * a.cos(), r * a.sin()]
        })
        .collect();
    let (curva, _, _) = curva_del_trazo(&muestras, None, 2.0);
    let aparta = |p: &[f64; 2]| (p[0].hypot(p[1]) - r).abs();
    let de_las_rectas = r * (1.0 - (std::f64::consts::PI / 12.0).cos());
    let dentro: Vec<&[f64; 2]> = curva
        .iter()
        .filter(|p| {
            let mut a = p[1].atan2(p[0]);
            if a < 0.0 {
                a += 2.0 * std::f64::consts::PI;
            }
            a > std::f64::consts::PI / 6.0
                && a < 2.0 * std::f64::consts::PI - std::f64::consts::PI / 6.0
        })
        .collect();
    let peor = dentro.iter().map(|p| aparta(p)).fold(0.0, f64::max);
    assert!(peor < 0.6, "se aparta {peor} del circulo");
    assert!(peor < de_las_rectas / 5.0);
    // En las puntas, nunca peor que la recta.
    let peor_todo = curva.iter().map(aparta).fold(0.0, f64::max);
    assert!(peor_todo <= de_las_rectas + 1e-9);
}

#[test]
fn la_presion_se_reparte_y_cuadra_punto_a_punto() {
    let muestras = [[0.0, 0.0], [10.0, 0.0], [20.0, 5.0]];
    let (curva, presiones, _) = curva_del_trazo(&muestras, Some(&[0.2, 0.6, 1.0]), 2.5);
    let presiones = presiones.expect("hay presiones");
    assert_eq!(curva.len(), presiones.len());
    assert!((presiones[0] - 0.2).abs() < 1e-6);
    assert!((presiones.last().unwrap() - 1.0).abs() < 1e-6);
    assert!(presiones.windows(2).all(|w| w[1] >= w[0]));
}

#[test]
fn las_muestras_repetidas_no_rompen_nada_y_dos_puntos_quedan_tal_cual() {
    // Caso negativo: muestras repetidas, sin NaN.
    let (curva, _, _) = curva_del_trazo(
        &[[0.0, 0.0], [0.0, 0.0], [5.0, 5.0], [5.0, 5.0], [9.0, 2.0]],
        None,
        1.0,
    );
    assert!(curva.iter().all(|p| !p[0].is_nan() && !p[1].is_nan()));
    let (dos, _, _) = curva_del_trazo(&[[0.0, 0.0], [3.0, 4.0]], None, 1.0);
    assert_eq!(dos, vec![[0.0, 0.0], [3.0, 4.0]]);
}

// ---------------------------------------------------------------------------
// Las durezas
// ---------------------------------------------------------------------------

#[test]
fn los_numeros_de_las_durezas_son_los_del_movil() {
    let esperado = [
        ("4H", 0.42, 0.70, 0.55),
        ("2H", 0.60, 0.80, 0.70),
        ("HB", 1.00, 1.00, 1.00),
        ("2B", 1.25, 1.10, 1.15),
        ("4B", 1.55, 1.25, 1.30),
        ("6B", 1.85, 1.40, 1.45),
        ("8B", 2.20, 1.55, 1.60),
    ];
    for (d, (rotulo, oscuro, gordo, agarre)) in Dureza::TODAS.into_iter().zip(esperado) {
        assert_eq!(d.rotulo(), rotulo);
        assert_eq!(Dureza::desde_rotulo(rotulo), Some(d));
        assert_eq!((d.oscuro(), d.gordo(), d.agarre()), (oscuro, gordo, agarre));
    }
    // Caso negativo: una palabra que no es una dureza.
    assert_eq!(Dureza::desde_rotulo("10B"), None);
    assert_eq!(Dureza::desde_rotulo("hb"), None);
}

#[test]
fn la_8b_es_mas_gorda_y_mas_negra_que_la_hb_y_la_4h_mas_fina_y_clara() {
    let h4 = grafito::cocer_sin_horno(&trazo(Some(Dureza::H4), 0.0)).unwrap();
    let hb = grafito::cocer_sin_horno(&trazo(None, 0.0)).unwrap();
    let b8 = grafito::cocer_sin_horno(&trazo(Some(Dureza::B8), 0.0)).unwrap();
    let (a4, n4) = carga(&h4);
    let (ab, nb) = carga(&hb);
    let (a8, n8) = carga(&b8);
    assert!(a8 > ab && ab > a4, "carga 4H {a4}, HB {ab}, 8B {a8}");
    assert!(n8 > nb && nb > n4, "casillas 4H {n4}, HB {nb}, 8B {n8}");
    // Por casilla tambien: la blanda tapa el grano, la dura lo deja ver.
    let media = |a: u64, n: usize| a as f64 / n as f64;
    assert!(media(a8, n8) > media(ab, nb) && media(ab, nb) > media(a4, n4));
}

#[test]
fn hb_escrita_y_sin_escribir_pintan_lo_mismo_bit_a_bit() {
    // Lo que llega del movil sin `dureza` y con «HB» es el mismo lapiz.
    let sin = grafito::cocer_sin_horno(&trazo(None, 0.0)).unwrap();
    let con = grafito::cocer_sin_horno(&trazo(Some(Dureza::Hb), 0.0)).unwrap();
    assert_eq!(sin.rgba, con.rgba);
    assert_eq!(
        (sin.ancho, sin.alto, sin.x0, sin.y0),
        (con.ancho, con.alto, con.x0, con.y0)
    );
}

#[test]
fn cambiar_la_dureza_recuece_el_mapa_del_horno() {
    let mut e = trazo(None, 0.0);
    e.id = 991;
    let antes = grafito::cocer(&e).unwrap();
    e.extras.dureza = Some(Dureza::B6);
    let despues = grafito::cocer(&e).unwrap();
    assert_ne!(antes.rgba, despues.rgba, "el horno devolvio el mapa viejo");
}

#[test]
fn una_figura_de_grafito_tambien_lleva_su_dureza() {
    let caja = |d| Elemento {
        id: 3,
        figura: Figura::Rectangulo,
        x: 10.0,
        y: 10.0,
        ancho: 120.0,
        alto: 80.0,
        grosor: 2.0,
        material: MaterialTinta::Cuadritos,
        trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
        extras: pixpin_motor2d::elemento::Extras {
            dureza: d,
            ..Default::default()
        },
        ..Default::default()
    };
    let (a4, _) = carga(&grafito::cocer_sin_horno(&caja(Some(Dureza::H4))).unwrap());
    let (a8, _) = carga(&grafito::cocer_sin_horno(&caja(Some(Dureza::B8))).unwrap());
    assert!(a8 > a4 * 2, "4H {a4}, 8B {a8}");
}

#[test]
fn el_grafito_nace_con_la_dureza_elegida_y_hb_no_se_apunta() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    g.estilo
        .aplicar(pixpin_motor2d::estilo::CambioEstilo::Dureza(Dureza::B4));
    let e = arrastrar(&mut g, &mut escena, (0.0, 0.0), (30.0, 30.0));
    assert_eq!(e.extras.dureza, Some(Dureza::B4));
    g.estilo
        .aplicar(pixpin_motor2d::estilo::CambioEstilo::Dureza(Dureza::Hb));
    let e = arrastrar(&mut g, &mut escena, (0.0, 50.0), (30.0, 80.0));
    assert_eq!(e.extras.dureza, None, "HB es la de fabrica: no se escribe");
    // Caso negativo: el lapiz de tinta no tiene dureza aunque este elegida.
    let mut g = con(Herramienta::Lapiz);
    g.estilo
        .aplicar(pixpin_motor2d::estilo::CambioEstilo::Dureza(Dureza::B8));
    let e = arrastrar(&mut g, &mut escena, (0.0, 100.0), (30.0, 130.0));
    assert_eq!(e.extras.dureza, None);
}

#[test]
fn la_dureza_del_panel_solo_llega_a_lo_de_grafito() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    let gr = arrastrar(&mut g, &mut escena, (0.0, 0.0), (60.0, 30.0)).id;
    g.tomar_herramienta(Herramienta::Lapiz);
    let li = arrastrar(&mut g, &mut escena, (0.0, 50.0), (60.0, 80.0)).id;
    let mut sel = pixpin_motor2d::seleccion::Seleccion::nueva();
    sel.poner_todos([gr, li]);
    pixpin_motor2d::estilo::aplicar_a(
        &mut escena,
        &sel,
        pixpin_motor2d::estilo::CambioEstilo::Dureza(Dureza::B8),
    );
    assert_eq!(escena.buscar(gr).unwrap().extras.dureza, Some(Dureza::B8));
    assert_eq!(escena.buscar(li).unwrap().extras.dureza, None);
    assert!(escena.deshacer());
    assert_eq!(escena.buscar(gr).unwrap().extras.dureza, None, "se deshace");
}

// ---------------------------------------------------------------------------
// El fichero
// ---------------------------------------------------------------------------

const DEL_MOVIL: &str = r##"{"type":"excalidraw","elements":[
 {"id":"a","type":"freedraw","x":10,"y":10,"width":50,"height":20,"angle":0,
  "strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid",
  "strokeWidth":1,"strokeStyle":"solid","roughness":0,"opacity":100,"seed":1,
  "version":3,"isDeleted":false,"points":[[0,0],[20,10],[50,20]],
  "pressures":[],"simulatePressure":true,"material":"cuadritos","dureza":"8B"},
 {"id":"b","type":"ellipse","x":100,"y":10,"width":80,"height":80,"angle":0,
  "strokeColor":"#1971c2","backgroundColor":"transparent","fillStyle":"solid",
  "strokeWidth":1,"strokeStyle":"solid","roughness":0,"opacity":100,"seed":2,
  "version":2,"isDeleted":false,"reference":true},
 {"id":"c","type":"freedraw","x":10,"y":60,"width":50,"height":20,"angle":0,
  "strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid",
  "strokeWidth":1,"strokeStyle":"solid","roughness":0,"opacity":100,"seed":3,
  "version":1,"isDeleted":false,"points":[[0,0],[50,20]],"pressures":[],
  "simulatePressure":true,"material":"cuadritos","dureza":"10B"}
 ],"referenciasVisibles":false,"tablas":[]}"##;

#[test]
fn lo_del_movil_vuelve_sin_cambiar_un_byte() {
    let l = excalidraw::leer(DEL_MOVIL).unwrap();
    let escena = excalidraw::a_escena(&l);
    assert_eq!(escena.elementos[0].extras.dureza, Some(Dureza::B8));
    assert!(escena.elementos[1].extras.referencia);
    // Una dureza desconocida se lee como HB...
    assert_eq!(escena.elementos[2].extras.dureza, None);
    assert!(!escena.referencias_visibles);
    let una = excalidraw::escribir(&excalidraw::con_escena(&l, &escena));
    let otra = excalidraw::escribir(&excalidraw::con_escena(
        &excalidraw::leer(&una).unwrap(),
        &escena,
    ));
    assert_eq!(una, otra);
    let v: serde_json::Value = serde_json::from_str(&una).unwrap();
    assert_eq!(v["elements"][0]["dureza"], "8B");
    assert_eq!(v["elements"][1]["reference"], true);
    assert_eq!(v["referenciasVisibles"], false);
}

#[test]
fn una_dureza_desconocida_sobrevive_a_mover_el_trazo() {
    let l = excalidraw::leer(DEL_MOVIL).unwrap();
    let mut escena = excalidraw::a_escena(&l);
    escena.elementos[2].x += 5.0;
    escena.elementos[2].tocar();
    let v: serde_json::Value =
        serde_json::from_str(&excalidraw::escribir(&excalidraw::con_escena(&l, &escena))).unwrap();
    assert_eq!(
        v["elements"][2]["dureza"], "10B",
        "se perdio la de una version futura"
    );
}

#[test]
fn cambiar_la_dureza_la_escribe_y_volver_a_hb_quita_la_clave() {
    let l = excalidraw::leer(DEL_MOVIL).unwrap();
    let mut escena = excalidraw::a_escena(&l);
    escena.elementos[0].extras.dureza = Some(Dureza::H2);
    escena.elementos[0].tocar();
    let v: serde_json::Value =
        serde_json::from_str(&excalidraw::escribir(&excalidraw::con_escena(&l, &escena))).unwrap();
    assert_eq!(v["elements"][0]["dureza"], "2H");
    escena.elementos[0].extras.dureza = None;
    escena.elementos[0].tocar();
    let v: serde_json::Value =
        serde_json::from_str(&excalidraw::escribir(&excalidraw::con_escena(&l, &escena))).unwrap();
    assert!(v["elements"][0].get("dureza").is_none(), "HB no se escribe");
}

#[test]
fn ensenar_las_guias_lo_escribe_y_un_lienzo_sin_la_clave_no_la_estrena() {
    let l = excalidraw::leer(DEL_MOVIL).unwrap();
    let mut escena = excalidraw::a_escena(&l);
    escena.referencias_visibles = true;
    let v: serde_json::Value =
        serde_json::from_str(&excalidraw::escribir(&excalidraw::con_escena(&l, &escena))).unwrap();
    assert_eq!(v["referenciasVisibles"], true);
    // Caso negativo: un lienzo del PC sin la clave y sin tocar no la estrena.
    let pc = r#"{"type":"excalidraw","elements":[]}"#;
    let l = excalidraw::leer(pc).unwrap();
    let escena = excalidraw::a_escena(&l);
    assert!(escena.referencias_visibles);
    let sale = excalidraw::escribir(&excalidraw::con_escena(&l, &escena));
    assert!(!sale.contains("referenciasVisibles"), "{sale}");
}

#[test]
fn una_guia_nacida_aqui_sale_con_reference_y_un_trazo_normal_sin_ella() {
    let pc = r#"{"type":"excalidraw","elements":[]}"#;
    let l = excalidraw::leer(pc).unwrap();
    let mut escena = excalidraw::a_escena(&l);
    let mut g = con(Herramienta::Rectangulo);
    g.poner_modo_guia(true);
    arrastrar(&mut g, &mut escena, (0.0, 0.0), (50.0, 50.0));
    g.poner_modo_guia(false);
    arrastrar(&mut g, &mut escena, (100.0, 0.0), (150.0, 50.0));
    let v: serde_json::Value =
        serde_json::from_str(&excalidraw::escribir(&excalidraw::con_escena(&l, &escena))).unwrap();
    assert_eq!(v["elements"][0]["reference"], true);
    assert!(v["elements"][1].get("reference").is_none());
    let vuelta = excalidraw::a_escena(&excalidraw::leer(&v.to_string()).unwrap());
    assert!(vuelta.elementos[0].extras.referencia);
    assert!(!vuelta.elementos[1].extras.referencia);
}

// ---------------------------------------------------------------------------
// Los gestos del grafito (GrafitoLibreTest.kt)
// ---------------------------------------------------------------------------

#[test]
fn escribiendo_con_grafito_pararse_no_convierte_la_letra() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    ev(&mut g, &mut escena, pulsar(0.0, 0.0));
    // Una letra en zigzag: cabe en 40 px de pantalla.
    let muestras: Vec<(f32, f32)> = (1..=12)
        .map(|i| (i as f32 * 3.0, if i % 2 == 0 { 20.0 } else { 0.0 }))
        .collect();
    for &(x, y) in &muestras {
        ev(&mut g, &mut escena, mover(x, y));
    }
    assert!(
        g.convertir_en_forma(&mut escena, Punto2::nuevo(36.0, 20.0), 1.0)
            .is_none(),
        "la letra se convirtio"
    );
    ev(&mut g, &mut escena, soltar(36.0, 20.0));
    let e = escena.elementos.last().unwrap();
    assert!(matches!(e.figura, Figura::Lapiz { .. }));
    assert_eq!(e.material, MaterialTinta::Cuadritos);
    // Y la misma letra a 5 aumentos (200 px en pantalla) ya es grande.
    assert!(pixpin_motor2d::forma_rapida::trazo_de_figura_del_grafito(
        &muestras
            .iter()
            .map(|p| Punto2::nuevo(p.0, p.1))
            .collect::<Vec<_>>(),
        5.0
    ));
}

#[test]
fn con_grafito_una_raya_larga_quieta_sale_recta_de_grafito() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    ev(&mut g, &mut escena, pulsar(0.0, 0.0));
    for i in 1..=20 {
        ev(
            &mut g,
            &mut escena,
            mover(i as f32 * 15.0, if i % 2 == 0 { 2.0 } else { -2.0 }),
        );
    }
    assert!(
        g.convertir_en_forma(&mut escena, Punto2::nuevo(300.0, 0.0), 1.0)
            .is_some()
    );
    let e = escena.elementos.last().unwrap();
    assert!(matches!(e.figura, Figura::Linea { .. }), "{:?}", e.figura);
    assert_eq!(e.material, MaterialTinta::Cuadritos);
}

#[test]
fn con_grafito_clavar_la_punta_abre_el_compas() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    ev(&mut g, &mut escena, pulsar(100.0, 100.0));
    ev(&mut g, &mut escena, mover(101.0, 100.0));
    assert!(
        g.convertir_en_forma(&mut escena, Punto2::nuevo(140.0, 100.0), 1.0)
            .is_some()
    );
    assert!(matches!(
        escena.elementos.last().unwrap().figura,
        Figura::Elipse
    ));
}

#[test]
fn el_grafito_espera_un_segundo_y_el_lapiz_medio() {
    assert_eq!(con(Herramienta::Grafito).espera_de_la_forma_ms(), 1000);
    assert_eq!(con(Herramienta::Lapiz).espera_de_la_forma_ms(), 550);
    assert_eq!(con(Herramienta::Resaltador).espera_de_la_forma_ms(), 550);
}

#[test]
fn el_lapiz_de_tinta_sigue_convirtiendo_una_raya_corta() {
    // Caso negativo del umbral: los 160 px son solo del grafito.
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Lapiz);
    ev(&mut g, &mut escena, pulsar(0.0, 0.0));
    for i in 1..=10 {
        ev(&mut g, &mut escena, mover(i as f32 * 8.0, 0.0));
    }
    assert!(
        g.convertir_en_forma(&mut escena, Punto2::nuevo(80.0, 0.0), 1.0)
            .is_some()
    );
}

// ---------------------------------------------------------------------------
// El modo guia
// ---------------------------------------------------------------------------

/// Una escena con un rectangulo de dibujo en (0..50) y uno guia en (100..150).
fn dos_mundos() -> (Escena, u64, u64) {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Rectangulo);
    let dibujo = arrastrar(&mut g, &mut escena, (0.0, 0.0), (50.0, 50.0)).id;
    g.poner_modo_guia(true);
    let guia = arrastrar(&mut g, &mut escena, (100.0, 0.0), (150.0, 50.0)).id;
    assert!(escena.buscar(guia).unwrap().extras.referencia);
    assert!(!escena.buscar(dibujo).unwrap().extras.referencia);
    (escena, dibujo, guia)
}

fn clic(g: &mut Gesto, e: &mut Escena, x: f32, y: f32) -> Vec<u64> {
    ev(g, e, pulsar(x, y));
    ev(g, e, soltar(x, y));
    g.seleccion.ids().to_vec()
}

#[test]
fn fuera_del_modo_guia_una_guia_no_se_elige_y_dentro_solo_las_guias() {
    let (mut escena, dibujo, guia) = dos_mundos();
    let mut g = con(Herramienta::Mano);
    assert_eq!(clic(&mut g, &mut escena, 100.0, 25.0), Vec::<u64>::new());
    assert_eq!(clic(&mut g, &mut escena, 0.0, 25.0), vec![dibujo]);
    assert!(g.poner_modo_guia(true));
    assert!(
        g.seleccion.ids().is_empty(),
        "cambiar de mundo suelta lo elegido"
    );
    assert_eq!(clic(&mut g, &mut escena, 0.0, 25.0), Vec::<u64>::new());
    assert_eq!(clic(&mut g, &mut escena, 100.0, 25.0), vec![guia]);
}

#[test]
fn seleccionar_todo_coge_solo_lo_de_su_mundo() {
    let (mut escena, dibujo, guia) = dos_mundos();
    let mut g = con(Herramienta::Mano);
    ev(&mut g, &mut escena, EventoGesto::SeleccionarTodo);
    assert_eq!(g.seleccion.ids(), &[dibujo]);
    g.poner_modo_guia(true);
    ev(&mut g, &mut escena, EventoGesto::SeleccionarTodo);
    assert_eq!(g.seleccion.ids(), &[guia]);
}

#[test]
fn la_goma_no_se_lleva_una_guia_fuera_del_modo() {
    let (escena, dibujo, guia) = dos_mundos();
    let fuera = impacto::con_mundo(impacto::Mundo::default(), || {
        (
            impacto::elemento_en(&escena.elementos, Punto2::nuevo(100.0, 25.0)),
            impacto::elemento_en(&escena.elementos, Punto2::nuevo(0.0, 25.0)),
        )
    });
    assert_eq!(fuera, (None, Some(dibujo)));
    let dentro = impacto::con_mundo(
        impacto::Mundo {
            guias: true,
            guias_visibles: true,
        },
        || impacto::elemento_en(&escena.elementos, Punto2::nuevo(100.0, 25.0)),
    );
    assert_eq!(dentro, Some(guia));
    // Al salir de `con_mundo` vuelve el de fabrica.
    assert_eq!(impacto::mundo(), impacto::Mundo::default());
}

#[test]
fn esconder_las_guias_no_las_borra_no_se_ven_y_no_se_tocan_ni_en_el_modo() {
    let (mut escena, dibujo, guia) = dos_mundos();
    let mut g = con(Herramienta::Mano);
    g.poner_modo_guia(true);
    assert_eq!(clic(&mut g, &mut escena, 100.0, 25.0), vec![guia]);
    assert!(escena.hay_referencias());
    g.alternar_referencias(&mut escena);
    assert!(!escena.referencias_visibles);
    assert!(
        g.seleccion.ids().is_empty(),
        "lo escondido no queda elegido"
    );
    let vistos: Vec<u64> = escena.a_la_vista().map(|e| e.id).collect();
    assert_eq!(vistos, vec![dibujo]);
    assert_eq!(escena.visibles().count(), 2, "escondida no es borrada");
    assert_eq!(clic(&mut g, &mut escena, 100.0, 25.0), Vec::<u64>::new());
    // Y no sale al exportar.
    let ordenes = pixpin_motor2d::pintado::ordenes_de_escena(&escena);
    let solo_dibujo = {
        let mut e = escena.clone();
        e.borrar(guia);
        pixpin_motor2d::pintado::ordenes_de_escena(&e)
    };
    assert_eq!(ordenes.len(), solo_dibujo.len());
    g.alternar_referencias(&mut escena);
    assert_eq!(escena.a_la_vista().count(), 2);
}

#[test]
fn lo_que_sale_de_un_gesto_en_modo_guia_sigue_siendo_guia() {
    let mut escena = Escena::nueva();
    let mut g = con(Herramienta::Grafito);
    g.poner_modo_guia(true);
    ev(&mut g, &mut escena, pulsar(100.0, 100.0));
    ev(&mut g, &mut escena, mover(101.0, 100.0));
    g.convertir_en_forma(&mut escena, Punto2::nuevo(150.0, 100.0), 1.0)
        .unwrap();
    let e = escena.elementos.last().unwrap();
    assert!(matches!(e.figura, Figura::Elipse));
    assert!(e.extras.referencia, "el compas de guia es guia");
}

// ---------------------------------------------------------------------------
// La muestra
// ---------------------------------------------------------------------------

/// **Las siete durezas, una al lado de otra**, en PNG: el mismo trazo de
/// arriba abajo de 4H a 8B, sobre papel blanco. Para mirarla:
/// `PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin-motor2d --test
/// grafito_durezas_y_guias -- --ignored muestra`.
#[test]
#[ignore]
fn muestra_de_las_siete_durezas() {
    let (ancho, alto_fila) = (230u32, 80u32);
    let alto = alto_fila * 7;
    let mut lienzo = vec![255u8; (ancho * alto * 4) as usize];
    for (k, d) in Dureza::TODAS.into_iter().enumerate() {
        let mut e = trazo(Some(d), 0.0);
        e.id = 100 + k as u64;
        let c = grafito::cocer_sin_horno(&e).unwrap();
        let oy = k as i32 * alto_fila as i32;
        for y in 0..c.alto as i32 {
            for x in 0..c.ancho as i32 {
                let (dx, dy) = (c.x0 as i32 + x, c.y0 as i32 + y + oy);
                if dx < 0 || dy < 0 || dx >= ancho as i32 || dy >= alto as i32 {
                    continue;
                }
                let s = ((y as u32 * c.ancho + x as u32) * 4) as usize;
                let a = c.rgba[s + 3] as f32 / 255.0;
                let t = ((dy as u32 * ancho + dx as u32) * 4) as usize;
                for i in 0..3 {
                    lienzo[t + i] =
                        (c.rgba[s + i] as f32 * a + lienzo[t + i] as f32 * (1.0 - a)) as u8;
                }
            }
        }
    }
    let todo = grafito::Cocido {
        rgba: lienzo,
        ancho,
        alto,
        x0: 0.0,
        y0: 0.0,
        finura: 1.0,
        huella: 0,
        angulo: 0.0,
        centro: Punto2::nuevo(0.0, 0.0),
        id: 0,
        generacion: 0,
        sucio: None,
    };
    let png = grafito::png_del_trozo(&todo, (0, 0, ancho, alto));
    let carpeta = std::env::var("PIXPIN_MUESTRAS")
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
    let ruta = std::path::Path::new(&carpeta).join("grafito-siete-durezas.png");
    std::fs::write(&ruta, png).unwrap();
    println!("muestra en {}", ruta.display());
}
