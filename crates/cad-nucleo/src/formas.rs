//! **De una entidad del plano a sus formas simples** ([`Forma`]): rayas,
//! arcos y circulos en 2D. Con ellas se engancha, se elige y se recorta.

use opencadcodec::{EntityType, Vector3};

use crate::geo::{self, Caja, Forma, P};

/// Hasta donde se desmonta una entidad compuesta (un bloque dentro de otro…).
const PROFUNDIDAD: u32 = 3;

pub fn p2(v: &Vector3) -> P {
    [v.x, v.y]
}

pub fn v3(p: P) -> Vector3 {
    Vector3::new(p[0], p[1], 0.0)
}

/// Si la entidad mira hacia abajo (normal con z negativa): su x va al reves.
fn al_reves(n: &Vector3) -> bool {
    n.z < 0.0
}

/// Un arco en el sistema propio (OCS) de una normal +-Z, pasado al dibujo.
fn arco_ocs(c: &Vector3, r: f64, a0: f64, a1: f64, n: &Vector3) -> Forma {
    let mut barrido = geo::normalizar(a1 - a0);
    if barrido < geo::EPS {
        barrido = std::f64::consts::TAU;
    }
    if al_reves(n) {
        // Espejo en x: el angulo a pasa a PI - a y el sentido se invierte.
        let inicio = geo::normalizar(std::f64::consts::PI - a1);
        Forma::Arco {
            c: [-c.x, c.y],
            r,
            inicio,
            barrido,
        }
    } else {
        Forma::Arco {
            c: [c.x, c.y],
            r,
            inicio: geo::normalizar(a0),
            barrido,
        }
    }
}

/// Los tramos de una polilinea ligera (con sus abombados).
pub fn tramos_lw(p: &opencadcodec::LwPolyline) -> Vec<Forma> {
    let n = p.vertices.len();
    let espejo = al_reves(&p.normal);
    let pt = |i: usize| {
        let l = &p.vertices[i].location;
        if espejo { [-l.x, l.y] } else { [l.x, l.y] }
    };
    let tramos = if p.is_closed { n } else { n.saturating_sub(1) };
    (0..tramos)
        .map(|i| {
            let j = (i + 1) % n;
            let b = p.vertices[i].bulge * if espejo { -1.0 } else { 1.0 };
            Forma::de_abombado(pt(i), pt(j), b)
        })
        .filter(|f| f.longitud() > geo::EPS)
        .collect()
}

/// **Las formas de una entidad.** Lo que no tiene rayas (un texto, un
/// punto) no da ninguna: se elige por su caja ([`caja`]).
pub fn formas(e: &EntityType) -> Vec<Forma> {
    let mut v = Vec::new();
    formas_en(e, 0, &mut v);
    v
}

fn formas_en(e: &EntityType, profundidad: u32, v: &mut Vec<Forma>) {
    match e {
        EntityType::Line(l) => {
            if geo::dist(p2(&l.start), p2(&l.end)) > geo::EPS {
                v.push(Forma::Segmento {
                    a: p2(&l.start),
                    b: p2(&l.end),
                });
            }
        }
        EntityType::Circle(c) => {
            let x = if al_reves(&c.normal) {
                -c.center.x
            } else {
                c.center.x
            };
            v.push(Forma::Circulo {
                c: [x, c.center.y],
                r: c.radius,
            });
        }
        EntityType::Arc(a) => v.push(arco_ocs(
            &a.center,
            a.radius,
            a.start_angle,
            a.end_angle,
            &a.normal,
        )),
        EntityType::LwPolyline(p) => v.extend(tramos_lw(p)),
        EntityType::Text(_) | EntityType::MText(_) | EntityType::Point(_) => {}
        // Lo demas (polilineas viejas, splines, elipses, cotas, solidos…),
        // desmontado en piezas simples por opencadcodec.
        otro if profundidad < PROFUNDIDAD => {
            for parte in otro.explode() {
                formas_en(&parte, profundidad + 1, v);
            }
        }
        _ => {}
    }
}

/// Los puntos sueltos donde se engancha una entidad sin rayas: el punto de
/// insercion de un texto, de un bloque o un punto.
pub fn puntos_de_insercion(e: &EntityType) -> Vec<P> {
    match e {
        EntityType::Text(t) => vec![p2(&t.insertion_point)],
        EntityType::MText(t) => vec![p2(&t.insertion_point)],
        EntityType::Point(p) => vec![p2(&p.location)],
        EntityType::Insert(i) => vec![p2(&i.insert_point)],
        _ => Vec::new(),
    }
}

