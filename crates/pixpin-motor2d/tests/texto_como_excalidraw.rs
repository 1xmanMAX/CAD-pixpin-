//! **El texto como en Excalidraw** (pedido del usuario, 2026-09-26): se
//! escribe donde se hace clic, sin caja; la caja sale al hacerle clic al
//! texto y mide exactamente lo escrito.
//!
//! El motor no tiene DirectWrite: aqui se le pone un medidor de mentira (10
//! unidades por letra) con `texto::con_medidor`, que es por donde entra el de
//! verdad del anfitrion. Asi se ve que la caja sale DEL MEDIDOR y no de la
//! cuenta a ojo.

use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::estilo::{CambioForma, NivelGrosor};
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::pintado::{Orden, ordenes};
use pixpin_motor2d::texto::{self, EstiloDeTexto, TeclaTexto};
use pixpin_motor2d::vector::Punto2;

fn medidor(t: &str, tam: f32, _familia: &str, _e: EstiloDeTexto) -> Option<(f32, f32)> {
    let largo = t.split('\n').map(|r| r.chars().count()).max().unwrap_or(0) as f32;
    Some((largo * 10.0, t.split('\n').count() as f32 * tam))
}

fn clic(g: &mut Gesto, e: &mut Escena, x: f32, y: f32) {
    g.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(x, y),
            shift: false,
            alt: false,
            presion: None,
        },
        e,
        1.0,
    );
    g.evento(
        EventoGesto::Soltar {
            p: Punto2::nuevo(x, y),
        },
        e,
        1.0,
    );
}

fn escribir(g: &mut Gesto, e: &mut Escena, s: &str) {
    for c in s.chars() {
        g.escribir(c, e);
    }
}

/// Un texto nuevo escrito con la herramienta de texto; devuelve su id.
fn texto_nuevo(g: &mut Gesto, e: &mut Escena, s: &str) -> u64 {
    g.tomar_herramienta(Herramienta::Texto);
    clic(g, e, 100.0, 50.0);
    escribir(g, e, s);
    g.texto_en_curso().expect("se esta escribiendo").0
}

#[test]
fn se_escribe_donde_se_hizo_clic_sin_marco_ni_tiradores_y_con_cursor() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let id = texto_nuevo(&mut g, &mut escena, "hola");
        assert!(!g.marco_visible(), "mientras se escribe no hay marco");
        assert!(
            g.tiradores(&escena, 1.0).is_none(),
            "ni tiradores que agarren"
        );
        // Elegido por dentro, para que el panel cambie la letra de lo que se
        // escribe.
        assert!(g.seleccion.contiene(id));
        // La barra, detras de «hola»: 4 letras x 10.
        let Some(Orden::Polilinea { puntos, .. }) = g.cursor_de_texto(&escena, 1.0) else {
            panic!("sin cursor");
        };
        assert_eq!(puntos[0].x, 140.0);
        assert!(puntos[0].y >= 50.0 && puntos[1].y <= 50.0 + 25.0);
    });
}

#[test]
fn la_caja_mide_lo_escrito_con_el_medidor_y_crece_al_escribir() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let id = texto_nuevo(&mut g, &mut escena, "hola");
        let e = escena.buscar(id).unwrap();
        assert_eq!(
            (e.ancho, e.alto),
            (40.0, 25.0),
            "lo escrito, sin letra de mas"
        );
        g.tecla_de_texto(TeclaTexto::Entrar, &mut escena);
        escribir(&mut g, &mut escena, "mundo!");
        let e = escena.buscar(id).unwrap();
        assert_eq!((e.ancho, e.alto), (60.0, 50.0));
        // La barra baja al segundo renglon y va detras de «mundo!».
        let Some(Orden::Polilinea { puntos, .. }) = g.cursor_de_texto(&escena, 1.0) else {
            panic!("sin cursor");
        };
        assert_eq!(puntos[0].x, 160.0);
        assert!(puntos[0].y >= 75.0, "{:?}", puntos);
        // Caso negativo: con el cursor al principio del renglon, la barra
        // esta en el borde, no un espacio mas alla.
        g.tecla_de_texto(TeclaTexto::Inicio, &mut escena);
        let Some(Orden::Polilinea { puntos, .. }) = g.cursor_de_texto(&escena, 1.0) else {
            panic!("sin cursor");
        };
        assert_eq!(puntos[0].x, 100.0);
    });
}

