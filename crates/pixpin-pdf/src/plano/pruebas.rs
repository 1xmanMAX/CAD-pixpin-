//! **Leer un PDF como lineas**, sin Windows. Los archivos se escriben aqui a
//! mano (como en `PlanoDePdfTest` del movil): asi cada prueba ensena
//! exactamente el caso que comprueba en vez de esconderlo en un binario. Lo
//! que se mira siempre es que la geometria caiga **donde se ve**: origen
//! arriba a la izquierda, la y hacia abajo y el giro ya aplicado.

use super::*;

/// Arma un PDF con estos objetos —el primero es el 1— y su tabla clasica.
pub(crate) fn pdf_con(objetos: &[Vec<u8>]) -> Vec<u8> {
    let mut s = b"%PDF-1.5\n".to_vec();
    let mut donde = Vec::new();
    for o in objetos {
        donde.push(s.len());
        s.extend_from_slice(o);
    }
    let inicio = s.len();
    let mut t = format!("xref\n0 {}\n0000000000 65535 f \n", objetos.len() + 1);
    for d in donde {
        t.push_str(&format!("{d:010} 00000 n \n"));
    }
    t.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{inicio}\n%%EOF\n",
        objetos.len() + 1
    ));
    s.extend_from_slice(t.as_bytes());
    s
}

fn latin(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u32 as u8).collect()
}

pub(crate) fn flujo(numero: u32, datos: &[u8], dicc: &str) -> Vec<u8> {
    let mut o = format!(
        "{numero} 0 obj\n<< /Length {}{dicc} >>\nstream\n",
        datos.len()
    )
    .into_bytes();
    o.extend_from_slice(datos);
    o.extend_from_slice(b"\nendstream\nendobj\n");
    o
}

#[derive(Default)]
pub(crate) struct Pagina<'a> {
    pub caja: Option<&'a str>,
    pub extra: &'a str,
    pub recursos: &'a str,
    pub catalogo: &'a str,
    pub filtro: &'a str,
    pub mas: Vec<Vec<u8>>,
}

/// Una pagina de 100x200 con este contenido; lo de mas va desde el objeto 5.
pub(crate) fn pagina_con(contenido: &[u8], p: Pagina) -> Vec<u8> {
    let caja = p.caja.unwrap_or("0 0 100 200");
    let mut objetos = vec![
        format!("1 0 obj\n<< /Type /Catalog /Pages 2 0 R {}>>\nendobj\n", p.catalogo).into_bytes(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [{caja}] /Contents 4 0 R /Resources << {} >> {}>>\nendobj\n",
            p.recursos, p.extra
        )
        .into_bytes(),
        flujo(4, contenido, p.filtro),
    ];
    objetos.extend(p.mas);
    pdf_con(&objetos)
}

fn leer(bytes: &[u8]) -> Option<Plano> {
    de_bytes(bytes, 0)
}

fn simple(contenido: &str) -> Plano {
    leer(&pagina_con(contenido.as_bytes(), Pagina::default())).expect("la pagina se lee")
}

/// Los puntos de una brocha en puntos del papel, para compararlos a ojo.
fn puntos(b: &Brocha) -> Vec<(f64, f64)> {
    b.xs.iter()
        .zip(&b.ys)
        .map(|(x, y)| (*x as f64 / FINEZA as f64, *y as f64 / FINEZA as f64))
        .collect()
}

// ---- La geometria ----

#[test]
fn una_raya_sale_con_su_color_su_grosor_y_del_derecho() {
    let p = simple("1 0 0 RG 2 w 10 20 m 30 40 l S");
    assert_eq!((p.ancho, p.alto), (100.0, 200.0));
    let b = &p.brochas[0];
    assert_eq!(p.brochas.len(), 1);
    assert_eq!(b.color, 0xff0000);
    assert!((b.grosor - 2.0).abs() < 1e-3);
    assert!(!b.relleno, "una raya no se rellena");
    assert_eq!(b.ops, vec![MOVER, LINEA]);
    // La y del PDF sube y la de la pantalla baja: 20 desde abajo son 180.
    assert_eq!(puntos(b), vec![(10.0, 180.0), (30.0, 160.0)]);
}

