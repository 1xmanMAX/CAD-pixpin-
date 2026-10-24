//! **El indice de un documento**: los titulos y los capitulos, para saltar
//! a ellos. En el movil el `WebView` no lo trae y el libro se recorria a
//! dedo; en el escritorio un libro de trescientas paginas sin indice es un
//! pasillo sin puertas.
//!
//! Sale de los bloques ya leidos, asi que vale igual para un Word (sus
//! `Heading1..3`), un libro (cada capitulo y sus titulos) y una pagina o un
//! Markdown (`h1..h3`, `#`).

use crate::documento::{Clase, Documento};

/// Una entrada del indice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// 1 = capitulo o titulo principal; 2 y 3, lo de dentro.
    pub nivel: u8,
    pub titulo: String,
    /// El bloque del documento al que lleva.
    pub bloque: usize,
}

/// Lo mas hondo que entra: mas alla de tres niveles la lista deja de
/// servir para orientarse.
const NIVEL_MAXIMO: u8 = 3;
/// Un titulo mas largo se corta: es un renglon del indice, no el parrafo.
const LETRAS_MAXIMAS: usize = 80;

/// El indice, en el orden del documento.
///
/// Cada capitulo de un libro entra aunque no traiga titulo propio: se
/// nombra con su primer renglon, que en la mayoria de los EPUB es el nombre
/// del capitulo escrito como parrafo grande.
pub fn de(doc: &Documento) -> Vec<Entrada> {
    let mut salida = Vec::new();
    // Un capitulo abierto que aun no tiene nombre: el siguiente bloque con
    // texto se lo da.
    let mut capitulo_sin_nombre: Option<usize> = None;
    for (i, b) in doc.bloques.iter().enumerate() {
        match b.clase {
            Clase::Capitulo => capitulo_sin_nombre = Some(i),
            Clase::Titulo(n) if n <= NIVEL_MAXIMO => {
                let titulo = limpio(&b.texto());
                if titulo.is_empty() {
                    continue;
                }
                // El titulo del capitulo es la entrada del capitulo: no se
                // repite como «capitulo» y luego «titulo».
                let bloque = capitulo_sin_nombre.take().unwrap_or(i);
                salida.push(Entrada {
                    nivel: n.max(1),
                    titulo,
                    bloque,
                });
            }
            _ => {
                if let Some(c) = capitulo_sin_nombre {
                    let titulo = limpio(&b.texto());
                    if titulo.is_empty() || b.clase == Clase::Regla {
                        continue;
                    }
                    salida.push(Entrada {
                        nivel: 1,
                        titulo,
                        bloque: c,
                    });
                    capitulo_sin_nombre = None;
                }
            }
        }
    }
    salida
}

/// Un renglon: sin saltos, sin espacios repetidos y acotado.
fn limpio(t: &str) -> String {
    let una_linea: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if una_linea.chars().count() <= LETRAS_MAXIMAS {
        return una_linea;
    }
    let mut corto: String = una_linea.chars().take(LETRAS_MAXIMAS - 1).collect();
    corto.push('…');
    corto
}

/// La entrada en la que se esta leyendo: la ultima que empieza en o antes
/// del bloque `bloque`. Sirve para resaltarla en la lista.
pub fn actual(indice: &[Entrada], bloque: usize) -> Option<usize> {
    let despues = indice.partition_point(|e| e.bloque <= bloque);
    despues.checked_sub(1)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::documento::{Bloque, Trozo};

    fn b(clase: Clase, t: &str) -> Bloque {
        Bloque::nuevo(clase, vec![Trozo::llano(t)])
    }

    fn doc(bloques: Vec<Bloque>) -> Documento {
        Documento {
            bloques,
            ..Default::default()
        }
    }

    #[test]
    fn los_titulos_de_un_word_hacen_el_indice_con_su_nivel() {
        let d = doc(vec![
            b(Clase::Titulo(1), "Tema 1"),
            b(Clase::Parrafo, "texto"),
            b(Clase::Titulo(2), "  Apartado\n uno "),
            b(Clase::Titulo(5), "demasiado hondo"),
            b(Clase::Titulo(1), ""),
        ]);
        let i = de(&d);
        assert_eq!(i.len(), 2);
        assert_eq!(i[0].titulo, "Tema 1");
        assert_eq!(i[1].nivel, 2);
        assert_eq!(i[1].titulo, "Apartado uno");
        assert_eq!(i[1].bloque, 2);
    }

    #[test]
    fn un_capitulo_sin_titulo_se_nombra_con_su_primer_renglon() {
        let d = doc(vec![
            b(Clase::Capitulo, ""),
            b(Clase::Parrafo, "CAPITULO PRIMERO"),
            b(Clase::Parrafo, "En un lugar..."),
            b(Clase::Capitulo, ""),
            b(Clase::Titulo(2), "Capitulo segundo"),
        ]);
        let i = de(&d);
        assert_eq!(i.len(), 2);
        assert_eq!(i[0].titulo, "CAPITULO PRIMERO");
        assert_eq!(i[0].bloque, 0, "lleva al principio del capitulo");
        assert_eq!(i[1].titulo, "Capitulo segundo");
        assert_eq!(i[1].bloque, 3, "el titulo del capitulo no sale dos veces");
    }

    #[test]
    fn un_documento_sin_titulos_no_tiene_indice() {
        let d = doc(vec![
            b(Clase::Parrafo, "solo texto"),
            b(Clase::Lista, "uno"),
        ]);
        assert!(de(&d).is_empty());
        assert!(de(&Documento::default()).is_empty());
    }

    #[test]
    fn un_titulo_larguisimo_se_corta_en_un_renglon() {
        let largo = "palabra ".repeat(40);
        let d = doc(vec![b(Clase::Titulo(1), &largo)]);
        let t = &de(&d)[0].titulo;
        assert_eq!(t.chars().count(), LETRAS_MAXIMAS);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn se_sabe_en_que_entrada_se_esta_leyendo() {
        let i = vec![
            Entrada {
                nivel: 1,
                titulo: "a".into(),
                bloque: 3,
            },
            Entrada {
                nivel: 1,
                titulo: "b".into(),
                bloque: 10,
            },
        ];
        assert_eq!(actual(&i, 0), None, "antes del primer titulo no hay");
        assert_eq!(actual(&i, 3), Some(0));
        assert_eq!(actual(&i, 9), Some(0));
        assert_eq!(actual(&i, 50), Some(1));
    }
}
