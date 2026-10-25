//! **El contorno de un sombreado desde un punto de dentro**: con lo que hay
//! cerca del punto (por el indice, nunca todo el plano) se arma el grafo
//! plano de rayas y arcos cortados entre si, se recorren sus caras y se toma
//! la mas pequena que encierra el punto, con sus islas.
//!
//! Vale igual para una polilinea cerrada o un circulo que para cuatro lineas
//! sueltas que se cruzan, o un muro hecho de lineas y arcos. Los huecos
//! pequenos entre extremos que casi se tocan se cierran (a menos de una
//! tolerancia, como HPGAPTOL), y se sabe que entidades forman cada borde
//! (para que el sombreado las siga al cambiar).

use std::collections::HashMap;
use std::f64::consts::TAU;

use opencadcodec::{EntityType, Handle};

use crate::dibujo::Dibujo;
use crate::geo::{self, Caja, Forma, P};
use crate::patrones::Anillo;

/// Cuantas formas se juntan como mucho (si no, la zona es demasiado grande).
pub const MAX_FORMAS: usize = 8000;

/// **El contorno encontrado**: la cara que encierra el punto y las islas
/// que tiene dentro.
#[derive(Debug, Clone)]
pub struct Contorno {
    pub exterior: Anillo,
    pub islas: Vec<Anillo>,
    /// El area de la cara (sin quitar las islas).
    pub area: f64,
    /// Las entidades que forman el borde de fuera (sin repetir).
    pub bordes: Vec<Handle>,
    /// Las que forman cada isla.
    pub bordes_islas: Vec<Vec<Handle>>,
}

/// **Busca el contorno cerrado mas pequeno que encierra `p`.** Empieza
/// mirando cerca y agranda la zona solo si hace falta: hasta que la cara
/// cabe entera en lo mirado (asi nada de fuera puede cortarla). Los huecos
/// de menos de `hueco` entre extremos sueltos se cierran (0: ninguno).
pub fn buscar(d: &Dibujo, p: P, hueco: f64) -> Option<Contorno> {
    let ind = d.indice();
    let total = ind.caja()?;
    if !geo::en_caja(&total, p) {
        return None;
    }
    let lado = (total[2] - total[0]).max(total[3] - total[1]).max(1e-9);
    let mut r = lado / 256.0;
    for _ in 0..40 {
        let zona = [p[0] - r, p[1] - r, p[0] + r, p[1] + r];
        let todo = geo::contiene(&zona, &total);
        let mut formas = Vec::new();
        let mut duenos = Vec::new();
        for e in ind.en(&zona) {
            if matches!(
                d.entidad(e.handle),
                Some(EntityType::Hatch(_) | EntityType::Dimension(_))
            ) {
                continue;
            }
            for f in e.formas.iter().filter(|f| geo::se_tocan(&f.caja(), &zona)) {
                formas.push(*f);
                duenos.push(e.handle);
            }
        }
        if formas.len() > MAX_FORMAS {
            return None;
        }
        let tol = 1e-6 * (1.0 + r);
        match cara(&formas, &duenos, p, tol, hueco) {
            Some((c, caja)) if todo || geo::contiene(&zona, &caja) => return Some(c),
            Some((_, caja)) => {
                // Se sale de lo mirado: agrandar hasta cubrirla.
                let m = (caja[0] - p[0])
                    .abs()
                    .max((caja[2] - p[0]).abs())
                    .max((caja[1] - p[1]).abs())
                    .max((caja[3] - p[1]).abs());
                r = (m * 1.01).max(r * 1.5);
            }
            None if todo => return None,
            None => r *= 4.0,
        }
    }
    None
}

/// **El contorno que encierra `p` hecho solo con estas formas** (cada una
/// con la entidad de la que sale): para volver a hacer un sombreado con sus
/// bordes de siempre, sin mirar lo demas del dibujo.
pub fn de_formas(formas: &[(Forma, Handle)], p: P, hueco: f64) -> Option<Contorno> {
    if formas.is_empty() || formas.len() > MAX_FORMAS {
        return None;
    }
    let caja = formas
        .iter()
        .fold(geo::CAJA_VACIA, |c, (f, _)| geo::unir_cajas(c, f.caja()));
    let lado = (caja[2] - caja[0]).max(caja[3] - caja[1]).max(0.0);
    let (fs, ds): (Vec<Forma>, Vec<Handle>) = formas.iter().copied().unzip();
    cara(&fs, &ds, p, 1e-6 * (1.0 + lado), hueco).map(|(c, _)| c)
}

