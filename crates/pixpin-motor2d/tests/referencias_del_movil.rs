//! **Las lineas de referencia del movil se ven como referencia** (`reference`
//! de `Element.kt`, pintadas al 35 % en `Renderer.kt`). Antes llegaban y se
//! pintaban opacas, como si fueran dibujo.

use pixpin_motor2d::excalidraw;
use pixpin_motor2d::pintado::{self, Orden};

const JSON: &str = r##"{"type":"excalidraw","elements":[
    {"id":"guia","type":"line","x":0,"y":0,"width":100,"height":0,"strokeColor":"#000000",
     "opacity":100,"reference":true,"points":[[0,0],[100,0]]},
    {"id":"trazo","type":"line","x":0,"y":50,"width":100,"height":0,"strokeColor":"#000000",
     "opacity":100,"points":[[0,0],[100,0]]}
]}"##;

fn alfa(o: &Orden) -> f32 {
    match o {
        Orden::Polilinea { color, .. }
        | Orden::Poligono { color, .. }
        | Orden::Tinta { color, .. } => color.a,
        _ => 1.0,
    }
}

#[test]
fn una_referencia_del_movil_se_pinta_translucida_y_el_dibujo_no() {
    let lienzo = excalidraw::leer(JSON).unwrap();
    let e = lienzo.elementos();
    assert!(e[0].extras.referencia);
    assert!(
        !e[1].extras.referencia,
        "caso negativo: el dibujo no es referencia"
    );
    let guia: Vec<f32> = pintado::ordenes(&e[0]).iter().map(alfa).collect();
    let trazo: Vec<f32> = pintado::ordenes(&e[1]).iter().map(alfa).collect();
    assert!(
        !guia.is_empty() && guia.iter().all(|a| (a - 0.35).abs() < 1e-3),
        "{guia:?}"
    );
    assert!(trazo.iter().all(|a| (a - 1.0).abs() < 1e-3), "{trazo:?}");
    let lejos: Vec<f32> = pintado::ordenes_a_distancia(&e[0], 0.05)
        .iter()
        .map(alfa)
        .collect();
    assert!(
        lejos.iter().all(|a| (a - 0.35).abs() < 1e-3),
        "de lejos tambien: {lejos:?}"
    );
}

#[test]
fn la_marca_de_referencia_vuelve_al_movil_aunque_se_edite_aqui() {
    let lienzo = excalidraw::leer(JSON).unwrap();
    let mut escena = excalidraw::a_escena(&lienzo);
    escena.buscar_mut(1).unwrap().mover(10.0, 0.0);
    let json = excalidraw::escribir(&excalidraw::con_escena(&lienzo, &escena));
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["elements"][0]["reference"], true);
    assert!(v["elements"][1].get("reference").is_none());
}
