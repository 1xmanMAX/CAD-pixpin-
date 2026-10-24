use super::*;
use crate::mensajero::{respuesta, validar_pedido};

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// Los «ficheros que existen» de las pruebas: los que acaban en lo que
/// parece una extension y empiezan por una unidad.
fn existe(p: &Path) -> bool {
    let s = p.to_string_lossy();
    s.starts_with("C:\\") && p.extension().is_some()
}

fn orden(a: &[&str]) -> Result<Llamada, Fallo> {
    de_argumentos(&args(a), &existe).expect("es una orden")
}

fn pedidos_de(a: &[&str]) -> Vec<Value> {
    match orden(a).unwrap().orden {
        Orden::Pedidos(p) => p,
        otra => panic!("no son pedidos: {otra:?}"),
    }
}

fn pedidos_del_enlace(u: &str) -> Vec<Value> {
    match de_enlace(u).unwrap().orden {
        Orden::Pedidos(p) => p,
        otra => panic!("no son pedidos: {otra:?}"),
    }
}

fn valido(p: &Value) {
    assert_eq!(validar_pedido(&p.to_string()), respuesta::ACEPTADO, "{p}");
}

#[test]
fn los_verbos_dan_el_pedido_del_protocolo() {
    let casos: Vec<(Vec<&str>, Value)> = vec![
        (vec!["capturar"], json!({"pixpin":1,"accion":"capturar"})),
        (
            vec!["tarea", "comprar", "yeso"],
            json!({"pixpin":1,"accion":"anadir_tarea","texto":"comprar yeso"}),
        ),
        (
            vec!["TAREA", "comprar yeso", "--proyecto", "Obra"],
            json!({"pixpin":1,"accion":"anadir_tarea","texto":"comprar yeso","proyecto":"Obra"}),
        ),
        (
            vec!["tarea"],
            json!({"pixpin":1,"accion":"ventana","cual":"tareas"}),
        ),
        (
            vec!["nota"],
            json!({"pixpin":1,"accion":"nota_nueva","proyecto":null}),
        ),
        (
            vec!["nota", "idea", "--proyecto=pr-1"],
            json!({"pixpin":1,"accion":"nota_nueva","texto":"idea","proyecto":"pr-1"}),
        ),
        (
            vec!["chat"],
            json!({"pixpin":1,"accion":"abrir","que":{"tipo":"proyecto","proyecto":null}}),
        ),
        (
            vec!["mensajes"],
            json!({"pixpin":1,"accion":"abrir","que":{"tipo":"proyecto","proyecto":null}}),
        ),
        (
            vec!["proyectos"],
            json!({"pixpin":1,"accion":"ventana_principal"}),
        ),
        (
            vec!["proyecto", "pr-7"],
            json!({"pixpin":1,"accion":"abrir","que":{"tipo":"proyecto","proyecto":"pr-7"}}),
        ),
        (
            vec!["pinear-ultima"],
            json!({"pixpin":1,"accion":"pinear_ultima"}),
        ),
        (
            vec!["grabar"],
            json!({"pixpin":1,"accion":"grabar","nombre":"Nota de voz","proyecto":null}),
        ),
        (
            vec!["lista"],
            json!({"pixpin":1,"accion":"lista_nueva","titulo":"Tareas","proyecto":null}),
        ),
        (
            vec!["sincronizar"],
            json!({"pixpin":1,"accion":"ventana","cual":"sincronizar"}),
        ),
        (
            vec!["sincronizar", "todos"],
            json!({"pixpin":1,"accion":"sincronizar","aparato":"todos"}),
        ),
        (
            vec!["ventana", "galeria"],
            json!({"pixpin":1,"accion":"ventana","cual":"galeria"}),
        ),
        (
            vec!["capturas"],
            json!({"pixpin":1,"accion":"ventana","cual":"galeria"}),
        ),
        (
            vec!["lecciones", "encofrado"],
            json!({"pixpin":1,"accion":"lecciones","consulta":"encofrado","proyecto":null}),
        ),
        (
            vec!["ia", "resume", "mis", "tareas"],
            json!({"pixpin":1,"accion":"asistente","texto":"resume mis tareas","proyecto":null}),
        ),
        (
            vec!["asistente"],
            json!({"pixpin":1,"accion":"asistente","texto":"","proyecto":null}),
        ),
        (
            vec!["leer", "hola", "mundo", "--titulo", "Web"],
            json!({"pixpin":1,"accion":"leer_en_voz","texto":"hola mundo","titulo":"Web"}),
        ),
    ];
    for (a, esperado) in casos {
        let p = pedidos_de(&a);
        assert_eq!(p, vec![esperado.clone()], "{a:?}");
        valido(&p[0]);
    }
}

