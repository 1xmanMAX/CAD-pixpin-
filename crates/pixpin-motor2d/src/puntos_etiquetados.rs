//! **El punto etiquetado: A, B, C sobre el dibujo.**
//!
//! Port de `Puntos.kt` (313 lineas) del PixPin de Android.
//!
//! Es la herramienta de las matematicas. Un croquis de geometria no se explica
//! con flechas y textos sueltos: se explica **nombrando** los puntos y hablando
//! de ellos. «El triangulo ABC», «la mediatriz de AB», «M es el punto medio».
//! Sin poder nombrar un vertice, cada afirmacion hay que acompañarla de un dedo
//! señalando.
//!
//! Poner un texto al lado de una esquina es otra cosa: un texto no sabe a que
//! punto pertenece, no se mueve con el, no se numera solo y hay que colocarlo a
//! ojo cada vez para que no tape la figura.
//!
//! # Las tres decisiones que lo hacen servir
//!
//! **Se numeran solos y en serie.** Se pone A, y el siguiente es B. Nombrar a
//! mano quince vertices son quince oportunidades de repetir una letra sin darse
//! cuenta, y una letra repetida en un problema de geometria lo invalida entero.
//! Ver [`siguiente_etiqueta`].
//!
//! **La letra se coloca donde no estorba.** No a la derecha por omision
//! —encima de la figura la mitad de las veces— sino en el hueco mas ancho que
//! quede alrededor del punto. Es lo que hace uno a mano sin pensarlo: la letra
//! va hacia fuera. Ver [`angulo_libre`].
//!
//! **La letra orbita, el punto no.** Se arrastra la letra y da vueltas
//! alrededor de su punto sin separarse de el, porque una etiqueta suelta a tres
//! centimetros ya no dice de quien es. Ver [`con_la_etiqueta_hacia`].

use crate::elemento::{Elemento, Figura};
use crate::enganche::{Anclaje, TipoAnclaje};
use crate::perimetros::{PASO_PERIMETRO, intersecciones_cerca, segmentos_de};
use crate::vector::{Punto2, distancia_a_segmento};

/// De que serie sale la etiqueta de un punto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SerieDePunto {
    /// A, B, C… Z, A1, B1… Es la de los vertices de toda la vida.
    #[default]
    Mayusculas,
    /// a, b, c… Para lo que no es un vertice: lados, rectas, angulos.
    Minusculas,
    /// 1, 2, 3… Para nubes de puntos, donde las letras se acaban.
    Numeros,
}

/// Radio al que se pone la letra, en pixeles del documento.
///
/// Lo justo para que el redondel no la toque y siga leyendose como suya. Mas
/// lejos empieza a parecer un texto suelto.
pub const RADIO_DE_LA_LETRA: f32 = 22.0;
pub const RADIO_MINIMO: f32 = 14.0;
pub const RADIO_MAXIMO: f32 = 60.0;

/// Hasta donde busca sitio la herramienta de puntos, en pixeles de pantalla.
///
/// Mas generoso que el iman de siempre: aqui el cursor **tiene** que acertar en
/// un cruce o en un extremo para que pase algo, asi que apretar el radio
/// convertiria la herramienta en un juego de punteria. Al no haber puntos
/// libres, un radio grande no puede colocar nada donde no debia — como mucho,
/// engancha al sitio notable de al lado.
pub const RADIO_PARA_PUNTOS: f32 = 30.0;

/// Hasta donde se mira para saber que sale del punto.
pub const ALCANCE_DE_LA_LETRA: f32 = 6.0;

/// Lo que hay que acertar con el cursor para coger la letra, en pixeles del
/// documento.
///
/// **Mucho mas que lo que se ve.** Es la regla de siempre: lo que se dibuja es
/// para el ojo y lo que se toca es para la mano.
pub const RADIO_DE_AGARRE: f32 = 16.0;

