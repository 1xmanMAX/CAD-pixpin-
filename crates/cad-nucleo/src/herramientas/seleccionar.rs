//! **Las ordenes de seleccion rapida** ([`crate::seleccion_rapida`]):
//! SELECCIONARSIMILAR, SELECCIONPREVIA, SELECCIONARCAPA e INVERTIRSELECCION.

use opencadcodec::Handle;

use crate::geo::P;
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, Registro};
use crate::ordenes::{Grupo, Id, Orden};
use crate::seleccion_rapida as sr;

pub const REGISTROS: &[Registro] = &[
    Registro {
        orden: Orden {
            id: Id::Otra("SELECCIONARSIMILAR"),
            nombre: "SELECCIONARSIMILAR",
            alias: &["SS", "SELECTSIMILAR"],
            grupo: Grupo::Edicion,
            ayuda: "Elegir todo lo del mismo tipo y capa que lo elegido",
            con_objetos: true,
        },
        etiqueta: "Elegir similar",
        crear: || Box::new(Similar),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("SELECCIONPREVIA"),
            nombre: "SELECCIONPREVIA",
            alias: &["SP", "SELECTPREVIOUS"],
            grupo: Grupo::Edicion,
            ayuda: "Volver a elegir lo de la ultima orden",
            con_objetos: false,
        },
        etiqueta: "Elegir previo",
        crear: || Box::new(Previa),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("SELECCIONARCAPA"),
            nombre: "SELECCIONARCAPA",
            alias: &["SLC", "SELECTLAYER"],
            grupo: Grupo::Edicion,
            ayuda: "Elegir todo lo de una capa (por su nombre o por un objeto)",
            con_objetos: false,
        },
        etiqueta: "Elegir capa",
        crear: || Box::new(DeCapa),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("INVERTIRSELECCION"),
            nombre: "INVERTIRSELECCION",
            alias: &["INV", "SELECTINVERSE"],
            grupo: Grupo::Edicion,
            ayuda: "Elegir lo que no esta elegido",
            con_objetos: false,
        },
        etiqueta: "Invertir",
        crear: || Box::new(Invertir),
    },
];

/// Deja `hs` como lo elegido y lo dice.
fn elegir(cx: &mut Contexto, hs: Vec<Handle>) -> Flujo {
    cx.decir(format!("{} objetos elegidos", hs.len()));
    *cx.seleccion = hs;
    Flujo::Termina
}

struct Similar;

impl Herramienta for Similar {
    fn indicacion(&self, _cx: &Lectura) -> String {
        String::new()
    }

    fn al_tener_objetos(&mut self, cx: &mut Contexto) -> Flujo {
        let hs = sr::similares(cx.dibujo, cx.seleccion);
        elegir(cx, hs)
    }
}

struct Previa;

impl Herramienta for Previa {
    fn indicacion(&self, _cx: &Lectura) -> String {
        String::new()
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        let hs = sr::vigentes(cx.dibujo, &cx.memoria.seleccion_previa);
        if hs.is_empty() {
            cx.decir("No hay una seleccion previa");
            return Flujo::Termina;
        }
        elegir(cx, hs)
    }
}

struct DeCapa;

impl DeCapa {
    fn con_nombre(cx: &mut Contexto, nombre: &str) -> Flujo {
        let Some(capa) = cx
            .dibujo
            .doc
            .layers
            .get(nombre.trim())
            .map(|l| l.name.clone())
        else {
            cx.decir(format!("No hay una capa «{}»", nombre.trim()));
            return Flujo::Sigue;
        };
        let hs = sr::de_capas(cx.dibujo, &[capa]);
        elegir(cx, hs)
    }
}

impl Herramienta for DeCapa {
    fn indicacion(&self, cx: &Lectura) -> String {
        format!(
            "Designe un objeto o escriba la capa <{}>:",
            cx.dibujo.capa_actual()
        )
    }

    fn pide_objeto(&self) -> bool {
        true
    }

