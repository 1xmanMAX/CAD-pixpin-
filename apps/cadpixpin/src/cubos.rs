//! **El plano en cubos**: para que editar un plano enorme (cientos de miles
//! de entidades en un Core i3) no obligue a convertirlo entero, se parte en
//! trozos por sitio y cada uno se convierte y se sube a la tarjeta por su
//! cuenta. Al cambiar algo solo se rehacen los cubos donde estaba y donde
//! esta ahora.
//!
//! Aqui va lo que no depende de Windows: como se parte (un arbol que corta
//! por la mediana, para que todos los cubos lleven mas o menos lo mismo), a
//! que cubo va cada entidad y que cubos toca un cambio. La conversion y la
//! tarjeta estan en `plano`.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use cad_nucleo::geo::P;
use cad_nucleo::opencadcodec::{CadDocument, EntityType, Handle};

/// Cuantas entidades lleva un cubo como mucho al partir: rehacer uno tarda
/// unos pocos milisegundos, y un plano de 500 000 da unos 128.
pub const CAPACIDAD: usize = 4_000;

/// Si un cubo crece (se dibuja mucho en un sitio) hasta tantas veces la
/// [`CAPACIDAD`], se vuelve a partir todo.
pub const CRECER: usize = 4;

/// Hasta donde baja el arbol (2^16 cubos ya es de sobra).
const PROFUNDIDAD: u32 = 16;

/// El indice que corta una tira de lineas (el `CORTE` de `pixpin-cad`).
pub const CORTE: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Nodo {
    Hoja(usize),
    /// Lo que tiene la coordenada `eje` menor que `valor` va a `menor`.
    Corte {
        eje: usize,
        valor: f64,
        menor: usize,
        mayor: usize,
    },
}

/// **Como se parte el plano**: un arbol de cortes por la mediana de los
/// centros de las entidades. Cada hoja es un cubo.
#[derive(Debug, Clone, PartialEq)]
pub struct Particion {
    nodos: Vec<Nodo>,
    hojas: usize,
}

impl Particion {
    /// La particion de estos centros, con `capacidad` como mucho por cubo
    /// (salvo muchos en el mismo punto, que no se pueden separar).
    pub fn de(centros: &[P], capacidad: usize) -> Particion {
        let mut pts: Vec<P> = centros.iter().map(|p| limpio(*p)).collect();
        let mut part = Particion {
            nodos: Vec::new(),
            hojas: 0,
        };
        part.partir(&mut pts, capacidad.max(1), 0);
        part
    }

    fn partir(&mut self, pts: &mut [P], capacidad: usize, profundidad: u32) -> usize {
        let id = self.nodos.len();
        self.nodos.push(Nodo::Hoja(0));
        let corte = if pts.len() > capacidad && profundidad < PROFUNDIDAD {
            cortar(pts)
        } else {
            None
        };
        let Some((eje, valor, i)) = corte else {
            self.nodos[id] = Nodo::Hoja(self.hojas);
            self.hojas += 1;
            return id;
        };
        let (a, b) = pts.split_at_mut(i);
        let menor = self.partir(a, capacidad, profundidad + 1);
        let mayor = self.partir(b, capacidad, profundidad + 1);
        self.nodos[id] = Nodo::Corte {
            eje,
            valor,
            menor,
            mayor,
        };
        id
    }

    /// El cubo de un punto.
    pub fn cubo(&self, p: P) -> usize {
        let p = limpio(p);
        let mut n = 0;
        loop {
            match self.nodos.get(n) {
                Some(Nodo::Corte {
                    eje,
                    valor,
                    menor,
                    mayor,
                }) => n = if p[*eje] < *valor { *menor } else { *mayor },
                Some(Nodo::Hoja(k)) => return *k,
                None => return 0,
            }
        }
    }

    /// Cuantos cubos hay.
    pub fn cuantos(&self) -> usize {
        self.hojas.max(1)
    }
}

