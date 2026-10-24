//! **El arco**: la parte de una circunferencia que uno quiere.
//!
//! Porte de `Arco.kt` del movil (172 lineas): `anguloEnElOvalo`,
//! `barridoAcumulado`, `puntosDelArco` y `cajaDelArco`.
//!
//! Se traza como con un transportador de verdad: se pone un ovalo **de
//! referencia** —el instrumento, translucido— y despues se repasa solo el
//! trozo que hace falta. Lo que queda dibujado es ese trozo; la guia se
//! esconde o se borra al final, como el lapiz azul de los planos de toda la
//! vida.
//!
//! Es de las pocas cosas que a mano alzada salen mal siempre. Un arco de
//! ochenta grados a pulso queda ovalado, y con la herramienta de elipse hay
//! que conformarse con el circulo entero o borrar a mano lo que sobra.
//!
//! # Que hace cada pieza y por que esta aqui y no en el pintado
//!
//! Donde empieza el arco y cuanto barre decide **que se ve**, y eso se
//! comprueba sin pantalla. `pintado.rs` tiene su propio muestreo minimo —lo
//! justo para que un arco del movil se VEA— con 64 tramos por vuelta; este
//! modulo es el porte fiel, con los 72 del movil (cinco grados por tramo) y,
//! sobre todo, con las dos piezas que alli no hay y sin las que el arco no se
//! puede ni trazar ni seleccionar: [`barrido_acumulado`] y [`caja_del_arco`].
//!
//! # Radianes, no grados
//!
//! El movil guarda `arcStart`/`arcSweep` en GRADOS y aqui todo lo angular va
//! en radianes, como `angle`. La traduccion la hace el puente
//! (`excalidraw.rs`), no este modulo: quien llame aqui ya trabaja en radianes.

use std::f32::consts::{PI, TAU};

use crate::vector::Punto2;

/// Cuantos tramos tiene una vuelta completa (`ARCO_PASOS` del movil).
///
/// Setenta y dos son cinco grados por tramo: por debajo se ven los vertices en
/// un arco grande, y por encima no se gana nada que el ojo note y si se paga
/// en cada fotograma.
pub const PASOS_DEL_ARCO: usize = 72;

/// El angulo **parametrico** de un punto respecto al centro del ovalo.
///
/// Parametrico y no geometrico: se divide por cada semieje antes del `atan2`,
/// de modo que el angulo es el de la circunferencia de la que sale el ovalo al
/// estirarlo. Es la unica forma de que recorrer el borde de una elipse
/// achatada avance a ritmo constante bajo el cursor; con el angulo geometrico,
/// el trazo corre en los extremos y se arrastra en los lados.
///
/// La rotacion del elemento se deshace antes, asi que un ovalo girado se
/// recorre igual que uno recto. `caja` es `(x, y, ancho, alto)`.
pub fn angulo_en_el_ovalo(caja: (f32, f32, f32, f32), angulo: f32, p: Punto2) -> f32 {
    let (x, y, ancho, alto) = caja;
    let (rx, ry) = (ancho / 2.0, alto / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return 0.0;
    }
    let centro = Punto2::nuevo(x + rx, y + ry);
    let local = p.girar(centro, -angulo);
    ((local.y - centro.y) / ry).atan2((local.x - centro.x) / rx)
}

/// Suma al barrido lo que se ha movido el cursor, **sin saltos**.
///
/// Es el problema clasico del transportador: al pasar por la izquierda del
/// circulo el angulo salta de +pi a -pi, y sumando angulos en bruto el arco se
/// daba la vuelta entera de golpe. Lo que se acumula es la diferencia **mas
/// corta** entre el angulo anterior y el nuevo, que nunca pasa de media vuelta
/// por fotograma —y con eso se pueden dar tantas vueltas como se quiera—.
///
/// Se topa en una vuelta completa: mas que eso vuelve a ser el ovalo entero.
pub fn barrido_acumulado(barrido_previo: f32, angulo_previo: f32, angulo_nuevo: f32) -> f32 {
    let mut delta = angulo_nuevo - angulo_previo;
    while delta > PI {
        delta -= TAU;
    }
    while delta < -PI {
        delta += TAU;
    }
    (barrido_previo + delta).clamp(-TAU, TAU)
}

