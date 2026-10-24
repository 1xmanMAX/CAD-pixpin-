//! **Marcadores con emoticono en el lienzo** (F5). Puerto de
//! `motor/Marcas.kt` del movil (v0.81-v0.81.2).
//!
//! En un documento basta con la altura (`pixpin_docs::lectura`); en un
//! lienzo no: uno marca **un sitio** de un plano infinito, y ademas quiere
//! poder moverlo de ahi. Asi que una marca es un punto:
//!
//! - **Lienzo 2D**: `x` e `y` son coordenadas del mundo, las mismas en las que
//!   vive lo dibujado. Se arrastra con la mano y se queda donde se suelte.
//! - **Lector de PDF** (E4, todavia sin hacer en el PC): `x` es la fraccion a
//!   lo ancho de la hoja e `y` es **la pagina mas la fraccion a lo alto**
//!   (2,5 = la mitad de la tercera hoja). Van aqui [`pagina_de`] y compania
//!   para que el lector las use tal cual cuando llegue.
//!
//! # Donde se guardan, y por que asi
//!
//! El movil las guarda **fuera del dibujo**: en sus ajustes
//! (`SharedPreferences` «marcas», clave `lienzo:<id del dibujo>`), en UNA
//! linea `id:x:y:emoji|…`. No van en el `.excalidraw`: son de quien lee, no
//! del dibujo, y no se exportan ni se imprimen. Aqui se usa exactamente la
//! misma linea ([`a_texto`] / [`de_texto`]) para que el dia que viajen el
//! formato ya sea uno; el sitio es un fichero hermano del dibujo (lo decide
//! la aplicacion, no el motor).
//!
//! Los tipos siguen a los del movil: `id` es un `Long` (milisegundos) y las
//! coordenadas son `Double`. Guardar `f32` aqui y escribirlo con otra
//! precision haria que el mismo texto no diera la misma lista en los dos
//! lados.

use crate::camara::Camara;
use crate::vector::Punto2;

/// Una marca: un sitio con su emoticono.
#[derive(Debug, Clone, PartialEq)]
pub struct Marca {
    /// Cuando se puso, en milisegundos. Sirve de identidad.
    pub id: i64,
    pub emoji: String,
    pub x: f64,
    pub y: f64,
}

/// Cuantas caben. Las de mas se van, las mas viejas primero, como en el movil.
pub const MAXIMO: usize = 24;

/// Los mismos emoticonos que los lectores (`Lectura.EMOJIS` del movil y
/// `pixpin_docs::lectura::EMOJIS` aqui): es la misma funcion. El motor no
/// depende de `pixpin-docs`, asi que la lista se repite y una prueba de la
/// aplicacion vigila que no se separen.
pub const EMOJIS: [&str; 12] = [
    "🔖", "⭐", "❤️", "❗", "❓", "💡", "📌", "✅", "🔥", "👀", "✏️", "🏁",
];

/// **En el orden en que se leen**: de arriba abajo y, a igualdad, de
/// izquierda a derecha. Es el orden del riel, y sale de donde esta cada marca
/// y no de cuando se puso: mover una marca **es** reordenarlas, y no hay dos
/// sitios que puedan contradecirse.
pub fn en_orden(lista: &mut [Marca]) {
    // `sort_by` es estable, como `sortedWith` de Kotlin: dos marcas en el
    // mismo punto conservan el orden en que venian.
    lista.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });
}

/// Una mas. Si ya no caben, se va la mas vieja: lo que se acaba de poner no
/// se pierde nunca.
pub fn con(lista: &[Marca], x: f64, y: f64, emoji: &str, ahora: i64) -> Vec<Marca> {
    let nueva = Marca {
        id: id_libre(lista, ahora),
        emoji: if emoji.trim().is_empty() {
            EMOJIS[0].to_string()
        } else {
            emoji.to_string()
        },
        x,
        y,
    };
    let mut todas = lista.to_vec();
    todas.push(nueva);
    if todas.len() > MAXIMO {
        todas.sort_by_key(|m| m.id);
        todas.drain(..todas.len() - MAXIMO);
    }
    en_orden(&mut todas);
    todas
}

