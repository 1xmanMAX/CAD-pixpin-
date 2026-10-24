//! **La goma, y a que no llega.**
//!
//! Lo primero que hay que decir, porque cambia lo que se puede prometer: **en
//! el movil la goma no borra pixeles ni trozos de trazo**. Se pasa por encima
//! y se lleva el elemento entero —o el grupo entero— que toque, con borrado
//! blando (`isDeleted = true`), recuperable con deshacer. Esta leido en su
//! `Scene.kt:1066-1110` (`eraseAt`, `intocableParaElBorrador`). No hay ahi un
//! selector de «borrar por trozos»: partir un trazo por la mitad es otra
//! herramienta —recortar— y vive en otro sitio.
//!
//! Lo que si tiene la goma son **cuatro protecciones y un modo**, y son las
//! que faltaban aqui:
//!
//! - Nunca se lleva lo **bloqueado**. Eso ya se respetaba, y no por la goma:
//!   lo respeta el picado entero (`impacto::toca`).
//! - Nunca se lleva una **imagen**. Una imagen se mete para dibujar encima, y
//!   la goma se pasa justo por encima de ella: sin esta regla, el primer
//!   trazo que se corrige se lleva la captura debajo.
//! - Nunca se lleva lo que tiene **enlace**: la marca de una zona mandada al
//!   chat es la puerta a un sublienzo, y borrarla de un roce deja el sublienzo
//!   huerfano.
//! - Nunca se lleva los **instrumentos** (plano, recta, espacio). Aqui esa
//!   proteccion sale sola y conviene decir por que: esos tipos todavia entran
//!   por el carril ajeno del puente y no llegan a ser `Elemento`, asi que la
//!   goma no los ve. Cuando alguien los ensene a dibujarse habra que anadirlos
//!   a [`intocable`], y por eso esta escrito aqui y no solo en un commit.
//!
//! Y el **modo guia**: fuera de el no se pueden borrar las lineas de
//! referencia; dentro de el **solo** se borran esas. El PC todavia no modela
//! el campo `reference` del movil —viaja intacto, pero no llega a `Elemento`—,
//! asi que aqui el modo se expresa con un predicado que decide quien es guia.
//! Cuando `reference` exista, ese predicado se cambia por una linea y ya.

use crate::elemento::{Elemento, Figura};
use crate::impacto;
use crate::vector::Punto2;

/// A que va la goma en esta pasada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoBorrador {
    /// Lo normal: se lleva el dibujo y **respeta** las guias.
    #[default]
    Dibujo,
    /// Solo guias: se lleva las lineas de referencia y **no toca** el dibujo.
    Guias,
}

/// Si la goma no puede llevarse este elemento, pase lo que pase.
///
/// Es la traduccion de `intocableParaElBorrador`. No incluye `bloqueado`
/// porque eso ya lo para el picado un escalon antes, y repetirlo aqui seria
/// tener dos verdades sobre lo mismo.
pub fn intocable(e: &Elemento) -> bool {
    matches!(e.figura, Figura::Imagen { .. }) || e.enlace.is_some()
}

/// Quien se lleva la goma al pasar por `p`.
///
/// `es_guia` dice de cada elemento si es una linea de referencia. Mientras el
/// PC no lea el `reference` del movil, lo honesto es que lo diga el llamante
/// (con `|_| false` la goma se comporta como hoy, sin modo guia).
///
/// Devuelve **identificadores**, no elementos: borrar es cosa de la escena,
/// que sabe abrir un paso de historial y arrastrar al grupo entero.
pub fn alcanzados(
    elementos: &[Elemento],
    p: Punto2,
    modo: ModoBorrador,
    es_guia: impl Fn(&Elemento) -> bool,
) -> Vec<u64> {
    elementos
        .iter()
        .filter(|e| impacto::toca(e, p))
        .filter(|e| !intocable(e))
        .filter(|e| match modo {
            // Fuera del modo guia, una guia es andamio y no se borra por
            // rozarla: es justo lo que se tiene debajo mientras se dibuja.
            ModoBorrador::Dibujo => !es_guia(e),
            // Dentro, SOLO las guias: es la pasada de recoger el andamio, y
            // llevarse dibujo con ella seria lo contrario de lo que se pidio.
            ModoBorrador::Guias => es_guia(e),
        })
        .map(|e| e.id)
        .collect()
}

