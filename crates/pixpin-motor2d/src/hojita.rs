//! **La hojita** (F6): la nota adhesiva. Puerto de las cuentas de
//! `motor/Hojita.kt` del movil (v0.67) y de lo que pide `c6df577` («la hoja
//! adhesiva: tres dedos hacia arriba, se garabatea y se pega»).
//!
//! En el movil es un lienzo pequeno bajo la barra que sale con tres dedos
//! hacia arriba; lo garabateado **se pega** como una imagen, o se **inserta**
//! en el lienzo como una hoja vectorial agrupada. En el PC no hay tres
//! dedos, pero si algo mejor que pegar dentro: un pin flota por encima de
//! todas las ventanas, que es justo lo que es un recado. Asi que aqui la
//! hojita sale con un atajo del editor, y al acabar se pega como pin o se
//! inserta en el lienzo como en el movil.
//!
//! Aqui van solo las cuentas —que papeles hay y que elementos entran al
//! insertar—; la ventana y el pin son de la aplicacion.

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use crate::escena::Escena;
use crate::relleno::EstiloRelleno;
use crate::vector::Punto2;

/// Los papeles, claros para que la tinta de siempre se lea. El primero, el
/// amarillo de toda la vida. Los mismos que `Hojita.PAPELES` del movil.
pub const PAPELES: [&str; 5] = ["#fff3a3", "#ffffff", "#d7f5dd", "#dbeafe", "#ffe0e6"];

/// El borde de la hoja al insertarla: gris, fino, el `#8a8f98` del movil.
pub const BORDE: ColorRgba = ColorRgba::opaco(
    0x8a as f32 / 255.0,
    0x8f as f32 / 255.0,
    0x98 as f32 / 255.0,
);

/// El color de un papel. Un indice que no existe da el amarillo: una hojita
/// sin papel no tiene sentido.
pub fn papel(indice: usize) -> ColorRgba {
    color_de_hex(PAPELES.get(indice).copied().unwrap_or(PAPELES[0]))
        .unwrap_or(ColorRgba::opaco(1.0, 0.95, 0.64))
}

fn color_de_hex(hex: &str) -> Option<ColorRgba> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let canal = |i: usize| {
        u8::from_str_radix(&h[i..i + 2], 16)
            .ok()
            .map(|v| v as f32 / 255.0)
    };
    Some(ColorRgba::opaco(canal(0)?, canal(2)?, canal(4)?))
}

