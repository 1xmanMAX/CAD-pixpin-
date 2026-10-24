//! **Los veintitres tipos de elemento del movil entran, se mueven y salen.**
//!
//! Es la prueba que guarda la promesa del puente, y por eso esta aqui y no
//! dentro de `excalidraw.rs`: las de dentro comprueban un tipo cada una y es
//! facil que una figura nueva se cuele sin la suya. Esta las tiene todas a la
//! vez, y falla si alguien anade un tipo al movil y no lo declara aqui.
//!
//! Lo que promete, en una frase: **lo que entra vuelve a salir, y lo unico
//! que cambia es lo que de verdad se movio**. Ni una clave de menos, ni un
//! valor cambiado por haber pasado por Windows.
//!
//! Los veintitres salen de `docs/investigacion/2026-09-20-herramientas-lienzo-android.md`
//! §1.4, que a su vez los saco de `Element.kt:17-25` de la v0.72.0.

use pixpin_motor2d::elemento::Figura;
use pixpin_motor2d::excalidraw::{Entrada, escribir, leer};
use serde_json::{Map, Value};

/// Los nueve de Excalidraw y los catorce propios de PixPin.
const LOS_VEINTITRES: [&str; 23] = [
    "rectangle",
    "diamond",
    "ellipse",
    "arrow",
    "line",
    "freedraw",
    "text",
    "image",
    "frame",
    "pixpin-mosaic",
    "pixpin-spotlight",
    "pixpin-lupa",
    "pixpin-serial",
    "pixpin-measure",
    "pixpin-arc",
    "pixpin-region",
    "pixpin-scalebar",
    "pixpin-point",
    "pixpin-axes",
    "pixpin-number-line",
    "pixpin-space",
    "pixpin-solid",
    "pixpin-gantt",
];

/// Los que el PC todavia no sabe dibujar y viajan por el carril ajeno.
///
/// **No es una lista de pendientes sino un contrato**: mientras un tipo este
/// aqui, lo que se promete de el es que vuelve intacto. Cuando su grupo le
/// ensene a dibujarse, hay que quitarlo de aqui — y la prueba de mas abajo
/// obliga a ello.
///
/// Los tres instrumentos de graficar (`pixpin-axes`, `pixpin-number-line`,
/// `pixpin-space`) salieron de aqui el 13-oct-2026: se leen como un solo
/// instrumento con su numero de ejes (`instrumento`).
const LOS_AJENOS: [&str; 2] = [
    "pixpin-solid",
    // El cronograma se supo dibujar (F12) y se quito: no estaba bien hecho.
    "pixpin-gantt",
];

/// Lo unico que puede cambiar al mover un elemento.
///
/// `version`, `versionNonce` y `updated` cambian porque son justo lo que le
/// dice al otro aparato «esto se ha tocado»: si no cambiaran, el movil se
/// quedaria con su copia vieja.
const PUEDE_CAMBIAR: [&str; 7] = [
    "x",
    "y",
    "points",
    "huecos",
    "version",
    "versionNonce",
    "updated",
];

