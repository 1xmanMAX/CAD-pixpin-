use super::*;
use crate::biblioteca::{self, MAX_PIEZAS};
use std::collections::HashSet;

#[test]
fn el_catalogo_tiene_ids_fijos_unicos_y_cada_figura_su_categoria() {
    let c = catalogo();
    assert!(c.len() >= 70, "{}", c.len());
    let mut ids = HashSet::new();
    let mut nombres = HashSet::new();
    for f in &c {
        assert!(es_de_serie(f), "{}", f.id);
        assert!(ids.insert(f.id.clone()), "id repetido: {}", f.id);
        assert!(
            nombres.insert(f.nombre.clone()),
            "nombre repetido: {}",
            f.nombre
        );
        let (cat, _) =
            categoria_de(&f.nombre).unwrap_or_else(|| panic!("sin categoria: {}", f.nombre));
        assert!(CATEGORIAS.contains(&cat), "{cat}");
        assert!(
            !f.elementos.is_empty() && f.elementos.len() < MAX_PIEZAS,
            "{} {}",
            f.nombre,
            f.elementos.len()
        );
    }
    for cat in CATEGORIAS {
        assert!(
            c.iter()
                .any(|f| categoria_de(&f.nombre).is_some_and(|(k, _)| k == cat)),
            "{cat} vacia"
        );
    }
}

#[test]
fn todo_es_finito_y_esta_puesto_en_el_origen() {
    for f in catalogo() {
        let (x0, y0, x1, y1) = biblioteca::caja_de(&f.elementos).unwrap();
        assert!(
            x0.abs() < 0.01 && y0.abs() < 0.01,
            "{} ({x0}, {y0})",
            f.nombre
        );
        assert!(
            x1.is_finite() && y1.is_finite() && x1 > 20.0 && y1 > 20.0,
            "{}",
            f.nombre
        );
        for e in &f.elementos {
            assert!(
                e.x.is_finite() && e.y.is_finite() && e.ancho >= 0.0 && e.alto >= 0.0,
                "{}",
                f.nombre
            );
        }
    }
}

#[test]
fn viajan_en_el_formato_del_movil_sin_perder_piezas() {
    let c = catalogo();
    let leidas = biblioteca::desde_json(&biblioteca::a_json(&c));
    assert_eq!(leidas.len(), c.len());
    for (a, b) in c.iter().zip(&leidas) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.nombre, b.nombre);
        assert_eq!(a.elementos.len(), b.elementos.len(), "{}", a.nombre);
    }
}

#[test]
fn instalar_sustituye_las_de_serie_y_respeta_las_del_usuario() {
    let propia = FiguraGuardada {
        id: "abc".into(),
        nombre: "Mi flecha".into(),
        elementos: catalogo()[0].elementos.clone(),
    };
    let tapa = FiguraGuardada {
        id: "xyz".into(),
        nombre: "Storyboard · Claqueta".into(),
        elementos: propia.elementos.clone(),
    };
    let vieja = FiguraGuardada {
        id: format!("{PREFIJO_ID}ya-no-existe"),
        nombre: "Blocs · Vieja".into(),
        elementos: propia.elementos.clone(),
    };
    let mut lista = vec![propia.clone(), tapa.clone(), vieja];
    instalar_en(&mut lista);
    assert_eq!(lista[0], propia);
    assert_eq!(lista[1], tapa);
    assert!(!lista.iter().any(|f| f.id.ends_with("ya-no-existe")));
    assert_eq!(
        lista
            .iter()
            .filter(|f| f.nombre == "Storyboard · Claqueta")
            .count(),
        1
    );
    let n = lista.len();
    instalar_en(&mut lista);
    assert_eq!(lista.len(), n, "instalar dos veces no duplica");
}

#[test]
fn la_categoria_sale_del_nombre() {
    assert_eq!(
        categoria_de("Diario · Gratitud"),
        Some(("Diario", "Gratitud"))
    );
    assert_eq!(categoria_de("arbol"), None);
    assert_eq!(categoria_de(" · x"), None);
}

fn textos(f: &FiguraGuardada) -> Vec<String> {
    f.elementos
        .iter()
        .filter_map(|e| match &e.figura {
            Figura::Texto { texto, .. } => Some(texto.clone()),
            _ => None,
        })
        .collect()
}

fn alto(f: &FiguraGuardada) -> f32 {
    biblioteca::caja_de(&f.elementos).unwrap().3
}

#[test]
fn del_1_al_40_es_una_hoja_mas_larga_con_los_cuarenta_numeros() {
    let iv = Intervalo {
        desde: Some(1),
        hasta: Some(40),
        ..Default::default()
    };
    let hojas = alargar("lista-numerada", &iv).unwrap();
    assert_eq!(hojas.len(), 1);
    let t = textos(&hojas[0]);
    for i in 1..=40 {
        assert!(t.contains(&format!("{i:02}")), "falta el {i}");
    }
    assert!(!t.contains(&"41".to_string()));
    let de_serie = catalogo()
        .into_iter()
        .find(|f| f.id == "serie-lista-numerada")
        .unwrap();
    assert!(alto(&hojas[0]) > alto(&de_serie) + 200.0, "la hoja crece");
    assert!(hojas[0].nombre.ends_with("(1–40)"), "{}", hojas[0].nombre);
}