/// Por donde cortar: el eje mas largo por la mediana. Deja a la izquierda
/// los menores que el corte y dice cuantos son (`None`: no se puede cortar,
/// todos en el mismo punto).
fn cortar(pts: &mut [P]) -> Option<(usize, f64, usize)> {
    let mut caja = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in pts.iter() {
        caja = [
            caja[0].min(p[0]),
            caja[1].min(p[1]),
            caja[2].max(p[0]),
            caja[3].max(p[1]),
        ];
    }
    let eje = if caja[2] - caja[0] >= caja[3] - caja[1] {
        0
    } else {
        1
    };
    if caja[eje + 2] <= caja[eje] {
        return None;
    }
    let k = pts.len() / 2;
    pts.select_nth_unstable_by(k, |a, b| a[eje].total_cmp(&b[eje]));
    let mut valor = pts[k][eje];
    let mut i = separar(pts, eje, valor);
    if i == 0 {
        // La mediana es el minimo (muchos iguales): se corta justo despues.
        valor = pts
            .iter()
            .map(|p| p[eje])
            .filter(|x| *x > valor)
            .fold(f64::MAX, f64::min);
        i = separar(pts, eje, valor);
    }
    (i > 0 && i < pts.len()).then_some((eje, valor, i))
}

/// Pone delante los de coordenada menor que `valor`; dice cuantos son.
fn separar(pts: &mut [P], eje: usize, valor: f64) -> usize {
    let mut i = 0;
    for j in 0..pts.len() {
        if pts[j][eje] < valor {
            pts.swap(i, j);
            i += 1;
        }
    }
    i
}

/// Sin NaN ni infinitos (una entidad rota va al punto 0,0).
fn limpio(p: P) -> P {
    let f = |x: f64| if x.is_finite() { x } else { 0.0 };
    [f(p[0]), f(p[1])]
}

/// **El punto que decide el cubo de una entidad**: el centro de su caja
/// aproximada (la de un bloque es su punto de insercion).
pub fn centro(e: &EntityType) -> P {
    let c = e.as_entity().bounding_box();
    limpio([(c.min.x + c.max.x) / 2.0, (c.min.y + c.max.y) / 2.0])
}

/// Un cambio que llega para dibujar: la entidad como queda (`None`: quitada).
pub type Cambio = (Handle, Option<Arc<EntityType>>);

/// **Junta dos tandas de cambios seguidas** en una: de cada entidad vale
/// como quedo al final.
pub fn juntar_cambios(cambios: &mut Vec<Cambio>, nuevos: Vec<Cambio>) {
    let mut donde: HashMap<Handle, usize> = cambios
        .iter()
        .enumerate()
        .map(|(i, (h, _))| (*h, i))
        .collect();
    for (h, e) in nuevos {
        match donde.get(&h) {
            Some(&i) => cambios[i].1 = e,
            None => {
                donde.insert(h, cambios.len());
                cambios.push((h, e));
            }
        }
    }
}

/// **Las entidades de cada cubo** y en que cubo esta cada una.
pub struct Reparto {
    particion: Particion,
    pub cubos: Vec<Vec<Arc<EntityType>>>,
    /// En que cubo esta cada entidad.
    de: HashMap<Handle, usize>,
    /// Las que se quitaron (deshacer las puede devolver).
    quitadas: HashSet<Handle>,
}

impl Reparto {
    /// Parte estas entidades en cubos de `capacidad` como mucho.
    pub fn nuevo(entidades: Vec<Arc<EntityType>>, capacidad: usize) -> Reparto {
        // Sin repetir (un handle que salga dos veces se dibuja una).
        let mut vistos = HashSet::with_capacity(entidades.len());
        let entidades: Vec<Arc<EntityType>> = entidades
            .into_iter()
            .filter(|e| vistos.insert(e.common().handle))
            .collect();
        let centros: Vec<P> = entidades.iter().map(|e| centro(e)).collect();
        let particion = Particion::de(&centros, capacidad);
        let mut cubos = vec![Vec::new(); particion.cuantos()];
        let mut de = HashMap::with_capacity(entidades.len());
        for (e, c) in entidades.into_iter().zip(centros) {
            let k = particion.cubo(c);
            de.insert(e.common().handle, k);
            cubos[k].push(e);
        }
        Reparto {
            particion,
            cubos,
            de,
            quitadas: HashSet::new(),
        }
    }