/// Un lienzo con un elemento de cada tipo, con los campos propios de cada uno
/// puestos: son justo los que no pueden perderse.
fn lienzo_con_los_veintitres() -> String {
    let mut n = 0.0;
    let mut elementos: Vec<Value> = Vec::new();
    for tipo in LOS_VEINTITRES {
        n += 1.0;
        let mut e = serde_json::json!({
            "id": format!("e{tipo}"),
            "type": tipo,
            "x": n * 100.0,
            "y": n * 10.0,
            "width": 80.0,
            "height": 40.0,
            "angle": 0.0,
            "strokeColor": "#1e1e1e",
            "backgroundColor": "transparent",
            "fillStyle": "hachure",
            "strokeWidth": 2.0,
            "strokeStyle": "solid",
            "roughness": 1,
            "opacity": 100,
            "seed": 1000.0 + n,
            "version": 3,
            "versionNonce": 55,
            "updated": 1_700_000_000_000i64,
            "isDeleted": false,
            "locked": false,
            "groupIds": ["g1"],
            "material": "rayado",
            // Los campos que el movil pone en casi todo y que esta tanda
            // acaba de aprender a leer: tienen que volver tal cual.
            "boundElements": [{ "id": "otro", "type": "arrow" }],
            "presionFirme": true,
            "negrita": true,
            "cursiva": false,
            "tachado": false,
            "containerId": null,
            "startBinding": null,
            "endBinding": null,
            "papel": null,
            "pauta": "lisa"
        });
        let m = e.as_object_mut().unwrap();
        // Lo propio de cada tipo, con los nombres exactos de `Element.kt`.
        match tipo {
            "arrow" | "line" | "freedraw" | "pixpin-measure" => {
                m.insert(
                    "points".into(),
                    serde_json::json!([{"x": 0.0, "y": 0.0}, {"x": 40.0, "y": 20.0}]),
                );
                if tipo == "freedraw" {
                    m.insert("pressures".into(), serde_json::json!([0.5, 0.8]));
                    m.insert("simulatePressure".into(), Value::Bool(false));
                }
                if tipo == "arrow" {
                    m.insert("startArrowhead".into(), Value::Null);
                    m.insert("endArrowhead".into(), Value::String("arrow".into()));
                    m.insert("elbowed".into(), Value::Bool(false));
                    m.insert(
                        "startBinding".into(),
                        serde_json::json!({
                            "elementId": "erectangle", "focus": 0.25, "gap": 4.0,
                            "fixedPoint": [0.5, 0.5], "mode": "inside"
                        }),
                    );
                }
            }
            "text" => {
                m.insert("text".into(), Value::String("hola".into()));
                m.insert("fontSize".into(), Value::from(20.0));
                m.insert("fontFamily".into(), Value::from(5));
                m.insert("containerId".into(), Value::String("erectangle".into()));
                m.insert("tachado".into(), Value::Bool(true));
            }
            "image" => {
                m.insert("fileId".into(), Value::String("f1".into()));
                m.insert("scale".into(), serde_json::json!([1.0, -1.0]));
                m.insert(
                    "crop".into(),
                    serde_json::json!({"x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0,
                                       "naturalWidth": 20.0, "naturalHeight": 20.0}),
                );
                m.insert("enElSuelo".into(), Value::Bool(true));
            }
            "frame" => {
                m.insert("name".into(), Value::String("Lamina 1".into()));
                m.insert("papel".into(), Value::String("a4".into()));
                m.insert("pauta".into(), Value::String("cuadros".into()));
            }
            "rectangle" | "diamond" => {
                m.insert("roundness".into(), serde_json::json!({"type": 3}));
            }
            "pixpin-mosaic" => {
                m.insert("mosaicBlur".into(), Value::Bool(true));
            }
            "pixpin-spotlight" => {
                m.insert("oscurecer".into(), Value::from(70));
                m.insert("forma".into(), Value::String("elipse".into()));
            }
            "pixpin-lupa" => {
                m.insert("foco".into(), serde_json::json!({"x": 10.0, "y": 20.0}));
                m.insert("aumento".into(), Value::from(2.5));
                m.insert("focoAncho".into(), Value::from(40.0));
                m.insert("focoAlto".into(), Value::from(20.0));
                m.insert("lupaRedonda".into(), Value::Bool(true));
                m.insert("guia".into(), Value::String("flecha".into()));
            }
            "pixpin-serial" => {
                m.insert("text".into(), Value::String("7".into()));
                m.insert("fontSize".into(), Value::from(20.0));
            }
            "pixpin-arc" => {
                // En RADIANES, que es como los escribe el movil. Se eligen
                // valores exactos en `f32` para que la ida y vuelta pueda
                // seguir comparandose numero a numero.
                m.insert("arcStart".into(), Value::from(0.5));
                m.insert("arcSweep".into(), Value::from(2.25));
            }
            "pixpin-region" => {
                m.insert(
                    "points".into(),
                    serde_json::json!([{"x": 0.0, "y": 0.0}, {"x": 80.0, "y": 0.0},
                                       {"x": 80.0, "y": 40.0}, {"x": 0.0, "y": 40.0}]),
                );
                m.insert(
                    "huecos".into(),
                    serde_json::json!([[{"x": 20.0, "y": 10.0}, {"x": 60.0, "y": 10.0},
                                        {"x": 60.0, "y": 30.0}, {"x": 20.0, "y": 30.0}]]),
                );
            }
            "pixpin-point" => {
                m.insert("text".into(), Value::String("A".into()));
                m.insert("width".into(), Value::from(0.0));
                m.insert("height".into(), Value::from(0.0));
                // En radianes, igual que el arco.
                m.insert("etiquetaAngulo".into(), Value::from(0.75));
                m.insert("etiquetaRadio".into(), Value::from(16.0));
            }
            "pixpin-axes" | "pixpin-number-line" | "pixpin-space" => {
                // `unidad` es un numero en `Element.kt` (pixeles por unidad):
                // aqui ponia «cm», que el movil ni sabria leer.
                m.insert("unidad".into(), Value::from(37.5));
                m.insert("fontSize".into(), Value::from(14.0));
                m.insert("pasoDeNumeros".into(), Value::from(1.0));
                m.insert("pasoDeCuadros".into(), Value::from(0.5));
                if tipo == "pixpin-space" {
                    m.insert("azimut".into(), Value::from(35.0));
                    m.insert("elevacion".into(), Value::from(20.0));
                }
            }
            "pixpin-solid" => {
                m.insert("altura".into(), Value::from(50.0));
                m.insert("cota".into(), Value::from(0.0));
                m.insert("giroEnPlanta".into(), Value::from(15.0));
                m.insert("formaSolida".into(), Value::String("cilindro".into()));
                m.insert("inclinacion".into(), Value::from(0.0));
                m.insert("esqueleto".into(), Value::Bool(false));
            }
            "pixpin-gantt" => {
                m.insert(
                    "tareas".into(),
                    serde_json::json!([{"nombre": "cimientos", "desde": 0, "cuanto": 3,
                                        "color": "#ff0000"}]),
                );
                m.insert("periodos".into(), Value::from(12));
            }
            _ => {}
        }
        elementos.push(e);
    }
    serde_json::json!({
        "type": "excalidraw",
        "version": 2,
        "source": "pixpin-android",
        "elements": elementos,
        "appState": { "viewBackgroundColor": "#ffffff" },
        "files": {},
        "luces": { "encendidas": true, "fuerza": 0.5 },
        "alfileres": [],
        "escala": { "unidadesPorPixel": 0.02, "unidad": "m", "decimales": 2 }
    })
    .to_string()
}