#[test]
fn los_ficheros_van_como_imagenes_o_archivos() {
    let p = pedidos_de(&[
        "chat",
        "mira",
        "C:\\a\\foto.PNG",
        "C:\\a\\plano.pdf",
        "--proyecto",
        "pr-1",
    ]);
    assert_eq!(
        p,
        vec![
            json!({"pixpin":1,"accion":"chat","texto":"mira","proyecto":"pr-1","imagenes":["C:\\a\\foto.PNG"],"archivos":["C:\\a\\plano.pdf"]})
        ]
    );
    // «Enviar a» con varios: un pin por fichero.
    let p = pedidos_de(&["pinear", "C:\\a\\uno.png", "C:\\a\\dos.pdf"]);
    assert_eq!(p.len(), 2);
    assert_eq!(
        p[1],
        json!({"pixpin":1,"accion":"pinear","ruta":"C:\\a\\dos.pdf"})
    );
    let p = pedidos_de(&["al-movil", "C:\\a\\uno.jpg", "C:\\a\\dos.docx"]);
    assert_eq!(
        p,
        vec![
            json!({"pixpin":1,"accion":"enviar_al_movil","aparato":"todos","imagenes":["C:\\a\\uno.jpg"],"archivos":["C:\\a\\dos.docx"]})
        ]
    );
    let p = pedidos_de(&[
        "abrir",
        "C:\\a\\libro.pdf",
        "--pagina",
        "12",
        "--buscar",
        "losa",
    ]);
    assert_eq!(
        p,
        vec![
            json!({"pixpin":1,"accion":"abrir","que":{"tipo":"fichero","ruta":"C:\\a\\libro.pdf","pagina":12,"buscar":"losa"}})
        ]
    );
    let p = pedidos_de(&["copiar-texto", "C:\\a\\uno.png", "C:\\a\\dos.jpg"]);
    assert_eq!(
        p,
        vec![json!({"pixpin":1,"accion":"ocr","ruta":"C:\\a\\uno.png","rutas":["C:\\a\\dos.jpg"]})]
    );
    for x in p {
        valido(&x);
    }
}

#[test]
fn ocr_y_preguntar_se_hacen_aqui() {
    assert_eq!(
        orden(&["ocr", "C:\\a\\uno.png", "--copiar"]).unwrap().orden,
        Orden::Ocr {
            rutas: vec![PathBuf::from("C:\\a\\uno.png")],
            copiar: true
        }
    );
    // Ficheros sin pregunta: la caja.
    assert_eq!(
        orden(&["asistente", "C:\\a\\plano.pdf"]).unwrap().orden,
        Orden::Preguntar {
            rutas: vec![PathBuf::from("C:\\a\\plano.pdf")],
            proyecto: None
        }
    );
    // Con pregunta: las imagenes las ve el modelo y del resto va la ruta.
    let p = pedidos_de(&[
        "asistente",
        "que dice",
        "C:\\a\\foto.png",
        "C:\\a\\plano.pdf",
    ]);
    assert_eq!(p[0]["imagenes"], json!(["C:\\a\\foto.png"]));
    assert_eq!(
        p[0]["texto"],
        json!("que dice\n\nArchivos:\n- C:\\a\\plano.pdf")
    );
    assert_eq!(orden(&["ayuda"]).unwrap().orden, Orden::Ayuda);
    assert_eq!(orden(&["--help"]).unwrap().orden, Orden::Ayuda);
}

