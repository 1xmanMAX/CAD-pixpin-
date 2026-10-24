//! Pruebas de la geometria de lamina: arcos, sectores, marcas, degradados,
//! el iman a cruces y tangentes, recortar y extender, guardar y leer (con la
//! reserva del movil) y exportar.

use super::*;
use crate::enganche::{self, Ajustes, AjustesFinos, Faena, TipoAnclaje};
use crate::escena::Escena;
use crate::gesto::{EventoGesto, Gesto, Herramienta};

const EPS: f32 = 1e-3;

fn p(x: f32, y: f32) -> Punto2 {
    Punto2::nuevo(x, y)
}

fn cerca(a: Punto2, b: Punto2, tol: f32) -> bool {
    a.distancia(b) <= tol
}

fn tramo(e: &Elemento) -> (f32, f32) {
    match e.figura {
        Figura::Arco {
            inicio,
            barrido: Some(b),
        } => (inicio, b),
        _ => panic!("no es un arco repasado: {:?}", e.figura),
    }
}

fn centro(e: &Elemento) -> Punto2 {
    p(e.x + e.ancho / 2.0, e.y + e.alto / 2.0)
}

// ---------------------------------------------------------------------------
// Geometria del arco y del sector

#[test]
fn el_arco_por_tres_puntos_pasa_por_los_tres_y_en_su_orden() {
    let (a, m, b) = (p(100.0, 0.0), p(0.0, 100.0), p(-100.0, 0.0));
    let e = arco_por_tres_puntos(a, m, b).expect("tres puntos en circulo");
    assert!(cerca(centro(&e), p(0.0, 0.0), 0.01));
    assert!((e.ancho / 2.0 - 100.0).abs() < 0.01);
    let (i, s) = tramo(&e);
    // Empieza en a (0 rad), pasa por m (pi/2, abajo en pantalla) y acaba en b.
    assert!(i.abs() < EPS);
    assert!((s - PI).abs() < 1e-3, "barrido {s}");
    // Y por el otro lado, si el punto de paso esta arriba.
    let e2 = arco_por_tres_puntos(a, p(0.0, -100.0), b).unwrap();
    assert!((tramo(&e2).1 + PI).abs() < 1e-3);
    // En linea no hay arco.
    assert!(arco_por_tres_puntos(p(0.0, 0.0), p(1.0, 1.0), p(2.0, 2.0)).is_none());
}

#[test]
fn el_arco_por_centro_y_dos_puntos_toma_el_radio_del_primero() {
    let c = p(50.0, 50.0);
    let e = arco_por_centro_y_dos_puntos(c, p(150.0, 50.0), p(50.0, 0.0), false);
    let (i, s) = tramo(&e);
    assert!((e.ancho - 200.0).abs() < 0.01);
    assert!(i.abs() < EPS);
    // A la vista, en contra del reloj: hacia arriba (y negativa), -90 grados.
    assert!((s + PI / 2.0).abs() < 1e-3, "barrido {s}");
    let h = arco_por_centro_y_dos_puntos(c, p(150.0, 50.0), p(50.0, 0.0), true);
    assert!((tramo(&h).1 - 3.0 * PI / 2.0).abs() < 1e-3);
}

#[test]
fn semicircunferencia_y_cuadrante_son_arcos_con_sus_puntas_en_su_sitio() {
    let (a, b) = (p(0.0, 100.0), p(200.0, 100.0));
    let s = semicircunferencia(a, b, true);
    let pts = crate::pintado::arco_muestreado(&s, tramo(&s).0, Some(tramo(&s).1));
    assert!(cerca(pts[0], a, 0.01) && cerca(*pts.last().unwrap(), b, 0.01));
    // La de arriba: todos sus puntos por encima del diametro.
    assert!(pts.iter().all(|q| q.y <= 100.0 + 0.01));
    assert!(pts.iter().any(|q| q.y < 1.0), "llega arriba del todo");

    let q = cuadrante(p(0.0, 0.0), p(0.0, -100.0), true);
    assert_eq!(q.extras.cierre, Some(CierreArco::Sector));
    let (_, barrido) = tramo(&q);
    assert!((barrido - PI / 2.0).abs() < EPS);
    // La caja del sector llega hasta el centro.
    let (x0, y0, x1, y1) = q.caja();
    assert!(
        x0 <= 0.01 && y1 >= -0.01 && x1 >= 99.9 && y0 <= -99.9,
        "{:?}",
        (x0, y0, x1, y1)
    );
}

