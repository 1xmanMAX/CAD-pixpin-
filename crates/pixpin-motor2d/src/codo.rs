//! **La flecha de codos**: el conector en angulos rectos del organigrama.
//!
//! Porte de `Elbow.kt` del movil (131 lineas), constante a constante.
//!
//! Es la otra forma que tiene que tener una flecha, y sirve para algo distinto
//! que la recta. Una flecha recta **va** de un sitio a otro: dice «esto lleva
//! a esto». Una de codos **estructura**: en un mapa mental o un organigrama
//! las conexiones ortogonales dejan ver la jerarquia de un vistazo, porque
//! todas comparten los mismos ejes y la vista las agrupa sola.
//!
//! El trazado es el conector ortogonal clasico: se sale por el eje dominante,
//! se cruza a mitad de camino y se entra por el mismo eje. Con eso se
//! resuelven los casos que aparecen de verdad al ordenar cajas —al lado,
//! encima, en diagonal— sin meter una busqueda de caminos que aqui no ganaria
//! nada.
//!
//! # Por que la geometria va separada del redondeo
//!
//! Porque asi se puede comprobar el trazado por sus **esquinas exactas**, que
//! es lo unico que decide si el conector es el bueno. Metidos en la misma
//! funcion, comprobar que dos cajas una al lado de la otra se unen con una
//! recta obligaria a buscar la recta entre sesenta puntos de curva.
//!
//! # Lo que este modulo NO hace todavia
//!
//! No lo elige nadie: `Figura::Flecha` (`elemento.rs`) no tiene el campo
//! `elbowed` de Excalidraw, asi que hoy la flecha de codos del movil **nace
//! recta** aqui. La geometria esta entera y probada; lo que falta es el campo
//! y el brazo del puente. Ver el informe del grupo A.

use crate::vector::Punto2;

/// Radio con el que se redondean los codos (`ELBOW_RADIUS` del movil, que a
/// su vez es el de `generateElbowArrowShape` de Excalidraw).
pub const RADIO_DEL_CODO: f32 = 16.0;

/// Debajo de esto los dos puntos se dan por alineados.
///
/// Sin este margen, dos cajas casi a la misma altura se unian con una escalera
/// de tres tramos de un pixel —visualmente un borron, y justo lo contrario de
/// lo que se busca al ordenar un esquema—.
pub const ALINEADO: f32 = 8.0;

/// Cuantos puntos por codo. Seis bastan: es un cuarto de vuelta corto.
pub const PASOS_DEL_CODO: usize = 6;

/// El camino en angulos rectos de `desde` a `hasta`.
///
/// Devuelve los vertices **sin redondear**: el redondeo es cosa de quien
/// dibuja, y separarlo permite comprobar el trazado por sus esquinas exactas.
///
/// Manda el eje en el que mas distancia hay que salvar. Yendo sobre todo a lo
/// ancho se sale en horizontal, se cruza por el medio y se entra en
/// horizontal; yendo a lo alto, al reves. Es lo que hace que dos cajas una al
/// lado de la otra se unan con una raya recta y no con una escalera.
pub fn puntos_de_codo(desde: Punto2, hasta: Punto2) -> Vec<Punto2> {
    let dx = hasta.x - desde.x;
    let dy = hasta.y - desde.y;

    // Practicamente alineadas: una recta es el mejor codo posible.
    if dy.abs() < ALINEADO {
        return vec![desde, Punto2::nuevo(hasta.x, desde.y)];
    }
    if dx.abs() < ALINEADO {
        return vec![desde, Punto2::nuevo(desde.x, hasta.y)];
    }

    if dx.abs() >= dy.abs() {
        let medio = desde.x + dx / 2.0;
        vec![
            desde,
            Punto2::nuevo(medio, desde.y),
            Punto2::nuevo(medio, hasta.y),
            hasta,
        ]
    } else {
        let medio = desde.y + dy / 2.0;
        vec![
            desde,
            Punto2::nuevo(desde.x, medio),
            Punto2::nuevo(hasta.x, medio),
            hasta,
        ]
    }
}

