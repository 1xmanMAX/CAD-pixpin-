//! **Modelar desde la planta**: extruir contornos cerrados, levantar los
//! muros dibujados con MURO (sus dos caras) o por su eje, con los vanos de
//! las puertas y ventanas que tienen encima, y las losas.

use opencadcodec::EntityType;

use super::geometria::{self as g, Malla, Prisma};
use crate::formas;
use crate::geo::{self, Forma, P};
use crate::herramientas::sombreado::anillo_de;

/// **La planta de un contorno cerrado** (polilinea cerrada o circulo), con
/// los arcos en tramos, y su cota (la elevacion de la entidad).
pub fn planta_de(e: &EntityType) -> Option<(Vec<P>, f64)> {
    let anillo = anillo_de(e)?;
    let z = match e {
        EntityType::LwPolyline(p) => p.elevation,
        EntityType::Circle(c) => c.center.z,
        _ => 0.0,
    };
    let pts = g::poligono_de_anillo(&anillo);
    (pts.len() >= 3 && geo::area_poligono(&pts).abs() > 1e-12).then_some((pts, z))
}

/// **EXTRUIR un contorno cerrado** `alto` hacia arriba (hacia abajo si es
/// negativo), desde su cota.
pub fn extruir(e: &EntityType, alto: f64) -> Result<Prisma, String> {
    if !alto.is_finite() || alto.abs() < 1e-9 {
        return Err("La altura no puede ser cero".into());
    }
    let (planta, z) =
        planta_de(e).ok_or("No es un contorno cerrado (polilinea cerrada o circulo)")?;
    Prisma::nuevo(planta, z, alto).ok_or_else(|| "El contorno no tiene area".into())
}

/// **Una LOSA**: el contorno con su `espesor` por debajo de `nivel` (la
/// cota de su cara de arriba, la del piso terminado).
pub fn losa(e: &EntityType, espesor: f64, nivel: f64) -> Result<Prisma, String> {
    if !espesor.is_finite() || espesor <= 1e-9 {
        return Err("El espesor tiene que ser mayor que cero".into());
    }
    let (planta, _) =
        planta_de(e).ok_or("No es un contorno cerrado (polilinea cerrada o circulo)")?;
    Prisma::nuevo(planta, nivel - espesor, espesor)
        .ok_or_else(|| "El contorno no tiene area".into())
}

/// **Lo que dibuja una entidad 2D con altura** (`thickness`), como lo ve
/// AutoCAD: cada tramo levantado como una pared sin tapas; un circulo, como
/// un cilindro cerrado. `None` si no tiene altura o no es de las que la
/// llevan.
pub fn de_altura(e: &EntityType) -> Option<Malla> {
    let (alto, z, normal_z) = match e {
        EntityType::Line(l) => (l.thickness, l.start.z, l.normal.z),
        EntityType::LwPolyline(p) => (p.thickness, p.elevation, p.normal.z),
        EntityType::Circle(c) => (c.thickness, c.center.z, c.normal.z),
        EntityType::Arc(a) => (a.thickness, a.center.z, a.normal.z),
        _ => return None,
    };
    if alto.abs() < 1e-12 || !alto.is_finite() {
        return None;
    }
    // La normal hacia abajo: la altura va hacia abajo.
    let alto = if normal_z < 0.0 { -alto } else { alto };
    if let EntityType::Circle(_) = e {
        let (planta, _) = planta_de(e)?;
        return Prisma::nuevo(planta, z, alto).map(|p| p.malla());
    }
    let (z0, z1) = (z, z + alto);
    let mut m = Malla::default();
    let tramo = |m: &mut Malla, a: P, b: P| {
        let base = m.puntos.len() as u32;
        m.puntos.extend([
            [a[0], a[1], z0],
            [b[0], b[1], z0],
            [b[0], b[1], z1],
            [a[0], a[1], z1],
        ]);
        m.caras.push(vec![base, base + 1, base + 2, base + 3]);
    };
    if let EntityType::Line(l) = e {
        let (a, b) = (formas::p2(&l.start), formas::p2(&l.end));
        if geo::dist(a, b) > 1e-12 {
            tramo(&mut m, a, b);
        }
    } else {
        for f in formas::formas(e) {
            let pts = match f {
                Forma::Segmento { a, b } => vec![a, b],
                otra => otra.muestras(24),
            };
            for w in pts.windows(2) {
                tramo(&mut m, w[0], w[1]);
            }
        }
    }
    (!m.caras.is_empty()).then_some(m)
}

