//! El plano empaquetado para la web (`PlanoWebTest` del movil): lo que entra
//! tiene que volver a salir punto por punto al deshacer el paquete como lo
//! deshace el visor de la pagina.

use super::*;
use crate::plano::pruebas::{Pagina, pagina_con};
use crate::plano::{Capa, Texto};
use std::io::Read;

fn brocha(ops: Vec<u8>, pts: &[(i32, i32)], relleno: bool) -> Brocha {
    Brocha {
        capa: -1,
        color: 0,
        alfa: 1.0,
        grosor: 1.0,
        relleno,
        raya: Vec::new(),
        ops,
        xs: pts.iter().map(|p| p.0).collect(),
        ys: pts.iter().map(|p| p.1).collect(),
        par_impar: false,
    }
}

fn plano(brochas: Vec<Brocha>) -> Plano {
    Plano {
        ancho: 100.0,
        alto: 200.0,
        capas: Vec::new(),
        brochas,
        textos: Vec::new(),
        fotos: Vec::new(),
        sin_entender: 0,
        cortado: false,
    }
}

/// Deshace el paquete como `crearPlano`: base64, DEFLATE crudo y varints
/// en zigzag. Devuelve, por brocha, sus ordenes y sus puntos.
#[allow(clippy::type_complexity)]
fn deshacer(json: &str) -> (serde_json::Value, Vec<(Vec<u8>, Vec<(i32, i32)>)>) {
    let v: serde_json::Value = serde_json::from_str(json).expect("es JSON");
    let datos = de_base64(v["datos"].as_str().unwrap());
    let mut d = Vec::new();
    flate2::read::DeflateDecoder::new(&datos[..])
        .read_to_end(&mut d)
        .unwrap();
    fn varint(d: &[u8], p: &mut usize) -> i32 {
        let (mut r, mut s) = (0u32, 0);
        loop {
            let c = d[*p];
            *p += 1;
            r |= ((c & 0x7f) as u32) << s;
            s += 7;
            if c & 0x80 == 0 {
                break;
            }
        }
        ((r >> 1) as i32) ^ -((r & 1) as i32)
    }
    let mut ops_de = Vec::new();
    let (mut x, mut y, mut p) = (0, 0, 0usize);
    for b in v["brochas"].as_array().unwrap() {
        let n = b["n"].as_u64().unwrap() as usize;
        let mut ops = Vec::new();
        let mut pts = Vec::new();
        for _ in 0..n {
            let op = d[p];
            p += 1;
            ops.push(op);
            let cuantos = match op {
                3 => 3,
                4 => 0,
                _ => 1,
            };
            for _ in 0..cuantos {
                x += varint(&d, &mut p);
                y += varint(&d, &mut p);
                pts.push((x, y));
            }
        }
        ops_de.push((ops, pts));
    }
    assert_eq!(p, d.len(), "no sobra nada en el flujo");
    (v, ops_de)
}

#[test]
fn lo_que_entra_vuelve_a_salir_punto_por_punto() {
    let p = plano(vec![brocha(
        vec![MOVER, LINEA, LINEA],
        &[(10, 20), (300, -40), (5, 5)],
        false,
    )]);
    let (_, b) = deshacer(&a_json(&p, 100.0));
    assert_eq!(b[0].0, vec![MOVER, LINEA, LINEA]);
    assert_eq!(b[0].1, vec![(10, 20), (300, -40), (5, 5)]);
}

#[test]
fn una_curva_viaja_con_sus_tres_puntos() {
    let p = plano(vec![brocha(
        vec![MOVER, CURVA],
        &[(0, 0), (1, 2), (3, 4), (5, 6)],
        false,
    )]);
    let (_, b) = deshacer(&a_json(&p, 100.0));
    assert_eq!(b[0].1.len(), 4);
    assert_eq!(b[0].1[3], (5, 6));
}

/// AutoCAD escribe cada tramo aparte aunque se toquen: cosidos, la segunda
/// orden de mover desaparece.
#[test]
fn los_tramos_que_se_tocan_se_cosen_en_un_solo_camino() {
    let p = plano(vec![brocha(
        vec![MOVER, LINEA, MOVER, LINEA],
        &[(0, 0), (10, 0), (10, 0), (10, 10)],
        false,
    )]);
    let (_, b) = deshacer(&a_json(&p, 100.0));
    assert_eq!(b[0].0, vec![MOVER, LINEA, LINEA]);
    assert_eq!(b[0].1, vec![(0, 0), (10, 0), (10, 10)]);
}

#[test]
fn dos_tramos_que_no_se_tocan_siguen_siendo_dos_caminos() {
    let p = plano(vec![brocha(
        vec![MOVER, LINEA, MOVER, LINEA],
        &[(0, 0), (10, 0), (50, 50), (60, 60)],
        false,
    )]);
    let (_, b) = deshacer(&a_json(&p, 100.0));
    assert_eq!(b[0].0.iter().filter(|o| **o == MOVER).count(), 2);
}

#[test]
fn un_camino_cerrado_no_se_empalma_con_otro() {
    let p = plano(vec![brocha(
        vec![MOVER, LINEA, CERRAR, MOVER, LINEA],
        &[(0, 0), (10, 0), (0, 0), (5, 5)],
        false,
    )]);
    let (_, b) = deshacer(&a_json(&p, 100.0));
    assert_eq!(b[0].0.iter().filter(|o| **o == MOVER).count(), 2);
}