    /// Si la entidad `h` es (o fue) de las que se dibujan.
    pub fn conoce(&self, h: Handle) -> bool {
        self.de.contains_key(&h) || self.quitadas.contains(&h)
    }

    /// **Aplica los cambios** y dice que cubos hay que rehacer: donde estaba
    /// cada entidad y donde esta ahora.
    pub fn aplicar(&mut self, cambios: Vec<Cambio>) -> BTreeSet<usize> {
        let mut tocados = BTreeSet::new();
        let mut quitar: HashMap<usize, HashSet<Handle>> = HashMap::new();
        for (h, _) in &cambios {
            if let Some(&k) = self.de.get(h) {
                quitar.entry(k).or_default().insert(*h);
                tocados.insert(k);
            }
        }
        // Una pasada por cubo, no una por entidad (Ctrl+A y mover 300 000).
        for (k, hs) in quitar {
            if let Some(c) = self.cubos.get_mut(k) {
                c.retain(|e| !hs.contains(&e.common().handle));
            }
        }
        for (h, e) in cambios {
            let Some(e) = e else {
                if self.de.remove(&h).is_some() {
                    self.quitadas.insert(h);
                }
                continue;
            };
            self.quitadas.remove(&h);
            let k = self.particion.cubo(centro(&e)).min(self.cubos.len() - 1);
            self.cubos[k].push(e);
            self.de.insert(h, k);
            tocados.insert(k);
        }
        tocados
    }

    /// Lo que lleva el cubo mas lleno.
    pub fn mayor(&self) -> usize {
        self.cubos.iter().map(Vec::len).max().unwrap_or(0)
    }

    /// Todas las entidades (para volver a partir).
    pub fn todas(self) -> Vec<Arc<EntityType>> {
        self.cubos.into_iter().flatten().collect()
    }
}

/// **Si un cambio se puede dibujar solo con su cubo**, convirtiendolo con
/// los bloques y las capas de `base` (el documento del ultimo dibujo
/// entero). No, si no es una entidad del espacio modelo (la de un bloque
/// cambia todos sus INSERT) o si usa un bloque que `base` aun no tiene.
pub fn admite(base: &CadDocument, reparto: &Reparto, h: Handle, e: Option<&EntityType>) -> bool {
    let Some(e) = e else {
        // Quitada: solo si era de las que se dibujan.
        return reparto.conoce(h);
    };
    if !reparto.conoce(h) {
        let duena = e.common().owner_handle;
        let modelo = [
            Some(base.header.model_space_block_handle),
            base.block_records.get("*Model_Space").map(|b| b.handle),
        ];
        if duena.is_null() || !modelo.contains(&Some(duena)) {
            return false;
        }
    }
    let bloque = match e {
        EntityType::Insert(i) => Some(i.block_name.as_str()),
        EntityType::Dimension(d) => Some(d.base().block_name.as_str()),
        _ => None,
    };
    bloque.is_none_or(|b| b.is_empty() || base.block_records.get(b).is_some())
}

