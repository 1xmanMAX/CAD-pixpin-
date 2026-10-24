//! **Los angulos que se forman donde dos rayas se juntan.**
//!
//! Port de `Angulos.kt` (145 lineas) del PixPin de Android.
//!
//! **Solo mientras se mueve algo.** Un dibujo con todos sus angulos escritos es
//! ilegible, y ademas nadie los mira cuando no esta tocando nada; pero mientras
//! se arrastra un vertice, el angulo es exactamente lo unico que se quiere
//! saber y lo unico que no se puede ver. Aparece al empezar el gesto y se va al
//! soltar, como la guia de un nivel de burbuja.
//!
//! Por eso **no crea ningun elemento y no se serializa nunca**: es un rotulo en
//! vivo, no una cota. Quien quiera dejar el angulo escrito en el papel tiene la
//! cota, que si se guarda.
//!
//! Se miden **entre las dos rayas que salen del vertice**, no respecto de la
//! horizontal: lo que importa de una esquina es si es recta, no como esta
//! inclinada la figura entera.
//!
//! Geometria pura y sin estado: quien forma angulo con quien se decide aqui, y
//! se comprueba sin pantalla.

use std::collections::HashMap;

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use crate::pintado::Orden;
use crate::vector::Punto2;

/// Un angulo que enseñar mientras dura el gesto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnguloInterno {
    pub vertice: Punto2,
    /// Cuanto mide, de 0 a 180.
    pub grados: f32,
    /// Hacia donde cae su bisectriz, en radianes.
    ///
    /// Es donde hay que escribir el numero: **dentro** del angulo, que es donde
    /// lo pone cualquiera a mano. Fuera se lee como si fuera del angulo de al
    /// lado.
    pub bisectriz: f32,
}

/// A cuanto se dan por juntas dos puntas, en pixeles del documento.
///
/// Es generoso a proposito: dos rayas que se ven unidas casi nunca comparten el
/// punto exacto, y un angulo que solo apareciera cuando las puntas coinciden al
/// pixel no apareceria nunca.
pub const JUNTA: f32 = 6.0;

/// Cuantos se enseñan a la vez.
///
/// Cuatro son los de un cuadrilatero: pasado eso, lo que se ve es una nube de
/// cifras y no un dibujo. Si hubiera mas, los que se quedan son los mas
/// cerrados — un angulo de 170 grados no lo esta mirando nadie.
const MAXIMOS: usize = 4;

/// Las figuras que tienen puntas con las que formar esquina.
fn es_raya(e: &Elemento) -> bool {
    matches!(
        e.figura,
        Figura::Linea { .. } | Figura::Flecha { .. } | Figura::Cota { .. } | Figura::Lapiz { .. }
    )
}

