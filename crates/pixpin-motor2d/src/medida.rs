//! Que mide un pixel, y como se escribe una medida.
//!
//! Puerto de `Medida.kt` del Android. Todo puro.
//!
//! # La escala vive en la escena, no en la cota (D31)
//!
//! Un lienzo tiene UNA escala y todas sus cotas la usan. Si cada cota llevara
//! la suya, dos cotas del mismo plano podrian discrepar — y un plano con dos
//! escalas no sirve para nada.
//!
//! # La unidad es texto libre y aqui no se convierte nada (D32)
//!
//! En obra se mide en metros, en un plano de pieza en milimetros y en un mapa
//! en kilometros, y ninguna de las tres necesita que el motor sepa convertir
//! entre ellas. Se calibra en la unidad en la que se va a leer y se acabo.
//! Convertir inventaria el problema de mezclarlas, que no existe si no se
//! pueden mezclar.
//!
//! # El separador decimal llega por parametro (D39)
//!
//! Y no se lee del sistema, para que la cifra sea comprobable sin depender de
//! la configuracion de la maquina: una prueba que dependiera de ella daria
//! verde en un equipo y rojo en otro.

use crate::elemento::{Elemento, Figura};
use crate::vector::Punto2;
use serde::{Deserialize, Serialize};

/// Las que se ofrecen al calibrar, de mayor a menor uso.
pub const UNIDADES: [&str; 6] = ["m", "cm", "mm", "km", "ft", "in"];

/// Que mide un pixel de la escena.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Escala {
    /// Unidades de mundo real que vale cada pixel.
    pub unidades_por_pixel: f32,
    /// Texto libre a proposito (D32).
    pub unidad: String,
    /// Dos son los de una cota de obra.
    pub decimales: u8,
}

impl Escala {
    /// La escala que sale de decir que `largo_px` pixeles son `medida_real`.
    ///
    /// Devuelve `None` cuando la calibracion no puede ser cierta: una medida
    /// que no sea positiva, o una raya sin arrastrar. **Se rechaza aqui y no
    /// al usarla** (D36): mas vale no medir que medir mal, y un error que se
    /// deje pasar se propaga a todas las cotas del lienzo.
    pub fn calibrando(
        largo_px: f32,
        medida_real: f32,
        unidad: &str,
        decimales: u8,
    ) -> Option<Escala> {
        if !largo_px.is_finite() || !medida_real.is_finite() {
            return None;
        }
        if largo_px <= f32::EPSILON || medida_real <= 0.0 {
            return None;
        }
        Some(Escala {
            unidades_por_pixel: medida_real / largo_px,
            unidad: unidad.to_string(),
            decimales: decimales.min(6),
        })
    }

    /// Si vale para medir. Una escala que no sea un numero positivo, no.
    pub fn valida(&self) -> bool {
        self.unidades_por_pixel.is_finite() && self.unidades_por_pixel > 0.0
    }
}

/// Lo que mide el segmento de un elemento, en pixeles.
pub fn longitud_de(e: &Elemento) -> f32 {
    match extremos(e) {
        Some((a, b)) => a.distancia(b),
        None => 0.0,
    }
}

/// Hacia donde va, en grados **como se leen en un plano**: cero a la derecha
/// y creciendo hacia arriba (`anguloDe` del movil, `Medida.kt`).
///
/// La `y` de la pantalla crece hacia abajo, asi que el `atan2` crudo va al
/// reves de lo que dice cualquiera al mirar el dibujo: se le da la vuelta
/// aqui. Antes se dejaba crudo (bajar a la derecha eran 45 positivos) y la
/// misma cota decia «30°» en el movil y «-30°» aqui; y quien dicta «30°»
/// quiere treinta grados hacia arriba, no hacia abajo.
///
/// Los puntos de `e` estan en marco LOCAL: quien pinta (`pintado.rs:353`) y
/// quien pica (`impacto.rs:28`) aplican `e.angulo` por su cuenta. Si esta
/// funcion no hiciera lo mismo, girar una cota con el tirador la dibujaria y
/// la tocaria en un sitio, pero el rotulo seguiria diciendo el angulo de
/// antes de girar: una cota que miente sobre su propio angulo (D34).
/// `e.angulo` esta en radianes -es lo que usa `Punto2::girar`- y esta
/// funcion devuelve grados, asi que se convierte antes de sumar.
pub fn angulo_de(e: &Elemento) -> f32 {
    match extremos(e) {
        Some((a, b)) => {
            let d = b.restar(a);
            let local = d.y.atan2(d.x).to_degrees();
            normalizar_grados(-(local + e.angulo.to_degrees()))
        }
        None => 0.0,
    }
}

