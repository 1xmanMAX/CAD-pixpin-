//! **Las cuatro herramientas de construir, cableadas a la ventana del editor.**
//!
//! El bote de relleno, recortar, extender y el punto etiquetado. La geometria
//! esta en el motor (`regiones`, `recorte`, `puntos_etiquetados`, `angulos`);
//! aqui solo esta lo que el motor no puede saber: **en que orden queda la
//! escena, que cuenta como un paso de deshacer y que hay que repintar**.
//!
//! # Por que son distintas de las demas
//!
//! Todas las herramientas del editor nacen de un arrastre: se pulsa, crece una
//! figura y al soltar se queda. Estas cuatro no. Tres de ellas
//! —bote, recortar, extender— **no dibujan nada: miran lo que ya hay y lo
//! cambian**, y por eso `Herramienta::deja_rastro` dice que no y el gesto las
//! deja pasar sin tocar la escena. La cuarta, el punto, si nace de un clic,
//! pero nace **incompleto**: sin letra y en el sitio crudo del cursor, y aqui
//! se remata.
//!
//! De ahi que todo esto cuelgue del `Pulsar` y no del `Soltar`.
//!
//! # El contrato con la ventana
//!
//! Una sola puerta, [`al_pulsar`], que devuelve si la escena cambio. La ventana
//! la llama justo despues de `gesto.evento(...)` y, si dice que si, repinta
//! entero y marca el documento como sucio. Nada de aqui necesita la ventana:
//! ni el motor de dibujo, ni la camara, ni las imagenes — solo la escena, el
//! gesto y el zoom, que es lo que hace que se pueda comprobar sin pantalla.

use pixpin_motor2d::elemento::Elemento;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{Gesto, Herramienta};
use pixpin_motor2d::nudos;
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::puntos_etiquetados::{self, RADIO_DE_AGARRE, RADIO_PARA_PUNTOS, SerieDePunto};
use pixpin_motor2d::recorte::{self, ALCANCE_EXTENDER};
use pixpin_motor2d::regiones::{self, AjustesRelleno};
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{ColorRgba, angulos};

/// **La puerta unica.** `true` si la escena cambio y hay que repintar.
///
/// `zoom` es el de la camara: los radios de agarre se dan en pixeles de
/// pantalla y aqui se trabaja en coordenadas del documento, igual que en el
/// iman. Sin eso, muy acercado se engancharia a medio dibujo y muy alejado a
/// nada.
pub fn al_pulsar(escena: &mut Escena, gesto: &Gesto, p: Punto2, zoom: f32) -> bool {
    match gesto.herramienta {
        Herramienta::Relleno => bote(escena, gesto, p),
        Herramienta::Recortar => recortar(escena, p, zoom),
        Herramienta::Extender => extender(escena, p, zoom),
        // Soldar vertices: clava (o desclava) en el cruce o la junta que se
        // pulse. La geometria y el deshacer estan en `nudos::soldar`.
        Herramienta::Nudo => nudos::soldar(escena, p, nudos::RADIO_DEL_CLAVO / zoom.max(0.0001)),
        // El punto lo crea el gesto —deja rastro— y llega sin letra y en el
        // sitio crudo del cursor: ver [`rematar_punto`].
        Herramienta::Punto => match gesto.elemento_en_curso() {
            Some((id, _)) => rematar_punto(escena, id, p, zoom, gesto.serie_de_punto),
            None => false,
        },
        _ => false,
    }
}

/// El elemento del que copian el estilo las figuras que nacen aqui.
///
/// Es el mismo «actual» del panel lateral con el que nace cualquier figura del
/// editor: el bote no puede inventarse un color distinto del que esta puesto.
fn plantilla(gesto: &Gesto) -> Elemento {
    Elemento {
        trazo: gesto.estilo.trazo,
        relleno: gesto.estilo.relleno,
        estilo_relleno: gesto.estilo.estilo_relleno,
        grosor: gesto.estilo.grosor.de_forma(),
        estilo: gesto.estilo.estilo,
        rugosidad: gesto.estilo.rugosidad,
        opacidad: gesto.estilo.opacidad,
        // **El bote rellena con lo que se tenia en la mano** (v0.76-v0.77 del
        // movil, `rellenar`): el grafito es una herramienta y no un material
        // del panel, asi que el bote no tiene donde elegirlo. Si lo ultimo
        // con lo que se pinto fue el grafito, el relleno sale de grafito.
        material: if gesto.grafito_en_la_mano {
            pixpin_motor2d::tinta::MaterialTinta::Cuadritos
        } else {
            gesto.estilo.material
        },
        // Con el modo guia puesto, el relleno tambien es guia; y el de
        // grafito, de la dureza elegida (`nuevaRegion` del movil).
        extras: pixpin_motor2d::elemento::Extras {
            referencia: gesto.modo_guia(),
            dureza: if gesto.grafito_en_la_mano {
                gesto.estilo.dureza.para_guardar()
            } else {
                None
            },
            ..Default::default()
        },
        ..Default::default()
    }
}

