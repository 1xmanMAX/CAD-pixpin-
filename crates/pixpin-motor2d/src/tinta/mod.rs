//! La tinta de Excalidraw, portada (E1). Ver `docs/excalidraw/mapa.md`.
//!
//! `freehand` y `laser` son portes literales y trabajan en f64. Este modulo
//! es la envoltura de Excalidraw (`getFreedrawOutlinePoints` en
//! `packages/element/src/shape.ts` @afa3a65): elige pluma, aplica los
//! factores de tamano y pasa a `Punto2`.

pub mod freehand;
pub mod grafito;
pub mod laser;
pub mod material;
pub mod prediccion;

pub use material::{MATERIALES, MaterialTinta, paso_del_grano, tejido};

use serde::{Deserialize, Serialize};

use crate::vector::Punto2;

pub const STREAMLINE_RATON: f32 = 0.5;
pub const STREAMLINE_LAPIZ: f32 = 0.2;
/// El suavizado de los trazos NUEVOS con raton. `STREAMLINE_RATON` (0,5, el
/// de Excalidraw) se queda para abrir los dibujos guardados sin valor, que
/// no deben cambiar de aspecto. Con 0,5 el trazo se separaba del camino de
/// la mano y el usuario lo sentia como «mucha correccion de escritura».
pub const STREAMLINE_RATON_NUEVO: f32 = 0.15;
/// `VARIABLE_WIDTH_FREEDRAW.SIZE_FACTOR`: afinado a ojo por Excalidraw.
pub const FACTOR_VARIABLE: f32 = 4.25;
/// `CONSTANT_WIDTH_FREEDRAW.SIZE_FACTOR`.
pub const FACTOR_CONSTANTE: f32 = 1.4;
/// `FREEDRAW_STROKE_WIDTH`: la mitad que el resto de figuras (esquema 2.0).
pub const GROSOR_FINO: f32 = 0.5;
pub const GROSOR_MEDIO: f32 = 1.0;
pub const GROSOR_GRUESO: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Variabilidad {
    #[default]
    #[serde(rename = "variable")]
    Variable,
    #[serde(rename = "constant")]
    Constante,
}

/// `strokeOptions` de Excalidraw, con sus mismos nombres en el fichero.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OpcionesTinta {
    #[serde(rename = "variability", default)]
    pub variabilidad: Variabilidad,
    #[serde(default = "streamline_raton")]
    pub streamline: f32,
}

fn streamline_raton() -> f32 {
    STREAMLINE_RATON
}

impl Default for OpcionesTinta {
    fn default() -> Self {
        Self {
            variabilidad: Variabilidad::Variable,
            streamline: STREAMLINE_RATON,
        }
    }
}

fn a_punto(v: [f64; 2]) -> Punto2 {
    Punto2::nuevo(v[0] as f32, v[1] as f32)
}

/// El contorno de un trazo de lapiz, como lo pinta Excalidraw.
///
/// `opciones == None` es un trazo de antes de E1: su `grosor` era el tamano
/// en pixeles y no un `strokeWidth`, asi que se divide por el factor para
/// que un dibujo viejo no se abra cuatro veces mas gordo.
pub fn contorno_de_lapiz(
    puntos: &[Punto2],
    presiones: &[f32],
    grosor: f32,
    opciones: Option<OpcionesTinta>,
) -> Vec<Punto2> {
    let (o, grosor) = match opciones {
        Some(o) => (o, grosor),
        None => (OpcionesTinta::default(), grosor / FACTOR_VARIABLE),
    };
    match o.variabilidad {
        Variabilidad::Constante => {
            let mut pts: Vec<[f64; 2]> = puntos.iter().map(|p| [p.x as f64, p.y as f64]).collect();
            // F4: lo tirado deprisa, derecho. Se asienta al pintar y no en los
            // puntos guardados: el movil hace lo mismo con los suyos, y si el
            // PC guardara los asentados el movil los asentaria otra vez.
            freehand::asentar_lo_rapido(&mut pts);
            laser::contorno(
                &pts,
                (grosor * FACTOR_CONSTANTE) as f64,
                o.streamline as f64,
            )
            .into_iter()
            .map(a_punto)
            .collect()
        }
        Variabilidad::Variable => {
            // Excalidraw guarda presiones solo si son reales; una lista de
            // otra longitud no puede venir de un lapiz y se trata como
            // simulada en vez de leer fuera de rango.
            let simular = presiones.len() != puntos.len();
            let mut entrada: Vec<freehand::Entrada> = puntos
                .iter()
                .enumerate()
                .map(|(i, p)| freehand::Entrada {
                    x: p.x as f64,
                    y: p.y as f64,
                    presion: if simular {
                        None
                    } else {
                        Some(presiones[i] as f64)
                    },
                })
                .collect();
            // F4, como en `strokeOptionsFor` del movil (`asentarLoRapido =
            // true`): antes del streamline, que es quien no llega a juntar
            // muestras tan separadas.
            freehand::asentar_entrada(&mut entrada);
            let op = freehand::Opciones {
                size: (grosor * FACTOR_VARIABLE) as f64,
                thinning: 0.6,
                smoothing: 0.5,
                streamline: o.streamline as f64,
                simular_presion: simular,
                last: true,
                easing: freehand::seno,
            };
            freehand::contorno(&entrada, &op)
                .into_iter()
                .map(a_punto)
                .collect()
        }
    }
}