#[test]
fn el_sector_cierra_por_el_centro_y_el_segmento_por_la_cuerda() {
    let c = p(0.0, 0.0);
    let s = sector(c, 50.0, 0.0, PI / 3.0);
    let cs = contorno_cerrado(&s).unwrap();
    assert!(
        cerca(*cs.last().unwrap(), c, 0.01),
        "el sector acaba en el centro"
    );
    let g = arco_cerrado(c, 50.0, 0.0, PI / 3.0, CierreArco::Segmento);
    let cg = contorno_cerrado(&g).unwrap();
    assert!(
        cg.iter().all(|q| (q.distancia(c) - 50.0).abs() < 0.01),
        "el segmento no pasa por el centro"
    );
    // Y el arco abierto no tiene contorno cerrado (caso negativo).
    assert!(contorno_cerrado(&arco(c, 50.0, 0.0, 1.0)).is_none());
    // Pintado: relleno + borde cerrado.
    let mut s2 = s.clone();
    s2.relleno = Some(ColorRgba::opaco(1.0, 0.5, 0.0));
    let o = crate::pintado::ordenes(&s2);
    assert!(matches!(o[0], Orden::Relleno { .. }));
    assert!(
        o.iter()
            .any(|x| matches!(x, Orden::Polilinea { puntos, .. } if puntos.len() > 3))
    );
    // Y se toca por dentro.
    assert!(crate::impacto::toca(&s2, p(30.0, 10.0)));
    assert!(!crate::impacto::toca(&s2, p(-30.0, -10.0)));
}

#[test]
fn la_marca_de_angulo_mide_el_angulo_menor_y_la_recta_lleva_cuadradito() {
    let v = p(0.0, 0.0);
    let m = marca_de_angulo(v, p(100.0, 0.0), p(0.0, -100.0), 20.0, 2);
    let (_, s) = tramo(&m);
    assert!((s + PI / 2.0).abs() < EPS);
    assert_eq!(m.extras.marca_angulo.as_ref().unwrap().arcos, 2);
    // Dos arquitos: dos polilineas de arco.
    let arcos = crate::pintado::ordenes(&m)
        .into_iter()
        .filter(|o| matches!(o, Orden::Polilinea { .. }))
        .count();
    assert_eq!(arcos, 2);
    // A 90 grados, `marca_para` pone el cuadradito: tres puntos de trazo.
    let r = marca_para(v, p(100.0, 0.0), p(0.0, 100.0), 20.0, 1);
    assert!(r.extras.marca_angulo.as_ref().unwrap().recto);
    let o = crate::pintado::ordenes(&r);
    assert!(
        o.iter()
            .any(|x| matches!(x, Orden::Polilinea { puntos, .. } if puntos.len() == 3))
    );
    // A 60 no (caso negativo).
    let n = marca_para(v, p(100.0, 0.0), p(50.0, -86.6), 20.0, 1);
    assert!(!n.extras.marca_angulo.as_ref().unwrap().recto);
}

#[test]
fn la_marca_con_valor_lo_escribe_fuera_sobre_la_bisectriz() {
    let mut m = marca_de_angulo(p(0.0, 0.0), p(100.0, 0.0), p(0.0, 100.0), 20.0, 1);
    m.extras.marca_angulo.as_mut().unwrap().valor = Some("θ".into());
    let texto = crate::pintado::ordenes(&m)
        .into_iter()
        .find_map(|o| match o {
            Orden::Texto { x, y, texto, .. } => Some((x, y, texto)),
            _ => None,
        })
        .expect("el valor");
    assert_eq!(texto.2, "θ");
    // Abajo a la derecha del vertice (la bisectriz va a 45 grados en pantalla).
    assert!(texto.0 > 10.0 && texto.1 > 10.0, "{texto:?}");
}

#[test]
fn el_angulo_en_un_clic_elige_los_lados_que_encierran_el_clic() {
    let elementos = vec![
        Elemento {
            id: 1,
            ..recta(p(0.0, 0.0), p(200.0, 0.0))
        },
        Elemento {
            id: 2,
            ..recta(p(0.0, 0.0), p(100.0, -100.0))
        },
    ];
    let (v, a, b) = angulo_en(&elementos, p(30.0, -10.0), 60.0).expect("hay angulo");
    assert!(cerca(v, p(0.0, 0.0), 0.01));
    let (_, s) = angulo_menor(v, a, b);
    assert!((s.abs() - PI / 4.0).abs() < 1e-3, "45 grados, no {s}");
    // Lejos del vertice no hay angulo (caso negativo).
    assert!(angulo_en(&elementos, p(150.0, -60.0), 30.0).is_none());
    // Con el vertice en medio de una recta: el clic de la izquierda da el de
    // 180 - 45 = 135 grados.
    let el2 = vec![
        Elemento {
            id: 1,
            ..recta(p(-100.0, 0.0), p(200.0, 0.0))
        },
        elementos[1].clone(),
    ];
    let (v2, a2, b2) = angulo_en(&el2, p(-20.0, -10.0), 60.0).unwrap();
    assert!((angulo_menor(v2, a2, b2).1.abs() - 3.0 * PI / 4.0).abs() < 1e-3);
}

