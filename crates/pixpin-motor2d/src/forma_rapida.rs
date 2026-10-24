//! Forma rapida: un trazo a mano que se queda quieto se convierte en figura.
//!
//! Es lo que hacen QuickShape de Procreate y «Dibujar a forma» de
//! Excalidraw: se dibuja un circulo torcido, se deja el lapiz quieto sin
//! soltar, y aparece un circulo limpio que se sigue ajustando al arrastrar.
//!
//! El reconocimiento es puro y barato: corre UNA vez, cuando el trazo se
//! para, sobre sus puntos. Es el «gesto de pararse» del movil
//! (`DrawController.latido`, v0.80.2), en este orden:
//! - **Compas**: el cursor se clavo sin ir a ningun sitio (todo el trazo
//!   cabe en `LO_QUE_ES_UN_TOQUE_PX`). Ahi esta el centro, y lo que se
//!   arrastre despues abre un circulo hasta el cursor.
//! - **Rectangulo de una «L»** (`GestoDeRectangulo.kt`): una raya, un codo
//!   de 60 a 120 grados y otra raya. Sale un rectangulo con una esquina
//!   donde empezo y la otra en el cursor, que sigue tirando de ella.
//! - Y lo que ya reconocia el PC con el trazo quieto: **linea** (todos los
//!   puntos cerca de la cuerda), **rectangulo cerrado** (esquinas de casi 90
//!   grados y lados derechos) y **elipse** (cerrado o casi, cerca de la
//!   elipse inscrita en su caja).
//!
//! Si no es ninguna, el trazo se queda como esta. El movil, en cambio,
//! endereza en recta cualquier trazo que se para; aqui no, porque con raton
//! se duda a mitad de una letra mucho mas que con el dedo y convertir un
//! garabato en raya sin pedirlo es peor que no tener el gesto.

use crate::vector::{Punto2, distancia_a_segmento};

/// Cuanto tiene que quedarse quieto el cursor, dibujando a mano, para que el
/// trazo salga limpio: `ESPERA_PARA_LA_RECTA` del movil. Es el mismo numero a
/// proposito: el gesto es el mismo, y un gesto que se cumple a distinto ritmo
/// en dos aparatos son dos gestos que aprender. Menos, y una raya trazada
/// despacio se endereza sola en mitad de una curva.
pub const ESPERA_PARA_LA_FORMA_MS: u64 = 550;

/// **Con el grafito, un segundo** (`ESPERA_DEL_GRAFITO` del movil, v0.92).
/// El grafito es escritura libre: la mano se para a cada letra, y con el
/// medio segundo del lapiz lo escrito se volvia raya —«me lo corrige»—. Los
/// gestos los quiere igual, asi que piden mas tiempo quieto.
pub const ESPERA_DEL_GRAFITO_MS: u64 = 1000;

/// Lo que mide en pantalla, en pixeles, un trazo de grafito para que
/// pararse lo vuelva raya, rectangulo o figura
/// (`TRAZO_DE_FIGURA_DEL_GRAFITO` del movil): la diagonal de su caja. Una
/// letra no llega; el compas no lo pide.
pub const TRAZO_DE_FIGURA_DEL_GRAFITO_PX: f32 = 160.0;

/// Si un trazo de grafito es lo bastante grande en pantalla para hacerse
/// figura al pararse (ver `TRAZO_DE_FIGURA_DEL_GRAFITO_PX`). `zoom` son
/// pixeles de pantalla por unidad de escena.
pub fn trazo_de_figura_del_grafito(puntos: &[Punto2], zoom: f32) -> bool {
    if puntos.is_empty() {
        return false;
    }
    let (x0, y0, x1, y1) = caja(puntos);
    (x1 - x0).hypot(y1 - y0) * zoom.max(0.0001) >= TRAZO_DE_FIGURA_DEL_GRAFITO_PX
}

/// Cuanto puede temblar el cursor, en pixeles de pantalla, sin dejar de estar
/// parado (`TEMBLOR_DEL_DEDO` del movil). Con dos o tres la cuenta no llega a
/// cumplirse nunca en una mano normal, ni con lapiz de tableta.
pub const TEMBLOR_PX: f32 = 8.0;

