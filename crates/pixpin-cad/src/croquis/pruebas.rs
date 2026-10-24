//! Pruebas del croquis: los casos de `Croquis3DTest.kt` y `PulsoDelMundoTest.kt`
//! que tocan a ver (la camara, las hojas, los cuerpos, el sol, leer un
//! croquis viejo, el pulso), los de la malla, y un PNG pintado en la CPU con
//! las mismas cuentas que el sombreador.

use std::f64::consts::PI;

use super::malla::{CONTRASTE, Foto, SIN_LUZ, a_modelo, abre_claro, caja_de_lo_dibujado};
use super::*;
use crate::modelo3d::Modelo3d;
use crate::ventana3d::Orbita;

const ANCHO: f64 = 1000.0;
const ALTO: f64 = 2000.0;
const TOL: f64 = 1e-9;

fn camara(giro: f64, inclinacion: f64, zoom: f64) -> Camara3D {
    Camara3D {
        giro,
        inclinacion,
        zoom,
        ..Camara3D::default()
    }
}

// ---- La camara (Camara3D.kt) ----

#[test]
fn el_centro_siempre_cae_en_el_centro() {
    for g in [0.0, 1.0, 2.5, -3.0] {
        for i in [-1.0, 0.0, 0.9] {
            let c = camara(g, i, 1.0);
            let p = c.a_pantalla(c.centro, ANCHO, ALTO);
            assert!((p.x - ANCHO / 2.0).abs() < TOL && (p.y - ALTO / 2.0).abs() < TOL);
        }
    }
}

#[test]
fn los_ejes_de_la_camara_estan_sanos() {
    for g in [0.0, 0.7, -2.2] {
        for i in [-1.2, 0.0, 1.2] {
            let c = camara(g, i, 1.0);
            for v in [c.adelante(), c.derecha(), c.arriba()] {
                assert!((largo(v) - 1.0).abs() < TOL);
            }
            assert!(escalar(c.adelante(), c.derecha()).abs() < TOL);
            assert!(escalar(c.adelante(), c.arriba()).abs() < TOL);
            assert!(escalar(c.derecha(), c.arriba()).abs() < TOL);
        }
    }
}

#[test]
fn del_dedo_al_mundo_y_de_vuelta() {
    for g in [0.0, 0.9, -2.0] {
        for i in [-0.8, 0.3, 1.1] {
            let c = camara(g, i, 1.7);
            let plano = Plano3D {
                punto: c.centro,
                normal: c.adelante(),
            };
            for p in [
                Pt { x: 10.0, y: 10.0 },
                Pt {
                    x: 500.0,
                    y: 1200.0,
                },
                Pt {
                    x: 999.0,
                    y: 1999.0,
                },
            ] {
                let corte = plano.corte(&c.rayo(p, ANCHO, ALTO)).expect("corta");
                let v = c.a_pantalla(corte.0, ANCHO, ALTO);
                assert!(
                    (v.x - p.x).abs() < 1e-6 && (v.y - p.y).abs() < 1e-6,
                    "{g}/{i}"
                );
            }
        }
    }
}

#[test]
fn la_inclinacion_y_el_zoom_tienen_tope() {
    let arriba = Camara3D::default().girada(0.0, 10.0);
    assert!(arriba.inclinacion <= TOPE_DE_INCLINACION + TOL);
    assert!((largo(arriba.arriba()) - 1.0).abs() < TOL);
    assert!((largo(Camara3D::default().girada(0.0, -10.0).arriba()) - 1.0).abs() < TOL);
    assert_eq!(Camara3D::default().con_zoom(1e6).zoom, ZOOM_MAXIMO);
    assert_eq!(Camara3D::default().con_zoom(1e-6).zoom, ZOOM_MINIMO);
}

#[test]
fn con_lente_el_rayo_y_la_pantalla_son_el_mismo_sitio() {
    for rectilinea in [false, true] {
        for lente in [0.0, 0.15, 0.45, 1.0] {
            let c = Camara3D {
                lente,
                rectilinea,
                ..camara(0.7, 0.4, 1.6)
            };
            for p in [
                Pt {
                    x: ANCHO / 2.0,
                    y: ALTO / 2.0,
                },
                Pt { x: 80.0, y: 200.0 },
                Pt {
                    x: 920.0,
                    y: 1850.0,
                },
                Pt { x: 500.0, y: 120.0 },
            ] {
                let r = c.rayo(p, ANCHO, ALTO);
                let v = c.a_pantalla(mas(r.origen, por(r.direccion, 300.0)), ANCHO, ALTO);
                assert!(
                    (v.x - p.x).abs() < 1e-6 && (v.y - p.y).abs() < 1e-6,
                    "lente {lente} {rectilinea}"
                );
            }
        }
    }
}

#[test]
fn abrir_la_lente_no_cambia_el_tamano_de_lo_que_hay_en_el_centro() {
    let recta = camara(0.0, 0.0, 2.0);
    let punto = p3(30.0, 0.0, 0.0);
    let orto = recta.a_pantalla(punto, ANCHO, ALTO);
    for lente in [0.2, 0.6, 1.0] {
        let l = Camara3D { lente, ..recta }.a_pantalla(punto, ANCHO, ALTO);
        assert!((orto.x - l.x).abs() < 4.0 && (orto.y - l.y).abs() < 4.0);
    }
}

#[test]
fn lo_de_detras_no_se_da_la_vuelta() {
    let c = Camara3D {
        lente: 1.0,
        ..camara(0.0, 0.0, 1.0)
    };
    let detras = mas(por(c.adelante(), -20000.0), por(c.derecha(), 50.0));
    let d = c.a_pantalla(detras, ANCHO, ALTO);
    assert!(d.x > ANCHO / 2.0 && d.x - ANCHO / 2.0 > ALTO / 4.0);
    // Y con la rectilinea se queda fuera por el borde, sin infinitos.
    let r = Camara3D {
        lente: 0.45,
        rectilinea: true,
        ..camara(0.0, 0.0, 1.0)
    };
    let d = r.a_pantalla(
        mas(por(r.adelante(), -20000.0), por(r.derecha(), 50.0)),
        ANCHO,
        ALTO,
    );
    assert!(d.x > ANCHO && d.x.is_finite() && d.y.is_finite());
}