/// Los puntos del arco, **en coordenadas del documento y sin girar**.
///
/// # Sin girar, y esto costo un fallo en el movil
///
/// Antes salian ya girados, y parecia lo comodo. Pero en este motor la
/// inclinacion no vive en la geometria: la aplica quien pinta, con una matriz
/// (`pintado::girar_orden`). Un arco que ya venia girado se giraba **dos
/// veces**, asi que aparecia en un sitio distinto del que decia su geometria:
/// se picaba donde no se veia, y recortar un ovalo girado dejaba el trozo
/// bueno en otra parte.
///
/// `barrido: None` es **la guia sin repasar** y sale como el ovalo entero, que
/// es lo que menos sorprende y lo que hay que ver para poder repasarlo.
pub fn puntos_del_arco(
    caja: (f32, f32, f32, f32),
    inicio: f32,
    barrido: Option<f32>,
    pasos_por_vuelta: usize,
) -> Vec<Punto2> {
    let (x, y, ancho, alto) = caja;
    let (rx, ry) = (ancho / 2.0, alto / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return Vec::new();
    }
    let barrido = barrido.unwrap_or(TAU);
    // Un barrido nulo no es un arco de un punto: es un arco que no existe.
    // Devolver un punto suelto dejaria en el dibujo una mota que no se puede
    // ni ver ni coger.
    if barrido.abs() < 1e-6 {
        return Vec::new();
    }
    let (cx, cy) = (x + rx, y + ry);
    // Los tramos se reparten en proporcion a lo que abarca: un arco de treinta
    // grados con setenta y dos tramos gastaria por gastar.
    let pasos = ((pasos_por_vuelta as f32 * barrido.abs() / TAU) as usize).max(2);
    (0..=pasos)
        .map(|i| {
            let t = inicio + barrido * i as f32 / pasos as f32;
            Punto2::nuevo(cx + rx * t.cos(), cy + ry * t.sin())
        })
        .collect()
}