/// Que parte del tamano del dibujo es la tolerancia de huecos por defecto.
pub const HUECO_RELATIVO: f64 = 1e-4;

/// **La tolerancia de huecos por defecto**: una diezmilesima de la diagonal
/// de lo dibujado (5 mm en un plano de 50 m), que no se ve en pantalla.
pub fn hueco_por_defecto(d: &Dibujo) -> f64 {
    d.indice()
        .caja()
        .map_or(0.0, |c| (c[2] - c[0]).hypot(c[3] - c[1]) * HUECO_RELATIVO)
}

/// Unas cajas repartidas en celdas de un lado fijo, para saber cuales
/// pueden tener un punto sin mirarlas todas (las muy grandes van aparte).
struct Rejilla {
    lado: f64,
    celdas: HashMap<(i64, i64), Vec<usize>>,
    grandes: Vec<usize>,
}

impl Rejilla {
    fn de(cajas: &[Caja], lado: f64) -> Rejilla {
        let mut r = Rejilla {
            lado,
            celdas: HashMap::new(),
            grandes: Vec::new(),
        };
        for (k, c) in cajas.iter().enumerate() {
            let (x0, y0) = r.celda([c[0], c[1]]);
            let (x1, y1) = r.celda([c[2], c[3]]);
            if (x1 - x0 + 1).saturating_mul(y1 - y0 + 1) > 64 {
                r.grandes.push(k);
                continue;
            }
            for x in x0..=x1 {
                for y in y0..=y1 {
                    r.celdas.entry((x, y)).or_default().push(k);
                }
            }
        }
        r
    }

    fn celda(&self, p: P) -> (i64, i64) {
        (
            (p[0] / self.lado).floor() as i64,
            (p[1] / self.lado).floor() as i64,
        )
    }

    /// Las cajas que pueden tener `q` (las de su celda y las grandes).
    fn en(&self, q: P, mut f: impl FnMut(usize)) {
        if let Some(v) = self.celdas.get(&self.celda(q)) {
            v.iter().for_each(|k| f(*k));
        }
        self.grandes.iter().for_each(|k| f(*k));
    }
}