#[test]
fn la_lente_rectilinea_proyecta_con_la_tangente() {
    let campo = 70f64.to_radians();
    let c = Camara3D {
        lente: campo / (160f64.to_radians()),
        rectilinea: true,
        ..camara(0.0, 0.0, 1.0)
    };
    let ojo = c.ojo(ALTO).expect("con lente hay ojo");
    let f = (ALTO / 2.0) / (campo / 2.0).tan();
    for grados in [10.0f64, 25.0, 34.0] {
        let a = grados.to_radians();
        let p = mas(
            ojo,
            mas(
                por(c.adelante(), 500.0 * a.cos()),
                por(c.arriba(), 500.0 * a.sin()),
            ),
        );
        let q = c.a_pantalla(p, ANCHO, ALTO);
        assert!(
            (q.x - ANCHO / 2.0).abs() < 1e-6 && (q.y - (ALTO / 2.0 - f * a.tan())).abs() < 1e-6
        );
    }
}

#[test]
fn el_balanceo_ladea_la_vista() {
    let recta = camara(0.0, 0.0, 1.0);
    let punto = p3(50.0, 0.0, 0.0);
    let sin = recta.a_pantalla(punto, ANCHO, ALTO);
    let ladeada = Camara3D {
        balanceo: PI / 2.0,
        ..recta
    }
    .a_pantalla(punto, ANCHO, ALTO);
    assert!((ladeada.x - ANCHO / 2.0).abs() < 1e-6);
    assert!((ladeada.y - (ALTO / 2.0 + (sin.x - ANCHO / 2.0))).abs() < 1e-6);
}

/// La vista del movil en la orbita del PC: mira en la misma direccion y al
/// mismo centro, y lo que mide `alto/zoom` llena el alto de la ventana.
#[test]
fn una_vista_guardada_se_ve_desde_el_mismo_sitio() {
    let vista = Vista3D {
        id: "v".into(),
        nombre: "Alzado".into(),
        camara: camara(0.7, 0.4, 2.0),
        recorte: None,
    };
    let o = orbita_de(&vista, [10.0, 0.0, 0.0], 800, 600, 1.0);
    // Misma direccion de mirada.
    let (c, s) = (o.altura.cos(), o.altura.sin());
    let adelante = p3(-c * o.rumbo.cos(), -c * o.rumbo.sin(), -s);
    assert!(largo(menos(adelante, vista.camara.adelante())) < 1e-9);
    assert_eq!(o.objetivo, [-10.0, 0.0, 0.0]);
    // Lo que se ve de alto en el objetivo: 600 dp a aumento 2 = 300 del mundo.
    let visto = 2.0 * o.dist * (crate::ventana3d::FOV / 2.0).tan();
    assert!((visto - 300.0).abs() < 1e-6);
    // Caso negativo: con un recorte de nada, la vista es la de siempre.
    let toque = Vista3D {
        recorte: Some(Recorte {
            x0: 0.5,
            y0: 0.5,
            x1: 0.51,
            y1: 0.51,
        }),
        ..vista.clone()
    };
    assert_eq!(orbita_de(&toque, [10.0, 0.0, 0.0], 800, 600, 1.0), o);
    // Con un recorte de medio ancho se acerca el doble.
    let mitad = Vista3D {
        recorte: Some(Recorte {
            x0: 0.25,
            y0: 0.25,
            x1: 0.75,
            y1: 0.75,
        }),
        ..vista
    };
    assert!((orbita_de(&mitad, [10.0, 0.0, 0.0], 800, 600, 1.0).dist - o.dist / 2.0).abs() < 1e-6);
}

// ---- Hojas, cuerpos y sol ----

fn hoja(perfil: Vec<Pt3>) -> Lamina3D {
    serde_json::from_value(serde_json::json!({
        "id": "l", "perfil": perfil.iter().map(|p| serde_json::json!({"x": p.x, "y": p.y, "z": p.z})).collect::<Vec<_>>(),
        "direccion": {"x": 0.0, "y": 1.0, "z": 0.0}, "fondo": 50.0, "color": "#1e1e1e", "grosor": 4.0
    }))
    .expect("hoja")
}

#[test]
fn la_lamina_es_simetrica_y_cada_tramo_es_una_tira() {
    let l = hoja(vec![p3(0.0, 0.0, 0.0), p3(10.0, 0.0, 0.0)]);
    let ys: Vec<f64> = l.esquinas().iter().map(|p| p.y).collect();
    assert_eq!(ys.iter().copied().fold(f64::MAX, f64::min), -50.0);
    assert_eq!(ys.iter().copied().fold(f64::MIN, f64::max), 50.0);
    let l = hoja(vec![
        p3(0.0, 0.0, 0.0),
        p3(10.0, 0.0, 0.0),
        p3(20.0, 5.0, 0.0),
        p3(30.0, 0.0, 0.0),
    ]);
    assert_eq!(l.tiras().len(), 3);
    // Caso negativo: un perfil de un punto no es una hoja.
    let l = hoja(vec![p3(0.0, 0.0, 0.0)]);
    assert!(l.tiras().is_empty() && l.contorno(50.0).is_empty());
}

