//! **Las figuras para estudiantes** (24-oct-2026): fisica, quimica,
//! matematicas y biologia de secundaria, medicina y mecanica. Graficas,
//! montajes de laboratorio, simbolos, esquemas rotulados y alguna hoja
//! (historia clinica, nota SOAP), para dibujar apuntes y trabajos rapido.
//!
//! Cada materia va en su archivo (`ciencias/<materia>.rs`) con su color, y
//! todas usan los mismos trazos de aqui: formas cerradas, curvas, ejes,
//! cotas, angulos y rotulos con su raya. Las claves llevan `ciencia-` (o
//! `salud-` las de medicina) para no chocar con las demas de serie.

use super::{Al, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, Figura};

mod biologia;
mod fisica;
mod matematicas;
mod mecanica;
mod medicina;
mod quimica;

/// Las categorias de aqui, en el orden en que salen en el menu (las mismas
/// que van al final de [`super::CATEGORIAS`]).
#[cfg(test)]
const MIS_CATEGORIAS: [&str; 6] = [
    "Física",
    "Química",
    "Matemáticas",
    "Biología",
    "Medicina",
    "Mecánica",
];

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    fisica::todas(v);
    quimica::todas(v);
    matematicas::todas(v);
    biologia::todas(v);
    medicina::todas(v);
    mecanica::todas(v);
}

/// Una figura de la lista: su clave (sin prefijo), su nombre y como se
/// dibuja.
type Dibujo = (&'static str, &'static str, fn(&mut Lapiz));

/// Dibuja cada una de la lista y la mete con `prefijo` en la clave.
fn meter(v: &mut Vec<FiguraGuardada>, prefijo: &str, categoria: &str, lista: &[Dibujo]) {
    for (clave, nombre, dibujar) in lista {
        let mut l = Lapiz::default();
        dibujar(&mut l);
        v.push(l.figura(&format!("{prefijo}{clave}"), categoria, nombre));
    }
}

// ---------------------------------------------------------------- colores

const TINTA: ColorRgba = hex(0x2b2f3a);
const GRIS: ColorRgba = hex(0x7d8798);
const GRIS_CLARO: ColorRgba = hex(0xdfe4eb);
const CLARO: ColorRgba = hex(0xeef1f6);
const BLANCO: ColorRgba = hex(0xffffff);
const ROJO: ColorRgba = hex(0xd1453b);
const ROSA: ColorRgba = hex(0xfbe3e1);
const AZUL: ColorRgba = hex(0x2f6fb5);
const AZUL_SUAVE: ColorRgba = hex(0xe3eefa);
const VERDE: ColorRgba = hex(0x3f9a4a);
const NARANJA: ColorRgba = hex(0xe0812b);
const MORADO: ColorRgba = hex(0x7a4fb3);
const AMARILLO: ColorRgba = hex(0xf4c542);
/// El grosor normal.
const G: f32 = 2.0;

// ----------------------------------------------------------------- trazos

/// **Una forma cerrada**. Con relleno va como region (la mancha del bote
/// de relleno, con su borde): una linea que se cierra no se rellena en el
/// lienzo del PC.
fn forma(l: &mut Lapiz, pts: &[(f32, f32)], trazo: ColorRgba, relleno: Option<ColorRgba>, g: f32) {
    let mut v = pts.to_vec();
    if v.len() > 2 && v.first() != v.last() {
        v.push(v[0]);
    }
    l.poli(&v, trazo, g, relleno);
    if relleno.is_some()
        && v.len() > 3
        && let Some(e) = l.v.last_mut()
        && let Figura::Linea { puntos } = &mut e.figura
    {
        let mut contorno = std::mem::take(puntos);
        contorno.pop();
        e.figura = Figura::Region {
            contorno,
            huecos: Vec::new(),
        };
    }
}

/// Gira unos puntos alrededor de `c` (grados; positivo, como las agujas
/// del reloj en pantalla).
fn girar(pts: &[(f32, f32)], c: (f32, f32), grados: f32) -> Vec<(f32, f32)> {
    let (s, co) = grados.to_radians().sin_cos();
    pts.iter()
        .map(|(x, y)| {
            let (dx, dy) = (x - c.0, y - c.1);
            (c.0 + dx * co - dy * s, c.1 + dx * s + dy * co)
        })
        .collect()
}