/// **El bote de pintura.**
///
/// Tres cosas pasan en un solo paso de deshacer, y las tres tienen que ir
/// juntas o insistir con otro color deja el dibujo peor que antes:
///
/// 1. se quita el relleno que ya hubiera en ese hueco —un bote no apila capas
///    de pintura—;
/// 2. nace el nuevo;
/// 3. y se baja **por debajo de la primera pared que se cruza en su camino**,
///    que es lo unico de esta herramienta que no es geometria sino orden de
///    pintado. Encima taparia por dentro los trazos que forman el hueco y las
///    lineas se verian mas finas que sus vecinas; al fondo del todo se
///    escondería detras de la foto sobre la que se anota.
fn bote(escena: &mut Escena, gesto: &Gesto, p: Punto2) -> bool {
    let elementos: Vec<Elemento> = escena.elementos.clone();
    // **El bote pinta con el color que hay puesto** (v0.76 del movil: el
    // trazo, que es el mando que el panel ensena con el bote en la mano; ver
    // `propiedades::de_herramienta`). Antes pintaba con el fondo del pincel,
    // que con el bote puesto no se podia elegir.
    let color = gesto.estilo.trazo;
    // **Dentro de una sola figura cerrada, el relleno es el suyo, exacto**
    // (F3, v0.67): su propio fondo, con su forma de verdad. De grafito no:
    // saldria de la tinta lisa de la figura; va una mancha aparte.
    if !gesto.grafito_en_la_mano
        && let Some(id) = regiones::figura_que_se_rellena_sola(&elementos, p)
    {
        escena.abrir_paso();
        // Una mancha de rejilla que hubiera ahi de antes se va: no se apilan
        // pinturas.
        for r in regiones::rellenos_en(&elementos, p) {
            escena.borrar_apuntando(r);
        }
        escena.apuntar_edicion(id);
        if let Some(e) = escena.buscar_mut(id) {
            e.relleno = Some(color);
            e.estilo_relleno = gesto.estilo.estilo_relleno;
            e.tocar();
        }
        escena.cerrar_paso();
        return true;
    }
    // `None` no es un fallo: es «este recinto esta abierto». Se deja la escena
    // exactamente como estaba, que es la unica respuesta honesta.
    let Some(region) = regiones::region_en(&elementos, p, &AjustesRelleno::default()) else {
        return false;
    };
    let mut molde = plantilla(gesto);
    molde.relleno = Some(color);
    let nuevo = regiones::nueva_region(region, &molde);

    escena.abrir_paso();
    for id in regiones::rellenos_en(&elementos, p) {
        escena.borrar_apuntando(id);
    }
    // El sitio se calcula ANTES de meterlo: con el ya dentro, «la primera
    // pared» podria ser el propio relleno recien puesto.
    let sitio = regiones::sitio_del_relleno(&escena.elementos, &nuevo);
    let id = escena.anadir(nuevo);
    if sitio < escena.elementos.len() - 1 {
        escena.apuntar_reordenamiento();
        if let Some(i) = escena.elementos.iter().position(|e| e.id == id) {
            let e = escena.elementos.remove(i);
            escena.elementos.insert(sitio, e);
        }
    }
    escena.cerrar_paso();
    true
}

