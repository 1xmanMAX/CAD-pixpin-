use super::*;

fn figura(id: &str, nombre: &str) -> FiguraGuardada {
    FiguraGuardada {
        id: id.into(),
        nombre: nombre.into(),
        elementos: vec![pixpin_motor2d::Elemento::default()],
    }
}

/// Las de serie y dos propias: una sin categoria y otra con una nueva.
fn biblioteca() -> Vec<FiguraGuardada> {
    let mut v = vec![figura("a", "Mi flecha"), figura("b", "Obra · Grúa torre")];
    serie::instalar_en(&mut v);
    v
}

fn estado() -> Estado {
    let mut e = Estado::nuevo(
        biblioteca(),
        vec![(1, "Gráfica".into())],
        false,
        "Mis figuras".into(),
    );
    e.rehacer_lista(900.0, 1.0);
    e
}

#[test]
fn las_categorias_van_en_su_orden_con_las_nuevas_y_mis_figuras_al_final() {
    let e = estado();
    assert_eq!(
        &e.cats[..serie::CATEGORIAS.len()],
        serie::CATEGORIAS.map(String::from).as_slice()
    );
    assert_eq!(e.cats[serie::CATEGORIAS.len()], "Obra");
    assert_eq!(e.cats.last().unwrap(), "Mis figuras");
    // Todas: cada figura una vez.
    assert_eq!(e.lista.len(), e.figuras.len());
}

#[test]
fn se_busca_sin_tildes_y_se_filtra_por_categoria() {
    let mut e = estado();
    e.consulta.poner("habitos");
    e.rehacer_lista(900.0, 1.0);
    assert_eq!(
        e.lista.len(),
        1,
        "{:?}",
        e.lista
            .iter()
            .map(|i| &e.figuras[*i].nombre)
            .collect::<Vec<_>>()
    );
    assert!(e.figuras[e.lista[0]].nombre.contains("Hábitos"));
    e.consulta.poner("");
    let storyboard = e.cats.iter().position(|c| c == "Storyboard").unwrap();
    e.cat = Some(storyboard);
    e.rehacer_lista(900.0, 1.0);
    assert!(
        e.lista
            .iter()
            .all(|i| e.figuras[*i].nombre.starts_with("Storyboard · "))
    );
    assert!(
        e.elegida.is_some_and(|i| e.lista.contains(&i)),
        "elige la primera de las que se ven"
    );
}

