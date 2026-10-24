//! **Piezas de dibujo tecnico** que comparten las figuras de arquitectura,
//! vegetacion, mobiliario, ingenieria civil y derecho: formas cerradas,
//! rayado de seccion (poche), copas en festón, cajas en isometrica,
//! cajas con texto y flechas de esquema.

use super::{Al, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::ColorRgba;

/// La tinta de los planos.
pub(super) const TINTA: ColorRgba = hex(0x2b2f3a);
/// La tinta fina (mobiliario, detalles).
pub(super) const FINA: ColorRgba = hex(0x4a5263);
pub(super) const BLANCO: ColorRgba = hex(0xffffff);
/// El relleno claro de los muebles.
pub(super) const CLARO: ColorRgba = hex(0xf2f4f7);
/// El gris de los muros cortados (poche) y del hormigon.
pub(super) const POCHE: ColorRgba = hex(0x9aa3b1);
pub(super) const GRIS: ColorRgba = hex(0xc9cfd8);

/// Una figura de una lista: su clave, su nombre y como se dibuja.
pub(super) type Dibujo = (&'static str, &'static str, fn(&mut Lapiz));

/// Dibuja las figuras de una lista y las mete en `v` con su categoria.
pub(super) fn poner(v: &mut Vec<FiguraGuardada>, categoria: &str, lista: &[Dibujo]) {
    for (clave, nombre, dibujar) in lista {
        let mut l = Lapiz::default();
        dibujar(&mut l);
        v.push(l.figura(clave, categoria, nombre));
    }
}

/// Una forma cerrada con relleno.
pub(super) fn forma(
    l: &mut Lapiz,
    pts: &[(f32, f32)],
    trazo: ColorRgba,
    relleno: Option<ColorRgba>,
    g: f32,
) {
    let mut v = pts.to_vec();
    if v.first() != v.last() {
        v.push(v[0]);
    }
    l.poli(&v, trazo, g, relleno);
}

/// Gira unos puntos alrededor de `c` (grados, en el sentido de la pantalla).
pub(super) fn girar(pts: &[(f32, f32)], c: (f32, f32), grados: f32) -> Vec<(f32, f32)> {
    let (s, co) = grados.to_radians().sin_cos();
    pts.iter()
        .map(|(x, y)| {
            let (dx, dy) = (x - c.0, y - c.1);
            (c.0 + dx * co - dy * s, c.1 + dx * s + dy * co)
        })
        .collect()
}

/// Los puntos de una circunferencia (o elipse) en `n` tramos.
pub(super) fn circulo_de_puntos(cx: f32, cy: f32, rx: f32, ry: f32, n: usize) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

/// **El rayado de seccion** a 45° dentro de un rectangulo, cada `paso`.
#[allow(clippy::too_many_arguments)]
pub(super) fn rayado(
    l: &mut Lapiz,
    x0: f32,
    y0: f32,
    w: f32,
    h: f32,
    paso: f32,
    c: ColorRgba,
    g: f32,
) {
    let mut t = paso;
    while t < w + h {
        // La recta (x - x0) + (h - (y - y0)) = t, de abajo a la izquierda
        // hacia arriba a la derecha.
        let xa = (t - h).max(0.0);
        let xb = t.min(w);
        if xb - xa > 0.5 {
            l.linea(x0 + xa, y0 + h - (t - xa), x0 + xb, y0 + h - (t - xb), c, g);
        }
        t += paso;
    }
}

/// **Una copa en feston** (la de los arboles en planta y en alzado): una
/// circunferencia con `lobulos` bultos, como se dibuja a mano.
#[allow(clippy::too_many_arguments)]
pub(super) fn copa(
    l: &mut Lapiz,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    lobulos: usize,
    trazo: ColorRgba,
    relleno: Option<ColorRgba>,
    g: f32,
) {
    let n = lobulos * 10;
    let pts: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            let k = 0.86 + 0.14 * (lobulos as f32 * a / 2.0).sin().abs();
            (cx + rx * k * a.cos(), cy + ry * k * a.sin())
        })
        .collect();
    forma(l, &pts, trazo, relleno, g);
}

