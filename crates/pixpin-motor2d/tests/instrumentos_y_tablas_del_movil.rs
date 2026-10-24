//! **El instrumento de graficar y la tabla de coordenadas, de ida y vuelta
//! con el movil** (Tanda 4 del 13-oct-2026).
//!
//! Lo que se promete: un lienzo hecho en Android con la recta numerica, el
//! plano cartesiano, el espacio y una tabla de coordenadas **se abre aqui y
//! se guarda sin cambiar un byte**; los tres tipos se leen como un solo
//! instrumento con su numero de ejes y salen con su tipo; estirar y girar
//! cambian solo lo que deben; y la tabla editada aqui vuelve en el formato
//! que lee su `Scene`, sin perder lo que no se conoce.

use pixpin_geom::Tirador;
use pixpin_motor2d::elemento::Figura;
use pixpin_motor2d::excalidraw::{Entrada, a_escena, con_escena, escribir, leer};
use pixpin_motor2d::instrumento::tabla::{PuntoDeTabla, TablaDeCoordenadas};
use pixpin_motor2d::instrumento::{self, Instrumento};
use pixpin_motor2d::transformar;
use pixpin_motor2d::vector::Punto2;
use serde_json::Value;

/// Un lienzo como lo guarda el movil (`ExcalidrawStore.guardar`): la escena
/// entera, con los tres instrumentos tal como los fabrica `Biblioteca.kt`
/// (uno con `fontSize` y `fontFamily`), dos tablas y su eje.
fn del_movil() -> String {
    let base = |id: &str, tipo: &str, x: f64, w: f64, h: f64| {
        serde_json::json!({
            "id": id, "type": tipo, "x": x, "y": 10.0, "width": w, "height": h,
            "angle": 0.0, "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
            "fillStyle": "solid", "strokeWidth": 2.0, "strokeStyle": "solid",
            "roughness": 0, "opacity": 100, "seed": 7, "version": 4,
            "versionNonce": 9, "updated": 1_760_000_000_000i64, "isDeleted": false,
            "groupIds": [], "locked": false,
            "unidad": 40.0, "pasoDeNumeros": 1.0, "pasoDeCuadros": 1.0
        })
    };
    let recta = base("r1", "pixpin-number-line", 0.0, 320.0, 56.0);
    let mut plano = base("p1", "pixpin-axes", 400.0, 320.0, 320.0);
    plano["fontSize"] = Value::from(16.0);
    plano["fontFamily"] = Value::from(6);
    plano["pasoDeNumeros"] = Value::from(0.5);
    let mut espacio = base("e1", "pixpin-space", 800.0, 320.0, 320.0);
    espacio["azimut"] = Value::from(35.0);
    espacio["elevacion"] = Value::from(20.0);
    serde_json::json!({
        "elements": [recta, plano, espacio],
        "files": {},
        "viewport": {"scrollX": 0.0, "scrollY": 0.0, "zoom": 1.0},
        "backgroundColor": "#ffffff",
        "escala": null,
        "tablas": [
            {"id": "t1", "nombre": "Perfil", "color": "#e03131",
             "puntos": [{"x": 0.0, "y": 0.0}, {"x": 3.0, "y": -2.0}],
             "visible": true, "futuro": 42},
            {"id": "t2", "color": "#1971c2", "puntos": [{"x": 1.5, "y": 2.25}],
             "visible": false}
        ],
        "origenCoordenadas": {"x": 560.0, "y": 170.0},
        "referenciasVisibles": true
    })
    .to_string()
}

fn escena_y_lienzo() -> (pixpin_motor2d::excalidraw::Lienzo, pixpin_motor2d::Escena) {
    let l = leer(&del_movil()).unwrap();
    let e = a_escena(&l);
    (l, e)
}

fn json(texto: &str) -> Value {
    serde_json::from_str(texto).unwrap()
}