#[test]
fn los_cuatro_cuerpos_caben_en_la_caja_unidad() {
    for forma in [
        FormaDeBola::Bola,
        FormaDeBola::Cilindro,
        FormaDeBola::Cono,
        FormaDeBola::Anillo,
    ] {
        let e = Esfera3D {
            centro: Pt3::default(),
            radio: 10.0,
            escala: p3(1.0, 1.0, 1.0),
            ejes: None,
            forma,
        };
        let m = e.malla();
        assert_eq!(m.len(), MERIDIANOS * PARALELOS);
        for q in &m {
            for p in q {
                assert!(
                    p.x.abs() <= 10.0 + TOL && p.y.abs() <= 10.0 + TOL && p.z.abs() <= 10.0 + TOL,
                    "{forma:?}"
                );
            }
        }
    }
    let cono = Esfera3D {
        centro: Pt3::default(),
        radio: 10.0,
        escala: p3(1.0, 1.0, 1.0),
        ejes: None,
        forma: FormaDeBola::Cono,
    };
    let punta = cono.punto(1.0, PI / 2.0);
    assert!(punta.x.hypot(punta.y) < TOL);
    let anillo = Esfera3D {
        forma: FormaDeBola::Anillo,
        ..cono
    };
    let dentro = anillo.punto(0.0, 0.0);
    assert!(dentro.x.hypot(dentro.y) > 3.0, "el anillo tiene agujero");
}

#[test]
fn el_sol_tira_la_sombra_en_el_suelo() {
    let sol = Sol3D {
        azimut: 0.0,
        altura: PI / 4.0,
    };
    let s = sol.sombra_de(p3(0.0, 0.0, 10.0)).expect("sombra");
    assert!(s.z.abs() < TOL && (s.x.hypot(s.y) - 10.0).abs() < 1e-6);
    let suya = sol.sombra_de(p3(3.0, 4.0, 0.0)).expect("en el suelo");
    assert!((suya.x - 3.0).abs() < TOL && (suya.y - 4.0).abs() < TOL);
    // Caso negativo: lo de debajo del suelo no tira sombra hacia arriba.
    assert!(sol.sombra_de(p3(0.0, 0.0, -5.0)).is_none());
    let alto = Sol3D {
        altura: PI / 3.0,
        ..Sol3D::default()
    }
    .sombra_de(p3(0.0, 0.0, 10.0))
    .unwrap();
    let bajo = Sol3D {
        altura: 0.2,
        ..Sol3D::default()
    }
    .sombra_de(p3(0.0, 0.0, 10.0))
    .unwrap();
    assert!(largo(bajo) > largo(alto));
    assert!(
        largo(
            Sol3D {
                altura: 0.0,
                ..Sol3D::default()
            }
            .sombra_de(p3(0.0, 0.0, 10.0))
            .unwrap()
        ) < 1e6
    );
}

// ---- Leer ----

#[test]
fn un_croquis_viejo_se_abre_con_la_punta_redonda() {
    let viejo = r##"{"trazos":[{"id":"t","puntos":[{"x":0.0,"y":0.0,"z":0.0},{"x":1.0,"y":0.0,"z":0.0}],"color":"#1e1e1e","grosor":4.0}],"laminas":[]}"##;
    let c = leer(viejo.as_bytes()).expect("se lee");
    let t = &c.trazos[0];
    assert_eq!(t.pincel, Pincel::Redondo);
    assert!(t.calibre.is_none() && t.punta.pulso && (t.opacidad - 1.0).abs() < TOL);
    assert!(c.pauta_de_las_hojas && c.luces.encendidas);
    // Las puntas retiradas abren como la redonda; una desconocida, tambien.
    assert_eq!(Pincel::Plano.vigente(), Pincel::Redondo);
    assert_eq!(Pincel::Lapiz.vigente(), Pincel::Redondo);
    assert_eq!(Pincel::Cuchilla.vigente(), Pincel::Cuchilla);
    let raro = viejo.replace(
        r#""grosor":4.0"#,
        r#""grosor":4.0,"pincel":"PLUMILLA_NUEVA","otroCampo":[1,2]"#,
    );
    assert_eq!(
        leer(raro.as_bytes())
            .expect("lo desconocido se ignora")
            .trazos[0]
            .pincel
            .vigente(),
        Pincel::Redondo
    );
}

#[test]
fn un_croquis_roto_no_se_abre() {
    assert!(leer(b"").is_err());
    assert!(
        leer(b"{\"trazos\":[{\"id\":\"t\"}]}").is_err(),
        "un trazo sin puntos ni color no es un trazo"
    );
    assert!(leer(&[0x1f, 0x8b, 0, 1, 2]).is_err(), "gzip cortado");
    assert!(leer(&[0xff, 0xfe, 0xfd]).is_err());
}

fn gz(texto: &str) -> Vec<u8> {
    use std::io::Write;
    let mut g = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    g.write_all(texto.as_bytes()).unwrap();
    g.finish().unwrap()
}