/// **Los vertices que usan unos indices**, renumerados (los [`CORTE`] se
/// quedan): para partir un modelo en lo de abajo y lo de encima sin
/// duplicar todos sus vertices.
pub fn compactar<T: Copy>(vertices: &[T], indices: &[u32]) -> (Vec<T>, Vec<u32>) {
    let mut nuevo = vec![CORTE; vertices.len()];
    let mut vs = Vec::new();
    let mut is = Vec::with_capacity(indices.len());
    for &i in indices {
        if i == CORTE {
            is.push(CORTE);
            continue;
        }
        // Uno roto queda como un corte: los tramos cuentan posiciones.
        let Some(v) = vertices.get(i as usize) else {
            is.push(CORTE);
            continue;
        };
        if nuevo[i as usize] == CORTE {
            nuevo[i as usize] = vs.len() as u32;
            vs.push(*v);
        }
        is.push(nuevo[i as usize]);
    }
    (vs, is)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use cad_nucleo::opencadcodec::Vector3;
    use cad_nucleo::opencadcodec::entities::Line;

    fn linea(h: u64, x: f64, y: f64) -> Arc<EntityType> {
        let mut l = Line::from_points(Vector3::new(x, y, 0.0), Vector3::new(x + 1.0, y, 0.0));
        l.common.handle = Handle::new(h);
        Arc::new(EntityType::Line(l))
    }

    #[test]
    fn la_particion_reparte_por_igual_y_cada_punto_cae_en_su_cubo() {
        let centros: Vec<P> = (0..10_000)
            .map(|i| [(i % 100) as f64, (i / 100) as f64 * 3.0])
            .collect();
        let p = Particion::de(&centros, 1000);
        assert!(p.cuantos() >= 10 && p.cuantos() <= 16, "{}", p.cuantos());
        let mut cuenta = vec![0usize; p.cuantos()];
        for c in &centros {
            cuenta[p.cubo(*c)] += 1;
        }
        assert!(cuenta.iter().all(|n| *n <= 1000 && *n > 0), "{cuenta:?}");
        // Cerca, el mismo cubo; lejos, otro.
        assert_eq!(p.cubo([0.1, 0.1]), p.cubo([0.2, 0.2]));
        assert_ne!(p.cubo([0.0, 0.0]), p.cubo([99.0, 297.0]));
    }

    #[test]
    fn todos_en_el_mismo_punto_no_se_parten_sin_fin() {
        let p = Particion::de(&vec![[5.0, 5.0]; 50_000], 100);
        assert_eq!(p.cuantos(), 1);
        // Muchos iguales y unos pocos distintos: se separan.
        let mut c = vec![[0.0, 0.0]; 900];
        c.extend((0..100).map(|i| [10.0 + i as f64, 0.0]));
        let p = Particion::de(&c, 500);
        assert!(p.cuantos() >= 2);
        assert_ne!(p.cubo([0.0, 0.0]), p.cubo([50.0, 0.0]));
        // Un NaN no rompe nada.
        let p = Particion::de(&[[f64::NAN, 1.0], [2.0, f64::INFINITY]], 1);
        assert!(p.cubo([f64::NAN, f64::NAN]) < p.cuantos());
    }

    #[test]
    fn un_cambio_toca_su_cubo_y_si_se_mueve_tambien_el_nuevo() {
        let ents: Vec<_> = (0..400)
            .map(|i| linea(i + 1, (i % 20) as f64 * 10.0, (i / 20) as f64 * 10.0))
            .collect();
        let mut r = Reparto::nuevo(ents, 50);
        let n = r.cubos.len();
        assert!(n >= 8);
        assert_eq!(r.cubos.iter().map(Vec::len).sum::<usize>(), 400);
        let k1 = r.particion.cubo(centro(&linea(1, 0.0, 0.0)));
        // Cambiar sin moverse: solo su cubo.
        let t = r.aplicar(vec![(Handle::new(1), Some(linea(1, 0.0, 0.5)))]);
        assert_eq!(t, BTreeSet::from([k1]));
        // Llevarla a la otra punta: el viejo y el nuevo.
        let lejos = linea(1, 190.0, 190.0);
        let k2 = r.particion.cubo(centro(&lejos));
        assert_ne!(k1, k2);
        let t = r.aplicar(vec![(Handle::new(1), Some(lejos))]);
        assert_eq!(t, BTreeSet::from([k1, k2]));
        assert_eq!(
            r.cubos[k2]
                .iter()
                .filter(|e| e.common().handle == Handle::new(1))
                .count(),
            1
        );
        assert!(
            r.cubos[k1]
                .iter()
                .all(|e| e.common().handle != Handle::new(1))
        );
        // Quitarla: su cubo; deshacer la devuelve a donde toque.
        let t = r.aplicar(vec![(Handle::new(1), None)]);
        assert_eq!(t, BTreeSet::from([k2]));
        assert_eq!(r.cubos.iter().map(Vec::len).sum::<usize>(), 399);
        assert!(r.conoce(Handle::new(1)));
        let t = r.aplicar(vec![(Handle::new(1), Some(linea(1, 0.0, 0.0)))]);
        assert_eq!(t, BTreeSet::from([k1]));
        // Una nueva va al cubo de su sitio.
        let t = r.aplicar(vec![(Handle::new(9999), Some(linea(9999, 100.0, 100.0)))]);
        assert_eq!(t.len(), 1);
        assert_eq!(r.cubos.iter().map(Vec::len).sum::<usize>(), 401);
        assert!(r.mayor() <= 52);
    }

    #[test]
    fn solo_se_admite_por_partes_lo_del_espacio_modelo_con_bloques_conocidos() {
        let mut d = cad_nucleo::Dibujo::nuevo();
        let h = d
            .editar("linea", |t| {
                t.anadir(EntityType::Line(Line::from_points(
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                )))
            })
            .unwrap();
        assert_eq!(d.doc.model_space_entities().count(), 1);
        let r = Reparto::nuevo(vec![d.doc.get_entity_arc(h).unwrap()], CAPACIDAD);
        let e = d.doc.get_entity(h).unwrap().clone();
        assert!(admite(&d.doc, &r, h, Some(&e)));
        assert!(admite(&d.doc, &r, h, None));
        // Una que no se conoce y se quita: pudo ser de un bloque.
        assert!(!admite(&d.doc, &r, Handle::new(0xFFFF), None));
        // Nueva en el espacio modelo: si; sin dueno: no.
        let h2 = d
            .editar("otra", |t| {
                t.anadir(EntityType::Line(Line::from_points(
                    Vector3::new(5.0, 0.0, 0.0),
                    Vector3::new(6.0, 0.0, 0.0),
                )))
            })
            .unwrap();
        let e2 = d.doc.get_entity(h2).unwrap().clone();
        assert!(admite(&d.doc, &r, h2, Some(&e2)));
        let mut suelta = e2.clone();
        suelta.common_mut().owner_handle = Handle::NULL;
        suelta.as_entity_mut().set_handle(Handle::new(0xFFF0));
        assert!(!admite(&d.doc, &r, Handle::new(0xFFF0), Some(&suelta)));
        // Un INSERT de un bloque que no esta: no.
        let mut ins =
            cad_nucleo::opencadcodec::entities::Insert::new("NO_ESTA", Vector3::new(0.0, 0.0, 0.0));
        ins.common.handle = h;
        assert!(!admite(&d.doc, &r, h, Some(&EntityType::Insert(ins))));
    }

    #[test]
    fn de_dos_tandas_de_cambios_vale_lo_ultimo() {
        let mut a = vec![
            (Handle::new(1), Some(linea(1, 0.0, 0.0))),
            (Handle::new(2), None),
        ];
        juntar_cambios(
            &mut a,
            vec![
                (Handle::new(1), None),
                (Handle::new(3), Some(linea(3, 1.0, 1.0))),
            ],
        );
        let hs: Vec<(u64, bool)> = a.iter().map(|(h, e)| (h.value(), e.is_some())).collect();
        assert_eq!(hs, vec![(1, false), (2, false), (3, true)]);
    }

    #[test]
    fn compactar_renumera_y_deja_los_cortes() {
        let v = [10, 11, 12, 13, 14];
        let (vs, is) = compactar(&v, &[3, 1, CORTE, 1, 4, 99]);
        assert_eq!(vs, vec![13, 11, 14]);
        assert_eq!(is, vec![0, 1, CORTE, 1, 2, CORTE]);
        let (vs, is) = compactar::<u8>(&[], &[]);
        assert!(vs.is_empty() && is.is_empty());
    }
}
