//! **ZOOM** y **ZE**: el nucleo no mueve la camara de la ventana, asi que la
//! orden deja un [`PedidoVista`] en [`crate::herramienta::Memoria::vista`] y
//! la app lo atiende en la siguiente vuelta de su bucle.

use crate::geo::{Forma, P};
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, Previa, Registro};
use crate::ordenes::{self, Grupo, Id, Orden};

/// **Lo que una orden pide a la vista** de la ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PedidoVista {
    /// Todo lo dibujado y visible, lo mas grande que quepa.
    Extension,
    /// Lo dibujado y los limites del dibujo (aunque haya capas apagadas).
    Todo,
    /// Lo que cabe entre dos esquinas.
    Ventana(P, P),
    /// La vista anterior (la ventana guarda unas 20).
    Previa,
    /// Acercar `k` veces (`k` < 1 aleja) alrededor del centro de la vista.
    Factor(f64),
}

pub const REGISTROS: &[Registro] = &[
    Registro {
        orden: Orden {
            id: Id::Otra("ZOOM"),
            nombre: "ZOOM",
            alias: &["Z"],
            grupo: Grupo::Vista,
            ayuda: "Ver mas cerca o mas lejos: Extension, Ventana, Previo, Todo o un factor",
            con_objetos: false,
        },
        etiqueta: "Zoom",
        crear: || Box::new(Zoom::default()),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("ZOOMEXTENSION"),
            nombre: "ZOOMEXTENSION",
            alias: &["ZE"],
            grupo: Grupo::Vista,
            ayuda: "Ver todo lo dibujado (doble clic central)",
            con_objetos: false,
        },
        etiqueta: "Ver todo",
        crear: || Box::new(ZoomExtension),
    },
];

/// Un factor escrito como `2`, `0.5`, `2x` o `0,5X`.
pub fn factor(t: &str) -> Option<f64> {
    let t = t.trim().trim_end_matches(['x', 'X']).replace(',', ".");
    let k: f64 = t.trim().parse().ok()?;
    (k.is_finite() && k > 0.0).then_some(k)
}

/// La opcion de ZOOM escrita (sin tildes ni mayusculas).
fn opcion(t: &str) -> Option<PedidoVista> {
    let up = ordenes::sin_tildes(t.trim()).to_uppercase();
    match up.as_str() {
        "E" | "EXTENSION" | "EXTENTS" => Some(PedidoVista::Extension),
        "T" | "TODO" | "ALL" => Some(PedidoVista::Todo),
        "P" | "PREVIO" | "PREVIOUS" => Some(PedidoVista::Previa),
        _ => factor(&up).map(PedidoVista::Factor),
    }
}

#[derive(Default)]
struct Zoom {
    /// La primera esquina de la ventana; `ventana` si se pidio con `V`.
    esquina: Option<P>,
    ventana: bool,
}

impl Zoom {
    fn pedir(cx: &mut Contexto, p: PedidoVista) -> Flujo {
        cx.memoria.vista = Some(p);
        Flujo::Termina
    }
}

impl Herramienta for Zoom {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match (self.esquina, self.ventana) {
            (Some(_), _) => "Esquina opuesta:".into(),
            (None, true) => "Primera esquina:".into(),
            (None, false) => {
                "Esquina de ventana, factor (2, 0.5) o [Extension/Ventana/Previo/Todo] <Extension>:"
                    .into()
            }
        }
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        match self.esquina {
            None => {
                self.esquina = Some(p);
                Flujo::Sigue
            }
            Some(a) if (a[0] - p[0]).abs() > 1e-9 && (a[1] - p[1]).abs() > 1e-9 => {
                Zoom::pedir(cx, PedidoVista::Ventana(a, p))
            }
            Some(_) => {
                cx.decir("La ventana no tiene area: elija otra esquina");
                Flujo::Sigue
            }
        }
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        if self.esquina.is_some() || self.ventana {
            return Flujo::NoEntiende;
        }
        if v.is_finite() && v > 0.0 {
            Zoom::pedir(cx, PedidoVista::Factor(v))
        } else {
            cx.decir("El factor tiene que ser mayor que cero");
            Flujo::Sigue
        }
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        if self.esquina.is_some() || self.ventana {
            return Flujo::NoEntiende;
        }
        let up = ordenes::sin_tildes(t.trim()).to_uppercase();
        if up == "V" || up == "VENTANA" || up == "W" || up == "WINDOW" {
            self.ventana = true;
            return Flujo::Sigue;
        }
        match opcion(t) {
            Some(p) => Zoom::pedir(cx, p),
            None => Flujo::NoEntiende,
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        if self.esquina.is_some() || self.ventana {
            return Flujo::Termina;
        }
        Zoom::pedir(cx, PedidoVista::Extension)
    }

