//! Los marcos: un recuadro con nombre que se lleva consigo lo que encierra.
//!
//! Es el `frame` de Excalidraw, y aqui va **por contencion**: pertenece al
//! marco lo que cae entero dentro de su caja, y se mira cada vez. Excalidraw
//! guarda ademas un `frameId` en cada hijo; no hacerlo asi es deliberado,
//! porque una lista de hijos es una segunda verdad sobre lo mismo y habria
//! que mantenerla al mover, al borrar, al deshacer y al pegar. Lo que se
//! pierde es poco: un elemento que asoma medio fuera no viaja con el marco,
//! que es justo lo que se ve en la pantalla.

//! # La hoja
//!
//! Un marco con `papel` puesto **es una hoja de cuaderno**, y no hay tipo de
//! elemento nuevo para ella. La razon es del movil (`Cuaderno.kt`) y vale
//! igual aqui: el marco ya significaba exactamente esto —«esto de aqui es el
//! dibujo»—, ya recorta lo que ensena un pin y ya decide el encuadre al
//! exportar. Un tipo nuevo habria duplicado todo eso y habria dejado dos
//! conceptos que se parecen y no son el mismo, que es la peor forma de tener
//! uno.
//!
//! Lo que le faltaba al marco para ser una hoja son tres datos, no
//! maquinaria: de que tamano es (`papel`), que pauta trae impresa (`pauta`) y
//! donde cae la siguiente. Los dos primeros ya viajan en `Elemento::extras`
//! desde la tanda cero; el tercero es [`sitio_de_la_hoja_siguiente`].

use crate::elemento::{ColorRgba, Elemento, Figura, PautaHoja, TamanoPapel};
use crate::pintado::Orden;
use crate::vector::Punto2;

/// Lo que mide de ancho una hoja recien puesta, en pixeles de escena.
pub const ANCHO_DE_LA_HOJA: f32 = 820.0;

/// El hueco entre dos hojas seguidas. Lo justo para que se lean como dos y no
/// como una.
pub const HUECO_ENTRE_HOJAS: f32 = 60.0;

/// El paso de la pauta impresa, el mismo que el de la cuadricula del lienzo.
pub const PASO_DE_LA_PAUTA: f32 = 20.0;

/// Lo que se ve la pauta, en parte del trazo.
///
/// Tenue: es papel, no dibujo. Si compite con lo escrito encima deja de ser
/// una guia y pasa a ser ruido —que es lo que le pasa a un cuaderno de rayas
/// mal impreso—.
pub const OPACIDAD_DE_LA_PAUTA: f32 = 0.22;

/// Si un elemento es un marco.
pub fn es_marco(e: &Elemento) -> bool {
    matches!(e.figura, Figura::Marco { .. })
}

/// Las hojas del cuaderno, **en el orden en que se leen**: de arriba abajo y,
/// a igualdad, de izquierda a derecha.
///
/// Sale de la posicion y no de una lista guardada aparte: mover una hoja **es**
/// reordenar el cuaderno, y asi no hay dos sitios que puedan contradecirse.
pub fn hojas_en_orden(elementos: &[Elemento]) -> Vec<&Elemento> {
    let mut hojas: Vec<&Elemento> = elementos
        .iter()
        .filter(|e| es_marco(e) && !e.borrado)
        .collect();
    hojas.sort_by(|a, b| {
        a.y.total_cmp(&b.y)
            .then(a.x.total_cmp(&b.x))
            .then(a.id.cmp(&b.id))
    });
    hojas
}

/// El numero de una hoja dentro del cuaderno, empezando en uno. Cero si no
/// esta. Se cuenta al vuelo por lo mismo que [`hojas_en_orden`]: un numero
/// guardado que no se renumera al mover es un numero que miente.
pub fn numero_de_hoja(elementos: &[Elemento], id: u64) -> usize {
    hojas_en_orden(elementos)
        .iter()
        .position(|h| h.id == id)
        .map_or(0, |i| i + 1)
}

