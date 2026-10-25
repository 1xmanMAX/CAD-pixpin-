//! **Modificar lo dibujado**: mover, girar, escalar, simetria (para todas las
//! entidades, con las transformaciones de opencadcodec), y desfase,
//! recortar, alargar, empalme, chaflan, matrices y pinzamientos (para las
//! rayas, arcos, circulos y polilineas, que es lo que se edita a mano).

use std::f64::consts::TAU;

use opencadcodec::types::Transform;
use opencadcodec::{EntityType, Vector3};

use crate::dibujo::Indice;
use crate::especialidad::{arco, linea};
use crate::formas::{self, p2, v3};
use crate::geo::{self, Forma, P};

pub fn trasladar(e: &mut EntityType, d: P) {
    e.translate(Vector3::new(d[0], d[1], 0.0));
}

/// Gira `ang` radianes (antihorario) alrededor de `c`.
pub fn girar(e: &mut EntityType, c: P, ang: f64) {
    let t = Transform::from_translation(Vector3::new(-c[0], -c[1], 0.0))
        .then(&Transform::from_rotation(Vector3::UNIT_Z, ang))
        .then(&Transform::from_translation(v3(c)));
    e.apply_transform(&t);
}

/// Escala `k` veces desde `c`.
pub fn escalar(e: &mut EntityType, c: P, k: f64) {
    e.apply_scaling_with_origin(Vector3::new(k, k, 1.0), v3(c));
}

/// Simetria respecto a la recta `a`-`b`.
pub fn simetria(e: &mut EntityType, a: P, b: P) {
    let n = geo::perp(geo::unitario(geo::restar(b, a)));
    e.apply_mirror(&Transform::from_mirror_plane(
        v3(a),
        Vector3::new(n[0], n[1], 0.0),
    ));
}

/// El arco (de opencadcodec) de una forma arco.
fn arco_de(c: P, r: f64, inicio: f64, barrido: f64) -> EntityType {
    arco(c, r, inicio, inicio + barrido)
}

/// Copia `base` con la geometria de `nueva`, conservando capa, color, tipo
/// de linea… (lo comun).
fn con_lo_comun(base: &EntityType, mut nueva: EntityType) -> EntityType {
    let mut comun = base.common().clone();
    comun.handle = opencadcodec::Handle::NULL;
    comun.raw_record = None;
    *nueva.common_mut() = comun;
    nueva
}

/// La entidad de una forma simple, con lo comun de `base`.
fn de_forma(base: &EntityType, f: &Forma) -> EntityType {
    let e = match *f {
        Forma::Segmento { a, b } => linea(a, b),
        Forma::Arco {
            c,
            r,
            inicio,
            barrido,
        } => arco_de(c, r, inicio, barrido),
        Forma::Circulo { c, r } => crate::especialidad::circulo(c, r),
    };
    con_lo_comun(base, e)
}

// ------------------------------------------------------------------ desfase

/// **Desfase** (OFFSET): una copia paralela a `d` del lado de `lado`. Para
/// lineas, arcos, circulos y polilineas.
pub fn desfase(e: &EntityType, d: f64, lado: P) -> Option<EntityType> {
    if d <= 0.0 {
        return None;
    }
    match e {
        EntityType::Line(l) => {
            let (a, b) = (p2(&l.start), p2(&l.end));
            let n = geo::perp(geo::unitario(geo::restar(b, a)));
            let s = if geo::punto(geo::restar(lado, a), n) >= 0.0 {
                d
            } else {
                -d
            };
            let o = geo::por(n, s);
            let mut l2 = l.clone();
            l2.start = v3(geo::sumar(a, o));
            l2.end = v3(geo::sumar(b, o));
            Some(EntityType::Line(l2))
        }
        EntityType::Circle(c) => {
            let fuera = geo::dist(lado, p2(&c.center)) > c.radius;
            let r = if fuera { c.radius + d } else { c.radius - d };
            (r > geo::EPS).then(|| {
                let mut c2 = c.clone();
                c2.radius = r;
                EntityType::Circle(c2)
            })
        }
        EntityType::Arc(a) => {
            let fuera = geo::dist(lado, p2(&a.center)) > a.radius;
            let r = if fuera { a.radius + d } else { a.radius - d };
            (r > geo::EPS).then(|| {
                let mut a2 = a.clone();
                a2.radius = r;
                EntityType::Arc(a2)
            })
        }
        EntityType::LwPolyline(p) => desfase_lw(p, d, lado).map(EntityType::LwPolyline),
        _ => None,
    }
}