/// Lo que se le deja moverse al trazo, en pixeles de pantalla, para que siga
/// siendo un punto y no un recorrido: y por tanto un compas y no una raya
/// (`LO_QUE_ES_UN_TOQUE` del movil). Se mide en pantalla porque lo que decide
/// si se quiso dibujar o clavar la punta es cuanto se movio la mano, a
/// cualquier aumento.
pub const LO_QUE_ES_UN_TOQUE_PX: f32 = 14.0;

/// Lo que tiene que medir cada brazo de la «L», en pixeles de pantalla
/// (`GestoDeRectangulo.BRAZO_MINIMO`). Menos, y el ganchito del final de una
/// raya pasa por esquina.
pub const BRAZO_MINIMO_PX: f32 = 36.0;

/// En que se convierte un trazo que se para.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlPararse {
    /// Compas clavado en `centro`: el radio lo pone el cursor.
    Compas { centro: Punto2 },
    /// Rectangulo de la «L»: `esquina` fija (donde empezo el trazo), la otra
    /// la pone el cursor.
    Ele { esquina: Punto2 },
    /// Lo que se reconoce por su dibujo entero.
    Forma(FormaReconocida),
}

/// El gesto de pararse entero, en el orden del movil: compas, «L», y luego
/// lo que se reconoce por su forma. `zoom` son pixeles de pantalla por unidad
/// de escena: los umbrales del gesto son de mano, no de dibujo.
pub fn al_pararse(puntos: &[Punto2], zoom: f32) -> Option<AlPararse> {
    let arranque = *puntos.first()?;
    let z = zoom.max(0.0001);
    // Parado sin haber ido a ningun sitio: eso es clavar la punta del compas.
    // Va el primero porque un punto no tiene forma que reconocer, y sin esto
    // mantener pulsado quieto no hacia nada (el reconocedor pide 20 de largo).
    if puntos
        .iter()
        .all(|p| p.distancia(arranque) * z <= LO_QUE_ES_UN_TOQUE_PX)
    {
        return Some(AlPararse::Compas { centro: arranque });
    }
    if es_una_ele(puntos, z) {
        return Some(AlPararse::Ele { esquina: arranque });
    }
    reconocer(puntos).map(AlPararse::Forma)
}

/// Si lo trazado es una «L»: porte de `GestoDeRectangulo.esUnaEle`.
///
/// Se simplifica el trazo (lo que tiembla la mano se va y quedan los
/// vertices) y se pide lo que tiene una L de verdad: dos brazos con cuerpo y
/// un codo de 60 a 120 grados. No hace falta que vayan de eje: el rectangulo
/// que sale va derecho igual, de la esquina de salida a la del cursor. Una
/// raya con una curva suave no llega a codo, y una V cerrada tampoco es L.
pub fn es_una_ele(puntos: &[Punto2], zoom: f32) -> bool {
    if puntos.len() < 3 {
        return false;
    }
    let z = zoom.max(0.0001);
    let largo: f32 = puntos.windows(2).map(|w| w[0].distancia(w[1])).sum();
    let mut s = simplificar(puntos, (10.0 / z).max(largo * 0.07));
    // Un codo redondeado deja dos vertices juntos: se funden en uno. Sin
    // esto, la «L» que la mano dobla en curva (la de casi todo el mundo) no
    // contaba, que es por lo que en el PC no salia el rectangulo.
    if s.len() == 4 {
        let entre = s[2].distancia(s[1]);
        let corto = s[1].distancia(s[0]).min(s[3].distancia(s[2]));
        if entre <= corto * 0.35 {
            let medio = Punto2::nuevo((s[1].x + s[2].x) / 2.0, (s[1].y + s[2].y) / 2.0);
            s = vec![s[0], medio, s[3]];
        }
    }
    if s.len() != 3 {
        return false;
    }
    let (la, lb) = (s[0].distancia(s[1]), s[1].distancia(s[2]));
    if la * z < BRAZO_MINIMO_PX || lb * z < BRAZO_MINIMO_PX {
        return false;
    }
    if la.min(lb) < 0.25 * la.max(lb) {
        return false;
    }
    // El coseno entre los brazos (como vectores de ida): 0 es un codo recto,
    // y hasta 0,5 son 60-120 grados.
    let (ax, ay) = (s[1].x - s[0].x, s[1].y - s[0].y);
    let (bx, by) = (s[2].x - s[1].x, s[2].y - s[1].y);
    ((ax * bx + ay * by) / (la * lb)).abs() <= 0.5
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormaReconocida {
    Linea {
        a: Punto2,
        b: Punto2,
    },
    /// Caja (x0, y0, x1, y1).
    Rectangulo {
        caja: (f32, f32, f32, f32),
    },
    Elipse {
        caja: (f32, f32, f32, f32),
    },
}

fn caja(puntos: &[Punto2]) -> (f32, f32, f32, f32) {
    puntos.iter().fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
    )
}