    fn objeto(&mut self, cx: &mut Contexto, h: Handle, _p: P) -> Flujo {
        let capas = sr::capas_de(cx.dibujo, &[h]);
        let hs = sr::de_capas(cx.dibujo, &capas);
        elegir(cx, hs)
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        DeCapa::con_nombre(cx, t)
    }

    /// Una capa que se llama con un numero («0», «100»).
    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        DeCapa::con_nombre(cx, &v.to_string())
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        let actual = cx.dibujo.capa_actual();
        DeCapa::con_nombre(cx, &actual)
    }
}

struct Invertir;

impl Herramienta for Invertir {
    fn indicacion(&self, _cx: &Lectura) -> String {
        String::new()
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        let hs = sr::invertir(cx.dibujo, cx.seleccion);
        elegir(cx, hs)
    }
}

#[cfg(test)]
mod pruebas {
    use crate::{Dibujo, Sesion};

    /// Dos lineas y un circulo en la capa 0 y una linea en A-MUROS.
    fn sesion() -> Sesion {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        for (a, b) in [("0,0", "100,0"), ("0,10", "100,10")] {
            s.escribir("l");
            s.escribir(a);
            s.escribir(b);
            s.escribir("");
        }
        s.escribir("c");
        s.escribir("500,500");
        s.escribir("10");
        s.dibujo.poner_capa_actual("A-MUROS");
        s.escribir("l");
        s.escribir("0,50");
        s.escribir("100,50");
        s.escribir("");
        s.dibujo.poner_capa_actual("0");
        s
    }

    #[test]
    fn similar_a_una_linea() {
        let mut s = sesion();
        s.clic([50.0, 0.0], 1.0);
        assert_eq!(s.seleccion.len(), 1);
        s.escribir("ss");
        assert_eq!(s.seleccion.len(), 2);
        assert_eq!(s.activa(), None);
    }

    #[test]
    fn similar_sin_nada_pide_objetos() {
        let mut s = sesion();
        s.escribir("ss");
        assert!(s.pide_objetos());
        s.clic([500.0, 510.0], 1.0);
        s.escribir("");
        assert_eq!(s.seleccion.len(), 1);
        assert_eq!(s.activa(), None);
    }

    #[test]
    fn previa_tras_una_orden() {
        let mut s = sesion();
        s.escribir("sp");
        assert!(s.registro.last().unwrap().contains("No hay"));
        s.clic([50.0, 0.0], 1.0);
        s.escribir("m");
        s.escribir("0,0");
        s.escribir("0,-5");
        s.escape();
        assert!(s.seleccion.is_empty());
        // Elegir otra cosa sin orden no cambia la previa.
        s.clic([50.0, 50.0], 1.0);
        s.escape();
        s.escribir("sp");
        assert_eq!(s.seleccion.len(), 1);
        let e = s.dibujo.entidad(s.seleccion[0]).unwrap();
        assert_eq!(e.common().layer, "0");
    }

    #[test]
    fn por_capa_escrita_por_objeto_y_por_enter() {
        let mut s = sesion();
        s.escribir("slc");
        s.escribir("a-muros");
        assert_eq!(s.seleccion.len(), 1);
        s.escribir("slc");
        s.escribir("no-existe");
        assert!(s.activa().is_some());
        s.clic([50.0, 10.0], 1.0);
        assert_eq!(s.seleccion.len(), 3);
        s.seleccion.clear();
        s.escribir("slc");
        s.escribir("0");
        assert_eq!(s.seleccion.len(), 3);
        s.seleccion.clear();
        s.escribir("slc");
        s.escribir("");
        assert_eq!(s.seleccion.len(), 3);
    }

    #[test]
    fn invertir_lo_elegido() {
        let mut s = sesion();
        s.clic([50.0, 0.0], 1.0);
        s.escribir("inv");
        assert_eq!(s.seleccion.len(), 3);
        s.elegir_todo();
        s.escribir("inv");
        assert!(s.seleccion.is_empty());
    }
}
