//! **Los textos**: el texto de varias lineas (MTEXTO), cambiar lo que dice
//! un texto o una cota (EDITARTEXTO) y el alto de los textos nuevos.

use opencadcodec::entities::{AttachmentPoint, MText};
use opencadcodec::{EntityType, Handle};

use crate::cotas_bloque as cb;
use crate::especialidad as esp;
use crate::formas::v3;
use crate::geo::{self, P};
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, Previa, Registro};
use crate::ordenes::{Grupo, Id, Orden};

pub const REGISTROS: &[Registro] = &[
    Registro {
        orden: Orden {
            id: Id::Otra("MTEXTO"),
            nombre: "MTEXTO",
            alias: &["MT", "TXM", "MTEXT"],
            grupo: Grupo::Anotar,
            ayuda: "Texto de varias lineas en un recuadro",
            con_objetos: false,
        },
        etiqueta: "Texto múltiple",
        crear: || Box::new(MTexto::default()),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("EDITARTEXTO"),
            nombre: "EDITARTEXTO",
            alias: &["ED", "DDEDIT", "TEXTEDIT"],
            grupo: Grupo::Anotar,
            ayuda: "Cambiar lo que dice un texto o una cota",
            con_objetos: false,
        },
        etiqueta: "Editar texto",
        crear: || Box::new(EditarTexto::default()),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("ALTOTEXTO"),
            nombre: "ALTOTEXTO",
            alias: &["AT"],
            grupo: Grupo::Anotar,
            ayuda: "El alto de los textos nuevos",
            con_objetos: false,
        },
        etiqueta: "Alto de texto",
        crear: || Box::new(AltoTexto),
    },
];

/// Un alto valido (mayor que cero).
fn alto_valido(v: f64) -> bool {
    v > 0.0 && v.is_finite()
}

// ------------------------------------------------------------ MTEXTO

/// **MTEXTO**: dos esquinas y el texto, renglon a renglon (Enter en vacio
/// termina). `A` antes de la primera esquina cambia el alto.
#[derive(Default)]
struct MTexto {
    alto: Option<f64>,
    pidiendo_alto: bool,
    esquinas: Vec<P>,
    renglones: Vec<String>,
}

impl MTexto {
    fn alto(&self, cx: &Lectura) -> f64 {
        self.alto.unwrap_or(cx.ajustes.alto_texto)
    }
}

impl Herramienta for MTexto {
    fn indicacion(&self, cx: &Lectura) -> String {
        if self.pidiendo_alto {
            return format!("Alto del texto <{}>:", self.alto(cx));
        }
        match self.esquinas.len() {
            0 => format!("Primera esquina (A alto <{}>):", self.alto(cx)),
            1 => "Esquina opuesta:".into(),
            _ if self.renglones.is_empty() => "Texto:".into(),
            _ => "Siguiente renglon (Enter en vacio termina):".into(),
        }
    }

    fn base(&self) -> Option<P> {
        (self.esquinas.len() == 1).then(|| self.esquinas[0])
    }

    fn pide_texto(&self) -> bool {
        self.esquinas.len() == 2
    }

    fn punto(&mut self, _cx: &mut Contexto, p: P) -> Flujo {
        if self.pidiendo_alto || self.esquinas.len() >= 2 {
            return Flujo::NoEntiende;
        }
        self.esquinas.push(p);
        Flujo::Sigue
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        if !self.pidiendo_alto {
            return Flujo::NoEntiende;
        }
        if alto_valido(v) {
            self.alto = Some(v);
            self.pidiendo_alto = false;
        } else {
            cx.decir("El alto tiene que ser mayor que cero");
        }
        Flujo::Sigue
    }

    fn texto(&mut self, _cx: &mut Contexto, t: &str) -> Flujo {
        if self.esquinas.len() == 2 {
            self.renglones.push(t.to_string());
            return Flujo::Sigue;
        }
        if self.esquinas.is_empty() && matches!(t.trim().to_uppercase().as_str(), "A" | "ALTO") {
            self.pidiendo_alto = true;
            return Flujo::Sigue;
        }
        Flujo::NoEntiende
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        if self.pidiendo_alto {
            self.pidiendo_alto = false;
            return Flujo::Sigue;
        }
        if self.esquinas.len() < 2 || self.renglones.is_empty() {
            return Flujo::Termina;
        }
        let (a, b) = (self.esquinas[0], self.esquinas[1]);
        let alto = self.alto(&cx.leer());
        let mut t = MText::with_value(
            self.renglones.join("\\P"),
            v3([a[0].min(b[0]), a[1].max(b[1])]),
        )
        .with_height(alto)
        .with_width((b[0] - a[0]).abs());
        t.attachment_point = AttachmentPoint::TopLeft;
        cx.crear("MTEXTO", vec![EntityType::MText(t)], Some(esp::CAPA_TEXTOS));
        Flujo::Termina
    }