fn desfase_lw(p: &opencadcodec::LwPolyline, d: f64, lado: P) -> Option<opencadcodec::LwPolyline> {
    let pts: Vec<P> = p
        .vertices
        .iter()
        .map(|v| [v.location.x, v.location.y])
        .collect();
    if pts.len() < 2 {
        return None;
    }
    // El lado: el del tramo mas cercano a `lado`.
    let tramos = formas::tramos_lw(p);
    let (k, _) = tramos
        .iter()
        .enumerate()
        .map(|(i, f)| (i, f.distancia(lado)))
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    let s = match tramos[k] {
        Forma::Segmento { a, b } => {
            let n = geo::perp(geo::unitario(geo::restar(b, a)));
            if geo::punto(geo::restar(lado, a), n) >= 0.0 {
                1.0
            } else {
                -1.0
            }
        }
        Forma::Arco { c, r, .. } | Forma::Circulo { c, r } => {
            // Dentro del arco es la izquierda si el tramo va antihorario.
            let dentro = geo::dist(lado, c) < r;
            let antihorario = p.vertices[k].bulge > 0.0;
            if dentro == antihorario { 1.0 } else { -1.0 }
        }
    };
    let tiene_curvas = p.vertices.iter().any(|v| v.bulge.abs() > 1e-12);
    let nuevos = if tiene_curvas {
        // Con curvas: cada vertice se mueve por la bisectriz de sus tramos
        // (aproximado en las juntas, exacto en los tramos).
        desplazar_con_curvas(
            &pts,
            &p.vertices.iter().map(|v| v.bulge).collect::<Vec<_>>(),
            d * s,
            p.is_closed,
        )?
    } else {
        crate::especialidad::desplazar_polilinea(&pts, d * s, p.is_closed)?
    };
    let mut q = p.clone();
    for (v, n) in q.vertices.iter_mut().zip(nuevos) {
        v.location = opencadcodec::Vector2::new(n[0], n[1]);
    }
    Some(q)
}

fn desplazar_con_curvas(pts: &[P], bulges: &[f64], d: f64, cerrada: bool) -> Option<Vec<P>> {
    let n = pts.len();
    // La normal de salida y de llegada de cada tramo en sus extremos.
    let tramos = if cerrada { n } else { n - 1 };
    let mut normales_ini = vec![[0.0; 2]; n];
    let mut normales_fin = vec![[0.0; 2]; n];
    for i in 0..tramos {
        let j = (i + 1) % n;
        let f = Forma::de_abombado(pts[i], pts[j], bulges[i]);
        let (ni, nf) = match f {
            Forma::Segmento { a, b } => {
                let k = geo::perp(geo::unitario(geo::restar(b, a)));
                (k, k)
            }
            Forma::Arco { c, .. } => {
                // Izquierda del avance: hacia el centro si va antihorario.
                let s = if bulges[i] > 0.0 { 1.0 } else { -1.0 };
                (
                    geo::por(geo::unitario(geo::restar(c, pts[i])), s),
                    geo::por(geo::unitario(geo::restar(c, pts[j])), s),
                )
            }
            Forma::Circulo { .. } => ([0.0, 0.0], [0.0, 0.0]),
        };
        normales_ini[i] = ni;
        normales_fin[j] = nf;
    }
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b) = (normales_fin[i], normales_ini[i]);
        let llega = cerrada || i > 0;
        let sale = cerrada || i + 1 < n;
        let m = match (llega, sale) {
            (true, true) => {
                let s = geo::unitario(geo::sumar(a, b));
                let cos = geo::punto(s, b).max(0.2);
                geo::por(s, 1.0 / cos)
            }
            (true, false) => a,
            _ => b,
        };
        v.push(geo::sumar(pts[i], geo::por(m, d)));
    }
    Some(v)
}