/// La etiqueta que hace el numero `i` de la serie, contando desde cero.
///
/// Pasada la Z no se para ni se inventa simbolos raros: sigue con **A1, B1…** y
/// luego A2. Es lo que se escribe a mano cuando hacen falta mas de veintiseis,
/// se teclea sin salir del teclado normal y no se confunde con nada.
pub fn etiqueta_numero(i: u32, serie: SerieDePunto) -> String {
    if serie == SerieDePunto::Numeros {
        return (i + 1).to_string();
    }
    let base = if serie == SerieDePunto::Mayusculas {
        b'A'
    } else {
        b'a'
    };
    let letra = (base + (i % 26) as u8) as char;
    let vuelta = i / 26;
    if vuelta == 0 {
        letra.to_string()
    } else {
        format!("{letra}{vuelta}")
    }
}

/// La siguiente etiqueta libre de `serie`, mirando las que ya hay.
///
/// **Se mira el dibujo, no un contador.** Un contador se desincroniza en cuanto
/// se borra un punto —el siguiente repetiria una letra que ya esta— y, sobre
/// todo, no sabria nada de los puntos que llegan al abrir un fichero del movil.
pub fn siguiente_etiqueta(usadas: &[String], serie: SerieDePunto) -> String {
    let ocupadas: Vec<&str> = usadas
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let mut i = 0u32;
    loop {
        let propuesta = etiqueta_numero(i, serie);
        if !ocupadas.iter().any(|o| *o == propuesta) {
            return propuesta;
        }
        i += 1;
        // Un tope por si alguien pega diez mil puntos: mejor repetir que
        // colgarse.
        if i > 100_000 {
            return etiqueta_numero(i, serie);
        }
    }
}

/// Las etiquetas que ya estan puestas en la escena.
pub fn etiquetas_usadas(elementos: &[Elemento]) -> Vec<String> {
    elementos
        .iter()
        .filter(|e| !e.borrado)
        .filter_map(|e| match &e.figura {
            Figura::Punto { letra, .. } if !letra.is_empty() => Some(letra.clone()),
            _ => None,
        })
        .collect()
}

/// Hacia donde poner la letra para que no tape nada.
///
/// Se miran las direcciones en las que **sale algo** del punto —los tramos de
/// las figuras que pasan por ahi— y la letra se pone en medio del hueco mas
/// ancho que queda entre ellas. Es exactamente lo que hace la mano sin
/// pensarlo: en un vertice de un triangulo la letra va por fuera; en el punto
/// medio de un lado, perpendicular a el; y en un punto suelto, donde sea, que
/// todo esta libre.
///
/// Sin esto habria que elegir un lado fijo, y un lado fijo acierta la mitad de
/// las veces: en la mitad de los vertices de cualquier figura, «arriba a la
/// derecha» cae justo encima de una raya.
///
/// Devuelve radianes, con la Y hacia abajo como en toda la escena.
pub fn angulo_libre(punto: Punto2, elementos: &[Elemento], alcance: f32) -> f32 {
    let mut salidas = direcciones_que_salen(punto, elementos, alcance);
    // Nada alrededor: arriba a la derecha, que es donde la pone todo el mundo.
    if salidas.is_empty() {
        return -std::f32::consts::FRAC_PI_4;
    }
    salidas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut mejor_hueco = -1.0;
    let mut mejor_angulo = -std::f32::consts::FRAC_PI_4;
    for i in 0..salidas.len() {
        let desde = salidas[i];
        // El hueco entre esta direccion y la siguiente, dando la vuelta al
        // final.
        let hasta = if i + 1 < salidas.len() {
            salidas[i + 1]
        } else {
            salidas[0] + std::f32::consts::TAU
        };
        let hueco = hasta - desde;
        if hueco > mejor_hueco {
            mejor_hueco = hueco;
            mejor_angulo = desde + hueco / 2.0;
        }
    }
    normalizado(mejor_angulo)
}