/// Los vertices con los codos redondeados, muestreados a puntos.
///
/// Un codo en pico se ve duro y no es lo que hace el original: recorta la
/// esquina y la cose con un cuarto de vuelta. El radio se acota a **la mitad
/// del tramo mas corto** que llega a esa esquina, porque en un codo entre dos
/// tramos de 10 px un radio de 16 se comeria los dos y el camino se cruzaria
/// consigo mismo.
pub fn codo_redondeado(vertices: &[Punto2], radio: f32) -> Vec<Punto2> {
    if vertices.len() < 3 {
        return vertices.to_vec();
    }
    let mut salida = Vec::with_capacity(vertices.len() + (vertices.len() - 2) * PASOS_DEL_CODO);
    salida.push(vertices[0]);
    for i in 1..vertices.len() - 1 {
        let previo = vertices[i - 1];
        let v = vertices[i];
        let siguiente = vertices[i + 1];

        let r = radio.min(v.distancia(previo).min(siguiente.distancia(v)) / 2.0);
        // Un codo con tramos de longitud cero no se redondea: el recorte
        // valdria mas que el propio tramo y la curva se cruzaria consigo
        // misma.
        if r <= 0.01 {
            salida.push(v);
            continue;
        }

        let entrada = avanzar(v, previo, r);
        let salida_curva = avanzar(v, siguiente, r);
        salida.push(entrada);
        // Cuarto de vuelta con el vertice de tirador: es la misma cuadratica
        // que redondea las puntas del rombo.
        for s in 1..=PASOS_DEL_CODO {
            let t = s as f32 / PASOS_DEL_CODO as f32;
            let u = 1.0 - t;
            salida.push(Punto2::nuevo(
                u * u * entrada.x + 2.0 * u * t * v.x + t * t * salida_curva.x,
                u * u * entrada.y + 2.0 * u * t * v.y + t * t * salida_curva.y,
            ));
        }
    }
    salida.push(vertices[vertices.len() - 1]);
    salida
}

/// `desde` avanzado `distancia` hacia `hacia`, sin pasarse de el.
fn avanzar(desde: Punto2, hacia: Punto2, distancia: f32) -> Punto2 {
    let d = hacia.restar(desde);
    let largo = d.longitud();
    if largo <= 0.0 {
        return desde;
    }
    desde.sumar(d.escalar((distancia / largo).min(1.0)))
}