// ---------------------------------------------------------------------------
// Degradado y raya-punto

#[test]
fn el_degradado_radial_va_del_centro_al_borde_en_bandas() {
    let mut e = circunferencia(p(100.0, 100.0), 100.0);
    let naranja = ColorRgba::opaco(1.0, 0.6, 0.1);
    e.extras.degradado = Some(Degradado::radial(naranja, BLANCO));
    let o = crate::pintado::ordenes(&e);
    let rellenos: Vec<&Orden> = o
        .iter()
        .filter(|x| matches!(x, Orden::Relleno { .. }))
        .collect();
    assert!(
        rellenos.len() > BANDAS_DEGRADADO / 2,
        "{} bandas",
        rellenos.len()
    );
    // La ultima banda (la de dentro) es casi naranja; la primera, casi blanca.
    let color = |o: &Orden| match o {
        Orden::Relleno { color, .. } => *color,
        _ => unreachable!(),
    };
    assert!(color(rellenos.last().unwrap()).b < 0.2);
    assert!(color(rellenos[0]).b > 0.9);
    // Sin degradado y sin relleno, nada que rellenar (caso negativo).
    let liso = circunferencia(p(0.0, 0.0), 10.0);
    assert!(
        !crate::pintado::ordenes(&liso)
            .iter()
            .any(|x| matches!(x, Orden::Relleno { .. }))
    );
}

#[test]
fn el_degradado_lineal_translucido_no_apila_bandas() {
    let cuadrado = vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)];
    let d = Degradado::lineal(
        ColorRgba { a: 0.5, ..BLANCO },
        ColorRgba {
            a: 0.5,
            r: 0.0,
            g: 0.0,
            b: 0.0,
        },
    );
    let o = ordenes_de_degradado(&cuadrado, (0.0, 0.0, 100.0, 100.0), &d, 1.0);
    // Bandas una al lado de otra: el area total es la del cuadrado.
    let area: f32 = o
        .iter()
        .map(|x| match x {
            Orden::Relleno { puntos, .. } => {
                let n = puntos.len();
                (0..n)
                    .map(|i| {
                        let (a, b) = (puntos[i], puntos[(i + 1) % n]);
                        a.x * b.y - b.x * a.y
                    })
                    .sum::<f32>()
                    .abs()
                    / 2.0
            }
            _ => 0.0,
        })
        .sum();
    assert!((area - 10_000.0).abs() < 50.0, "area {area}");
}

#[test]
fn el_raya_punto_sale_en_tramos_lisos_de_dos_largos() {
    let mut e = recta(p(0.0, 0.0), p(300.0, 0.0));
    e.estilo = EstiloTrazo::RayaPunto;
    let o = crate::pintado::ordenes(&e);
    assert!(o.iter().all(|x| !matches!(
        x,
        Orden::Polilinea {
            estilo: EstiloTrazo::RayaPunto,
            ..
        }
    )));
    let largos: Vec<f32> = o
        .iter()
        .filter_map(|x| match x {
            Orden::Polilinea { puntos, .. } => Some(puntos[0].distancia(*puntos.last().unwrap())),
            _ => None,
        })
        .collect();
    assert!(largos.len() >= 10);
    assert!(
        largos.iter().any(|l| *l > 10.0) && largos.iter().any(|l| *l < 3.0),
        "{largos:?}"
    );
}

// ---------------------------------------------------------------------------
// El iman: cruces y tangentes

#[test]
fn las_tangentes_desde_un_punto_son_perpendiculares_al_radio() {
    let (c, r, q) = (p(0.0, 0.0), 50.0, p(130.0, 0.0));
    let ts = tangentes_desde(q, c, r).unwrap();
    for t in ts {
        assert!((t.distancia(c) - r).abs() < 1e-3);
        assert!(t.restar(c).producto(t.restar(q)).abs() < 0.05);
    }
    assert!(
        tangentes_desde(p(10.0, 0.0), c, r).is_none(),
        "desde dentro no hay"
    );
}

fn escena_con(elementos: Vec<Elemento>) -> Escena {
    let mut e = Escena::nueva();
    for x in elementos {
        e.anadir(x);
    }
    e
}