/// Lo mismo, arrastrando **el grupo entero** de cada alcanzado.
///
/// En el movil la goma se lleva el grupo, no la pieza: una flecha con su
/// rotulo es una cosa sola para quien la dibujo, y dejar el rotulo flotando
/// es peor que no haber borrado.
pub fn alcanzados_con_grupo(
    elementos: &[Elemento],
    p: Punto2,
    modo: ModoBorrador,
    es_guia: impl Fn(&Elemento) -> bool,
) -> Vec<u64> {
    let directos = alcanzados(elementos, p, modo, &es_guia);
    let grupos: Vec<&String> = elementos
        .iter()
        .filter(|e| directos.contains(&e.id))
        .flat_map(|e| e.grupos.iter())
        .collect();
    let mut salida = directos;
    for e in elementos {
        if salida.contains(&e.id) || e.borrado || intocable(e) {
            continue;
        }
        // El grupo arrastra, pero **no se salta las protecciones**: si en el
        // grupo hay una imagen, la imagen se queda.
        if e.grupos.iter().any(|g| grupos.contains(&g)) {
            salida.push(e.id);
        }
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn caja(id: u64, figura: Figura, x: f32) -> Elemento {
        Elemento {
            id,
            figura,
            x,
            y: 0.0,
            ancho: 20.0,
            alto: 20.0,
            relleno: Some(crate::elemento::ColorRgba::opaco(1.0, 1.0, 1.0)),
            ..Default::default()
        }
    }

    fn dentro(x: f32) -> Punto2 {
        Punto2::nuevo(x + 10.0, 10.0)
    }

    #[test]
    fn la_goma_se_lleva_una_figura_normal() {
        let es = vec![caja(1, Figura::Rectangulo, 0.0)];
        assert_eq!(
            alcanzados(&es, dentro(0.0), ModoBorrador::Dibujo, |_| false),
            vec![1]
        );
    }

    #[test]
    fn la_goma_no_se_lleva_la_imagen_sobre_la_que_se_dibuja() {
        // El caso que mas duele: se corrige un trazo encima de una captura y
        // la captura desaparece.
        let es = vec![caja(1, Figura::Imagen { id_objeto: 9 }, 0.0)];
        assert!(
            alcanzados(&es, dentro(0.0), ModoBorrador::Dibujo, |_| false).is_empty(),
            "una imagen no se borra con la goma"
        );
    }

    #[test]
    fn la_goma_no_se_lleva_la_puerta_a_un_sublienzo() {
        let mut e = caja(1, Figura::Rectangulo, 0.0);
        e.enlace = Some("otra-hoja".into());
        assert!(alcanzados(&[e], dentro(0.0), ModoBorrador::Dibujo, |_| false).is_empty());
    }

    #[test]
    fn la_goma_no_se_lleva_lo_bloqueado() {
        let mut e = caja(1, Figura::Rectangulo, 0.0);
        e.bloqueado = true;
        assert!(alcanzados(&[e], dentro(0.0), ModoBorrador::Dibujo, |_| false).is_empty());
    }

    #[test]
    fn fuera_del_modo_guia_una_guia_no_se_borra_y_dentro_solo_se_borra_ella() {
        let raya = Figura::Linea {
            puntos: vec![Punto2::nuevo(0.0, 10.0), Punto2::nuevo(20.0, 10.0)],
        };
        let es = vec![caja(1, Figura::Rectangulo, 0.0), caja(2, raya, 0.0)];
        let guia = |e: &Elemento| e.id == 2;
        // La linea esta debajo del dibujo y no se la lleva por rozarla.
        assert_eq!(
            alcanzados(&es, dentro(0.0), ModoBorrador::Dibujo, guia),
            vec![1]
        );
        // Y en la pasada de recoger el andamio, al reves.
        assert_eq!(
            alcanzados(&es, dentro(0.0), ModoBorrador::Guias, guia),
            vec![2]
        );
    }

    #[test]
    fn la_goma_se_lleva_el_grupo_entero() {
        let mut a = caja(1, Figura::Rectangulo, 0.0);
        let mut b = caja(2, Figura::Rectangulo, 500.0);
        a.grupos = vec!["g".into()];
        b.grupos = vec!["g".into()];
        let es = vec![a, b];
        let r = alcanzados_con_grupo(&es, dentro(0.0), ModoBorrador::Dibujo, |_| false);
        assert!(r.contains(&1) && r.contains(&2), "{r:?}");
    }

    #[test]
    fn el_grupo_no_arrastra_a_la_imagen_que_lleva_dentro() {
        // Caso negativo del anterior, y el que hace que la regla sirva de
        // algo: si el grupo se saltara las protecciones, agrupar un dibujo
        // con su captura convertiria la captura en borrable.
        let mut a = caja(1, Figura::Rectangulo, 0.0);
        let mut foto = caja(2, Figura::Imagen { id_objeto: 3 }, 500.0);
        a.grupos = vec!["g".into()];
        foto.grupos = vec!["g".into()];
        let es = vec![a, foto];
        let r = alcanzados_con_grupo(&es, dentro(0.0), ModoBorrador::Dibujo, |_| false);
        assert_eq!(r, vec![1], "la foto del grupo se queda");
    }

    #[test]
    fn pasar_por_donde_no_hay_nada_no_borra_nada() {
        let es = vec![caja(1, Figura::Rectangulo, 0.0)];
        assert!(
            alcanzados(
                &es,
                Punto2::nuevo(900.0, 900.0),
                ModoBorrador::Dibujo,
                |_| { false }
            )
            .is_empty()
        );
    }
}