/// **`presionFirme` del movil**: trazo de ancho constante, para escribir.
///
/// Sin el, lo escrito a mano adelgaza en las curvas —la pluma variable quita
/// tinta donde el trazo gira despacio— y la letra se rompe: una «e» pequena se
/// queda en un garabato con agujeros. Es la razon de que el campo exista.
///
/// Se resuelve **con la pluma que ya hay** y no con una tercera: ancho
/// constante es exactamente `Variabilidad::Constante`, o sea el contorno de
/// `laser.rs`, el mismo que Excalidraw usa para su lapiz de ancho fijo. Lo que
/// hace esta funcion es traducir el campo del movil a esa opcion, en un solo
/// sitio, para que quien pinta no tenga que saberlo.
///
/// **No inventa opciones donde no las habia.** Un trazo legado —sin
/// `strokeOptions`— con la presion firme puesta pasa a tener las de fabrica
/// con la pluma constante, que es lo que el campo pide; sin la presion firme
/// se queda como estaba, `None`, que es lo que le dice a
/// [`contorno_de_lapiz`] que su grosor son pixeles y no un `strokeWidth`.
pub fn opciones_con_presion_firme(
    opciones: Option<OpcionesTinta>,
    presion_firme: bool,
) -> Option<OpcionesTinta> {
    if !presion_firme {
        return opciones;
    }
    Some(OpcionesTinta {
        variabilidad: Variabilidad::Constante,
        ..opciones.unwrap_or_default()
    })
}

/// El contorno de un trazo de lapiz teniendo en cuenta la presion firme.
///
/// Es [`contorno_de_lapiz`] con el campo del movil ya aplicado. Existe para
/// que quien pinta llame a una sola funcion y no pueda olvidarse del campo en
/// uno de los dos sitios donde se pinta un lapiz —el de la escena y el del
/// trazo en curso—, que es como se veia el fallo: la letra salia firme
/// mientras se escribia y adelgazaba al soltar.
pub fn contorno_de_lapiz_firme(
    puntos: &[Punto2],
    presiones: &[f32],
    grosor: f32,
    opciones: Option<OpcionesTinta>,
    presion_firme: bool,
) -> Vec<Punto2> {
    // Con la presion firme las presiones sobran: la pluma constante no las
    // mira. Se pasan igual para no tener dos caminos que puedan discrepar.
    contorno_de_lapiz(
        puntos,
        presiones,
        grosor,
        opciones_con_presion_firme(opciones, presion_firme),
    )
}