#[test]
fn de_10_en_10_son_cuatro_hojas_seguidas_con_su_pie() {
    let iv = Intervalo {
        desde: Some(1),
        hasta: Some(40),
        de_a: Some(10),
        ..Default::default()
    };
    for clave in [
        "lista-de-tareas",
        "cronograma",
        "metas",
        "storyboard-6",
        "receta",
        "asistencia",
        "lista-numerada",
    ] {
        let hojas = alargar(clave, &iv).unwrap();
        assert_eq!(hojas.len(), 4, "{clave}");
        let ultima = textos(&hojas[3]);
        assert!(
            ultima
                .iter()
                .any(|s| s.starts_with("HOJA 4 DE 4") && s.ends_with("31–40")),
            "{clave}: {ultima:?}"
        );
        let primera = textos(&hojas[0]);
        assert!(
            primera
                .iter()
                .any(|s| s == "10" || s == "Paso 10" || s == "10"),
            "{clave}: {primera:?}"
        );
        assert!(!primera.iter().any(|s| s == "11"), "{clave}");
        // Todas las hojas iguales de tamano: se ponen en fila.
        assert!((alto(&hojas[0]) - alto(&hojas[2])).abs() < 1.0, "{clave}");
    }
}

#[test]
fn las_horas_y_las_semanas_tambien_son_series() {
    let iv = Intervalo {
        desde: Some(5),
        hasta: Some(23),
        ..Default::default()
    };
    let t = textos(&alargar("planificador-diario", &iv).unwrap()[0]);
    assert!(t.contains(&"05".to_string()) && t.contains(&"23".to_string()));
    let iv = Intervalo {
        semanas: Some((13, 24)),
        ..Default::default()
    };
    let t = textos(&alargar("cronograma", &iv).unwrap()[0]);
    assert!(
        t.contains(&"S13".to_string())
            && t.contains(&"MES 4".to_string())
            && !t.contains(&"MES 1".to_string()),
        "{t:?}"
    );
    // Semanas de 26 en 26 de un año entero: dos hojas.
    let iv = Intervalo {
        semanas: Some((1, 52)),
        de_a: Some(26),
        ..Default::default()
    };
    assert_eq!(alargar("cronograma", &iv).unwrap().len(), 2);
}

#[test]
fn el_mes_de_verdad_pone_cada_dia_en_su_dia_de_la_semana() {
    use super::plantillas::{dia_de_la_semana, dias_del_mes};
    assert_eq!(
        dia_de_la_semana(2026, 10, 22),
        3,
        "el 22 de octubre de 2026 es jueves"
    );
    assert_eq!(
        dia_de_la_semana(2026, 11, 1),
        6,
        "el 1 de noviembre de 2026 es domingo"
    );
    assert_eq!(dias_del_mes(2028, 2), 29);
    assert_eq!(dias_del_mes(2100, 2), 28);
    let iv = Intervalo {
        mes: Some((2026, 11)),
        meses: Some(3),
        ..Default::default()
    };
    let hojas = alargar("planificador-mensual", &iv).unwrap();
    assert_eq!(hojas.len(), 3);
    let t = textos(&hojas[0]);
    assert!(
        t.contains(&"Noviembre".to_string())
            && t.contains(&"30".to_string())
            && !t.contains(&"31".to_string())
    );
    assert!(
        textos(&hojas[2]).contains(&"Enero".to_string()),
        "pasa de año"
    );
    let feb = alargar(
        "habitos",
        &Intervalo {
            mes: Some((2028, 2)),
            ..Default::default()
        },
    )
    .unwrap();
    let t = textos(&feb[0]);
    assert!(t.contains(&"29".to_string()) && !t.contains(&"30".to_string()));
}

#[test]
fn se_reconoce_la_plantilla_elegida_por_su_titulo() {
    for f in catalogo() {
        let clave = clave_de(&f).unwrap().to_string();
        if let Some(c) = cual_es(&f.elementos) {
            assert_eq!(c, clave, "{}", f.nombre);
        }
        if se_alarga(&clave) {
            assert_eq!(
                cual_es(&f.elementos).as_deref(),
                Some(clave.as_str()),
                "{}",
                f.nombre
            );
        }
    }
    // Y una ya alargada sigue siendo la misma.
    let iv = Intervalo {
        desde: Some(1),
        hasta: Some(12),
        ..Default::default()
    };
    let h = alargar("storyboard-4", &iv).unwrap();
    assert_eq!(cual_es(&h[0].elementos).as_deref(), Some("storyboard-4"));
    assert_eq!(
        cual_es(
            &catalogo()
                .into_iter()
                .find(|f| f.id == "serie-claqueta")
                .unwrap()
                .elementos
        ),
        None
    );
}

#[test]
fn lo_que_no_se_puede_se_dice_en_palabras() {
    let iv = Intervalo {
        desde: Some(1),
        hasta: Some(40),
        ..Default::default()
    };
    let e = alargar("curso", &iv).unwrap_err();
    assert!(
        e.contains("no lleva números") && e.contains("Cronograma"),
        "{e}"
    );
    let e = alargar(
        "lista-de-tareas",
        &Intervalo {
            desde: Some(9),
            hasta: Some(3),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(e.contains("al revés"), "{e}");
    let e = alargar(
        "planificador-diario",
        &Intervalo {
            desde: Some(6),
            hasta: Some(30),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(e.contains("como mucho"), "{e}");
    let e = alargar(
        "lista-numerada",
        &Intervalo {
            desde: Some(1),
            hasta: Some(1000),
            de_a: Some(1),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(e.contains("hojas"), "{e}");
}
