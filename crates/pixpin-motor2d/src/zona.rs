//! **La herramienta Zona** (F8; `Tool.ZONA` del movil, pedida el
//! 13-sep-2026 y con la copia recortada en redondo desde la v0.67).
//!
//! Se arrastra un rectangulo y sale **una foto de lo que hay dentro** —el
//! papel de fondo (la pagina del PDF, la captura) y lo dibujado encima—,
//! puesta encima un poco corrida y elegida, para arrastrarla a otro sitio. La
//! herramienta se queda puesta: se pueden sacar varias seguidas, y con la
//! Zona en la mano una copia ya sacada se arrastra sin cambiar de
//! herramienta (`DrawController.pointerDown`, `Tool.ZONA`).
//!
//! La foto va **recortada con las esquinas redondas y con su filo pintado
//! dentro** (`enRedondoYConFilo`): el marco de antes era un rectangulo
//! redondeado puesto sobre una foto de esquinas cuadradas, y los picos
//! asomaban por fuera. El filo es negro sobre papel claro y blanco sobre
//! papel de noche, para que se vea.
//!
//! Aqui va lo que no necesita pantalla: las medidas, el recorte en redondo
//! de los pixeles y la figura que la pone. La foto la saca quien pinta (el
//! editor, con `exportar::de_una_zona`).

use crate::elemento::{ColorRgba, Elemento};

/// El prefijo del grupo de una copia de zona. Es lo que reconoce una copia
/// para arrastrarla con la Zona puesta (`GRUPO_DE_ZONA` del movil).
pub const GRUPO_DE_ZONA: &str = "zona-";

/// Lo minimo que tiene que medir en pantalla una zona para que cuente, y no
/// sea un clic (`TAMANO_MINIMO_DE_ZONA`).
pub const TAMANO_MINIMO_PX: f32 = 24.0;

/// El lado mayor de la foto, en pixeles (`LADO_DE_LA_FOTO_DE_ZONA`): se lee
/// igual de nitida que acercandose.
pub const LADO_DE_LA_FOTO: f32 = 1600.0;

/// Cuanto se corre la copia de su sitio, en pixeles de pantalla.
pub const CORRIMIENTO_PX: f32 = 24.0;

/// A cuantos pixeles por unidad del dibujo se saca la foto de `caja`: entre
/// uno y cuatro, como el movil.
// `!(lado > 0.0)` es a proposito: tambien atrapa un NaN.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn escala_de_la_foto(caja: (f32, f32, f32, f32)) -> f32 {
    let lado = (caja.2 - caja.0).max(caja.3 - caja.1);
    if !(lado > 0.0) {
        return 1.0;
    }
    (LADO_DE_LA_FOTO / lado).clamp(1.0, 4.0)
}

/// Si el rectangulo arrastrado cuenta como zona a este aumento.
pub fn cuenta(a: crate::vector::Punto2, b: crate::vector::Punto2, zoom: f32) -> bool {
    let minimo = TAMANO_MINIMO_PX / zoom.max(0.0001);
    (a.x - b.x).abs() >= minimo && (a.y - b.y).abs() >= minimo
}