/// Deja el angulo entre -180 y 180 grados.
pub fn normalizar_grados(g: f32) -> f32 {
    if !g.is_finite() {
        return 0.0;
    }
    let mut r = g % 360.0;
    if r > 180.0 {
        r -= 360.0;
    } else if r <= -180.0 {
        r += 360.0;
    }
    r
}

/// Si al rotulo hay que darle la vuelta para que no se lea boca abajo.
pub fn rotulo_del_reves(grados: f32) -> bool {
    normalizar_grados(grados).abs() > 90.0
}

/// El numero y su unidad, ya formateados: los decimales que toquen, el punto
/// cambiado por `coma`, y la unidad detras.
///
/// Es solo el formato. Quien llama ya trae el valor en las unidades que hay
/// que escribir — esta funcion no convierte nada, eso es cosa de la escala.
pub(crate) fn formatear_valor(valor: f32, unidad: &str, decimales: u8, coma: char) -> String {
    let d = decimales.min(6) as usize;
    let s = format!("{valor:.d$}");
    format!("{} {}", s.replace('.', &coma.to_string()), unidad)
}

/// Como se escribe una longitud en pixeles.
///
/// Sin escala valida se escriben pixeles, que al menos no enganan. Es solo
/// el texto: el gris que completa el aviso de D35 lo pone quien pinta
/// (`pintado::ordenes_medibles`, con `COLOR_SIN_ESCALA`), porque esta
/// funcion no sabe de colores.
pub fn texto_de_medida(largo_px: f32, escala: Option<&Escala>, coma: char) -> String {
    let Some(e) = escala.filter(|e| e.valida()) else {
        return format!("{} px", largo_px.round() as i64);
    };
    let valor = largo_px * e.unidades_por_pixel;
    formatear_valor(valor, &e.unidad, e.decimales, coma)
}

/// Lo que va escrito **dentro** de la cota: cuanto mide y hacia donde va.
///
/// Las dos cosas en la propia raya y no en un panel aparte: una medida que
/// hay que ir a leer a otro sitio se usa para comprobar al final, cuando ya
/// esta todo mal. Encima de la raya, se traza mirando el numero.
///
/// El angulo solo sale si la raya esta torcida: en una horizontal, «0°» ocupa
/// la mitad del rotulo para no decir nada.
pub fn texto_de_cota(e: &Elemento, escala: Option<&Escala>, coma: char) -> String {
    let medida = texto_de_medida(longitud_de(e), escala, coma);
    let grados = angulo_de(e);
    if grados.abs() < 0.5 {
        return medida;
    }
    format!("{medida} · {}°", grados.round() as i64)
}

/// La medida en unidades de mundo, o `None` si todavia no hay escala.
///
/// Va aparte del texto porque quien exporta o suma cotas necesita el numero,
/// no la cadena con la que se pinta.
pub fn medida_de(e: &Elemento, escala: Option<&Escala>) -> Option<f32> {
    let esc = escala.filter(|x| x.valida())?;
    Some(longitud_de(e) * esc.unidades_por_pixel)
}

/// Estira o encoge el segmento hasta que mida `largo_px_deseado`.
///
/// **Mueve el otro extremo, nunca el origen.** Mover los dos, o mover el
/// origen, deja la cota donde no estaba, y es lo que se hace mal a la
/// primera.
pub fn con_longitud(e: &mut Elemento, largo_px_deseado: f32) {
    let Some((a, b)) = extremos(e) else { return };
    let actual = a.distancia(b);
    if actual <= f32::EPSILON || !largo_px_deseado.is_finite() {
        // Una raya sin arrastrar no tiene direccion que conservar. Dividir
        // por cero daria NaN, y un NaN en la geometria borra el elemento de
        // la pantalla sin ningun error visible.
        return;
    }
    let factor = largo_px_deseado / actual;
    let nuevo = Punto2::nuevo(a.x + (b.x - a.x) * factor, a.y + (b.y - a.y) * factor);
    poner_extremos(e, a, nuevo);
}

