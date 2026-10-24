//! **Los bloques de una nota**, para moverlos con su asa `⋮⋮` como en el
//! editor de documentos de Claude (H12, 1-oct-2026; el usuario: «esos
//! puntos que tienen en la esquina que sirven para mover el contenido… y
//! donde se mueve se proyecta una linea azul»).
//!
//! Un bloque es lo que se coge y se suelta entero: un parrafo, un titulo,
//! una cita, una foto o pagina viva (su renglon), un separador, un elemento
//! de lista **con lo que cuelga de el** (los renglones mas sangrados de
//! debajo), un bloque de codigo de valla a valla y una tabla entera. Los
//! renglones vacios no son bloques: son el aire entre ellos.
//!
//! Se cuenta en renglones del texto del editor (el Markdown, con las tablas
//! del control en sus renglones), y es puro: se prueba sin ventanas.

use crate::md_edicion::{self, Renglon};

/// Un bloque: del renglon `desde` al `hasta`, los dos dentro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bloque {
    pub desde: usize,
    pub hasta: usize,
}

fn sangria(r: &str) -> usize {
    r.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn es_de_lista(r: &str) -> bool {
    let t = r.trim_start();
    let vineta =
        t.len() >= 2 && matches!(t.as_bytes()[0], b'-' | b'*' | b'+') && t.as_bytes()[1] == b' ';
    let cifras = t.bytes().take_while(u8::is_ascii_digit).count();
    let numero = cifras > 0
        && matches!(t.as_bytes().get(cifras), Some(b'.' | b')'))
        && t.as_bytes().get(cifras + 1) == Some(&b' ');
    vineta || numero
}

/// **Los bloques del texto**, en orden.
pub fn bloques(texto: &str) -> Vec<Bloque> {
    let tipos = md_edicion::renglones(texto);
    let renglones: Vec<&str> = texto.split(['\n', '\r']).collect();
    let total = tipos.len().min(renglones.len());
    let mut v = Vec::new();
    let mut n = 0;
    while n < total {
        let desde = n;
        match tipos[n] {
            Renglon::Tabla => {
                while n + 1 < total && tipos[n + 1] == Renglon::Tabla {
                    n += 1;
                }
            }
            Renglon::Valla => {
                n += 1;
                while n < total && tipos[n] != Renglon::Valla {
                    n += 1;
                }
                n = n.min(total - 1);
            }
            Renglon::Texto(_) if renglones[n].trim().is_empty() => {
                n += 1;
                continue;
            }
            Renglon::Texto(_) if es_de_lista(renglones[n]) => {
                // Lo que cuelga de el: los renglones de debajo mas sangrados.
                let s = sangria(renglones[n]);
                while n + 1 < total
                    && matches!(tipos[n + 1], Renglon::Texto(_))
                    && !renglones[n + 1].trim().is_empty()
                    && sangria(renglones[n + 1]) > s
                {
                    n += 1;
                }
            }
            _ => {}
        }
        v.push(Bloque { desde, hasta: n });
        n += 1;
    }
    v
}

/// El bloque que tiene el renglon `linea`, si lo hay.
pub fn bloque_en(bloques: &[Bloque], linea: usize) -> Option<Bloque> {
    bloques
        .iter()
        .copied()
        .find(|b| b.desde <= linea && linea <= b.hasta)
}

/// **Donde se puede soltar**: delante de cada bloque y detras del ultimo
/// (en renglones), sin los dos bordes del bloque que se mueve (soltarlo
/// ahi lo deja donde estaba).
pub fn sitios(bloques: &[Bloque], movido: Bloque) -> Vec<usize> {
    let mut s: Vec<usize> = bloques.iter().map(|b| b.desde).collect();
    if let Some(u) = bloques.last() {
        s.push(u.hasta + 1);
    }
    s.retain(|&k| k != movido.desde && k != movido.hasta + 1);
    s
}

/// **Mueve el bloque `b`** para que quede delante del renglon `antes_de`
/// de ahora (o al final, con el numero de renglones). Devuelve el texto y
/// el renglon donde empieza ahora el bloque; `None` si no se mueve.
pub fn mover(texto: &str, b: Bloque, antes_de: usize) -> Option<(String, usize)> {
    let mut renglones: Vec<&str> = texto.split('\n').collect();
    let n = renglones.len();
    if b.hasta >= n
        || b.desde > b.hasta
        || antes_de > n
        || (b.desde..=b.hasta + 1).contains(&antes_de)
    {
        return None;
    }
    let movidos: Vec<&str> = renglones.drain(b.desde..=b.hasta).collect();
    let destino = if antes_de > b.hasta {
        antes_de - movidos.len()
    } else {
        antes_de
    };
    renglones.splice(destino..destino, movidos);
    Some((renglones.join("\n"), destino))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const NOTA: &str =
        "# Titulo\nUn parrafo.\n\n- uno\n  - de uno\n- dos\n```\ncodigo\n```\n![foto](a.png)\nFin.";

    #[test]
    fn cada_parrafo_titulo_foto_codigo_y_lista_es_un_bloque() {
        let b = bloques(NOTA);
        let rangos: Vec<(usize, usize)> = b.iter().map(|b| (b.desde, b.hasta)).collect();
        // El renglon vacio no es un bloque; «uno» se lleva lo que cuelga.
        assert_eq!(
            rangos,
            [(0, 0), (1, 1), (3, 4), (5, 5), (6, 8), (9, 9), (10, 10)]
        );
        assert_eq!(bloque_en(&b, 7), Some(Bloque { desde: 6, hasta: 8 }));
        assert_eq!(bloque_en(&b, 2), None, "el aire no se coge");
    }

    #[test]
    fn mover_un_bloque_abajo_y_arriba_vuelve_a_dejarlo_igual() {
        let (t, donde) = mover(NOTA, Bloque { desde: 1, hasta: 1 }, 11).unwrap();
        assert!(t.ends_with("Fin.\nUn parrafo."), "{t}");
        assert_eq!(donde, 10);
        let (t, donde) = mover(
            &t,
            Bloque {
                desde: 10,
                hasta: 10,
            },
            1,
        )
        .unwrap();
        assert_eq!(t, NOTA);
        assert_eq!(donde, 1);
        // La lista con lo que cuelga va entera.
        let (t, _) = mover(NOTA, Bloque { desde: 3, hasta: 4 }, 0).unwrap();
        assert!(t.starts_with("- uno\n  - de uno\n# Titulo"));
        // Casos negativos: soltarlo pegado a si mismo no mueve nada.
        assert_eq!(mover(NOTA, Bloque { desde: 3, hasta: 4 }, 3), None);
        assert_eq!(mover(NOTA, Bloque { desde: 3, hasta: 4 }, 5), None);
        assert_eq!(
            mover(
                NOTA,
                Bloque {
                    desde: 3,
                    hasta: 40
                },
                0
            ),
            None
        );
    }

    #[test]
    fn los_sitios_de_soltar_no_incluyen_los_bordes_del_movido() {
        let b = bloques(NOTA);
        let s = sitios(&b, Bloque { desde: 3, hasta: 4 });
        assert_eq!(s, [0, 1, 6, 9, 10, 11]);
    }
}