// --------------------------------------------------------------- MUROS

/// **Un tramo recto de muro**: su planta (cuadrilatero convexo antihorario),
/// su eje (un punto y la direccion) y su espesor.
#[derive(Debug, Clone, PartialEq)]
pub struct TramoMuro {
    pub planta: Vec<P>,
    pub origen: P,
    pub u: P,
    pub espesor: f64,
}

impl TramoMuro {
    /// Lo que avanza `p` a lo largo del eje.
    pub fn s(&self, p: P) -> f64 {
        geo::punto(geo::restar(p, self.origen), self.u)
    }

    /// Lo que se separa `p` del eje.
    pub fn d(&self, p: P) -> f64 {
        geo::cruz(self.u, geo::restar(p, self.origen)).abs()
    }

    /// Desde donde hasta donde llega la planta a lo largo del eje.
    pub fn rango(&self) -> (f64, f64) {
        self.planta
            .iter()
            .map(|p| self.s(*p))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), s| {
                (a.min(s), b.max(s))
            })
    }

    /// La planta entre `s0` y `s1` del eje.
    pub fn trozo(&self, s0: f64, s1: f64) -> Vec<P> {
        let k = geo::punto(self.origen, self.u);
        let p = g::recortar(&self.planta, self.u, s1 + k);
        g::recortar(&p, geo::por(self.u, -1.0), -(s0 + k))
    }

    fn nuevo(cara1: (P, P), cara2: (P, P)) -> Option<TramoMuro> {
        let u = geo::unitario(geo::restar(cara1.1, cara1.0));
        if geo::norma(u) < 0.5 {
            return None;
        }
        // Las dos caras en el mismo sentido.
        let (a2, b2) = if geo::punto(geo::restar(cara2.1, cara2.0), u) < 0.0 {
            (cara2.1, cara2.0)
        } else {
            cara2
        };
        let n = geo::perp(u);
        let sep = geo::punto(geo::restar(a2, cara1.0), n);
        let espesor = sep.abs();
        if espesor < 1e-9 {
            return None;
        }
        let origen = geo::sumar(cara1.0, geo::por(n, sep / 2.0));
        let planta = g::antihorario(vec![cara1.0, cara1.1, b2, a2]);
        if !g::es_convexo(&planta) || geo::area_poligono(&planta) < 1e-12 {
            return None;
        }
        Some(TramoMuro {
            planta,
            origen,
            u,
            espesor,
        })
    }
}

/// Los tramos rectos de una entidad (lineas y polilineas; los arcos no).
fn segmentos(e: &EntityType) -> Vec<(P, P)> {
    formas::formas(e)
        .into_iter()
        .filter_map(|f| match f {
            Forma::Segmento { a, b } if geo::dist(a, b) > 1e-9 => Some((a, b)),
            _ => None,
        })
        .collect()
}

/// Los vertices de una polilinea ligera sin abombados (`None` si tiene).
fn vertices_rectos(e: &EntityType) -> Option<(Vec<P>, bool)> {
    match e {
        EntityType::LwPolyline(p)
            if p.vertices.len() >= 2
                && p.normal.z > 0.0
                && p.vertices.iter().all(|v| v.bulge.abs() < 1e-9) =>
        {
            let mut v: Vec<P> = p
                .vertices
                .iter()
                .map(|v| [v.location.x, v.location.y])
                .collect();
            let cerrada =
                p.is_closed || (v.len() >= 4 && geo::dist(v[0], *v.last().unwrap()) < 1e-9);
            if cerrada && geo::dist(v[0], *v.last().unwrap()) < 1e-9 {
                v.pop();
            }
            Some((v, cerrada))
        }
        _ => None,
    }
}

fn paralelos(a: (P, P), b: (P, P)) -> bool {
    let (ua, ub) = (
        geo::unitario(geo::restar(a.1, a.0)),
        geo::unitario(geo::restar(b.1, b.0)),
    );
    geo::cruz(ua, ub).abs() < 1e-3
}