/// **Los puentes que cierran los huecos**: cada extremo suelto (que no toca
/// nada) se une con el extremo suelto mas cercano a menos de `hueco` (los
/// pares mas cortos primero) o, si no le queda ninguno, con lo mas cercano
/// de otra forma.
fn puentes(formas: &[Forma], hueco: f64, tol: f64) -> Vec<Forma> {
    let cajas: Vec<Caja> = formas
        .iter()
        .map(|f| geo::ensanchar(f.caja(), hueco.max(tol)))
        .collect();
    let total = cajas
        .iter()
        .fold(geo::CAJA_VACIA, |c, k| geo::unir_cajas(c, *k));
    let lado = (hueco * 4.0)
        .max((total[2] - total[0]).max(total[3] - total[1]) / 256.0)
        .max(1e-9);
    let rej = Rejilla::de(&cajas, lado);
    let mut sueltos: Vec<(usize, P)> = Vec::new();
    for (i, f) in formas.iter().enumerate() {
        let ext = f.extremos();
        for (k, q) in ext.iter().enumerate() {
            let cerrada = ext
                .iter()
                .enumerate()
                .any(|(m, o)| m != k && geo::dist(*o, *q) <= tol);
            if cerrada {
                continue;
            }
            let mut toca = false;
            rej.en(*q, |j| {
                toca |= j != i && geo::en_caja(&cajas[j], *q) && formas[j].distancia(*q) <= tol;
            });
            if !toca {
                sueltos.push((i, *q));
            }
        }
    }
    if sueltos.is_empty() {
        return Vec::new();
    }
    // Los pares de extremos sueltos, de mas cerca a mas lejos.
    let mut orden: Vec<usize> = (0..sueltos.len()).collect();
    orden.sort_by(|a, b| sueltos[*a].1[0].total_cmp(&sueltos[*b].1[0]));
    let mut pares = Vec::new();
    for (k, &a) in orden.iter().enumerate() {
        for &b in &orden[k + 1..] {
            if sueltos[b].1[0] - sueltos[a].1[0] > hueco {
                break;
            }
            let d = geo::dist(sueltos[a].1, sueltos[b].1);
            if d <= hueco && d > tol {
                pares.push((d, a, b));
            }
        }
    }
    pares.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut usado = vec![false; sueltos.len()];
    let mut out = Vec::new();
    for (_, a, b) in pares {
        if usado[a] || usado[b] {
            continue;
        }
        usado[a] = true;
        usado[b] = true;
        out.push(Forma::Segmento {
            a: sueltos[a].1,
            b: sueltos[b].1,
        });
    }
    // Los que quedan: a lo mas cercano de otra forma.
    for (k, (i, q)) in sueltos.iter().enumerate() {
        if usado[k] {
            continue;
        }
        let mut mejor: Option<(f64, P)> = None;
        rej.en(*q, |j| {
            if j == *i || !geo::en_caja(&cajas[j], *q) {
                return;
            }
            let c = formas[j].cercano(*q);
            let d = geo::dist(c, *q);
            if d <= hueco && d > tol && mejor.is_none_or(|(m, _)| d < m) {
                mejor = Some((d, c));
            }
        });
        if let Some((_, c)) = mejor {
            out.push(Forma::Segmento { a: *q, b: c });
        }
    }
    out
}

/// Un tramo del grafo: una raya de `a` a `b` o un arco antihorario.
#[derive(Debug, Clone, Copy)]
enum Pieza {
    Seg(P, P),
    Arco { c: P, r: f64, ini: f64, barr: f64 },
}

impl Pieza {
    /// El punto a la fraccion `s` (0 al principio, 1 al final).
    fn punto(&self, s: f64) -> P {
        match *self {
            Pieza::Seg(a, b) => geo::sumar(a, geo::por(geo::restar(b, a), s)),
            Pieza::Arco { c, r, ini, barr } => geo::polar(c, r, ini + barr * s),
        }
    }

    fn longitud(&self) -> f64 {
        match *self {
            Pieza::Seg(a, b) => geo::dist(a, b),
            Pieza::Arco { r, barr, .. } => r * barr,
        }
    }

    fn abombado(&self) -> f64 {
        match *self {
            Pieza::Seg(..) => 0.0,
            Pieza::Arco { barr, .. } => (barr / 4.0).tan(),
        }
    }

    fn muestras(&self) -> usize {
        match *self {
            Pieza::Seg(..) => 1,
            Pieza::Arco { barr, .. } => ((barr / (TAU / 64.0)).ceil() as usize).max(2),
        }
    }
}

/// Las fracciones donde empieza y acaba cada forma (un circulo, partido en dos).
fn iniciales(f: &Forma) -> Vec<f64> {
    match f {
        Forma::Circulo { .. } => vec![0.0, 0.5],
        _ => vec![0.0, 1.0],
    }
}

/// La fraccion de la forma en la que cae `q` (que esta sobre ella).
fn fraccion(f: &Forma, q: P) -> f64 {
    match *f {
        Forma::Segmento { a, b } => geo::parametro_en_segmento(a, b, q).clamp(0.0, 1.0),
        Forma::Circulo { c, .. } => geo::normalizar(geo::angulo(c, q)) / TAU,
        Forma::Arco {
            c, inicio, barrido, ..
        } => {
            let d = geo::desde(inicio, geo::angulo(c, q));
            let d = if d <= barrido {
                d
            } else if d > barrido + (TAU - barrido) / 2.0 {
                0.0
            } else {
                barrido
            };
            d / barrido.max(geo::EPS)
        }
    }
}