/// Poner dos en el mismo milisegundo es raro, pero dos marcas con el mismo id
/// serian una.
fn id_libre(lista: &[Marca], ahora: i64) -> i64 {
    let mut id = ahora;
    while lista.iter().any(|m| m.id == id) {
        id += 1;
    }
    id
}

/// Movida a otro sitio. La lista vuelve ordenada, que es lo que cambia el
/// riel.
pub fn movida(lista: &[Marca], id: i64, x: f64, y: f64) -> Vec<Marca> {
    let mut salida: Vec<Marca> = lista
        .iter()
        .map(|m| {
            if m.id == id {
                Marca { x, y, ..m.clone() }
            } else {
                m.clone()
            }
        })
        .collect();
    en_orden(&mut salida);
    salida
}

/// Sin esa marca.
pub fn sin(lista: &[Marca], id: i64) -> Vec<Marca> {
    lista.iter().filter(|m| m.id != id).cloned().collect()
}

/// Guardadas en una linea, como las del movil: `id:x:y:emoji|…`.
pub fn a_texto(lista: &[Marca]) -> String {
    let mut ordenadas = lista.to_vec();
    en_orden(&mut ordenadas);
    ordenadas
        .iter()
        .map(|m| format!("{}:{}:{}:{}", m.id, num(m.x), num(m.y), m.emoji))
        .collect::<Vec<_>>()
        .join("|")
}

/// La linea de vuelta a marcas. Lo que no se entiende se salta: una marca
/// rota no puede llevarse las demas.
pub fn de_texto(texto: &str) -> Vec<Marca> {
    let mut salida: Vec<Marca> = texto
        .split('|')
        .filter_map(|trozo| {
            let p: Vec<&str> = trozo.split(':').collect();
            if p.len() < 4 {
                return None;
            }
            let id = p[0].trim().parse::<i64>().ok()?;
            let x = p[1].trim().parse::<f64>().ok().filter(|v| v.is_finite())?;
            let y = p[2].trim().parse::<f64>().ok().filter(|v| v.is_finite())?;
            // El emoticono puede llevar dos puntos dentro; lo que queda de la
            // linea es suyo.
            let emoji = p[3..].join(":");
            let emoji = if emoji.trim().is_empty() {
                EMOJIS[0].to_string()
            } else {
                emoji
            };
            Some(Marca { id, emoji, x, y })
        })
        .collect();
    en_orden(&mut salida);
    salida
}