/// En que direcciones sale algo del punto.
///
/// Se recorren los tramos de todas las figuras y se toma la direccion de los
/// que **tocan** el punto. Un tramo que solo pasa cerca sin tocarlo no estorba
/// a la letra, asi que no cuenta.
fn direcciones_que_salen(punto: Punto2, elementos: &[Elemento], alcance: f32) -> Vec<f32> {
    let mut salidas = Vec::new();
    for e in elementos {
        if e.borrado || matches!(e.figura, Figura::Punto { .. }) {
            continue;
        }
        // La caja primero: montar los tramos de un plano entero para tirarlos
        // despues es el gasto que este filtro evita.
        let (x0, y0, x1, y1) = e.caja();
        if punto.x < x0 - alcance
            || punto.x > x1 + alcance
            || punto.y < y0 - alcance
            || punto.y > y1 + alcance
        {
            continue;
        }
        for (a, b) in segmentos_de(e, PASO_PERIMETRO) {
            let en_a = a.distancia(punto) <= alcance;
            let en_b = b.distancia(punto) <= alcance;
            if en_a {
                salidas.push(normalizado((b.y - punto.y).atan2(b.x - punto.x)));
            }
            if en_b {
                salidas.push(normalizado((a.y - punto.y).atan2(a.x - punto.x)));
            }
            // **Un tramo que pasa POR EL punto sin acabar en el sale hacia los
            // dos lados**, y esto es lo unico que aqui no es igual al movil.
            //
            // Alli solo se miraban los extremos de cada tramo, asi que en el
            // punto medio de una recta larga no se detectaba nada y la letra se
            // iba al sitio de fabrica —arriba a la derecha— que en una recta
            // que sube es justo encima de ella. Su propio comentario decia que
            // ahi la letra tenia que salir perpendicular; el codigo no lo
            // hacia. Aqui si, y «M es el punto medio de AB» se lee.
            if !en_a && !en_b && distancia_a_segmento(punto, a, b) <= alcance {
                salidas.push(normalizado((b.y - punto.y).atan2(b.x - punto.x)));
                salidas.push(normalizado((a.y - punto.y).atan2(a.x - punto.x)));
            }
        }
    }
    salidas
}

/// Un angulo llevado al intervalo `[0, 2pi)`.
fn normalizado(a: f32) -> f32 {
    let mut v = a % std::f32::consts::TAU;
    if v < 0.0 {
        v += std::f32::consts::TAU;
    }
    v
}

/// **Un punto nuevo, ya colocado y ya etiquetado.**
///
/// Se le da todo hecho: la letra que toca y el hueco donde ponerla. Plantar un
/// punto tiene que ser un clic, no un clic mas un menu mas arrastrar la letra.
///
/// `plantilla` aporta el color, el grosor y la semilla, como en cualquier
/// figura nueva; el id lo pone la escena.
pub fn nuevo_punto(
    donde: Punto2,
    elementos: &[Elemento],
    serie: SerieDePunto,
    plantilla: &Elemento,
) -> Elemento {
    Elemento {
        id: 0,
        figura: Figura::Punto {
            letra: siguiente_etiqueta(&etiquetas_usadas(elementos), serie),
            angulo: angulo_libre(donde, elementos, ALCANCE_DE_LA_LETRA),
            radio: RADIO_DE_LA_LETRA,
        },
        // **La caja es el propio punto: sin tamaño.** Lo que se ve —el redondel
        // y la letra— se dibuja alrededor y no depende de la caja, igual que el
        // rotulo de una cota. Asi arrastrarlo mueve el punto y no lo estira.
        x: donde.x,
        y: donde.y,
        ancho: 0.0,
        alto: 0.0,
        angulo: 0.0,
        relleno: None,
        ..plantilla.clone()
    }
}

