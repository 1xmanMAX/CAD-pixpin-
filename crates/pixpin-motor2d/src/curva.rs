//! **La flecha curva** (y la linea curva): la «round» de Excalidraw.
//!
//! En Excalidraw una flecha o una linea con `roundness` no se traza de punto
//! a punto en rectas: se traza una curva suave que **pasa por** todos sus
//! puntos (`shape.ts`, rama `generator.curve`). Es la spline de Catmull-Rom
//! de rough.js (`_curve` con `curveTightness` 0), la misma que porta el movil
//! en `Rough.curveThrough`: entre cada par de puntos una Bezier cubica cuyos
//! tiradores salen de los vecinos, `P[i] + (P[i+1] - P[i-1]) / 6`.
//!
//! Los extremos se repiten al entrar (`_curveWithOffset` mete el primero y el
//! ultimo dos veces), y eso es lo que hace que la curva **empiece y acabe
//! exactamente en los extremos** con la direccion del primer y el ultimo
//! tramo: la punta de la flecha cae donde se solto el raton, no un poco mas
//! alla.
//!
//! Con dos puntos la curva es la recta: por eso quien pinta sigue usando la
//! recta de siempre con dos puntos, y la curva solo cambia algo cuando la
//! flecha tiene un vertice en medio (el tirador del medio de un tramo lo
//! crea, como en Excalidraw).
//!
//! Una sola cuenta para las tres cosas que tienen que coincidir: lo que se
//! pinta (`pintado.rs`), lo que se toca (`impacto.rs`) y hacia donde mira la
//! punta (que se calcula sobre la curva muestreada).

use crate::azar::Azar;
use crate::elemento::{Elemento, Figura};
use crate::vector::Punto2;

/// Cuantos tramos rectos por cada Bezier al muestrearla: con 12 no se ve la
/// quebrada a los zooms de trabajo, y es lo mismo que usa `formas::linea`.
pub const PASOS_POR_TRAMO: usize = 12;

/// Las Bezier cubicas de la curva que pasa por `puntos`: inicio, dos
/// tiradores y fin, una por cada par de puntos seguidos. Vacio con menos de
/// dos puntos, que no son una curva.
pub fn cubicas(puntos: &[Punto2]) -> Vec<[Punto2; 4]> {
    if puntos.len() < 2 {
        return Vec::new();
    }
    // Los extremos repetidos, como `_curveWithOffset`: la lista ampliada es
    // [P0, P0, P1, ..., Pn, Pn]. Sin esto el primer tirador tiraria hacia un
    // vecino que no existe.
    let ultimo = puntos.len() - 1;
    let ampliado = |k: usize| puntos[k.saturating_sub(1).min(ultimo)];
    let n = puntos.len() + 2;
    (1..n - 2)
        .map(|i| {
            let (a, b, c, d) = (
                ampliado(i - 1),
                ampliado(i),
                ampliado(i + 1),
                ampliado(i + 2),
            );
            [
                b,
                b.sumar(c.restar(a).escalar(1.0 / 6.0)),
                c.sumar(b.restar(d).escalar(1.0 / 6.0)),
                c,
            ]
        })
        .collect()
}