/// **Un croquis de prueba con el formato del movil**: Android no trae
/// ficheros `.croquis.gz` de prueba, asi que se hace aqui con todo lo que se
/// ve: trazos con pulso (rotulador, liston, rodillo, cuchilla, lapiz, luz),
/// una hoja, una bola, un cono, un anillo, una imagen, el sol y dos vistas.
fn croquis_de_muestra() -> String {
    let mut trazos = Vec::new();
    // Una espiral a mano alzada con pulso y reloj.
    let n = 60;
    let pts: Vec<_> = (0..n)
        .map(|i| {
            let a = i as f64 * 0.18;
            serde_json::json!({"x": 60.0 * a.cos(), "y": 60.0 * a.sin(), "z": 20.0 + i as f64 * 2.2})
        })
        .collect();
    let pres: Vec<f64> = (0..n)
        .map(|i| 0.4 + 0.6 * ((i as f64) * 0.2).sin().abs())
        .collect();
    let tiempos: Vec<f64> = (0..n).map(|i| i as f64 * 0.016).collect();
    trazos.push(serde_json::json!({"id":"t1","puntos":pts,"color":"#1e66f5","grosor":10.0,"calibre":10.0,"presiones":pres,"tiempos":tiempos}));
    let linea = |id: &str,
                 a: [f64; 3],
                 b: [f64; 3],
                 pincel: &str,
                 color: &str,
                 extra: serde_json::Value| {
        let mut v = serde_json::json!({"id": id, "puntos": [{"x":a[0],"y":a[1],"z":a[2]},{"x":(a[0]+b[0])/2.0,"y":(a[1]+b[1])/2.0,"z":(a[2]+b[2])/2.0},{"x":b[0],"y":b[1],"z":b[2]}], "color": color, "grosor": 8.0, "calibre": 8.0, "pincel": pincel});
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    trazos.push(linea(
        "t2",
        [-120.0, -80.0, 0.0],
        [-120.0, -80.0, 120.0],
        "CUADRADO",
        "#d20f39",
        serde_json::json!({}),
    ));
    trazos.push(linea(
        "t3",
        [-100.0, 100.0, 0.0],
        [100.0, 100.0, 0.0],
        "RESALTADOR",
        "#f9e2af",
        serde_json::json!({"normal":{"x":0.0,"y":0.0,"z":1.0},"grosor":24.0,"calibre":24.0}),
    ));
    trazos.push(linea(
        "t4",
        [100.0, -100.0, 0.0],
        [100.0, 100.0, 0.0],
        "CUCHILLA",
        "#40a02b",
        serde_json::json!({"normal":{"x":0.0,"y":0.0,"z":1.0},"calibre":30.0}),
    ));
    trazos.push(linea(
        "t5",
        [-150.0, 0.0, 150.0],
        [150.0, 0.0, 150.0],
        "REDONDO",
        "#4c4f69",
        serde_json::json!({"punta":{"plana":true},"calibre":2.0}),
    ));
    trazos.push(linea(
        "t6",
        [-150.0, -150.0, 60.0],
        [150.0, -150.0, 60.0],
        "LUZ",
        "#ff00ff",
        serde_json::json!({}),
    ));
    trazos.push(linea(
        "t7",
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 500.0],
        "REDONDO",
        "#000000",
        serde_json::json!({"oculto":true}),
    ));
    serde_json::json!({
        "trazos": trazos,
        "laminas": [
            {"id":"h1","perfil":[{"x":-150.0,"y":150.0,"z":0.0},{"x":150.0,"y":150.0,"z":0.0}],"direccion":{"x":0.0,"y":0.0,"z":1.0},"fondo":90.0,"color":"#1e1e1e","grosor":2.0},
            {"id":"b1","perfil":[],"direccion":{"x":0.0,"y":0.0,"z":1.0},"fondo":0.0,"color":"#1e1e1e","grosor":2.0,"esfera":{"centro":{"x":180.0,"y":60.0,"z":60.0},"radio":45.0}},
            {"id":"b2","perfil":[],"direccion":{"x":0.0,"y":0.0,"z":1.0},"fondo":0.0,"color":"#1e1e1e","grosor":2.0,"esfera":{"centro":{"x":-200.0,"y":60.0,"z":50.0},"radio":40.0,"forma":"CONO"}},
            {"id":"b3","perfil":[],"direccion":{"x":0.0,"y":0.0,"z":1.0},"fondo":0.0,"color":"#1e1e1e","grosor":2.0,"esfera":{"centro":{"x":0.0,"y":-60.0,"z":200.0},"radio":40.0,"forma":"ANILLO"}}
        ],
        "imagenes": [{"id":"i1","ruta":"pixpin:files/croquis3d/img/foto.png","esquinas":[
            {"x":-60.0,"y":180.0,"z":120.0},{"x":60.0,"y":180.0,"z":120.0},{"x":60.0,"y":180.0,"z":30.0},{"x":-60.0,"y":180.0,"z":30.0}]}],
        "sol": {"azimut": 0.6, "altura": 0.8},
        "vistas": [
            {"id":"v1","nombre":"Planta","camara":{"giro":0.0,"inclinacion":1.55,"zoom":1.5}},
            {"id":"v2","nombre":"Alzado","camara":{"giro":0.0,"inclinacion":0.0,"zoom":1.2,"centro":{"x":0.0,"y":0.0,"z":100.0}}}
        ],
        "favoritos": ["#1e66f5"],
        "colorDelFondo": null
    })
    .to_string()
}

/// La foto de la imagen: un degradado con una cruz, para verla en el PNG.
fn foto_de_muestra(ruta: &str) -> Option<Foto> {
    if !ruta.ends_with("foto.png") {
        return None;
    }
    let (w, h) = (64u32, 48u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let cruz = x == w / 2 || y == h / 2;
            rgba.extend(if cruz {
                [255, 255, 255, 255]
            } else {
                [(x * 4) as u8, 120, (y * 5) as u8, 255]
            });
        }
    }
    Some(Foto {
        ancho: w,
        alto: h,
        rgba,
    })
}