/// **Donde va la hoja siguiente: debajo de la ultima.**
///
/// Con el cuaderno vacio, centrada en `donde_si_no_hay` —lo que se esta
/// mirando— para que la primera hoja aparezca delante y no en el origen del
/// lienzo, que puede estar a media pantalla de distancia.
///
/// Devuelve la caja `(x, y, ancho, alto)` y la pauta heredada. El elemento lo
/// monta el llamante, que es quien reparte identificadores y semillas.
pub fn sitio_de_la_hoja_siguiente(
    elementos: &[Elemento],
    tamano: TamanoPapel,
    donde_si_no_hay: Punto2,
) -> ((f32, f32, f32, f32), PautaHoja) {
    let ultima = hojas_en_orden(elementos).last().copied();
    // Del ancho de la anterior: un cuaderno con hojas de tamanos distintos no
    // es un cuaderno, y si alguien estira una, las siguientes la siguen.
    let ancho = ultima.map_or(ANCHO_DE_LA_HOJA, |u| u.ancho);
    let alto = ancho * tamano.proporcion();
    let x = ultima.map_or(donde_si_no_hay.x - ancho / 2.0, |u| u.x);
    let y = ultima.map_or(donde_si_no_hay.y - alto / 2.0, |u| {
        u.y + u.alto + HUECO_ENTRE_HOJAS
    });
    (
        (x, y, ancho, alto),
        ultima.map_or(PautaHoja::default(), |u| u.extras.pauta),
    )
}

/// **Las rayas de la pauta de una hoja**, en coordenadas de la escena.
///
/// Segmentos y no un camino porque quien pinta —la pantalla, el SVG y el PDF—
/// los quiere de tres formas distintas, y porque asi la cuenta se comprueba
/// sin dispositivo. Los puntos van como segmentos de largo cero: quien pinta
/// decide si son un redondel o una cruz.
///
/// El margen de un paso en el borde no es adorno: una raya pegada al canto de
/// la hoja se confunde con el canto.
pub fn rayas_de_la_pauta(hoja: &Elemento, paso: f32) -> Vec<(Punto2, Punto2)> {
    let pauta = hoja.extras.pauta;
    if pauta == PautaHoja::Lisa || paso <= 0.0 || !es_marco(hoja) {
        return Vec::new();
    }
    let (x1, y1, x2, y2) = hoja.caja();
    // Una hoja mas pequena que su propio paso no lleva pauta: seria una raya
    // sola en medio, que se lee como un tachon.
    if x2 - x1 <= paso || y2 - y1 <= paso {
        return Vec::new();
    }
    let mut salida = Vec::new();
    let mut y = y1 + paso;
    while y < y2 - 1e-6 {
        if pauta == PautaHoja::Puntos {
            let mut x = x1 + paso;
            while x < x2 - 1e-6 {
                salida.push((Punto2::nuevo(x, y), Punto2::nuevo(x, y)));
                x += paso;
            }
        } else {
            salida.push((Punto2::nuevo(x1, y), Punto2::nuevo(x2, y)));
        }
        y += paso;
    }
    if pauta != PautaHoja::Cuadros {
        return salida;
    }
    let mut x = x1 + paso;
    while x < x2 - 1e-6 {
        salida.push((Punto2::nuevo(x, y1), Punto2::nuevo(x, y2)));
        x += paso;
    }
    salida
}