#[test]
fn caso_negativo_lo_que_no_se_entiende_sale_con_su_codigo() {
    for (a, c) in [
        (vec!["tarea", "x", "--proyeto", "P"], codigo::NO_SE_ENTIENDE),
        (vec!["tarea", "x", "--proyecto"], codigo::NO_SE_ENTIENDE),
        (vec!["pinear"], codigo::NO_SE_ENTIENDE),
        (vec!["pinear", "C:\\no\\esta"], codigo::FALLO),
        (vec!["ocr", "C:\\a\\plano.pdf"], codigo::FALLO),
        (vec!["ventana", "cocina"], codigo::NO_SE_ENTIENDE),
        (
            vec!["abrir", "C:\\a\\x.pdf", "--pagina", "0"],
            codigo::NO_SE_ENTIENDE,
        ),
        (
            vec!["tarea", "x", "C:\\a\\plano.pdf"],
            codigo::NO_SE_ENTIENDE,
        ),
        (vec!["--pedido"], codigo::NO_SE_ENTIENDE),
        (vec!["--pedido", "no es json"], codigo::NO_SE_ENTIENDE),
        (
            vec!["--pedido", r#"{"pixpin":1,"accion":"borrar_todo"}"#],
            codigo::NO_SE_ENTIENDE,
        ),
        (
            vec!["--pedido", r#"{"pixpin":2,"accion":"chat"}"#],
            codigo::VERSION,
        ),
        (
            vec!["--url", "pixpin://ocr?ruta=C:\\a.png"],
            codigo::RECHAZADO,
        ),
        (
            vec!["--pedido-archivo", "C:\\no\\existe\\pedido.json"],
            codigo::FALLO,
        ),
    ] {
        let f = orden(&a).unwrap_err();
        assert_eq!(f.codigo, c, "{a:?}: {}", f.motivo);
        assert!(!f.motivo.is_empty());
    }
}

#[test]
fn caso_negativo_lo_que_no_es_una_orden_sigue_el_arranque_de_siempre() {
    for a in [
        vec![],
        vec!["C:\\a\\foto.png"],
        vec!["--atajo", "capturar"],
        vec!["--cad-convertir", "x"],
        vec!["hola"],
    ] {
        assert!(de_argumentos(&args(&a), &existe).is_none(), "{a:?}");
    }
}

#[test]
fn un_pedido_a_mano_pasa_tal_cual() {
    let j = r#"{"pixpin":1,"accion":"anadir_tarea","texto":"pan"}"#;
    let l = orden(&["--pedido", j, "--sin-arrancar"]).unwrap();
    assert!(l.sin_arrancar);
    assert_eq!(l.origen, Origen::Consola);
    assert_eq!(
        l.orden,
        Orden::Pedidos(vec![serde_json::from_str(j).unwrap()])
    );
    // Desde un fichero con el BOM de PowerShell.
    let ruta = std::env::temp_dir().join(format!("pixpin-orden-{}.json", std::process::id()));
    std::fs::write(&ruta, format!("\u{feff}{j}\r\n")).unwrap();
    let l = orden(&["--pedido-archivo", ruta.to_str().unwrap()]).unwrap();
    std::fs::remove_file(&ruta).unwrap();
    assert_eq!(
        l.orden,
        Orden::Pedidos(vec![serde_json::from_str(j).unwrap()])
    );
}

#[test]
fn los_enlaces_dan_los_mismos_pedidos() {
    assert_eq!(
        pedidos_del_enlace("pixpin://tarea?texto=comprar%20yeso&proyecto=pr-1"),
        vec![json!({"pixpin":1,"accion":"anadir_tarea","texto":"comprar yeso","proyecto":"pr-1"})]
    );
    assert_eq!(
        pedidos_del_enlace("PIXPIN://Capturar/"),
        vec![json!({"pixpin":1,"accion":"capturar"})]
    );
    assert_eq!(
        pedidos_del_enlace("pixpin://chat?texto=hola+mundo"),
        vec![json!({"pixpin":1,"accion":"chat","texto":"hola mundo","proyecto":null})]
    );
    assert_eq!(
        pedidos_del_enlace("pixpin://abrir?tipo=mensaje&proyecto=pr-1&codigo=m-9"),
        vec![
            json!({"pixpin":1,"accion":"abrir","que":{"tipo":"mensaje","proyecto":"pr-1","codigo":"m-9"}})
        ]
    );
    assert_eq!(
        pedidos_del_enlace("pixpin://abrir?ruta=C:%5Cobra%5Cplano.pdf&pagina=3"),
        vec![
            json!({"pixpin":1,"accion":"abrir","que":{"tipo":"fichero","ruta":"C:\\obra\\plano.pdf","pagina":3}})
        ]
    );
    assert_eq!(
        pedidos_del_enlace(
            "pixpin://pedido?json=%7B%22pixpin%22%3A1%2C%22accion%22%3A%22capturar%22%7D"
        ),
        vec![json!({"pixpin":1,"accion":"capturar"})]
    );
    // Acentos en UTF-8.
    assert_eq!(
        pedidos_del_enlace("pixpin://nota?texto=a%C3%B1o")[0]["texto"],
        json!("año")
    );
    // Un campo repetido no cambia lo ya leido.
    assert_eq!(
        pedidos_del_enlace("pixpin://tarea?texto=uno&texto=dos")[0]["texto"],
        json!("uno")
    );
}

/// Las señas de `Atajos.kt` de Android, todas: el mismo enlace sirve en
/// los dos aparatos.
#[test]
fn las_senas_de_android_funcionan_en_el_pc() {
    let casos = [
        (
            "pixpin://atajo/capturar",
            json!({"pixpin":1,"accion":"capturar"}),
        ),
        (
            "pixpin://atajo/mensajes",
            json!({"pixpin":1,"accion":"abrir","que":{"tipo":"proyecto","proyecto":null}}),
        ),
        (
            "pixpin://atajo/proyectos",
            json!({"pixpin":1,"accion":"ventana_principal"}),
        ),
        (
            "pixpin://atajo/sincronizar",
            json!({"pixpin":1,"accion":"ventana","cual":"sincronizar"}),
        ),
        (
            "pixpin://atajo/proyecto/pr-1789",
            json!({"pixpin":1,"accion":"abrir","que":{"tipo":"proyecto","proyecto":"pr-1789"}}),
        ),
        (
            "pixpin://atajo/archivo/m%201",
            json!({"pixpin":1,"accion":"pinear","codigo":"m 1","proyecto":null}),
        ),
        (
            "pixpin://atajo/grabar",
            json!({"pixpin":1,"accion":"grabar","nombre":"Nota de voz","proyecto":null}),
        ),
        (
            "pixpin://atajo/tarea?texto=pan",
            json!({"pixpin":1,"accion":"anadir_tarea","texto":"pan"}),
        ),
        (
            "pixpin://atajo/tarea",
            json!({"pixpin":1,"accion":"ventana","cual":"tareas"}),
        ),
        (
            "pixpin://atajo/tareas",
            json!({"pixpin":1,"accion":"ventana","cual":"tareas"}),
        ),
        (
            "pixpin://atajo/capturas",
            json!({"pixpin":1,"accion":"ventana","cual":"galeria"}),
        ),
        (
            "pixpin://atajo/soltar?proyecto=pr-1",
            json!({"pixpin":1,"accion":"soltar","proyecto":"pr-1"}),
        ),
        (
            "pixpin://atajo/chat?texto=hola&proyecto=pr-1",
            json!({"pixpin":1,"accion":"chat","texto":"hola","proyecto":"pr-1"}),
        ),
        (
            "pixpin://atajo/chat",
            json!({"pixpin":1,"accion":"abrir","que":{"tipo":"proyecto","proyecto":null}}),
        ),
        (
            "pixpin://atajo/nota?texto=idea",
            json!({"pixpin":1,"accion":"nota_nueva","texto":"idea","proyecto":null}),
        ),
        (
            "pixpin://atajo/lista?titulo=Compra&proyecto=pr-1",
            json!({"pixpin":1,"accion":"lista_nueva","titulo":"Compra","proyecto":"pr-1"}),
        ),
        (
            "pixpin://atajo/leccion",
            json!({"pixpin":1,"accion":"leccion_nueva","proyecto":null}),
        ),
        (
            "pixpin://atajo/lecciones",
            json!({"pixpin":1,"accion":"lecciones","proyecto":null}),
        ),
    ];
    for (u, esperado) in casos {
        let p = pedidos_del_enlace(u);
        assert_eq!(p, vec![esperado], "{u}");
        valido(&p[0]);
        // Y todas son de la lista blanca, menos grabar (el microfono).
        let s = seguridad(&p[0]);
        if u.contains("grabar") {
            assert!(matches!(s, Seguridad::Confirmar(_)), "{u}");
        } else {
            assert_eq!(s, Seguridad::Directa, "{u}");
        }
    }
    // `buscar` de Android aun no tiene pedido en el PC: se dice, no se inventa.
    assert_eq!(
        de_enlace("pixpin://atajo/buscar?texto=x")
            .unwrap_err()
            .codigo,
        codigo::NO_SE_ENTIENDE
    );
}

#[test]
fn caso_negativo_enlaces_rotos() {
    for (u, c) in [
        ("pixpin://", codigo::NO_SE_ENTIENDE),
        ("pixpin://atajo", codigo::NO_SE_ENTIENDE),
        ("pixpin://borrar_todo", codigo::NO_SE_ENTIENDE),
        ("pixpin://tarea?texto=%ZZ", codigo::NO_SE_ENTIENDE),
        ("pixpin://tarea?texto=%FF%FE", codigo::NO_SE_ENTIENDE),
        ("http://tarea?texto=x", codigo::NO_SE_ENTIENDE),
        ("pixpin://ayuda", codigo::RECHAZADO),
        ("pixpin://al-movil?ruta=C:\\a.png", codigo::RECHAZADO),
    ] {
        assert_eq!(de_enlace(u).unwrap_err().codigo, c, "{u}");
    }
    let largo = format!("pixpin://tarea?texto={}", "a".repeat(TOPE_ENLACE));
    assert_eq!(
        de_enlace(&largo).unwrap_err().codigo,
        codigo::NO_SE_ENTIENDE
    );
    // Por enlace, una ruta suelta en el camino es texto, nunca un fichero.
    let p = pedidos_del_enlace("pixpin://chat/C:%5Cfoto.png");
    assert!(p[0].get("imagenes").is_none());
}

/// **Enlaces maliciosos**: lo que una pagina web podria intentar.
#[test]
fn caso_negativo_un_enlace_malicioso_no_pasa() {
    let prohibidos = [
        // Una ruta de red: Windows mandaria las credenciales al servidor.
        "pixpin://pinear?ruta=%5C%5Catacante.example%5Ccomparte%5Cx.png",
        "pixpin://abrir?ruta=//atacante.example/x.pdf",
        "pixpin://abrir?ruta=%5C%5C%3F%5CC:%5Cwindows%5Cx.pdf",
        "pixpin://abrir?ruta=file:///C:/x.pdf",
        "pixpin://abrir?ruta=..%5C..%5Cx.pdf",
        // Crear una automatizacion que corra sola cada dia.
        "pixpin://pedido?json=%7B%22pixpin%22%3A1%2C%22accion%22%3A%22automatizacion%22%2C%22operacion%22%3A%22crear%22%2C%22receta%22%3A%7B%7D%7D",
    ];
    for u in prohibidos {
        let p = pedidos_del_enlace(u);
        assert!(
            matches!(seguridad(&p[0]), Seguridad::Prohibida(_)),
            "{u}: {:?}",
            seguridad(&p[0])
        );
    }
    let a_confirmar = [
        "pixpin://pedido?json=%7B%22pixpin%22%3A1%2C%22accion%22%3A%22borrar_lista%22%2C%22codigo%22%3A%221%22%7D",
        "pixpin://pedido?json=%7B%22pixpin%22%3A1%2C%22accion%22%3A%22quitar_tarea%22%2C%22codigo%22%3A%221%22%2C%22indice%22%3A0%7D",
        "pixpin://pedido?json=%7B%22pixpin%22%3A1%2C%22accion%22%3A%22automatizacion%22%2C%22operacion%22%3A%22ejecutar%22%2C%22id%22%3A%22a%22%7D",
        "pixpin://sincronizar/todos",
        "pixpin://asistente?texto=borra%20todas%20mis%20tareas",
        "pixpin://pinear?ruta=C:%5Ca%5Cx.png",
        "pixpin://abrir?ruta=C:%5Ca%5Cx.pdf",
        "pixpin://grabar",
    ];
    for u in a_confirmar {
        let p = pedidos_del_enlace(u);
        assert!(
            matches!(seguridad(&p[0]), Seguridad::Confirmar(_)),
            "{u}: {:?}",
            seguridad(&p[0])
        );
    }
    // Una firma inventada no convierte un enlace en «de la casa».
    let l = de_enlace(&format!(
        "pixpin://sincronizar/todos?firma={}",
        "ab".repeat(32)
    ))
    .unwrap();
    assert_eq!(l.origen, Origen::Enlace { firmado: false });
}

#[test]
fn toda_accion_del_protocolo_tiene_su_seguridad() {
    // Lo que no esta en la lista blanca pregunta: ninguna accion nueva entra
    // por enlace sin que alguien la ponga a proposito en `SEGURAS`.
    for a in crate::mensajero::ACCIONES {
        let s = seguridad(&json!({"pixpin":1,"accion":a}));
        if SEGURAS.contains(&a) || a == "asistente" {
            assert_eq!(s, Seguridad::Directa, "{a}");
        } else {
            assert!(matches!(s, Seguridad::Confirmar(_)), "{a}: {s:?}");
        }
    }
    for s in SEGURAS {
        assert!(
            crate::mensajero::ACCIONES.contains(&s),
            "{s} no es una accion"
        );
    }
}

#[test]
fn las_rutas_locales_y_las_que_no() {
    for r in ["C:\\a.png", "d:/fotos/x.jpg"] {
        assert!(es_ruta_local(r), "{r}");
    }
    for r in [
        "\\\\srv\\x",
        "//srv/x",
        "\\\\?\\C:\\x",
        "\\\\.\\pipe\\x",
        "x.png",
        "..\\x",
        "file:///C:/x",
        "C:",
        "C:x.png",
        "",
    ] {
        assert!(!es_ruta_local(r), "{r}");
    }
}

#[test]
fn la_firma_va_y_vuelve_y_no_se_puede_falsificar() {
    let clave = [7u8; 32];
    let url = firma::firmar_con(&clave, "pixpin://pedido?json=%7B%22pixpin%22%3A1%7D&b=2").unwrap();
    let partes = partir_enlace(&url).unwrap();
    let f = partes.firma.clone().unwrap();
    assert!(firma::verificar_con(&clave, &partes.canonico(), &f));
    // Reescrito por el camino (+ en vez de %20, otro orden, barra final):
    // la forma canonica es la misma.
    let a = partir_enlace("pixpin://tarea?texto=a%20b&proyecto=p").unwrap();
    let b = partir_enlace("pixpin://Tarea/?proyecto=p&texto=a+b").unwrap();
    assert_eq!(a.canonico(), b.canonico());
    // Casos negativos: otra clave, otro contenido, una firma cortada.
    assert!(!firma::verificar_con(&[8u8; 32], &partes.canonico(), &f));
    assert!(!firma::verificar_con(&clave, &a.canonico(), &f));
    assert!(!firma::verificar_con(&clave, &partes.canonico(), &f[..62]));
    assert!(!firma::verificar_con(&clave, &partes.canonico(), "zz"));
    // Firmar dos veces no amontona firmas.
    let otra = firma::firmar_con(&clave, &url).unwrap();
    assert_eq!(otra.matches("firma=").count(), 1);
}

#[test]
fn la_ayuda_tiene_todos_los_verbos_y_los_codigos() {
    let a = ayuda();
    for v in VERBOS {
        assert!(a.contains(v.uso), "{}", v.uso);
    }
    for n in ["0 aceptado", "4 PixPin", "6 rechazado"] {
        assert!(a.contains(n), "{n}");
    }
    // Ningun nombre de verbo esta repetido en dos filas.
    let mut vistos = std::collections::HashSet::new();
    for v in VERBOS {
        for n in v.nombres {
            assert!(vistos.insert(*n), "«{n}» en dos filas");
        }
    }
}

#[test]
fn codificar_y_descodificar_van_y_vuelven() {
    for t in [
        "comprar yeso",
        "año ñu — 2 kg",
        "a&b=c?d/e%f+g",
        "C:\\obra\\plano.pdf",
    ] {
        assert_eq!(descodificar(&codificar(t), true).unwrap(), t);
    }
    assert_eq!(
        enlace_con("pixpinmax", "tarea", &[("texto", "a b&c")]),
        "pixpinmax://tarea?texto=a%20b%26c"
    );
    assert_eq!(
        pedidos_del_enlace(&enlace("tarea", &[("texto", "a b&c")]))[0]["texto"],
        json!("a b&c")
    );
}

#[test]
fn pixpinmax_y_pixpin_son_el_mismo_enlace() {
    // En el PC, `pixpin:` puede ser del PixPin original: `pixpinmax:` es el
    // que siempre es nuestro, y los dos dan el mismo pedido.
    assert_eq!(
        pedidos_del_enlace("pixpinmax://tarea?texto=pan"),
        pedidos_del_enlace("pixpin://tarea?texto=pan")
    );
    assert_eq!(
        pedidos_del_enlace("PixPinMax://atajo/capturar"),
        vec![json!({"pixpin":1,"accion":"capturar"})]
    );
    assert!(es_enlace("pixpinmax:capturar") && es_enlace("PIXPIN://x"));
    // Caso negativo: lo que solo se le parece no es nuestro.
    for u in [
        "pixpi://x",
        "pixpinmaxi://x",
        "xpixpin://x",
        "pixpin",
        "ñpixpin://x",
    ] {
        assert!(!es_enlace(u) || u.starts_with("pixpinmaxi"), "{u}");
    }
    assert_eq!(
        de_enlace("pixpinmaxi://capturar").unwrap_err().codigo,
        codigo::NO_SE_ENTIENDE
    );
}

#[test]
fn una_sola_lista_blanca_para_enlaces_y_botones_de_avisos() {
    // Lo de los botones de los avisos (`notificacion`) pasa directo...
    for p in [
        json!({"pixpin":1,"accion":"marcar_tarea","codigo":"L","indice":0,"hecha":true}),
        json!({"pixpin":1,"accion":"posponer","proyecto":"p","mensaje":"m","minutos":10}),
        json!({"pixpin":1,"accion":"conservar_captura","ruta":r"C:\c\captura-1.png"}),
    ] {
        assert_eq!(seguridad(&p), Seguridad::Directa, "{p}");
        assert!(crate::notificacion::permitido(&p), "{p}");
    }
    // ...pero conservar una captura de red, no.
    let red = json!({"pixpin":1,"accion":"conservar_captura","ruta":r"\\srv\c.png"});
    assert!(matches!(seguridad(&red), Seguridad::Prohibida(_)));
    assert!(!crate::notificacion::permitido(&red));
    // `pixpinmax:pedido?json=` (la sena de los avisos) es un alias del mismo
    // traductor, tambien por `--atajo url`.
    let sena = crate::notificacion::sena_con("pixpinmax", &json!({"pixpin":1,"accion":"capturar"}));
    assert_eq!(
        pedidos_del_enlace(&sena),
        vec![json!({"pixpin":1,"accion":"capturar"})]
    );
    let l = orden(&["--atajo", "url", &sena]).unwrap();
    assert_eq!(
        l.orden,
        Orden::Pedidos(vec![json!({"pixpin":1,"accion":"capturar"})])
    );
    assert!(matches!(l.origen, Origen::Enlace { .. }));
    // `--atajo capturar` (la Jump List) sigue siendo cosa del arranque.
    assert!(de_argumentos(&args(&["--atajo", "capturar"]), &existe).is_none());
}

/// 17-oct: `al-lienzo <nombre> …` manda texto y fotos a un lienzo por su
/// nombre, como `chat` a un chat.
#[test]
fn al_lienzo_manda_texto_y_fotos_al_lienzo_por_su_nombre() {
    let p = pedidos_de(&[
        "al-lienzo",
        "Plano",
        "la puerta",
        "C:\\a\\foto.png",
        "--proyecto",
        "pr-1",
    ]);
    assert_eq!(
        p,
        vec![
            json!({"pixpin":1,"accion":"al_lienzo","lienzo":"Plano","texto":"la puerta","proyecto":"pr-1","imagenes":["C:\\a\\foto.png"]})
        ]
    );
    valido(&p[0]);
    // Con espacios en el nombre, por --lienzo.
    let p = pedidos_de(&["al-lienzo", "--lienzo", "Plano casa", "hola"]);
    assert_eq!(p[0]["lienzo"], "Plano casa");
    assert_eq!(p[0]["texto"], "hola");
    // Casos negativos: sin nombre, sin nada que mandar, o con un fichero que
    // no es imagen.
    assert!(orden(&["al-lienzo"]).is_err());
    assert!(orden(&["al-lienzo", "Plano"]).is_err());
    assert!(orden(&["al-lienzo", "Plano", "C:\\a\\plano.pdf"]).is_err());
}