// ------------------------------------------------------- recortar y alargar

/// Las formas de los bordes que cortan (todas las del indice menos la propia).
fn bordes<'a>(
    ind: &'a Indice,
    propia: opencadcodec::Handle,
    bordes: Option<&[opencadcodec::Handle]>,
) -> Vec<&'a Forma> {
    ind.entradas
        .iter()
        .filter(|e| e.handle != propia && bordes.is_none_or(|b| b.contains(&e.handle)))
        .flat_map(|e| e.formas.iter())
        .collect()
}

/// Donde cae `p` a lo largo de una forma: de 0 a 1 en un segmento, en
/// radianes desde el inicio en un arco o circulo.
fn parametro(f: &Forma, p: P) -> f64 {
    match *f {
        Forma::Segmento { a, b } => geo::parametro_en_segmento(a, b, p),
        Forma::Arco { c, inicio, .. } => geo::desde(inicio, geo::angulo(c, p)),
        Forma::Circulo { c, .. } => geo::normalizar(geo::angulo(c, p)),
    }
}

fn punto_en(f: &Forma, t: f64) -> P {
    match *f {
        Forma::Segmento { a, b } => geo::sumar(a, geo::por(geo::restar(b, a), t)),
        Forma::Arco { c, r, inicio, .. } => geo::polar(c, r, inicio + t),
        Forma::Circulo { c, r } => geo::polar(c, r, t),
    }
}

/// **Recortar** (TRIM): quita de la entidad el trozo entre los dos cortes
/// que rodean a `pick`. Da lo que queda (vacio: borrarla entera; `None`: no
/// hay cortes, no se toca). Para lineas, arcos y circulos.
pub fn recortar(
    ind: &Indice,
    h: opencadcodec::Handle,
    e: &EntityType,
    pick: P,
    limites: Option<&[opencadcodec::Handle]>,
) -> Option<Vec<EntityType>> {
    let f = match e {
        EntityType::Line(_) | EntityType::Arc(_) | EntityType::Circle(_) => {
            *formas::formas(e).first()?
        }
        _ => return None,
    };
    let largo = match f {
        Forma::Segmento { .. } => 1.0,
        Forma::Arco { barrido, .. } => barrido,
        Forma::Circulo { .. } => TAU,
    };
    let tol = 1e-9 * (1.0 + largo);
    let mut ts: Vec<f64> = bordes(ind, h, limites)
        .into_iter()
        .flat_map(|b| geo::cortes(&f, b))
        .map(|p| parametro(&f, p))
        .filter(|t| matches!(f, Forma::Circulo { .. }) || (*t > tol && *t < largo - tol))
        .collect();
    ts.sort_by(f64::total_cmp);
    ts.dedup_by(|a, b| (*a - *b).abs() < tol);
    if ts.is_empty() {
        return None;
    }
    let tp = parametro(&f, f.cercano(pick));
    match f {
        Forma::Circulo { c, r } => {
            if ts.len() < 2 {
                return None;
            }
            // El hueco entre los cortes que rodean al pick; queda el resto.
            let k = ts.iter().position(|t| *t > tp).unwrap_or(0);
            let fin_hueco = ts[k];
            let ini_hueco = ts[(k + ts.len() - 1) % ts.len()];
            let barrido = geo::desde(fin_hueco, ini_hueco);
            Some(vec![de_forma(
                e,
                &Forma::Arco {
                    c,
                    r,
                    inicio: fin_hueco,
                    barrido,
                },
            )])
        }
        _ => {
            let antes = ts
                .iter()
                .copied()
                .filter(|t| *t < tp)
                .fold(None, |_, t| Some(t));
            let despues = ts.iter().copied().find(|t| *t > tp);
            let mut v = Vec::new();
            if let Some(t0) = antes {
                v.push(trozo(e, &f, 0.0, t0));
            }
            if let Some(t1) = despues {
                v.push(trozo(e, &f, t1, largo));
            }
            Some(v)
        }
    }
}