#[test]
fn al_pulsar_en_otro_sitio_el_texto_queda_sin_elegir() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let id = texto_nuevo(&mut g, &mut escena, "hola");
        // Otro clic con la de texto, lejos: cierra el primero y abre otro.
        clic(&mut g, &mut escena, 400.0, 400.0);
        assert!(!g.seleccion.contiene(id), "el de antes no se queda elegido");
        // Y con la mano en otro sitio vacio, tampoco queda nada escrito.
        escribir(&mut g, &mut escena, "x");
        g.tomar_herramienta(Herramienta::Mano);
        clic(&mut g, &mut escena, 900.0, 900.0);
        assert!(!g.esta_escribiendo(), "un clic de la mano cierra el texto");
        assert!(g.seleccion.ids().is_empty());
        assert!(escena.buscar(id).is_some_and(|e| !e.borrado));
    });
}

#[test]
fn con_escape_queda_elegido_y_el_marco_es_la_caja_exacta_del_texto() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let id = texto_nuevo(&mut g, &mut escena, "hola");
        assert!(g.cerrar_texto_con_teclado(&mut escena));
        assert!(g.marco_visible());
        assert_eq!(g.seleccion.ids(), &[id]);
        let caja = g.seleccion.caja(&escena).expect("caja");
        assert_eq!(caja, (100.0, 50.0, 140.0, 75.0));
        assert!(g.tiradores(&escena, 1.0).is_some());
    });
}

#[test]
fn un_texto_que_quedo_en_blanco_se_borra_al_cerrar_y_no_queda_elegido() {
    // Caso negativo: solo espacios es tan invisible como vacio.
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let id = texto_nuevo(&mut g, &mut escena, "   ");
        assert!(g.cerrar_texto_con_teclado(&mut escena));
        assert!(escena.buscar(id).is_none_or(|e| e.borrado));
        assert!(g.seleccion.ids().is_empty());
    });
}

fn texto_viejo(ancho: f32) -> Elemento {
    Elemento {
        figura: Figura::Texto {
            texto: "hola".into(),
            tam: 20.0,
            familia: "Excalifont".into(),
        },
        x: 0.0,
        y: 0.0,
        ancho,
        alto: 26.0,
        ..Default::default()
    }
}

#[test]
fn al_hacer_clic_a_un_texto_de_caja_vieja_el_marco_se_ajusta_a_lo_escrito() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        // La caja de la cuenta a ojo de antes: un caracter de mas.
        let id = escena.anadir(texto_viejo(62.0));
        let mut g = Gesto::nuevo();
        g.tomar_herramienta(Herramienta::Mano);
        clic(&mut g, &mut escena, 10.0, 10.0);
        assert_eq!(g.seleccion.ids(), &[id]);
        let e = escena.buscar(id).unwrap();
        assert_eq!((e.ancho, e.alto), (40.0, 25.0));
    });
}

#[test]
fn sin_medidor_elegir_un_texto_no_le_toca_la_caja() {
    // Caso negativo: medir a ojo no es medir; «corregir» con eso cambiaria
    // los textos del movil al abrirlos en un anfitrion sin DirectWrite.
    let mut escena = Escena::nueva();
    let id = escena.anadir(texto_viejo(62.0));
    let version = escena.buscar(id).unwrap().version;
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Mano);
    clic(&mut g, &mut escena, 10.0, 10.0);
    let e = escena.buscar(id).unwrap();
    assert_eq!(e.ancho, 62.0);
    assert_eq!(e.version, version);
}

#[test]
fn un_texto_suelto_no_parte_renglones_y_uno_de_figura_si() {
    let mut suelto = texto_viejo(40.0);
    suelto.extras.negrita = true;
    let o = ordenes(&suelto);
    let Some(Orden::Texto {
        ancho_max, negrita, ..
    }) = o.iter().find(|o| matches!(o, Orden::Texto { .. }))
    else {
        panic!("sin texto");
    };
    assert!(*ancho_max >= texto::SIN_PARTIR);
    assert!(*negrita, "la negrita viaja en la orden");
    let mut de_figura = texto_viejo(40.0);
    de_figura.extras.contenedor = Some("caja".into());
    let o = ordenes(&de_figura);
    let Some(Orden::Texto { ancho_max, .. }) = o.iter().find(|o| matches!(o, Orden::Texto { .. }))
    else {
        panic!("sin texto");
    };
    assert_eq!(*ancho_max, 40.0);
}

#[test]
fn cambiar_la_letra_de_un_texto_le_remide_la_caja() {
    fn ancho_por_familia(
        t: &str,
        _tam: f32,
        familia: &str,
        _e: EstiloDeTexto,
    ) -> Option<(f32, f32)> {
        let k = if familia == "Nunito" { 12.0 } else { 10.0 };
        Some((t.chars().count() as f32 * k, 25.0))
    }
    texto::con_medidor(ancho_por_familia, || {
        let mut escena = Escena::nueva();
        let id = escena.anadir(texto_viejo(40.0));
        let mut g = Gesto::nuevo();
        g.seleccion.poner(id);
        pixpin_motor2d::estilo::aplicar_forma(
            &mut escena,
            &g.seleccion,
            CambioForma::Familia(texto::FUENTE_NUNITO),
        );
        let e = escena.buscar(id).unwrap();
        assert_eq!(e.ancho, 48.0);
        assert!(matches!(&e.figura, Figura::Texto { familia, .. } if familia == "Nunito"));
    });
}