/// **¿Se puede poner un punto ahi?** El sitio al que se pega, o `None`.
///
/// Un punto de geometria no va donde caiga el cursor. Va donde hay algo que
/// nombrar: un cruce, el final de una recta, su punto medio, el centro de una
/// circunferencia. Poner uno «mas o menos» en un vertice no es un descuido
/// estetico — es que el punto deja de ser ese punto, y todo lo que se deduzca
/// de el a partir de ahi es falso.
///
/// Asi que la herramienta **no planta puntos libres: se pega a un sitio notable
/// o no hace nada**. Que es tambien lo que evita llenar el dibujo de puntos
/// sueltos a dos pixeles de donde iban.
///
/// El orden es el del movil: **la interseccion primero, siempre**, porque es lo
/// que mas se busca y porque no pertenece a ninguna figura —nace de dos—. Se
/// resuelve aqui con `perimetros::intersecciones_cerca` en vez de esperar a que
/// el iman aprenda ese anclaje: es el cimiento comun de los dos, y asi esta
/// herramienta no depende de otro grupo para hacer lo suyo.
pub fn sitio_para_punto(elementos: &[Elemento], p: Punto2, radio: f32) -> Option<Punto2> {
    let cruces = intersecciones_cerca(elementos, p, radio, None, true);
    if let Some((q, _)) = cruces.into_iter().min_by(|(a, _), (b, _)| {
        a.distancia(p)
            .partial_cmp(&b.distancia(p))
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        return Some(q);
    }
    // Sin cruce, los sitios propios de cada figura. Se buscan con el mismo
    // criterio que `sitio_valido_para_punto` decide: ver su tabla.
    let mut mejor: Option<(f32, Punto2)> = None;
    for e in elementos {
        if e.borrado {
            continue;
        }
        for (q, tipo) in sitios_notables(e) {
            if !vale_el_sitio(e, tipo) {
                continue;
            }
            let d = q.distancia(p);
            if d <= radio && mejor.is_none_or(|(md, _)| d < md) {
                mejor = Some((d, q));
            }
        }
    }
    mejor.map(|(_, q)| q)
}

/// ¿Vale este anclaje del iman para plantar un punto?
///
/// Es la misma tabla, contada desde el otro lado: la usa quien ya tiene un
/// [`Anclaje`] en la mano (el gesto, cuando el iman aprenda la interseccion) en
/// vez de volver a buscar.
///
/// | Donde | Que vale |
/// |---|---|
/// | Cualquier figura con cualquier otra | El **cruce**: siempre |
/// | Rectas, flechas, cotas y trazos a mano | Sus **extremos**, vertices y **puntos medios** |
/// | Circunferencias y arcos | Solo el **centro** |
/// | Lo demas | Nada suyo: solo los cruces que tenga con otras |
///
/// Las esquinas de un rectangulo o de un rombo se quedan fuera **a proposito**:
/// son vertices de una caja, no puntos de una construccion, y ofrecerlos
/// llenaria de candidatos justo cuando se busca el cruce que hay al lado.
pub fn sitio_valido_para_punto(anclaje: &Anclaje, elementos: &[Elemento]) -> bool {
    // El cruce no pertenece a ninguna figura: nace de dos, y es el que mas
    // falta hace. Vale siempre, sin mirar de quien es.
    if anclaje.tipo == TipoAnclaje::Interseccion {
        return true;
    }
    let Some(dueno) = elementos.iter().find(|e| e.id == anclaje.id) else {
        return false;
    };
    vale_el_sitio(dueno, anclaje.tipo)
}

fn vale_el_sitio(e: &Elemento, tipo: TipoAnclaje) -> bool {
    match &e.figura {
        Figura::Linea { .. }
        | Figura::Flecha { .. }
        | Figura::Cota { .. }
        | Figura::Lapiz { .. } => matches!(
            tipo,
            TipoAnclaje::Extremo | TipoAnclaje::Esquina | TipoAnclaje::Medio
        ),
        Figura::Elipse | Figura::Arco { .. } => tipo == TipoAnclaje::Centro,
        _ => false,
    }
}

/// Los sitios notables propios de una figura, sin pasar por el iman.
fn sitios_notables(e: &Elemento) -> Vec<(Punto2, TipoAnclaje)> {
    let mut salida = Vec::new();
    match &e.figura {
        Figura::Elipse | Figura::Arco { .. } => {
            let (x0, y0, x1, y1) = e.caja();
            salida.push((
                Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0),
                TipoAnclaje::Centro,
            ));
        }
        _ => {
            let Some(puntos) = e.puntos() else {
                return salida;
            };
            if puntos.len() < 2 {
                return salida;
            }
            let (x0, y0, x1, y1) = e.caja();
            let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let al_mundo = |q: Punto2| {
                if e.angulo == 0.0 {
                    q
                } else {
                    q.girar(centro, e.angulo)
                }
            };
            salida.push((al_mundo(puntos[0]), TipoAnclaje::Extremo));
            salida.push((al_mundo(puntos[puntos.len() - 1]), TipoAnclaje::Extremo));
            for q in &puntos[1..puntos.len().saturating_sub(1)] {
                salida.push((al_mundo(*q), TipoAnclaje::Esquina));
            }
            // El medio del recorrido: en una recta es su punto medio, que es de
            // lo que mas se nombra en un croquis («M es el punto medio de AB»).
            salida.push((
                al_mundo(Punto2::nuevo(
                    (puntos[0].x + puntos[puntos.len() - 1].x) / 2.0,
                    (puntos[0].y + puntos[puntos.len() - 1].y) / 2.0,
                )),
                TipoAnclaje::Medio,
            ));
        }
    }
    salida
}