/// El tramo de la forma entre dos fracciones.
fn pieza(f: &Forma, s0: f64, s1: f64) -> Pieza {
    match *f {
        Forma::Segmento { a, b } => {
            let d = geo::restar(b, a);
            Pieza::Seg(
                geo::sumar(a, geo::por(d, s0)),
                geo::sumar(a, geo::por(d, s1)),
            )
        }
        Forma::Circulo { c, r } => Pieza::Arco {
            c,
            r,
            ini: TAU * s0,
            barr: TAU * (s1 - s0),
        },
        Forma::Arco {
            c,
            r,
            inicio,
            barrido,
        } => Pieza::Arco {
            c,
            r,
            ini: inicio + barrido * s0,
            barr: barrido * (s1 - s0),
        },
    }
}

/// Los nudos del grafo: los puntos que caen a menos de `tol`, juntos.
struct Nudos {
    tol: f64,
    puntos: Vec<P>,
    rejilla: HashMap<(i64, i64), Vec<usize>>,
}

impl Nudos {
    fn de(&mut self, p: P) -> usize {
        let celda = |v: f64| (v / self.tol).floor() as i64;
        let (cx, cy) = (celda(p[0]), celda(p[1]));
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = self.rejilla.get(&(cx + dx, cy + dy)) {
                    for &k in v {
                        if geo::dist(self.puntos[k], p) <= self.tol {
                            return k;
                        }
                    }
                }
            }
        }
        let k = self.puntos.len();
        self.puntos.push(p);
        self.rejilla.entry((cx, cy)).or_default().push(k);
        k
    }
}

/// Una arista del grafo; la media arista `2k` va de `u` a `v` y la `2k+1` al reves.
struct Arista {
    u: usize,
    v: usize,
    pieza: Pieza,
    /// La forma de la que sale.
    forma: usize,
}