/// Una funcion muestreada de `x0` a `x1` en `n` tramos.
fn curva(f: impl Fn(f32) -> f32, x0: f32, x1: f32, n: usize) -> Vec<(f32, f32)> {
    (0..=n)
        .map(|i| {
            let x = x0 + (x1 - x0) * i as f32 / n as f32;
            (x, f(x))
        })
        .collect()
}

/// Una curva de Bezier cuadratica de `a` a `b` tirando hacia `c`.
fn bezier(a: (f32, f32), c: (f32, f32), b: (f32, f32), n: usize) -> Vec<(f32, f32)> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            (
                u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
                u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
            )
        })
        .collect()
}

/// **Redondea una forma cerrada** (Chaikin): cada vuelta corta las
/// esquinas. Asi los organos y las celulas salen de pocos puntos.
fn suavizar(pts: &[(f32, f32)], vueltas: usize) -> Vec<(f32, f32)> {
    let mut v = pts.to_vec();
    for _ in 0..vueltas {
        let n = v.len();
        let mut s = Vec::with_capacity(n * 2);
        for i in 0..n {
            let (a, b) = (v[i], v[(i + 1) % n]);
            s.push((0.75 * a.0 + 0.25 * b.0, 0.75 * a.1 + 0.25 * b.1));
            s.push((0.25 * a.0 + 0.75 * b.0, 0.25 * a.1 + 0.75 * b.1));
        }
        v = s;
    }
    v
}

/// Una forma redondeada cerrada.
fn blanda(l: &mut Lapiz, pts: &[(f32, f32)], trazo: ColorRgba, relleno: Option<ColorRgba>, g: f32) {
    forma(l, &suavizar(pts, 3), trazo, relleno, g);
}

/// Los mismos puntos vistos en un espejo vertical en `x = eje`, en orden
/// contrario (para cerrar una silueta simetrica).
fn espejo(pts: &[(f32, f32)], eje: f32) -> Vec<(f32, f32)> {
    pts.iter().rev().map(|(x, y)| (2.0 * eje - x, *y)).collect()
}

/// Un zigzag de `a` a `b` (resortes y ondas), con un tramo recto en cada
/// punta.
fn zigzag(
    a: (f32, f32),
    b: (f32, f32),
    vueltas: usize,
    amplitud: f32,
    punta: f32,
) -> Vec<(f32, f32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let largo = (dx * dx + dy * dy).sqrt().max(1.0);
    let (ux, uy) = (dx / largo, dy / largo);
    let (nx, ny) = (-uy, ux);
    let util = largo - 2.0 * punta;
    let mut v = vec![a, (a.0 + ux * punta, a.1 + uy * punta)];
    let pasos = vueltas * 2;
    for i in 0..pasos {
        let t = punta + util * (i as f32 + 0.5) / pasos as f32;
        let s = if i % 2 == 0 { amplitud } else { -amplitud };
        v.push((a.0 + ux * t + nx * s, a.1 + uy * t + ny * s));
    }
    v.push((b.0 - ux * punta, b.1 - uy * punta));
    v.push(b);
    v
}

// ---------------------------------------------------------------- textos

/// Un texto centrado en `(x, y)`.
fn txt(l: &mut Lapiz, x: f32, y: f32, s: &str, tam: f32, c: ColorRgba) {
    l.texto(x, y - tam * 0.62, s, tam, WORK_SANS, c, false, Al::Centro);
}

/// Un texto en negrita centrado en `(x, y)`.
fn txt_n(l: &mut Lapiz, x: f32, y: f32, s: &str, tam: f32, c: ColorRgba) {
    l.texto(x, y - tam * 0.62, s, tam, WORK_SANS, c, true, Al::Centro);
}

/// Un texto con su lado `al` en `x` y centrado en `y`.
fn txt_a(l: &mut Lapiz, x: f32, y: f32, s: &str, tam: f32, al: Al) {
    l.texto(x, y - tam * 0.62, s, tam, WORK_SANS, TINTA, false, al);
}