#[test]
fn el_iman_se_pega_al_cruce_de_una_recta_con_una_circunferencia() {
    let escena = escena_con(vec![
        circunferencia(p(0.0, 0.0), 100.0),
        recta(p(-200.0, 50.0), p(200.0, 50.0)),
    ]);
    let x = (100.0f32 * 100.0 - 50.0 * 50.0).sqrt();
    let a = enganche::sitio_fino(
        &escena.elementos,
        p(x - 3.0, 53.0),
        1.0,
        Faena::Trazando,
        &Ajustes::default(),
        &AjustesFinos::default(),
        &[],
    )
    .expect("engancha");
    assert_eq!(a.tipo, TipoAnclaje::Interseccion);
    assert!(cerca(a.punto, p(x, 50.0), 1.0), "{:?}", a);
}

#[test]
fn el_iman_se_pega_a_la_tangente_desde_donde_sale_la_raya() {
    let c = p(0.0, 0.0);
    let escena = escena_con(vec![circunferencia(c, 100.0)]);
    let desde = p(250.0, 30.0);
    let t = tangentes_desde(desde, c, 100.0).unwrap()[0];
    let finos = AjustesFinos {
        desde: Some(desde),
        ..AjustesFinos::default()
    };
    let a = enganche::sitio_fino(
        &escena.elementos,
        t.sumar(p(3.0, -2.0)),
        1.0,
        Faena::Trazando,
        &Ajustes::default(),
        &finos,
        &[],
    )
    .expect("engancha");
    assert_eq!(a.tipo, TipoAnclaje::Tangente);
    assert!(cerca(a.punto, t, 0.01));
    // Sin `desde` no hay tangente (caso negativo).
    let b = enganche::sitio_fino(
        &escena.elementos,
        t.sumar(p(3.0, -2.0)),
        1.0,
        Faena::Trazando,
        &Ajustes::default(),
        &AjustesFinos::default(),
        &[],
    );
    assert!(b.is_none_or(|b| b.tipo != TipoAnclaje::Tangente));
}