/// El trozo de la forma `f` (linea o arco) entre los parametros `t0` y `t1`.
fn trozo(base: &EntityType, f: &Forma, t0: f64, t1: f64) -> EntityType {
    match *f {
        Forma::Arco { c, r, inicio, .. } => de_forma(
            base,
            &Forma::Arco {
                c,
                r,
                inicio: inicio + t0,
                barrido: t1 - t0,
            },
        ),
        _ => de_forma(
            base,
            &Forma::Segmento {
                a: punto_en(f, t0),
                b: punto_en(f, t1),
            },
        ),
    }
}

/// **Alargar** (EXTEND): lleva el extremo mas cercano a `pick` hasta el
/// primer borde que encuentre. Para lineas y arcos.
pub fn alargar(
    ind: &Indice,
    h: opencadcodec::Handle,
    e: &EntityType,
    pick: P,
    limites: Option<&[opencadcodec::Handle]>,
) -> Option<EntityType> {
    let f = match e {
        EntityType::Line(_) | EntityType::Arc(_) => *formas::formas(e).first()?,
        _ => return None,
    };
    let cortes: Vec<P> = bordes(ind, h, limites)
        .into_iter()
        .flat_map(|b| geo::cortes_alargando(&f, b))
        .collect();
    match f {
        Forma::Segmento { a, b } => {
            let por_el_final = geo::dist(pick, b) < geo::dist(pick, a);
            let tol = 1e-9;
            let t = cortes
                .iter()
                .map(|p| geo::parametro_en_segmento(a, b, *p))
                .filter(|t| {
                    if por_el_final {
                        *t > 1.0 + tol
                    } else {
                        *t < -tol
                    }
                })
                .min_by(|x, y| {
                    let (dx, dy) = if por_el_final { (*x, *y) } else { (-*x, -*y) };
                    dx.total_cmp(&dy)
                })?;
            let q = punto_en(&f, t);
            let seg = if por_el_final {
                Forma::Segmento { a, b: q }
            } else {
                Forma::Segmento { a: q, b }
            };
            Some(de_forma(e, &seg))
        }
        Forma::Arco {
            c,
            r,
            inicio,
            barrido,
        } => {
            let fin = inicio + barrido;
            let por_el_final =
                geo::dist(pick, geo::polar(c, r, fin)) < geo::dist(pick, geo::polar(c, r, inicio));
            let angs = cortes.iter().map(|p| geo::angulo(c, *p));
            let nuevo = if por_el_final {
                // Lo que hay que seguir girando desde el final.
                let extra = angs
                    .map(|a| geo::desde(fin, a))
                    .filter(|d| *d > 1e-9 && barrido + d < TAU)
                    .min_by(f64::total_cmp)?;
                Forma::Arco {
                    c,
                    r,
                    inicio,
                    barrido: barrido + extra,
                }
            } else {
                let extra = angs
                    .map(|a| geo::desde(a, inicio))
                    .filter(|d| *d > 1e-9 && barrido + d < TAU)
                    .min_by(f64::total_cmp)?;
                Forma::Arco {
                    c,
                    r,
                    inicio: inicio - extra,
                    barrido: barrido + extra,
                }
            };
            Some(de_forma(e, &nuevo))
        }
        Forma::Circulo { .. } => None,
    }
}

// --------------------------------------------------------- empalme y chaflan

/// Lo que da un empalme o un chaflan: las dos lineas con sus extremos nuevos
/// y lo que las une (el arco o la linea), si algo.
pub type Esquina = ((P, P), (P, P), Option<EntityType>);

/// La linea `a`-`b` llevada (recortada o alargada) a terminar en `q`,
/// conservando el extremo del lado de `pick`.
fn llevar_a(a: P, b: P, pick: P, q: P) -> (P, P) {
    // Se queda el extremo que esta del mismo lado que el pick respecto a q.
    let t_pick = geo::parametro_en_segmento(a, b, pick);
    let t_q = geo::parametro_en_segmento(a, b, q);
    if t_pick < t_q { (a, q) } else { (q, b) }
}