#[test]
fn el_croquis_de_muestra_se_hace_triangulos() {
    let c = leer(&gz(&croquis_de_muestra())).expect("se lee del gzip");
    assert_eq!(c.trazos.len(), 7);
    assert_eq!(c.vistas.len(), 2);
    let m = a_modelo(&c, &foto_de_muestra);
    assert!(!m.vacio());
    let tipos: Vec<&str> = m.elementos.iter().map(|e| e.tipo.as_str()).collect();
    for t in [
        "Rotulador",
        "Listón",
        "Rodillo",
        "Cuchilla",
        "Lápiz",
        "Luz",
        "Hoja",
        "Bola",
        "Cono",
        "Anillo",
        "Imagen",
        "Suelo",
    ] {
        assert!(tipos.contains(&t), "falta {t} en {tipos:?}");
    }
    // Lo oculto no se pinta: seis trazos, no siete.
    let trazos = ["Rotulador", "Listón", "Rodillo", "Cuchilla", "Lápiz", "Luz"]
        .iter()
        .map(|t| tipos.iter().filter(|x| *x == t).count())
        .sum::<usize>();
    assert_eq!(trazos, 6);
    // Lo que alumbra y la foto van sin luz; los velos, de contraste.
    let marca = |e: &str, bit: u32| {
        let k = m.elementos.iter().position(|x| x.tipo == e).unwrap() as u32;
        m.opacos
            .iter()
            .chain(&m.lineas)
            .chain(if bit == SIN_LUZ && e == "Rotulador" {
                &[][..]
            } else {
                &m.transparentes[..]
            })
            .map(|i| &m.vertices[*i as usize])
            .filter(|v| v.elemento == k)
            .any(|v| v.normal & bit != 0)
    };
    assert!(marca("Luz", SIN_LUZ) && marca("Imagen", SIN_LUZ) && marca("Bola", CONTRASTE));
    assert!(!marca("Rotulador", SIN_LUZ), "un trazo normal se sombrea");
    assert!(
        marca("Rotulador", super::malla::PLUMA) && !marca("Bola", super::malla::PLUMA),
        "con el tono de pluma, solo los trazos"
    );
    // El sol tira sombras (translucidas, en el suelo).
    assert!(!m.transparentes.is_empty());
    // Todos los indices apuntan a vertices que existen.
    let n = m.vertices.len() as u32;
    assert!(
        m.opacos
            .iter()
            .chain(&m.transparentes)
            .chain(&m.lineas)
            .all(|i| *i < n)
    );
    assert!(m.opacos.len() % 3 == 0 && m.transparentes.len() % 3 == 0 && m.lineas.len() % 2 == 0);
    // Se encuadra lo dibujado, no el suelo.
    let caja = caja_de_lo_dibujado(&c, m.origen).unwrap();
    assert!(caja[3] - caja[0] < m.caja[3] - m.caja[0] + 1.0);
}