/// La caja del **trozo que se ve**, no la del ovalo del que salio.
///
/// Un arco guarda el ovalo entero —`x`, `y`, `ancho`, `alto`— y aparte por
/// donde empieza y cuanto barre: eso es lo que le permite seguir creciendo con
/// el compas sin degenerar en cien rayas. Pero significa que la caja del
/// elemento es **el circulo completo**, y una una de trazo se seleccionaba por
/// un recuadro enorme y vacio: encerrarla con el raton no la cogia nunca,
/// porque el recuadro tenia que contener una caja que no se veia por ninguna
/// parte.
///
/// Sale **exacta** en vez de muestreando: de un arco solo pueden asomar sus
/// dos puntas y los cuatro puntos donde el ovalo toca sus ejes, asi que basta
/// mirar los que caigan dentro del barrido. Sin girar, como todo lo demas: el
/// angulo se pone despues y alrededor del centro del ovalo.
///
/// Devuelve `(x0, y0, x1, y1)`.
pub fn caja_del_arco(
    caja: (f32, f32, f32, f32),
    inicio: f32,
    barrido: Option<f32>,
) -> (f32, f32, f32, f32) {
    let (x, y, ancho, alto) = caja;
    let entera = (x, y, x + ancho, y + alto);
    let (rx, ry) = (ancho / 2.0, alto / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return entera;
    }
    let barrido = barrido.unwrap_or(TAU);
    // La vuelta entera ya es el ovalo: no hay nada que recortar de su caja.
    if barrido.abs() >= TAU - 1e-6 {
        return entera;
    }
    let (cx, cy) = (x + rx, y + ry);
    let desde = inicio.min(inicio + barrido);
    let hasta = inicio.max(inicio + barrido);

    let punto = |t: f32| Punto2::nuevo(cx + rx * t.cos(), cy + ry * t.sin());
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut mete = |p: Punto2| {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    };
    mete(punto(desde));
    mete(punto(hasta));
    // Los cuatro extremos del ovalo, los que caigan dentro del barrido: son
    // los unicos sitios donde la curva puede sobresalir de sus propias puntas.
    let cuarto = PI / 2.0;
    let mut k = (desde / cuarto).ceil();
    while k * cuarto <= hasta {
        mete(punto(k * cuarto));
        k += 1.0;
    }
    (min_x, min_y, max_x, max_y)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La caja de una lista de puntos, para comparar con la caja exacta.
    fn caja_de(p: &[Punto2]) -> (f32, f32, f32, f32) {
        (
            p.iter().map(|q| q.x).fold(f32::MAX, f32::min),
            p.iter().map(|q| q.y).fold(f32::MAX, f32::min),
            p.iter().map(|q| q.x).fold(f32::MIN, f32::max),
            p.iter().map(|q| q.y).fold(f32::MIN, f32::max),
        )
    }

    #[test]
    fn el_angulo_es_parametrico_y_no_geometrico() {
        // En un ovalo muy achatado (200x40) el punto de la derecha es 0 y el
        // de abajo, un cuarto de vuelta. Con el angulo GEOMETRICO, el punto de
        // abajo daria un angulo mucho menor y recorrer el borde correria en
        // los extremos y se arrastraria en los lados.
        let caja = (0.0, 0.0, 200.0, 40.0);
        let derecha = angulo_en_el_ovalo(caja, 0.0, Punto2::nuevo(200.0, 20.0));
        let abajo = angulo_en_el_ovalo(caja, 0.0, Punto2::nuevo(100.0, 40.0));
        assert!(derecha.abs() < 1e-5, "{derecha}");
        assert!((abajo - PI / 2.0).abs() < 1e-5, "{abajo}");
    }

    #[test]
    fn un_ovalo_girado_se_recorre_igual_que_uno_recto() {
        // La rotacion se deshace antes: si no, trazar sobre una guia inclinada
        // devolveria angulos de otro ovalo y el arco saldria descolocado.
        let caja = (0.0, 0.0, 100.0, 100.0);
        let centro = Punto2::nuevo(50.0, 50.0);
        let p = Punto2::nuevo(100.0, 50.0).girar(centro, PI / 3.0);
        let a = angulo_en_el_ovalo(caja, PI / 3.0, p);
        assert!(a.abs() < 1e-4, "{a}");
    }

    #[test]
    fn un_ovalo_sin_tamano_da_angulo_cero_y_no_nan() {
        // Caso negativo: un NaN dentro de la geometria borra el elemento
        // entero de la pantalla sin dar ningun error.
        let a = angulo_en_el_ovalo((10.0, 10.0, 0.0, 0.0), 0.0, Punto2::nuevo(5.0, 5.0));
        assert_eq!(a, 0.0);
    }

    #[test]
    fn el_barrido_no_pega_un_salto_al_cruzar_la_izquierda_del_circulo() {
        // El fallo clasico del transportador: el angulo salta de +pi a -pi y
        // el arco se daba la vuelta entera de golpe.
        let casi = PI - 0.05;
        let pasado = -PI + 0.05;
        let b = barrido_acumulado(1.0, casi, pasado);
        assert!((b - 1.1).abs() < 1e-5, "dio un salto: {b}");
    }

    #[test]
    fn el_barrido_se_topa_en_una_vuelta_entera() {
        // Mas que eso ya es el ovalo, y seguir sumando dejaria un arco que da
        // tres vueltas sobre si mismo.
        assert_eq!(barrido_acumulado(TAU - 0.01, 0.0, 1.0), TAU);
        assert_eq!(barrido_acumulado(-TAU + 0.01, 1.0, 0.0), -TAU);
    }

    #[test]
    fn media_vuelta_de_arco_se_queda_en_media_caja() {
        // La mitad de arriba de un circulo de 100: la caja del trozo QUE SE VE
        // mide 100x50 y no 100x100.
        let caja = (0.0, 0.0, 100.0, 100.0);
        let (x0, y0, x1, y1) = caja_del_arco(caja, PI, Some(PI));
        assert!((x0 - 0.0).abs() < 0.01 && (x1 - 100.0).abs() < 0.01);
        assert!((y0 - 0.0).abs() < 0.01, "{y0}");
        assert!(
            (y1 - 50.0).abs() < 0.01,
            "la caja se comio medio circulo: {y1}"
        );
    }

    #[test]
    fn la_caja_exacta_coincide_con_la_de_los_puntos_muestreados() {
        // Lo que dice que es exacta: para un arco cualquiera, la caja
        // calculada por los extremos del ovalo es la misma que la de setenta y
        // dos puntos. Si no coincidiera, el recuadro de seleccion cortaria el
        // arco o sobraria por algun lado.
        let caja = (10.0, 20.0, 180.0, 90.0);
        for (inicio, barrido) in [
            (0.3f32, 1.2f32),
            (-2.0, 3.5),
            (1.0, -2.2),
            (PI, PI * 0.9),
            (0.0, 0.2),
        ] {
            let exacta = caja_del_arco(caja, inicio, Some(barrido));
            let muestreada = caja_de(&puntos_del_arco(caja, inicio, Some(barrido), 2048));
            for (a, b) in [
                (exacta.0, muestreada.0),
                (exacta.1, muestreada.1),
                (exacta.2, muestreada.2),
                (exacta.3, muestreada.3),
            ] {
                assert!((a - b).abs() < 0.2, "{inicio}/{barrido}: {a} contra {b}");
            }
        }
    }

    #[test]
    fn la_vuelta_entera_conserva_la_caja_del_ovalo() {
        let caja = (10.0, 20.0, 180.0, 90.0);
        assert_eq!(caja_del_arco(caja, 0.0, None), (10.0, 20.0, 190.0, 110.0));
        assert_eq!(
            caja_del_arco(caja, 1.0, Some(TAU)),
            (10.0, 20.0, 190.0, 110.0)
        );
    }

    #[test]
    fn la_guia_sin_repasar_es_el_ovalo_entero() {
        // `barrido: None` es un estado de verdad —«todavia es solo la guia»—
        // y tiene que verse entero para poder repasarlo.
        let p = puntos_del_arco((0.0, 0.0, 100.0, 100.0), 0.0, None, PASOS_DEL_ARCO);
        assert_eq!(p.len(), PASOS_DEL_ARCO + 1);
        let (x0, y0, x1, y1) = caja_de(&p);
        assert!(x0 < 0.1 && y0 < 0.1 && x1 > 99.9 && y1 > 99.9);
    }

    #[test]
    fn un_arco_de_barrido_nulo_no_deja_una_mota_en_el_dibujo() {
        // Caso negativo: con un punto suelto el arco no se ve, no se puede
        // coger y aun asi se guarda para siempre.
        assert!(
            puntos_del_arco((0.0, 0.0, 100.0, 100.0), 0.0, Some(0.0), PASOS_DEL_ARCO).is_empty()
        );
        assert!(puntos_del_arco((0.0, 0.0, 0.0, 0.0), 0.0, None, PASOS_DEL_ARCO).is_empty());
    }

    #[test]
    fn un_arco_corto_gasta_menos_tramos_que_uno_largo_pero_nunca_menos_de_dos() {
        let corto = puntos_del_arco((0.0, 0.0, 100.0, 100.0), 0.0, Some(0.2), PASOS_DEL_ARCO);
        let largo = puntos_del_arco((0.0, 0.0, 100.0, 100.0), 0.0, Some(PI), PASOS_DEL_ARCO);
        assert!(corto.len() < largo.len());
        assert!(corto.len() >= 3, "un arco de dos tramos es una raya");
    }
}