/// **Recortar**: quita el trozo de raya que se toca, hasta donde la cruzan las
/// demas.
///
/// Si el trozo estaba en medio, la raya se parte en dos y la segunda mitad es
/// un elemento nuevo — no la de antes: compartir el id las convertiria en la
/// misma raya dos veces, y borrar una borraria la otra.
fn recortar(escena: &mut Escena, p: Punto2, zoom: f32) -> bool {
    let radio = RADIO_DE_AGARRE / zoom.max(0.0001);
    let Some(id) = mas_cercano(escena, p, radio, recorte::se_recorta) else {
        return false;
    };
    // La geometria y el paso de deshacer viven en el motor: la misma
    // operacion que usa el asistente (`pixpin_ia::edicion`).
    recorte::recortar_en_escena(escena, id, p, None)
}

/// **Extender**: estira la punta que se queda corta hasta lo primero que topa.
///
/// Si no hay nada en su camino no pasa nada, y eso es deliberado: estirar hasta
/// el infinito no es extender, es tirar una raya al vacio.
fn extender(escena: &mut Escena, p: Punto2, zoom: f32) -> bool {
    let radio = RADIO_DE_AGARRE / zoom.max(0.0001);
    let Some(id) = mas_cercano(escena, p, radio, recorte::es_lineal) else {
        return false;
    };
    recorte::extender_en_escena(escena, id, p, None, ALCANCE_EXTENDER)
}

/// **Remata el punto que acaba de nacer**: lo pega a un sitio notable y le pone
/// su letra.
///
/// Si no hay sitio notable cerca, **se quita**. Un punto de geometria no va
/// donde caiga el cursor: va donde hay algo que nombrar —un cruce, el final de
/// una recta, el centro de una circunferencia—. Poner uno «mas o menos» en un
/// vertice no es un descuido estetico: es que el punto deja de ser ese punto, y
/// todo lo que se deduzca de el a partir de ahi es falso.
fn rematar_punto(escena: &mut Escena, id: u64, p: Punto2, zoom: f32, serie: SerieDePunto) -> bool {
    let radio = RADIO_PARA_PUNTOS / zoom.max(0.0001);
    // Sin el recien nacido: si contara, el punto se engancharia a si mismo.
    let otros: Vec<Elemento> = escena
        .elementos
        .iter()
        .filter(|e| e.id != id)
        .cloned()
        .collect();
    let Some(donde) = puntos_etiquetados::sitio_para_punto(&otros, p, radio) else {
        escena.borrar_apuntando(id);
        return true;
    };
    let rematado = puntos_etiquetados::nuevo_punto(
        donde,
        &otros,
        // La que se eligio en el panel con el punto en la mano (A B C, a b c
        // o 1 2 3); de fabrica, la de los vertices de toda la vida.
        serie,
        &plantilla_del_punto(escena, id),
    );
    if let Some(e) = escena.buscar_mut(id) {
        let suyo = e.id;
        *e = rematado;
        e.id = suyo;
    }
    true
}

/// El punto recien nacido sirve de plantilla de si mismo: ya trae el color, el
/// grosor y la semilla que le puso el gesto.
fn plantilla_del_punto(escena: &Escena, id: u64) -> Elemento {
    escena.buscar(id).cloned().unwrap_or_default()
}

/// El elemento tocable mas cercano a `p` que cumple `vale`.
///
/// Se mira **el recorrido y no la caja**: la caja de una diagonal larga ocupa
/// media pantalla, y con ella recortar cogeria la raya de al lado.
fn mas_cercano(escena: &Escena, p: Punto2, radio: f32, vale: fn(&Elemento) -> bool) -> Option<u64> {
    let mut mejor: Option<(f32, u64)> = None;
    for e in escena.visibles() {
        if e.bloqueado || !vale(e) {
            continue;
        }
        let mut d = f32::MAX;
        for (a, b) in pixpin_motor2d::segmentos_de(e, pixpin_motor2d::PASO_PERIMETRO) {
            d = d.min(pixpin_motor2d::distancia_a_segmento(p, a, b));
        }
        if d <= radio && mejor.is_none_or(|(md, _)| d < md) {
            mejor = Some((d, e.id));
        }
    }
    mejor.map(|(_, id)| id)
}