/// Donde cae el centro de la letra de `e`.
pub fn sitio_de_la_etiqueta(e: &Elemento) -> Punto2 {
    let Figura::Punto { angulo, radio, .. } = &e.figura else {
        return Punto2::nuevo(e.x, e.y);
    };
    Punto2::nuevo(e.x + radio * angulo.cos(), e.y + radio * angulo.sin())
}

/// Mueve la letra hacia `hacia`, **sin separarla de su punto**.
///
/// Da vueltas alrededor y el radio se puede estirar un poco, pero no soltarse:
/// una etiqueta a tres centimetros de su punto ya no dice de quien es, y en un
/// dibujo con doce puntos eso convierte el croquis en un jeroglifico.
pub fn con_la_etiqueta_hacia(e: &mut Elemento, hacia: Punto2) {
    let (dx, dy) = (hacia.x - e.x, hacia.y - e.y);
    let d = (dx * dx + dy * dy).sqrt();
    if d < 0.001 {
        return;
    }
    if let Figura::Punto { angulo, radio, .. } = &mut e.figura {
        *angulo = dy.atan2(dx);
        *radio = d.clamp(RADIO_MINIMO, RADIO_MAXIMO);
    }
    e.tocar();
}

/// ¿El cursor ha caido sobre la letra de `e`?
pub fn toca_la_etiqueta(e: &Elemento, p: Punto2, radio: f32) -> bool {
    if !matches!(e.figura, Figura::Punto { .. }) {
        return false;
    }
    let centro = sitio_de_la_etiqueta(e);
    // El tamaño de la letra sale del grosor, igual que en `pintado`: no hay un
    // `fontSize` propio en este elemento.
    let tam = (e.grosor * 5.0).max(10.0);
    (p.x - centro.x).abs() <= tam * 0.7 + radio && (p.y - centro.y).abs() <= tam * 0.7 + radio
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::ColorRgba;

    fn raya(id: u64, a: (f32, f32), b: (f32, f32)) -> Elemento {
        Elemento {
            id,
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
            },
            x: a.0.min(b.0),
            y: a.1.min(b.1),
            ancho: (b.0 - a.0).abs(),
            alto: (b.1 - a.1).abs(),
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 2.0,
            ..Default::default()
        }
    }

    fn punto(letra: &str, x: f32, y: f32) -> Elemento {
        Elemento {
            id: 99,
            figura: Figura::Punto {
                letra: letra.into(),
                angulo: 0.0,
                radio: RADIO_DE_LA_LETRA,
            },
            x,
            y,
            grosor: 2.0,
            ..Default::default()
        }
    }

    #[test]
    fn las_letras_van_en_serie_y_pasada_la_z_siguen_con_a1() {
        assert_eq!(etiqueta_numero(0, SerieDePunto::Mayusculas), "A");
        assert_eq!(etiqueta_numero(25, SerieDePunto::Mayusculas), "Z");
        assert_eq!(etiqueta_numero(26, SerieDePunto::Mayusculas), "A1");
        assert_eq!(etiqueta_numero(1, SerieDePunto::Minusculas), "b");
        assert_eq!(etiqueta_numero(0, SerieDePunto::Numeros), "1");
    }

    #[test]
    fn la_siguiente_letra_mira_el_dibujo_y_no_un_contador() {
        // Borrar la B tiene que dejar la B libre otra vez: un contador daria D
        // y el croquis se quedaria sin su B para siempre.
        let escena = vec![punto("A", 0.0, 0.0), punto("C", 10.0, 0.0)];
        assert_eq!(
            siguiente_etiqueta(&etiquetas_usadas(&escena), SerieDePunto::Mayusculas),
            "B"
        );
    }

    #[test]
    fn una_letra_repetida_invalidaria_el_croquis_y_por_eso_nunca_se_propone() {
        // Caso negativo: con veintiseis puestas, la siguiente NO puede ser una
        // de ellas.
        let escena: Vec<Elemento> = (0..26)
            .map(|i| punto(&etiqueta_numero(i, SerieDePunto::Mayusculas), i as f32, 0.0))
            .collect();
        let usadas = etiquetas_usadas(&escena);
        let siguiente = siguiente_etiqueta(&usadas, SerieDePunto::Mayusculas);
        assert_eq!(siguiente, "A1");
        assert!(!usadas.contains(&siguiente));
    }

    #[test]
    fn la_letra_se_va_al_hueco_y_no_encima_de_la_raya() {
        // En el punto medio de una recta horizontal la raya sale a los dos
        // lados: la letra tiene que irse arriba o abajo, nunca a un lado.
        let escena = vec![raya(1, (0.0, 0.0), (200.0, 0.0))];
        let a = angulo_libre(Punto2::nuevo(100.0, 0.0), &escena, ALCANCE_DE_LA_LETRA);
        let (sx, sy) = (a.cos(), a.sin());
        assert!(
            sx.abs() < 0.3 && sy.abs() > 0.9,
            "la letra se fue hacia la raya: angulo {a}, direccion ({sx}, {sy})"
        );
    }

    #[test]
    fn en_un_vertice_la_letra_se_va_por_fuera_de_la_esquina() {
        // En un vertice de un triangulo la letra va por fuera: es lo que hace
        // la mano sin pensarlo, y un lado fijo acertaria la mitad de las veces.
        let esquina = vec![
            raya(1, (0.0, 0.0), (100.0, 0.0)),
            raya(2, (0.0, 0.0), (0.0, 100.0)),
        ];
        let a = angulo_libre(Punto2::nuevo(0.0, 0.0), &esquina, ALCANCE_DE_LA_LETRA);
        let (sx, sy) = (a.cos(), a.sin());
        assert!(
            sx < 0.0 && sy < 0.0,
            "la letra cayo dentro de la esquina: angulo {a}, direccion ({sx}, {sy})"
        );
    }

    #[test]
    fn en_un_punto_suelto_la_letra_va_arriba_a_la_derecha() {
        // Sin nada alrededor todo esta libre, y donde la pone todo el mundo es
        // arriba a la derecha.
        let a = angulo_libre(Punto2::nuevo(0.0, 0.0), &[], ALCANCE_DE_LA_LETRA);
        assert_eq!(a, -std::f32::consts::FRAC_PI_4);
    }

    #[test]
    fn el_punto_se_pega_al_cruce_de_dos_rectas() {
        // El cruce es lo que mas se busca y no pertenece a ninguna de las dos
        // figuras: nace de las dos.
        let escena = vec![
            raya(1, (0.0, 50.0), (200.0, 50.0)),
            raya(2, (100.0, 0.0), (100.0, 100.0)),
        ];
        let q = sitio_para_punto(&escena, Punto2::nuevo(104.0, 47.0), RADIO_PARA_PUNTOS)
            .expect("hay un cruce ahi al lado");
        assert!(
            q.distancia(Punto2::nuevo(100.0, 50.0)) < 0.5,
            "no se pego al cruce: {q:?}"
        );
    }

    #[test]
    fn el_punto_se_pega_al_centro_de_una_circunferencia_y_no_a_su_borde() {
        // De una circunferencia lo unico que se nombra es su centro: ofrecer
        // su borde llenaria de candidatos justo donde se busca otra cosa.
        let circulo = Elemento {
            id: 1,
            figura: Figura::Elipse,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 2.0,
            ..Default::default()
        };
        let q = sitio_para_punto(&[circulo], Punto2::nuevo(55.0, 52.0), RADIO_PARA_PUNTOS)
            .expect("el centro esta a menos de un radio");
        assert_eq!(q, Punto2::nuevo(50.0, 50.0));
    }

    #[test]
    fn en_medio_de_la_nada_no_se_planta_ningun_punto() {
        // **Caso negativo, y el que define la herramienta.** Un punto «mas o
        // menos» en un vertice deja de ser ese punto, y todo lo que se deduzca
        // de el a partir de ahi es falso: mas vale no plantarlo.
        let escena = vec![raya(1, (0.0, 0.0), (100.0, 0.0))];
        assert!(
            sitio_para_punto(&escena, Punto2::nuevo(400.0, 400.0), RADIO_PARA_PUNTOS).is_none()
        );
    }

    #[test]
    fn las_esquinas_de_un_rectangulo_no_valen_para_plantar_un_punto() {
        // Caso negativo con intencion: son vertices de una caja, no puntos de
        // una construccion. Sin otra figura que las cruce, no hay sitio.
        let caja = Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 2.0,
            ..Default::default()
        };
        assert!(sitio_para_punto(&[caja], Punto2::nuevo(1.0, 1.0), RADIO_PARA_PUNTOS).is_none());
    }

    #[test]
    fn la_letra_orbita_su_punto_y_no_se_suelta_de_el() {
        // Una etiqueta a tres centimetros ya no dice de quien es: el radio se
        // estira un poco y se para.
        let mut e = punto("A", 100.0, 100.0);
        con_la_etiqueta_hacia(&mut e, Punto2::nuevo(1000.0, 100.0));
        let Figura::Punto { radio, angulo, .. } = e.figura else {
            unreachable!()
        };
        assert_eq!(radio, RADIO_MAXIMO, "la letra se solto de su punto");
        assert_eq!(angulo, 0.0);
        // Y al mover el punto, la letra lo sigue: es toda la gracia de
        // guardarla en polares.
        let antes = sitio_de_la_etiqueta(&e);
        e.mover(50.0, 0.0);
        assert_eq!(
            sitio_de_la_etiqueta(&e),
            Punto2::nuevo(antes.x + 50.0, antes.y)
        );
    }

    #[test]
    fn se_agarra_la_letra_por_mas_sitio_del_que_ocupa() {
        // Lo que se dibuja es para el ojo y lo que se toca es para la mano.
        let mut e = punto("A", 0.0, 0.0);
        con_la_etiqueta_hacia(&mut e, Punto2::nuevo(0.0, -22.0));
        assert!(toca_la_etiqueta(
            &e,
            Punto2::nuevo(4.0, -20.0),
            RADIO_DE_AGARRE
        ));
        // Caso negativo: lejos de la letra no se agarra, o arrastrar el punto
        // moveria siempre la etiqueta.
        assert!(!toca_la_etiqueta(
            &e,
            Punto2::nuevo(0.0, 120.0),
            RADIO_DE_AGARRE
        ));
    }

    #[test]
    fn el_punto_nuevo_nace_sin_caja_para_que_arrastrarlo_no_lo_estire() {
        let plantilla = raya(1, (0.0, 0.0), (1.0, 0.0));
        let e = nuevo_punto(
            Punto2::nuevo(30.0, 40.0),
            &[],
            SerieDePunto::Mayusculas,
            &plantilla,
        );
        assert_eq!((e.ancho, e.alto), (0.0, 0.0));
        assert_eq!((e.x, e.y), (30.0, 40.0));
        let Figura::Punto { letra, radio, .. } = &e.figura else {
            panic!("no nacio como punto");
        };
        assert_eq!(letra, "A");
        assert_eq!(*radio, RADIO_DE_LA_LETRA);
    }
}
