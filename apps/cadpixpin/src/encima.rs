//! **Lo que se pinta encima del plano sin rehacerlo cada fotograma**: el
//! resalte de lo elegido (con Ctrl+A en un plano de 300 000 entidades no se
//! puede volver a buscar y trazar todo en cada movimiento del raton) y el
//! enganche bajo el cursor, que solo se busca si algo cambio.

use std::collections::HashSet;

use cad_nucleo::Sesion;
use cad_nucleo::dibujo::Indice;
use cad_nucleo::enganche::Modos;
use cad_nucleo::geo::{self, Caja, P};
use cad_nucleo::opencadcodec::Handle;

/// **Lo elegido, listo para resaltar**: la caja de cada cosa elegida y su
/// entrada en el indice, de la mayor a la menor. Se rehace solo si cambia
/// la eleccion o el dibujo.
#[derive(Debug, Default)]
pub struct Resalte {
    version: u64,
    seleccion: Vec<Handle>,
    cajas: Vec<(Caja, u32)>,
    /// La caja de todo lo elegido.
    pub total: Option<Caja>,
}

impl Resalte {
    /// Al dia con la eleccion `sel` del dibujo en la version `version`.
    pub fn al_dia(&mut self, version: u64, sel: &[Handle], ind: &Indice) {
        if version == self.version && sel == self.seleccion.as_slice() {
            return;
        }
        self.version = version;
        self.seleccion.clear();
        self.seleccion.extend_from_slice(sel);
        self.cajas.clear();
        self.total = None;
        if sel.is_empty() {
            return;
        }
        let elegidos: HashSet<Handle> = sel.iter().copied().collect();
        for (k, en) in ind.entradas.iter().enumerate() {
            if elegidos.contains(&en.handle) {
                self.cajas.push((en.caja, k as u32));
            }
        }
        self.cajas.sort_by(|a, b| lado(&b.0).total_cmp(&lado(&a.0)));
        let mut total = geo::CAJA_VACIA;
        for (c, _) in &self.cajas {
            total = geo::unir_cajas(total, *c);
        }
        self.total = geo::caja_valida(&total).then_some(total);
    }

    /// **Lo elegido que se ve** en `vista` (entradas del indice), de lo mayor
    /// a lo menor, sin lo que mide menos de `minimo` y como mucho `tope`.
    /// Dice tambien si quedo algo sin resaltar.
    pub fn visibles(&self, vista: &Caja, minimo: f64, tope: usize) -> (Vec<u32>, bool) {
        let mut v = Vec::new();
        for (i, (c, k)) in self.cajas.iter().enumerate() {
            if lado(c) < minimo || v.len() >= tope {
                // Van de mayor a menor: lo que queda es menor.
                return (v, i < self.cajas.len());
            }
            if geo::se_tocan(c, vista) {
                v.push(*k);
            }
        }
        (v, false)
    }
}

fn lado(c: &Caja) -> f64 {
    (c[2] - c[0]).max(c[3] - c[1])
}

/// **De que depende el enganche bajo el cursor**: si nada de esto cambia,
/// vale el de antes.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaveAjuste {
    cursor: P,
    radio: f64,
    /// Sin enganches (eligiendo objetos dentro de una orden).
    crudo: bool,
    version: u64,
    enganche: bool,
    modos: Modos,
    rejilla: Option<f64>,
    orto: bool,
    polar: Option<f64>,
    base: Option<P>,
}

impl ClaveAjuste {
    pub fn de(s: &Sesion, cursor: P, radio: f64, crudo: bool) -> ClaveAjuste {
        let a = &s.ajustes;
        ClaveAjuste {
            cursor,
            radio,
            crudo,
            version: s.dibujo.version(),
            enganche: a.enganche,
            modos: a.modos,
            rejilla: a.rejilla,
            orto: a.orto,
            polar: a.polar,
            base: s.base(),
        }
    }
}

/// **Lo ultimo calculado** y de que dependia.
#[derive(Debug)]
pub struct Memo<K, V> {
    ultimo: Option<(K, V)>,
}

impl<K, V> Default for Memo<K, V> {
    fn default() -> Self {
        Memo { ultimo: None }
    }
}

impl<K: PartialEq, V: Clone> Memo<K, V> {
    /// Lo de antes si `clave` es la misma; si no, `f()`.
    pub fn o_calcular(&mut self, clave: K, f: impl FnOnce() -> V) -> V {
        if let Some((k, v)) = &self.ultimo
            && *k == clave
        {
            return v.clone();
        }
        let v = f();
        self.ultimo = Some((clave, v.clone()));
        v
    }
}

/// **Lo calculado de lo elegido** (las propiedades del panel), que solo se
/// rehace si cambia la eleccion o el dibujo: con Ctrl+A en un plano grande,
/// sumar longitudes y areas de todo en cada fotograma se comia el pintado.
#[derive(Debug)]
pub struct MemoEleccion<V> {
    version: u64,
    seleccion: Vec<Handle>,
    valor: Option<V>,
}

impl<V> Default for MemoEleccion<V> {
    fn default() -> Self {
        MemoEleccion {
            version: 0,
            seleccion: Vec::new(),
            valor: None,
        }
    }
}

impl<V> MemoEleccion<V> {
    /// Lo de antes si la version y la eleccion son las mismas; si no, `f()`.
    pub fn o_calcular(&mut self, version: u64, sel: &[Handle], f: impl FnOnce() -> V) -> &V {
        let igual = self.valor.is_some() && self.version == version && self.seleccion == sel;
        if !igual {
            self.version = version;
            self.seleccion.clear();
            self.seleccion.extend_from_slice(sel);
            self.valor = Some(f());
        }
        self.valor.as_ref().expect("recien calculado")
    }