/// Lo que hay que pintar de la hoja **antes** de su contenido: el papel y su
/// pauta.
///
/// Va antes y no despues por lo obvio: el papel debajo de lo escrito. Un
/// marco sin `papel` no devuelve nada, que es como se comportaba hasta ahora y
/// como tiene que seguir comportandose un marco que solo agrupa laminas.
pub fn ordenes_del_papel(e: &Elemento) -> Vec<Orden> {
    if !es_marco(e) || e.extras.papel.is_none() {
        return Vec::new();
    }
    let (x1, y1, x2, y2) = e.caja();
    let mut salida = vec![Orden::Relleno {
        puntos: vec![
            Punto2::nuevo(x1, y1),
            Punto2::nuevo(x2, y1),
            Punto2::nuevo(x2, y2),
            Punto2::nuevo(x1, y2),
        ],
        // Papel blanco y opaco: una hoja translucida sobre el lienzo no es una
        // hoja, y lo que la hoja promete es que lo de debajo no se ve.
        color: ColorRgba::opaco(1.0, 1.0, 1.0),
    }];
    let tinta = ColorRgba {
        a: OPACIDAD_DE_LA_PAUTA,
        ..e.trazo
    };
    for (a, b) in rayas_de_la_pauta(e, PASO_DE_LA_PAUTA) {
        salida.push(Orden::Polilinea {
            // Un punto se pinta como un segmento de largo cero; con el
            // extremo redondo de Direct2D sale el redondel que toca.
            puntos: vec![a, b],
            color: tinta,
            grosor: 1.0,
            estilo: crate::elemento::EstiloTrazo::Solido,
        });
    }
    salida
}

/// La hoja que **contiene** a un punto, si alguna. Es lo que decide en que
/// pagina cae lo que se dibuja.
pub fn hoja_en(elementos: &[Elemento], p: Punto2) -> Option<u64> {
    // De la ultima a la primera: la de encima manda, como en el picado.
    hojas_en_orden(elementos)
        .into_iter()
        .rev()
        .find(|h| {
            let (x1, y1, x2, y2) = h.caja();
            p.x >= x1 && p.x <= x2 && p.y >= y1 && p.y <= y2
        })
        .map(|h| h.id)
}

/// Lo que cae ENTERO dentro del marco. Nunca otro marco: dos marcos que se
/// solapan se arrastrarian el uno al otro y no habria forma de separarlos.
pub fn contenidos(elementos: &[Elemento], marco: &Elemento) -> Vec<u64> {
    let (mx0, my0, mx1, my1) = marco.caja();
    elementos
        .iter()
        .filter(|e| e.id != marco.id && !e.borrado && !es_marco(e))
        .filter(|e| {
            let (x0, y0, x1, y1) = e.caja();
            x0 >= mx0 && x1 <= mx1 && y0 >= my0 && y1 <= my1
        })
        .map(|e| e.id)
        .collect()
}