/// Ramer-Douglas-Peucker: los vertices que sobreviven con tolerancia `eps`.
fn simplificar(puntos: &[Punto2], eps: f32) -> Vec<Punto2> {
    if puntos.len() < 3 {
        return puntos.to_vec();
    }
    let (a, b) = (puntos[0], puntos[puntos.len() - 1]);
    let (mut peor, mut indice) = (0.0, 0);
    for (i, p) in puntos.iter().enumerate().take(puntos.len() - 1).skip(1) {
        let d = distancia_a_segmento(*p, a, b);
        if d > peor {
            peor = d;
            indice = i;
        }
    }
    if peor <= eps {
        return vec![a, b];
    }
    let mut izquierda = simplificar(&puntos[..=indice], eps);
    let derecha = simplificar(&puntos[indice..], eps);
    izquierda.pop();
    izquierda.extend(derecha);
    izquierda
}

/// Coseno del angulo en `b` entre `a-b` y `c-b`. 0 es un angulo recto.
fn coseno_esquina(a: Punto2, b: Punto2, c: Punto2) -> f32 {
    let (ux, uy) = (a.x - b.x, a.y - b.y);
    let (vx, vy) = (c.x - b.x, c.y - b.y);
    let n = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
    if n == 0.0 {
        return 1.0;
    }
    (ux * vx + uy * vy) / n
}

/// Si un segmento va casi en horizontal o casi en vertical (menos de ~20
/// grados de desvio): los rectangulos de Excalidraw van derechos.
fn casi_de_eje(a: Punto2, b: Punto2) -> bool {
    let (dx, dy) = ((b.x - a.x).abs(), (b.y - a.y).abs());
    dx.min(dy) <= dx.max(dy) * 0.36
}