/// La cara mas pequena del grafo de `formas` (cada una de la entidad
/// `duenos[k]`) que encierra `p`, con su caja, cerrando antes los huecos de
/// menos de `hueco`.
fn cara(
    formas: &[Forma],
    duenos: &[Handle],
    p: P,
    tol: f64,
    hueco: f64,
) -> Option<(Contorno, Caja)> {
    let mut formas = formas.to_vec();
    let mut duenos = duenos.to_vec();
    if hueco > tol {
        for f in puentes(&formas, hueco, tol) {
            formas.push(f);
            duenos.push(Handle::NULL);
        }
    }
    let n = formas.len();
    if n == 0 {
        return None;
    }
    // 1. Donde se cortan (y donde un extremo toca a otra forma).
    let cajas: Vec<Caja> = formas
        .iter()
        .map(|f| geo::ensanchar(f.caja(), tol))
        .collect();
    let mut fr: Vec<Vec<f64>> = formas.iter().map(iniciales).collect();
    let mut orden: Vec<usize> = (0..n).collect();
    orden.sort_by(|a, b| cajas[*a][0].total_cmp(&cajas[*b][0]));
    for (k, &i) in orden.iter().enumerate() {
        for &j in &orden[k + 1..] {
            if cajas[j][0] > cajas[i][2] {
                break;
            }
            if !geo::se_tocan(&cajas[i], &cajas[j]) {
                continue;
            }
            for q in geo::cortes(&formas[i], &formas[j]) {
                fr[i].push(fraccion(&formas[i], q));
                fr[j].push(fraccion(&formas[j], q));
            }
            for q in formas[j].extremos() {
                if formas[i].distancia(q) <= tol {
                    fr[i].push(fraccion(&formas[i], q));
                }
            }
            for q in formas[i].extremos() {
                if formas[j].distancia(q) <= tol {
                    fr[j].push(fraccion(&formas[j], q));
                }
            }
        }
    }
    // 2. Los tramos entre cortes, con sus nudos (sin repetir los que se pisan).
    let mut nudos = Nudos {
        tol,
        puntos: Vec::new(),
        rejilla: HashMap::new(),
    };
    let mut aristas: Vec<Arista> = Vec::new();
    let mut vistas: HashMap<(usize, usize), Vec<P>> = HashMap::new();
    for (i, (f, v)) in formas.iter().zip(fr.iter_mut()).enumerate() {
        v.sort_by(|a, b| a.total_cmp(b));
        v.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        let mut tramos: Vec<(f64, f64)> = v.windows(2).map(|w| (w[0], w[1])).collect();
        if matches!(f, Forma::Circulo { .. }) {
            tramos.push((v[v.len() - 1], v[0] + 1.0));
        }
        for (s0, s1) in tramos {
            let pz = pieza(f, s0, s1);
            if pz.longitud() <= tol {
                continue;
            }
            let (u, w) = (nudos.de(pz.punto(0.0)), nudos.de(pz.punto(1.0)));
            if u == w {
                continue;
            }
            let medio = pz.punto(0.5);
            let clave = (u.min(w), u.max(w));
            let repetida = vistas
                .get(&clave)
                .is_some_and(|ms| ms.iter().any(|m| geo::dist(*m, medio) <= tol * 10.0));
            if repetida {
                continue;
            }
            vistas.entry(clave).or_default().push(medio);
            aristas.push(Arista {
                u,
                v: w,
                pieza: pz,
                forma: i,
            });
        }
    }
    // 3. Fuera las colas sueltas (los nudos de una sola arista, una y otra vez).
    let nn = nudos.puntos.len();
    let mut grado = vec![0usize; nn];
    let mut de_nudo: Vec<Vec<usize>> = vec![Vec::new(); nn];
    for (k, a) in aristas.iter().enumerate() {
        grado[a.u] += 1;
        grado[a.v] += 1;
        de_nudo[a.u].push(k);
        de_nudo[a.v].push(k);
    }
    let mut viva = vec![true; aristas.len()];
    let mut cola: Vec<usize> = (0..nn).filter(|k| grado[*k] == 1).collect();
    while let Some(x) = cola.pop() {
        for &k in &de_nudo[x] {
            if viva[k] {
                viva[k] = false;
                let a = &aristas[k];
                for y in [a.u, a.v] {
                    grado[y] -= 1;
                    if grado[y] == 1 {
                        cola.push(y);
                    }
                }
            }
        }
    }
    // 4. Las medias aristas que salen de cada nudo, por el angulo con que salen.
    let origen = |h: usize| {
        let a = &aristas[h / 2];
        if h % 2 == 0 { a.u } else { a.v }
    };
    let en = |h: usize, s: f64| {
        let a = &aristas[h / 2];
        if h % 2 == 0 {
            a.pieza.punto(s)
        } else {
            a.pieza.punto(1.0 - s)
        }
    };
    let mut salen: Vec<Vec<(f64, usize)>> = vec![Vec::new(); nn];
    for k in (0..aristas.len()).filter(|k| viva[*k]) {
        for h in [2 * k, 2 * k + 1] {
            // Un poco mas alla del nudo: asi una raya y un arco tangentes se distinguen.
            let ang = geo::angulo(en(h, 0.0), en(h, 1e-4));
            salen[origen(h)].push((ang, h));
        }
    }
    let mut pos = vec![0usize; aristas.len() * 2];
    for v in &mut salen {
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (i, (_, h)) in v.iter().enumerate() {
            pos[*h] = i;
        }
    }
    // La siguiente con la cara a la izquierda: la de antes de la vuelta, en el nudo de llegada.
    let siguiente = |h: usize| {
        let t = h ^ 1;
        let l = &salen[origen(t)];
        l[(pos[t] + l.len() - 1) % l.len()].1
    };
    // 5. Las caras.
    let mut hecha = vec![false; aristas.len() * 2];
    let mut ciclos: Vec<(Vec<usize>, Vec<P>, f64, Caja)> = Vec::new();
    for k in (0..aristas.len()).filter(|k| viva[*k]) {
        for h0 in [2 * k, 2 * k + 1] {
            if hecha[h0] {
                continue;
            }
            let mut ciclo = Vec::new();
            let mut h = h0;
            while !hecha[h] && ciclo.len() <= 4 * aristas.len() {
                hecha[h] = true;
                ciclo.push(h);
                h = siguiente(h);
            }
            if h != h0 {
                continue;
            }
            let mut pts = Vec::new();
            for &h in &ciclo {
                let m = aristas[h / 2].pieza.muestras();
                for i in 0..m {
                    pts.push(en(h, i as f64 / m as f64));
                }
            }
            // El area exacta (los arcos con su casquete): la mitad de la
            // integral de x dy - y dx por el borde.
            let area = ciclo
                .iter()
                .map(|&h| {
                    let (p0, p1) = (en(h, 0.0), en(h, 1.0));
                    match aristas[h / 2].pieza {
                        Pieza::Seg(..) => geo::cruz(p0, p1),
                        Pieza::Arco { c, r, barr, .. } => {
                            let b = if h % 2 == 0 { barr } else { -barr };
                            geo::cruz(c, geo::restar(p1, p0)) + r * r * b
                        }
                    }
                })
                .sum::<f64>()
                / 2.0;
            let caja = geo::caja_de(&pts);
            ciclos.push((ciclo, pts, area, caja));
        }
    }
    // La cara (antihoraria) mas pequena que tiene el punto.
    let (i, _) = ciclos
        .iter()
        .enumerate()
        .filter(|(_, c)| c.2 > 0.0 && geo::en_caja(&c.3, p) && dentro(&c.1, p))
        .min_by(|a, b| a.1.2.total_cmp(&b.1.2))?;
    let anillo = |ciclo: &[usize]| -> Anillo {
        ciclo
            .iter()
            .map(|&h| {
                let b = aristas[h / 2].pieza.abombado();
                (en(h, 0.0), if h % 2 == 0 { b } else { -b })
            })
            .collect()
    };
    let bordes = |ciclo: &[usize]| -> Vec<Handle> {
        let mut v: Vec<Handle> = Vec::new();
        for &h in ciclo {
            let d = duenos[aristas[h / 2].forma];
            if !d.is_null() && !v.contains(&d) {
                v.push(d);
            }
        }
        v
    };
    let (ciclo, pts, area, caja) = &ciclos[i];
    // Las islas: el borde de fuera (horario) de lo que hay suelto dentro.
    let de_islas: Vec<&Vec<usize>> = ciclos
        .iter()
        .filter(|c| {
            c.2 < 0.0
                && geo::contiene(caja, &c.3)
                && !dentro(&c.1, p)
                && c.1.iter().all(|q| dentro(pts, *q))
        })
        .map(|c| &c.0)
        .collect();
    Some((
        Contorno {
            exterior: anillo(ciclo),
            islas: de_islas.iter().map(|c| anillo(c)).collect(),
            area: *area,
            bordes: bordes(ciclo),
            bordes_islas: de_islas.iter().map(|c| bordes(c)).collect(),
        },
        *caja,
    ))
}