/// Los identificadores que hay que mover de verdad: los elegidos, mas lo que
/// encierre cada marco elegido.
///
/// Se devuelve sin repetidos y en orden: mover dos veces el mismo elemento lo
/// desplazaria el doble, que es el fallo tipico de esta funcion.
pub fn con_contenidos(elementos: &[Elemento], elegidos: &[u64]) -> Vec<u64> {
    let mut salida: Vec<u64> = elegidos.to_vec();
    for id in elegidos {
        let Some(marco) = elementos
            .iter()
            .find(|e| e.id == *id)
            .filter(|e| es_marco(e))
        else {
            continue;
        };
        for hijo in contenidos(elementos, marco) {
            if !salida.contains(&hijo) {
                salida.push(hijo);
            }
        }
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo};
    use crate::relleno::EstiloRelleno;

    fn caja(id: u64, figura: Figura, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id,
            figura,
            x,
            y,
            ancho,
            alto,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            estilo_relleno: EstiloRelleno::Solido,
            grosor: 1.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    fn escena() -> Vec<Elemento> {
        vec![
            caja(
                1,
                Figura::Marco {
                    nombre: "Lamina 1".into(),
                },
                0.0,
                0.0,
                200.0,
                200.0,
            ),
            // Dentro del todo.
            caja(2, Figura::Rectangulo, 10.0, 10.0, 50.0, 50.0),
            // Asomando por la derecha: no es suyo.
            caja(3, Figura::Rectangulo, 180.0, 10.0, 50.0, 50.0),
            // Lejos.
            caja(4, Figura::Rectangulo, 400.0, 400.0, 10.0, 10.0),
        ]
    }

    #[test]
    fn el_marco_se_queda_con_lo_que_cabe_entero() {
        let es = escena();
        let dentro = contenidos(&es, &es[0]);
        assert_eq!(dentro, vec![2], "el que asoma y el de fuera no son suyos");
    }

    #[test]
    fn dos_marcos_solapados_no_se_arrastran_el_uno_al_otro() {
        // Si un marco contara como hijo de otro, moverlos seria imposible:
        // cada uno se llevaria al otro y se separarian solos.
        let mut es = escena();
        es.push(caja(
            5,
            Figura::Marco {
                nombre: "Dentro".into(),
            },
            20.0,
            20.0,
            40.0,
            40.0,
        ));
        assert_eq!(contenidos(&es, &es[0]), vec![2]);
    }

    #[test]
    fn mover_un_marco_mueve_lo_de_dentro_una_sola_vez() {
        let es = escena();
        // El hijo va elegido TAMBIEN a mano: sin quitar repetidos se moveria
        // el doble que el marco, y se saldria de el.
        let ids = con_contenidos(&es, &[1, 2]);
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn lo_elegido_que_no_es_marco_se_queda_como_esta() {
        let es = escena();
        assert_eq!(con_contenidos(&es, &[4]), vec![4]);
    }

    // -- La hoja ---------------------------------------------------------

    fn hoja(id: u64, y: f32, papel: Option<TamanoPapel>, pauta: PautaHoja) -> Elemento {
        let mut e = caja(
            id,
            Figura::Marco {
                nombre: String::new(),
            },
            0.0,
            y,
            100.0,
            140.0,
        );
        e.extras.papel = papel;
        e.extras.pauta = pauta;
        e
    }

    #[test]
    fn las_hojas_se_leen_de_arriba_abajo_y_moverlas_las_reordena() {
        // Sin indice guardado: arrastrar una hoja mas arriba la pone antes,
        // sin renumerar nada.
        let mut es = vec![
            hoja(1, 300.0, Some(TamanoPapel::A4), PautaHoja::Lisa),
            hoja(2, 0.0, Some(TamanoPapel::A4), PautaHoja::Lisa),
        ];
        assert_eq!(numero_de_hoja(&es, 2), 1);
        assert_eq!(numero_de_hoja(&es, 1), 2);
        es[1].y = 900.0;
        assert_eq!(numero_de_hoja(&es, 1), 1, "moverla la reordeno");
        assert_eq!(numero_de_hoja(&es, 99), 0, "la que no esta no tiene numero");
    }

    #[test]
    fn la_primera_hoja_nace_delante_y_con_la_proporcion_de_su_papel() {
        // Con el cuaderno vacio, centrada en lo que se esta mirando: en el
        // origen del lienzo podria caer a media pantalla de distancia.
        let ((x, y, ancho, alto), pauta) =
            sitio_de_la_hoja_siguiente(&[], TamanoPapel::A4, Punto2::nuevo(500.0, 400.0));
        assert_eq!(ancho, ANCHO_DE_LA_HOJA);
        assert!((alto / ancho - 297.0 / 210.0).abs() < 1e-4, "no es un A4");
        assert_eq!((x + ancho / 2.0, y + alto / 2.0), (500.0, 400.0));
        assert_eq!(pauta, PautaHoja::Lisa);
    }

    #[test]
    fn la_hoja_siguiente_hereda_ancho_y_pauta_y_cae_debajo() {
        // Un cuaderno con hojas de tamanos distintos no es un cuaderno; y si
        // alguien estira una, las siguientes la siguen.
        let es = vec![hoja(1, 50.0, Some(TamanoPapel::A5), PautaHoja::Cuadros)];
        let ((x, y, ancho, _), pauta) =
            sitio_de_la_hoja_siguiente(&es, TamanoPapel::A4, Punto2::nuevo(9999.0, 9999.0));
        assert_eq!(ancho, 100.0, "el ancho de la anterior");
        assert_eq!(x, 0.0, "alineada por la izquierda con la anterior");
        assert_eq!(y, 50.0 + 140.0 + HUECO_ENTRE_HOJAS);
        assert_eq!(pauta, PautaHoja::Cuadros, "la pauta se hereda");
    }

    #[test]
    fn una_hoja_lisa_no_tiene_ni_una_raya() {
        // Caso negativo: la pauta de fabrica es lisa, y una lisa con rayas
        // seria un cuaderno que nadie pidio.
        let h = hoja(1, 0.0, Some(TamanoPapel::A4), PautaHoja::Lisa);
        assert!(rayas_de_la_pauta(&h, PASO_DE_LA_PAUTA).is_empty());
        // Y un paso absurdo tampoco cuelga ni llena la hoja de rayas.
        let h = hoja(1, 0.0, Some(TamanoPapel::A4), PautaHoja::Cuadros);
        assert!(rayas_de_la_pauta(&h, 0.0).is_empty());
        assert!(
            rayas_de_la_pauta(&h, 10_000.0).is_empty(),
            "mas grande que la hoja"
        );
    }

    #[test]
    fn las_tres_pautas_dan_las_rayas_que_tienen_que_dar() {
        // Hoja de 100 x 140 con paso 20: 6 rayas horizontales (20..120, sin
        // llegar a 140) y 4 verticales (20..80, sin llegar a 100).
        let rayada = rayas_de_la_pauta(&hoja(1, 0.0, None, PautaHoja::Rayada), 20.0);
        assert_eq!(rayada.len(), 6);
        assert!(
            rayada.iter().all(|(a, b)| a.y == b.y),
            "rayada va en horizontal"
        );
        let cuadros = rayas_de_la_pauta(&hoja(1, 0.0, None, PautaHoja::Cuadros), 20.0);
        assert_eq!(cuadros.len(), 6 + 4);
        let puntos = rayas_de_la_pauta(&hoja(1, 0.0, None, PautaHoja::Puntos), 20.0);
        assert_eq!(puntos.len(), 6 * 4);
        assert!(
            puntos.iter().all(|(a, b)| a == b),
            "un punto es un segmento de largo cero"
        );
        // Y ninguna raya toca el canto de la hoja.
        let (x1, y1, x2, y2) = hoja(1, 0.0, None, PautaHoja::Cuadros).caja();
        assert!(
            cuadros
                .iter()
                .flat_map(|(a, b)| [a, b])
                .all(|p| (p.x > x1 && p.x < x2) || (p.y > y1 && p.y < y2))
        );
    }

    #[test]
    fn un_marco_sin_papel_no_pinta_hoja_y_uno_con_papel_si() {
        // Un marco que solo agrupa laminas tiene que seguir siendo
        // transparente: pintarle papel blanco taparia el dibujo de debajo.
        let sin = hoja(1, 0.0, None, PautaHoja::Rayada);
        assert!(ordenes_del_papel(&sin).is_empty(), "sin papel no hay hoja");
        let con = hoja(1, 0.0, Some(TamanoPapel::A4), PautaHoja::Rayada);
        let o = ordenes_del_papel(&con);
        assert!(
            matches!(o.first(), Some(Orden::Relleno { .. })),
            "el papel va primero, debajo de todo"
        );
        assert_eq!(o.len(), 1 + 6, "el papel y sus seis rayas");
        // Y la pauta va tenue: si compitiera con lo escrito seria ruido.
        let Orden::Polilinea { color, .. } = &o[1] else {
            panic!("raya esperada");
        };
        assert_eq!(color.a, OPACIDAD_DE_LA_PAUTA);
    }

    #[test]
    fn se_sabe_en_que_hoja_cae_un_punto() {
        let es = vec![
            hoja(1, 0.0, Some(TamanoPapel::A4), PautaHoja::Lisa),
            hoja(2, 300.0, Some(TamanoPapel::A4), PautaHoja::Lisa),
        ];
        assert_eq!(hoja_en(&es, Punto2::nuevo(50.0, 50.0)), Some(1));
        assert_eq!(hoja_en(&es, Punto2::nuevo(50.0, 350.0)), Some(2));
        assert_eq!(
            hoja_en(&es, Punto2::nuevo(50.0, 250.0)),
            None,
            "el hueco entre hojas no es de nadie"
        );
    }
}
