//! Comentarios de celda: las notas de siempre (`xl/comments*.xml`, el
//! triangulo rojo) y los comentarios encadenados de Excel 365
//! (`xl/threadedComments/*.xml`, con respuestas y nombres de las personas).
//!
//! Solo para ver: el visor pinta el triangulo rojo en la esquina y ensena
//! una tarjeta con el texto al pasar el raton.

use std::collections::HashMap;

use crate::hoja::referencia;
use crate::paquete::{Paquete, Relacion};
use crate::xml::{self, Nodo};

/// Tope de comentarios por hoja y de largo de cada texto.
pub const MAX_COMENTARIOS: usize = 50_000;
const MAX_TEXTO: usize = 32_768;

#[derive(Debug, Clone, PartialEq)]
pub struct Comentario {
    pub fila: u32,
    pub col: u32,
    pub autor: String,
    pub texto: String,
    /// Las respuestas de un comentario encadenado: (autor, texto).
    pub respuestas: Vec<(String, String)>,
    /// Es un comentario encadenado (y no una nota).
    pub encadenado: bool,
}

fn recortar(mut s: String) -> String {
    if s.len() > MAX_TEXTO {
        let mut i = MAX_TEXTO;
        while !s.is_char_boundary(i) {
            i -= 1;
        }
        s.truncate(i);
        s.push('…');
    }
    s
}

/// El texto de un `<text>` o `<si>`: los `<t>` de sus tramos, sin el
/// fonetico (`rPh`).
fn texto_de(n: &Nodo) -> String {
    fn rec(n: &Nodo, s: &mut String) {
        for h in &n.hijos {
            match h.nombre.as_str() {
                "rPh" | "phoneticPr" | "rPr" => {}
                "t" => s.push_str(&h.texto),
                _ => rec(h, s),
            }
        }
    }
    let mut s = n.texto.clone();
    rec(n, &mut s);
    s
}

/// Lee las notas y los comentarios encadenados de una hoja a partir de sus
/// relaciones. Salen ordenados por celda; si una celda tiene comentario
/// encadenado, ese manda sobre su nota de compatibilidad.
pub fn leer(paquete: &Paquete, rels: &[Relacion]) -> Vec<Comentario> {
    let mut v: Vec<Comentario> = Vec::new();
    for r in rels.iter().filter(|r| r.tipo == "comments" && !r.externa) {
        if let Some(b) = paquete.leer(&r.destino) {
            notas(&xml::arbol(&b), &mut v);
        }
    }
    let hilos: Vec<Comentario> = {
        let personas = paquete
            .leer("xl/persons/person.xml")
            .map(|b| personas(&xml::arbol(&b)))
            .unwrap_or_default();
        let mut hilos = Vec::new();
        for r in rels
            .iter()
            .filter(|r| r.tipo == "threadedComment" && !r.externa)
        {
            if let Some(b) = paquete.leer(&r.destino) {
                encadenados(&xml::arbol(&b), &personas, &mut hilos);
            }
        }
        hilos
    };
    // El hilo sustituye a la nota de la misma celda.
    v.retain(|n| !hilos.iter().any(|h| h.fila == n.fila && h.col == n.col));
    v.extend(hilos);
    v.sort_by_key(|c| (c.fila, c.col));
    v.truncate(MAX_COMENTARIOS);
    v
}

fn notas(raiz: &Nodo, salida: &mut Vec<Comentario>) {
    let mut autores = Vec::new();
    raiz.todos("author", &mut autores);
    let autores: Vec<String> = autores.iter().map(|a| a.texto.trim().to_string()).collect();
    let mut cs = Vec::new();
    raiz.todos("comment", &mut cs);
    for c in cs {
        if salida.len() >= MAX_COMENTARIOS {
            break;
        }
        let Some((fila, col)) = c.attr("ref").and_then(|r| referencia(r.as_bytes())) else {
            continue;
        };
        let autor = c
            .attr("authorId")
            .and_then(|i| i.parse::<usize>().ok())
            .and_then(|i| autores.get(i))
            .cloned()
            .unwrap_or_default();
        let texto = c.hijo("text").map(texto_de).unwrap_or_default();
        if texto.trim().is_empty() {
            continue;
        }
        salida.push(Comentario {
            fila,
            col,
            autor,
            texto: recortar(texto),
            respuestas: Vec::new(),
            encadenado: false,
        });
    }
}

fn personas(raiz: &Nodo) -> HashMap<String, String> {
    let mut v = Vec::new();
    raiz.todos("person", &mut v);
    v.iter()
        .filter_map(|p| {
            Some((
                p.attr("id")?.to_ascii_lowercase(),
                p.attr("displayName")?.to_string(),
            ))
        })
        .collect()
}