/// Un numero como lo escribe el movil: redondeado a centesimas y, si es
/// entero, sin decimales (`12`, no `12.0`). Asi el mismo dibujo da la misma
/// linea en los dos aparatos y un cambio real no se pierde entre ruido.
fn num(v: f64) -> String {
    // `Math.round` de Kotlin redondea las mitades hacia arriba (tambien las
    // negativas), que no es lo que hace `f64::round`: por eso el suelo.
    let r = (v * 100.0 + 0.5).floor() / 100.0;
    if r == r.trunc() && r.abs() < 1e15 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// **Adonde hay que llevar la camara para que la marca quede arriba**: lo
/// que pidio el usuario —«que me lleve directo a ese punto estando el
/// emoticono en la parte superior»—. Centrada a lo ancho y a `margen`
/// pixeles del borde de arriba (por debajo de la barra, que la taparia). El
/// aumento no se toca: se marca un sitio, no un encuadre.
///
/// `ancho` y `margen` van en las mismas unidades que `zoom` convierte:
/// pixeles logicos si `zoom` es el de la camara del usuario.
pub fn camara_con_la_marca_arriba(m: &Marca, zoom: f32, ancho: f32, margen: f32) -> Camara {
    let z = zoom.max(1e-6);
    Camara {
        x: m.x as f32 - ancho / (2.0 * z),
        y: m.y as f32 - margen / z,
        zoom: z,
    }
}

/// Que marca hay bajo un punto de la pantalla, si hay alguna. `radio` es el
/// del redondel que se pinta, en pixeles: solo coge el raton el emoticono, no
/// lo que hay alrededor (lo pidio el usuario: que no estorbe al dibujar).
///
/// Si dos se tapan gana la ultima de la lista, que es la que se pinta encima.
pub fn marca_en(lista: &[Marca], camara: &Camara, pantalla: Punto2, radio: f32) -> Option<i64> {
    lista
        .iter()
        .rev()
        .find(|m| {
            let s = camara.a_pantalla(Punto2::nuevo(m.x as f32, m.y as f32));
            s.distancia(pantalla) <= radio
        })
        .map(|m| m.id)
}

/// De que pagina es una marca del lector de PDF. Ver el modulo.
pub fn pagina_de(m: &Marca) -> u32 {
    m.y.floor().max(0.0) as u32
}

/// A que altura de su pagina cae, de 0 a 1.
pub fn alto_en_la_pagina(m: &Marca) -> f64 {
    (m.y - pagina_de(m) as f64).clamp(0.0, 1.0)
}

/// Como se guarda un sitio del lector de PDF: `(x, y)` con la pagina y en
/// que parte de ella cae.
pub fn en_la_pagina(pagina: i32, fraccion_alto: f64, fraccion_ancho: f64) -> (f64, f64) {
    (
        fraccion_ancho.clamp(0.0, 1.0),
        pagina.max(0) as f64 + fraccion_alto.clamp(0.0, 1.0),
    )
}

/// **El viaje de la camara hasta una marca.** Ir de golpe desorienta: en un
/// lienzo infinito un salto seco no dice ni hacia donde se ha ido. Se
/// persigue el destino como el zoom del pin (`pixpin_pin::zoom`, el de
/// QuickView): lo que falta se multiplica por `exp(-VELOCIDAD·dt)` en cada
/// paso, asi que se ve igual a 60 que a 144 Hz, y pedir otro destino a mitad
/// de camino lo redirige sin tirones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vuelo {
    pub destino: Camara,
}

/// Mas lento que el zoom del pin (72): alli se recorren unos cientos de
/// pixeles y aqui a veces la pantalla entera varias veces. Con 14, un viaje
/// de 2.000 px llega a medio pixel en algo mas de medio segundo; se ve de
/// donde se viene sin hacerse esperar.
pub const VELOCIDAD_VUELO: f32 = 14.0;

/// Por debajo de medio pixel de pantalla ya no se ve nada: se clava.
const LLEGADA_PX: f32 = 0.5;

impl Vuelo {
    pub fn hacia(destino: Camara) -> Self {
        Self { destino }
    }