#[test]
fn abrir_y_guardar_sin_tocar_nada_no_cambia_un_byte() {
    let (l, e) = escena_y_lienzo();
    let antes = escribir(&l);
    let despues = escribir(&con_escena(&l, &e));
    assert_eq!(antes, despues);
    // Y lo escrito dice lo mismo que lo que vino: ni una clave de menos.
    assert_eq!(json(&despues)["tablas"], json(&del_movil())["tablas"]);
    assert_eq!(json(&despues)["elements"], json(&del_movil())["elements"]);
}

#[test]
fn los_tres_tipos_son_un_solo_instrumento_con_su_numero_de_ejes() {
    let l = leer(&del_movil()).unwrap();
    assert_eq!(l.cuantos_ajenos(), 0);
    let ejes: Vec<u8> = l
        .elementos()
        .iter()
        .map(|e| match &e.figura {
            Figura::Instrumento(i) => i.ejes,
            otra => panic!("no es un instrumento: {otra:?}"),
        })
        .collect();
    assert_eq!(ejes, [1, 2, 3]);
    let esp = instrumento::de(&l.elementos()[2]).copied().unwrap();
    assert_eq!((esp.azimut, esp.elevacion), (Some(35.0), Some(20.0)));
    // Las cifras del plano en su letra y tamano.
    let plano = &l.elementos()[1];
    assert_eq!(plano.extras.tam_letra, Some(16.0));
    assert_eq!(plano.extras.familia.as_deref(), Some("Nunito"));
}

#[test]
fn estirar_por_un_lado_ensena_mas_numeros_y_por_una_esquina_los_agranda() {
    let (l, mut e) = escena_y_lienzo();
    let id = e.elementos[1].id;
    let cifras_antes = {
        let p = e.buscar(id).unwrap();
        instrumento::cifras(p, instrumento::de(p).unwrap()).len()
    };
    // Por el lado este, cien pixeles mas.
    {
        let p = e.buscar_mut(id).unwrap();
        transformar::escalar(
            p,
            Tirador::EsteBorde,
            Punto2::nuevo(820.0, 170.0),
            false,
            false,
        );
    }
    let p = e.buscar(id).unwrap().clone();
    let i = *instrumento::de(&p).unwrap();
    assert_eq!(i.unidad, Some(40.0), "por un lado la unidad no se toca");
    assert!(instrumento::cifras(&p, &i).len() > cifras_antes);
    let salida = json(&escribir(&con_escena(&l, &e)));
    let el = &salida["elements"][1];
    assert_eq!(el["type"], "pixpin-axes");
    assert_eq!(el["unidad"], 40.0);
    assert_eq!(el["width"], 420.0);
    // Caso negativo: nada mas del plano cambio, y los otros dos ni se tocan.
    for clave in [
        "pasoDeNumeros",
        "pasoDeCuadros",
        "fontSize",
        "fontFamily",
        "height",
        "y",
    ] {
        assert_eq!(
            el[clave],
            json(&del_movil())["elements"][1][clave],
            "{clave}"
        );
    }
    assert_eq!(salida["elements"][0], json(&del_movil())["elements"][0]);
    assert_eq!(salida["elements"][2], json(&del_movil())["elements"][2]);

    // Por la esquina sureste, al doble: la unidad tambien.
    let (_, mut e) = escena_y_lienzo();
    {
        let p = e.buscar_mut(id).unwrap();
        transformar::escalar(
            p,
            Tirador::SuresteEsquina,
            Punto2::nuevo(1040.0, 650.0),
            false,
            false,
        );
    }
    let p = e.buscar(id).unwrap();
    let i = instrumento::de(p).unwrap();
    assert_eq!(i.unidad, Some(80.0));
    assert_eq!(p.extras.tam_letra, Some(32.0));
}