#[test]
fn trazando_una_recta_con_el_gesto_acaba_en_la_tangente() {
    let c = p(0.0, 0.0);
    let mut escena = escena_con(vec![circunferencia(c, 100.0)]);
    let mut g = Gesto::nuevo();
    g.finos = AjustesFinos::default();
    g.tomar_herramienta(Herramienta::Linea);
    let desde = p(250.0, 30.0);
    let t = tangentes_desde(desde, c, 100.0).unwrap()[1];
    g.evento(
        EventoGesto::Pulsar {
            p: desde,
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );
    let casi = t.sumar(p(2.0, 3.0));
    g.evento(
        EventoGesto::Mover {
            p: casi,
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );
    g.evento(EventoGesto::Soltar { p: casi }, &mut escena, 1.0);
    let linea = escena.elementos.last().unwrap();
    let Figura::Linea { puntos } = &linea.figura else {
        panic!()
    };
    assert!(cerca(puntos[1], t, 0.01), "{:?} vs {:?}", puntos[1], t);
}

#[test]
fn el_iman_ofrece_las_puntas_el_medio_y_el_centro_del_arco() {
    let a = arco(p(0.0, 0.0), 100.0, 0.0, PI / 2.0);
    let escena = escena_con(vec![a]);
    let pide = |q: Punto2| {
        enganche::sitio(
            &escena.elementos,
            q,
            1.0,
            Faena::Trazando,
            &Ajustes::default(),
            &[],
        )
        .unwrap()
    };
    assert_eq!(pide(p(98.0, 2.0)).tipo, TipoAnclaje::Extremo);
    let medio = en_la_circunferencia(p(0.0, 0.0), 100.0, PI / 4.0);
    assert!(cerca(pide(medio.sumar(p(1.0, 1.0))).punto, medio, 0.01));
    assert_eq!(pide(p(2.0, 2.0)).tipo, TipoAnclaje::Centro);
}

// ---------------------------------------------------------------------------
// Recortar y extender

#[test]
fn recortar_una_circunferencia_entre_dos_rectas_deja_un_arco() {
    let mut escena = escena_con(vec![
        circunferencia(p(0.0, 0.0), 100.0),
        recta(p(0.0, -200.0), p(0.0, 200.0)),
    ]);
    let id = escena.elementos[0].id;
    // Se quita la mitad de la derecha: queda la semicircunferencia izquierda.
    assert!(crate::recorte::recortar_en_escena(
        &mut escena,
        id,
        p(100.0, 0.0),
        None
    ));
    let quedan: Vec<&Elemento> = escena
        .elementos
        .iter()
        .filter(|e| !e.borrado && matches!(e.figura, Figura::Arco { .. }))
        .collect();
    assert_eq!(quedan.len(), 1);
    let (i, s) = tramo(quedan[0]);
    assert!((s.abs() - PI).abs() < 0.02, "media vuelta, no {s}");
    let medio = en_la_circunferencia(p(0.0, 0.0), 100.0, i + s / 2.0);
    assert!(medio.x < -99.0, "queda la izquierda: {medio:?}");
}

#[test]
fn extender_un_arco_sigue_por_su_circunferencia_hasta_la_recta() {
    let a = Elemento {
        id: 1,
        ..arco(p(0.0, 0.0), 100.0, 0.0, PI / 4.0)
    };
    // La vertical x = -50, por debajo del centro.
    let tope = Elemento {
        id: 2,
        ..recta(p(-50.0, 0.0), p(-50.0, 200.0))
    };
    let nuevo = crate::recorte::extender_en(&a, std::slice::from_ref(&tope), p(70.0, 70.0), 1000.0)
        .expect("topa");
    let (i, s) = tramo(&nuevo);
    assert!(i.abs() < EPS);
    let fin = en_la_circunferencia(p(0.0, 0.0), 100.0, i + s);
    assert!(
        (fin.x + 50.0).abs() < 0.6 && fin.y > 0.0,
        "acaba en la recta: {fin:?}"
    );
    // Sin nada en el camino, nada (caso negativo).
    let lejos = Elemento {
        id: 3,
        ..recta(p(500.0, 500.0), p(600.0, 600.0))
    };
    assert!(crate::recorte::extender_en(&a, &[lejos], p(70.0, 70.0), 1000.0).is_none());
}

// ---------------------------------------------------------------------------
// Las herramientas del gesto

fn clic(g: &mut Gesto, escena: &mut Escena, q: Punto2) {
    g.evento(
        EventoGesto::Pulsar {
            p: q,
            shift: false,
            alt: false,
            presion: None,
        },
        escena,
        1.0,
    );
    g.evento(EventoGesto::Soltar { p: q }, escena, 1.0);
}

fn hover(g: &mut Gesto, escena: &mut Escena, q: Punto2) {
    g.evento(
        EventoGesto::Mover {
            p: q,
            shift: false,
            alt: false,
            presion: None,
        },
        escena,
        1.0,
    );
}

#[test]
fn el_arco_por_centro_y_dos_puntos_con_tres_clics_y_un_solo_deshacer() {
    let mut escena = Escena::nueva();
    let mut g = Gesto::nuevo();
    g.enganche.activo = false;
    g.tomar_herramienta(Herramienta::Arco);
    g.estilo.modo_arco = ModoArco::CentroYPuntos;
    clic(&mut g, &mut escena, p(0.0, 0.0));
    clic(&mut g, &mut escena, p(100.0, 0.0));
    assert_eq!(g.clics_de_arco(), 2);
    // El cursor da la vuelta por abajo (a favor del reloj en pantalla).
    for k in 1..=9 {
        let t = (k as f32) * 10.0_f32.to_radians();
        hover(
            &mut g,
            &mut escena,
            en_la_circunferencia(p(0.0, 0.0), 80.0, t),
        );
    }
    assert_eq!(escena.visibles().count(), 1, "el de muestra");
    clic(&mut g, &mut escena, p(0.0, 50.0));
    let vivos: Vec<&Elemento> = escena.visibles().collect();
    assert_eq!(vivos.len(), 1);
    let (i, s) = tramo(vivos[0]);
    assert!(i.abs() < EPS && (s - PI / 2.0).abs() < 1e-3, "{i} {s}");
    assert!((vivos[0].ancho - 200.0).abs() < 0.01);
    assert!(escena.deshacer());
    assert_eq!(escena.visibles().count(), 0, "un Ctrl+Z se lo lleva entero");
}

#[test]
fn el_arco_por_tres_puntos_y_escape_a_medias_no_deja_nada() {
    let mut escena = Escena::nueva();
    let mut g = Gesto::nuevo();
    g.enganche.activo = false;
    g.tomar_herramienta(Herramienta::Arco);
    g.estilo.modo_arco = ModoArco::TresPuntos;
    clic(&mut g, &mut escena, p(100.0, 0.0));
    clic(&mut g, &mut escena, p(0.0, 100.0));
    hover(&mut g, &mut escena, p(-90.0, 10.0));
    assert_eq!(escena.visibles().count(), 1);
    g.evento(EventoGesto::Escape, &mut escena, 1.0);
    assert_eq!(escena.visibles().count(), 0);
    clic(&mut g, &mut escena, p(100.0, 0.0));
    clic(&mut g, &mut escena, p(0.0, 100.0));
    clic(&mut g, &mut escena, p(-100.0, 0.0));
    let e = escena.visibles().next().unwrap();
    assert!(cerca(centro(e), p(0.0, 0.0), 0.01));
    assert!((tramo(e).1 - PI).abs() < 1e-3);
}

#[test]
fn repasar_una_guia_con_el_ovalo_deja_el_arco_repasado() {
    let mut escena = Escena::nueva();
    let mut guia = circunferencia(p(0.0, 0.0), 100.0);
    guia.extras.referencia = true;
    escena.anadir(guia);
    let mut g = Gesto::nuevo();
    g.enganche.activo = false;
    g.tomar_herramienta(Herramienta::Elipse);
    g.evento(
        EventoGesto::Pulsar {
            p: p(100.0, 0.0),
            shift: false,
            alt: false,
            presion: None,
        },
        &mut escena,
        1.0,
    );
    for k in 1..=6 {
        let t = (k as f32) * 15.0_f32.to_radians();
        hover(
            &mut g,
            &mut escena,
            en_la_circunferencia(p(0.0, 0.0), 100.0, t),
        );
    }
    g.evento(EventoGesto::Soltar { p: p(0.0, 100.0) }, &mut escena, 1.0);
    let arco = escena
        .elementos
        .iter()
        .find(|e| !e.borrado && !e.extras.referencia)
        .expect("el repasado");
    assert!((tramo(arco).1 - PI / 2.0).abs() < 1e-3);
    // Un clic sin arrastrar sobre la guia no deja nada (caso negativo).
    let antes = escena.elementos.iter().filter(|e| !e.borrado).count();
    clic(&mut g, &mut escena, p(-100.0, 0.0));
    assert_eq!(
        escena.elementos.iter().filter(|e| !e.borrado).count(),
        antes
    );
}

#[test]
fn la_herramienta_angulo_pone_la_marca_de_un_clic() {
    let mut escena = escena_con(vec![
        recta(p(0.0, 0.0), p(200.0, 0.0)),
        recta(p(0.0, 0.0), p(0.0, -200.0)),
    ]);
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Angulo);
    clic(&mut g, &mut escena, p(20.0, -20.0));
    let m = escena
        .visibles()
        .find(|e| e.extras.marca_angulo.is_some())
        .expect("la marca");
    assert!(
        m.extras.marca_angulo.as_ref().unwrap().recto,
        "90 grados: el cuadradito"
    );
    assert!(m.relleno.is_some_and(|c| c.a < 0.5), "relleno translucido");
    // Un clic en el vacio no pone nada.
    let antes = escena.visibles().count();
    clic(&mut g, &mut escena, p(500.0, 500.0));
    assert_eq!(escena.visibles().count(), antes);
}

// ---------------------------------------------------------------------------
// Guardar y leer, y la reserva del movil

fn lamina() -> Vec<Elemento> {
    let mut s = sector(p(0.0, 0.0), 100.0, -PI / 2.0, PI / 2.0);
    s.extras.degradado = Some(Degradado::radial(ColorRgba::opaco(1.0, 0.7, 0.2), BLANCO));
    let mut m = marca_de_angulo(p(0.0, 0.0), p(100.0, 0.0), p(70.0, -70.0), 20.0, 2);
    m.extras.marca_angulo.as_mut().unwrap().valor = Some("45°".into());
    m.extras.tam_letra = Some(16.0);
    let mut r = recta(p(0.0, 0.0), p(100.0, 100.0));
    r.estilo = EstiloTrazo::RayaPunto;
    let mut pt = punto_con_letra(p(10.0, 10.0), "P", 0.0);
    pt.extras.tam_letra = Some(18.0);
    let mut seg = arco_cerrado(p(0.0, 0.0), 80.0, 0.0, 1.0, CierreArco::Segmento);
    seg.extras.degradado = Some(Degradado::lineal(ColorRgba::opaco(0.0, 0.0, 1.0), BLANCO));
    vec![s, m, r, pt, seg]
}

#[test]
fn guardar_y_leer_el_pixpin2d_conserva_todo() {
    let escena = escena_con(lamina());
    let dir = std::env::temp_dir().join(format!("pixpin-geom-{}", std::process::id()));
    let ruta = dir.join("lamina.pixpin2d");
    crate::formato::guardar(&ruta, &escena).unwrap();
    let leida = crate::formato::cargar(&ruta).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(leida.elementos.len(), escena.elementos.len());
    for (a, b) in escena.elementos.iter().zip(&leida.elementos) {
        assert_eq!(a.extras, b.extras);
        assert_eq!(a.estilo, b.estilo);
        assert_eq!(a.figura, b.figura);
    }
}

fn al_movil(escena: &Escena) -> String {
    crate::excalidraw::escribir(&crate::excalidraw::con_escena(
        &crate::excalidraw::Lienzo::vacio(),
        escena,
    ))
}

#[test]
fn ida_y_vuelta_por_el_fichero_del_movil() {
    let escena = escena_con(lamina());
    let json = al_movil(&escena);
    let vuelta = crate::excalidraw::a_escena(&crate::excalidraw::leer(&json).unwrap());
    let vivos: Vec<&Elemento> = vuelta.visibles().collect();
    assert_eq!(vivos.len(), 5);
    for (a, b) in escena.elementos.iter().zip(vivos) {
        assert_eq!(a.extras.cierre, b.extras.cierre);
        assert_eq!(a.extras.marca_angulo, b.extras.marca_angulo);
        assert_eq!(a.estilo, b.estilo);
        assert_eq!(a.extras.tam_letra, b.extras.tam_letra);
        match (&a.extras.degradado, &b.extras.degradado) {
            (Some(x), Some(y)) => {
                assert_eq!(x.tipo, y.tipo);
                assert!(
                    (x.desde.r - y.desde.r).abs() < 0.01 && (x.hasta.g - y.hasta.g).abs() < 0.01
                );
            }
            (None, None) => {}
            otro => panic!("degradado perdido: {otro:?}"),
        }
    }
}

#[test]
fn la_reserva_el_movil_ve_arcos_rayas_y_rellenos_que_entiende() {
    let escena = escena_con(lamina());
    let json = al_movil(&escena);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let els = v["elements"].as_array().unwrap();
    // El sector: un `pixpin-arc` de siempre, con su barrido, y el relleno
    // liso de reserva en `backgroundColor`.
    assert_eq!(els[0]["type"], "pixpin-arc");
    assert!(els[0]["arcSweep"].as_f64().is_some());
    assert_ne!(els[0]["backgroundColor"], "transparent");
    assert_eq!(els[0]["cierreArco"], "sector");
    // El raya-punto viaja como `dashed`.
    assert_eq!(els[2]["strokeStyle"], "dashed");
    assert_eq!(els[2]["trazoRayaPunto"], true);
    // Un `pixpin-arc` del movil, sin las claves nuevas, se lee como antes.
    let viejo = r##"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"pixpin-arc","x":0,"y":0,"width":100,"height":100,"angle":0,"strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,"strokeStyle":"dashed","roughness":0,"opacity":100,"seed":1,"arcStart":0,"arcSweep":1.5}]}"##;
    let e = crate::excalidraw::a_escena(&crate::excalidraw::leer(viejo).unwrap());
    let a = e.visibles().next().unwrap();
    assert!(
        a.extras.cierre.is_none()
            && a.extras.marca_angulo.is_none()
            && a.extras.degradado.is_none()
    );
    assert_eq!(a.estilo, EstiloTrazo::Discontinuo);
    // Un cierre de una version futura no se pierde al pasar por aqui.
    let futuro = viejo.replace(
        "\"arcSweep\":1.5",
        "\"arcSweep\":1.5,\"cierreArco\":\"espiral\"",
    );
    let l = crate::excalidraw::leer(&futuro).unwrap();
    let de_vuelta = crate::excalidraw::escribir(&crate::excalidraw::con_escena(
        &l,
        &crate::excalidraw::a_escena(&l),
    ));
    assert!(de_vuelta.contains("espiral"));
}

// ---------------------------------------------------------------------------
// Exportar

#[test]
fn exportar_a_svg_pinta_las_bandas_el_raya_punto_y_la_marca() {
    let escena = escena_con(lamina());
    let hojas = crate::exportar::hojas(&escena, crate::exportar::Alcance::Todo, &[], None);
    assert_eq!(hojas.len(), 1);
    let svg = crate::exportar_svg::svg(&hojas[0], Default::default(), &|_| None);
    // Las bandas del degradado y el raya-punto son muchas piezas lisas.
    let piezas = svg.matches("<path").count()
        + svg.matches("<polygon").count()
        + svg.matches("<polyline").count();
    assert!(piezas > BANDAS_DEGRADADO, "{piezas} piezas");
    assert!(svg.contains("45°"), "el valor de la marca");
    assert!(!svg.contains("NaN"));
}

// ---------------------------------------------------------------------------
// Los angulos con su letra griega (17-oct)

/// Un triangulo de lados rectos: A(0,0), B(200,0), C(60,-150). Ningun angulo
/// es recto.
fn triangulo() -> Escena {
    escena_con(vec![
        recta(p(0.0, 0.0), p(200.0, 0.0)),
        recta(p(200.0, 0.0), p(60.0, -150.0)),
        recta(p(60.0, -150.0), p(0.0, 0.0)),
    ])
}

fn letras(escena: &Escena) -> Vec<Option<String>> {
    escena
        .visibles()
        .filter_map(|e| e.extras.marca_angulo.as_ref())
        .map(|m| m.valor.clone())
        .collect()
}

#[test]
fn la_serie_griega_sigue_el_orden_pedido_y_no_repite() {
    assert_eq!(letra_de_angulo(0), "α");
    assert_eq!(letra_de_angulo(4), "ω");
    assert_eq!(letra_de_angulo(16), "α1", "pasada la ultima, con numero");
    let usadas = vec!["α".to_string(), "θ".to_string()];
    assert_eq!(siguiente_letra_de_angulo(&usadas), "β");
    // La de al lado salta las que usan otras marcas, y da la vuelta.
    assert_eq!(letra_vecina(Some("β"), 1, &usadas), "γ");
    assert_eq!(letra_vecina(Some("β"), -1, &usadas), "κ");
    // Sin letra, la primera libre (caso negativo: no inventa una usada).
    assert_eq!(letra_vecina(None, 1, &usadas), "β");
}

#[test]
fn la_herramienta_angulo_pone_alfa_beta_theta_y_reutiliza_la_borrada() {
    let mut escena = triangulo();
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Angulo);
    assert_eq!(
        g.estilo.modo_angulo,
        ModoAngulo::Griega,
        "de fabrica, con letra"
    );
    clic(&mut g, &mut escena, p(25.0, -8.0)); // en A
    clic(&mut g, &mut escena, p(175.0, -8.0)); // en B
    assert_eq!(letras(&escena), vec![Some("α".into()), Some("β".into())]);
    // Se borra α: la siguiente vuelve a ser α, no θ (se mira el dibujo).
    let alfa = escena
        .visibles()
        .find(|e| {
            e.extras
                .marca_angulo
                .as_ref()
                .and_then(|m| m.valor.as_deref())
                == Some("α")
        })
        .map(|e| e.id)
        .unwrap();
    escena.borrar(alfa);
    clic(&mut g, &mut escena, p(62.0, -125.0)); // en C
    assert_eq!(letras(&escena), vec![Some("β".into()), Some("α".into())]);
}