/// **Un rotulo con su raya**: de `p` (lo que se nombra, con un puntito) a
/// `t`, y el texto pegado a `t` del lado `al`.
fn rotulo(l: &mut Lapiz, p: (f32, f32), t: (f32, f32), s: &str, al: Al) {
    l.linea(p.0, p.1, t.0, t.1, GRIS, 1.0);
    l.circulo(p.0, p.1, 2.2, None, Some(TINTA), 1.0);
    let x = match al {
        Al::Izq => t.0 + 5.0,
        Al::Der => t.0 - 5.0,
        Al::Centro => t.0,
    };
    let y = match al {
        Al::Centro if t.1 < p.1 => t.1 - 8.0,
        Al::Centro => t.1 + 8.0,
        _ => t.1,
    };
    txt_a(l, x, y, s, 11.0, al);
}

/// Rotulos en columna a la derecha (`x`), de arriba abajo desde `y0` cada
/// `paso`: cada uno con su raya al punto que nombra.
fn rotulos_derecha(l: &mut Lapiz, x: f32, y0: f32, paso: f32, lista: &[((f32, f32), &str)]) {
    for (i, (p, s)) in lista.iter().enumerate() {
        rotulo(l, *p, (x, y0 + paso * i as f32), s, Al::Izq);
    }
}

/// Lo mismo en columna a la izquierda (`x` es donde acaba el texto).
fn rotulos_izquierda(l: &mut Lapiz, x: f32, y0: f32, paso: f32, lista: &[((f32, f32), &str)]) {
    for (i, (p, s)) in lista.iter().enumerate() {
        rotulo(l, *p, (x, y0 + paso * i as f32), s, Al::Der);
    }
}

// ------------------------------------------------------- ejes y medidas

/// **Unos ejes**: el origen `o`, hasta donde llega cada eje (`xs`: de
/// izquierda a derecha; `ys`: de arriba abajo) y sus nombres.
fn ejes(l: &mut Lapiz, o: (f32, f32), xs: (f32, f32), ys: (f32, f32), nombres: (&str, &str)) {
    l.flecha(&[(xs.0, o.1), (xs.1, o.1)], TINTA, 1.6, false);
    l.flecha(&[(o.0, ys.1), (o.0, ys.0)], TINTA, 1.6, false);
    if !nombres.0.is_empty() {
        txt_a(l, xs.1, o.1 + 16.0, nombres.0, 12.0, Al::Der);
    }
    if !nombres.1.is_empty() {
        txt_a(l, o.0 + 10.0, ys.0 + 4.0, nombres.1, 12.0, Al::Izq);
    }
}

/// Una cuadricula suave de `x0,y0` a `x1,y1` cada `paso`.
fn cuadricula(l: &mut Lapiz, x0: f32, y0: f32, x1: f32, y1: f32, paso: f32) {
    let mut x = x0;
    while x <= x1 + 0.1 {
        l.linea(x, y0, x, y1, GRIS_CLARO, 1.0);
        x += paso;
    }
    let mut y = y0;
    while y <= y1 + 0.1 {
        l.linea(x0, y, x1, y, GRIS_CLARO, 1.0);
        y += paso;
    }
}

/// **Un angulo** en el vertice `v` entre los lados que van a `a` y a `b`,
/// con su arco de radio `r` y su nombre.
fn angulo(
    l: &mut Lapiz,
    v: (f32, f32),
    a: (f32, f32),
    b: (f32, f32),
    r: f32,
    s: &str,
    c: ColorRgba,
) {
    let a1 = (a.1 - v.1).atan2(a.0 - v.0).to_degrees();
    let mut a2 = (b.1 - v.1).atan2(b.0 - v.0).to_degrees();
    if a2 - a1 > 180.0 {
        a2 -= 360.0;
    } else if a2 - a1 < -180.0 {
        a2 += 360.0;
    }
    l.arco(v.0, v.1, r, a1, a2, c, 1.5);
    if !s.is_empty() {
        let m = ((a1 + a2) / 2.0).to_radians();
        txt(
            l,
            v.0 + (r + 11.0) * m.cos(),
            v.1 + (r + 11.0) * m.sin(),
            s,
            12.0,
            c,
        );
    }
}