#[test]
fn un_rectangulo_relleno_son_cuatro_esquinas_y_un_cierre() {
    let p = simple("0.5 g 10 10 50 40 re f");
    let b = &p.brochas[0];
    assert!(b.relleno);
    assert_eq!(b.color, 0x808080);
    assert_eq!(b.ops, vec![MOVER, LINEA, LINEA, LINEA, CERRAR]);
    assert_eq!(puntos(b)[0], (10.0, 190.0));
    assert_eq!(puntos(b)[2], (60.0, 150.0));
}

#[test]
fn pintar_y_trazar_el_mismo_camino_da_dos_brochas() {
    let p = simple("1 0 0 rg 0 0 1 RG 10 10 20 20 re B");
    assert_eq!(p.brochas.len(), 2);
    assert_eq!(
        p.brochas.iter().find(|b| b.relleno).unwrap().color,
        0xff0000
    );
    assert_eq!(
        p.brochas.iter().find(|b| !b.relleno).unwrap().color,
        0x0000ff
    );
}

/// La matriz manda en todo, **incluido el grosor**: un plano de AutoCAD
/// empieza con un `cm` que lo achica doce veces.
#[test]
fn la_matriz_encoge_el_dibujo_y_el_grosor_con_el() {
    let p = simple("0.5 0 0 0.5 0 0 cm 4 w 0 0 m 100 100 l S");
    let b = &p.brochas[0];
    assert!((b.grosor - 2.0).abs() < 1e-3);
    assert_eq!(puntos(b), vec![(0.0, 200.0), (50.0, 150.0)]);
}

#[test]
fn q_y_mayuscula_q_devuelven_el_estado_como_estaba() {
    let p = simple("q 3 w 1 0 0 RG 0 0 m 1 1 l S Q 0 0 m 2 2 l S");
    assert_eq!(p.brochas.len(), 2);
    let fuera = p.brochas.iter().find(|b| b.color == 0).unwrap();
    assert!(
        (fuera.grosor - 1.0).abs() < 1e-3,
        "el grosor de dentro se ha escapado"
    );
}

#[test]
fn las_curvas_viajan_como_curvas_y_no_como_trocitos() {
    let p = simple("0 0 m 10 0 20 10 20 20 c S");
    let b = &p.brochas[0];
    assert_eq!(b.ops, vec![MOVER, CURVA]);
    assert_eq!(b.xs.len(), 4, "una curva gasta tres puntos");
}

