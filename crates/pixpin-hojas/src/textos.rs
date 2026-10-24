//! `sharedStrings.xml`, perezoso: al abrir solo se apunta donde empieza y
//! acaba cada `<si>`; el texto se saca cuando una celda visible lo pide.
//!
//! Un libro de cien mil filas puede traer un millon de textos: crear un
//! `String` para cada uno al abrir costaria mas que pintar la primera
//! pantalla, y casi ninguno se va a ver.

use crate::estilos::{Fuente, Tema, fuente_de};
use crate::xml::{self, Evento, Paso};

#[derive(Default)]
pub struct Textos {
    datos: Vec<u8>,
    /// Inicio y fin (bytes de `datos`) del contenido de cada `<si>`.
    rangos: Vec<(u32, u32)>,
}

/// Un tramo de texto rico (`<r>`), con su fuente ya completada sobre la de
/// la celda.
#[derive(Debug, Clone, PartialEq)]
pub struct TramoRico {
    pub texto: String,
    pub fuente: Fuente,
}

impl Textos {
    pub fn leer(datos: Vec<u8>) -> Textos {
        let mut rangos = Vec::new();
        let mut pos = 0usize;
        let mut abierto: Option<usize> = None;
        let tope = datos.len().min(u32::MAX as usize);
        let d = &datos[..tope];
        loop {
            let antes = pos;
            match xml::siguiente(d, &mut pos, true) {
                Paso::Evento(Evento::Abre {
                    nombre: b"si",
                    vacia,
                    ..
                }) => {
                    if vacia {
                        rangos.push((pos as u32, pos as u32));
                    } else {
                        abierto = Some(pos);
                    }
                }
                Paso::Evento(Evento::Cierra { nombre: b"si" }) => {
                    if let Some(a) = abierto.take() {
                        rangos.push((a as u32, antes as u32));
                    }
                }
                Paso::Evento(_) => {}
                _ => break,
            }
        }
        if let Some(a) = abierto {
            rangos.push((a as u32, tope as u32));
        }
        Textos { datos, rangos }
    }

    pub fn len(&self) -> usize {
        self.rangos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rangos.is_empty()
    }

    fn trozo(&self, i: u32) -> Option<&[u8]> {
        let &(a, b) = self.rangos.get(i as usize)?;
        self.datos.get(a as usize..b as usize)
    }

    /// El texto plano del texto `i` (vacio si no existe).
    pub fn texto(&self, i: u32) -> String {
        let Some(d) = self.trozo(i) else {
            return String::new();
        };
        texto_de_t(d)
    }

    /// Los tramos si el texto es rico (`<r>`); `None` si es plano.
    pub fn rico(
        &self,
        i: u32,
        base: &Fuente,
        tema: &Tema,
        paleta: &[u32],
    ) -> Option<Vec<TramoRico>> {
        let d = self.trozo(i)?;
        if !d.windows(2).any(|w| w == b"<r" || w == b":r") {
            return None;
        }
        let raiz = xml::arbol(d);
        let rs: Vec<_> = raiz.hijos_de("r").collect();
        if rs.is_empty() {
            return None;
        }
        let mut v = Vec::new();
        // Texto suelto antes de los tramos (`<si><t>a</t><r>...`).
        if let Some(t) = raiz.hijo("t") {
            v.push(TramoRico {
                texto: xml::sin_escapes_ooxml(&t.texto).into_owned(),
                fuente: base.clone(),
            });
        }
        for r in rs {
            let fuente = r
                .hijo("rPr")
                .map(|p| fuente_de(p, tema, paleta, base))
                .unwrap_or_else(|| base.clone());
            let texto: String = r.hijos_de("t").map(|t| t.texto.as_str()).collect();
            v.push(TramoRico {
                texto: xml::sin_escapes_ooxml(&texto).into_owned(),
                fuente,
            });
        }
        Some(v)
    }
}

/// El texto de los `<t>` de un trozo, saltando la fonetica (`<rPh>`).
pub fn texto_de_t(d: &[u8]) -> String {
    let mut pos = 0;
    let mut en_t = 0u32;
    let mut en_fonetica = 0u32;
    let mut s = String::new();
    loop {
        match xml::siguiente(d, &mut pos, true) {
            Paso::Evento(Evento::Abre {
                nombre,
                vacia: false,
                ..
            }) => match nombre {
                b"t" => en_t += 1,
                b"rPh" => en_fonetica += 1,
                _ => {}
            },
            Paso::Evento(Evento::Cierra { nombre }) => match nombre {
                b"t" => en_t = en_t.saturating_sub(1),
                b"rPh" => en_fonetica = en_fonetica.saturating_sub(1),
                _ => {}
            },
            Paso::Evento(Evento::Texto(t)) if en_t > 0 && en_fonetica == 0 => {
                s.push_str(&xml::desescapar(t))
            }
            Paso::Evento(Evento::Cdata(t)) if en_t > 0 && en_fonetica == 0 => {
                s.push_str(&String::from_utf8_lossy(t))
            }
            Paso::Evento(_) => {}
            _ => break,
        }
    }
    match xml::sin_escapes_ooxml(&s) {
        std::borrow::Cow::Borrowed(_) => s,
        std::borrow::Cow::Owned(o) => o,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn textos_planos_ricos_y_foneticos() {
        let x = br#"<sst count="3"><si><t>Hola &amp; adios</t></si><si/><si><r><rPr><b/><sz val="14"/></rPr><t>Neg</t></r><r><t xml:space="preserve"> normal</t></r><rPh><t>NO</t></rPh></si><si><t>a_x000D_b</t></si></sst>"#;
        let t = Textos::leer(x.to_vec());
        assert_eq!(t.len(), 4);
        assert_eq!(t.texto(0), "Hola & adios");
        assert_eq!(t.texto(1), "");
        assert_eq!(t.texto(2), "Neg normal");
        assert_eq!(t.texto(3), "a\rb");
        assert_eq!(t.texto(99), "");
        let r = t
            .rico(
                2,
                &Fuente::default(),
                &Tema::default(),
                &crate::estilos::PALETA,
            )
            .unwrap();
        assert_eq!(r.len(), 2);
        assert!(r[0].fuente.negrita && r[0].fuente.tam == 14.0);
        assert!(!r[1].fuente.negrita);
        assert!(
            t.rico(
                0,
                &Fuente::default(),
                &Tema::default(),
                &crate::estilos::PALETA
            )
            .is_none()
        );
    }

    #[test]
    fn un_sst_cortado_no_revienta() {
        let t = Textos::leer(b"<sst><si><t>uno</t></si><si><t>do".to_vec());
        assert_eq!(t.len(), 2);
        assert_eq!(t.texto(1), "do");
    }
}