/// La caja aproximada de un texto de una linea.
fn caja_texto(ins: P, alto: f64, rot: f64, letras: usize, ancho: f64) -> Caja {
    let largo = alto * 0.8 * letras.max(1) as f64 * ancho.max(0.1);
    let esquinas = [[0.0, 0.0], [largo, 0.0], [largo, alto], [0.0, alto]];
    let pts: Vec<P> = esquinas
        .iter()
        .map(|q| geo::sumar(ins, geo::rotar(*q, [0.0, 0.0], rot)))
        .collect();
    geo::caja_de(&pts)
}

/// **La caja de una entidad** (o `None` si no hay nada que la ocupe).
pub fn caja(e: &EntityType, formas: &[Forma]) -> Option<Caja> {
    let mut c = geo::CAJA_VACIA;
    for f in formas {
        c = geo::unir_cajas(c, f.caja());
    }
    match e {
        EntityType::Text(t) => {
            let n = t.value.chars().count();
            c = geo::unir_cajas(
                c,
                caja_texto(
                    p2(&t.insertion_point),
                    t.height,
                    t.rotation,
                    n,
                    t.width_factor,
                ),
            );
        }
        EntityType::MText(t) => {
            let n = t
                .value
                .lines()
                .map(|l| l.chars().count())
                .max()
                .unwrap_or(1);
            let lineas = t.value.lines().count().max(1) as f64;
            let ins = p2(&t.insertion_point);
            let mut k = caja_texto(ins, t.height, t.rotation, n, 1.0);
            k[1] -= t.height * 1.6 * (lineas - 1.0);
            c = geo::unir_cajas(c, k);
        }
        _ => {
            for p in puntos_de_insercion(e) {
                c = geo::caja_con(c, p);
            }
        }
    }
    geo::caja_valida(&c).then_some(c)
}

/// Lo que mide una entidad: su largo y, si es cerrada, su area.
pub fn largo_y_area(e: &EntityType) -> (f64, Option<f64>) {
    match e {
        EntityType::Circle(c) => (
            std::f64::consts::TAU * c.radius,
            Some(std::f64::consts::PI * c.radius * c.radius),
        ),
        EntityType::LwPolyline(p) => {
            let t = tramos_lw(p);
            let largo = t.iter().map(Forma::longitud).sum();
            (largo, p.is_closed.then(|| area_lw(p).abs()))
        }
        otro => (formas(otro).iter().map(Forma::longitud).sum(), None),
    }
}

/// El area con signo de una polilinea cerrada, con sus tramos curvos.
pub fn area_lw(p: &opencadcodec::LwPolyline) -> f64 {
    let n = p.vertices.len();
    if n < 2 {
        return 0.0;
    }
    let pts: Vec<P> = p
        .vertices
        .iter()
        .map(|v| [v.location.x, v.location.y])
        .collect();
    let mut a = geo::area_poligono(&pts);
    for i in 0..n {
        let b = p.vertices[i].bulge;
        if b.abs() < 1e-12 {
            continue;
        }
        // El casquete que anade (o quita) cada tramo curvo.
        let j = (i + 1) % n;
        let cuerda = geo::dist(pts[i], pts[j]);
        let theta = 4.0 * b.atan();
        let r = cuerda / (2.0 * (theta / 2.0).sin()).abs();
        let casquete = r * r / 2.0 * (theta.abs() - theta.abs().sin());
        a += casquete * theta.signum();
    }
    if al_reves(&p.normal) { -a } else { a }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use opencadcodec::{LwPolyline, Vector2};

    #[test]
    fn area_de_un_rectangulo_con_un_lado_curvo() {
        let mut p = LwPolyline::from_points(vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(2.0, 0.0),
            Vector2::new(2.0, 2.0),
            Vector2::new(0.0, 2.0),
        ]);
        p.is_closed = true;
        assert!((area_lw(&p) - 4.0).abs() < 1e-9);
        // Un semicirculo de radio 1 hacia afuera en el lado de abajo: 4 + PI/2.
        p.vertices[0].bulge = 1.0;
        assert!((area_lw(&p) - (4.0 + std::f64::consts::FRAC_PI_2)).abs() < 1e-9);
        // Hacia adentro, lo quita.
        p.vertices[0].bulge = -1.0;
        assert!((area_lw(&p) - (4.0 - std::f64::consts::FRAC_PI_2)).abs() < 1e-9);
    }

    #[test]
    fn un_rectangulo_da_cuatro_tramos() {
        let mut p = LwPolyline::from_points(vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(2.0, 0.0),
            Vector2::new(2.0, 1.0),
            Vector2::new(0.0, 1.0),
        ]);
        p.is_closed = true;
        let f = formas(&EntityType::LwPolyline(p.clone()));
        assert_eq!(f.len(), 4);
        p.is_closed = false;
        assert_eq!(formas(&EntityType::LwPolyline(p)).len(), 3);
    }
}
