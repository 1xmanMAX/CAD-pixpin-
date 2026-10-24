//! **Recortar una imagen del lienzo**: quedarse con un trozo de la foto.
//!
//! El lienzo ya sabia PINTAR una imagen recortada (`Extras::recorte`, el
//! `crop` de Excalidraw que llega de la web y que el movil pinta y exporta),
//! pero nada de aqui lo creaba. Esta es la operacion que lo crea, en el motor
//! y no en quien la pide: la usa el asistente («recorta la parte de arriba de
//! la foto») y la puede usar mañana un tirador de recorte del editor, con el
//! mismo resultado y el mismo paso de deshacer.
//!
//! El recorte se guarda como Excalidraw: en pixeles del ORIGINAL, con el
//! tamano natural al lado. Recortar dos veces compone: el segundo trozo se
//! mide sobre lo que ya se veia, no sobre la foto entera, que es lo que
//! espera quien mira la pantalla.
//!
//! La caja del elemento encoge con el trozo y lo que queda **no se mueve**: la
//! parte de la foto que se conserva sigue exactamente donde estaba.

use crate::elemento::{Elemento, Figura, RecorteImagen};
use crate::escena::Escena;
use crate::vector::Punto2;

/// Lo minimo que se puede conservar de cada lado (una fraccion): menos es
/// una raya de pixeles que no se puede ni volver a coger.
pub const MINIMO: f32 = 0.01;

/// **Deja de `e` solo el trozo `conservar`** = `(x, y, ancho, alto)` en
/// fracciones (0 a 1) de lo que se ve ahora. `natural` es el tamano en
/// pixeles del bitmap, si quien tiene la imagen lo sabe; sin el se mide
/// contra la caja (las proporciones son las mismas).
///
/// `false` si no es una imagen o el trozo no tiene sentido (fuera de 0..1,
/// vacio): entonces `e` no se toca.
pub fn recortar_imagen(
    e: &mut Elemento,
    conservar: (f32, f32, f32, f32),
    natural: Option<(f32, f32)>,
) -> bool {
    if !matches!(e.figura, Figura::Imagen { .. }) {
        return false;
    }
    let (fx, fy, fw, fh) = conservar;
    let fx = fx.clamp(0.0, 1.0);
    let fy = fy.clamp(0.0, 1.0);
    let fw = fw.min(1.0 - fx);
    let fh = fh.min(1.0 - fy);
    if ![fx, fy, fw, fh].iter().all(|v| v.is_finite()) || fw < MINIMO || fh < MINIMO {
        return false;
    }
    if e.ancho <= 0.0 || e.alto <= 0.0 {
        return false;
    }
    // Lo que se ve ahora, en pixeles del original.
    let (bx, by, bw, bh, nw, nh) = match e.extras.recorte {
        Some(r)
            if r.ancho > 0.0 && r.alto > 0.0 && r.ancho_natural > 0.0 && r.alto_natural > 0.0 =>
        {
            (r.x, r.y, r.ancho, r.alto, r.ancho_natural, r.alto_natural)
        }
        _ => {
            let (nw, nh) = natural
                .filter(|(w, h)| *w > 0.0 && *h > 0.0)
                .unwrap_or((e.ancho, e.alto));
            (0.0, 0.0, nw, nh, nw, nh)
        }
    };
    e.extras.recorte = Some(RecorteImagen {
        x: bx + fx * bw,
        y: by + fy * bh,
        ancho: fw * bw,
        alto: fh * bh,
        ancho_natural: nw,
        alto_natural: nh,
    });
    // La caja: el trozo que queda no se mueve. Con la foto girada, el centro
    // nuevo se gira alrededor del viejo.
    let centro_viejo = Punto2::nuevo(e.x + e.ancho / 2.0, e.y + e.alto / 2.0);
    let (ancho, alto) = (e.ancho * fw, e.alto * fh);
    let local = Punto2::nuevo(
        e.x + e.ancho * fx + ancho / 2.0,
        e.y + e.alto * fy + alto / 2.0,
    );
    let centro = if e.angulo == 0.0 {
        local
    } else {
        local.girar(centro_viejo, e.angulo)
    };
    e.x = centro.x - ancho / 2.0;
    e.y = centro.y - alto / 2.0;
    e.ancho = ancho;
    e.alto = alto;
    e.tocar();
    true
}

/// Lo mismo sobre la escena, como un paso de deshacer.
pub fn recortar_imagen_en_escena(
    escena: &mut Escena,
    id: u64,
    conservar: (f32, f32, f32, f32),
    natural: Option<(f32, f32)>,
) -> bool {
    let Some(mut copia) = escena.buscar(id).filter(|e| !e.borrado).cloned() else {
        return false;
    };
    if !recortar_imagen(&mut copia, conservar, natural) {
        return false;
    }
    escena.abrir_paso();
    escena.apuntar_edicion(id);
    if let Some(e) = escena.buscar_mut(id) {
        *e = copia;
    }
    escena.cerrar_paso();
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn foto() -> Elemento {
        Elemento {
            figura: Figura::Imagen { id_objeto: 7 },
            x: 100.0,
            y: 50.0,
            ancho: 400.0,
            alto: 200.0,
            ..Default::default()
        }
    }

    #[test]
    fn la_mitad_izquierda_conserva_su_sitio() {
        let mut e = foto();
        assert!(recortar_imagen(
            &mut e,
            (0.0, 0.0, 0.5, 1.0),
            Some((800.0, 400.0))
        ));
        assert_eq!((e.x, e.y, e.ancho, e.alto), (100.0, 50.0, 200.0, 200.0));
        let r = e.extras.recorte.unwrap();
        assert_eq!((r.x, r.ancho, r.ancho_natural), (0.0, 400.0, 800.0));
    }

    #[test]
    fn recortar_dos_veces_mide_sobre_lo_que_se_ve() {
        let mut e = foto();
        recortar_imagen(&mut e, (0.5, 0.0, 0.5, 1.0), Some((800.0, 400.0)));
        // Ahora la mitad derecha; de ella, la mitad de abajo.
        recortar_imagen(&mut e, (0.0, 0.5, 1.0, 0.5), None);
        let r = e.extras.recorte.unwrap();
        assert_eq!((r.x, r.y, r.ancho, r.alto), (400.0, 200.0, 400.0, 200.0));
        assert_eq!((e.x, e.y, e.ancho, e.alto), (300.0, 150.0, 200.0, 100.0));
    }

    #[test]
    fn lo_que_no_es_imagen_o_un_trozo_vacio_no_se_toca() {
        // Casos negativos.
        let mut r = Elemento {
            ancho: 10.0,
            alto: 10.0,
            ..Default::default()
        };
        assert!(!recortar_imagen(&mut r, (0.0, 0.0, 0.5, 0.5), None));
        let mut e = foto();
        let antes = e.clone();
        assert!(!recortar_imagen(&mut e, (0.2, 0.2, 0.0, 0.5), None));
        assert!(!recortar_imagen(&mut e, (1.0, 0.0, 0.5, 0.5), None));
        assert_eq!(e, antes);
    }

    #[test]
    fn en_la_escena_es_un_paso_de_deshacer() {
        let mut s = Escena::nueva();
        let id = s.anadir(foto());
        assert!(recortar_imagen_en_escena(
            &mut s,
            id,
            (0.0, 0.0, 1.0, 0.5),
            None
        ));
        assert_eq!(s.buscar(id).unwrap().alto, 100.0);
        assert!(s.deshacer());
        assert_eq!(s.buscar(id).unwrap().alto, 200.0);
        assert!(s.buscar(id).unwrap().extras.recorte.is_none());
    }
}
