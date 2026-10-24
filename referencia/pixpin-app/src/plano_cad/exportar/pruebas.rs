//! Pruebas del exportador de planos: un DXF pequeno con dos capas y
//! grosores, una cota, un marco y una raya anotada con el motor.

use super::*;
use pixpin_motor2d::{ColorRgba, Elemento, Figura};

/// Un DXF de dos capas: «Muros» con 0,50 mm y «Ejes» apagada.
const DXF: &str = "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n2\n0\nLAYER\n2\nMuros\n70\n0\n62\n1\n6\nCONTINUOUS\n370\n50\n0\nLAYER\n2\nEjes\n70\n0\n62\n-5\n6\nCONTINUOUS\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nMuros\n10\n0\n20\n0\n30\n0\n11\n100\n21\n0\n31\n0\n0\nLINE\n8\nMuros\n10\n100\n20\n0\n30\n0\n11\n100\n21\n60\n31\n0\n0\nCIRCLE\n8\nEjes\n10\n50\n20\n30\n30\n0\n40\n20\n0\nENDSEC\n0\nEOF\n";

struct Prueba {
    carpeta: PathBuf,
    plano: Plano,
}

fn carpeta(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "pixpin-exportar-cad-{nombre}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn prueba(nombre: &str, anotar: bool) -> Prueba {
    let d = carpeta(nombre);
    let ruta = d.join("planta.dxf");
    std::fs::write(&ruta, DXF).unwrap();
    let (modelo, _) = pixpin_cad::convertir::convertir_fichero(&ruta).unwrap();
    assert!(!modelo.vacio());
    let u = modelo.caja_util();
    let cotas = vec![vec![[u[0] as f64, u[1] as f64], [u[2] as f64, u[1] as f64]]];
    let marcos = vec![[u[0] as f64, u[1] as f64, u[2] as f64, u[3] as f64]];
    let plano = Plano::nuevo(&d, &ruta, &modelo, &cotas, &marcos);
    if anotar {
        // Una raya roja gorda en la capa, como la deja el motor.
        let mut capa = crate::lector_tinta::Capa::default();
        let puntos = vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(40.0, 20.0)];
        capa.escena.anadir(Elemento {
            x: 0.0,
            y: 0.0,
            ancho: 40.0,
            alto: 20.0,
            trazo: ColorRgba::opaco(0.9, 0.1, 0.1),
            grosor: 3.0,
            figura: Figura::Linea { puntos },
            ..Default::default()
        });
        std::fs::create_dir_all(plano.capa.tinta.parent().unwrap()).unwrap();
        capa.sucia = true;
        capa.guardar(&plano.capa.tinta).unwrap();
    }
    Prueba { carpeta: d, plano }
}