/// La marca de angulo recto en `v` con los lados hacia `a` y `b`.
fn recto(l: &mut Lapiz, v: (f32, f32), a: (f32, f32), b: (f32, f32), lado: f32) {
    let u = |p: (f32, f32)| {
        let (dx, dy) = (p.0 - v.0, p.1 - v.1);
        let n = (dx * dx + dy * dy).sqrt().max(1.0);
        (dx / n * lado, dy / n * lado)
    };
    let (ua, ub) = (u(a), u(b));
    l.poli(
        &[
            (v.0 + ua.0, v.1 + ua.1),
            (v.0 + ua.0 + ub.0, v.1 + ua.1 + ub.1),
            (v.0 + ub.0, v.1 + ub.1),
        ],
        TINTA,
        1.2,
        None,
    );
}

/// **Una cota horizontal** de `x0` a `x1` a la altura `y`, con sus rayas
/// de referencia desde `desde` y la medida encima.
fn cota_h(l: &mut Lapiz, x0: f32, x1: f32, y: f32, desde: f32, s: &str) {
    let signo = if desde > y { -1.0 } else { 1.0 };
    l.linea(x0, desde, x0, y + signo * 5.0, GRIS, 1.0);
    l.linea(x1, desde, x1, y + signo * 5.0, GRIS, 1.0);
    l.flecha(&[(x0, y), (x1, y)], TINTA, 1.0, true);
    txt(l, (x0 + x1) / 2.0, y - 9.0, s, 11.0, TINTA);
}

/// Una cota vertical de `y0` a `y1` en `x`, con las rayas desde `desde` y
/// la medida a un lado.
fn cota_v(l: &mut Lapiz, y0: f32, y1: f32, x: f32, desde: f32, s: &str) {
    let signo = if desde > x { -1.0 } else { 1.0 };
    l.linea(desde, y0, x + signo * 5.0, y0, GRIS, 1.0);
    l.linea(desde, y1, x + signo * 5.0, y1, GRIS, 1.0);
    l.flecha(&[(x, y0), (x, y1)], TINTA, 1.0, true);
    let al = if signo < 0.0 { Al::Der } else { Al::Izq };
    let dx = if signo < 0.0 { -6.0 } else { 6.0 };
    txt_a(l, x + dx, (y0 + y1) / 2.0, s, 11.0, al);
}

/// Un vector con su nombre junto a la punta.
fn vector(l: &mut Lapiz, a: (f32, f32), b: (f32, f32), c: ColorRgba, s: &str) {
    l.flecha(&[a, b], c, 2.5, false);
    if !s.is_empty() {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let n = (dx * dx + dy * dy).sqrt().max(1.0);
        txt_n(l, b.0 + dx / n * 14.0, b.1 + dy / n * 14.0, s, 13.0, c);
    }
}