/// **Los angulos que se ven mientras se mueve algo**, listos para pintar.
///
/// No es un elemento y no se guarda: aparece al empezar el gesto y se va al
/// soltar, como la guia de un nivel de burbuja. La ventana lo encadena detras
/// de las ordenes de la escena, igual que hace con la pista del iman, y cuando
/// el gesto termina deja de llamar aqui y el rotulo desaparece solo.
pub fn angulos_en_vivo(escena: &Escena, gesto: &Gesto, zoom: f32) -> Vec<Orden> {
    if gesto.en_reposo() {
        return Vec::new();
    }
    let moviendose: Vec<u64> = match gesto.elemento_en_curso() {
        Some((id, _)) => vec![id],
        None => gesto.seleccion.ids().to_vec(),
    };
    let angulos = angulos::angulos_internos(&escena.elementos, &moviendose, angulos::JUNTA);
    angulos::ordenes_de_angulos(&angulos, zoom, COLOR_DEL_ROTULO)
}

/// El azul de las guias del editor: el mismo con el que se pinta la pista del
/// iman, para que se lea como ayuda y no como dibujo.
const COLOR_DEL_ROTULO: ColorRgba = ColorRgba {
    r: 0.20,
    g: 0.45,
    b: 0.90,
    a: 0.95,
};