/// Dos JSON iguales, comparando los numeros por su valor y no por su forma.
///
/// Sin esto la prueba se rompe por nada: un `2` que vuelve como `2.0` es el
/// mismo ancho de trazo, y exigir la forma exacta obligaria a escribir el
/// fichero de muestra adivinando como serializa cada campo.
///
/// La holgura es la del `f32`, que es en lo que trabaja el motor: una
/// presion de `0.8` del movil vuelve como `0.800000011920929`, que es el
/// `f32` mas cercano a ocho decimas. Eso no es perder un dato, es la anchura
/// del numero con el que se dibuja; lo que la prueba vigila es que no se
/// pierda ni cambie de verdad.
const HOLGURA_F32: f64 = 1e-6;

fn mismo_valor(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() <= HOLGURA_F32 * x.abs().max(1.0),
            _ => x == y,
        },
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| mismo_valor(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| mismo_valor(v, w)))
        }
        _ => a == b,
    }
}

fn por_tipo(json: &str) -> Vec<(String, Map<String, Value>)> {
    let raiz: Value = serde_json::from_str(json).unwrap();
    raiz["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["type"].as_str().unwrap().to_string(),
                e.as_object().unwrap().clone(),
            )
        })
        .collect()
}

#[test]
fn los_veintitres_tipos_estan_en_la_muestra_y_no_hay_dos_iguales() {
    // La prueba de la prueba: si alguien anade un tipo a la lista y se olvida
    // de darle campos, esto lo avisa antes que los demas casos.
    let tipos = por_tipo(&lienzo_con_los_veintitres());
    assert_eq!(tipos.len(), 23);
    let mut vistos: Vec<&str> = tipos.iter().map(|(t, _)| t.as_str()).collect();
    vistos.sort_unstable();
    vistos.dedup();
    assert_eq!(vistos.len(), 23, "hay tipos repetidos en la muestra");
}