/// **Los angulos que forman las rayas de `elementos` en sus juntas.**
///
/// Una junta es un sitio donde acaban dos rayas de figuras distintas. Se enseña
/// solo lo que toca a `moviendose` — con todo el dibujo lleno de numeros no se
/// ve el dibujo.
pub fn angulos_internos(
    elementos: &[Elemento],
    moviendose: &[u64],
    tolerancia: f32,
) -> Vec<AnguloInterno> {
    if moviendose.is_empty() {
        return Vec::new();
    }
    // Cada punta, con hacia donde se va la raya desde ella.
    let mut puntas: Vec<(u64, Punto2, f32)> = Vec::new();
    for e in elementos.iter().filter(|e| !e.borrado && es_raya(e)) {
        let Some(puntos) = e.puntos() else { continue };
        if puntos.len() < 2 {
            continue;
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
        let n = puntos.len();
        let primero = al_mundo(puntos[0]);
        let segundo = al_mundo(puntos[1]);
        let ultimo = al_mundo(puntos[n - 1]);
        let penultimo = al_mundo(puntos[n - 2]);
        puntas.push((e.id, primero, direccion(primero, segundo)));
        puntas.push((e.id, ultimo, direccion(ultimo, penultimo)));
    }
    if puntas.len() < 4 {
        return Vec::new();
    }

    let mut todos: Vec<AnguloInterno> = Vec::new();
    for i in 0..puntas.len() {
        for j in (i + 1)..puntas.len() {
            let (id_a, donde_a, hacia_a) = puntas[i];
            let (id_b, donde_b, hacia_b) = puntas[j];
            // Las dos puntas de la MISMA raya no forman esquina entre si:
            // forman la raya.
            if id_a == id_b {
                continue;
            }
            if !moviendose.contains(&id_a) && !moviendose.contains(&id_b) {
                continue;
            }
            if donde_a.distancia(donde_b) > tolerancia {
                continue;
            }
            let grados = normalizar_rad(hacia_b - hacia_a).abs().to_degrees();
            // Dos rayas en la misma direccion no forman esquina: forman una
            // raya mas larga, y un «180» ahi es ruido.
            if !(0.5..=179.5).contains(&grados) {
                continue;
            }
            todos.push(AnguloInterno {
                vertice: donde_a,
                grados,
                bisectriz: bisectriz_de(hacia_a, hacia_b),
            });
        }
    }

    // **Uno por junta, y pocos.**
    //
    // Enseñarlos todos era peor que no enseñar ninguno: en una esquina donde se
    // juntan tres rayas salen tres numeros amontonados encima del vertice, y en
    // una figura cerrada salen tantos que tapan el propio dibujo. Lo que hace
    // falta mientras se mueve algo es **el angulo de la esquina que se esta
    // tocando**, asi que se deja uno por sitio —el mas cerrado, que es el que
    // se esta intentando ajustar— y como mucho un puñado.
    let mut por_junta: HashMap<(i64, i64), AnguloInterno> = HashMap::new();
    for a in todos {
        let llave = (
            (a.vertice.x / JUNTA).round() as i64,
            (a.vertice.y / JUNTA).round() as i64,
        );
        por_junta
            .entry(llave)
            .and_modify(|v| {
                if a.grados < v.grados {
                    *v = a;
                }
            })
            .or_insert(a);
    }
    let mut salida: Vec<AnguloInterno> = por_junta.into_values().collect();
    salida.sort_by(|a, b| {
        a.grados
            .partial_cmp(&b.grados)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    salida.truncate(MAXIMOS);
    salida
}

/// Hacia donde va la raya que sale de `desde` pasando por `hacia`.
fn direccion(desde: Punto2, hacia: Punto2) -> f32 {
    (hacia.y - desde.y).atan2(hacia.x - desde.x)
}

/// El angulo, llevado a `(-pi, pi]`.
fn normalizar_rad(a: f32) -> f32 {
    let mut d = a % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    if d <= -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

/// La bisectriz de dos direcciones, **por el lado corto**.
///
/// Sumar y dividir entre dos parece lo natural y falla justo donde mas se nota:
/// con una direccion a 170 grados y otra a -170 da 0, que apunta al lado
/// contrario del angulo. Yendo por la diferencia corta sale siempre dentro.
fn bisectriz_de(a: f32, b: f32) -> f32 {
    normalizar_rad(a + normalizar_rad(b - a) / 2.0)
}

/// Cuanto se ve del arquito, en pixeles de PANTALLA.
///
/// De pantalla y no del documento: el rotulo tiene que leerse igual al zoom que
/// sea, y un arco de radio fijo en documento se hace invisible al alejarse y
/// tapa la esquina al acercarse.
const RADIO_ARCO_PX: f32 = 26.0;

/// El tamaño de la cifra, tambien en pixeles de pantalla.
const TAM_CIFRA_PX: f32 = 13.0;

/// **El arquito y la cifra de cada angulo, listos para pintar.**
///
/// Vive aqui y no en `pintado` porque esto **no es un elemento**: no esta en la
/// escena, no se guarda y solo existe mientras dura el gesto. Quien pinta lo
/// encadena detras de las ordenes de la escena, como hace con la pista del
/// iman.
pub fn ordenes_de_angulos(angulos: &[AnguloInterno], zoom: f32, color: ColorRgba) -> Vec<Orden> {
    let radio = RADIO_ARCO_PX / zoom.max(0.0001);
    let tam = TAM_CIFRA_PX / zoom.max(0.0001);
    let mut salida = Vec::with_capacity(angulos.len() * 2);
    for a in angulos {
        let medio = a.grados.to_radians() / 2.0;
        let desde = a.bisectriz - medio;
        // Ocho tramos bastan para que un arquito de veintipocos pixeles no se
        // lea como un poligono, y no hay mas que dibujar.
        let puntos: Vec<Punto2> = (0..=8)
            .map(|i| {
                let t = desde + a.grados.to_radians() * i as f32 / 8.0;
                Punto2::nuevo(a.vertice.x + radio * t.cos(), a.vertice.y + radio * t.sin())
            })
            .collect();
        salida.push(Orden::Polilinea {
            puntos,
            color,
            grosor: 1.0 / zoom.max(0.0001),
            estilo: EstiloTrazo::Solido,
        });
        // La cifra va un poco mas afuera que el arco, sobre la bisectriz:
        // encima del arco taparia justo la curva que dice cual es el angulo.
        let texto = format!("{}°", a.grados.round() as i32);
        let ancho = tam * 0.6 * texto.chars().count() as f32;
        salida.push(Orden::Texto {
            x: a.vertice.x + (radio + tam * 0.6) * a.bisectriz.cos() - ancho / 2.0,
            y: a.vertice.y + (radio + tam * 0.6) * a.bisectriz.sin() - tam / 2.0,
            texto,
            tam,
            familia: "Segoe UI".to_string(),
            color,
            ancho_max: ancho.max(1.0),
            negrita: false,
            cursiva: false,
        });
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

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
            grosor: 1.0,
            ..Default::default()
        }
    }

    /// Una esquina recta: una raya hacia la derecha y otra hacia abajo, las dos
    /// saliendo del origen.
    fn esquina_recta() -> Vec<Elemento> {
        vec![
            raya(1, (0.0, 0.0), (100.0, 0.0)),
            raya(2, (0.0, 0.0), (0.0, 100.0)),
        ]
    }

    #[test]
    fn una_esquina_recta_mide_noventa_grados() {
        let a = angulos_internos(&esquina_recta(), &[1], JUNTA);
        assert_eq!(a.len(), 1);
        assert!((a[0].grados - 90.0).abs() < 0.01, "midio {}", a[0].grados);
        assert_eq!(a[0].vertice, Punto2::nuevo(0.0, 0.0));
        // La bisectriz apunta DENTRO del angulo: hacia abajo y a la derecha.
        assert!(a[0].bisectriz.cos() > 0.0 && a[0].bisectriz.sin() > 0.0);
    }

    #[test]
    fn sin_mover_nada_no_se_enseña_ningun_angulo() {
        // **Caso negativo y razon de ser de la herramienta**: un dibujo con
        // todos sus angulos escritos es ilegible, y nadie los mira cuando no
        // esta tocando nada.
        assert!(angulos_internos(&esquina_recta(), &[], JUNTA).is_empty());
    }

    #[test]
    fn dos_rayas_que_no_se_tocan_no_forman_esquina() {
        // Caso negativo: sin junta no hay angulo, por muy cerca que pasen.
        let sueltas = vec![
            raya(1, (0.0, 0.0), (100.0, 0.0)),
            raya(2, (0.0, 50.0), (0.0, 150.0)),
        ];
        assert!(angulos_internos(&sueltas, &[1], JUNTA).is_empty());
    }

    #[test]
    fn dos_rayas_seguidas_en_linea_recta_no_dan_un_angulo_de_ciento_ochenta() {
        // Forman una raya mas larga, y un «180» ahi es ruido.
        let seguidas = vec![
            raya(1, (0.0, 0.0), (100.0, 0.0)),
            raya(2, (100.0, 0.0), (200.0, 0.0)),
        ];
        assert!(angulos_internos(&seguidas, &[1], JUNTA).is_empty());
    }

    #[test]
    fn una_junta_a_cuatro_pixeles_cuenta_como_junta() {
        // Dos rayas que se ven unidas casi nunca comparten el punto exacto: un
        // angulo que solo apareciera al pixel no apareceria nunca.
        let casi = vec![
            raya(1, (0.0, 0.0), (100.0, 0.0)),
            raya(2, (4.0, 0.0), (4.0, 100.0)),
        ];
        assert_eq!(angulos_internos(&casi, &[2], JUNTA).len(), 1);
        // Caso negativo: a veinte pixeles ya no estan juntas, y un angulo ahi
        // seria una esquina inventada.
        let lejos = vec![
            raya(1, (0.0, 0.0), (100.0, 0.0)),
            raya(2, (20.0, 0.0), (20.0, 100.0)),
        ];
        assert!(angulos_internos(&lejos, &[2], JUNTA).is_empty());
    }

    #[test]
    fn en_una_junta_de_tres_rayas_solo_sale_un_numero_y_es_el_mas_cerrado() {
        // Tres numeros amontonados encima del mismo vertice se leen peor que
        // ninguno; el que interesa es el que se esta intentando ajustar.
        let abanico = vec![
            raya(1, (0.0, 0.0), (100.0, 0.0)),
            raya(2, (0.0, 0.0), (100.0, 100.0)),
            raya(3, (0.0, 0.0), (0.0, 100.0)),
        ];
        let a = angulos_internos(&abanico, &[1, 2, 3], JUNTA);
        assert_eq!(a.len(), 1, "un numero por junta: {a:?}");
        assert!((a[0].grados - 45.0).abs() < 0.01, "midio {}", a[0].grados);
    }

    #[test]
    fn la_bisectriz_cae_dentro_del_angulo_aunque_cruce_la_media_vuelta() {
        // Sumar y dividir entre dos falla justo aqui: con una direccion a 170
        // grados y otra a -170 da 0, que apunta al lado contrario.
        let cruzando = vec![
            raya(1, (0.0, 0.0), (-100.0, 17.0)),
            raya(2, (0.0, 0.0), (-100.0, -17.0)),
        ];
        let a = angulos_internos(&cruzando, &[1], JUNTA);
        assert_eq!(a.len(), 1);
        assert!(
            a[0].bisectriz.cos() < 0.0,
            "la bisectriz apunta al lado contrario: {}",
            a[0].bisectriz
        );
    }

    #[test]
    fn el_rotulo_lleva_su_arquito_y_su_cifra() {
        let a = angulos_internos(&esquina_recta(), &[1], JUNTA);
        let ordenes = ordenes_de_angulos(&a, 1.0, ColorRgba::opaco(0.0, 0.0, 0.0));
        assert_eq!(ordenes.len(), 2);
        assert!(matches!(ordenes[0], Orden::Polilinea { .. }));
        let Orden::Texto { texto, .. } = &ordenes[1] else {
            panic!("falta la cifra");
        };
        assert_eq!(texto, "90°");
    }
}