/// Los puntos que hay que dibujar para una flecha, segun su forma.
///
/// Es el unico sitio donde se decide, asi que quien pinta, quien pica y quien
/// exporta no pueden discrepar sobre por donde va una flecha. Con `codos` a
/// falso devuelve los puntos tal cual, que es lo que hace el movil: una
/// flecha normal no pasa por aqui transformada.
pub fn trazado_de_flecha(puntos: &[Punto2], codos: bool) -> Vec<Punto2> {
    if !codos || puntos.len() < 2 {
        return puntos.to_vec();
    }
    codo_redondeado(
        &puntos_de_codo(puntos[0], puntos[puntos.len() - 1]),
        RADIO_DEL_CODO,
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Si todos los tramos de un camino van por un eje o por el otro, que es
    /// lo unico que hace «de codos» a una flecha de codos.
    fn todo_ortogonal(camino: &[Punto2]) -> bool {
        camino
            .windows(2)
            .all(|p| (p[1].x - p[0].x).abs() < 1e-3 || (p[1].y - p[0].y).abs() < 1e-3)
    }

    #[test]
    fn dos_cajas_a_la_misma_altura_se_unen_con_una_recta_y_no_con_una_escalera() {
        // El margen de ALINEADO: sin el, cuatro pixeles de desnivel daban tres
        // tramos de un pixel, que en pantalla es un borron.
        let c = puntos_de_codo(Punto2::nuevo(0.0, 100.0), Punto2::nuevo(200.0, 104.0));
        assert_eq!(
            c,
            vec![Punto2::nuevo(0.0, 100.0), Punto2::nuevo(200.0, 100.0)]
        );
    }

    #[test]
    fn dos_cajas_en_la_misma_columna_se_unen_con_una_vertical() {
        let c = puntos_de_codo(Punto2::nuevo(50.0, 0.0), Punto2::nuevo(53.0, 300.0));
        assert_eq!(
            c,
            vec![Punto2::nuevo(50.0, 0.0), Punto2::nuevo(50.0, 300.0)]
        );
    }

    #[test]
    fn yendo_sobre_todo_a_lo_ancho_se_sale_y_se_entra_en_horizontal() {
        let c = puntos_de_codo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(200.0, 60.0));
        assert_eq!(
            c,
            vec![
                Punto2::nuevo(0.0, 0.0),
                Punto2::nuevo(100.0, 0.0),
                Punto2::nuevo(100.0, 60.0),
                Punto2::nuevo(200.0, 60.0),
            ]
        );
        assert!(todo_ortogonal(&c));
    }

    #[test]
    fn yendo_sobre_todo_a_lo_alto_se_sale_y_se_entra_en_vertical() {
        // Caso negativo del anterior: si mandara siempre la horizontal, dos
        // cajas una encima de otra se unirian con una ese tumbada.
        let c = puntos_de_codo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(60.0, 200.0));
        assert_eq!(
            c,
            vec![
                Punto2::nuevo(0.0, 0.0),
                Punto2::nuevo(0.0, 100.0),
                Punto2::nuevo(60.0, 100.0),
                Punto2::nuevo(60.0, 200.0),
            ]
        );
        assert!(todo_ortogonal(&c));
    }

    #[test]
    fn el_codo_redondeado_no_se_sale_de_la_esquina_que_recorta() {
        let v = puntos_de_codo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(200.0, 60.0));
        let r = codo_redondeado(&v, RADIO_DEL_CODO);
        assert!(r.len() > v.len(), "no se redondeo nada");
        // Empieza y acaba exactamente donde el camino en pico.
        assert_eq!(r[0], v[0]);
        assert_eq!(*r.last().unwrap(), *v.last().unwrap());
        // Y la curva se queda dentro de la caja del camino: una cuadratica
        // con los vertices de tirador nunca sale del casco de sus puntos.
        for p in &r {
            assert!((-0.01..=200.01).contains(&p.x), "{p:?}");
            assert!((-0.01..=60.01).contains(&p.y), "{p:?}");
        }
        // Y la esquina en pico ya no esta: si estuviera, no se habria
        // redondeado nada aunque hubiera mas puntos.
        assert!(
            !r.iter().any(|p| p.distancia(v[1]) < 1e-4),
            "la esquina sigue en pico"
        );
    }

    #[test]
    fn un_tramo_mas_corto_que_el_radio_no_se_come_el_camino() {
        // Caso negativo, y el que da sentido al acotado: con un radio de 16
        // sobre tramos de 10, redondear las dos esquinas cruzaria el camino
        // consigo mismo.
        let v = vec![
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(10.0, 0.0),
            Punto2::nuevo(10.0, 10.0),
        ];
        let r = codo_redondeado(&v, RADIO_DEL_CODO);
        for p in &r {
            assert!((-0.01..=10.01).contains(&p.x), "se salio por x: {p:?}");
            assert!((-0.01..=10.01).contains(&p.y), "se salio por y: {p:?}");
        }
    }

    #[test]
    fn con_menos_de_tres_vertices_no_hay_nada_que_redondear() {
        let dos = vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(10.0, 0.0)];
        assert_eq!(codo_redondeado(&dos, RADIO_DEL_CODO), dos);
        assert_eq!(codo_redondeado(&[], RADIO_DEL_CODO), Vec::new());
    }

    #[test]
    fn una_flecha_sin_codos_sale_tal_cual_y_con_codos_no() {
        // Caso negativo: si el trazado se aplicara siempre, una flecha curva
        // del movil se abriria aqui convertida en una escalera.
        let p = vec![
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(90.0, 10.0),
            Punto2::nuevo(200.0, 60.0),
        ];
        assert_eq!(trazado_de_flecha(&p, false), p);
        let codos = trazado_de_flecha(&p, true);
        assert_ne!(codos, p);
        // Y el punto de en medio del trazo original no manda: el codo va de la
        // primera punta a la ultima.
        assert_eq!(codos[0], p[0]);
        assert_eq!(*codos.last().unwrap(), p[2]);
    }

    #[test]
    fn una_flecha_de_un_solo_punto_no_entra_en_panico() {
        let uno = vec![Punto2::nuevo(5.0, 5.0)];
        assert_eq!(trazado_de_flecha(&uno, true), uno);
        assert_eq!(trazado_de_flecha(&[], true), Vec::new());
    }
}