#[test]
fn abrir_y_guardar_sin_tocar_nada_devuelve_los_veintitres_byte_a_byte() {
    // Es la promesa mas basica del puente: mirar un fichero no puede
    // cambiarlo. Antes de esta tanda ya se cumplia, y el riesgo de portar
    // tipos nuevos es romperla sin enterarse.
    let entrada = lienzo_con_los_veintitres();
    let salida = escribir(&leer(&entrada).unwrap());
    let antes = por_tipo(&entrada);
    let despues = por_tipo(&salida);
    assert_eq!(antes.len(), despues.len(), "cambio el numero de elementos");
    for ((t1, a), (t2, b)) in antes.iter().zip(&despues) {
        assert_eq!(t1, t2, "cambio el ORDEN de pintado");
        assert!(
            mismo_valor(&Value::Object(a.clone()), &Value::Object(b.clone())),
            "«{t1}» cambio al abrirlo y cerrarlo:\n{a:#?}\n{b:#?}"
        );
    }
}

#[test]
fn el_pc_sabe_dibujar_veintiuno_de_los_veintitres_y_los_otros_dos_viajan() {
    let l = leer(&lienzo_con_los_veintitres()).unwrap();
    let mut ajenos = Vec::new();
    for (entrada, (tipo, _)) in l
        .entradas
        .iter()
        .zip(por_tipo(&lienzo_con_los_veintitres()))
    {
        if matches!(entrada, Entrada::Ajeno(_)) {
            ajenos.push(tipo);
        }
    }
    ajenos.sort();
    let mut esperados: Vec<String> = LOS_AJENOS.iter().map(|s| s.to_string()).collect();
    esperados.sort();
    assert_eq!(
        ajenos, esperados,
        "la lista de tipos que el PC todavia no dibuja ya no cuadra: \
         si un grupo acaba de ensenarle uno, quitalo de LOS_AJENOS"
    );
    assert_eq!(l.cuantos_ajenos(), LOS_AJENOS.len());
}

#[test]
fn cada_uno_de_los_veintitres_se_mueve_y_lo_unico_que_cambia_es_donde_esta() {
    // El caso de verdad. Mover es la operacion mas inocente que hay y es la
    // que mas campos toca al guardar, porque obliga a reescribir el elemento
    // entero en vez de devolver el original tal cual.
    let entrada = lienzo_con_los_veintitres();
    let mut l = leer(&entrada).unwrap();
    let (dx, dy) = (7.0, -3.0);
    for e in &mut l.entradas {
        if let Entrada::Nuestro { elemento, .. } = e {
            elemento.mover(dx, dy);
        }
    }
    let salida = escribir(&l);
    let antes = por_tipo(&entrada);
    let despues = por_tipo(&salida);

    for ((tipo, a), (_, b)) in antes.iter().zip(&despues) {
        let ajeno = LOS_AJENOS.contains(&tipo.as_str());
        // Ni una clave puede desaparecer, ni de los que dibujamos ni de los
        // que no. Esa es la tabla de §5.2 del inventario: cada campo que se
        // pierda es un plano que vuelve sin sus cotas.
        for (clave, valor) in a {
            let Some(nuevo) = b.get(clave) else {
                panic!("«{tipo}» perdio la clave «{clave}» al moverlo");
            };
            if PUEDE_CAMBIAR.contains(&clave.as_str()) && !ajeno {
                continue;
            }
            assert!(
                mismo_valor(valor, nuevo),
                "«{tipo}» cambio «{clave}» al moverlo: {valor} -> {nuevo}"
            );
        }
        // Y lo movido se movio de verdad.
        if !ajeno {
            let (x0, y0) = (a["x"].as_f64().unwrap(), a["y"].as_f64().unwrap());
            let (x1, y1) = (b["x"].as_f64().unwrap(), b["y"].as_f64().unwrap());
            assert!(
                (x1 - x0 - dx as f64).abs() < 1e-4 && (y1 - y0 - dy as f64).abs() < 1e-4,
                "«{tipo}» no se movio: ({x0},{y0}) -> ({x1},{y1})"
            );
        }
    }
}