/// **El resaltador del movil** (`Tool.HIGHLIGHTER`, `DrawController`): no es
/// un tipo propio, es **el lapiz translucido y gordo** —mismo FREEDRAW, misma
/// tinta de `strokeOptionsFor` (tamano x4,25, adelgazado 0,6, presion
/// simulada, tapas redondas)— con el grosor x5 (`ENGORDE_DEL_MARCADOR`) y la
/// opacidad al 40 % (`HIGHLIGHTER_OPACITY`). Antes (D45) iba a grosor
/// constante y `grosor * 3` de tamano: un tercio de ancho que en el movil, y
/// el usuario lo veia «muy delgado y extrano».
///
/// El suavizado es el del movil para su pulso de fabrica (arquitecto, 0,55).
pub const STREAMLINE_RESALTADOR: f32 = 0.55;
/// `HIGHLIGHTER_OPACITY` del movil, en fraccion.
pub const OPACIDAD_DEL_RESALTADOR: f32 = 0.40;
/// `ItemStyle.ENGORDE_DEL_MARCADOR`: cuanto engorda respecto al lapiz.
pub const ENGORDE_DEL_MARCADOR: f32 = 5.0;

/// Lo que mide de ancho, en unidades del mundo, la tinta de un resaltador de
/// `grosor` (`anchoPintadoDelLapiz` del movil, con su presion tipica 0,5). En
/// un solo sitio para que la figura que sale de el al pararse (`gesto.rs`) no
/// pueda medir otra cosa que su tinta.
pub fn ancho_del_resaltador(grosor: f32) -> f32 {
    let size = (grosor * FACTOR_VARIABLE) as f64;
    // strokeRadius(size, 0,6, 0,5, easeOutSine) = size * sen(0,5 * pi/2).
    (2.0 * size * freehand::seno(0.5)) as f32
}