    fn previa(&self, _cx: &Lectura, c: P) -> Previa {
        match self.esquinas[..] {
            [a] => {
                Previa::default().con(&[esp::polilinea(&[a, [c[0], a[1]], c, [a[0], c[1]]], true)])
            }
            [a, b] => {
                Previa::default().con(&[esp::polilinea(&[a, [b[0], a[1]], b, [a[0], b[1]]], true)])
            }
            _ => Previa::default(),
        }
    }
}

// ------------------------------------------------------- EDITARTEXTO

/// Lo que dice un texto, un MTEXT o una cota (su texto propio, o `<>`).
fn contenido(e: &EntityType) -> Option<String> {
    match e {
        EntityType::Text(t) => Some(t.value.clone()),
        EntityType::MText(t) => Some(t.value.clone()),
        EntityType::Dimension(d) => Some(
            d.base()
                .text_override()
                .filter(|t| !t.is_empty())
                .unwrap_or("<>")
                .to_string(),
        ),
        _ => None,
    }
}

/// **Cambia lo que dice** el texto o la cota `h` (en una cota, `<>` es lo
/// medido y vuelve a su medida). Un paso de deshacer.
pub fn cambiar_texto(cx: &mut Contexto, h: Handle, nuevo: &str) -> bool {
    let defecto = cb::Medidas::para(cx.ajustes.alto_texto, cx.ajustes.decimales);
    cx.dibujo.editar("EDITARTEXTO", |t| {
        let es_cota = matches!(t.entidad(h), Some(EntityType::Dimension(_)));
        let ok = t.cambiar(h, |e| match e {
            EntityType::Text(x) => x.value = nuevo.to_string(),
            EntityType::MText(x) => x.value = nuevo.to_string(),
            EntityType::Dimension(d) => {
                let propio =
                    (!nuevo.trim().is_empty() && nuevo.trim() != "<>").then(|| nuevo.to_string());
                d.base_mut().set_text_override(propio);
            }
            _ => {}
        });
        if ok && es_cota {
            cb::rehacer_bloque(t, h, defecto);
        }
        ok
    })
}

/// **EDITARTEXTO**: un clic en un texto o una cota y lo nuevo que dice;
/// sigue con otro hasta Enter.
#[derive(Default)]
struct EditarTexto {
    elegido: Option<Handle>,
}

impl Herramienta for EditarTexto {
    fn indicacion(&self, cx: &Lectura) -> String {
        match self
            .elegido
            .and_then(|h| cx.dibujo.entidad(h))
            .and_then(contenido)
        {
            Some(t) => format!("Nuevo texto <{t}>:"),
            None => "Elija el texto o la cota (Enter termina):".into(),
        }
    }

    fn pide_texto(&self) -> bool {
        self.elegido.is_some()
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        if self.elegido.is_some() {
            return Flujo::NoEntiende;
        }
        let tol = cx.ajustes.alto_texto.max(geo::EPS);
        match cb::elegir(cx.dibujo, p, tol, true, true) {
            Some(h) => self.elegido = Some(h),
            None => cx.decir("No hay un texto ni una cota ahi"),
        }
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        let Some(h) = self.elegido.take() else {
            return Flujo::NoEntiende;
        };
        if !cambiar_texto(cx, h, t) {
            cx.decir("Ese objeto ya no esta");
        }
        Flujo::Sigue
    }

    fn enter(&mut self, _cx: &mut Contexto) -> Flujo {
        // Enter con un texto elegido: lo deja como estaba.
        if self.elegido.take().is_some() {
            return Flujo::Sigue;
        }
        Flujo::Termina
    }
}

// ---------------------------------------------------------- ALTOTEXTO

/// **ALTOTEXTO**: el alto de los textos nuevos (y de las cotas de un dibujo
/// que aun no tiene estilo de cota).
struct AltoTexto;

impl Herramienta for AltoTexto {
    fn indicacion(&self, cx: &Lectura) -> String {
        format!("Alto de los textos <{}>:", cx.ajustes.alto_texto)
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        if !alto_valido(v) {
            cx.decir("El alto tiene que ser mayor que cero");
            return Flujo::Sigue;
        }
        cx.ajustes.alto_texto = v;
        cx.decir(format!("Alto de texto: {v}"));
        Flujo::Termina
    }
}

#[cfg(test)]
mod pruebas {
    use opencadcodec::entities::Dimension;

    use super::*;
    use crate::{Dibujo, Sesion};

    fn sesion() -> Sesion {
        Sesion::nueva(Dibujo::nuevo())
    }