    /// Lo ultimo calculado, si hay.
    pub fn valor(&self) -> Option<&V> {
        self.valor.as_ref()
    }
}

/// **Cuantos decimales** tienen sentido en las coordenadas del cursor con
/// `px` unidades del dibujo por pixel: los que distinguen un pixel del
/// siguiente (de 0 a 6), y al menos los `minimo` del dibujo.
pub fn decimales_para(px: f64, minimo: usize) -> usize {
    let d = if px > 0.0 && px.is_finite() {
        (-px.log10()).ceil().clamp(0.0, 6.0) as usize
    } else {
        0
    };
    d.max(minimo).min(6)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use cad_nucleo::Dibujo;
    use cad_nucleo::opencadcodec::Vector3;
    use cad_nucleo::opencadcodec::entities::{EntityType, Line};

    fn dibujo_con_lineas(n: usize) -> (Dibujo, Vec<Handle>) {
        let mut d = Dibujo::nuevo();
        let hs = d.editar("lineas", |t| {
            (0..n)
                .filter_map(|i| {
                    let x = i as f64 * 10.0;
                    // Cada una mas larga que la anterior.
                    t.anadir(EntityType::Line(Line::from_points(
                        Vector3::new(x, 0.0, 0.0),
                        Vector3::new(x, 1.0 + i as f64, 0.0),
                    )))
                })
                .collect()
        });
        (d, hs)
    }

    #[test]
    fn el_resalte_da_lo_que_se_ve_de_mayor_a_menor_y_con_tope() {
        let (d, hs) = dibujo_con_lineas(100);
        let ind = d.indice();
        let mut r = Resalte::default();
        r.al_dia(d.version(), &hs, &ind);
        assert_eq!(r.total, Some([0.0, 0.0, 990.0, 100.0]));
        // Todo a la vista, con tope de 10: las 10 mas largas, y que quedan.
        let (v, quedan) = r.visibles(&[-1.0, -1.0, 1000.0, 200.0], 0.0, 10);
        assert_eq!(v.len(), 10);
        assert!(quedan);
        assert_eq!(ind.entradas[v[0] as usize].handle, hs[99]);
        // Solo las primeras 5 a la vista (x de 0 a 40).
        let (v, quedan) = r.visibles(&[-1.0, -1.0, 45.0, 200.0], 0.0, 1000);
        assert_eq!(v.len(), 5);
        assert!(!quedan);
        // Lo que mide menos de 50 no se resalta.
        let (v, quedan) = r.visibles(&[-1.0, -1.0, 1000.0, 200.0], 50.0, 1000);
        assert_eq!(v.len(), 51);
        assert!(quedan);
        // Sin eleccion, nada.
        r.al_dia(d.version(), &[], &ind);
        assert_eq!(
            r.visibles(&[-1e9, -1e9, 1e9, 1e9], 0.0, 10),
            (vec![], false)
        );
        assert_eq!(r.total, None);
    }

    #[test]
    fn el_memo_solo_calcula_si_cambia_la_clave() {
        let mut m: Memo<u32, u32> = Memo::default();
        let mut veces = 0;
        for k in [1, 1, 1, 2, 2, 1] {
            m.o_calcular(k, || {
                veces += 1;
                k * 10
            });
        }
        assert_eq!(veces, 3);
        assert_eq!(m.o_calcular(1, || 0), 10);
    }

    #[test]
    fn los_decimales_siguen_al_zoom() {
        // Un plano en mm visto entero: 10 mm por pixel, sin decimales.
        assert_eq!(decimales_para(10.0, 0), 0);
        assert_eq!(decimales_para(1.0, 0), 0);
        assert_eq!(decimales_para(0.5, 0), 1);
        assert_eq!(decimales_para(0.004, 0), 3);
        // En metros, al menos los dos del dibujo.
        assert_eq!(decimales_para(10.0, 2), 2);
        assert_eq!(decimales_para(1e-12, 0), 6);
        assert_eq!(decimales_para(f64::NAN, 1), 1);
    }

    #[test]
    fn el_memo_de_la_eleccion_sigue_a_la_version_y_a_lo_elegido() {
        let (d, hs) = dibujo_con_lineas(4);
        let mut m: MemoEleccion<usize> = MemoEleccion::default();
        let mut veces = 0;
        let mut pedir = |v: u64, sel: &[Handle]| {
            *m.o_calcular(v, sel, || {
                veces += 1;
                sel.len()
            })
        };
        assert_eq!(pedir(d.version(), &hs), 4);
        assert_eq!(pedir(d.version(), &hs), 4);
        assert_eq!(pedir(d.version(), &hs[..2]), 2);
        assert_eq!(pedir(d.version() + 1, &hs[..2]), 2);
        // Vacio tambien se calcula (la primera vez).
        assert_eq!(pedir(d.version() + 1, &[]), 0);
        assert_eq!(veces, 4);
    }

    #[test]
    fn la_clave_del_enganche_cambia_con_el_cursor_y_los_ajustes() {
        let (d, _) = dibujo_con_lineas(3);
        let mut s = Sesion::nueva(d);
        let a = ClaveAjuste::de(&s, [1.0, 2.0], 5.0, false);
        assert_eq!(a, ClaveAjuste::de(&s, [1.0, 2.0], 5.0, false));
        assert_ne!(a, ClaveAjuste::de(&s, [1.0, 2.5], 5.0, false));
        s.ajustes.orto = !s.ajustes.orto;
        assert_ne!(a, ClaveAjuste::de(&s, [1.0, 2.0], 5.0, false));
    }
}