#[test]
fn el_recto_lleva_su_cuadradito_y_sin_letra() {
    // En un angulo recto de verdad, aunque el modo sea con letra.
    let mut escena = escena_con(vec![
        recta(p(0.0, 0.0), p(200.0, 0.0)),
        recta(p(0.0, 0.0), p(0.0, -200.0)),
    ]);
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Angulo);
    clic(&mut g, &mut escena, p(20.0, -20.0));
    let m = escena
        .visibles()
        .find_map(|e| e.extras.marca_angulo.clone())
        .unwrap();
    assert!(m.recto && m.valor.is_none());
    // La variante de 90: el cuadradito aunque el angulo no mida 90 (el
    // dibujo a mano), y otra vez el atajo la devuelve a la letra.
    let mut escena = triangulo();
    assert_eq!(g.alternar_angulo_recto(), ModoAngulo::Recto);
    clic(&mut g, &mut escena, p(25.0, -8.0));
    let m = escena
        .visibles()
        .find_map(|e| e.extras.marca_angulo.clone())
        .unwrap();
    assert!(m.recto, "a la fuerza, el cuadradito");
    assert!(m.valor.is_none());
    assert_eq!(g.alternar_angulo_recto(), ModoAngulo::Griega);
}

#[test]
fn la_rueda_cambia_la_letra_de_la_ultima_marca_sin_repetir_y_se_deshace() {
    let mut escena = triangulo();
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Angulo);
    clic(&mut g, &mut escena, p(25.0, -8.0)); // α
    clic(&mut g, &mut escena, p(175.0, -8.0)); // β
    // β -> θ (la siguiente), -> γ; y hacia atras salta α, que es de otra.
    assert!(g.cambiar_letra_de_angulo(&mut escena, 1));
    assert_eq!(letras(&escena)[1].as_deref(), Some("θ"));
    assert!(g.cambiar_letra_de_angulo(&mut escena, -1));
    assert_eq!(letras(&escena)[1].as_deref(), Some("β"));
    assert!(g.cambiar_letra_de_angulo(&mut escena, -1));
    assert_eq!(
        letras(&escena)[1].as_deref(),
        Some("κ"),
        "salta α, que ya la tiene otra"
    );
    assert!(escena.deshacer());
    assert_eq!(letras(&escena)[1].as_deref(), Some("β"));
    // Con otra herramienta y nada elegido no toca nada (caso negativo).
    g.tomar_herramienta(Herramienta::Lapiz);
    assert!(!g.cambiar_letra_de_angulo(&mut escena, 1));
}