    fn previa(&self, _cx: &Lectura, c: P) -> Previa {
        let Some(a) = self.esquina else {
            return Previa::default();
        };
        let esquinas = [a, [c[0], a[1]], c, [a[0], c[1]]];
        Previa {
            formas: (0..4)
                .map(|i| Forma::Segmento {
                    a: esquinas[i],
                    b: esquinas[(i + 1) % 4],
                })
                .collect(),
            ..Default::default()
        }
    }
}

/// ZE: ver todo, sin preguntar.
struct ZoomExtension;

impl Herramienta for ZoomExtension {
    fn indicacion(&self, _cx: &Lectura) -> String {
        String::new()
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        Zoom::pedir(cx, PedidoVista::Extension)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::{Dibujo, Sesion};

    fn sesion() -> Sesion {
        Sesion::nueva(Dibujo::nuevo())
    }

    #[test]
    fn factores_escritos() {
        assert_eq!(factor("2"), Some(2.0));
        assert_eq!(factor("0,5x"), Some(0.5));
        assert_eq!(factor(" 3X "), Some(3.0));
        assert_eq!(factor("0"), None);
        assert_eq!(factor("-2"), None);
        assert_eq!(factor("x"), None);
        assert_eq!(factor("dos"), None);
    }

    #[test]
    fn extension_por_enter_y_por_ze() {
        let mut s = sesion();
        s.escribir("z");
        assert!(s.memoria.vista.is_none());
        s.escribir("");
        assert_eq!(s.memoria.vista.take(), Some(PedidoVista::Extension));
        assert_eq!(s.activa(), None);
        s.escribir("ze");
        assert_eq!(s.memoria.vista.take(), Some(PedidoVista::Extension));
        assert_eq!(s.activa(), None);
    }

    #[test]
    fn opciones_y_factor() {
        let mut s = sesion();
        for (t, p) in [
            ("p", PedidoVista::Previa),
            ("todo", PedidoVista::Todo),
            ("2", PedidoVista::Factor(2.0)),
            ("0.5x", PedidoVista::Factor(0.5)),
            ("extensión", PedidoVista::Extension),
        ] {
            s.escribir("zoom");
            s.escribir(t);
            assert_eq!(s.memoria.vista.take(), Some(p), "{t}");
            assert_eq!(s.activa(), None);
        }
    }

    #[test]
    fn ventana_por_dos_esquinas_con_previa() {
        let mut s = sesion();
        s.escribir("z");
        s.escribir("v");
        s.clic([0.0, 0.0], 1.0);
        assert_eq!(s.previa([10.0, 5.0]).formas.len(), 4);
        // Una esquina en la misma vertical: no hay ventana, sigue pidiendo.
        s.clic([0.0, 50.0], 1.0);
        assert!(s.memoria.vista.is_none());
        s.clic([100.0, 50.0], 1.0);
        assert_eq!(
            s.memoria.vista.take(),
            Some(PedidoVista::Ventana([0.0, 0.0], [100.0, 50.0]))
        );
        // Un clic directo tambien empieza la ventana.
        s.escribir("z");
        s.clic([1.0, 1.0], 1.0);
        s.clic([2.0, 3.0], 1.0);
        assert!(matches!(
            s.memoria.vista.take(),
            Some(PedidoVista::Ventana(..))
        ));
    }

    #[test]
    fn lo_que_no_vale_no_pide_nada() {
        let mut s = sesion();
        s.escribir("z");
        s.escribir("0");
        s.escribir("-3");
        s.escribir("nada");
        assert!(s.memoria.vista.is_none());
        assert!(s.activa().is_some());
        s.escape();
        assert!(s.memoria.vista.is_none());
    }
}