/// Un punto del espacio en isometrica, con su origen en pantalla: `x` va
/// hacia abajo a la derecha, `y` hacia abajo a la izquierda y `z` arriba.
pub(super) fn iso(o: (f32, f32), x: f32, y: f32, z: f32) -> (f32, f32) {
    let c = 30f32.to_radians().cos();
    (o.0 + (x - y) * c, o.1 + (x + y) * 0.5 - z)
}

/// Los colores de una caja en isometrica: arriba, cara izquierda y cara
/// derecha (la luz viene de arriba a la izquierda).
#[derive(Clone, Copy)]
pub(super) struct Caras {
    pub arriba: ColorRgba,
    pub izq: ColorRgba,
    pub der: ColorRgba,
}

pub(super) const MADERA: Caras = Caras {
    arriba: hex(0xf1e3cf),
    izq: hex(0xdcc4a2),
    der: hex(0xc4a77f),
};
pub(super) const TELA: Caras = Caras {
    arriba: hex(0xe3eaf4),
    izq: hex(0xc4d1e3),
    der: hex(0xa7b8d0),
};
pub(super) const BLANCA: Caras = Caras {
    arriba: hex(0xffffff),
    izq: hex(0xe6e9ee),
    der: hex(0xcdd2da),
};
pub(super) const HORMIGON: Caras = Caras {
    arriba: hex(0xe4e6ea),
    izq: hex(0xc7cbd2),
    der: hex(0xaab0ba),
};

/// **Una caja en isometrica** desde la esquina `(x, y, z)` con medidas
/// `(dx, dy, dz)`: las tres caras que se ven. Varias cajas se dibujan de
/// atras hacia delante.
#[allow(clippy::too_many_arguments)]
pub(super) fn caja_iso(
    l: &mut Lapiz,
    o: (f32, f32),
    x: f32,
    y: f32,
    z: f32,
    dx: f32,
    dy: f32,
    dz: f32,
    caras: Caras,
) {
    let p = |a: f32, b: f32, c: f32| iso(o, a, b, c);
    let (x1, y1, z1) = (x + dx, y + dy, z + dz);
    forma(
        l,
        &[p(x, y1, z), p(x1, y1, z), p(x1, y1, z1), p(x, y1, z1)],
        TINTA,
        Some(caras.izq),
        1.25,
    );
    forma(
        l,
        &[p(x1, y, z), p(x1, y1, z), p(x1, y1, z1), p(x1, y, z1)],
        TINTA,
        Some(caras.der),
        1.25,
    );
    forma(
        l,
        &[p(x, y, z1), p(x1, y, z1), p(x1, y1, z1), p(x, y1, z1)],
        TINTA,
        Some(caras.arriba),
        1.25,
    );
}

/// **Una caja con texto centrado** (los esquemas de derecho y civil).
#[allow(clippy::too_many_arguments)]
pub(super) fn caja_con_texto(
    l: &mut Lapiz,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    s: &str,
    tam: f32,
    trazo: ColorRgba,
    relleno: ColorRgba,
    tinta: ColorRgba,
) {
    l.rect(x, y, w, h, Some(trazo), Some(relleno), 1.5, true);
    let lineas: Vec<&str> = s.lines().collect();
    let alto = tam * 1.25 * lineas.len() as f32;
    for (i, t) in lineas.iter().enumerate() {
        l.texto(
            x + w / 2.0,
            y + (h - alto) / 2.0 + tam * 1.25 * i as f32,
            t,
            tam,
            WORK_SANS,
            tinta,
            i == 0,
            Al::Centro,
        );
    }
}