/// Donde dejar lo que se quiere mirar: `PIXPIN_SALIDA` o la temporal.
fn salida() -> PathBuf {
    let d = std::env::var("PIXPIN_SALIDA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("pixpin-exportar-cad-salida"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn el_html_lleva_las_capas_los_grosores_las_cotas_y_lo_anotado() {
    let p = prueba("html", true);
    let h = html(&p.plano, pixpin_store::Idioma::Espanol).unwrap();
    assert!(h.starts_with("<!DOCTYPE html>"));
    assert!(h.contains("<title>planta</title>"));
    // Las dos capas, la apagada apagada; el grosor de 0,50 mm.
    assert!(h.contains(r#"{"n":"Muros","on":true"#), "sin la capa Muros");
    assert!(
        h.contains(r#"{"n":"Ejes","on":false"#),
        "sin la capa Ejes apagada"
    );
    assert!(h.contains(r#""g":50"#), "sin el grosor");
    // La cota, el marco y lo anotado (el SVG del motor y su encaje).
    assert!(h.contains(r#""cotas":[["#) && h.contains(r#""marcos":[["#));
    assert!(h.contains("<svg") && !h.contains(r#""anotado":null"#));
    std::fs::write(salida().join("plano-de-prueba.html"), &h).unwrap();
    let _ = std::fs::remove_dir_all(&p.carpeta);
}

#[test]
fn el_pdf_es_valido_se_abre_y_lleva_lo_anotado() {
    let p = prueba("pdf", true);
    let limpio = {
        let mut q = p.plano.clone();
        q.capa.tinta = p.carpeta.join("no-hay-capa.excalidraw");
        pdf(&q).unwrap()
    };
    let b = pdf(&p.plano).unwrap();
    assert!(b.starts_with(b"%PDF-1.4") && limpio.ends_with(b"%%EOF\n"));
    // Lo anotado va encima, en una revision mas: pesa mas que el limpio.
    assert!(b.len() > limpio.len(), "{} <= {}", b.len(), limpio.len());
    let r = salida().join("plano-de-prueba.pdf");
    std::fs::write(&r, &b).unwrap();
    // Se abre con el lector de Windows y tiene una hoja (un marco).
    let doc = pixpin_pdf::Documento::abrir(&r).unwrap();
    assert_eq!(doc.medidas().len(), 1);
    let img = doc.renderizar(0, 1200).unwrap();
    // Hay rojo (lo anotado o la capa Muros) sobre el blanco.
    let rojos = img
        .pixeles
        .chunks_exact(4)
        .filter(|p| p[0] > 150 && p[1] < 90 && p[2] < 90)
        .count();
    assert!(rojos > 50, "{rojos}");
    std::fs::write(
        salida().join("plano-de-prueba-pdf.png"),
        pixpin_codec::codificar_png(&img).unwrap(),
    )
    .unwrap();
    let _ = std::fs::remove_dir_all(&p.carpeta);
}

#[test]
fn la_imagen_sale_con_el_plano_pintado() {
    let p = prueba("png", true);
    let img = imagen(&p.plano).unwrap();
    assert!(img.ancho >= 1000 && img.alto >= 100);
    let oscuros = img
        .pixeles
        .chunks_exact(4)
        .filter(|p| p[0] < 200 || p[1] < 200)
        .count();
    assert!(oscuros > 100, "{oscuros}");
    std::fs::write(
        salida().join("plano-de-prueba.png"),
        pixpin_codec::codificar_png(&img).unwrap(),
    )
    .unwrap();
    let _ = std::fs::remove_dir_all(&p.carpeta);
}

#[test]
fn un_plano_vacio_o_roto_no_da_pdf_ni_imagen_pero_si_pagina() {
    let d = carpeta("vacio");
    let ruta = d.join("vacio.dxf");
    std::fs::write(&ruta, "0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n").unwrap();
    let p = Plano::nuevo(&d, &ruta, &Modelo::default(), &[], &[]);
    assert!(pdf(&p).is_err());
    assert!(imagen(&p).is_err());
    // La pagina sale (sin nada que pintar), no se cae.
    assert!(
        html(&p, pixpin_store::Idioma::Espanol)
            .unwrap()
            .contains("<canvas")
    );
    // Uno roto no se lee por capas.
    std::fs::write(&ruta, b"esto no es un plano").unwrap();
    assert!(pixpin_cad::convertir::convertir_fichero_por_capas(&ruta).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn el_dxf_de_un_plano_se_vuelve_a_leer() {
    let p = prueba("dxf", false);
    let r = p.carpeta.join("copia.dxf");
    pixpin_cad::leer::a_dxf(&p.plano.ruta, &r).unwrap();
    let (m, _) = pixpin_cad::convertir::convertir_fichero(&r).unwrap();
    assert!(!m.vacio());
    // Caso negativo: un destino donde no se puede escribir.
    assert!(
        pixpin_cad::leer::a_dxf(&p.plano.ruta, &p.carpeta.join("no-existe").join("x.dxf")).is_err()
    );
    let _ = std::fs::remove_dir_all(&p.carpeta);
}

#[test]
fn el_exportar_del_plano_ofrece_lo_minimo_y_falla_sin_permiso() {
    let p = prueba("menu", false);
    let x = exportable(p.plano.clone(), pixpin_store::Idioma::Espanol);
    let f = x.formatos();
    for id in ["web", "pdf", "png", "original", "pixpin"] {
        assert!(f.contains(&id), "falta {id}: {f:?}");
    }
    // Un DXF no se exporta a DXF; un DWG si.
    assert!(!f.contains(&"dxf"));
    // Caso negativo: una carpeta que no existe no deja escribir el HTML.
    let web = x.propios.iter().find(|q| q.id == "web").unwrap();
    assert!((web.hacer)(&p.carpeta.join("no-existe")).is_err());
    let hechos = (web.hacer)(&p.carpeta).unwrap();
    assert!(hechos[0].is_file());
    let _ = std::fs::remove_dir_all(&p.carpeta);
}