/// Si `p` cae dentro del poligono (par o impar).
pub fn dentro(pol: &[P], p: P) -> bool {
    let n = pol.len();
    let mut d = false;
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (pol[i], pol[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                d = !d;
            }
        }
        j = i;
    }
    d
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::especialidad as esp;

    fn dibujo(v: Vec<EntityType>) -> Dibujo {
        let mut d = Dibujo::nuevo();
        d.editar("X", |t| {
            for e in v {
                t.anadir(e);
            }
        });
        d
    }

    #[test]
    fn cuatro_lineas_que_se_cruzan_dan_un_cuadrado() {
        // Como un #: las lineas sobresalen y la cara es la del medio.
        let d = dibujo(vec![
            esp::linea([-10.0, 0.0], [110.0, 0.0]),
            esp::linea([-10.0, 100.0], [110.0, 100.0]),
            esp::linea([0.0, -10.0], [0.0, 110.0]),
            esp::linea([100.0, -10.0], [100.0, 110.0]),
        ]);
        let c = buscar(&d, [50.0, 50.0], 0.0).unwrap();
        assert!((c.area - 10_000.0).abs() < 1e-6, "{}", c.area);
        assert_eq!(c.exterior.len(), 4);
        // Las cuatro lineas son el borde.
        assert_eq!(c.bordes.len(), 4);
        assert!(buscar(&d, [105.0, 50.0], 0.0).is_none());
    }

    /// Un cuadrado de 100 hecho de cuatro lineas con huecos de 2 en las
    /// esquinas (y una que se pasa).
    fn con_huecos() -> Dibujo {
        dibujo(vec![
            esp::linea([0.0, 0.0], [98.0, 0.0]),
            esp::linea([100.0, 2.0], [100.0, 100.0]),
            esp::linea([99.0, 101.5], [0.0, 101.5]),
            esp::linea([0.0, 100.0], [0.0, 1.5]),
        ])
    }

    #[test]
    fn los_huecos_pequenos_se_cierran_y_los_grandes_no() {
        let d = con_huecos();
        // Sin tolerancia no hay contorno.
        assert!(buscar(&d, [50.0, 50.0], 0.0).is_none());
        // Con 1 tampoco: los huecos son de 1,5 a 2,8.
        assert!(buscar(&d, [50.0, 50.0], 1.0).is_none());
        let c = buscar(&d, [50.0, 50.0], 3.0).unwrap();
        assert!((c.area - 10_000.0).abs() < 400.0, "{}", c.area);
        // Los puentes no son entidades: el borde son las cuatro lineas.
        assert_eq!(c.bordes.len(), 4);
        // Caso negativo: fuera sigue sin haber nada.
        assert!(buscar(&d, [150.0, 50.0], 3.0).is_none());
    }

    #[test]
    fn un_extremo_cerca_de_una_linea_se_une_a_ella() {
        // Una T que no llega: la vertical se queda a 1 de la horizontal.
        let d = dibujo(vec![
            esp::polilinea(&[[0.0, 0.0], [100.0, 0.0], [100.0, 100.0]], false),
            esp::linea([100.0, 100.0], [0.0, 100.0]),
            esp::linea([0.0, 100.0], [0.0, 0.0]),
            esp::linea([50.0, 99.0], [50.0, 1.0]),
        ]);
        // Sin tolerancia la del medio es una cola suelta: un solo recinto.
        let c = buscar(&d, [25.0, 50.0], 0.0).unwrap();
        assert!((c.area - 10_000.0).abs() < 1e-6);
        // Con tolerancia parte el cuadrado en dos.
        let c = buscar(&d, [25.0, 50.0], 1.5).unwrap();
        assert!((c.area - 5_000.0).abs() < 1.0, "{}", c.area);
        let c = buscar(&d, [75.0, 50.0], 1.5).unwrap();
        assert!((c.area - 5_000.0).abs() < 1.0, "{}", c.area);
    }

    #[test]
    fn de_formas_solo_mira_las_suyas() {
        let d = con_huecos();
        let ind = d.indice();
        let formas: Vec<(Forma, Handle)> = ind
            .entradas
            .iter()
            .flat_map(|e| e.formas.iter().map(move |f| (*f, e.handle)))
            .collect();
        let c = de_formas(&formas, [50.0, 50.0], 3.0).unwrap();
        assert_eq!(c.bordes.len(), 4);
        assert!(de_formas(&formas, [50.0, 50.0], 0.0).is_none());
        assert!(de_formas(&[], [0.0, 0.0], 1.0).is_none());
    }

    #[test]
    fn un_circulo_con_una_isla_cuadrada() {
        let d = dibujo(vec![
            esp::circulo([0.0, 0.0], 100.0),
            esp::polilinea(
                &[[-20.0, -20.0], [20.0, -20.0], [20.0, 20.0], [-20.0, 20.0]],
                true,
            ),
        ]);
        let c = buscar(&d, [60.0, 0.0], 0.0).unwrap();
        assert!((c.area - std::f64::consts::PI * 10_000.0).abs() < 1.0);
        assert_eq!(c.islas.len(), 1);
        assert_eq!((c.bordes.len(), c.bordes_islas.len()), (1, 1));
        assert_ne!(c.bordes, c.bordes_islas[0]);
        // Dentro de la isla, el contorno es la isla.
        let c = buscar(&d, [0.0, 0.0], 0.0).unwrap();
        assert!((c.area - 1600.0).abs() < 1e-6 && c.islas.is_empty());
    }

    #[test]
    fn el_punto_dentro_de_un_poligono() {
        let cuadro = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        assert!(dentro(&cuadro, [0.5, 0.5]));
        assert!(!dentro(&cuadro, [1.5, 0.5]));
    }
}