fn encadenados(raiz: &Nodo, personas: &HashMap<String, String>, salida: &mut Vec<Comentario>) {
    let mut cs = Vec::new();
    raiz.todos("threadedComment", &mut cs);
    let nombre = |c: &Nodo| {
        c.attr("personId")
            .and_then(|p| personas.get(&p.to_ascii_lowercase()))
            .cloned()
            .unwrap_or_else(|| "Comentarista".to_string())
    };
    // Primero las raices; despues las respuestas, en el orden del archivo.
    for c in cs
        .iter()
        .filter(|c| c.attr("parentId").is_none_or(|p| p.is_empty()))
    {
        if salida.len() >= MAX_COMENTARIOS {
            break;
        }
        let Some((fila, col)) = c.attr("ref").and_then(|r| referencia(r.as_bytes())) else {
            continue;
        };
        let texto = c.hijo("text").map(|t| t.texto.clone()).unwrap_or_default();
        if texto.trim().is_empty() {
            continue;
        }
        let id = c.attr("id").unwrap_or_default();
        let respuestas = cs
            .iter()
            .filter(|r| !id.is_empty() && r.attr("parentId") == Some(id))
            .filter_map(|r| {
                let t = r.hijo("text")?.texto.clone();
                (!t.trim().is_empty()).then(|| (nombre(r), recortar(t)))
            })
            .take(200)
            .collect();
        salida.push(Comentario {
            fila,
            col,
            autor: nombre(c),
            texto: recortar(texto),
            respuestas,
            encadenado: true,
        });
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn notas_antiguas_con_autor_y_texto_rico() {
        let x = br#"<comments><authors><author>Ana</author><author>Luis</author></authors><commentList><comment ref="B2" authorId="1"><text><r><rPr><b/></rPr><t>Luis:</t></r><r><t xml:space="preserve"> revisar</t></r><rPh><t>x</t></rPh></text></comment><comment ref="C3" authorId="9"><text><t>sin autor</t></text></comment><comment ref="ZZZZ99999" authorId="0"><text><t>mal</t></text></comment><comment ref="D4" authorId="0"><text><t>  </t></text></comment></commentList></comments>"#;
        let mut v = Vec::new();
        notas(&xml::arbol(x), &mut v);
        assert_eq!(
            v.len(),
            2,
            "la de referencia imposible y la vacia se descartan: {v:?}"
        );
        assert_eq!(
            (
                v[0].fila,
                v[0].col,
                v[0].autor.as_str(),
                v[0].texto.as_str()
            ),
            (1, 1, "Luis", "Luis: revisar")
        );
        assert_eq!(v[1].autor, "", "autor fuera de la lista");
        assert!(!v[0].encadenado);
    }

    #[test]
    fn hilos_con_respuestas_y_personas() {
        let p = br#"<personList><person displayName="Ana Paz" id="{AAA}" userId="a"/><person displayName="Luis" id="{BBB}"/></personList>"#;
        let personas = personas(&xml::arbol(p));
        let x = br#"<ThreadedComments><threadedComment ref="A1" personId="{aaa}" id="{T1}"><text>Hola</text></threadedComment><threadedComment ref="A1" personId="{BBB}" id="{T2}" parentId="{T1}"><text>Respuesta</text></threadedComment><threadedComment ref="A1" personId="{ZZZ}" id="{T3}" parentId="{T1}"><text>  </text></threadedComment><threadedComment ref="B2" id="{T4}"><text>Otro</text></threadedComment><threadedComment ref="Q" id="{T5}"><text>mal</text></threadedComment></ThreadedComments>"#;
        let mut v = Vec::new();
        encadenados(&xml::arbol(x), &personas, &mut v);
        assert_eq!(v.len(), 2);
        assert_eq!(
            v[0].autor, "Ana Paz",
            "el id se compara sin mirar mayusculas"
        );
        assert_eq!(
            v[0].respuestas,
            vec![("Luis".to_string(), "Respuesta".to_string())]
        );
        assert_eq!(v[1].autor, "Comentarista");
        assert!(v[1].encadenado && v[1].respuestas.is_empty());
    }

    #[test]
    fn textos_enormes_se_cortan_sin_romper_utf8() {
        let largo = "é".repeat(MAX_TEXTO);
        let t = recortar(largo);
        assert!(t.len() <= MAX_TEXTO + 4 && t.ends_with('…'));
        assert_eq!(recortar("corto".into()), "corto");
    }
}