/// El techo o el suelo rayado de los montajes, de `x0` a `x1` en `y`
/// (las rayitas hacia arriba si `arriba`).
fn rayado(l: &mut Lapiz, x0: f32, x1: f32, y: f32, arriba: bool) {
    l.linea(x0, y, x1, y, TINTA, 2.5);
    let d = if arriba { -10.0 } else { 10.0 };
    let mut x = x0 + 4.0;
    while x < x1 {
        l.linea(x, y, x + 8.0, y + d, GRIS, 1.0);
        x += 12.0;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::biblioteca;
    use std::collections::HashSet;

    fn mias() -> Vec<FiguraGuardada> {
        let mut v = Vec::new();
        todas(&mut v);
        v
    }

    #[test]
    fn cada_materia_tiene_sus_figuras_y_va_al_final_del_menu() {
        let v = mias();
        assert!(v.len() >= 60, "{}", v.len());
        let n = super::super::CATEGORIAS.len();
        assert_eq!(
            &super::super::CATEGORIAS[n - MIS_CATEGORIAS.len()..],
            MIS_CATEGORIAS.as_slice()
        );
        for cat in MIS_CATEGORIAS {
            let cuantas = v
                .iter()
                .filter(|f| super::super::categoria_de(&f.nombre).is_some_and(|(c, _)| c == cat))
                .count();
            assert!(cuantas >= 4, "{cat}: {cuantas}");
        }
        for f in &v {
            let (cat, _) = super::super::categoria_de(&f.nombre).unwrap();
            assert!(MIS_CATEGORIAS.contains(&cat), "{}", f.nombre);
        }
    }

    #[test]
    fn los_ids_llevan_su_prefijo_y_no_se_repiten() {
        let v = mias();
        let mut ids = HashSet::new();
        for f in &v {
            let clave = f.id.strip_prefix(super::super::PREFIJO_ID).unwrap();
            let medicina = f.nombre.starts_with("Medicina");
            assert!(
                clave.starts_with(if medicina { "salud-" } else { "ciencia-" }),
                "{}",
                f.id
            );
            assert!(ids.insert(f.id.clone()), "repetido: {}", f.id);
        }
        // Y no pisan a ninguna otra de serie.
        let todas = super::super::catalogo();
        assert_eq!(
            todas.iter().filter(|f| ids.contains(&f.id)).count(),
            ids.len()
        );
    }

    #[test]
    fn ninguna_esta_vacia_y_todas_caben_en_un_tamano_razonable() {
        for f in mias() {
            assert!(f.elementos.len() >= 3, "{} {}", f.nombre, f.elementos.len());
            let (x0, y0, x1, y1) = biblioteca::caja_de(&f.elementos).unwrap();
            let (w, h) = (x1 - x0, y1 - y0);
            assert!(w > 30.0 && h > 30.0, "{} {w}×{h}", f.nombre);
            assert!(w <= 1200.0 && h <= 1200.0, "{} {w}×{h}", f.nombre);
            for e in &f.elementos {
                assert!(
                    e.x.is_finite() && e.y.is_finite() && e.ancho.is_finite() && e.alto.is_finite(),
                    "{}",
                    f.nombre
                );
            }
        }
    }

    #[test]
    fn los_rotulos_estan_en_espanol_y_no_parecen_titulos_de_hoja() {
        // Un texto de 30 o mas es el titulo de una plantilla: solo las dos
        // hojas de medicina lo llevan, y cada una el suyo.
        let mut titulos = HashSet::new();
        for f in mias() {
            for e in &f.elementos {
                if let Figura::Texto { texto, tam, .. } = &e.figura {
                    assert!(!texto.trim().is_empty(), "{}", f.nombre);
                    if *tam >= 30.0 {
                        assert!(f.id.contains("salud-"), "{}: {texto}", f.nombre);
                        assert!(titulos.insert(texto.clone()), "{texto}");
                    }
                }
            }
        }
        let v = mias();
        let corazon = v.iter().find(|f| f.id.ends_with("salud-corazon")).unwrap();
        let t: Vec<&str> = corazon
            .elementos
            .iter()
            .filter_map(|e| match &e.figura {
                Figura::Texto { texto, .. } => Some(texto.as_str()),
                _ => None,
            })
            .collect();
        assert!(t.contains(&"VI") && t.contains(&"AD"), "{t:?}");
    }

    #[test]
    fn la_tabla_periodica_tiene_los_118() {
        let v = mias();
        let tabla = v
            .iter()
            .find(|f| f.id.ends_with("ciencia-tabla-periodica"))
            .unwrap();
        let t: Vec<&str> = tabla
            .elementos
            .iter()
            .filter_map(|e| match &e.figura {
                Figura::Texto { texto, .. } => Some(texto.as_str()),
                _ => None,
            })
            .collect();
        for s in ["H", "He", "Fe", "Au", "Og", "Lu", "Lr"] {
            assert!(t.contains(&s), "falta {s}");
        }
        assert!(t.contains(&"118") && !t.contains(&"119"));
    }

    #[test]
    fn los_trazos_de_ayuda_hacen_lo_que_dicen() {
        let z = zigzag((0.0, 0.0), (100.0, 0.0), 3, 5.0, 10.0);
        assert_eq!(z.len(), 2 + 6 + 2);
        assert_eq!(z.first(), Some(&(0.0, 0.0)));
        assert_eq!(z.last(), Some(&(100.0, 0.0)));
        let s = suavizar(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], 2);
        assert_eq!(s.len(), 12);
        let g = girar(&[(10.0, 0.0)], (0.0, 0.0), 90.0);
        assert!(
            (g[0].0).abs() < 1e-4 && (g[0].1 - 10.0).abs() < 1e-4,
            "{g:?}"
        );
        let e = espejo(&[(1.0, 2.0), (3.0, 4.0)], 10.0);
        assert_eq!(e, vec![(17.0, 4.0), (19.0, 2.0)]);
    }
}