#[test]
fn el_pulso_hace_el_tubo_mas_gordo_donde_se_aprieta() {
    // Un trazo recto con la presion subiendo: el final es mas gordo que el principio.
    let n = 30;
    let pts: Vec<_> = (0..n)
        .map(|i| serde_json::json!({"x": i as f64 * 5.0, "y": 0.0, "z": 0.0}))
        .collect();
    let pres: Vec<f64> = (0..n).map(|i| i as f64 / (n - 1) as f64).collect();
    let texto = serde_json::json!({"trazos":[{"id":"t","puntos":pts,"color":"#000000","grosor":10.0,"calibre":10.0,"presiones":pres}]}).to_string();
    let c = leer(texto.as_bytes()).unwrap();
    let m = a_modelo(&c, &|_| None);
    let e = m
        .elementos
        .iter()
        .position(|x| x.tipo == "Rotulador")
        .unwrap() as u32;
    let gordo_en = |x0: f32| {
        m.vertices
            .iter()
            .filter(|v| v.elemento == e && (v.pos[0] + m.origen[0] as f32 - x0).abs() < 1.0)
            .map(|v| v.pos[1].abs())
            .fold(0f32, f32::max)
    };
    assert!(
        gordo_en(120.0) > gordo_en(20.0) * 1.2,
        "{} {}",
        gordo_en(120.0),
        gordo_en(20.0)
    );
    // Caso negativo: sin pulso (`pulso: false`) sale parejo.
    let parejo = texto.replace(r#""presiones""#, r#""punta":{"pulso":false},"presiones""#);
    let m2 = a_modelo(&leer(parejo.as_bytes()).unwrap(), &|_| None);
    let r: Vec<f32> = m2
        .vertices
        .iter()
        .filter(|v| v.elemento == e)
        .map(|v| v.pos[1].abs())
        .collect();
    assert!((r.iter().copied().fold(0f32, f32::max) - 5.0).abs() < 0.01);
}

#[test]
fn un_croquis_vacio_no_tiene_nada_que_ver() {
    let c = leer(b"{}").unwrap();
    assert!(c.vacio());
    assert!(a_modelo(&c, &|_| None).vacio());
    // Una imagen cuya foto no esta en este equipo: su marco, no un agujero.
    let t = r#"{"imagenes":[{"id":"i","ruta":"pixpin:files/no/esta.jpg","esquinas":[{"x":0,"y":0,"z":0},{"x":1,"y":0,"z":0},{"x":1,"y":0,"z":1},{"x":0,"y":0,"z":1}]}]}"#;
    let m = a_modelo(&leer(t.as_bytes()).unwrap(), &|_| None);
    assert!(m.opacos.is_empty() && !m.lineas.is_empty());
}

#[test]
fn el_tema_con_el_que_abre() {
    let mut c = leer(croquis_de_muestra().as_bytes()).unwrap();
    assert!(abre_claro(&c), "tinta oscura: fondo claro");
    c.color_del_fondo = Some("#101418".into());
    assert!(!abre_claro(&c), "su fondo manda");
    let noche =
        r##"{"trazos":[{"id":"t","puntos":[{"x":0,"y":0,"z":0}],"color":"#f1f3f5","grosor":4}]}"##;
    assert!(
        !abre_claro(&leer(noche.as_bytes()).unwrap()),
        "tinta de noche: fondo oscuro"
    );
}

/// **Abrir no cambia un byte**: se lee el fichero, se hace la malla, y el
/// fichero sigue igual (ni fecha ni contenido).
#[test]
fn abrir_no_cambia_el_fichero() {
    let dir = std::env::temp_dir().join(format!("pixpin-croquis-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("c1.croquis.gz");
    let bytes = gz(&croquis_de_muestra());
    std::fs::write(&ruta, &bytes).unwrap();
    let antes = std::fs::metadata(&ruta).unwrap().modified().unwrap();
    let c = leer(&std::fs::read(&ruta).unwrap()).unwrap();
    let _ = a_modelo(&c, &foto_de_muestra);
    assert_eq!(std::fs::read(&ruta).unwrap(), bytes);
    assert_eq!(std::fs::metadata(&ruta).unwrap().modified().unwrap(), antes);
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        1,
        "no aparece nada al lado"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- El pulso (PulsoDelMundoTest.kt) ----

const CALIBRE: f64 = 4.0;

fn recorrido_de(huecos: &[f64]) -> Vec<f64> {
    let mut r = vec![0.0];
    for h in huecos {
        r.push(r.last().unwrap() + h);
    }
    r
}
fn parejos(n: usize) -> Vec<f64> {
    recorrido_de(&vec![1.0; n])
}
fn sano(a: &[f64]) {
    for x in a {
        assert!(!x.is_nan() && *x > 0.0 && *x <= CALIBRE + 1e-9, "{x}");
    }
}
fn anchos(p: Option<&[f64]>, r: &[f64], t: Option<&[f64]>) -> Vec<f64> {
    anchos_del_mundo(CALIBRE, p, r, t, true, true)
}

#[test]
fn parejo_por_dentro_sale_al_calibre_y_las_puntas_se_afilan() {
    let a = anchos(None, &parejos(19), None);
    for x in &a[5..a.len() - 5] {
        assert!((x - CALIBRE).abs() < 1e-12);
    }
    let n = a.len();
    for i in 0..3 {
        assert!(a[i] < a[i + 1] && a[n - 1 - i] < a[n - 2 - i]);
    }
    for i in 0..4 {
        assert!((a[i] - a[n - 1 - i]).abs() < 1e-12);
    }
    sano(&a);
    assert!(
        (anchos(Some(&[0.4; 24]), &parejos(23), None)[10] - CALIBRE).abs() < 1e-12,
        "presion constante"
    );
}

#[test]
fn lo_rapido_sale_fino_y_apretar_engorda() {
    let huecos: Vec<f64> = (0..23).map(|i| if i < 14 { 1.0 } else { 3.0 }).collect();
    let a = anchos(None, &recorrido_de(&huecos), None);
    assert!(a[18] < a[7] && (a[7] - CALIBRE).abs() < 1e-12);
    let mut t = vec![0.0; 24];
    for i in 1..24 {
        t[i] = t[i - 1] + if i <= 14 { 0.1 } else { 0.02 };
    }
    let con = anchos(None, &parejos(23), Some(&t));
    assert!(con[18] < con[7]);
    let sin = anchos(None, &parejos(23), None);
    assert!((sin[7] - sin[18]).abs() < 1e-12);
    let p: Vec<f64> = (0..24).map(|i| if i < 12 { 0.3 } else { 0.9 }).collect();
    let a = anchos(Some(&p), &parejos(23), None);
    assert!(a[6] < a[18]);
}

#[test]
fn mil_veces_mas_grande_deja_los_mismos_anchos() {
    let huecos = [1.0, 2.5, 0.5, 3.0, 1.0, 4.0, 0.75, 2.0, 1.5, 1.0, 3.5, 0.5];
    let pres = [
        0.2, 0.9, 0.5, 1.0, 0.3, 0.7, 0.6, 0.8, 0.4, 1.0, 0.25, 0.65, 0.55,
    ];
    let chico = recorrido_de(&huecos);
    let grande: Vec<f64> = chico.iter().map(|x| x * 1000.0).collect();
    let (a, b) = (
        anchos(Some(&pres), &chico, None),
        anchos(Some(&pres), &grande, None),
    );
    for i in 0..a.len() {
        assert!((a[i] - b[i]).abs() < 1e-9);
    }
}

#[test]
fn los_trazos_cortos_y_las_entradas_raras_no_rompen() {
    let solo = anchos(Some(&[0.5]), &[0.0], None);
    assert_eq!(solo, vec![CALIBRE * 0.5]);
    assert_eq!(anchos(None, &[0.0], None), vec![CALIBRE]);
    let raya = anchos(Some(&[0.4, 0.8]), &[0.0, 7.0], Some(&[0.0, 0.1]));
    assert!((raya[0] - CALIBRE * 0.6).abs() < 1e-12 && (raya[0] - raya[1]).abs() < 1e-12);
    assert!(anchos(None, &[], None).is_empty());
    sano(&anchos(
        None,
        &parejos(9),
        Some(&[0.0, 0.1, 0.1, 0.05, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]),
    ));
    let quieto = recorrido_de(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 5.0, 0.0, 0.0]);
    let a = anchos(None, &quieto, None);
    sano(&a);
    assert!(a[6] < a[4]);
    let t: Vec<f64> = (0..40).map(|i| (i / 4) as f64 * 0.016).collect();
    let a = anchos(None, &parejos(39), Some(&t));
    sano(&a);
    for x in &a[6..=33] {
        assert!((x - CALIBRE).abs() <= CALIBRE * 0.02);
    }
    let r = parejos(11);
    sano(&anchos(
        Some(
            &(0..12)
                .map(|i| if i % 2 == 0 { 0.0 } else { 1.0 })
                .collect::<Vec<_>>(),
        ),
        &r,
        None,
    ));
    sano(&anchos(
        Some(&[
            0.5,
            f64::NAN,
            0.9,
            0.2,
            0.7,
            0.4,
            0.6,
            0.3,
            0.8,
            0.5,
            0.6,
            0.7,
        ]),
        &r,
        None,
    ));
    sano(&anchos(Some(&[0.3, 0.9]), &r, None));
}

// ---- Verlo: el modelo pintado en la CPU, con las cuentas del sombreador ----

fn pintar(m: &Modelo3d, o: &Orbita, w: u32, h: u32, claro: bool) -> Vec<[f32; 3]> {
    let fondo = if claro {
        [0.955, 0.958, 0.965]
    } else {
        [0.13, 0.15, 0.19]
    };
    let mut px = vec![fondo; (w * h) as usize];
    let mut z = vec![f32::INFINITY; (w * h) as usize];
    let radio = (m.caja[3] - m.caja[0])
        .hypot(m.caja[4] - m.caja[1])
        .hypot(m.caja[5] - m.caja[2]) as f64
        / 2.0;
    let mvp = o.matriz(w, h, radio);
    let ojo = o.ojo();
    let (r, u, f) = {
        let a = [
            -(o.altura.cos()) * o.rumbo.cos(),
            -(o.altura.cos()) * o.rumbo.sin(),
            -(o.altura.sin()),
        ];
        let rr = [a[1], -a[0], 0.0];
        let l = (rr[0] * rr[0] + rr[1] * rr[1]).sqrt().max(1e-12);
        let rr = [rr[0] / l, rr[1] / l, 0.0];
        let uu = [
            rr[1] * a[2] - rr[2] * a[1],
            rr[2] * a[0] - rr[0] * a[2],
            rr[0] * a[1] - rr[1] * a[0],
        ];
        (rr, uu, a)
    };
    let luz = {
        let v = [
            -0.5 * f[0] + 0.75 * u[0] + 0.35 * r[0],
            -0.5 * f[1] + 0.75 * u[1] + 0.35 * r[1],
            -0.5 * f[2] + 0.75 * u[2] + 0.35 * r[2],
        ];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / l, v[1] / l, v[2] / l]
    };
    let proy = |p: [f32; 3]| -> Option<[f64; 3]> {
        let v = [p[0], p[1], p[2], 1.0];
        let c: Vec<f32> = (0..4)
            .map(|j| (0..4).map(|i| v[i] * mvp[i][j]).sum())
            .collect();
        if c[3] <= 1e-6 {
            return None;
        }
        Some([
            ((c[0] / c[3]) as f64 + 1.0) / 2.0 * w as f64,
            (1.0 - (c[1] / c[3]) as f64) / 2.0 * h as f64,
            (c[2] / c[3]) as f64,
        ])
    };
    let color = |vi: usize| -> [f32; 4] {
        let v = &m.vertices[vi];
        let c = v.color;
        let mut col = [
            (c & 255) as f32 / 255.0,
            (c >> 8 & 255) as f32 / 255.0,
            (c >> 16 & 255) as f32 / 255.0,
            (c >> 24 & 255) as f32 / 255.0,
        ];
        let marca = (v.normal >> 29) & 7;
        if marca & 2 != 0 {
            let k = if claro { 0.0 } else { 1.0 };
            col[0] = k;
            col[1] = k;
            col[2] = k;
        }
        if marca & 4 != 0 {
            let s = |b: u32| ((b as u8) as i8) as f32 / 127.0;
            let mut n = [s(v.normal), s(v.normal >> 8), s(v.normal >> 16)];
            let p = [v.pos[0] as f64, v.pos[1] as f64, v.pos[2] as f64];
            let vv = [ojo[0] - p[0], ojo[1] - p[1], ojo[2] - p[2]];
            if (n[0] as f64 * vv[0] + n[1] as f64 * vv[1] + n[2] as f64 * vv[2]) < 0.0 {
                n = [-n[0], -n[1], -n[2]];
            }
            let t = (n[0] * luz[0] as f32 + n[1] * luz[1] as f32 + n[2] * luz[2] as f32) * 0.42;
            for c in col.iter_mut().take(3) {
                *c = if t > 0.0 {
                    *c + (1.0 - *c) * t
                } else {
                    *c * (1.0 + t)
                };
            }
        } else if marca == 0 {
            let s = |b: u32| ((b as u8) as i8) as f32 / 127.0;
            let mut n = [s(v.normal), s(v.normal >> 8), s(v.normal >> 16)];
            let p = [v.pos[0] as f64, v.pos[1] as f64, v.pos[2] as f64];
            let vv = [ojo[0] - p[0], ojo[1] - p[1], ojo[2] - p[2]];
            if (n[0] as f64 * vv[0] + n[1] as f64 * vv[1] + n[2] as f64 * vv[2]) < 0.0 {
                n = [-n[0], -n[1], -n[2]];
            }
            let dif = (n[0] * luz[0] as f32 + n[1] * luz[1] as f32 + n[2] * luz[2] as f32)
                .clamp(0.0, 1.0);
            let k = 0.30 + 0.22 * (0.5 + 0.5 * n[2]) + 0.55 * dif;
            for c in col.iter_mut().take(3) {
                *c *= k;
            }
        }
        col
    };
    let triangulos = |lista: &[u32], escribe: bool, px: &mut Vec<[f32; 3]>, z: &mut Vec<f32>| {
        for t in lista.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (
                proy(m.vertices[t[0] as usize].pos),
                proy(m.vertices[t[1] as usize].pos),
                proy(m.vertices[t[2] as usize].pos),
            ) else {
                continue;
            };
            let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            if area.abs() < 1e-9 {
                continue;
            }
            let cols = [
                color(t[0] as usize),
                color(t[1] as usize),
                color(t[2] as usize),
            ];
            let x0 = a[0].min(b[0]).min(c[0]).floor().max(0.0) as i64;
            let x1 = a[0].max(b[0]).max(c[0]).ceil().min(w as f64 - 1.0) as i64;
            let y0 = a[1].min(b[1]).min(c[1]).floor().max(0.0) as i64;
            let y1 = a[1].max(b[1]).max(c[1]).ceil().min(h as f64 - 1.0) as i64;
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (px_, py) = (x as f64 + 0.5, y as f64 + 0.5);
                    let wa = ((b[0] - px_) * (c[1] - py) - (b[1] - py) * (c[0] - px_)) / area;
                    let wb = ((c[0] - px_) * (a[1] - py) - (c[1] - py) * (a[0] - px_)) / area;
                    let wc = 1.0 - wa - wb;
                    if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                        continue;
                    }
                    let zz = (wa * a[2] + wb * b[2] + wc * c[2]) as f32;
                    let k = (y as u32 * w + x as u32) as usize;
                    if zz > z[k] {
                        continue;
                    }
                    let mut cc = [0f32; 4];
                    for i in 0..4 {
                        cc[i] = wa as f32 * cols[0][i]
                            + wb as f32 * cols[1][i]
                            + wc as f32 * cols[2][i];
                    }
                    let al = if escribe { 1.0 } else { cc[3] };
                    for i in 0..3 {
                        px[k][i] = px[k][i] * (1.0 - al) + cc[i] * al;
                    }
                    if escribe {
                        z[k] = zz;
                    }
                }
            }
        }
    };
    triangulos(&m.opacos, true, &mut px, &mut z);
    // Las rayas, con su alfa y detras de lo opaco.
    for s in m.lineas.chunks_exact(2) {
        let (Some(a), Some(b)) = (
            proy(m.vertices[s[0] as usize].pos),
            proy(m.vertices[s[1] as usize].pos),
        ) else {
            continue;
        };
        let col = color(s[0] as usize);
        let n = ((b[0] - a[0]).abs().max((b[1] - a[1]).abs()).ceil() as usize).clamp(1, 4000);
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let (x, y, zz) = (
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                (a[2] + (b[2] - a[2]) * t) as f32,
            );
            if x < 0.0 || y < 0.0 || x >= w as f64 || y >= h as f64 {
                continue;
            }
            let k = (y as u32 * w + x as u32) as usize;
            if zz > z[k] + 1e-5 {
                continue;
            }
            for c in 0..3 {
                px[k][c] = px[k][c] * (1.0 - col[3]) + col[c] * col[3];
            }
        }
    }
    let transp = m.transparentes.clone();
    triangulos(&transp, false, &mut px, &mut z);
    px
}