fn cubica(c: &[Punto2; 4], t: f32) -> Punto2 {
    let u = 1.0 - t;
    let (a, b, cc, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    Punto2::nuevo(
        c[0].x * a + c[1].x * b + c[2].x * cc + c[3].x * d,
        c[0].y * a + c[1].y * b + c[2].y * cc + c[3].y * d,
    )
}

/// La curva que pasa por `puntos`, muestreada a una polilinea que empieza y
/// acaba en sus extremos exactos.
pub fn muestrear(puntos: &[Punto2]) -> Vec<Punto2> {
    let tramos = cubicas(puntos);
    let mut salida = Vec::with_capacity(tramos.len() * PASOS_POR_TRAMO + 1);
    for c in &tramos {
        for i in 0..PASOS_POR_TRAMO {
            salida.push(cubica(c, i as f32 / PASOS_POR_TRAMO as f32));
        }
    }
    if let Some(ultimo) = puntos.last() {
        if !tramos.is_empty() {
            salida.push(*ultimo);
        }
    }
    salida
}

/// **Si este elemento se traza curvo, y por donde.** `None` para todo lo
/// que va recto: sin `roundness`, de codos (sus codos ya vienen redondeados
/// y curvarlos otra vez perderia los 90 grados, igual que en el movil), o con
/// dos puntos, donde la curva ES la recta y no hace falta pagarla.
pub fn trazado_curvo(e: &Elemento) -> Option<Vec<Punto2>> {
    if !e.redondo {
        return None;
    }
    let puntos = match &e.figura {
        Figura::Flecha {
            puntos,
            codos: false,
            ..
        }
        | Figura::Linea { puntos } => puntos,
        _ => return None,
    };
    (puntos.len() >= 3).then(|| muestrear(puntos))
}

/// Las pasadas «a mano» de la curva, como `Rough.curveThrough`: dos curvas
/// por los mismos puntos, cada una con cada punto sacudido por el azar del
/// elemento (1 y 1,5 veces la rugosidad, con el mismo crecimiento que
/// rough.js). Con rugosidad cero, una sola pasada exacta: dos iguales solo
/// engordarian el trazo.
pub fn pasadas_a_mano(puntos: &[Punto2], rugosidad: f32, azar: &mut Azar) -> Vec<Vec<Punto2>> {
    if rugosidad <= 0.0 {
        return vec![muestrear(puntos)];
    }
    [
        1.0 * (1.0 + rugosidad * 0.2),
        1.5 * (1.0 + rugosidad * 0.22),
    ]
    .into_iter()
    .map(|desvio| {
        let d = desvio * rugosidad;
        let movidos: Vec<Punto2> = puntos
            .iter()
            .map(|p| Punto2::nuevo(p.x + azar.desvio(d), p.y + azar.desvio(d)))
            .collect();
        muestrear(&movidos)
    })
    .collect()
}

/// La caja de una polilinea: la de la curva y no la de sus puntos, porque la
/// curva se pasa un poco de ellos en las vueltas cerradas.
pub fn caja(puntos: &[Punto2]) -> Option<(f32, f32, f32, f32)> {
    let primero = puntos.first()?;
    Some(puntos.iter().fold(
        (primero.x, primero.y, primero.x, primero.y),
        |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
    ))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::formas::TipoPunta;

    fn p(x: f32, y: f32) -> Punto2 {
        Punto2::nuevo(x, y)
    }

    #[test]
    fn la_curva_pasa_por_todos_sus_puntos_y_acaba_en_los_extremos() {
        let puntos = [p(0.0, 0.0), p(50.0, 80.0), p(100.0, 0.0)];
        let curva = muestrear(&puntos);
        assert_eq!(curva.first(), Some(&puntos[0]));
        assert_eq!(curva.last(), Some(&puntos[2]));
        // El vertice del medio es un punto de la curva, no un tirador.
        assert!(curva.iter().any(|q| q.distancia(puntos[1]) < 1e-3));
        // Y no es la quebrada: a mitad del primer tramo se ha separado de la
        // recta de 0,0 a 50,80.
        let mitad = curva[PASOS_POR_TRAMO / 2];
        let en_la_recta = crate::vector::distancia_a_segmento(mitad, puntos[0], puntos[1]);
        assert!(en_la_recta > 1.0, "{en_la_recta}");
    }

    #[test]
    fn con_dos_puntos_la_curva_es_la_recta() {
        let curva = muestrear(&[p(0.0, 0.0), p(90.0, 30.0)]);
        for q in &curva {
            let d = crate::vector::distancia_a_segmento(*q, p(0.0, 0.0), p(90.0, 30.0));
            assert!(d < 1e-3, "{q:?}");
        }
        // Caso negativo: con uno solo no hay curva que muestrear.
        assert!(muestrear(&[p(1.0, 1.0)]).is_empty());
        assert!(cubicas(&[]).is_empty());
    }

    #[test]
    fn solo_se_curva_lo_redondo_que_no_es_de_codos_y_tiene_vertice() {
        let puntos = vec![p(0.0, 0.0), p(40.0, 50.0), p(90.0, 60.0)];
        let flecha = |codos| Figura::Flecha {
            puntos: puntos.clone(),
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos,
        };
        let con = |figura, redondo| Elemento {
            figura,
            redondo,
            ..Default::default()
        };
        assert!(trazado_curvo(&con(flecha(false), true)).is_some());
        assert!(
            trazado_curvo(&con(
                Figura::Linea {
                    puntos: puntos.clone()
                },
                true
            ))
            .is_some()
        );
        // Casos negativos: recta, de codos, con dos puntos, o un rectangulo
        // redondeado (que se redondea por sus esquinas, no por aqui).
        assert!(trazado_curvo(&con(flecha(false), false)).is_none());
        assert!(trazado_curvo(&con(flecha(true), true)).is_none());
        let corta = Figura::Linea {
            puntos: puntos[..2].to_vec(),
        };
        assert!(trazado_curvo(&con(corta, true)).is_none());
        assert!(trazado_curvo(&con(Figura::Rectangulo, true)).is_none());
    }

    #[test]
    fn a_mano_son_dos_pasadas_parecidas_y_liso_una_exacta() {
        let puntos = [p(0.0, 0.0), p(50.0, 80.0), p(100.0, 0.0)];
        let mut azar = Azar::nuevo(7);
        let dos = pasadas_a_mano(&puntos, 1.0, &mut azar);
        assert_eq!(dos.len(), 2);
        assert_ne!(dos[0], dos[1], "dos pasadas iguales no parecen a mano");
        // Cada pasada se aparta de la exacta poco: es un temblor, no otra
        // curva.
        let exacta = muestrear(&puntos);
        for pasada in &dos {
            for (a, b) in pasada.iter().zip(&exacta) {
                assert!(a.distancia(*b) < 5.0, "{a:?} {b:?}");
            }
        }
        let una = pasadas_a_mano(&puntos, 0.0, &mut Azar::nuevo(7));
        assert_eq!(una, vec![exacta]);
    }
}
