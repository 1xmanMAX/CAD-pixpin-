//! **Copiar, cortar y pegar** con el [`crate::portapapeles`]: las ordenes
//! detras de Ctrl+C, Ctrl+X, Ctrl+Mayus+C y Ctrl+V.

use std::rc::Rc;

use crate::geo::P;
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, Previa, Registro};
use crate::ordenes::{Grupo, Id, Orden};
use crate::portapapeles::{self as pp, Contenido};

pub const COPIAR: &str = "COPIARPORTAPAPELES";
pub const CORTAR: &str = "CORTARPORTAPAPELES";
pub const COPIABASE: &str = "COPIABASE";
pub const PEGAR: &str = "PEGARPORTAPAPELES";

pub const REGISTROS: &[Registro] = &[
    Registro {
        orden: Orden {
            id: Id::Otra(COPIAR),
            nombre: COPIAR,
            alias: &["COPYCLIP"],
            grupo: Grupo::Edicion,
            ayuda: "Copiar lo elegido al portapapeles  Ctrl+C",
            con_objetos: true,
        },
        etiqueta: "Copiar",
        crear: || Box::new(Copiar { borrar: false }),
    },
    Registro {
        orden: Orden {
            id: Id::Otra(CORTAR),
            nombre: CORTAR,
            alias: &["CUTCLIP"],
            grupo: Grupo::Edicion,
            ayuda: "Llevar lo elegido al portapapeles  Ctrl+X",
            con_objetos: true,
        },
        etiqueta: "Cortar",
        crear: || Box::new(Copiar { borrar: true }),
    },
    Registro {
        orden: Orden {
            id: Id::Otra(COPIABASE),
            nombre: COPIABASE,
            alias: &["COPYBASE"],
            grupo: Grupo::Edicion,
            ayuda: "Copiar al portapapeles con un punto base  Ctrl+Mayus+C",
            con_objetos: true,
        },
        etiqueta: "Copiar con base",
        crear: || Box::new(CopiaBase),
    },
    Registro {
        orden: Orden {
            id: Id::Otra(PEGAR),
            nombre: PEGAR,
            alias: &["PASTECLIP", "PEGAR"],
            grupo: Grupo::Edicion,
            ayuda: "Pegar lo del portapapeles donde se haga clic  Ctrl+V",
            con_objetos: false,
        },
        etiqueta: "Pegar",
        crear: || Box::new(Pegar { contenido: None }),
    },
];

/// Copia lo elegido (y lo borra, si corta) con su base en `base`.
fn copiar(cx: &mut Contexto, base: Option<P>, borrar: bool) -> Flujo {
    let Some(c) = pp::copiar(cx.dibujo, cx.seleccion, base) else {
        cx.decir("No hay nada que copiar");
        return Flujo::Termina;
    };
    let n = c.entidades.len();
    pp::poner(c);
    if borrar {
        let hs = std::mem::take(cx.seleccion);
        cx.dibujo
            .editar("CORTAR", |t| hs.iter().filter(|h| t.quitar(**h)).count());
        cx.decir(format!("{n} objetos cortados al portapapeles"));
    } else {
        cx.decir(format!("{n} objetos copiados al portapapeles"));
    }
    Flujo::Termina
}

struct Copiar {
    borrar: bool,
}

impl Herramienta for Copiar {
    fn indicacion(&self, _cx: &Lectura) -> String {
        String::new()
    }

    fn al_tener_objetos(&mut self, cx: &mut Contexto) -> Flujo {
        copiar(cx, None, self.borrar)
    }
}

struct CopiaBase;

impl Herramienta for CopiaBase {
    fn indicacion(&self, _cx: &Lectura) -> String {
        "Punto base:".into()
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        copiar(cx, Some(p), false)
    }
}

struct Pegar {
    contenido: Option<Rc<Contenido>>,
}

impl Herramienta for Pegar {
    fn indicacion(&self, _cx: &Lectura) -> String {
        "Punto de insercion:".into()
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        self.contenido = pp::actual();
        if self.contenido.is_none() {
            cx.decir("El portapapeles esta vacio: copie algo antes (Ctrl+C)");
            return Flujo::Termina;
        }
        Flujo::Sigue
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        let Some(c) = &self.contenido else {
            return Flujo::Termina;
        };
        let (nuevas, fuera) = pp::pegar(cx.dibujo, c, p);
        cx.decir(format!("{} objetos pegados", nuevas.len()));
        if fuera > 0 {
            cx.decir(format!(
                "{fuera} objetos no se pegaron: usan bloques que este dibujo no tiene"
            ));
        }
        *cx.seleccion = nuevas;
        Flujo::Termina
    }

    fn previa(&self, _cx: &Lectura, cursor: P) -> Previa {
        match &self.contenido {
            Some(c) => Previa {
                formas: pp::formas_en(c, cursor),
                ..Default::default()
            },
            None => Previa::default(),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use crate::portapapeles as pp;
    use crate::{Dibujo, Sesion};

    fn cuantas(s: &Sesion) -> usize {
        s.dibujo.doc.model_space_entities().count()
    }

    fn con_una_linea() -> Sesion {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("l");
        s.escribir("0,0");
        s.escribir("100,0");
        s.escribir("");
        s
    }

    #[test]
    fn copiar_y_pegar_en_otro_dibujo() {
        pp::vaciar();
        let mut s = con_una_linea();
        s.elegir_todo();
        s.escribir("copyclip");
        assert_eq!(s.activa(), None);
        assert_eq!(s.seleccion.len(), 1);
        let mut otro = Sesion::nueva(Dibujo::nuevo());
        otro.escribir("pegar");
        assert_eq!(otro.previa([500.0, 500.0]).formas.len(), 1);
        otro.clic([500.0, 500.0], 1.0);
        assert_eq!(cuantas(&otro), 1);
        assert_eq!(otro.seleccion.len(), 1);
        assert_eq!(otro.activa(), None);
    }

    #[test]
    fn cortar_borra_y_pegar_con_base() {
        pp::vaciar();
        let mut s = con_una_linea();
        s.elegir_todo();
        s.escribir("copybase");
        s.escribir("100,0");
        assert_eq!(pp::actual().unwrap().base, [100.0, 0.0]);
        s.elegir_todo();
        s.escribir("cutclip");
        assert_eq!(cuantas(&s), 0);
        assert!(s.seleccion.is_empty());
        s.escribir("pasteclip");
        s.escribir("0,0");
        let e = s.dibujo.doc.model_space_entities().next().unwrap().clone();
        let l = match e {
            opencadcodec::EntityType::Line(l) => l,
            _ => panic!(),
        };
        // Cortado con la esquina como base: la linea empieza en el cursor.
        assert_eq!(l.start.x, 0.0);
        assert_eq!(l.end.x, 100.0);
    }

    #[test]
    fn pegar_sin_nada_no_hace_nada() {
        pp::vaciar();
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("pegar");
        assert_eq!(s.activa(), None);
        assert!(s.registro.last().unwrap().contains("vacio"));
        // Copiar sin nada elegido pide los objetos, y Enter sin ellos sale.
        s.escribir("copyclip");
        assert!(s.pide_objetos());
        s.escribir("");
        assert_eq!(s.activa(), None);
        assert!(pp::actual().is_none());
    }
}