    /// Avanza `dt` segundos. Devuelve `true` mientras siga en el aire; al
    /// llegar deja la camara exactamente en el destino, porque la exponencial
    /// no llega nunca sola y quedaria un resto de redondeo para siempre.
    pub fn avanzar(&self, camara: &mut Camara, dt: f32) -> bool {
        // Un reloj roto (NaN) o una pausa larga del sistema no pueden dar un
        // salto seco ni envenenar la camara: se acota como en el pin.
        let dt = if dt.is_finite() {
            dt.clamp(1.0 / 240.0, 0.05)
        } else {
            1.0 / 240.0
        };
        let avance = 1.0 - (-VELOCIDAD_VUELO * dt).exp();
        camara.x += (self.destino.x - camara.x) * avance;
        camara.y += (self.destino.y - camara.y) * avance;
        camara.zoom = self.destino.zoom;
        let falta = (self.destino.x - camara.x).hypot(self.destino.y - camara.y) * camara.zoom;
        if falta < LLEGADA_PX {
            *camara = self.destino;
            return false;
        }
        true
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn emojis(l: &[Marca]) -> Vec<&str> {
        l.iter().map(|m| m.emoji.as_str()).collect()
    }

    #[test]
    fn se_ordenan_de_arriba_abajo_y_mover_una_las_reordena() {
        let mut lista = con(&[], 100.0, 500.0, "⭐", 1);
        lista = con(&lista, 10.0, 20.0, "🔖", 2);
        lista = con(&lista, 900.0, 20.0, "📌", 3);
        assert_eq!(emojis(&lista), ["🔖", "📌", "⭐"]);
        // La de abajo sube del todo: el riel la ensena la primera.
        let la_de_abajo = lista.iter().find(|m| m.emoji == "⭐").unwrap().id;
        lista = movida(&lista, la_de_abajo, 0.0, -50.0);
        assert_eq!(emojis(&lista), ["⭐", "🔖", "📌"]);
    }

    #[test]
    fn dos_en_el_mismo_milisegundo_son_dos() {
        let dos = con(&con(&[], 0.0, 0.0, "⭐", 7), 1.0, 1.0, "🔖", 7);
        assert_eq!(dos.len(), 2);
        assert_ne!(dos[0].id, dos[1].id);
    }

    #[test]
    fn pasado_el_tope_se_va_la_mas_vieja_y_se_queda_la_recien_puesta() {
        let mut lista = Vec::new();
        for i in 0..MAXIMO {
            lista = con(&lista, i as f64, i as f64, "🔖", 1000 + i as i64);
        }
        lista = con(&lista, -1.0, -1.0, "🏁", 9999);
        assert_eq!(lista.len(), MAXIMO);
        assert!(lista.iter().any(|m| m.emoji == "🏁"));
        assert!(lista.iter().all(|m| m.id != 1000));
    }

    #[test]
    fn ida_y_vuelta_por_texto_con_emoticonos_que_llevan_dos_puntos() {
        let lista = con(&con(&[], -12.5, 3.25, "⭐", 1), 4.0, 900.125, "a:b", 2);
        let vuelta = de_texto(&a_texto(&lista));
        assert_eq!(emojis(&vuelta), emojis(&lista));
        assert_eq!(
            vuelta.iter().map(|m| m.id).collect::<Vec<_>>(),
            lista.iter().map(|m| m.id).collect::<Vec<_>>()
        );
        assert!((vuelta[0].x - -12.5).abs() < 1e-9);
        assert!((vuelta[0].y - 3.25).abs() < 1e-9);
        // Centesimas, como el movil: 900,125 se queda en 900,13.
        assert!((vuelta[1].y - 900.13).abs() < 1e-9);
    }

    #[test]
    fn la_linea_es_la_misma_que_escribe_el_movil() {
        let lista = vec![
            Marca {
                id: 1_758_000_000_000,
                emoji: "🔖".into(),
                x: 12.0,
                y: -3.5,
            },
            Marca {
                id: 2,
                emoji: "⭐".into(),
                x: 0.004,
                y: 40.0,
            },
        ];
        // Enteros sin «.0» y el redondeo de `Math.round`: es lo que hace que
        // el texto de un aparato lo lea el otro sin diferencias.
        assert_eq!(a_texto(&lista), "1758000000000:12:-3.5:🔖|2:0:40:⭐");
    }

    #[test]
    fn un_texto_vacio_o_roto_no_da_marcas_ni_revienta() {
        assert!(de_texto("").is_empty());
        assert!(de_texto("basura|1:2").is_empty());
        assert!(de_texto("x:1:2:⭐|1:no:2:⭐|1:2:inf:⭐").is_empty());
        // La buena sobrevive a la rota de al lado.
        let una = de_texto("roto|5:1:2:⭐");
        assert_eq!(una.len(), 1);
        assert_eq!(una[0].id, 5);
        // Sin emoticono sale el primero de la tira, no una marca invisible.
        assert_eq!(de_texto("5:1:2:")[0].emoji, EMOJIS[0]);
    }

    #[test]
    fn quitar_una_marca_deja_las_demas_y_quitar_una_que_no_esta_no_hace_nada() {
        let lista = con(&con(&[], 0.0, 0.0, "⭐", 1), 1.0, 1.0, "🔖", 2);
        assert_eq!(emojis(&sin(&lista, 1)), ["🔖"]);
        assert_eq!(sin(&lista, 99), lista);
    }

    #[test]
    fn ir_a_una_marca_la_deja_arriba_y_centrada_a_lo_ancho() {
        let m = Marca {
            id: 1,
            emoji: "⭐".into(),
            x: 250.0,
            y: 400.0,
        };
        let c = camara_con_la_marca_arriba(&m, 2.0, 1000.0, 100.0);
        let en_pantalla = c.a_pantalla(Punto2::nuevo(250.0, 400.0));
        assert!((en_pantalla.x - 500.0).abs() < 1e-3);
        assert!((en_pantalla.y - 100.0).abs() < 1e-3);
        assert_eq!(c.zoom, 2.0);
    }

    #[test]
    fn el_raton_coge_la_marca_solo_dentro_de_su_redondel() {
        let lista = con(&[], 100.0, 100.0, "⭐", 1);
        let c = Camara {
            x: 0.0,
            y: 0.0,
            zoom: 2.0,
        };
        // La marca cae en (200, 200) de pantalla.
        assert_eq!(
            marca_en(&lista, &c, Punto2::nuevo(210.0, 195.0), 18.0),
            Some(1)
        );
        assert_eq!(
            marca_en(&lista, &c, Punto2::nuevo(230.0, 200.0), 18.0),
            None
        );
        assert_eq!(marca_en(&[], &c, Punto2::nuevo(200.0, 200.0), 18.0), None);
    }

    #[test]
    fn el_vuelo_llega_clavado_y_no_se_pasa() {
        let destino = Camara {
            x: 2000.0,
            y: -500.0,
            zoom: 1.5,
        };
        let v = Vuelo::hacia(destino);
        let mut c = Camara {
            x: 0.0,
            y: 0.0,
            zoom: 1.5,
        };
        let mut pasos = 0;
        let mut antes = f32::INFINITY;
        while v.avanzar(&mut c, 1.0 / 60.0) {
            let falta = (destino.x - c.x).hypot(destino.y - c.y);
            assert!(falta < antes, "cada paso acerca");
            assert!(c.x <= destino.x, "nunca se pasa");
            antes = falta;
            pasos += 1;
            assert!(pasos < 120, "a 60 Hz llega en menos de dos segundos");
        }
        assert_eq!(c, destino);
        // Ni instantaneo: se tiene que ver el viaje.
        assert!(pasos > 5, "{pasos} pasos es un salto, no un viaje");
    }

    #[test]
    fn un_reloj_roto_no_envenena_la_camara() {
        let v = Vuelo::hacia(Camara {
            x: 10.0,
            y: 10.0,
            zoom: 1.0,
        });
        let mut c = Camara::nueva();
        v.avanzar(&mut c, f32::NAN);
        assert!(c.x.is_finite() && c.y.is_finite());
        // Una pausa de un segundo no hace llegar de golpe.
        let mut d = Camara::nueva();
        assert!(v.avanzar(&mut d, 1.0));
    }

    #[test]
    fn una_pagina_del_pdf_y_donde_cae_dentro_de_ella() {
        let (x, y) = en_la_pagina(2, 0.5, 0.5);
        assert!((x - 0.5).abs() < 1e-9);
        assert!((y - 2.5).abs() < 1e-9);
        let m = Marca {
            id: 1,
            emoji: "🔖".into(),
            x,
            y,
        };
        assert_eq!(pagina_de(&m), 2);
        assert!((alto_en_la_pagina(&m) - 0.5).abs() < 1e-9);
        // Nada de paginas negativas ni de fracciones fuera de la hoja.
        assert!((en_la_pagina(-3, 4.0, 0.5).1 - 1.0).abs() < 1e-9);
        assert!((en_la_pagina(0, -2.0, 0.5).1).abs() < 1e-9);
        assert!((en_la_pagina(0, 0.5, -1.0).0).abs() < 1e-9);
    }
}