/// ¿Hay que llamar a [`angulos_en_vivo`] en este fotograma?
///
/// La pregunta barata, para no montar los angulos de un plano entero en cada
/// movimiento del raton cuando no se esta moviendo nada que forme esquina.
pub fn hay_angulos_que_ensenar(gesto: &Gesto) -> bool {
    !gesto.en_reposo() && (gesto.elemento_en_curso().is_some() || !gesto.seleccion.ids().is_empty())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::Figura;

    fn raya(a: (f32, f32), b: (f32, f32)) -> Elemento {
        Elemento {
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
            },
            x: a.0.min(b.0),
            y: a.1.min(b.1),
            ancho: (b.0 - a.0).abs(),
            alto: (b.1 - a.1).abs(),
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 2.0,
            ..Default::default()
        }
    }

    /// Un cuadrado de 200x200 hecho con cuatro rayas sueltas, mas una foto al
    /// fondo: la escena tipica de anotar una captura.
    fn escena_con_recinto() -> Escena {
        let mut e = Escena::nueva();
        e.anadir(Elemento {
            figura: Figura::Imagen { id_objeto: 1 },
            x: -50.0,
            y: -50.0,
            ancho: 400.0,
            alto: 400.0,
            ..Default::default()
        });
        e.anadir(raya((0.0, 0.0), (200.0, 0.0)));
        e.anadir(raya((200.0, 0.0), (200.0, 200.0)));
        e.anadir(raya((200.0, 200.0), (0.0, 200.0)));
        e.anadir(raya((0.0, 200.0), (0.0, 0.0)));
        e
    }

    fn con_herramienta(h: Herramienta) -> Gesto {
        let mut g = Gesto::nuevo();
        g.herramienta = h;
        g.estilo.trazo = ColorRgba::opaco(1.0, 0.8, 0.2);
        g
    }

    #[test]
    fn el_bote_deja_la_mancha_debajo_de_las_paredes_y_encima_de_la_foto() {
        // Encima taparia por dentro los trazos que forman el hueco; al fondo
        // del todo se escondería detras de la foto sobre la que se anota.
        let mut escena = escena_con_recinto();
        let gesto = con_herramienta(Herramienta::Relleno);
        assert!(al_pulsar(
            &mut escena,
            &gesto,
            Punto2::nuevo(100.0, 100.0),
            1.0
        ));
        let i = escena
            .elementos
            .iter()
            .position(|e| matches!(e.figura, Figura::Region { .. }))
            .expect("tiene que haber nacido un relleno");
        assert_eq!(i, 1, "el relleno no quedo entre la foto y las rayas");
        assert!(matches!(escena.elementos[0].figura, Figura::Imagen { .. }));
    }

    #[test]
    fn un_recinto_abierto_no_cambia_la_escena_ni_deja_paso_que_deshacer() {
        // **La prueba que importa del bote.** «No esta cerrado» es una
        // respuesta, no un fallo: si se pintara algo, seria una mancha del
        // tamaño del dibujo entero que habria que deshacer a ciegas.
        let mut escena = escena_con_recinto();
        // Se abre un boquete de 60 px en el lado de arriba.
        escena.elementos[1] = raya((0.0, 0.0), (70.0, 0.0));
        escena.anadir(raya((130.0, 0.0), (200.0, 0.0)));
        let cuantos = escena.elementos.len();
        let pasos = escena.pasos_cerrados();
        let gesto = con_herramienta(Herramienta::Relleno);
        assert!(!al_pulsar(
            &mut escena,
            &gesto,
            Punto2::nuevo(100.0, 100.0),
            1.0
        ));
        assert_eq!(escena.elementos.len(), cuantos, "nacio algo que no debia");
        assert_eq!(
            escena.pasos_cerrados(),
            pasos,
            "un bote que no pinta no puede dejar un paso de deshacer vacio"
        );
    }

    #[test]
    fn volver_a_dar_el_bote_cambia_el_color_en_vez_de_apilar_manchas() {
        let mut escena = escena_con_recinto();
        let mut gesto = con_herramienta(Herramienta::Relleno);
        al_pulsar(&mut escena, &gesto, Punto2::nuevo(100.0, 100.0), 1.0);
        gesto.estilo.trazo = ColorRgba::opaco(0.2, 0.4, 1.0);
        al_pulsar(&mut escena, &gesto, Punto2::nuevo(100.0, 100.0), 1.0);
        let manchas: Vec<&Elemento> = escena
            .visibles()
            .filter(|e| matches!(e.figura, Figura::Region { .. }))
            .collect();
        assert_eq!(manchas.len(), 1, "se apilaron manchas: {}", manchas.len());
        assert_eq!(manchas[0].relleno, Some(ColorRgba::opaco(0.2, 0.4, 1.0)));
    }

    #[test]
    fn con_el_grafito_en_la_mano_el_bote_rellena_de_grafito_y_sin_el_de_la_tinta_del_panel() {
        use pixpin_motor2d::tinta::{MaterialTinta, grafito};
        let mancha = |escena: &Escena| {
            escena
                .visibles()
                .find(|e| matches!(e.figura, Figura::Region { .. }))
                .cloned()
                .expect("nacio el relleno")
        };
        let mut escena = escena_con_recinto();
        let mut gesto = con_herramienta(Herramienta::Grafito);
        gesto.tomar_herramienta(Herramienta::Grafito);
        gesto.tomar_herramienta(Herramienta::Relleno);
        assert!(al_pulsar(
            &mut escena,
            &gesto,
            Punto2::nuevo(100.0, 100.0),
            1.0
        ));
        let m = mancha(&escena);
        assert_eq!(m.material, MaterialTinta::Cuadritos);
        // Y se pinta de grafito de verdad: un mapa, no la mancha lisa.
        assert!(grafito::es_de_grafito(&m));
        assert!(grafito::cocer_sin_horno(&m).is_some());

        // Caso negativo: tras coger el lapiz, el bote vuelve a la tinta del
        // panel, que es lisa.
        let mut escena = escena_con_recinto();
        gesto.tomar_herramienta(Herramienta::Lapiz);
        gesto.tomar_herramienta(Herramienta::Relleno);
        al_pulsar(&mut escena, &gesto, Punto2::nuevo(100.0, 100.0), 1.0);
        assert_eq!(mancha(&escena).material, MaterialTinta::Lisa);
    }

    #[test]
    fn recortar_y_deshacer_devuelve_la_raya_entera_de_una_vez() {
        // Partir una raya son dos cambios —editar una mitad y hacer nacer la
        // otra— y tienen que deshacerse juntos o queda media raya huerfana.
        let mut escena = Escena::nueva();
        let id = escena.anadir(raya((0.0, 50.0), (300.0, 50.0)));
        escena.anadir(raya((100.0, 0.0), (100.0, 100.0)));
        escena.anadir(raya((200.0, 0.0), (200.0, 100.0)));
        let gesto = con_herramienta(Herramienta::Recortar);
        assert!(al_pulsar(
            &mut escena,
            &gesto,
            Punto2::nuevo(150.0, 50.0),
            1.0
        ));
        assert_eq!(escena.visibles().count(), 4, "la raya se partio en dos");
        assert!(escena.deshacer());
        assert_eq!(escena.visibles().count(), 3);
        let vuelta = escena.buscar(id).unwrap();
        let p = vuelta.puntos().unwrap();
        assert_eq!(p[p.len() - 1], Punto2::nuevo(300.0, 50.0));
    }

    #[test]
    fn recortar_lejos_de_toda_raya_no_hace_nada() {
        // Caso negativo: sin esto, un clic en el vacio con la herramienta
        // puesta se llevaria por delante la raya mas cercana del dibujo.
        let mut escena = Escena::nueva();
        escena.anadir(raya((0.0, 0.0), (100.0, 0.0)));
        let gesto = con_herramienta(Herramienta::Recortar);
        assert!(!al_pulsar(
            &mut escena,
            &gesto,
            Punto2::nuevo(0.0, 400.0),
            1.0
        ));
    }

    #[test]
    fn extender_lleva_la_punta_hasta_la_pared_y_conserva_el_id() {
        // Sigue siendo la misma raya: si naciera otra, deshacer dejaria dos.
        let mut escena = Escena::nueva();
        let id = escena.anadir(raya((0.0, 50.0), (50.0, 50.0)));
        escena.anadir(raya((100.0, 0.0), (100.0, 100.0)));
        let gesto = con_herramienta(Herramienta::Extender);
        assert!(al_pulsar(
            &mut escena,
            &gesto,
            Punto2::nuevo(50.0, 50.0),
            1.0
        ));
        assert_eq!(escena.visibles().count(), 2);
        let p = escena.buscar(id).unwrap().puntos().unwrap();
        assert!((p[1].x - 100.0).abs() < 0.01, "llego a {:?}", p[1]);
    }

    #[test]
    fn los_angulos_solo_se_ven_mientras_dura_el_gesto() {
        // Caso negativo, y la razon de ser del rotulo: en reposo no se enseña
        // ninguno, o el dibujo se llenaria de cifras que nadie esta mirando.
        let escena = escena_con_recinto();
        let gesto = con_herramienta(Herramienta::Mano);
        assert!(!hay_angulos_que_ensenar(&gesto));
        assert!(angulos_en_vivo(&escena, &gesto, 1.0).is_empty());
    }

    /// Las tres que trabajan sobre lo que ya hay no pueden abrir un arrastre,
    /// y quien lo dice es `Herramienta::deja_rastro` y solo el: una segunda
    /// lista aqui con los mismos tres nombres acabaria discrepando de la
    /// primera el dia que se anada una cuarta.
    #[test]
    fn las_tres_que_trabajan_sobre_lo_que_hay_no_abren_un_arrastre() {
        assert!(!Herramienta::Relleno.deja_rastro());
        assert!(!Herramienta::Recortar.deja_rastro());
        assert!(!Herramienta::Extender.deja_rastro());
        // El punto si nace de un clic: lo crea el gesto y aqui se remata.
        assert!(Herramienta::Punto.deja_rastro());
        assert!(Herramienta::Lapiz.deja_rastro());
    }

    fn caja_sola(x: f32, y: f32, w: f32, h: f32, f: Figura) -> Elemento {
        Elemento {
            figura: f,
            x,
            y,
            ancho: w,
            alto: h,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 2.0,
            ..Default::default()
        }
    }

    #[test]
    fn el_bote_dentro_de_una_sola_figura_le_pone_su_propio_fondo_del_color_puesto() {
        // F3 (v0.67 y v0.76 del movil): el hueco ES la figura, y su propio
        // fondo la rellena exacta; el color es el del trazo que hay puesto.
        for f in [Figura::Rectangulo, Figura::Elipse, Figura::Rombo] {
            let mut escena = Escena::nueva();
            let id = escena.anadir(caja_sola(0.0, 0.0, 200.0, 120.0, f.clone()));
            let gesto = con_herramienta(Herramienta::Relleno);
            let pasos = escena.pasos_cerrados();
            assert!(al_pulsar(
                &mut escena,
                &gesto,
                Punto2::nuevo(100.0, 60.0),
                1.0
            ));
            assert_eq!(
                escena.visibles().count(),
                1,
                "{f:?}: no nace una mancha aparte"
            );
            assert_eq!(escena.buscar(id).unwrap().relleno, Some(gesto.estilo.trazo));
            assert_eq!(escena.pasos_cerrados(), pasos + 1, "un paso de deshacer");
            escena.deshacer();
            assert_eq!(escena.buscar(id).unwrap().relleno, None);
        }
    }

    #[test]
    fn con_algo_dentro_de_la_figura_el_bote_vuelve_a_la_mancha_con_su_agujero() {
        // Caso negativo: un circulo dentro ya no deja que el hueco sea la
        // figura entera, y la figura no se toca.
        let mut escena = Escena::nueva();
        let id = escena.anadir(caja_sola(0.0, 0.0, 300.0, 300.0, Figura::Rectangulo));
        escena.anadir(caja_sola(100.0, 100.0, 100.0, 100.0, Figura::Elipse));
        let gesto = con_herramienta(Herramienta::Relleno);
        assert!(al_pulsar(
            &mut escena,
            &gesto,
            Punto2::nuevo(30.0, 30.0),
            1.0
        ));
        assert_eq!(escena.buscar(id).unwrap().relleno, None);
        assert!(
            escena
                .visibles()
                .any(|e| matches!(e.figura, Figura::Region { .. }))
        );
    }

    #[test]
    fn soldar_clava_en_la_junta_y_otro_clic_en_el_clavo_lo_quita() {
        let mut escena = Escena::nueva();
        escena.anadir(raya((0.0, 100.0), (100.0, 100.0)));
        escena.anadir(raya((100.0, 100.0), (100.0, 0.0)));
        let gesto = con_herramienta(Herramienta::Nudo);
        // Cinco pixeles de pantalla al lado de la junta, a zoom 2: el clavo
        // va a la junta de verdad, no donde cayo el raton.
        assert!(al_pulsar(
            &mut escena,
            &gesto,
            Punto2::nuevo(102.0, 101.0),
            2.0
        ));
        assert_eq!(escena.alfileres.len(), 1);
        assert_eq!(escena.alfileres[0].punto, Punto2::nuevo(100.0, 100.0));
        assert!(al_pulsar(
            &mut escena,
            &gesto,
            Punto2::nuevo(100.0, 100.0),
            2.0
        ));
        assert!(escena.alfileres.is_empty(), "el segundo clic no lo quito");
    }

    #[test]
    fn soldar_en_el_vacio_no_cambia_nada() {
        // Caso negativo: un clavo que no atraviesa dos figuras no sujeta nada.
        let mut escena = escena_con_recinto();
        let pasos = escena.pasos_cerrados();
        let gesto = con_herramienta(Herramienta::Nudo);
        assert!(!al_pulsar(
            &mut escena,
            &gesto,
            Punto2::nuevo(100.0, 100.0),
            1.0
        ));
        assert!(escena.alfileres.is_empty());
        assert_eq!(escena.pasos_cerrados(), pasos);
        assert!(
            !Herramienta::Nudo.deja_rastro(),
            "soldar no hace nacer nada"
        );
    }

    #[test]
    fn el_punto_sale_numerado_con_la_serie_que_tiene_puesta_el_gesto() {
        use pixpin_motor2d::gesto::EventoGesto;
        let letra = |serie: SerieDePunto, x: f32| {
            let mut escena = Escena::nueva();
            escena.anadir(raya((0.0, 0.0), (200.0, 0.0)));
            escena.anadir(raya((x, -50.0), (x, 50.0)));
            let mut gesto = con_herramienta(Herramienta::Punto);
            gesto.serie_de_punto = serie;
            let p = Punto2::nuevo(x + 2.0, 1.0);
            gesto.evento(
                EventoGesto::Pulsar {
                    p,
                    shift: false,
                    alt: false,
                    presion: None,
                },
                &mut escena,
                1.0,
            );
            assert!(al_pulsar(&mut escena, &gesto, p, 1.0));
            gesto.evento(EventoGesto::Soltar { p }, &mut escena, 1.0);
            escena
                .visibles()
                .find_map(|e| match &e.figura {
                    Figura::Punto { letra, .. } => Some((letra.clone(), e.x, e.y)),
                    _ => None,
                })
                .expect("no nacio el punto")
        };
        assert_eq!(letra(SerieDePunto::Numeros, 80.0).0, "1");
        assert_eq!(letra(SerieDePunto::Minusculas, 80.0).0, "a");
        // Caso de siempre: sin tocar nada, mayusculas; y en el cruce exacto.
        let (l, x, y) = letra(SerieDePunto::default(), 80.0);
        assert_eq!(l, "A");
        assert_eq!((x, y), (80.0, 0.0));
    }
}