fn guardar_png(px: &[[f32; 3]], w: u32, h: u32, ruta: &std::path::Path) {
    let f = std::fs::File::create(ruta).expect("png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    let mut wr = e.write_header().unwrap();
    let datos: Vec<u8> = px
        .iter()
        .flat_map(|c| c.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8))
        .collect();
    wr.write_image_data(&datos).unwrap();
}

/// **El croquis de muestra en PNG**, como lo encuadra la ventana al abrir y
/// desde sus dos vistas guardadas. `PIXPIN_CROQUIS=<.croquis.gz>` pinta uno
/// real; `PIXPIN_CROQUIS_PNG=<carpeta>` dice donde.
#[test]
#[ignore = "pinta PNG para mirarlos"]
fn pintar_croquis_a_png() {
    let bytes = match std::env::var("PIXPIN_CROQUIS") {
        Ok(r) => std::fs::read(r).expect("croquis"),
        Err(_) => gz(&croquis_de_muestra()),
    };
    let c = leer(&bytes).expect("se lee");
    let m = a_modelo(&c, &foto_de_muestra);
    let carpeta = std::path::PathBuf::from(
        std::env::var("PIXPIN_CROQUIS_PNG")
            .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned()),
    );
    let (w, h) = (900u32, 700u32);
    let caja = caja_de_lo_dibujado(&c, m.origen).unwrap_or(m.caja);
    let por_defecto = Camara3D::default();
    let mut o = Orbita::encuadrar(caja, w, h);
    o.rumbo = -por_defecto.giro - PI / 2.0;
    o.altura = por_defecto.inclinacion;
    let o = {
        // `distancia_para` es privada: se encuadra igual girando desde la orbita ya encuadrada.
        let mut x = Orbita::encuadrar(caja, w, h);
        x = x.girar(o.rumbo - x.rumbo, o.altura - x.altura, x.objetivo);
        x
    };
    let claro = abre_claro(&c);
    guardar_png(
        &pintar(&m, &o, w, h, claro),
        w,
        h,
        &carpeta.join("croquis-abrir.png"),
    );
    guardar_png(
        &pintar(&m, &o, w, h, !claro),
        w,
        h,
        &carpeta.join("croquis-otro-tema.png"),
    );
    // Y desde arriba, de frente y de lado, aunque el croquis no traiga vistas.
    for (nombre, giro, inclinacion) in [
        ("planta", 0.0, 1.5),
        ("frente", 0.0, 0.05),
        ("lado", PI / 2.0, 0.3),
    ] {
        let mut x = Orbita::encuadrar(caja, w, h);
        x = x.girar(
            -giro - PI / 2.0 - x.rumbo,
            inclinacion - x.altura,
            x.objetivo,
        );
        guardar_png(
            &pintar(&m, &x, w, h, claro),
            w,
            h,
            &carpeta.join(format!("croquis-{nombre}.png")),
        );
    }
    for (i, v) in c.vistas.iter().enumerate() {
        let ov = orbita_de(v, m.origen, w, h, 1.0);
        guardar_png(
            &pintar(&m, &ov, w, h, claro),
            w,
            h,
            &carpeta.join(format!("croquis-vista-{}.png", i + 1)),
        );
    }
}

#[test]
fn la_banda_no_se_retuerce_donde_el_trazo_va_por_la_normal() {
    use super::malla::lados_de_la_banda;
    let x = p3(1.0, 0.0, 0.0);
    let z = p3(0.0, 0.0, 1.0);
    // Tangente en x; en la muestra 2 el trazo va justo por la normal (un salto).
    let tg = [x, x, z, x, x];
    let n = [z, z, z, z, z];
    let l = lados_de_la_banda(&tg, Some(&n), false);
    for s in &l {
        assert!(
            (largo(*s) - 1.0).abs() < 1e-9,
            "el lado nunca se queda en nada"
        );
    }
    assert!(
        largo(menos(l[2], l[1])) < 1e-9,
        "en el salto sigue el de antes"
    );
    // Caso negativo: una normal dada la vuelta no voltea la banda.
    let n2 = [z, z, por(z, -1.0), por(z, -1.0), z];
    let l2 = lados_de_la_banda(&[x; 5], Some(&n2), false);
    for i in 1..5 {
        assert!(escalar(l2[i], l2[i - 1]) > 0.0);
    }
}