/// **Los tramos de muro de unas entidades de la capa de muros**: primero
/// las parejas de polilineas que dibuja MURO (las dos caras, vertice a
/// vertice, con sus ingletes exactos) y luego, con lo que sobre, las
/// parejas de rayas paralelas a menos de `espesor_max` que se solapan.
pub fn tramos_por_caras(entidades: &[EntityType], espesor_max: f64) -> Vec<TramoMuro> {
    let mut tramos = Vec::new();
    let polis: Vec<Option<(Vec<P>, bool)>> = entidades.iter().map(vertices_rectos).collect();
    let mut usada = vec![false; entidades.len()];
    for i in 0..entidades.len() {
        if usada[i] {
            continue;
        }
        let Some((pi, ci)) = &polis[i] else { continue };
        for j in i + 1..entidades.len() {
            let Some((pj, cj)) = &polis[j] else { continue };
            if usada[j] || ci != cj || pi.len() != pj.len() {
                continue;
            }
            let n = pi.len();
            let lados = if *ci { n } else { n - 1 };
            // Vertice a vertice, cada lado paralelo a su pareja y a la
            // misma distancia.
            let mut v = Vec::new();
            let mut espesor = None;
            for k in 0..lados {
                let (a, b) = ((pi[k], pi[(k + 1) % n]), (pj[k], pj[(k + 1) % n]));
                let Some(t) = paralelos(a, b).then(|| TramoMuro::nuevo(a, b)).flatten() else {
                    break;
                };
                if t.espesor > espesor_max
                    || espesor.is_some_and(|e: f64| (e - t.espesor).abs() > 1e-6 * e.max(1.0))
                {
                    break;
                }
                espesor = Some(t.espesor);
                v.push(t);
            }
            if v.len() == lados {
                usada[i] = true;
                usada[j] = true;
                tramos.extend(v);
                break;
            }
        }
    }
    // Lo que sobra, raya a raya.
    let sueltos: Vec<(P, P)> = entidades
        .iter()
        .zip(&usada)
        .filter(|(_, u)| !**u)
        .flat_map(|(e, _)| segmentos(e))
        .collect();
    let mut hechos = std::collections::HashSet::new();
    for (i, a) in sueltos.iter().enumerate() {
        let ua = geo::unitario(geo::restar(a.1, a.0));
        let n = geo::perp(ua);
        // Las parejas posibles: paralelas, enfrente y que se solapen al
        // menos su separacion (asi las tapas no hacen pareja entre si).
        let mut candidatas: Vec<(usize, f64)> = Vec::new();
        for (j, b) in sueltos.iter().enumerate() {
            if i == j || !paralelos(*a, *b) {
                continue;
            }
            let d = geo::punto(geo::restar(b.0, a.0), n).abs();
            if d < 1e-9 || d > espesor_max {
                continue;
            }
            let s = |p: P| geo::punto(geo::restar(p, a.0), ua);
            let (b0, b1) = (s(b.0).min(s(b.1)), s(b.0).max(s(b.1)));
            let solape = b1.min(geo::dist(a.0, a.1)) - b0.max(0.0);
            if solape >= d {
                candidatas.push((j, d));
            }
        }
        let Some(dmin) = candidatas.iter().map(|c| c.1).min_by(f64::total_cmp) else {
            continue;
        };
        for (j, d) in candidatas {
            if (d - dmin).abs() > 1e-6 * dmin.max(1.0) || !hechos.insert((i.min(j), i.max(j))) {
                continue;
            }
            let b = sueltos[j];
            // Si las dos rayas acaban casi a la par, el trapecio de sus
            // extremos (un inglete); si no, el rectangulo de lo que se solapan.
            let s = |p: P| geo::punto(geo::restar(p, a.0), ua);
            let (b0, b1) = if s(b.0) <= s(b.1) {
                (b.0, b.1)
            } else {
                (b.1, b.0)
            };
            let largo = geo::dist(a.0, a.1);
            let parejas = (s(b0)).abs() <= d * 1.5 && (s(b1) - largo).abs() <= d * 1.5;
            let t = if parejas {
                TramoMuro::nuevo(*a, (b0, b1))
            } else {
                let (lo, hi) = (s(b0).max(0.0), s(b1).min(largo));
                let en = |p: P, k: f64| geo::sumar(p, geo::por(ua, k));
                let pie = |p: P| geo::sumar(a.0, geo::por(n, geo::punto(geo::restar(p, a.0), n)));
                TramoMuro::nuevo(
                    (en(a.0, lo), en(a.0, hi)),
                    (en(pie(b0), lo), en(pie(b0), hi)),
                )
            };
            tramos.extend(t);
        }
    }
    tramos
}

