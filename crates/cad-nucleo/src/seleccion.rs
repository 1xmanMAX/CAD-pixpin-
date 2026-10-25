//! **Elegir lo dibujado**: con un clic (lo mas cercano) o con un recuadro,
//! como en AutoCAD: de izquierda a derecha (ventana, azul) solo lo que cabe
//! entero; de derecha a izquierda (captura, verde) todo lo que toque.

use opencadcodec::Handle;

use crate::dibujo::{Entrada, Indice};
use crate::geo::{self, Caja, P};

/// **Lo elegible mas cercano a `p`**, a menos de `tol`. Solo mira las
/// celdas del indice alrededor de `p`.
pub fn elegir(ind: &Indice, p: P, tol: f64) -> Option<Handle> {
    let zona = geo::ensanchar([p[0], p[1], p[0], p[1]], tol);
    let mut mejor: Option<(f64, Handle)> = None;
    ind.en_cada(&zona, |e| {
        if e.bloqueada {
            return;
        }
        let tope = mejor.map_or(tol, |(m, _)| m.min(tol));
        if let Some(d) = distancia(e, p, &zona, tope)
            && d <= tol
            && mejor.is_none_or(|(m, _)| d < m)
        {
            mejor = Some((d, e.handle));
        }
    });
    mejor.map(|(_, h)| h)
}

/// Lo que dista `p` de una entrada: de sus formas o, si no tiene (un texto),
/// cero dentro de su caja. Las formas cuya caja no toca `zona` no se miden;
/// `None` si ninguna queda a `tope` o menos.
fn distancia(e: &Entrada, p: P, zona: &Caja, tope: f64) -> Option<f64> {
    if e.formas.is_empty() {
        let c = e.caja;
        let dx = (c[0] - p[0]).max(0.0).max(p[0] - c[2]);
        let dy = (c[1] - p[1]).max(0.0).max(p[1] - c[3]);
        return Some(dx.hypot(dy));
    }
    let mut d = f64::INFINITY;
    for f in &e.formas {
        if geo::se_tocan(&f.caja(), zona) {
            d = d.min(f.distancia(p));
        }
    }
    (d <= tope).then_some(d)
}

/// Si el recuadro de `a` a `b` es de captura (de derecha a izquierda).
pub fn es_captura(a: P, b: P) -> bool {
    b[0] < a[0]
}

/// **Lo elegido por un recuadro** de `a` a `b`. Primero por las cajas (lo
/// que cabe entero entra sin mas); solo en captura, y solo lo que el borde
/// del recuadro corta, se mira forma a forma y de forma exacta.
pub fn por_recuadro(ind: &Indice, a: P, b: P) -> Vec<Handle> {
    let caja: Caja = [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ];
    let captura = es_captura(a, b);
    let mut v = Vec::new();
    ind.en_cada(&caja, |e| {
        if !e.bloqueada && (geo::contiene(&caja, &e.caja) || (captura && toca(e, &caja))) {
            v.push(e.handle);
        }
    });
    v
}

fn toca(e: &Entrada, caja: &Caja) -> bool {
    if e.formas.is_empty() {
        return geo::se_tocan(&e.caja, caja);
    }
    e.formas.iter().any(|f| {
        let k = f.caja();
        geo::se_tocan(&k, caja) && (geo::contiene(caja, &k) || f.toca_caja(caja))
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::Dibujo;
    use crate::especialidad::{arco, circulo, linea, texto};

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
    fn captura_exacta_con_circulos_y_arcos() {
        let mut d = Dibujo::nuevo();
        let (mut c, mut a) = (Handle::NULL, Handle::NULL);
        d.editar("x", |tr| {
            c = tr.anadir(circulo([0.0, 0.0], 100.0)).unwrap();
            // El cuarto de arriba a la derecha.
            a = tr
                .anadir(arco([300.0, 0.0], 50.0, 0.0, std::f64::consts::FRAC_PI_2))
                .unwrap();
        });
        let ind = d.indice();
        // Un recuadro dentro del circulo, sin tocar la curva: nada.
        assert!(por_recuadro(&ind, [10.0, 10.0], [-10.0, -10.0]).is_empty());
        // Uno que corta la curva: el circulo.
        assert_eq!(por_recuadro(&ind, [110.0, 10.0], [90.0, -10.0]), vec![c]);
        // Dentro de la caja del arco pero en el lado que no tiene: nada.
        assert!(por_recuadro(&ind, [315.0, 20.0], [301.0, 1.0]).is_empty());
        // Cruzando su trozo: el arco.
        assert_eq!(por_recuadro(&ind, [345.0, 30.0], [330.0, 20.0]), vec![a]);
        // Clic al lado de la curva, no en el centro.
        assert_eq!(elegir(&ind, [0.0, 99.5], 1.0), Some(c));
        assert_eq!(elegir(&ind, [0.0, 0.0], 1.0), None);
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