pub fn contorno_de_resaltador(puntos: &[Punto2], grosor: f32) -> Vec<Punto2> {
    contorno_de_lapiz(
        puntos,
        &[],
        grosor,
        Some(OpcionesTinta {
            variabilidad: Variabilidad::Variable,
            streamline: STREAMLINE_RESALTADOR,
        }),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn recta() -> Vec<Punto2> {
        (0..40)
            .map(|i| Punto2::nuevo(i as f32 * 5.0, 0.0))
            .collect()
    }

    fn alto(c: &[Punto2]) -> f32 {
        let (mn, mx) = c
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        mx - mn
    }

    #[test]
    fn un_trazo_legado_no_engorda_al_abrirlo_con_la_tinta_nueva() {
        // Grosor 4,25 sin opciones = el mismo trazo que grosor 1 con opciones.
        let legado = contorno_de_lapiz(&recta(), &[], 4.25, None);
        let nuevo = contorno_de_lapiz(&recta(), &[], 1.0, Some(OpcionesTinta::default()));
        assert_eq!(legado, nuevo);
    }

    #[test]
    fn la_pluma_constante_da_otra_forma_que_la_variable() {
        let v = contorno_de_lapiz(&recta(), &[], 1.0, Some(OpcionesTinta::default()));
        let constante = OpcionesTinta {
            variabilidad: Variabilidad::Constante,
            ..Default::default()
        };
        let c = contorno_de_lapiz(&recta(), &[], 1.0, Some(constante));
        assert_ne!(v, c);
    }

    #[test]
    fn presiones_de_otra_longitud_se_tratan_como_simuladas() {
        let p = recta();
        let simulada = contorno_de_lapiz(&p, &[], 1.0, Some(OpcionesTinta::default()));
        let rota = contorno_de_lapiz(&p, &[0.9, 0.9], 1.0, Some(OpcionesTinta::default()));
        assert_eq!(simulada, rota);
    }

    /// Un zigzag de +-`amplitud` con muestras cada `paso` unidades.
    fn zigzag(paso: f32, amplitud: f32) -> Vec<Punto2> {
        (0..30)
            .map(|i| {
                Punto2::nuevo(
                    i as f32 * paso,
                    if i % 2 == 0 { amplitud } else { -amplitud },
                )
            })
            .collect()
    }

    #[test]
    fn una_raya_lanzada_con_temblor_de_digitalizador_pinta_como_una_recta() {
        // F4 de punta a punta: el temblor de +-1,5 a muestras de 20 no llega
        // al contorno. Se compara la franja central (sin las tapas, que las
        // puntas no se asientan) contra la de una recta del mismo grosor.
        let o = Some(OpcionesTinta::default());
        let franja = |c: &[Punto2]| {
            let (mn, mx) = c
                .iter()
                .filter(|p| p.x > 100.0 && p.x < 480.0)
                .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
            mx - mn
        };
        let lanzada = franja(&contorno_de_lapiz(&zigzag(20.0, 1.5), &[], 1.0, o));
        let recta: Vec<Punto2> = (0..30)
            .map(|i| Punto2::nuevo(i as f32 * 20.0, 0.0))
            .collect();
        let limpia = franja(&contorno_de_lapiz(&recta, &[], 1.0, o));
        assert!(
            (lanzada - limpia).abs() < 0.6,
            "la onda se sigue viendo: {lanzada} frente a {limpia}"
        );
    }

    #[test]
    fn la_letra_escrita_despacio_se_pinta_igual_que_sin_asentar() {
        // El negativo de F4: a dos unidades por muestra no se toca nada, asi
        // que el contorno es el de perfect-freehand a pelo, bit a bit.
        let p = zigzag(2.0, 1.0);
        let de_lapiz = contorno_de_lapiz(&p, &[], 1.0, Some(OpcionesTinta::default()));
        let entrada: Vec<freehand::Entrada> = p
            .iter()
            .map(|q| freehand::Entrada {
                x: q.x as f64,
                y: q.y as f64,
                presion: None,
            })
            .collect();
        let op = freehand::Opciones {
            size: FACTOR_VARIABLE as f64,
            thinning: 0.6,
            smoothing: 0.5,
            streamline: STREAMLINE_RATON as f64,
            simular_presion: true,
            last: true,
            easing: freehand::seno,
        };
        let a_pelo: Vec<Punto2> = freehand::contorno(&entrada, &op)
            .into_iter()
            .map(a_punto)
            .collect();
        assert_eq!(de_lapiz, a_pelo);
    }

    #[test]
    fn mas_grosor_es_mas_ancho() {
        let o = Some(OpcionesTinta::default());
        assert!(
            alto(&contorno_de_lapiz(&recta(), &[], GROSOR_GRUESO, o))
                > alto(&contorno_de_lapiz(&recta(), &[], GROSOR_FINO, o))
        );
    }

    /// Lo que mas y lo que menos engorda un contorno a lo largo del trazo:
    /// con la pluma variable el trazo adelgaza y con la firme, no.
    fn cuanto_adelgaza(c: &[Punto2]) -> f32 {
        // Se mide el alto del contorno en dos franjas, una del principio y
        // otra del medio: con adelgazamiento la del principio es mas fina.
        let franja = |x0: f32, x1: f32| {
            let dentro: Vec<f32> = c
                .iter()
                .filter(|p| p.x >= x0 && p.x <= x1)
                .map(|p| p.y)
                .collect();
            if dentro.len() < 2 {
                return 0.0;
            }
            dentro.iter().fold(f32::MIN, |a, b| a.max(*b))
                - dentro.iter().fold(f32::MAX, |a, b| a.min(*b))
        };
        franja(90.0, 110.0) - franja(0.0, 20.0)
    }

    #[test]
    fn la_presion_firme_deja_el_trazo_del_mismo_ancho_de_punta_a_punta() {
        // La razon de que el campo exista: sin el, lo escrito adelgaza en las
        // curvas y la letra se rompe.
        let p = recta();
        let variable = contorno_de_lapiz_firme(&p, &[], 2.0, None, false);
        let firme = contorno_de_lapiz_firme(&p, &[], 2.0, None, true);
        assert_ne!(variable, firme, "la presion firme no cambio nada");
        assert!(
            cuanto_adelgaza(&firme).abs() < cuanto_adelgaza(&variable).abs(),
            "la firme adelgaza tanto como la variable"
        );
    }

    #[test]
    fn la_presion_firme_es_exactamente_la_pluma_constante() {
        // Y no una tercera pluma: ancho constante ya lo sabe hacer `laser.rs`.
        let o = opciones_con_presion_firme(Some(OpcionesTinta::default()), true).unwrap();
        assert_eq!(o.variabilidad, Variabilidad::Constante);
        assert_eq!(o.streamline, OpcionesTinta::default().streamline);
    }

    #[test]
    fn sin_presion_firme_no_se_inventan_opciones_donde_no_las_habia() {
        // Caso negativo, y el que protege los dibujos viejos: un trazo legado
        // al que se le pusieran opciones de la nada se abriria cuatro veces
        // mas gordo, porque su grosor son pixeles y no un strokeWidth.
        assert_eq!(opciones_con_presion_firme(None, false), None);
        let suyas = OpcionesTinta {
            variabilidad: Variabilidad::Variable,
            streamline: 0.2,
        };
        assert_eq!(opciones_con_presion_firme(Some(suyas), false), Some(suyas));
        // Y con la presion firme puesta, un legado conserva su streamline de
        // fabrica y solo cambia de pluma.
        let legado = opciones_con_presion_firme(None, true).unwrap();
        assert_eq!(legado.variabilidad, Variabilidad::Constante);
    }

    /// El ancho de un contorno medido de canto a canto a traves del trazo
    /// (de `a` a `b`), en su tercio central: lo que se ve de la raya sin las
    /// tapas.
    fn ancho_a_traves(c: &[Punto2], a: Punto2, b: Punto2) -> f32 {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let largo = (dx * dx + dy * dy).sqrt();
        let (ux, uy) = (dx / largo, dy / largo);
        let (mn, mx) = c
            .iter()
            .filter(|p| {
                let t = (p.x - a.x) * ux + (p.y - a.y) * uy;
                t > largo / 3.0 && t < largo * 2.0 / 3.0
            })
            .map(|p| (p.x - a.x) * -uy + (p.y - a.y) * ux)
            .fold((f32::MAX, f32::MIN), |(a, b), d| (a.min(d), b.max(d)));
        mx - mn
    }

    #[test]
    fn el_resaltador_es_el_lapiz_del_movil_con_su_grosor() {
        // `DrawController`: el marcador es un FREEDRAW como el lapiz, con la
        // misma tinta (`strokeOptionsFor`: 4,25, adelgazado 0,6, presion
        // simulada). Solo cambian el grosor (x5) y la opacidad (40).
        for grosor in [2.5_f32, 5.0, 10.0] {
            let lapiz = contorno_de_lapiz(
                &recta(),
                &[],
                grosor,
                Some(OpcionesTinta {
                    variabilidad: Variabilidad::Variable,
                    streamline: STREAMLINE_RESALTADOR,
                }),
            );
            assert_eq!(
                contorno_de_resaltador(&recta(), grosor),
                lapiz,
                "grosor {grosor}"
            );
        }
    }

    #[test]
    fn el_resaltador_mediano_mide_lo_que_el_del_movil() {
        // `anchoPintadoDelLapiz(5)`: 2 * 5 * 4,25 * sen(45 grados) = 30,05.
        // El de antes media 9 (grosor 3 por 3): un tercio, «muy delgado».
        let tipico = ancho_del_resaltador(5.0);
        assert!((tipico - 30.05).abs() < 0.05, "{tipico}");
        // Y una raya real, trazada a ritmo normal, sale de ese orden y no de
        // nueve: entre el ancho tipico y el de presion llena.
        let a = Punto2::nuevo(0.0, 0.0);
        let b = Punto2::nuevo(195.0, 0.0);
        let ancho = ancho_a_traves(&contorno_de_resaltador(&recta(), 5.0), a, b);
        assert!(ancho > 25.0 && ancho < 42.0, "{ancho}");
    }

    #[test]
    fn el_resaltador_grueso_es_mas_ancho_que_el_fino() {
        // Caso negativo: medir el ancho no sirve si todo midiera lo mismo.
        let a = Punto2::nuevo(0.0, 0.0);
        let b = Punto2::nuevo(195.0, 0.0);
        let fino = ancho_a_traves(&contorno_de_resaltador(&recta(), 2.5), a, b);
        let grueso = ancho_a_traves(&contorno_de_resaltador(&recta(), 10.0), a, b);
        assert!(grueso > fino * 2.0, "{grueso} frente a {fino}");
    }

    #[test]
    fn las_opciones_viajan_con_los_nombres_de_excalidraw() {
        let o = OpcionesTinta {
            variabilidad: Variabilidad::Constante,
            streamline: 0.2,
        };
        let json = serde_json::to_string(&o).unwrap();
        assert!(json.contains("\"variability\":\"constant\""), "{json}");
    }
}