pub fn reconocer(puntos: &[Punto2]) -> Option<FormaReconocida> {
    if puntos.len() < 4 {
        return None;
    }
    let largo: f32 = puntos.windows(2).map(|w| w[0].distancia(w[1])).sum();
    let c = caja(puntos);
    let diagonal = ((c.2 - c.0).powi(2) + (c.3 - c.1).powi(2)).sqrt();
    // Un garabato de menos de 20 unidades no es una forma: es un punto.
    if largo < 20.0 || diagonal < 12.0 {
        return None;
    }
    let (inicio, fin) = (puntos[0], puntos[puntos.len() - 1]);
    let cuerda = inicio.distancia(fin);

    // Linea: la cuerda casi es el recorrido y nadie se aparta de ella.
    if cuerda >= 0.85 * largo {
        let peor = puntos
            .iter()
            .map(|p| distancia_a_segmento(*p, inicio, fin))
            .fold(0.0, f32::max);
        if peor <= (0.06 * cuerda).max(3.0) {
            return Some(FormaReconocida::Linea { a: inicio, b: fin });
        }
    }

    let cerrado = cuerda <= 0.2 * diagonal.max(largo * 0.25);
    let vertices = simplificar(puntos, 0.07 * diagonal);

    // La «L» abierta ya no se mira aqui: es `es_una_ele`, la del movil, que
    // da un rectangulo que sigue al cursor en vez de la caja de lo trazado.
    if cerrado {
        // Rectangulo cerrado: 4 esquinas casi rectas. Se admite un vertice de
        // mas por el pequeño gancho al cerrar.
        let mut v = vertices.clone();
        if v.len() >= 2 && v[0].distancia(v[v.len() - 1]) <= 0.2 * diagonal {
            v.pop();
        }
        if (4..=5).contains(&v.len()) {
            let n = v.len();
            let rectas = (0..n)
                .filter(|&i| coseno_esquina(v[(i + n - 1) % n], v[i], v[(i + 1) % n]).abs() < 0.4)
                .count();
            let derechos = (0..n).all(|i| {
                let (a, b) = (v[i], v[(i + 1) % n]);
                a.distancia(b) < 0.08 * largo || casi_de_eje(a, b)
            });
            if rectas >= 3 && derechos {
                return Some(FormaReconocida::Rectangulo { caja: c });
            }
        }
    }

    // Elipse: cerca de la elipse inscrita en la caja, y cerrada o casi.
    if cuerda <= 0.4 * diagonal {
        let (cx, cy) = ((c.0 + c.2) / 2.0, (c.1 + c.3) / 2.0);
        let (rx, ry) = (((c.2 - c.0) / 2.0).max(1.0), ((c.3 - c.1) / 2.0).max(1.0));
        let error: f32 = puntos
            .iter()
            .map(|p| {
                let (u, w) = ((p.x - cx) / rx, (p.y - cy) / ry);
                ((u * u + w * w).sqrt() - 1.0).abs()
            })
            .sum::<f32>()
            / puntos.len() as f32;
        if error < 0.12 {
            return Some(FormaReconocida::Elipse { caja: c });
        }
    }
    None
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn circulo(cx: f32, cy: f32, r: f32, vuelta: f32, ruido: f32) -> Vec<Punto2> {
        (0..=60)
            .map(|i| {
                let t = std::f32::consts::TAU * vuelta * i as f32 / 60.0;
                // Un temblor determinista, como el de una mano.
                let d = ruido * ((i as f32 * 1.7).sin());
                Punto2::nuevo(cx + (r + d) * t.cos(), cy + (r + d) * t.sin())
            })
            .collect()
    }

    fn tramo(a: Punto2, b: Punto2, n: usize) -> Vec<Punto2> {
        (0..n)
            .map(|i| {
                let t = i as f32 / n as f32;
                Punto2::nuevo(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
            })
            .collect()
    }

    #[test]
    fn un_circulo_a_mano_es_una_elipse() {
        match reconocer(&circulo(100.0, 100.0, 50.0, 1.0, 2.5)) {
            Some(FormaReconocida::Elipse { caja }) => {
                assert!((caja.0 - 50.0).abs() < 5.0 && (caja.2 - 150.0).abs() < 5.0);
            }
            otro => panic!("esperaba elipse: {otro:?}"),
        }
    }

    #[test]
    fn un_circulo_casi_cerrado_tambien_cuenta() {
        assert!(matches!(
            reconocer(&circulo(0.0, 0.0, 40.0, 0.92, 1.0)),
            Some(FormaReconocida::Elipse { .. })
        ));
    }

    #[test]
    fn una_raya_torcida_poco_es_una_linea() {
        let mut p = tramo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(200.0, 40.0), 30);
        for (i, q) in p.iter_mut().enumerate() {
            q.y += ((i as f32) * 0.9).sin() * 2.0;
        }
        assert!(matches!(reconocer(&p), Some(FormaReconocida::Linea { .. })));
    }

    #[test]
    fn bajar_y_luego_ir_en_horizontal_es_una_ele_con_la_esquina_donde_empezo() {
        // Lo que pidio el usuario: una raya vertical y luego una horizontal.
        let mut p = tramo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(2.0, 100.0), 20);
        p.extend(tramo(
            Punto2::nuevo(2.0, 100.0),
            Punto2::nuevo(150.0, 97.0),
            20,
        ));
        p.push(Punto2::nuevo(150.0, 97.0));
        assert_eq!(
            al_pararse(&p, 1.0),
            Some(AlPararse::Ele {
                esquina: Punto2::nuevo(0.0, 0.0)
            })
        );
    }

    #[test]
    fn una_ele_con_el_codo_redondeado_tambien_cuenta() {
        // La mano no dobla en seco: el codo es un cuarto de circulo. Sin
        // fundir los dos vertices del codo, esta L no contaba.
        let mut p = tramo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(0.0, 85.0), 17);
        p.extend((0..=8).map(|i| {
            let t = std::f32::consts::FRAC_PI_2 * i as f32 / 8.0;
            Punto2::nuevo(15.0 - 15.0 * t.cos(), 85.0 + 15.0 * t.sin())
        }));
        p.extend(tramo(
            Punto2::nuevo(15.0, 100.0),
            Punto2::nuevo(140.0, 100.0),
            25,
        ));
        p.push(Punto2::nuevo(140.0, 100.0));
        assert!(es_una_ele(&p, 1.0));
    }

    #[test]
    fn una_ele_diminuta_en_pantalla_no_es_ele() {
        // Caso negativo: brazos de 20 px de pantalla son el ganchito del
        // final de una raya, no una esquina. La misma L a zoom 4 si cuenta.
        let mut p = tramo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(0.0, 20.0), 10);
        p.extend(tramo(
            Punto2::nuevo(0.0, 20.0),
            Punto2::nuevo(20.0, 20.0),
            10,
        ));
        p.push(Punto2::nuevo(20.0, 20.0));
        assert!(!es_una_ele(&p, 1.0));
        assert!(es_una_ele(&p, 4.0));
    }

    #[test]
    fn clavar_el_cursor_sin_ir_a_ningun_sitio_es_un_compas() {
        let temblor = [
            Punto2::nuevo(50.0, 50.0),
            Punto2::nuevo(52.0, 51.0),
            Punto2::nuevo(49.0, 53.0),
        ];
        assert_eq!(
            al_pararse(&temblor, 1.0),
            Some(AlPararse::Compas {
                centro: Punto2::nuevo(50.0, 50.0)
            })
        );
        // Un solo punto (pulsar y quedarse quieto, sin avisos de movimiento).
        assert!(matches!(
            al_pararse(&temblor[..1], 1.0),
            Some(AlPararse::Compas { .. })
        ));
        // Caso negativo: 30 px de recorrido ya es dibujar, no clavar.
        let raya = tramo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(30.0, 0.0), 10);
        assert!(!matches!(
            al_pararse(&raya, 1.0),
            Some(AlPararse::Compas { .. })
        ));
    }

    #[test]
    fn el_toque_del_compas_se_mide_en_pantalla() {
        // 10 unidades de escena: a zoom 1 es un toque; a zoom 3 son 30 px de
        // mano, y eso ya es haberse movido.
        let p = [Punto2::nuevo(0.0, 0.0), Punto2::nuevo(10.0, 0.0)];
        assert!(matches!(
            al_pararse(&p, 1.0),
            Some(AlPararse::Compas { .. })
        ));
        assert!(!matches!(
            al_pararse(&p, 3.0),
            Some(AlPararse::Compas { .. })
        ));
    }

    #[test]
    fn un_rectangulo_cerrado_a_mano_es_un_rectangulo_y_no_una_elipse() {
        let esquinas = [
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(120.0, 2.0),
            Punto2::nuevo(118.0, 80.0),
            Punto2::nuevo(-2.0, 78.0),
            Punto2::nuevo(1.0, 3.0),
        ];
        let mut p = Vec::new();
        for w in esquinas.windows(2) {
            p.extend(tramo(w[0], w[1], 15));
        }
        p.push(esquinas[4]);
        assert!(matches!(
            reconocer(&p),
            Some(FormaReconocida::Rectangulo { .. })
        ));
    }

    #[test]
    fn un_garabato_o_una_v_no_se_convierten() {
        // Caso negativo: una V de 45 grados no es una esquina de rectangulo.
        let mut v = tramo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(50.0, 100.0), 20);
        v.extend(tramo(
            Punto2::nuevo(50.0, 100.0),
            Punto2::nuevo(100.0, 0.0),
            20,
        ));
        v.push(Punto2::nuevo(100.0, 0.0));
        assert_eq!(reconocer(&v), None);
        // Ni para el gesto de pararse entero: una V cerrada no es una «L».
        assert_eq!(al_pararse(&v, 1.0), None);
        // Un zigzag tampoco.
        let zig: Vec<Punto2> = (0..40)
            .map(|i| Punto2::nuevo(i as f32 * 5.0, if i % 2 == 0 { 0.0 } else { 30.0 }))
            .collect();
        assert_eq!(reconocer(&zig), None);
        // Y un toque de nada.
        assert_eq!(
            reconocer(&tramo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(3.0, 3.0), 5)),
            None
        );
    }
}