/// **Empalme** (FILLET) de dos lineas con radio `r` (0: solo unirlas en la
/// esquina). Da las dos lineas nuevas y el arco, si lo hay.
pub fn empalme(l1: (P, P), pick1: P, l2: (P, P), pick2: P, r: f64) -> Option<Esquina> {
    let x = geo::corte_rectas(l1.0, l1.1, l2.0, l2.1)?;
    if r <= geo::EPS {
        return Some((
            llevar_a(l1.0, l1.1, pick1, x),
            llevar_a(l2.0, l2.1, pick2, x),
            None,
        ));
    }
    // Las direcciones desde la esquina hacia lo que se queda.
    let lado = |a: P, b: P, pick: P| {
        let t = geo::parametro_en_segmento(a, b, pick) - geo::parametro_en_segmento(a, b, x);
        geo::unitario(geo::por(geo::restar(b, a), t.signum()))
    };
    let (u1, u2) = (lado(l1.0, l1.1, pick1), lado(l2.0, l2.1, pick2));
    let cos = geo::punto(u1, u2).clamp(-1.0, 1.0);
    let theta = cos.acos();
    if theta < 1e-6 || (std::f64::consts::PI - theta) < 1e-6 {
        return None;
    }
    // Distancia de la esquina a los puntos de tangencia.
    let t = r / (theta / 2.0).tan();
    let (p1, p2) = (
        geo::sumar(x, geo::por(u1, t)),
        geo::sumar(x, geo::por(u2, t)),
    );
    let bis = geo::unitario(geo::sumar(u1, u2));
    let c = geo::sumar(x, geo::por(bis, r / (theta / 2.0).sin()));
    let (a1, a2) = (geo::angulo(c, p1), geo::angulo(c, p2));
    // El arco corto entre las dos tangencias.
    let (inicio, fin) = if geo::desde(a1, a2) <= std::f64::consts::PI {
        (a1, a2)
    } else {
        (a2, a1)
    };
    Some((
        llevar_a(l1.0, l1.1, pick1, p1),
        llevar_a(l2.0, l2.1, pick2, p2),
        Some(arco(c, r, inicio, fin)),
    ))
}

/// **Chaflan** (CHAMFER) de dos lineas: corta `d1` de la primera y `d2` de
/// la segunda desde la esquina y las une con una linea.
pub fn chaflan(l1: (P, P), pick1: P, l2: (P, P), pick2: P, d1: f64, d2: f64) -> Option<Esquina> {
    let x = geo::corte_rectas(l1.0, l1.1, l2.0, l2.1)?;
    if d1 <= geo::EPS && d2 <= geo::EPS {
        return empalme(l1, pick1, l2, pick2, 0.0);
    }
    let lado = |a: P, b: P, pick: P| {
        let t = geo::parametro_en_segmento(a, b, pick) - geo::parametro_en_segmento(a, b, x);
        geo::unitario(geo::por(geo::restar(b, a), t.signum()))
    };
    let (p1, p2) = (
        geo::sumar(x, geo::por(lado(l1.0, l1.1, pick1), d1)),
        geo::sumar(x, geo::por(lado(l2.0, l2.1, pick2), d2)),
    );
    Some((
        llevar_a(l1.0, l1.1, pick1, p1),
        llevar_a(l2.0, l2.1, pick2, p2),
        Some(linea(p1, p2)),
    ))
}

/// Los extremos de una linea, si lo es.
pub fn extremos_de_linea(e: &EntityType) -> Option<(P, P)> {
    match e {
        EntityType::Line(l) => Some((p2(&l.start), p2(&l.end))),
        _ => None,
    }
}

/// La linea `e` con extremos nuevos.
pub fn con_extremos(e: &EntityType, a: P, b: P) -> EntityType {
    let mut e = e.clone();
    if let EntityType::Line(l) = &mut e {
        l.start = Vector3::new(a[0], a[1], l.start.z);
        l.end = Vector3::new(b[0], b[1], l.end.z);
    }
    e
}