/// **La misma raya con el largo y el angulo que se dicten, anclada por donde
/// empieza** (`conLargoYAngulo` del movil).
///
/// Ese anclaje es lo que la hace servir: dibujando a mano se acierta el sitio
/// donde empieza una medida —una esquina, un cruce— y nunca el largo ni el
/// angulo. `grados` va como en un plano (ver [`angulo_de`]). Los intermedios
/// de una polilinea se pierden a proposito: con dos numeros se dice «de aqui
/// hasta alla», y eso es una recta. Sin nada que hacer (largo no positivo,
/// numeros que no lo son) la deja como estaba.
pub fn con_largo_y_angulo(e: &mut Elemento, largo_px: f32, grados: f32) {
    if !largo_px.is_finite() || largo_px <= 0.0 || !grados.is_finite() {
        return;
    }
    let Some(inicio) = crate::nudos::vertices_en_el_mundo(e).first().copied() else {
        return;
    };
    let rad = (-normalizar_grados(grados)).to_radians();
    let fin = Punto2::nuevo(
        inicio.x + largo_px * rad.cos(),
        inicio.y + largo_px * rad.sin(),
    );
    // El giro va ya en los puntos: con el angulo puesto se contaria dos veces.
    e.angulo = 0.0;
    poner_extremos(e, inicio, fin);
    // Cambiar el largo o el angulo suelta los anclajes: la punta ya no esta
    // donde estaba.
    e.extras.enganche_inicio = None;
    e.extras.enganche_fin = None;
}

/// El camino de vuelta de lo tecleado: de unidades de la escala a pixeles de
/// escena (`largoEnPixeles`). Sin escala valida, lo tecleado ya son pixeles.
pub fn largo_en_pixeles(valor: f32, escala: Option<&Escala>) -> f32 {
    match escala.filter(|e| e.valida()) {
        Some(e) => valor / e.unidades_por_pixel,
        None => valor,
    }
}

/// Lo que hay que teclear para ver la medida de `e`: en unidades si hay
/// escala, en pixeles si no (`largoEnUnidades`).
pub fn largo_en_unidades(e: &Elemento, escala: Option<&Escala>) -> f32 {
    medida_de(e, escala).unwrap_or_else(|| longitud_de(e))
}

/// Los dos extremos de un elemento con puntos, si los tiene.
fn extremos(e: &Elemento) -> Option<(Punto2, Punto2)> {
    let p = e.puntos()?;
    if p.len() < 2 {
        return None;
    }
    Some((p[0], p[p.len() - 1]))
}