/// Una punta de flecha rellena en `(x, y)` apuntando en `grados`.
pub(super) fn punta(l: &mut Lapiz, x: f32, y: f32, grados: f32, largo: f32, c: ColorRgba) {
    let pts = girar(
        &[
            (x, y),
            (x - largo, y - largo * 0.42),
            (x - largo, y + largo * 0.42),
        ],
        (x, y),
        grados,
    );
    forma(l, &pts, c, Some(c), 1.0);
}

#[cfg(test)]
mod pruebas {
    use super::super::{
        CATEGORIAS, arquitectura, catalogo, categoria_de, civil, derecho, mobiliario, vegetacion,
    };
    use crate::biblioteca;
    use std::collections::HashSet;

    /// Las categorias de estudios y el principio de sus ids.
    const DE_ESTUDIOS: [(&str, &str); 6] = [
        (arquitectura::CATEGORIA, "serie-arq-"),
        (arquitectura::ESCALA, "serie-arq-"),
        (vegetacion::CATEGORIA, "serie-arq-"),
        (mobiliario::CATEGORIA, "serie-arq-"),
        (civil::CATEGORIA, "serie-civil-"),
        (derecho::CATEGORIA, "serie-der-"),
    ];
    const PREFIJOS: [&str; 3] = ["serie-arq-", "serie-civil-", "serie-der-"];

    #[test]
    fn cada_categoria_de_estudios_esta_en_el_menu_y_tiene_figuras() {
        let c = catalogo();
        let mut total = 0;
        for (cat, prefijo) in DE_ESTUDIOS {
            assert!(CATEGORIAS.contains(&cat), "{cat} no esta en CATEGORIAS");
            let suyas: Vec<_> = c
                .iter()
                .filter(|f| categoria_de(&f.nombre).is_some_and(|(k, _)| k == cat))
                .collect();
            assert!(suyas.len() >= 6, "{cat}: {}", suyas.len());
            for f in &suyas {
                assert!(f.id.starts_with(prefijo), "{} {}", f.nombre, f.id);
            }
            total += suyas.len();
        }
        let con_prefijo = c
            .iter()
            .filter(|f| PREFIJOS.iter().any(|p| f.id.starts_with(p)))
            .count();
        assert_eq!(
            total, con_prefijo,
            "toda figura de estudios va en una de sus categorias"
        );
        assert!(total >= 80, "{total}");
    }

    #[test]
    fn los_ids_de_estudios_son_unicos_y_ninguna_figura_esta_vacia() {
        let mut ids = HashSet::new();
        for f in catalogo()
            .into_iter()
            .filter(|f| PREFIJOS.iter().any(|p| f.id.starts_with(p)))
        {
            assert!(ids.insert(f.id.clone()), "id repetido: {}", f.id);
            assert!(!f.elementos.is_empty(), "{}", f.nombre);
            assert!(
                f.elementos.len() < 2_000,
                "{}: {} piezas",
                f.nombre,
                f.elementos.len()
            );
            let (x0, y0, x1, y1) = biblioteca::caja_de(&f.elementos).unwrap();
            let (w, h) = (x1 - x0, y1 - y0);
            assert!(w.is_finite() && h.is_finite(), "{}", f.nombre);
            // Ni un punto ni una sabana: de un simbolo chico a una hoja.
            assert!(
                (24.0..=1400.0).contains(&w) && (24.0..=1400.0).contains(&h),
                "{}: {w} x {h}",
                f.nombre
            );
        }
    }

    #[test]
    fn el_rayado_no_se_sale_del_rectangulo() {
        let mut l = super::Lapiz::default();
        super::rayado(&mut l, 10.0, 20.0, 100.0, 40.0, 7.0, super::TINTA, 1.0);
        assert!(!l.v.is_empty());
        for e in &l.v {
            assert!(
                e.x >= 9.99 && e.y >= 19.99 && e.ancho <= 100.01 && e.alto <= 40.01,
                "{} {} {} {}",
                e.x,
                e.y,
                e.ancho,
                e.alto
            );
        }
    }
}