// ------------------------------------------------------------------ matrices

/// **Matriz rectangular**: las copias (sin la original) en `filas` x
/// `columnas`, separadas `dx` y `dy`.
pub fn matriz_rectangular(
    e: &EntityType,
    filas: u32,
    columnas: u32,
    dx: f64,
    dy: f64,
) -> Vec<EntityType> {
    let mut v = Vec::new();
    for f in 0..filas.max(1) {
        for c in 0..columnas.max(1) {
            if f == 0 && c == 0 {
                continue;
            }
            let mut k = e.clone();
            trasladar(&mut k, [c as f64 * dx, f as f64 * dy]);
            v.push(k);
        }
    }
    v
}

/// **Matriz polar**: `n` elementos (con la original) repartidos en `total`
/// radianes alrededor de `centro`, girando cada copia.
pub fn matriz_polar(e: &EntityType, centro: P, n: u32, total: f64) -> Vec<EntityType> {
    if n < 2 {
        return Vec::new();
    }
    // Una vuelta entera no repite el primero.
    let paso = if (total.abs() - TAU).abs() < 1e-9 {
        total / n as f64
    } else {
        total / (n - 1) as f64
    };
    (1..n)
        .map(|i| {
            let mut k = e.clone();
            girar(&mut k, centro, paso * i as f64);
            k
        })
        .collect()
}

// ------------------------------------------------------------- pinzamientos

/// **Los pinzamientos** de una entidad (los cuadraditos azules al elegirla).
pub fn pinzamientos(e: &EntityType) -> Vec<P> {
    match e {
        EntityType::Line(l) => {
            let (a, b) = (p2(&l.start), p2(&l.end));
            vec![a, geo::medio(a, b), b]
        }
        EntityType::Circle(c) => {
            let k = p2(&c.center);
            let mut v = vec![k];
            v.extend(
                (0..4).map(|q| geo::polar(k, c.radius, q as f64 * std::f64::consts::FRAC_PI_2)),
            );
            v
        }
        EntityType::Arc(_) => {
            let Some(Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            }) = formas::formas(e).first().copied()
            else {
                return Vec::new();
            };
            vec![
                c,
                geo::polar(c, r, inicio),
                geo::polar(c, r, inicio + barrido / 2.0),
                geo::polar(c, r, inicio + barrido),
            ]
        }
        EntityType::LwPolyline(p) => p
            .vertices
            .iter()
            .map(|v| [v.location.x, v.location.y])
            .collect(),
        otro => formas::puntos_de_insercion(otro),
    }
}