/// **Los tramos de un muro por su eje** (una polilinea o una linea) con su
/// `espesor`: a medio espesor de cada lado, con ingletes en las esquinas.
pub fn tramos_por_eje(e: &EntityType, espesor: f64) -> Vec<TramoMuro> {
    let (pts, cerrado) = match vertices_rectos(e) {
        Some(v) => v,
        None => match e {
            EntityType::Line(l) => (vec![formas::p2(&l.start), formas::p2(&l.end)], false),
            _ => return Vec::new(),
        },
    };
    let mitad = espesor.abs() / 2.0;
    let (Some(izq), Some(der)) = (
        crate::especialidad::desplazar_polilinea(&pts, mitad, cerrado),
        crate::especialidad::desplazar_polilinea(&pts, -mitad, cerrado),
    ) else {
        return Vec::new();
    };
    let n = pts.len();
    let lados = if cerrado { n } else { n - 1 };
    (0..lados)
        .filter_map(|k| {
            let j = (k + 1) % n;
            TramoMuro::nuevo((izq[k], izq[j]), (der[k], der[j]))
        })
        .collect()
}

/// **Un vano** en un tramo de muro: de `s0` a `s1` a lo largo del eje y de
/// `z0` a `z1` de alto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vano {
    pub s0: f64,
    pub s1: f64,
    pub z0: f64,
    pub z1: f64,
    pub puerta: bool,
}

/// Las alturas de los vanos, en unidades del dibujo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Alturas {
    /// Lo alto de una puerta (2,10 m).
    pub puerta: f64,
    /// El alfeizar de una ventana (0,90 m) y su dintel (2,10 m).
    pub alfeizar: f64,
    pub dintel: f64,
}

impl Alturas {
    pub fn en(unidades_por_metro: f64) -> Alturas {
        Alturas {
            puerta: 2.10 * unidades_por_metro,
            alfeizar: 0.90 * unidades_por_metro,
            dintel: 2.10 * unidades_por_metro,
        }
    }
}

/// **Los vanos que abren en un tramo** las puertas (el arco de su giro: la
/// bisagra en el muro y la hoja cerrada a lo largo de el) y las ventanas
/// (su marco, un rectangulo cerrado sobre el muro) dibujadas en planta.
pub fn vanos(
    t: &TramoMuro,
    puertas: &[EntityType],
    ventanas: &[EntityType],
    base: f64,
    al: Alturas,
) -> Vec<Vano> {
    let (smin, smax) = t.rango();
    let holgura = t.espesor * 0.6 + 1e-9;
    let mut v = Vec::new();
    for e in puertas {
        let EntityType::Arc(a) = e else { continue };
        let c = formas::p2(&a.center);
        if t.d(c) > holgura || a.radius <= 1e-9 {
            continue;
        }
        // La hoja cerrada: el extremo del arco que va a lo largo del muro.
        let Some(dir) = [a.start_angle, a.end_angle]
            .into_iter()
            .map(|ang| [ang.cos(), ang.sin()])
            .find(|d| geo::cruz(*d, t.u).abs() < 0.05)
        else {
            continue;
        };
        let (s0, s1) = (t.s(c), t.s(geo::sumar(c, geo::por(dir, a.radius))));
        let (s0, s1) = (s0.min(s1).max(smin), s0.max(s1).min(smax));
        if s1 - s0 > 1e-9 {
            v.push(Vano {
                s0,
                s1,
                z0: base,
                z1: base + al.puerta,
                puerta: true,
            });
        }
    }
    for e in ventanas {
        let Some((pts, _)) = vertices_rectos(e) else {
            continue;
        };
        if pts.len() < 3 {
            continue;
        }
        let centro = geo::por(
            pts.iter().fold([0.0, 0.0], |s, p| geo::sumar(s, *p)),
            1.0 / pts.len() as f64,
        );
        if t.d(centro) > holgura {
            continue;
        }
        let ss: Vec<f64> = pts.iter().map(|p| t.s(*p)).collect();
        let (s0, s1) = (
            ss.iter().copied().fold(f64::INFINITY, f64::min).max(smin),
            ss.iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max)
                .min(smax),
        );
        // A lo largo del muro (no una ventana del muro de al lado): su lado
        // mas largo, paralelo al eje.
        let n = pts.len();
        let largo = (0..n)
            .map(|k| (pts[k], pts[(k + 1) % n]))
            .max_by(|a, b| geo::dist(a.0, a.1).total_cmp(&geo::dist(b.0, b.1)));
        let a_lo_largo = largo
            .is_some_and(|(a, b)| geo::cruz(geo::unitario(geo::restar(b, a)), t.u).abs() < 0.05);
        if s1 - s0 > 1e-9 && a_lo_largo {
            v.push(Vano {
                s0,
                s1,
                z0: base + al.alfeizar,
                z1: base + al.dintel,
                puerta: false,
            });
        }
    }
    v.sort_by(|a, b| a.s0.total_cmp(&b.s0));
    v
}