#[test]
fn el_tirador_de_giro_da_la_vuelta_al_espacio_y_no_inclina_la_caja() {
    let (l, mut e) = escena_y_lienzo();
    let id = e.elementos[2].id;
    let p = e.buscar_mut(id).unwrap();
    // A la derecha del centro: un cuarto de vuelta.
    let (x0, y0, x1, y1) = p.caja();
    let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    assert!(transformar::girar_el_espacio(
        p,
        Punto2::nuevo(c.x + 100.0, c.y),
        false
    ));
    assert_eq!(p.angulo, 0.0);
    let a = instrumento::de(p).unwrap().azimut.unwrap();
    assert!((a - 90.0).abs() < 1e-3, "{a}");
    let salida = json(&escribir(&con_escena(&l, &e)));
    assert!((salida["elements"][2]["azimut"].as_f64().unwrap() - 90.0).abs() < 1e-3);
    assert_eq!(salida["elements"][2]["angle"], 0.0);
    // Caso negativo: el plano no es un espacio y no se deja girar asi.
    let plano = e.elementos[1].clone();
    let mut copia = plano.clone();
    assert!(!transformar::girar_el_espacio(
        &mut copia,
        Punto2::nuevo(0.0, 0.0),
        false
    ));
    assert_eq!(copia, plano);
}

#[test]
fn un_instrumento_nacido_aqui_sale_con_las_claves_del_movil() {
    let mut e = pixpin_motor2d::Escena::nueva();
    let nuevo = instrumento::elemento_de_fabrica(
        3,
        pixpin_motor2d::ColorRgba::opaco(0.0, 0.0, 0.0),
        2.0,
        1.0,
    );
    e.anadir(nuevo);
    let salida = json(&escribir(&con_escena(
        &pixpin_motor2d::excalidraw::Lienzo::vacio(),
        &e,
    )));
    let el = &salida["elements"][0];
    assert_eq!(el["type"], "pixpin-space");
    assert_eq!(el["unidad"], 40.0);
    assert_eq!(el["pasoDeNumeros"], 1.0);
    assert_eq!(el["pasoDeCuadros"], 1.0);
    assert_eq!(el["azimut"], 0.0);
    assert_eq!(el["elevacion"], 25.0);
    assert_eq!(el["roughness"], 0);
    // Y se vuelve a leer igual.
    let vuelta = leer(&salida.to_string()).unwrap();
    assert!(matches!(
        &vuelta.entradas[0],
        Entrada::Nuestro { elemento, .. } if instrumento::de(elemento) == Some(&Instrumento::de_fabrica(3))
    ));
}

#[test]
fn una_unidad_que_no_es_un_numero_se_dibuja_con_la_de_fabrica_y_vuelve_intacta() {
    let texto = del_movil().replace("\"unidad\":40.0", "\"unidad\":\"cm\"");
    assert!(texto.contains("\"cm\""));
    let mut l = leer(&texto).unwrap();
    let i = instrumento::de(&l.elementos()[0]).copied().unwrap();
    assert_eq!(i.unidad, None);
    assert_eq!(i.unidad_util(), 40.0);
    for en in &mut l.entradas {
        if let Entrada::Nuestro { elemento, .. } = en {
            elemento.mover(5.0, 0.0);
        }
    }
    let salida = json(&escribir(&l));
    assert_eq!(salida["elements"][0]["unidad"], "cm");
}

#[test]
fn la_tabla_editada_aqui_vuelve_como_la_lee_el_movil_y_se_deshace() {
    let (l, mut e) = escena_y_lienzo();
    assert_eq!(e.tablas.len(), 2);
    assert_eq!(e.origen_coordenadas, Some(Punto2::nuevo(560.0, 170.0)));
    let mut tablas = e.tablas.clone();
    tablas[0].puntos.push(PuntoDeTabla { x: -1.0, y: 4.5 });
    tablas.push(TablaDeCoordenadas {
        id: "pc1".into(),
        nombre: String::new(),
        color: "#2f9e44".into(),
        puntos: vec![PuntoDeTabla { x: 2.0, y: 2.0 }],
        visible: true,
    });
    e.abrir_paso();
    assert!(e.poner_tablas(tablas.clone(), e.origen_coordenadas));
    e.cerrar_paso();
    let salida = json(&escribir(&con_escena(&l, &e)));
    let t = &salida["tablas"];
    assert_eq!(t.as_array().unwrap().len(), 3);
    assert_eq!(t[0]["puntos"][2], serde_json::json!({"x": -1.0, "y": 4.5}));
    // Lo que el PC no conoce de la tabla sigue ahi, y la que no se toco
    // vuelve tal cual (sin estrenar `nombre`).
    assert_eq!(t[0]["futuro"], 42);
    assert_eq!(t[1], json(&del_movil())["tablas"][1]);
    assert_eq!(t[2]["color"], "#2f9e44");
    assert_eq!(t[2]["visible"], true);
    // Deshacer devuelve las tablas de antes.
    assert!(e.deshacer());
    assert_eq!(e.tablas.len(), 2);
    assert_eq!(escribir(&con_escena(&l, &e)), escribir(&l));
    // Caso negativo: poner las mismas no gasta un Ctrl+Z.
    let mismas = e.tablas.clone();
    assert!(!e.poner_tablas(mismas, e.origen_coordenadas));
}

