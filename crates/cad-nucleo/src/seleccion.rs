//! **Elegir lo dibujado**: con un clic (lo mas cercano) o con un recuadro,
//! como en AutoCAD: de izquierda a derecha (ventana, azul) solo lo que cabe
//! entero; de derecha a izquierda (captura, verde) todo lo que toque.

use opencadcodec::Handle;

use crate::dibujo::{Entrada, Indice};
use crate::geo::{self, Caja, P};

/// Lo elegible mas cercano a `p`, a menos de `tol`.
pub fn elegir(ind: &Indice, p: P, tol: f64) -> Option<Handle> {
    let zona = geo::ensanchar([p[0], p[1], p[0], p[1]], tol);
    let mut mejor: Option<(f64, Handle)> = None;
    for e in ind.en(&zona) {
        if e.bloqueada {
            continue;
        }
        let d = distancia(e, p);
        if d <= tol && mejor.is_none_or(|(m, _)| d < m) {
            mejor = Some((d, e.handle));
        }
    }
    mejor.map(|(_, h)| h)
}

/// Lo que dista `p` de una entrada: de sus formas o, si no tiene (un texto),
/// cero dentro de su caja.
fn distancia(e: &Entrada, p: P) -> f64 {
    if e.formas.is_empty() {
        if geo::en_caja(&e.caja, p) {
            return 0.0;
        }
        let c = e.caja;
        let dx = (c[0] - p[0]).max(0.0).max(p[0] - c[2]);
        let dy = (c[1] - p[1]).max(0.0).max(p[1] - c[3]);
        return dx.hypot(dy);
    }
    e.formas
        .iter()
        .map(|f| f.distancia(p))
        .fold(f64::INFINITY, f64::min)
}

/// Si el recuadro de `a` a `b` es de captura (de derecha a izquierda).
pub fn es_captura(a: P, b: P) -> bool {
    b[0] < a[0]
}

/// **Lo elegido por un recuadro** de `a` a `b`.
pub fn por_recuadro(ind: &Indice, a: P, b: P) -> Vec<Handle> {
    let caja: Caja = [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ];
    let captura = es_captura(a, b);
    ind.en(&caja)
        .into_iter()
        .filter(|e| !e.bloqueada)
        .filter(|e| {
            if geo::contiene(&caja, &e.caja) {
                return true;
            }
            captura && toca(e, &caja)
        })
        .map(|e| e.handle)
        .collect()
}

fn toca(e: &Entrada, caja: &Caja) -> bool {
    if e.formas.is_empty() {
        return geo::se_tocan(&e.caja, caja);
    }
    e.formas.iter().any(|f| {
        if !geo::se_tocan(&f.caja(), caja) {
            return false;
        }
        let m = f.muestras(48);
        m.windows(2)
            .any(|s| geo::segmento_toca_caja(s[0], s[1], caja))
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::Dibujo;
    use crate::especialidad::{linea, texto};

    #[test]
    fn clic_ventana_y_captura() {
        let mut d = Dibujo::nuevo();
        let (mut a, mut b, mut t) = (Handle::NULL, Handle::NULL, Handle::NULL);
        d.editar("x", |tr| {
            a = tr.anadir(linea([0.0, 0.0], [10.0, 0.0])).unwrap();
            b = tr.anadir(linea([0.0, 5.0], [30.0, 5.0])).unwrap();
            t = tr
                .anadir(texto("Hola", [0.0, 20.0], 2.0, 0.0, false))
                .unwrap();
        });
        let ind = d.indice();
        assert_eq!(elegir(&ind, [5.0, 0.4], 1.0), Some(a));
        assert_eq!(elegir(&ind, [1.0, 21.0], 1.0), Some(t));
        assert_eq!(elegir(&ind, [5.0, 2.5], 1.0), None);
        // Ventana de 0 a 20: solo la linea corta entera.
        let v = por_recuadro(&ind, [-1.0, -1.0], [20.0, 6.0]);
        assert_eq!(v, vec![a]);
        // Captura (al reves): las dos.
        let mut c = por_recuadro(&ind, [20.0, 6.0], [-1.0, -1.0]);
        c.sort();
        let mut esperado = vec![a, b];
        esperado.sort();
        assert_eq!(c, esperado);
    }

    #[test]
    fn lo_de_una_capa_bloqueada_no_se_elige() {
        let mut d = Dibujo::nuevo();
        d.editar("x", |tr| {
            tr.anadir_en(linea([0.0, 0.0], [10.0, 0.0]), "A-MUROS")
        });
        d.con_capa("A-MUROS", |l| l.lock());
        assert_eq!(elegir(&d.indice(), [5.0, 0.0], 1.0), None);
    }
}