#[test]
fn en_todas_cada_categoria_lleva_su_titulo_y_las_celdas_no_se_pisan() {
    let e = estado();
    let titulos: Vec<&str> = e
        .disp
        .piezas
        .iter()
        .filter_map(|(_, p)| match p {
            Pieza::Titulo(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(titulos.first(), Some(&"Planificación"));
    assert!(titulos.contains(&"Mis figuras") && titulos.contains(&"Obra"));
    assert!(e.disp.columnas >= 3, "{}", e.disp.columnas);
    let celdas: Vec<RectF> = e
        .disp
        .piezas
        .iter()
        .filter(|(_, p)| matches!(p, Pieza::Celda(_)))
        .map(|(r, _)| *r)
        .collect();
    for (k, a) in celdas.iter().enumerate() {
        assert!(a.x >= 0.0 && a.x + a.ancho <= 900.0 + 0.5, "{a:?}");
        for b in &celdas[k + 1..] {
            let pisa = a.x < b.x + b.ancho
                && b.x < a.x + a.ancho
                && a.y < b.y + b.alto
                && b.y < a.y + a.alto;
            assert!(!pisa, "{a:?} {b:?}");
        }
    }
}

#[test]
fn el_intervalo_sale_de_los_campos() {
    let campos = |v: &[(Dato, &str)]| {
        v.iter()
            .map(|(c, s)| (*c, s.to_string()))
            .collect::<HashMap<_, _>>()
    };
    assert_eq!(intervalo_de(&campos(&[])), Ok(None));
    assert_eq!(
        intervalo_de(&campos(&[(Dato::Desde, " "), (Dato::DeA, "")])),
        Ok(None)
    );
    assert_eq!(
        intervalo_de(&campos(&[
            (Dato::Desde, "1"),
            (Dato::Hasta, "40"),
            (Dato::DeA, "10")
        ]))
        .unwrap(),
        Some(Intervalo {
            desde: Some(1),
            hasta: Some(40),
            de_a: Some(10),
            ..Default::default()
        })
    );
    let iv = intervalo_de(&campos(&[
        (Dato::Mes, "noviembre 2026"),
        (Dato::Meses, "3"),
    ]))
    .unwrap()
    .unwrap();
    assert_eq!((iv.mes, iv.meses), (Some((2026, 11)), Some(3)));
    assert!(
        intervalo_de(&campos(&[(Dato::Hasta, "cuarenta")]))
            .unwrap_err()
            .contains("cuarenta")
    );
}

#[test]
fn insertar_una_plantilla_con_intervalo_da_sus_hojas_y_sin_el_la_de_serie() {
    let mut e = estado();
    let i = e
        .figuras
        .iter()
        .position(|f| f.id == "serie-lista-de-tareas")
        .unwrap();
    e.elegir(Some(i));
    e.insertar();
    assert_eq!(e.pedido, Some(Pedido::Estampar(i)));
    e.pedido = None;
    for (c, s) in [(Dato::Desde, "1"), (Dato::Hasta, "40"), (Dato::DeA, "10")] {
        e.campos.entry(c).or_default().poner(s);
    }
    let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
    e.rehacer_previa(&t);
    assert_eq!(e.previa, Some(Ok("4 hojas".to_string())));
    e.insertar();
    assert!(matches!(&e.pedido, Some(Pedido::Hojas(h)) if h.len() == 4));
    // Una que no se alarga no lleva intervalo: se estampa tal cual.
    let mut e = estado();
    let j = e
        .figuras
        .iter()
        .position(|f| f.id == "serie-claqueta")
        .unwrap();
    e.elegir(Some(j));
    assert!(e.alargable().is_none());
    e.insertar();
    assert_eq!(e.pedido, Some(Pedido::Estampar(j)));
}

#[test]
fn quitar_pide_confirmacion_y_renombrar_cambia_la_biblioteca() {
    let mut e = estado();
    let n = e.figuras.len();
    let i = e
        .figuras
        .iter()
        .position(|f| f.nombre == "Mi flecha")
        .unwrap();
    e.elegir(Some(i));
    e.campos
        .get_mut(&Dato::Nombre)
        .unwrap()
        .poner("Obra · Flecha de obra");
    e.renombrar();
    assert_eq!(e.figuras[i].nombre, "Obra · Flecha de obra");
    assert!(e.cambiada);
    e.quitar();
    assert_eq!(e.figuras.len(), n, "el primer clic solo pregunta");
    e.quitar();
    assert_eq!(e.figuras.len(), n - 1);
    assert!(
        !e.figuras
            .iter()
            .any(|f| f.nombre == "Obra · Flecha de obra")
    );
}

#[test]
fn las_flechas_mueven_la_eleccion_por_la_rejilla() {
    let mut e = estado();
    let primera = e.lista[0];
    e.elegir(Some(primera));
    e.mover_eleccion(1);
    assert_eq!(e.elegida, Some(e.lista[1]));
    e.mover_eleccion(-5);
    assert_eq!(e.elegida, Some(primera), "no se sale por arriba");
}

/// `PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin muestra_de_la_ventana_de_figuras -- --ignored`
#[test]
#[ignore = "necesita GPU; deja un PNG para mirarlo"]
fn muestra_de_la_ventana_de_figuras() {
    let mut e = Estado::nuevo(
        biblioteca(),
        vec![
            (1, "Gráfica de una función…".into()),
            (2, "Tabla en blanco".into()),
            (3, "Plano de funciones".into()),
        ],
        true,
        "Mis figuras".into(),
    );
    // Como en el portatil del usuario: 1440 x 810 logicos al 120 %.
    let (w, h, s) = (1728.0, 972.0, 1.2);
    e.rehacer_lista(w - (ANCHO_LATERAL + ANCHO_PANEL) * s, s);
    let i = e
        .figuras
        .iter()
        .position(|f| f.id == "serie-cronograma")
        .unwrap();
    e.elegir(Some(i));
    for (c, v) in [(Dato::Desde, "1"), (Dato::Hasta, "40"), (Dato::DeA, "10")] {
        e.campos.entry(c).or_default().poner(v);
    }
    let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
    e.rehacer_previa(&t);
    e.botones.raton = (700.0, 300.0);
    crate::ventanita::muestra("ventana-figuras", w as u32, h as u32, |p, motor| {
        subir_miniaturas(&mut e, motor, LADO_MINI as u32);
        pintar_todo(&mut e, p, w, h, s, &t);
    });
}