/// **Mueve el pinzamiento `i`** a `q`: estira un extremo, cambia el radio,
/// mueve un vertice, o (el del medio y el centro) lo mueve entero.
pub fn mover_pinzamiento(e: &mut EntityType, i: usize, q: P) -> bool {
    let antes = pinzamientos(e);
    let Some(p) = antes.get(i).copied() else {
        return false;
    };
    match e {
        EntityType::Line(l) => match i {
            0 => l.start = Vector3::new(q[0], q[1], l.start.z),
            2 => l.end = Vector3::new(q[0], q[1], l.end.z),
            _ => trasladar(e, geo::restar(q, p)),
        },
        EntityType::Circle(c) => {
            if i == 0 {
                trasladar(e, geo::restar(q, p));
            } else {
                let r = geo::dist(p2(&c.center), q);
                if r <= geo::EPS {
                    return false;
                }
                c.radius = r;
            }
        }
        EntityType::Arc(_) => {
            if i == 0 {
                trasladar(e, geo::restar(q, p));
            } else {
                // Un extremo o el medio nuevos: el arco por los tres puntos.
                let mut tres = [antes[1], antes[2], antes[3]];
                tres[i - 1] = q;
                let Some(Forma::Arco {
                    c,
                    r,
                    inicio,
                    barrido,
                }) = Forma::arco_por_tres(tres[0], tres[1], tres[2])
                else {
                    return false;
                };
                let nuevo = arco(c, r, inicio, inicio + barrido);
                let comun = e.common().clone();
                *e = nuevo;
                *e.common_mut() = comun;
            }
        }
        EntityType::LwPolyline(pl) => {
            pl.vertices[i].location = opencadcodec::Vector2::new(q[0], q[1]);
        }
        _ => trasladar(e, geo::restar(q, p)),
    }
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::Dibujo;
    use crate::especialidad::{circulo, polilinea};

    fn cerca(a: P, b: P) -> bool {
        geo::dist(a, b) < 1e-6
    }

    fn linea_de(e: &EntityType) -> (P, P) {
        extremos_de_linea(e).unwrap()
    }

    #[test]
    fn mover_girar_escalar_y_simetria() {
        let mut e = linea([0.0, 0.0], [10.0, 0.0]);
        trasladar(&mut e, [1.0, 2.0]);
        assert_eq!(linea_de(&e), ([1.0, 2.0], [11.0, 2.0]));
        let mut e = linea([0.0, 0.0], [10.0, 0.0]);
        girar(&mut e, [0.0, 0.0], std::f64::consts::FRAC_PI_2);
        assert!(cerca(linea_de(&e).1, [0.0, 10.0]));
        let mut e = linea([1.0, 1.0], [2.0, 1.0]);
        escalar(&mut e, [1.0, 1.0], 3.0);
        assert!(cerca(linea_de(&e).1, [4.0, 1.0]));
        let mut e = linea([1.0, 1.0], [3.0, 2.0]);
        simetria(&mut e, [0.0, 0.0], [0.0, 1.0]);
        assert!(cerca(linea_de(&e).0, [-1.0, 1.0]) && cerca(linea_de(&e).1, [-3.0, 2.0]));
    }

    #[test]
    fn desfase_de_linea_circulo_y_polilinea() {
        let l = desfase(&linea([0.0, 0.0], [10.0, 0.0]), 2.0, [5.0, -7.0]).unwrap();
        assert_eq!(linea_de(&l), ([0.0, -2.0], [10.0, -2.0]));
        let EntityType::Circle(c) = desfase(&circulo([0.0, 0.0], 5.0), 1.0, [1.0, 0.0]).unwrap()
        else {
            panic!()
        };
        assert_eq!(c.radius, 4.0);
        assert!(desfase(&circulo([0.0, 0.0], 1.0), 2.0, [0.1, 0.0]).is_none());
        // Un cuadrado hacia fuera crece 1 por lado.
        let sq = polilinea(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]], true);
        let EntityType::LwPolyline(p) = desfase(&sq, 1.0, [5.0, -3.0]).unwrap() else {
            panic!()
        };
        assert!(cerca(
            [p.vertices[0].location.x, p.vertices[0].location.y],
            [-1.0, -1.0]
        ));
        assert!(cerca(
            [p.vertices[2].location.x, p.vertices[2].location.y],
            [11.0, 11.0]
        ));
    }

    #[test]
    fn recortar_el_trozo_del_medio_y_el_de_la_punta() {
        let mut d = Dibujo::nuevo();
        let mut h = opencadcodec::Handle::NULL;
        d.editar("x", |t| {
            h = t.anadir(linea([0.0, 0.0], [10.0, 0.0])).unwrap();
            t.anadir(linea([3.0, -1.0], [3.0, 1.0]));
            t.anadir(linea([7.0, -1.0], [7.0, 1.0]));
        });
        let ind = d.indice();
        let e = d.entidad(h).unwrap().clone();
        let v = recortar(&ind, h, &e, [5.0, 0.0], None).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(linea_de(&v[0]), ([0.0, 0.0], [3.0, 0.0]));
        assert_eq!(linea_de(&v[1]), ([7.0, 0.0], [10.0, 0.0]));
        let v = recortar(&ind, h, &e, [9.0, 0.0], None).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(linea_de(&v[0]), ([0.0, 0.0], [7.0, 0.0]));
    }

    #[test]
    fn recortar_un_circulo_entre_dos_cortes() {
        let mut d = Dibujo::nuevo();
        let mut h = opencadcodec::Handle::NULL;
        d.editar("x", |t| {
            h = t.anadir(circulo([0.0, 0.0], 5.0)).unwrap();
            t.anadir(linea([-10.0, 0.0], [10.0, 0.0]));
        });
        let ind = d.indice();
        let e = d.entidad(h).unwrap().clone();
        // Pick arriba: queda el semicirculo de abajo.
        let v = recortar(&ind, h, &e, [0.0, 5.0], None).unwrap();
        let f = formas::formas(&v[0]);
        assert!(cerca(f[0].punto_medio().unwrap(), [0.0, -5.0]), "{f:?}");
    }

    #[test]
    fn alargar_una_linea_hasta_un_muro() {
        let mut d = Dibujo::nuevo();
        let mut h = opencadcodec::Handle::NULL;
        d.editar("x", |t| {
            h = t.anadir(linea([0.0, 0.0], [5.0, 0.0])).unwrap();
            t.anadir(linea([8.0, -5.0], [8.0, 5.0]));
            t.anadir(linea([12.0, -5.0], [12.0, 5.0]));
        });
        let ind = d.indice();
        let e = d.entidad(h).unwrap().clone();
        let n = alargar(&ind, h, &e, [4.9, 0.0], None).unwrap();
        assert_eq!(linea_de(&n), ([0.0, 0.0], [8.0, 0.0]));
        // Por el otro extremo no hay nada.
        assert!(alargar(&ind, h, &e, [0.1, 0.0], None).is_none());
    }

    #[test]
    fn empalme_en_esquina_y_con_radio() {
        let (l1, l2) = (([0.0, 0.0], [8.0, 0.0]), ([10.0, 2.0], [10.0, 10.0]));
        let (a, b, arco) = empalme(l1, [1.0, 0.0], l2, [10.0, 9.0], 0.0).unwrap();
        assert!(arco.is_none());
        assert_eq!(a, ([0.0, 0.0], [10.0, 0.0]));
        assert_eq!(b, ([10.0, 0.0], [10.0, 10.0]));
        let (a, b, arco) = empalme(l1, [1.0, 0.0], l2, [10.0, 9.0], 2.0).unwrap();
        assert!(cerca(a.1, [8.0, 0.0]) && cerca(b.0, [10.0, 2.0]));
        let EntityType::Arc(k) = arco.unwrap() else {
            panic!()
        };
        assert!(cerca(p2(&k.center), [8.0, 2.0]));
        assert!((k.sweep_angle() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        let (_, _, ch) = chaflan(l1, [1.0, 0.0], l2, [10.0, 9.0], 1.0, 1.0).unwrap();
        assert_eq!(linea_de(&ch.unwrap()), ([9.0, 0.0], [10.0, 1.0]));
    }

    #[test]
    fn matrices() {
        let e = circulo([0.0, 0.0], 1.0);
        assert_eq!(matriz_rectangular(&e, 3, 4, 10.0, 10.0).len(), 11);
        let v = matriz_polar(&e.clone(), [10.0, 0.0], 4, TAU);
        assert_eq!(v.len(), 3);
        let EntityType::Circle(c) = &v[1] else {
            panic!()
        };
        assert!(cerca(p2(&c.center), [20.0, 0.0]));
    }

    #[test]
    fn pinzamientos_que_estiran() {
        let mut e = linea([0.0, 0.0], [10.0, 0.0]);
        assert_eq!(pinzamientos(&e).len(), 3);
        mover_pinzamiento(&mut e, 2, [10.0, 5.0]);
        assert_eq!(linea_de(&e).1, [10.0, 5.0]);
        let mut c = circulo([0.0, 0.0], 1.0);
        mover_pinzamiento(&mut c, 1, [3.0, 0.0]);
        let EntityType::Circle(k) = c else { panic!() };
        assert_eq!(k.radius, 3.0);
    }
}