#[test]
fn unas_tablas_rotas_no_se_pisan_si_aqui_no_se_cambian() {
    let texto = del_movil().replace("\"tablas\":[", "\"tablas\":\"rota\",\"x\":[");
    let l = leer(&texto).unwrap();
    let e = a_escena(&l);
    assert!(e.tablas.is_empty());
    assert_eq!(escribir(&con_escena(&l, &e)), escribir(&l));
}

#[test]
fn los_puntos_tecleados_salen_al_exportar_y_el_eje_no() {
    let (_, e) = escena_y_lienzo();
    let hojas =
        pixpin_motor2d::exportar::hojas(&e, pixpin_motor2d::exportar::Alcance::Todo, &[], None);
    let rotulos: Vec<String> = hojas[0]
        .ordenes
        .iter()
        .filter_map(|o| match o {
            pixpin_motor2d::Orden::Rotulo { texto, .. } => Some(texto.clone()),
            _ => None,
        })
        .collect();
    // Solo la tabla visible: `1'` y `2'`; la escondida no sale.
    assert_eq!(rotulos, ["1'", "2'"]);
    // Las cifras de los tres instrumentos salen como texto.
    assert!(hojas[0].ordenes.iter().any(|o| matches!(o,
        pixpin_motor2d::Orden::Texto { texto, .. } if texto == "z")));
    // El (3, -2) de la tabla cae a 3 a la derecha y 2 por debajo del eje.
    let p = pixpin_motor2d::instrumento::tabla::punto_en_escena(
        e.origen_coordenadas.unwrap(),
        PuntoDeTabla { x: 3.0, y: -2.0 },
        None,
    );
    assert_eq!((p.x, p.y), (563.0, 172.0));
}

#[test]
fn el_iman_se_pega_a_un_punto_tecleado_y_cerca_del_cero_manda_el_cero() {
    use pixpin_motor2d::enganche::{Ajustes, Faena, TipoAnclaje, con_coordenadas};
    let config = Ajustes::default();
    let o = Punto2::nuevo(0.0, 0.0);
    let tecleado = [Punto2::nuevo(100.0, -50.0)];
    let a = con_coordenadas(
        None,
        Punto2::nuevo(103.0, -48.0),
        1.0,
        Faena::Trazando,
        &config,
        Some(o),
        &tecleado,
    )
    .unwrap();
    assert_eq!(a.punto, tecleado[0]);
    let cero = con_coordenadas(
        None,
        Punto2::nuevo(3.0, 1.0),
        1.0,
        Faena::Trazando,
        &config,
        Some(o),
        &[],
    )
    .unwrap();
    assert_eq!((cero.punto, cero.tipo), (o, TipoAnclaje::Eje));
    // Sobre la recta del eje, lejos del cero: la proyeccion.
    let recta = con_coordenadas(
        None,
        Punto2::nuevo(200.0, 4.0),
        1.0,
        Faena::Trazando,
        &config,
        Some(o),
        &[],
    )
    .unwrap();
    assert_eq!(recta.punto, Punto2::nuevo(200.0, 0.0));
    // Caso negativo: a mano alzada no tira nada.
    assert!(
        con_coordenadas(
            None,
            Punto2::nuevo(3.0, 1.0),
            1.0,
            Faena::AMano,
            &config,
            Some(o),
            &tecleado
        )
        .is_none()
    );
}