/// Deja el elemento con exactamente esos dos puntos, y su caja al dia.
///
/// Este `match` lleva comodin: una figura nueva con puntos no rompe la
/// compilacion al añadirse, asi que hay que acordarse de venir aqui. Es lo
/// que dejo a la `Cota` sin arrastrar (el fallo que este comentario evita
/// repetir): caia en el `_ => return` y `con_longitud` no hacia nada.
fn poner_extremos(e: &mut Elemento, a: Punto2, b: Punto2) {
    match &mut e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. }
        | Figura::Cota { puntos } => {
            puntos.clear();
            puntos.push(a);
            puntos.push(b);
        }
        _ => return,
    }
    e.x = a.x.min(b.x);
    e.y = a.y.min(b.y);
    e.ancho = (b.x - a.x).abs();
    e.alto = (b.y - a.y).abs();
    e.tocar();
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo};

    /// Un segmento de a a b, con grosor cero para que la caja sean los puntos.
    fn cota(a: Punto2, b: Punto2) -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Linea { puntos: vec![a, b] },
            x: a.x.min(b.x),
            y: a.y.min(b.y),
            ancho: (b.x - a.x).abs(),
            alto: (b.y - a.y).abs(),
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 0.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    fn metros(por_pixel: f32) -> Escala {
        Escala {
            unidades_por_pixel: por_pixel,
            unidad: "m".to_string(),
            decimales: 2,
        }
    }

    #[test]
    fn calibrar_cien_pixeles_como_tres_metros_da_tres_centesimas_por_pixel() {
        let e = Escala::calibrando(100.0, 3.0, "m", 2).expect("calibracion valida");
        assert!((e.unidades_por_pixel - 0.03).abs() < 1e-6);
        assert_eq!(e.unidad, "m");
        assert_eq!(e.decimales, 2);
    }

    #[test]
    fn una_calibracion_imposible_se_rechaza_al_construirla() {
        // D36: mas vale no medir que medir mal. Un error que se deje pasar
        // se propaga a todas las cotas del lienzo.
        assert!(
            Escala::calibrando(100.0, 0.0, "m", 2).is_none(),
            "medida cero"
        );
        assert!(
            Escala::calibrando(100.0, -3.0, "m", 2).is_none(),
            "medida negativa"
        );
        assert!(
            Escala::calibrando(0.0, 3.0, "m", 2).is_none(),
            "raya sin arrastrar"
        );
        assert!(Escala::calibrando(f32::NAN, 3.0, "m", 2).is_none(), "NaN");
        assert!(
            Escala::calibrando(100.0, f32::INFINITY, "m", 2).is_none(),
            "infinito"
        );
    }

    #[test]
    fn sin_escala_el_rotulo_va_en_pixeles() {
        // D35: sin calibrar se mide en pixeles. Quien lo pinta lo pone en
        // gris, que es el aviso de que no es una medida de plano.
        assert_eq!(texto_de_medida(245.0, None, ','), "245 px");
        assert_eq!(texto_de_medida(245.7, None, ','), "246 px", "sin decimales");
    }

    #[test]
    fn con_escala_el_rotulo_va_en_la_unidad_calibrada() {
        assert_eq!(texto_de_medida(245.0, Some(&metros(0.01)), ','), "2,45 m");
    }

    #[test]
    fn el_separador_decimal_llega_por_parametro_y_no_del_sistema() {
        // D39: una prueba que dependiera del Windows donde corre daria
        // verde en un equipo y rojo en otro.
        let e = metros(0.01);
        assert_eq!(texto_de_medida(245.0, Some(&e), ','), "2,45 m");
        assert_eq!(texto_de_medida(245.0, Some(&e), '.'), "2.45 m");
    }

    #[test]
    fn los_decimales_son_los_de_la_escala() {
        let mut e = metros(0.01);
        e.decimales = 0;
        assert_eq!(texto_de_medida(245.0, Some(&e), ','), "2 m");
        e.decimales = 3;
        assert_eq!(texto_de_medida(245.0, Some(&e), ','), "2,450 m");
    }

    #[test]
    fn la_longitud_es_la_del_segmento() {
        let c = cota(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(3.0, 4.0));
        assert!(
            (longitud_de(&c) - 5.0).abs() < 1e-5,
            "el 3-4-5 de toda la vida"
        );
    }

    #[test]
    fn el_angulo_se_mide_en_grados_desde_la_horizontal() {
        let recta = cota(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(10.0, 0.0));
        assert!(angulo_de(&recta).abs() < 1e-3, "horizontal es cero");

        // Como en un plano y en el movil: bajar a la derecha es negativo,
        // subir, positivo (la y de la pantalla crece hacia abajo).
        let baja = cota(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(10.0, 10.0));
        assert!((angulo_de(&baja) + 45.0).abs() < 1e-3);
        let sube = cota(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(10.0, -10.0));
        assert!((angulo_de(&sube) - 45.0).abs() < 1e-3);
    }

    #[test]
    fn el_angulo_solo_sale_en_el_rotulo_si_la_raya_esta_torcida() {
        // «En una horizontal, 0 grados es ruido que ocupa la mitad del
        // rotulo para no decir nada.»
        let e = metros(0.01);

        let recta = cota(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        assert_eq!(texto_de_cota(&recta, Some(&e), ','), "1,00 m", "sin angulo");

        let torcida = cota(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 100.0));
        let t = texto_de_cota(&torcida, Some(&e), ',');
        assert!(t.contains("45"), "la torcida si lleva grados: {t}");
        assert!(t.contains('°'), "y su simbolo: {t}");
    }

    #[test]
    fn el_rotulo_dice_el_angulo_aunque_se_gire_con_e_angulo_y_no_con_los_puntos() {
        // El fallo real: `angulo_de` derivaba el angulo solo de los puntos,
        // en marco local, e ignoraba `e.angulo`. Pintar (`pintado.rs:353`) y
        // picar (`impacto.rs:28`) si lo aplican, asi que girar una cota con
        // el tirador la dibujaba y la tocaba a otro angulo mientras el
        // rotulo seguia diciendo el de antes de girar. El invariante de
        // verdad: girar el elemento entero (`e.angulo`) y girar sus puntos
        // a mano tienen que rotular exactamente igual.
        let recta = cota(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        let mut girada_por_angulo = recta.clone();
        girada_por_angulo.angulo = std::f32::consts::FRAC_PI_2;

        let centro = Punto2::nuevo(50.0, 0.0);
        let girada_por_puntos = cota(
            Punto2::nuevo(0.0, 0.0).girar(centro, std::f32::consts::FRAC_PI_2),
            Punto2::nuevo(100.0, 0.0).girar(centro, std::f32::consts::FRAC_PI_2),
        );

        let e = metros(0.01);
        let t_angulo = texto_de_cota(&girada_por_angulo, Some(&e), ',');
        let t_puntos = texto_de_cota(&girada_por_puntos, Some(&e), ',');
        assert_eq!(
            t_angulo, t_puntos,
            "girar con e.angulo y girar los puntos tienen que rotular igual"
        );
        assert_ne!(
            t_angulo,
            texto_de_cota(&recta, Some(&e), ','),
            "una cota girada 90 grados no puede rotular lo mismo que una recta"
        );
    }

    #[test]
    fn normalizar_grados_deja_todo_entre_menos_ciento_ochenta_y_ciento_ochenta() {
        assert!((normalizar_grados(370.0) - 10.0).abs() < 1e-3);
        assert!((normalizar_grados(-370.0) + 10.0).abs() < 1e-3);
        assert!((normalizar_grados(180.0) - 180.0).abs() < 1e-3);
        assert!((normalizar_grados(540.0) - 180.0).abs() < 1e-3);
    }

    #[test]
    fn el_rotulo_se_da_la_vuelta_para_no_leerse_boca_abajo() {
        assert!(!rotulo_del_reves(0.0));
        assert!(!rotulo_del_reves(45.0));
        assert!(!rotulo_del_reves(-45.0));
        assert!(rotulo_del_reves(91.0), "pasado el cuarto de vuelta");
        assert!(rotulo_del_reves(180.0));
        assert!(rotulo_del_reves(-91.0));
    }

    #[test]
    fn fijar_un_largo_exacto_mueve_el_otro_extremo_y_no_el_origen() {
        // Es lo que se hace mal a la primera: mover los dos extremos, o
        // mover el origen, deja la cota donde no estaba.
        let mut c = cota(Punto2::nuevo(10.0, 20.0), Punto2::nuevo(110.0, 20.0));
        con_longitud(&mut c, 50.0);

        let Figura::Linea { puntos } = &c.figura else {
            panic!("sigue siendo linea")
        };
        assert_eq!(
            puntos[0],
            Punto2::nuevo(10.0, 20.0),
            "el origen no se mueve"
        );
        assert!((puntos[1].x - 60.0).abs() < 1e-3, "el otro extremo si");
        assert!((longitud_de(&c) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn fijar_el_largo_conserva_la_direccion() {
        let mut c = cota(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(30.0, 40.0));
        let antes = angulo_de(&c);
        con_longitud(&mut c, 10.0);
        assert!((angulo_de(&c) - antes).abs() < 1e-3, "misma direccion");
        assert!((longitud_de(&c) - 10.0).abs() < 1e-3, "otro largo");
    }

    #[test]
    fn con_longitud_sobre_una_cota_cambia_su_longitud() {
        // Critico 3 del re-revisor: `poner_extremos` salia por
        // `_ => return` antes de tocar nada para una `Figura::Cota`, asi
        // que `con_longitud` sobre una cota de verdad no hacia
        // absolutamente nada. El fixture `cota()` de arriba construye una
        // `Figura::Linea`, asi que ninguna prueba existente lo cazaba.
        let mut c = Elemento {
            figura: Figura::Cota {
                puntos: vec![Punto2::nuevo(10.0, 20.0), Punto2::nuevo(110.0, 20.0)],
            },
            ..cota(Punto2::nuevo(10.0, 20.0), Punto2::nuevo(110.0, 20.0))
        };
        con_longitud(&mut c, 50.0);

        let Figura::Cota { puntos } = &c.figura else {
            panic!("sigue siendo una cota")
        };
        assert_eq!(
            puntos[0],
            Punto2::nuevo(10.0, 20.0),
            "el origen no se mueve"
        );
        assert!((puntos[1].x - 60.0).abs() < 1e-3, "el otro extremo si");
        assert!((longitud_de(&c) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn fijar_el_largo_de_una_raya_sin_arrastrar_no_revienta() {
        // Dividir por cero daria NaN, y un NaN en la geometria borra el
        // elemento de la pantalla sin ningun error visible.
        let mut c = cota(Punto2::nuevo(5.0, 5.0), Punto2::nuevo(5.0, 5.0));
        con_longitud(&mut c, 50.0);
        let Figura::Linea { puntos } = &c.figura else {
            panic!()
        };
        assert!(puntos.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
    }

    #[test]
    fn la_medida_en_numero_va_aparte_del_texto() {
        // Quien exporta o suma cotas necesita el numero, no la cadena.
        let e = metros(0.01);
        let c = cota(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        assert!((medida_de(&c, Some(&e)).unwrap() - 1.0).abs() < 1e-5);
        assert!(medida_de(&c, None).is_none(), "sin escala no hay medida");
    }

    #[test]
    fn una_escala_invalida_se_comporta_como_no_tenerla() {
        let mala = Escala {
            unidades_por_pixel: 0.0,
            unidad: "m".to_string(),
            decimales: 2,
        };
        assert!(!mala.valida());
        assert_eq!(texto_de_medida(245.0, Some(&mala), ','), "245 px");
    }
}

#[cfg(test)]
mod pruebas_dictar {
    //! **Dictarle a una cota cuanto mide y hacia donde va** (`conLargoYAngulo`
    //! del movil).
    use super::*;

    fn raya(a: (f32, f32), b: (f32, f32)) -> Elemento {
        Elemento {
            figura: Figura::Cota {
                puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
            },
            x: a.0.min(b.0),
            y: a.1.min(b.1),
            ancho: (b.0 - a.0).abs(),
            alto: (b.1 - a.1).abs(),
            ..Default::default()
        }
    }

    #[test]
    fn dictar_largo_y_angulo_deja_la_raya_anclada_por_donde_empieza() {
        let mut c = raya((10.0, 10.0), (57.0, 23.0));
        con_largo_y_angulo(&mut c, 50.0, 90.0);
        let p = c.puntos().unwrap();
        assert_eq!(p[0], Punto2::nuevo(10.0, 10.0), "se movio el principio");
        // Noventa grados es hacia ARRIBA, como en un plano.
        assert!(
            (p[1].x - 10.0).abs() < 1e-3 && (p[1].y + 40.0).abs() < 1e-3,
            "{:?}",
            p[1]
        );
        assert!((longitud_de(&c) - 50.0).abs() < 1e-3);
        assert!((angulo_de(&c) - 90.0).abs() < 1e-3);
    }

    #[test]
    fn una_raya_girada_se_dicta_desde_donde_se_ve_su_principio() {
        let mut c = raya((0.0, 0.0), (100.0, 0.0));
        c.angulo = std::f32::consts::FRAC_PI_2;
        let visto = crate::nudos::vertices_en_el_mundo(&c)[0];
        con_largo_y_angulo(&mut c, 30.0, 0.0);
        assert_eq!(c.angulo, 0.0);
        let p = c.puntos().unwrap();
        assert!(p[0].distancia(visto) < 1e-3, "{:?} y {visto:?}", p[0]);
        assert!((angulo_de(&c)).abs() < 1e-3);
    }

    #[test]
    fn un_largo_imposible_deja_la_cota_como_estaba() {
        let antes = raya((0.0, 0.0), (30.0, 40.0));
        for largo in [0.0, -5.0, f32::NAN, f32::INFINITY] {
            let mut c = antes.clone();
            con_largo_y_angulo(&mut c, largo, 10.0);
            assert_eq!(c, antes, "{largo}");
        }
        let mut c = antes.clone();
        con_largo_y_angulo(&mut c, 10.0, f32::NAN);
        assert_eq!(c, antes);
    }

    #[test]
    fn lo_tecleado_se_pasa_a_pixeles_con_la_escala_y_sin_ella_ya_son_pixeles() {
        let e = Escala {
            unidades_por_pixel: 0.01,
            unidad: "m".into(),
            decimales: 2,
        };
        assert!((largo_en_pixeles(2.0, Some(&e)) - 200.0).abs() < 1e-3);
        assert_eq!(largo_en_pixeles(2.0, None), 2.0);
        let c = raya((0.0, 0.0), (300.0, 400.0));
        assert!((largo_en_unidades(&c, Some(&e)) - 5.0).abs() < 1e-4);
        assert_eq!(largo_en_unidades(&c, None), 500.0);
    }
}