    fn mtextos(s: &Sesion) -> Vec<MText> {
        s.dibujo
            .doc
            .model_space_entities()
            .filter_map(|e| match e {
                EntityType::MText(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn mtexto_con_alto_y_renglones_tal_cual() {
        let mut s = sesion();
        s.escribir("mt");
        s.escribir("a");
        // Caso negativo: un alto que no vale.
        s.escribir("0");
        s.escribir("400");
        s.escribir("0,1000");
        s.escribir("3000,0");
        assert!(s.pide_texto());
        // Va tal cual: con espacios, comas y numeros.
        s.escribir("Sala comedor, 1,5 m");
        s.escribir("12,40");
        s.escribir("");
        assert_eq!(s.activa(), None);
        let v = mtextos(&s);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].value, "Sala comedor, 1,5 m\\P12,40");
        assert_eq!(v[0].height, 400.0);
        assert_eq!(v[0].rectangle_width, 3000.0);
        assert_eq!(
            (v[0].insertion_point.x, v[0].insertion_point.y),
            (0.0, 1000.0)
        );
        assert_eq!(v[0].common.layer, esp::CAPA_TEXTOS);
        // Caso negativo: sin texto no deja nada.
        s.escribir("mtext");
        s.escribir("0,0");
        s.escribir("10,10");
        s.escribir("");
        assert_eq!(mtextos(&s).len(), 1);
    }

    #[test]
    fn alto_de_texto() {
        let mut s = sesion();
        s.escribir("at");
        s.escribir("-1");
        assert!(s.activa().is_some());
        s.escribir("180");
        assert_eq!(s.ajustes.alto_texto, 180.0);
        s.escribir("mt");
        s.escribir("0,0");
        s.escribir("100,-100");
        s.escribir("hola");
        s.escribir("");
        assert_eq!(mtextos(&s)[0].height, 180.0);
    }

    #[test]
    fn editar_un_texto_un_mtexto_y_una_cota() {
        let mut s = sesion();
        s.escribir("t");
        s.escribir("0,0");
        s.escribir("250");
        s.escribir("Dormitorio");
        s.escribir("mt");
        s.escribir("0,3000");
        s.escribir("2000,2000");
        s.escribir("Nota");
        s.escribir("");
        s.escribir("dli");
        s.escribir("0,-2000");
        s.escribir("3000,-2000");
        s.escribir("1500,-2500");
        s.escribir("ed");
        // Caso negativo: nada ahi.
        s.escribir("50000,50000");
        assert!(s.registro.iter().any(|l| l.contains("No hay un texto")));
        s.escribir("100,100");
        assert!(s.indicacion().contains("Dormitorio"), "{}", s.indicacion());
        s.escribir("Dormitorio principal");
        s.escribir("100,2900");
        s.escribir("Nota: ver detalle 3");
        // La cota: un clic en su linea de cota.
        s.escribir("1500,-2500");
        assert!(s.indicacion().contains("<>"), "{}", s.indicacion());
        s.escribir("<> (eje)");
        s.escribir("");
        assert_eq!(s.activa(), None);
        let textos: Vec<String> = s
            .dibujo
            .doc
            .model_space_entities()
            .filter_map(|e| match e {
                EntityType::Text(t) => Some(t.value.clone()),
                EntityType::MText(t) => Some(t.value.clone()),
                _ => None,
            })
            .collect();
        assert!(
            textos.contains(&"Dormitorio principal".to_string()),
            "{textos:?}"
        );
        assert!(
            textos.contains(&"Nota: ver detalle 3".to_string()),
            "{textos:?}"
        );
        let dim = s
            .dibujo
            .doc
            .model_space_entities()
            .find_map(|e| match e {
                EntityType::Dimension(d) => Some(d.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(dim.base().text_override(), Some("<> (eje)"));
        let en_bloque: Vec<String> = s
            .dibujo
            .doc
            .entities_in_block(&dim.base().block_name)
            .filter_map(|e| match e {
                EntityType::MText(t) => Some(t.value.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(en_bloque, vec!["3000 (eje)".to_string()]);
        // Deshacer devuelve el texto medido al bloque.
        s.escribir("u");
        let dim2 = s
            .dibujo
            .doc
            .model_space_entities()
            .find_map(|e| match e {
                EntityType::Dimension(d) => Some(d.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(dim2.base().text_override(), None);
        let en_bloque: Vec<String> = s
            .dibujo
            .doc
            .entities_in_block(&dim2.base().block_name)
            .filter_map(|e| match e {
                EntityType::MText(t) => Some(t.value.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(en_bloque, vec!["3000".to_string()]);
        // Y `<>` vuelve a lo medido.
        let h = dim2.base().common.handle;
        s.escribir("ed");
        s.escribir("1500,-2500");
        s.escribir("Otra");
        s.escribir("ed");
        s.escribir("1500,-2500");
        s.escribir("<>");
        s.escape();
        let Some(EntityType::Dimension(Dimension::Linear(l))) = s.dibujo.entidad(h) else {
            panic!()
        };
        assert_eq!(l.base.text_override(), None);
    }
}