/// **Los prismas de un tramo de muro** de `base` a `base + alto`, sin lo de
/// sus vanos (en cada vano quedan el antepecho y el dintel).
pub fn muro_3d(t: &TramoMuro, base: f64, alto: f64, vanos: &[Vano]) -> Vec<Prisma> {
    let (smin, smax) = t.rango();
    let tope = base + alto;
    // Los cortes a lo largo del eje.
    let mut cortes = vec![smin, smax];
    for v in vanos {
        cortes.extend([v.s0.clamp(smin, smax), v.s1.clamp(smin, smax)]);
    }
    cortes.sort_by(f64::total_cmp);
    cortes.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let mut v = Vec::new();
    for w in cortes.windows(2) {
        let (a, b) = (w[0], w[1]);
        let medio = (a + b) / 2.0;
        // Lo macizo a esta altura: lo que no tapan los vanos de aqui.
        let mut libres = vec![(base, tope)];
        for h in vanos.iter().filter(|h| h.s0 <= medio && medio <= h.s1) {
            libres = libres
                .into_iter()
                .flat_map(|(z0, z1)| {
                    let mut r = Vec::new();
                    if h.z0 > z0 {
                        r.push((z0, h.z0.min(z1)));
                    }
                    if h.z1 < z1 {
                        r.push((h.z1.max(z0), z1));
                    }
                    r
                })
                .filter(|(z0, z1)| z1 - z0 > 1e-9)
                .collect();
        }
        let planta = if a <= smin + 1e-9 && b >= smax - 1e-9 {
            t.planta.clone()
        } else {
            t.trozo(a, b)
        };
        for (z0, z1) in libres {
            v.extend(Prisma::nuevo(planta.clone(), z0, z1 - z0));
        }
    }
    v
}