#[test]
fn los_campos_propios_del_movil_siguen_ahi_despues_del_viaje() {
    // Caso negativo explicito: no basta con que el elemento «exista», tienen
    // que seguir estando los campos que le dan sentido. Un arco sin
    // `arcSweep` vuelve a ser el ovalo entero, y una region sin `huecos`
    // tapa justo lo que se queria dejar ver.
    let mut l = leer(&lienzo_con_los_veintitres()).unwrap();
    for e in &mut l.entradas {
        if let Entrada::Nuestro { elemento, .. } = e {
            elemento.mover(1.0, 1.0);
        }
    }
    let salida = escribir(&l);
    for (clave, cuantos) in [
        ("arcSweep", 1),
        ("huecos", 1),
        ("etiquetaRadio", 1),
        ("mosaicBlur", 1),
        ("oscurecer", 1),
        ("aumento", 1),
        ("tareas", 1),
        ("altura", 1),
        ("azimut", 1),
        ("enElSuelo", 1),
        ("crop", 1),
        ("presionFirme", 23),
        ("boundElements", 23),
    ] {
        assert_eq!(
            salida.matches(&format!("\"{clave}\"")).count(),
            cuantos,
            "«{clave}» no aparece las {cuantos} veces que tiene que aparecer"
        );
    }
}

#[test]
fn los_tipos_nuevos_de_esta_tanda_se_leen_como_lo_que_son() {
    // Que el JSON sobreviva no basta: si el arco entrara como rectangulo se
    // veria mal y se guardaria mal en cuanto alguien lo tocara.
    let l = leer(&lienzo_con_los_veintitres()).unwrap();
    let figura = |tipo: &str| {
        let i = LOS_VEINTITRES.iter().position(|t| *t == tipo).unwrap();
        match &l.entradas[i] {
            Entrada::Nuestro { elemento, .. } => elemento.figura.clone(),
            Entrada::Ajeno(_) => panic!("«{tipo}» entro como ajeno"),
        }
    };
    assert_eq!(figura("diamond"), Figura::Rombo);
    assert_eq!(figura("pixpin-serial"), Figura::Serie { numero: 7 });
    let Figura::Arco { inicio, barrido } = figura("pixpin-arc") else {
        panic!("el arco no entro como arco");
    };
    // El movil los guarda en RADIANES (`Element.kt`: «en radianes»; `Arco.kt`
    // los usa crudos contra 2π), asi que el numero entra tal cual. Convertir
    // aqui encogia un arco de media vuelta del movil hasta un punto.
    assert!((inicio - 0.5).abs() < 1e-6, "arcStart no entro en radianes");
    assert!(
        (barrido.unwrap() - 2.25).abs() < 1e-6,
        "arcSweep no entro en radianes"
    );
    // El caso negativo: si alguien volviera a meter una conversion, el
    // valor pasaria a grados y no se parceria en nada al del fichero.
    assert!(
        (inicio - 0.5f32.to_radians()).abs() > 0.4 && (inicio - 0.5f32.to_degrees()).abs() > 0.4,
        "arcStart llego convertido de unidad"
    );
    let Figura::Region { contorno, huecos } = figura("pixpin-region") else {
        panic!("la region no entro como region");
    };
    assert_eq!(contorno.len(), 4);
    assert_eq!(huecos.len(), 1, "el agujero del anillo es lo que la define");
    let Figura::Punto {
        letra,
        radio,
        angulo,
    } = figura("pixpin-point")
    else {
        panic!("el punto no entro como punto");
    };
    assert_eq!(letra, "A");
    assert_eq!(radio, 16.0);
    // `etiquetaAngulo` tambien va en radianes: `Puntos.kt` lo mete directo
    // en `cos`/`sin`. Un 0.75 leido como grados dejaria la letra pegada a
    // la derecha del punto en vez de arriba a la derecha.
    assert!(
        (angulo - 0.75).abs() < 1e-6,
        "el angulo no entro en radianes"
    );
    assert!(
        (angulo - 0.75f32.to_radians()).abs() > 0.7,
        "el angulo llego convertido a radianes otra vez"
    );
}

