//! **PUNTO**: el ejemplo mas pequeno de una orden enchufable (y util: los
//! puntos de replanteo, de nivel, de referencia).

use opencadcodec::entities::Point;

use crate::formas::v3;
use crate::geo::P;
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, Previa, Registro};
use crate::ordenes::{Grupo, Id, Orden};

pub const REGISTROS: &[Registro] = &[Registro {
    orden: Orden {
        id: Id::Otra("PUNTO"),
        nombre: "PUNTO",
        alias: &["PO", "POINT"],
        grupo: Grupo::Dibujo,
        ayuda: "Puntos sueltos (replanteo, referencia)",
        con_objetos: false,
    },
    etiqueta: "Punto",
    crear: || Box::new(Puntos),
}];

/// Pone un punto en cada clic hasta Enter.
struct Puntos;

impl Herramienta for Puntos {
    fn indicacion(&self, _cx: &Lectura) -> String {
        "Situe el punto (Enter termina):".into()
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        let mut punto = Point::new();
        punto.location = v3(p);
        cx.crear("PUNTO", vec![opencadcodec::EntityType::Point(punto)], None);
        Flujo::Sigue
    }

    fn previa(&self, _cx: &Lectura, _cursor: P) -> Previa {
        Previa::default()
    }
}

#[cfg(test)]
mod pruebas {
    use crate::{Dibujo, Sesion};

    #[test]
    fn puntos_hasta_enter() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("po");
        s.escribir("0,0");
        s.escribir("10,10");
        s.escribir("");
        assert_eq!(s.activa(), None);
        assert_eq!(s.dibujo.doc.model_space_entities().count(), 2);
    }
}