/// La caja ordenada de dos esquinas.
pub fn caja(a: crate::vector::Punto2, b: crate::vector::Punto2) -> (f32, f32, f32, f32) {
    (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
}

/// El filo que se ve sobre ese papel: blanco en papel de noche, casi negro
/// (`#1E1E1E`) en papel claro.
pub fn color_del_filo(papel_de_noche: bool) -> ColorRgba {
    if papel_de_noche {
        ColorRgba::opaco(1.0, 1.0, 1.0)
    } else {
        ColorRgba::opaco(30.0 / 255.0, 30.0 / 255.0, 30.0 / 255.0)
    }
}

/// El radio y el grueso del filo para una foto de `w` x `h`: un 5 % del lado
/// corto entre 10 y 36 px (el de antes, un cuarto del lado, era «muy
/// redondeado» para un recorte) y un 0,6 % entre 2 y 5.
pub fn radio_y_filo(w: u32, h: u32) -> (f32, f32) {
    let corto = w.min(h) as f32;
    (
        (corto * 0.05).clamp(10.0, 36.0),
        (corto * 0.006).clamp(2.0, 5.0),
    )
}

/// Distancia con signo de `(x, y)` al borde de un rectangulo redondeado
/// centrado en el origen con semilados `(a, b)` y radio `r`: negativa dentro.
fn distancia_redondeada(x: f32, y: f32, a: f32, b: f32, r: f32) -> f32 {
    let qx = x.abs() - (a - r);
    let qy = y.abs() - (b - r);
    let fuera = (qx.max(0.0)).hypot(qy.max(0.0));
    let dentro = qx.max(qy).min(0.0);
    fuera + dentro - r
}

/// **La foto recortada en redondo y con su filo dentro**, sobre sus pixeles
/// RGBA sin premultiplicar. Fuera de las esquinas queda transparente; el
/// borde, suavizado a un pixel; y el filo, `grosor` pixeles hacia dentro.
pub fn en_redondo_y_con_filo(rgba: &mut [u8], w: u32, h: u32, filo: ColorRgba) {
    if w == 0 || h == 0 || rgba.len() != (w * h * 4) as usize {
        return;
    }
    let (radio, grosor) = radio_y_filo(w, h);
    let (a, b) = (w as f32 / 2.0, h as f32 / 2.0);
    let color = [filo.r * 255.0, filo.g * 255.0, filo.b * 255.0];
    for y in 0..h {
        // Solo las filas de las esquinas y el filo tienen algo que hacer: en
        // el medio de la foto, solo los `grosor` pixeles de cada lado.
        for x in 0..w {
            let (cx, cy) = (x as f32 + 0.5 - a, y as f32 + 0.5 - b);
            let d = distancia_redondeada(cx, cy, a, b, radio);
            if d < -grosor - 1.0 {
                continue;
            }
            let i = ((y * w + x) * 4) as usize;
            // Cuanto del pixel queda dentro de la foto (suavizado de un px).
            let cubierto = (0.5 - d).clamp(0.0, 1.0);
            // Y cuanto es filo: la franja de `grosor` junto al borde.
            let en_filo = (d + grosor + 0.5).clamp(0.0, 1.0);
            for k in 0..3 {
                let v = rgba[i + k] as f32;
                rgba[i + k] = (v + (color[k] - v) * en_filo).round().clamp(0.0, 255.0) as u8;
            }
            let alfa = rgba[i + 3] as f32 * cubierto;
            // Donde hay filo el pixel es opaco aunque la foto no lo fuera.
            let alfa = alfa.max(255.0 * en_filo * cubierto);
            rgba[i + 3] = alfa.round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Si un elemento es de una copia de zona (lleva su grupo).
pub fn es_copia(e: &Elemento) -> bool {
    e.grupos.iter().any(|g| g.starts_with(GRUPO_DE_ZONA))
}

/// El nombre del grupo de una copia nueva, unico en la escena.
pub fn grupo_nuevo(escena: &crate::escena::Escena) -> String {
    format!("{GRUPO_DE_ZONA}{}", escena.siguiente_id.max(1))
}

// ---------------------------------------------------------------------------
// La zona mandada al chat: la marca con enlace
// ---------------------------------------------------------------------------

/// El azul de la marca de una zona mandada al chat y de su icono de enlace
/// (`COLOR_DE_LA_ZONA` del movil, `#1971c2`).
pub const COLOR_DE_LA_ZONA: ColorRgba = ColorRgba {
    r: 0x19 as f32 / 255.0,
    g: 0x71 as f32 / 255.0,
    b: 0xc2 as f32 / 255.0,
    a: 1.0,
};

/// El radio del redondel del icono de enlace, en pixeles de pantalla
/// (`RADIO_DEL_ICONO_DE_ENLACE`): mide lo mismo a cualquier aumento.
pub const RADIO_DEL_ICONO_DE_ENLACE: f32 = 14.0;

/// **La marca que queda en el origen** cuando la zona va al chat
/// (`DrawController.marcarZona`): un rectangulo discontinuo del tamano de la
/// zona, azul, sin relleno y sin temblor, con el enlace al dibujo de su
/// sublienzo. El borrador no la quita (`borrador::intocable`) y la foto de
/// otra zona no la saca (`exportar::de_una_zona`).
pub fn marca(caja: (f32, f32, f32, f32), dibujo: &str) -> Elemento {
    Elemento {
        figura: crate::elemento::Figura::Rectangulo,
        x: caja.0,
        y: caja.1,
        ancho: caja.2 - caja.0,
        alto: caja.3 - caja.1,
        trazo: COLOR_DE_LA_ZONA,
        relleno: None,
        grosor: 2.0,
        estilo: crate::elemento::EstiloTrazo::Discontinuo,
        rugosidad: 0.0,
        enlace: Some(dibujo.to_string()),
        ..Default::default()
    }
}

/// Pone la marca en la escena como **un solo paso de deshacer**: en el
/// movil `marcarZona` va al historial y se deshace como cualquier trazo.
/// Devuelve el id de la marca.
pub fn marcar(escena: &mut crate::escena::Escena, caja: (f32, f32, f32, f32), dibujo: &str) -> u64 {
    escena.abrir_paso();
    let id = escena.anadir(marca(caja, dibujo));
    escena.cerrar_paso();
    id
}

/// Donde va el icono de enlace de un elemento: la esquina de arriba a la
/// derecha de su caja, como en el movil (`pintarIconoDeEnlace`). `None` si
/// no lleva enlace o esta borrado.
pub fn icono_de_enlace(e: &Elemento) -> Option<crate::vector::Punto2> {
    if e.borrado || e.enlace.is_none() {
        return None;
    }
    let (_, y0, x1, _) = e.caja();
    Some(crate::vector::Punto2::nuevo(x1, y0))
}

/// **El enlace cuyo icono esta bajo el puntero** (`enlaceBajoElDedo` del
/// movil): el redondel de la esquina de arriba a la derecha, con un 40 % de
/// holgura para acertarlo, medido en pixeles de pantalla (`zoom` = pixeles por
/// unidad). Si hay varios, el de encima, que es el que se ve. El icono es el
/// sitio que se pulsa: sin esto un clic en el ahi pintaba un punto con el
/// lapiz, porque cae fuera del recuadro.
pub fn enlace_bajo_el_puntero(
    elementos: &[Elemento],
    p: crate::vector::Punto2,
    zoom: f32,
) -> Option<&str> {
    let radio = RADIO_DEL_ICONO_DE_ENLACE * 1.4 / zoom.max(0.0001);
    elementos
        .iter()
        .rev()
        .find(|e| icono_de_enlace(e).is_some_and(|c| (p.x - c.x).hypot(p.y - c.y) <= radio))
        .and_then(|e| e.enlace.as_deref())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::vector::Punto2;

    #[test]
    fn el_icono_de_enlace_se_acierta_con_holgura_y_mide_en_pantalla() {
        let m = marca((10.0, 20.0, 110.0, 70.0), "foto-1");
        let v = [m];
        // Justo en la esquina, y un poco fuera del recuadro.
        assert_eq!(
            enlace_bajo_el_puntero(&v, Punto2::nuevo(110.0, 20.0), 1.0),
            Some("foto-1")
        );
        assert_eq!(
            enlace_bajo_el_puntero(&v, Punto2::nuevo(125.0, 10.0), 1.0),
            Some("foto-1")
        );
        // Caso negativo: a 25 px de pantalla ya no.
        assert_eq!(
            enlace_bajo_el_puntero(&v, Punto2::nuevo(135.0, 20.0), 1.0),
            None
        );
        // Acercado al doble, 25 px de pantalla son 12,5 del dibujo: si.
        assert_eq!(
            enlace_bajo_el_puntero(&v, Punto2::nuevo(122.5, 20.0), 2.0),
            None
        );
        assert_eq!(
            enlace_bajo_el_puntero(&v, Punto2::nuevo(118.0, 20.0), 2.0),
            Some("foto-1")
        );
        // Caso negativo: una borrada o sin enlace no tiene icono.
        let mut b = v[0].clone();
        b.borrado = true;
        assert_eq!(
            enlace_bajo_el_puntero(&[b], Punto2::nuevo(110.0, 20.0), 1.0),
            None
        );
    }

    #[test]
    fn con_dos_iconos_juntos_gana_el_de_encima() {
        let abajo = marca((0.0, 0.0, 100.0, 50.0), "abajo");
        let arriba = marca((0.0, 0.0, 100.0, 50.0), "arriba");
        assert_eq!(
            enlace_bajo_el_puntero(&[abajo, arriba], Punto2::nuevo(100.0, 0.0), 1.0),
            Some("arriba")
        );
    }

    #[test]
    fn la_marca_de_una_zona_mandada_es_un_recuadro_discontinuo_azul_con_su_enlace() {
        let m = marca((10.0, 20.0, 110.0, 70.0), "foto-123");
        assert_eq!(m.figura, crate::elemento::Figura::Rectangulo);
        assert_eq!((m.x, m.y, m.ancho, m.alto), (10.0, 20.0, 100.0, 50.0));
        assert_eq!(m.estilo, crate::elemento::EstiloTrazo::Discontinuo);
        assert_eq!(m.trazo, COLOR_DE_LA_ZONA);
        assert_eq!(m.relleno, None, "no tapa lo de debajo");
        assert_eq!(m.rugosidad, 0.0);
        assert_eq!(m.enlace.as_deref(), Some("foto-123"));
        // Y el borrador no se la lleva, como en el movil.
        assert!(crate::borrador::intocable(&m));
    }

    #[test]
    fn marcar_la_zona_se_deshace_de_una_vez() {
        let mut e = crate::escena::Escena::nueva();
        let id = marcar(&mut e, (0.0, 0.0, 50.0, 50.0), "foto-1");
        assert!(e.buscar(id).is_some_and(|x| !x.borrado));
        assert!(e.deshacer());
        assert!(
            e.buscar(id).is_none_or(|x| x.borrado),
            "un deshacer la quita"
        );
        // Caso negativo: no quedaba otro paso detras.
        assert!(!e.deshacer());
    }

    #[test]
    fn el_icono_de_enlace_va_arriba_a_la_derecha_y_solo_si_hay_enlace() {
        let m = marca((10.0, 20.0, 110.0, 70.0), "foto-1");
        assert_eq!(icono_de_enlace(&m), Some(Punto2::nuevo(110.0, 20.0)));
        let mut sin = m.clone();
        sin.enlace = None;
        assert_eq!(icono_de_enlace(&sin), None);
        let mut borrada = m;
        borrada.borrado = true;
        assert_eq!(icono_de_enlace(&borrada), None);
    }

    fn foto(w: u32, h: u32) -> Vec<u8> {
        // Roja y opaca entera.
        (0..w * h).flat_map(|_| [200u8, 20, 20, 255]).collect()
    }

    fn px(v: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        [v[i], v[i + 1], v[i + 2], v[i + 3]]
    }

    #[test]
    fn la_esquina_queda_transparente_y_el_medio_intacto() {
        let (w, h) = (400, 300);
        let mut v = foto(w, h);
        en_redondo_y_con_filo(&mut v, w, h, color_del_filo(false));
        assert_eq!(px(&v, w, 0, 0)[3], 0, "la punta de la esquina se va");
        assert_eq!(px(&v, w, w - 1, h - 1)[3], 0);
        assert_eq!(
            px(&v, w, 200, 150),
            [200, 20, 20, 255],
            "el medio no se toca"
        );
    }

    #[test]
    fn el_filo_va_dentro_y_del_color_del_papel() {
        let (w, h) = (400, 300);
        let mut v = foto(w, h);
        en_redondo_y_con_filo(&mut v, w, h, color_del_filo(false));
        // En el medio del lado izquierdo, el primer pixel es filo casi negro.
        let borde = px(&v, w, 0, 150);
        assert!(borde[0] < 60 && borde[3] > 200, "{borde:?}");
        // Sobre papel de noche, blanco.
        let mut n = foto(w, h);
        en_redondo_y_con_filo(&mut n, w, h, color_del_filo(true));
        let borde = px(&n, w, 0, 150);
        assert!(borde[1] > 200, "{borde:?}");
        // Caso negativo: a diez pixeles del borde ya no hay filo.
        assert_eq!(px(&v, w, 10, 150), [200, 20, 20, 255]);
    }

    #[test]
    fn el_radio_es_suave_y_tiene_tope() {
        assert_eq!(radio_y_filo(100, 100), (10.0, 2.0));
        assert_eq!(radio_y_filo(4000, 3000).0, 36.0);
        assert!((radio_y_filo(400, 300).0 - 15.0).abs() < 1e-4);
    }

    #[test]
    fn un_clic_no_es_una_zona_y_la_foto_se_saca_nitida() {
        let a = Punto2::nuevo(0.0, 0.0);
        assert!(!cuenta(a, Punto2::nuevo(10.0, 50.0), 1.0));
        assert!(cuenta(a, Punto2::nuevo(30.0, 30.0), 1.0));
        // Acercado al doble, 24 px de pantalla son 12 del dibujo.
        assert!(cuenta(a, Punto2::nuevo(13.0, 13.0), 2.0));
        assert_eq!(escala_de_la_foto((0.0, 0.0, 400.0, 100.0)), 4.0);
        assert_eq!(escala_de_la_foto((0.0, 0.0, 3200.0, 100.0)), 1.0);
        assert_eq!(escala_de_la_foto((0.0, 0.0, 800.0, 100.0)), 2.0);
    }
}