#[test]
fn los_diez_campos_nuevos_del_elemento_llegan_al_escritorio() {
    let l = leer(&lienzo_con_los_veintitres()).unwrap();
    let extras = |tipo: &str| {
        let i = LOS_VEINTITRES.iter().position(|t| *t == tipo).unwrap();
        match &l.entradas[i] {
            Entrada::Nuestro { elemento, .. } => elemento.extras.clone(),
            Entrada::Ajeno(_) => panic!("«{tipo}» entro como ajeno"),
        }
    };
    let texto = extras("text");
    assert!(texto.presion_firme && texto.negrita && texto.tachado && !texto.cursiva);
    assert_eq!(texto.contenedor.as_deref(), Some("erectangle"));
    assert_eq!(texto.atados.len(), 1);
    assert_eq!(texto.atados[0].tipo, "arrow");

    let hoja = extras("frame");
    assert_eq!(
        hoja.papel,
        Some(pixpin_motor2d::elemento::TamanoPapel::A4),
        "una hoja A4 rayada no puede volver a ser un recuadro"
    );
    assert_eq!(hoja.pauta, pixpin_motor2d::elemento::PautaHoja::Cuadros);

    let flecha = extras("arrow");
    let b = flecha.enganche_inicio.expect("la flecha venia enganchada");
    assert_eq!(b.elemento, "erectangle");
    assert_eq!(b.punto_fijo, Some((0.5, 0.5)));
    assert_eq!(b.modo, pixpin_motor2d::elemento::ModoEnganche::Dentro);
    assert!(
        flecha.enganche_fin.is_none(),
        "un `endBinding` nulo es «no esta atada», no un enganche a nadie"
    );
}

#[test]
fn un_punto_sin_etiqueta_guardada_coloca_la_letra_donde_la_coloca_el_movil() {
    // `etiquetaAngulo` y `etiquetaRadio` son nulables en `Element.kt`, y con
    // `explicitNulls = false` el movil NO los escribe mientras no se toque la
    // letra. Quien rellena el hueco es `Puntos.sitioDeLaEtiqueta`, con
    // `-PI/4` y `RADIO_DE_LA_LETRA = 22.0`. Si aqui se rellenara con otra
    // cosa, la letra de un punto recien hecho en el movil saldria en otro
    // sitio y al guardar se le moveria tambien a el.
    let lienzo = serde_json::json!({
        "type": "excalidraw",
        "version": 2,
        "source": "prueba",
        "elements": [{
            "id": "p1", "type": "pixpin-point", "x": 10.0, "y": 20.0,
            "width": 0.0, "height": 0.0, "angle": 0.0, "text": "B",
            "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
            "fillStyle": "solid", "strokeWidth": 1, "strokeStyle": "solid",
            "roughness": 1, "opacity": 100, "seed": 1, "version": 1,
            "versionNonce": 1, "isDeleted": false, "updated": 1, "locked": false
        }]
    })
    .to_string();

    let l = leer(&lienzo).unwrap();
    let Entrada::Nuestro { elemento, .. } = &l.entradas[0] else {
        panic!("el punto entro como ajeno");
    };
    let Figura::Punto { angulo, radio, .. } = &elemento.figura else {
        panic!("no entro como punto");
    };
    assert!(
        (*angulo - (-std::f32::consts::FRAC_PI_4)).abs() < 1e-6,
        "sin `etiquetaAngulo` la letra tiene que ir donde la pone el movil, no a 0"
    );
    assert!(
        (*radio - 22.0).abs() < 1e-6,
        "el radio por omision es 22, no 14"
    );
    // Caso negativo: los valores viejos eran otros y no valen.
    assert!(*angulo != 0.0 && *radio != 14.0);
}