/// **`sc` habla en el espacio que puso `cs`**, y con una paleta sus numeros
/// son indices: adivinando, un `scn 2` salia negro.
#[test]
fn una_paleta_indexada_da_el_color_de_su_indice_y_no_un_gris() {
    let p = leer(&pagina_con(
        b"/P0 cs 2 scn 10 10 20 20 re f",
        Pagina {
            recursos: "/ColorSpace << /P0 5 0 R >>",
            mas: vec![b"5 0 obj\n[/Indexed /DeviceRGB 2 <FF000000FF000000FF>]\nendobj\n".to_vec()],
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(p.brochas[0].color, 0x0000ff);
    assert_eq!(
        p.sin_entender, 0,
        "una paleta se entiende: no hay que volver a la foto"
    );
}

#[test]
fn poner_un_espacio_indexado_deja_el_primer_color_de_la_paleta() {
    let p = leer(&pagina_con(
        b"/P0 cs 10 10 20 20 re f",
        Pagina {
            recursos: "/ColorSpace << /P0 5 0 R >>",
            mas: vec![b"5 0 obj\n[/Indexed /DeviceRGB 1 <11223344 5566>]\nendobj\n".to_vec()],
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(p.brochas[0].color, 0x112233);
}

#[test]
fn el_icc_dice_cuantos_canales_tiene_su_espacio() {
    let p = leer(&pagina_con(
        b"/I0 cs 0 1 1 0 sc 10 10 20 20 re f",
        Pagina {
            recursos: "/ColorSpace << /I0 [/ICCBased 5 0 R] >>",
            mas: vec![b"5 0 obj\n<< /N 4 >>\nendobj\n".to_vec()],
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(
        p.brochas[0].color, 0xff0000,
        "cian a cero, magenta y amarillo a tope: rojo"
    );
}

#[test]
fn una_tinta_plana_va_de_blanco_a_negro() {
    let p = leer(&pagina_con(
        b"/S0 cs 1 sc 10 10 20 20 re f /S0 cs 0 sc 40 10 20 20 re f",
        Pagina {
            recursos: "/ColorSpace << /S0 [/Separation /Tinta /DeviceCMYK 5 0 R] >>",
            mas: vec![b"5 0 obj\n<< /FunctionType 2 /N 1 >>\nendobj\n".to_vec()],
            ..Default::default()
        },
    ))
    .unwrap();
    let izq = p.brochas.iter().find(|b| puntos(b)[0].0 < 30.0).unwrap();
    let der = p.brochas.iter().find(|b| puntos(b)[0].0 > 30.0).unwrap();
    assert_eq!((izq.color, der.color), (0x000000, 0xffffff));
}

#[test]
fn sin_espacio_conocido_se_adivina_por_los_numeros() {
    assert_eq!(
        simple("1 0 0 sc 10 10 20 20 re f").brochas[0].color,
        0xff0000
    );
}

#[test]
fn el_cmyk_se_convierte_a_color_de_pantalla() {
    assert_eq!(simple("0 1 1 0 K 0 0 m 1 1 l S").brochas[0].color, 0xff0000);
}

/// Los rellenos van en el orden en que se pintan: una caja blanca encima de
/// una negra, otra negra encima. Juntarlos por color los reordenaba.
#[test]
fn los_rellenos_conservan_el_orden_en_que_se_pintaron() {
    let p = simple("0 g 0 0 10 10 re f 1 g 2 2 5 5 re f 0 g 3 3 1 1 re f");
    let colores: Vec<u32> = p.brochas.iter().map(|b| b.color).collect();
    assert_eq!(colores, vec![0x000000, 0xffffff, 0x000000]);
}

#[test]
fn un_relleno_par_impar_se_dice() {
    let p = simple("0 0 10 10 re 2 2 5 5 re f*");
    assert!(p.brochas[0].par_impar);
    assert!(
        !simple("0 0 10 10 re f").brochas[0].par_impar,
        "caso negativo: el normal no lo es"
    );
}

// ---- Las capas ----

fn con_capas(contenido: &str, off: &str) -> Plano {
    let catalogo = format!(
        "/OCProperties << /OCGs [5 0 R 6 0 R] /D << /Order [5 0 R 6 0 R] /OFF [{off}] >> >> "
    );
    leer(&pagina_con(
        contenido.as_bytes(),
        Pagina {
            recursos: "/Properties << /oc1 5 0 R /oc2 6 0 R >>",
            catalogo: &catalogo,
            mas: vec![
                b"5 0 obj\n<< /Type /OCG /Name (Muros) >>\nendobj\n".to_vec(),
                b"6 0 obj\n<< /Type /OCG /Name (Cotas) >>\nendobj\n".to_vec(),
            ],
            ..Default::default()
        },
    ))
    .unwrap()
}

#[test]
fn cada_capa_del_pdf_sale_con_su_nombre_y_lo_suyo_dentro() {
    let p = con_capas(
        "/OC /oc1 BDC 0 0 m 1 1 l S EMC /OC /oc2 BDC 1 0 0 RG 2 0 m 3 3 l S EMC 5 5 m 6 6 l S",
        "",
    );
    let nombres: Vec<&str> = p.capas.iter().map(|c| c.nombre.as_str()).collect();
    assert_eq!(nombres, vec!["Muros", "Cotas"]);
    assert!(p.capas.iter().all(|c| c.encendida));
    assert!(p.brochas.iter().any(|b| b.color == 0 && b.capa == 0));
    assert_eq!(
        p.brochas.iter().find(|b| b.color == 0xff0000).unwrap().capa,
        1
    );
    assert!(
        p.brochas.iter().any(|b| b.capa == -1),
        "falta lo que va fuera de las capas"
    );
}

#[test]
fn una_capa_apagada_en_el_pdf_llega_apagada() {
    let p = con_capas("/OC /oc2 BDC 0 0 m 1 1 l S EMC", "6 0 R");
    assert!(p.capas[0].encendida);
    assert!(!p.capas[1].encendida);
}

#[test]
fn las_marcas_metidas_unas_en_otras_devuelven_la_capa_de_fuera() {
    let p = con_capas(
        "/OC /oc1 BDC /Span <</Lang (es)>> BDC 0 0 m 1 1 l S EMC 2 2 m 3 3 l S EMC",
        "",
    );
    assert!(p.brochas.iter().all(|b| b.capa == 0));
}

// ---- La pagina ----

#[test]
fn una_pagina_girada_un_cuarto_de_vuelta_sale_apaisada() {
    let p = leer(&pagina_con(
        b"0 0 m 10 20 l S",
        Pagina {
            extra: "/Rotate 90 ",
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!((p.ancho, p.alto), (200.0, 100.0));
    assert_eq!(puntos(&p.brochas[0]), vec![(0.0, 0.0), (20.0, 10.0)]);
}

#[test]
fn con_tres_cuartos_de_vuelta_el_papel_tambien_se_tumba() {
    let p = leer(&pagina_con(
        b"0 0 m 10 20 l S",
        Pagina {
            extra: "/Rotate 270 ",
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!((p.ancho, p.alto), (200.0, 100.0));
    assert_eq!(puntos(&p.brochas[0]), vec![(200.0, 100.0), (180.0, 90.0)]);
}

#[test]
fn el_mediabox_que_no_empieza_en_cero_se_lleva_al_origen() {
    let p = leer(&pagina_con(
        b"50 50 m 60 60 l S",
        Pagina {
            caja: Some("50 50 150 250"),
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(p.ancho, 100.0);
    assert_eq!(puntos(&p.brochas[0]), vec![(0.0, 200.0), (10.0, 190.0)]);
}

// ---- Lo que no se manda ----

#[test]
fn lo_que_el_recorte_deja_fuera_no_viaja() {
    let p = simple("q 0 0 100 100 re W n 10 10 m 20 20 l S 10 150 m 20 160 l S Q");
    let b = &p.brochas[0];
    assert_eq!(b.xs.len(), 2, "solo tenia que quedar la raya de dentro");
    assert_eq!(puntos(b)[0], (10.0, 190.0));
}

fn imagen(numero: u32, datos: &[u8], dicc: &str) -> Vec<u8> {
    flujo(
        numero,
        datos,
        &format!(" /Type /XObject /Subtype /Image {dicc}"),
    )
}

#[test]
fn una_imagen_que_no_es_jpeg_se_cuenta_y_no_se_dibuja() {
    let p = leer(&pagina_con(
        b"q 100 0 0 100 0 0 cm /Im1 Do Q 0 0 m 1 1 l S",
        Pagina {
            recursos: "/XObject << /Im1 5 0 R >>",
            mas: vec![imagen(5, b"xxxx", "/Width 2 /Height 2")],
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(p.sin_entender, 1);
    assert_eq!(p.brochas.len(), 1);
    assert!(
        !p.se_manda_como_lineas(),
        "con algo que falta, la pagina va como foto"
    );
}

#[test]
fn una_foto_en_jpeg_se_pasa_tal_cual_con_su_sitio() {
    let jpeg = latin("\u{ff}\u{d8}\u{ff}\u{e0}FOTO");
    let p = leer(&pagina_con(
        b"q 40 0 0 20 10 30 cm /Im1 Do Q 0 0 m 1 1 l S",
        Pagina {
            recursos: "/XObject << /Im1 5 0 R >>",
            mas: vec![imagen(
                5,
                &jpeg,
                "/Width 4 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode",
            )],
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(p.sin_entender, 0);
    let f = &p.fotos[0];
    assert_eq!(f.tipo, "image/jpeg");
    assert_eq!(*f.datos, jpeg);
    // El cuadro va de (10,30) a (50,50): arriba a la izquierda es (10, 200-50).
    assert_eq!((f.x, f.y, f.a, f.d), (10.0, 150.0, 40.0, 20.0));
}

#[test]
fn la_misma_imagen_colocada_dos_veces_lleva_la_misma_sena() {
    let jpeg = latin("\u{ff}\u{d8}\u{ff}\u{e0}FOTO");
    let p = leer(&pagina_con(
        b"q 10 0 0 10 0 0 cm /Im1 Do Q q 10 0 0 10 40 40 cm /Im1 Do Q 0 0 m 1 1 l S",
        Pagina {
            recursos: "/XObject << /Im1 5 0 R >>",
            mas: vec![imagen(5, &jpeg, "/Width 4 /Height 2 /Filter /DCTDecode")],
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(p.fotos.len(), 2);
    assert_eq!(p.fotos[0].id, p.fotos[1].id);
    assert!(
        Arc::ptr_eq(&p.fotos[0].datos, &p.fotos[1].datos),
        "los bytes se comparten, no se copian"
    );
    assert_ne!(p.fotos[0].x, p.fotos[1].x);
}

#[test]
fn una_foto_con_mascara_de_transparencia_se_pasa_con_las_dos() {
    let color = latin("\u{ff}\u{d8}\u{ff}\u{e0}COLOR");
    let gris = latin("\u{ff}\u{d8}\u{ff}\u{e0}MASCARA");
    let p = leer(&pagina_con(
        b"q 10 0 0 10 0 0 cm /Im1 Do Q 0 0 m 1 1 l S",
        Pagina {
            recursos: "/XObject << /Im1 5 0 R >>",
            mas: vec![
                imagen(
                    5,
                    &color,
                    "/Width 4 /Height 2 /SMask 6 0 R /Filter /DCTDecode",
                ),
                imagen(
                    6,
                    &gris,
                    "/Width 4 /Height 2 /ColorSpace /DeviceGray /Filter /DCTDecode",
                ),
            ],
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(p.sin_entender, 0);
    assert_eq!(p.fotos[0].tipo_mascara, Some("image/jpeg"));
    assert_eq!(**p.fotos[0].mascara.as_ref().unwrap(), gris);
}

#[test]
fn una_mascara_de_otro_tamano_no_se_pasa() {
    let p = leer(&pagina_con(
        b"q 10 0 0 10 0 0 cm /Im1 Do Q",
        Pagina {
            recursos: "/XObject << /Im1 5 0 R >>",
            mas: vec![
                imagen(
                    5,
                    b"xx",
                    "/Width 4 /Height 2 /SMask 6 0 R /Filter /DCTDecode",
                ),
                imagen(6, b"yy", "/Width 2 /Height 1 /Filter /DCTDecode"),
            ],
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(p.sin_entender, 1);
    assert!(p.fotos.is_empty());
}

#[test]
fn una_pagina_que_es_solo_una_imagen_no_vale_la_pena() {
    let p = leer(&pagina_con(
        b"q 100 0 0 100 0 0 cm /Im1 Do Q",
        Pagina {
            recursos: "/XObject << /Im1 5 0 R >>",
            mas: vec![imagen(5, b"xxxx", "/Width 2 /Height 2")],
            ..Default::default()
        },
    ))
    .unwrap();
    assert!(!p.vale_la_pena(), "un escaneo no es un plano vectorial");
}

#[test]
fn un_formulario_se_ejecuta_con_su_matriz() {
    let p = leer(&pagina_con(
        b"q 1 0 0 1 10 10 cm /Fm1 Do Q",
        Pagina {
            recursos: "/XObject << /Fm1 5 0 R >>",
            mas: vec![flujo(
                5,
                b"0 0 m 10 10 l S",
                " /Type /XObject /Subtype /Form /BBox [0 0 20 20] /Matrix [2 0 0 2 0 0]",
            )],
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(puntos(&p.brochas[0]), vec![(10.0, 190.0), (30.0, 170.0)]);
}

// ---- El texto ----

fn con_fuente(contenido: &str) -> Plano {
    let cmap = b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n2 beginbfchar\n<0003> <0041>\n<0004> <00d1>\nendbfchar\nendcmap\nend end";
    leer(&pagina_con(
        contenido.as_bytes(),
        Pagina {
            recursos: "/Font << /F1 5 0 R >>",
            mas: vec![
                b"5 0 obj\n<< /Type /Font /Subtype /Type0 /BaseFont /ArialMT /Encoding /Identity-H /DescendantFonts [6 0 R] /ToUnicode 7 0 R >>\nendobj\n".to_vec(),
                b"6 0 obj\n<< /Type /Font /Subtype /CIDFontType2 /BaseFont /ArialMT /DW 1000 /W [3 [500 500]] >>\nendobj\n".to_vec(),
                flujo(7, cmap, ""),
            ],
            ..Default::default()
        },
    ))
    .unwrap()
}

#[test]
fn un_rotulo_sale_con_lo_que_dice_donde_y_cuanto_ocupa() {
    let p = con_fuente("BT /F1 10 Tf 20 100 Td <00030004> Tj ET");
    let t = &p.textos[0];
    assert_eq!(t.texto, "AÑ");
    assert_eq!((t.x, t.y), (20.0, 100.0));
    assert!(
        (t.ancho - 1.0).abs() < 1e-9,
        "dos letras de media eme: una eme"
    );
    assert_eq!((t.a, t.b, t.d), (10.0, 0.0, 10.0));
    assert_eq!(t.familia, "sans-serif");
}

/// **MacRoman** (Android v0.98.0): la «fi» de un articulo hecho en Mac es
/// «fi» y no «Þ», o el Ctrl+F de la pagina web no encuentra «defined».
#[test]
fn en_macroman_la_ligadura_fi_sale_fi_y_en_winansi_sigue_siendo_thorn() {
    let con = |encoding: &str| {
        leer(&pagina_con(
            b"BT /F1 10 Tf 20 100 Td (de\\336ned) Tj ET",
            Pagina {
                recursos: "/Font << /F1 5 0 R >>",
                mas: vec![format!("5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman /Encoding {encoding} >>\nendobj\n").into_bytes()],
                ..Default::default()
            },
        ))
        .unwrap()
    };
    assert_eq!(con("/MacRomanEncoding").textos[0].texto, "defined");
    assert_eq!(
        con("<< /BaseEncoding /MacRomanEncoding >>").textos[0].texto,
        "defined"
    );
    // Caso negativo: en WinAnsi (o sin decir nada) sigue siendo la thorn.
    assert_eq!(con("/WinAnsiEncoding").textos[0].texto, "deÞned");
}

#[test]
fn la_matriz_del_texto_lo_gira() {
    let p = con_fuente("BT /F1 10 Tf 0 1 -1 0 30 40 Tm <0003> Tj ET");
    let t = &p.textos[0];
    assert!(t.a.abs() < 1e-9);
    assert_eq!((t.b, t.x, t.y), (-10.0, 30.0, 160.0));
}

#[test]
fn el_texto_invisible_del_escaneo_no_se_manda() {
    assert!(
        con_fuente("BT /F1 10 Tf 3 Tr 20 100 Td <0003> Tj ET")
            .textos
            .is_empty()
    );
}

#[test]
fn los_saltos_de_renglon_van_bajando_el_texto() {
    let p = con_fuente("BT /F1 10 Tf 12 TL 20 100 Td <0003> Tj T* <0003> Tj ET");
    assert_eq!(p.textos.len(), 2);
    assert_eq!((p.textos[0].y, p.textos[1].y), (100.0, 112.0));
}

#[test]
fn un_empujon_dentro_de_tj_separa_lo_que_viene_detras() {
    let p = con_fuente("BT /F1 10 Tf 20 100 Td [<0003> -1000 <0003>] TJ ET");
    assert_eq!(p.textos[1].x, 35.0);
}

#[test]
fn un_tramo_del_cmap_con_lista_da_cada_letra() {
    let mut out = HashMap::new();
    leer_cmap(
        b"1 beginbfrange\n<0001> <0002> [<0048> <0049>]\nendbfrange",
        &mut out,
    );
    assert_eq!(out.get(&1).map(String::as_str), Some("H"));
    assert_eq!(out.get(&2).map(String::as_str), Some("I"));
    let mut seguidas = HashMap::new();
    leer_cmap(
        b"1 beginbfrange\n<20> <22> <0041>\nendbfrange",
        &mut seguidas,
    );
    assert_eq!(seguidas.get(&0x22).map(String::as_str), Some("C"));
}

// ---- Cuando no se puede ----

#[test]
fn un_pdf_cifrado_no_se_toca() {
    let b = pagina_con(b"0 0 m 1 1 l S", Pagina::default());
    let t = String::from_utf8_lossy(&b).replace("/Root 1 0 R", "/Root 1 0 R /Encrypt 9 0 R");
    assert!(leer(t.as_bytes()).is_none());
}

#[test]
fn un_contenido_en_hexadecimal_se_lee_igual() {
    let hex: String = "1 0 0 RG 10 20 m 30 40 l S"
        .bytes()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        + ">";
    let p = leer(&pagina_con(
        hex.as_bytes(),
        Pagina {
            filtro: " /Filter /ASCIIHexDecode",
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(puntos(&p.brochas[0]), vec![(10.0, 180.0), (30.0, 160.0)]);
}

fn a_base85(t: &[u8]) -> Vec<u8> {
    let mut s = Vec::new();
    for trozo in t.chunks(4) {
        let mut v: u64 = 0;
        for k in 0..4 {
            v = v * 256 + *trozo.get(k).unwrap_or(&0) as u64;
        }
        let mut letras = [0u8; 5];
        for k in (0..5).rev() {
            letras[k] = b'!' + (v % 85) as u8;
            v /= 85;
        }
        s.extend_from_slice(&letras[..trozo.len() + 1]);
    }
    s.extend_from_slice(b"~>");
    s
}

#[test]
fn un_contenido_en_base_85_se_lee_igual() {
    let p = leer(&pagina_con(
        &a_base85(b"10 20 m 30 40 l S"),
        Pagina {
            filtro: " /Filter /ASCII85Decode",
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(puntos(&p.brochas[0]), vec![(10.0, 180.0), (30.0, 160.0)]);
}

#[test]
fn un_contenido_comprimido_con_flate_se_lee_igual() {
    use std::io::Write;
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(b"10 20 m 30 40 l S").unwrap();
    let p = leer(&pagina_con(
        &z.finish().unwrap(),
        Pagina {
            filtro: " /Filter /FlateDecode",
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(puntos(&p.brochas[0]), vec![(10.0, 180.0), (30.0, 160.0)]);
}

#[test]
fn un_archivo_que_no_es_un_pdf_devuelve_nada() {
    assert!(leer(b"esto no es un pdf").is_none());
    assert!(
        de_bytes(&pagina_con(b"0 0 m 1 1 l S", Pagina::default()), 3).is_none(),
        "ni una pagina que no existe"
    );
}

#[test]
fn lo_que_no_se_entiende_se_salta_sin_llevarse_el_resto() {
    let p = simple("/Nada gs 4 sh 0 0 m 1 1 l S 99 99 raro");
    assert_eq!(p.brochas.len(), 1);
    assert_eq!(p.sin_entender, 1, "el degradado se cuenta");
}

/// El tope: una pagina con mas puntos de la cuenta se corta y lo dice, y
/// entonces no se manda como lineas.
#[test]
fn pasado_el_tope_de_puntos_la_lectura_se_corta_y_lo_dice() {
    let mut c = String::new();
    // Cada raya son dos puntos; se escriben las justas para pasar el tope
    // en una prueba rapida bajando el tope no se puede (es constante), asi
    // que se usa un rectangulo repetido con `re`, que da cuatro.
    let veces = TOPE_DE_PUNTOS / 4 + 10;
    c.reserve(veces * 16);
    for _ in 0..veces {
        c.push_str("1 1 2 2 re f\n");
    }
    let p = simple(&c);
    assert!(p.cortado);
    assert!(!p.se_manda_como_lineas());
}

// ---- Los renglones, juntos para que se busquen ----

#[test]
fn una_palabra_partida_por_el_interletrado_sale_en_un_solo_rotulo() {
    // Word escribe «Beneficios» como `[(B)20(e)-15(ne)…] TJ`: sin juntar,
    // el Ctrl+F del navegador no encuentra la palabra.
    let p = con_fuente("BT /F1 10 Tf 20 100 Td [<0003> 20 <0004> -30 <0003>] TJ ET");
    assert_eq!(
        p.textos.len(),
        1,
        "{:?}",
        p.textos.iter().map(|t| &t.texto).collect::<Vec<_>>()
    );
    assert_eq!(p.textos[0].texto, "AÑA");
    // Lo que ocupa es de la primera letra al final de la ultima.
    let t = &p.textos[0];
    assert!(
        (t.x + t.a * t.ancho - (20.0 + 15.0 + 0.1)).abs() < 0.01,
        "ancho {}",
        t.ancho
    );
}

#[test]
fn dos_palabras_separadas_por_un_hueco_de_espacio_llevan_su_espacio() {
    let p = con_fuente("BT /F1 10 Tf 20 100 Td [<0003> -300 <0004>] TJ ET");
    assert_eq!(p.textos.len(), 1);
    assert_eq!(p.textos[0].texto, "A Ñ");
}

#[test]
fn lo_de_otro_renglon_otro_tamano_o_muy_lejos_no_se_junta() {
    assert_eq!(
        con_fuente("BT /F1 10 Tf 12 TL 20 100 Td <0003> Tj T* <0003> Tj ET")
            .textos
            .len(),
        2
    );
    assert_eq!(
        con_fuente("BT /F1 10 Tf 20 100 Td <0003> Tj /F1 14 Tf <0003> Tj ET")
            .textos
            .len(),
        2
    );
    // Una columna al lado: el hueco es de varias emes.
    assert_eq!(
        con_fuente("BT /F1 10 Tf 20 100 Td [<0003> -3000 <0003>] TJ ET")
            .textos
            .len(),
        2
    );
}

// ---- Las imagenes que no son JPEG ----

fn comprimido(datos: &[u8]) -> Vec<u8> {
    use std::io::Write as _;
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(datos).unwrap();
    z.finish().unwrap()
}

fn con_imagen(dicc: &str, datos: &[u8], mas: Vec<Vec<u8>>) -> Plano {
    let mut todo = vec![imagen(5, datos, dicc)];
    todo.extend(mas);
    leer(&pagina_con(
        b"q 40 0 0 20 10 30 cm /Im1 Do Q 0 0 m 1 1 l S",
        Pagina {
            recursos: "/XObject << /Im1 5 0 R >>",
            mas: todo,
            ..Default::default()
        },
    ))
    .unwrap()
}

#[test]
fn una_imagen_en_flate_se_pasa_como_png_sin_mandar_la_hoja_como_foto() {
    // Una captura de Word: RGB en Flate, sin predictor.
    let p = con_imagen(
        "/Width 2 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode",
        &comprimido(&[255, 0, 0, 0, 0, 255]),
        Vec::new(),
    );
    assert_eq!(p.sin_entender, 0);
    assert_eq!(p.fotos.len(), 1);
    assert_eq!(p.fotos[0].tipo, "image/png");
    assert!(p.fotos[0].datos.starts_with(b"\x89PNG"));
    assert!(p.fotos[0].mascara.is_none());
}

#[test]
fn con_predictor_y_transparencia_la_imagen_sale_entera_en_un_png() {
    // Predictor PNG «Sub» (tipo 1) en color, y su mascara en gris.
    let filas = [1u8, 10, 20, 30, 5, 5, 5];
    let p = con_imagen(
        "/Width 2 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /DecodeParms << /Predictor 15 /Colors 3 /Columns 2 >> /SMask 6 0 R",
        &comprimido(&filas),
        vec![imagen(
            6,
            &comprimido(&[255, 0]),
            "/Width 2 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode",
        )],
    );
    assert_eq!(p.sin_entender, 0);
    let png = &p.fotos[0].datos;
    // RGBA: el tipo de color 6 en la cabecera.
    assert_eq!(png[25], 6, "sin su alfa");
}

#[test]
fn una_imagen_en_jbig2_o_con_una_mascara_rara_sigue_mandando_la_hoja_como_foto() {
    let p = con_imagen(
        "/Width 8 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /JBIG2Decode",
        &[0xAA],
        Vec::new(),
    );
    assert_eq!(p.sin_entender, 1);
    let p = con_imagen(
        "/Width 2 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /SMask 6 0 R",
        &comprimido(&[1, 2, 3, 4, 5, 6]),
        vec![imagen(
            6,
            &comprimido(&[1, 2, 3]),
            "/Width 3 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode",
        )],
    );
    assert_eq!(p.sin_entender, 1, "una mascara de otro tamano no se estira");
}

#[test]
fn la_silueta_de_un_bit_con_su_mascara_de_word_se_pasa_como_png() {
    // Lo que escribe Word para una figura: un bit por pixel en gris y la
    // forma de verdad en su mascara.
    let p = con_imagen(
        "/Width 8 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /FlateDecode /SMask 6 0 R",
        &comprimido(&[0x0F]),
        vec![imagen(
            6,
            &comprimido(&[0, 255, 0, 255, 0, 255, 0, 255]),
            "/Width 8 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode",
        )],
    );
    assert_eq!(p.sin_entender, 0);
    assert_eq!(p.fotos[0].tipo, "image/png");
    assert_eq!(p.fotos[0].datos[25], 4, "gris con alfa");
}

#[test]
fn un_jpeg_con_su_mascara_en_flate_lleva_la_mascara_como_png() {
    let jpeg = latin("\u{ff}\u{d8}\u{ff}\u{e0}FOTO");
    let p = con_imagen(
        "/Width 2 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /SMask 6 0 R",
        &jpeg,
        vec![imagen(
            6,
            &comprimido(&[255, 0]),
            "/Width 2 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode",
        )],
    );
    assert_eq!(p.sin_entender, 0);
    assert_eq!(p.fotos[0].tipo, "image/jpeg");
    assert_eq!(p.fotos[0].tipo_mascara, Some("image/png"));
}