/// Los rellenos no se reordenan: su orden es su dibujo.
#[test]
fn las_manchas_conservan_el_orden_en_que_se_pintaron() {
    let p = plano(vec![brocha(
        vec![MOVER, LINEA, MOVER, LINEA],
        &[(0, 90_000), (1, 90_000), (0, 0), (1, 0)],
        true,
    )]);
    let (_, b) = deshacer(&a_json(&p, 100.0));
    assert_eq!(b[0].1[0], (0, 90_000), "el primero pintado sigue primero");
}

#[test]
fn la_cabecera_lleva_la_escala_el_papel_y_las_capas() {
    let mut p = plano(vec![brocha(vec![MOVER, LINEA], &[(0, 0), (256, 0)], false)]);
    p.capas = vec![
        Capa {
            nombre: "Muros".into(),
            encendida: true,
        },
        Capa {
            nombre: "Cotas".into(),
            encendida: false,
        },
    ];
    let (v, _) = deshacer(&a_json(&p, 1400.0));
    assert_eq!(v["a"], 1400);
    assert_eq!(v["b"], 2800);
    // Un paso son 1400/100/256 unidades.
    assert!((v["e"].as_f64().unwrap() - 14.0 / 256.0).abs() < 1e-9);
    assert_eq!(v["capas"][0]["n"], "Muros");
    assert!(v["capas"][0].get("v").is_none());
    assert_eq!(v["capas"][1]["v"], 0);
    assert_eq!(v["brochas"][0]["g"], 14, "el grosor, en unidades del papel");
}

#[test]
fn un_nombre_de_capa_no_puede_cerrar_la_etiqueta() {
    let mut p = plano(vec![brocha(vec![MOVER, LINEA], &[(0, 0), (1, 0)], false)]);
    p.capas = vec![Capa {
        nombre: "</script><b>\"x\"".into(),
        encendida: true,
    }];
    let j = a_json(&p, 100.0);
    assert!(!j.contains("</script"));
    let (v, _) = deshacer(&j);
    assert_eq!(v["capas"][0]["n"], "</script><b>\"x\"");
}

#[test]
fn un_texto_viaja_con_su_matriz_su_ancho_y_su_tipo_de_letra() {
    let mut p = plano(vec![]);
    p.textos.push(Texto {
        capa: 2,
        color: 0x112233,
        alfa: 1.0,
        texto: "Cota 3,20".into(),
        a: 10.0,
        b: 0.0,
        c: 0.0,
        d: 10.0,
        x: 5.0,
        y: 7.0,
        ancho: 4.12345,
        familia: "sans-serif-condensed",
        negrita: true,
        cursiva: false,
    });
    let (v, _) = deshacer(&a_json(&p, 200.0));
    let t = &v["textos"][0];
    assert_eq!(t["s"], "Cota 3,20");
    assert_eq!(t["t"], "#112233");
    assert_eq!(t["m"][0], 20);
    assert_eq!(t["m"][4], 10);
    assert_eq!(t["w"], 4.1235);
    assert_eq!(t["f"], "sans-serif-condensed");
    assert_eq!(t["n"], 1);
    assert!(t.get("i").is_none());
}

#[test]
fn del_pdf_a_la_pagina_web_y_de_vuelta_sin_perder_un_punto() {
    let mut c = String::from("1 0 0 RG ");
    for i in 0..20 {
        c.push_str(&format!("{} 10 m {} 10 l S ", i * 2, i * 2 + 2));
    }
    let bytes = pagina_con(c.as_bytes(), Pagina::default());
    let json = de_bytes(&bytes, 0, 100.0).expect("un plano de rayas va como lineas");
    let (v, b) = deshacer(&json);
    assert_eq!(v["brochas"][0]["t"], "#ff0000");
    // Veinte tramos seguidos se cosen en un camino de 21 puntos.
    assert_eq!(b[0].1.len(), 21);
    assert_eq!(b[0].1[20], (40 * 256, 190 * 256));
}

#[test]
fn un_pdf_que_es_solo_una_imagen_no_se_manda_como_plano() {
    let bytes = pagina_con(
        b"q 100 0 0 100 0 0 cm /Im1 Do Q",
        Pagina {
            recursos: "/XObject << /Im1 5 0 R >>",
            mas: vec![crate::plano::pruebas::flujo(
                5,
                b"xxxx",
                " /Type /XObject /Subtype /Image /Width 2 /Height 2",
            )],
            ..Default::default()
        },
    );
    assert!(de_bytes(&bytes, 0, 100.0).is_none());
}

#[test]
fn los_numeros_llevan_los_decimales_justos() {
    assert_eq!(numero(0.916_666_666, 3), "0.917");
    assert_eq!(numero(2.0, 3), "2");
    assert_eq!(numero(-0.5, 3), "-0.5");
    assert_eq!(numero(f64::NAN, 3), "0");
}

fn de_base64(s: &str) -> Vec<u8> {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut v, mut n) = (0u32, 0);
    for c in s.bytes().filter(|c| *c != b'=') {
        v = (v << 6) | A.iter().position(|x| *x == c).expect("letra de base64") as u32;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((v >> n) as u8);
        }
    }
    out
}

#[test]
fn varias_paginas_de_una_vez_dan_una_respuesta_por_pagina() {
    let mut c = String::new();
    for i in 0..20 {
        c.push_str(&format!("{i} 10 m {} 10 l S ", i + 1));
    }
    let bytes = pagina_con(c.as_bytes(), Pagina::default());
    let r = de_paginas(&bytes, &[0, 7], 800.0);
    assert_eq!(r.len(), 2);
    assert!(r[0].is_some());
    assert!(r[1].is_none(), "una pagina que no existe no da plano");
    assert_eq!(de_paginas(b"no es un pdf", &[0], 800.0), vec![None]);
}