#[test]
fn un_texto_nuevo_nace_en_excalifont_como_en_excalidraw() {
    let mut escena = Escena::nueva();
    let mut g = Gesto::nuevo();
    let id = texto_nuevo(&mut g, &mut escena, "a");
    assert!(matches!(
        &escena.buscar(id).unwrap().figura,
        Figura::Texto { familia, .. } if familia == "Excalifont"
    ));
}

// -------------------------------------------------------------------------
// El resaltador
// -------------------------------------------------------------------------

fn resaltador(g: &mut Gesto, e: &mut Escena) -> u64 {
    g.tomar_herramienta(Herramienta::Resaltador);
    g.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(0.0, 0.0),
            shift: false,
            alt: false,
            presion: None,
        },
        e,
        1.0,
    );
    let id = g.trazo_en_curso().expect("trazando");
    for i in 1..40 {
        g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(i as f32 * 5.0, 0.0),
                shift: false,
                alt: false,
                presion: None,
            },
            e,
            1.0,
        );
    }
    id
}

/// Lo alto de la mancha de una raya horizontal: su grosor de verdad.
fn alto_de(o: &[Orden]) -> f32 {
    let c = o
        .iter()
        .find_map(|o| match o {
            Orden::Tinta { contorno, .. } => Some(contorno),
            _ => None,
        })
        .expect("sin tinta");
    let (a, b) = c
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    b - a
}

#[test]
fn el_resaltador_medio_vuelve_a_su_grosor_de_siempre_y_fino_y_grueso_lo_rodean() {
    let mut escena = Escena::nueva();
    let mut g = Gesto::nuevo();
    let id = resaltador(&mut g, &mut escena);
    // El del movil (27-sep): el lapiz x5 (`ENGORDE_DEL_MARCADOR`) sobre la
    // mitad del grosor de las formas: Medio 2 / 2 x 5 = 5, que pinta unos
    // 30 de ancho. El 3 de D45 (9 de raya) era un tercio: «muy delgado».
    assert_eq!(
        escena.buscar(id).unwrap().grosor,
        5.0,
        "Medio: el 5 del movil"
    );
    let medio = alto_de(&ordenes(escena.buscar(id).unwrap()));
    assert!(medio > 25.0 && medio < 42.0, "raya de {medio}");
    for (nivel, esperado) in [(NivelGrosor::Fino, 2.5), (NivelGrosor::Grueso, 10.0)] {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.estilo.grosor = nivel;
        let id = resaltador(&mut g, &mut escena);
        assert_eq!(escena.buscar(id).unwrap().grosor, esperado);
    }
    // Caso negativo: el grosor de las figuras no cambio con esto.
    assert_eq!(NivelGrosor::Medio.de_forma(), 2.0);
}

#[test]
fn el_resaltador_no_adelgaza_al_soltar() {
    let mut escena = Escena::nueva();
    let mut g = Gesto::nuevo();
    let id = resaltador(&mut g, &mut escena);
    let en_vivo = alto_de(&ordenes(escena.buscar(id).unwrap()));
    g.evento(
        EventoGesto::Soltar {
            p: Punto2::nuevo(195.0, 0.0),
        },
        &mut escena,
        1.0,
    );
    let e = escena.buscar(id).unwrap();
    assert_eq!(e.grosor, 5.0);
    let soltado = alto_de(&ordenes(e));
    assert!((en_vivo - soltado).abs() < 0.2, "{en_vivo} -> {soltado}");
    // La punta que se predice (las ultimas muestras mas una) mide lo mismo.
    let Figura::Resaltador { puntos } = &e.figura else {
        panic!("no es resaltador");
    };
    let mut cola = puntos[puntos.len() - 6..].to_vec();
    cola.push(Punto2::nuevo(205.0, 0.0));
    let punta = pixpin_motor2d::tinta::contorno_de_resaltador(&cola, e.grosor);
    let (a, b) = punta
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    // Con la tinta del movil la punta sola arranca con su propia presion,
    // asi que no mide exacto lo mismo; si del mismo orden (como el lapiz).
    assert!(
        (b - a) > soltado * 0.5 && (b - a) < soltado * 1.5,
        "punta {} y trazo {soltado}",
        b - a
    );
}