/// **La hoja de una puerta o el vidrio de una ventana** en su vano: una
/// placa de `grueso` en el eje del muro.
pub fn hoja(t: &TramoMuro, v: &Vano, grueso: f64) -> Option<Prisma> {
    let n = geo::perp(t.u);
    let e = grueso.min(t.espesor) / 2.0;
    let en = |s: f64, k: f64| geo::sumar(t.origen, geo::sumar(geo::por(t.u, s), geo::por(n, k)));
    Prisma::nuevo(
        vec![en(v.s0, -e), en(v.s1, -e), en(v.s1, e), en(v.s0, e)],
        v.z0,
        v.z1 - v.z0,
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::especialidad as esp;

    #[test]
    fn extruir_un_rectangulo_y_un_circulo() {
        let r = esp::polilinea(&[[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]], true);
        let p = extruir(&r, 2.0).unwrap();
        assert!((p.volumen() - 24.0).abs() < 1e-9);
        let c = esp::circulo([0.0, 0.0], 1.0);
        let p = extruir(&c, -1.0).unwrap();
        assert!((p.z0 + 1.0).abs() < 1e-12);
        assert!((p.volumen() - std::f64::consts::PI).abs() < 0.02);
    }

    #[test]
    fn extruir_rechaza_lo_abierto_y_la_altura_cero() {
        let abierta = esp::polilinea(&[[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]], false);
        assert!(extruir(&abierta, 2.0).unwrap_err().contains("cerrado"));
        let r = esp::polilinea(&[[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]], true);
        assert!(extruir(&r, 0.0).unwrap_err().contains("cero"));
        assert!(extruir(&esp::linea([0.0, 0.0], [1.0, 1.0]), 1.0).is_err());
        assert!(losa(&r, 0.0, 0.0).is_err());
    }

    #[test]
    fn la_losa_cuelga_de_su_nivel() {
        let r = esp::polilinea(&[[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]], true);
        let p = losa(&r, 0.2, 3.0).unwrap();
        assert!((p.z0 - 2.8).abs() < 1e-12 && (p.alto - 0.2).abs() < 1e-12);
    }

    #[test]
    fn la_altura_de_una_linea_y_un_circulo() {
        let mut l = opencadcodec::Line::from_points(
            opencadcodec::Vector3::new(0.0, 0.0, 1.0),
            opencadcodec::Vector3::new(3.0, 0.0, 1.0),
        );
        assert!(de_altura(&EntityType::Line(l.clone())).is_none());
        l.thickness = 2.0;
        let m = de_altura(&EntityType::Line(l)).unwrap();
        assert_eq!(m.caras.len(), 1);
        assert_eq!(m.caja().unwrap()[5], 3.0);
        let EntityType::Circle(mut c) = esp::circulo([0.0, 0.0], 1.0) else {
            panic!()
        };
        c.thickness = 1.0;
        let m = de_altura(&EntityType::Circle(c)).unwrap();
        assert_eq!(m.prismas().unwrap().len(), 1);
    }

    #[test]
    fn un_muro_de_muro_con_puerta_y_ventana() {
        // MURO de 0,0 a 5000,0 de 150: dos caras y dos tapas.
        let caras = esp::muro(&[[0.0, 0.0], [5000.0, 0.0]], 150.0, false);
        let t = tramos_por_caras(&caras, 600.0);
        assert_eq!(t.len(), 1, "{t:?}");
        assert!((t[0].espesor - 150.0).abs() < 1e-9);
        assert_eq!(t[0].rango(), (0.0, 5000.0));
        let puerta = esp::puerta([1000.0, 0.0], [2000.0, 0.0], 900.0, true);
        let ventana = esp::ventana([3000.0, 0.0], [4200.0, 0.0], 150.0);
        let v = vanos(&t[0], &puerta, &ventana, 0.0, Alturas::en(1000.0));
        assert_eq!(v.len(), 2);
        assert!(v[0].puerta && (v[0].s0 - 1000.0).abs() < 1e-6 && (v[0].s1 - 1900.0).abs() < 1e-6);
        assert!(!v[1].puerta && (v[1].z0 - 900.0).abs() < 1e-9);
        let p = muro_3d(&t[0], 0.0, 2600.0, &v);
        // Tres macizos enteros, el dintel de la puerta y el antepecho y
        // dintel de la ventana.
        assert_eq!(p.len(), 6, "{p:?}");
        let vol: f64 = p.iter().map(Prisma::volumen).sum();
        let lleno = 5000.0 * 150.0 * 2600.0;
        let huecos = 900.0 * 150.0 * 2100.0 + 1200.0 * 150.0 * 1200.0;
        assert!((vol - (lleno - huecos)).abs() < 1e-3, "{vol}");
        assert!(hoja(&t[0], &v[0], 40.0).is_some());
    }

    #[test]
    fn un_muro_en_l_por_su_eje_cierra_la_esquina() {
        let eje = esp::polilinea(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], false);
        let t = tramos_por_eje(&eje, 2.0);
        assert_eq!(t.len(), 2);
        let area: f64 = t.iter().map(|t| geo::area_poligono(&t.planta)).sum();
        // El eje mide 20; con el inglete, el area es justo largo x espesor.
        assert!((area - 40.0).abs() < 1e-9, "{area}");
        // Y las dos caras dibujadas con MURO dan lo mismo.
        let caras = esp::muro(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], 2.0, false);
        let t2 = tramos_por_caras(&caras, 5.0);
        let area2: f64 = t2.iter().map(|t| geo::area_poligono(&t.planta)).sum();
        assert!((area2 - 40.0).abs() < 1e-9, "{area2}");
    }

    #[test]
    fn dos_lineas_paralelas_sueltas_hacen_muro() {
        let v = vec![
            esp::linea([0.0, 0.0], [3000.0, 0.0]),
            esp::linea([3000.0, 200.0], [0.0, 200.0]),
            // Lejos: no es pareja.
            esp::linea([0.0, 5000.0], [3000.0, 5000.0]),
        ];
        let t = tramos_por_caras(&v, 600.0);
        assert_eq!(t.len(), 1);
        assert!((t[0].espesor - 200.0).abs() < 1e-9);
    }
}