/// **Lo que entra en el lienzo al insertar**: un rectangulo con el color del
/// papel y, encima, lo anotado, todo en un grupo nuevo y llevado a `en` (la
/// esquina de arriba a la izquierda). `vista` es el trozo de la hojita que
/// se veia, en sus coordenadas `(x0, y0, x1, y1)`: ese es el papel.
///
/// Todo en **un solo paso** de deshacer, como pegar: meter una hojita y
/// tener que deshacerla trazo a trazo seria un castigo. Devuelve los ids
/// nuevos (la hoja la primera) para dejarlos elegidos y poder llevarla a su
/// sitio de un tiron. Sin nada anotado no entra nada: una hoja en blanco en
/// el lienzo es un rectangulo que nadie pidio.
pub fn insertar(
    escena: &mut Escena,
    anotado: &[Elemento],
    vista: (f32, f32, f32, f32),
    color_papel: ColorRgba,
    en: Punto2,
) -> Vec<u64> {
    let vivos: Vec<&Elemento> = anotado.iter().filter(|e| !e.borrado).collect();
    if vivos.is_empty() {
        return Vec::new();
    }
    let (x0, y0, x1, y1) = vista;
    let grupo = format!("hojita-g{}", escena.siguiente_id);
    escena.siguiente_id += 1;
    let (dx, dy) = (en.x - x0, en.y - y0);
    let hoja = Elemento {
        figura: Figura::Rectangulo,
        x: en.x,
        y: en.y,
        ancho: (x1 - x0).abs(),
        alto: (y1 - y0).abs(),
        trazo: BORDE,
        relleno: Some(color_papel),
        estilo_relleno: EstiloRelleno::Solido,
        grosor: 1.0,
        estilo: EstiloTrazo::Solido,
        // Lisa y redondeada, como la del movil: es un papel, no un croquis.
        rugosidad: 0.0,
        redondo: true,
        grupos: vec![grupo.clone()],
        ..Default::default()
    };
    escena.abrir_paso();
    let mut ids = vec![escena.anadir(hoja)];
    for e in vivos {
        let mut copia = e.clone();
        copia.mover(dx, dy);
        // Lo que ya venia agrupado dentro de la hojita sigue agrupado, y
        // ademas entra en el grupo de la hoja: desagrupar una vez devuelve
        // los grupos de dentro, como en Excalidraw.
        copia.grupos.push(grupo.clone());
        copia.borrado = false;
        ids.push(escena.anadir(copia));
    }
    escena.cerrar_paso();
    ids
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn trazo(x: f32, y: f32) -> Elemento {
        Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(x, y), Punto2::nuevo(x + 10.0, y + 5.0)],
                presiones: Vec::new(),
                opciones: None,
            },
            x,
            y,
            ancho: 10.0,
            alto: 5.0,
            ..Default::default()
        }
    }

    #[test]
    fn insertar_mete_la_hoja_debajo_y_lo_anotado_encima_en_un_solo_grupo() {
        let mut escena = Escena::nueva();
        let ids = insertar(
            &mut escena,
            &[trazo(20.0, 30.0), trazo(50.0, 60.0)],
            (0.0, 0.0, 300.0, 200.0),
            papel(0),
            Punto2::nuevo(1000.0, 500.0),
        );
        assert_eq!(ids.len(), 3);
        let hoja = escena.buscar(ids[0]).unwrap();
        assert_eq!(hoja.figura, Figura::Rectangulo);
        assert_eq!(
            (hoja.x, hoja.y, hoja.ancho, hoja.alto),
            (1000.0, 500.0, 300.0, 200.0)
        );
        assert_eq!(hoja.relleno, Some(papel(0)));
        let grupo = &hoja.grupos[0];
        for id in &ids[1..] {
            assert!(escena.buscar(*id).unwrap().grupos.contains(grupo));
        }
        // Lo anotado se lleva con la hoja: el trazo de (20, 30) cae en
        // (1020, 530), en el mismo sitio del papel.
        let primero = escena.buscar(ids[1]).unwrap().puntos().unwrap()[0];
        assert_eq!((primero.x, primero.y), (1020.0, 530.0));
    }

    #[test]
    fn insertar_se_deshace_de_una_vez() {
        let mut escena = Escena::nueva();
        insertar(
            &mut escena,
            &[trazo(0.0, 0.0), trazo(5.0, 5.0), trazo(9.0, 9.0)],
            (0.0, 0.0, 100.0, 100.0),
            papel(1),
            Punto2::nuevo(0.0, 0.0),
        );
        assert_eq!(escena.cuantos_visibles(), 4);
        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 0);
    }

    #[test]
    fn una_hojita_en_blanco_o_toda_borrada_no_mete_nada() {
        let mut escena = Escena::nueva();
        assert!(
            insertar(
                &mut escena,
                &[],
                (0.0, 0.0, 10.0, 10.0),
                papel(0),
                Punto2::nuevo(0.0, 0.0)
            )
            .is_empty()
        );
        let mut borrado = trazo(0.0, 0.0);
        borrado.borrado = true;
        assert!(
            insertar(
                &mut escena,
                &[borrado],
                (0.0, 0.0, 10.0, 10.0),
                papel(0),
                Punto2::nuevo(0.0, 0.0)
            )
            .is_empty()
        );
        assert_eq!(escena.cuantos_visibles(), 0);
        assert!(
            !escena.hay_que_deshacer(),
            "no deja un paso vacio que deshacer"
        );
    }

    #[test]
    fn los_papeles_se_leen_y_uno_que_no_existe_es_el_amarillo() {
        let amarillo = papel(0);
        assert!((amarillo.r - 1.0).abs() < 1e-6 && (amarillo.b - 0xa3 as f32 / 255.0).abs() < 1e-6);
        assert_eq!(papel(99), amarillo);
        assert_eq!(papel(1), ColorRgba::opaco(1.0, 1.0, 1.0));
        assert!(color_de_hex("fff3a3").is_none());
        assert!(color_de_hex("#zzzzzz").is_none());
    }
}
