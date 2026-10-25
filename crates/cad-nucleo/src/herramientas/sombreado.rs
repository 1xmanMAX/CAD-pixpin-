//! **SOMBREADO**: rellenar un recinto con un patron (solido, ANSI31, ANSI37
//! o arena) con un clic dentro, o eligiendo contornos cerrados.

use opencadcodec::{EntityType, Handle};

use crate::contorno;
use crate::especialidad as esp;
use crate::geo::P;
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, Registro};
use crate::ordenes::{Grupo, Id, Orden};
use crate::patrones::{self, Anillo, Patron};

pub const REGISTROS: &[Registro] = &[Registro {
    orden: Orden {
        id: Id::Otra("SOMBREADO"),
        nombre: "SOMBREADO",
        alias: &["SO", "SOM", "HATCH", "BH"],
        grupo: Grupo::Bloques,
        ayuda: "Rellena un recinto con un patron (clic dentro u objetos cerrados)",
        con_objetos: false,
    },
    etiqueta: "Sombreado",
    crear: || Box::new(Sombreado::default()),
}];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Espera {
    #[default]
    Nada,
    Patron,
    Escala,
    Angulo,
}

#[derive(Default)]
struct Sombreado {
    patron: Option<Patron>,
    /// `None`: la de la lamina (el patron se ve igual en el papel).
    escala: Option<f64>,
    /// En radianes.
    giro: f64,
    /// Elegir contornos cerrados en vez de un punto de dentro.
    objetos: bool,
    espera: Espera,
}

/// **El anillo de una entidad cerrada** (polilinea ligera cerrada o circulo).
pub fn anillo_de(e: &EntityType) -> Option<Anillo> {
    match e {
        EntityType::LwPolyline(p) if p.vertices.len() >= 2 => {
            let n = p.vertices.len();
            let primero = &p.vertices[0].location;
            let ultimo = &p.vertices[n - 1].location;
            let junta = (primero.x - ultimo.x).hypot(primero.y - ultimo.y) < 1e-9;
            if !p.is_closed && !(junta && n >= 4) {
                return None;
            }
            let espejo = p.normal.z < 0.0;
            let tomar = if p.is_closed { n } else { n - 1 };
            Some(
                p.vertices[..tomar]
                    .iter()
                    .map(|v| {
                        let x = if espejo { -v.location.x } else { v.location.x };
                        let b = if espejo { -v.bulge } else { v.bulge };
                        ([x, v.location.y], b)
                    })
                    .collect(),
            )
        }
        EntityType::Circle(c) => {
            let x = if c.normal.z < 0.0 {
                -c.center.x
            } else {
                c.center.x
            };
            Some(patrones::anillo_circulo([x, c.center.y], c.radius))
        }
        _ => None,
    }
}

impl Sombreado {
    fn patron(&self) -> Patron {
        self.patron.unwrap_or(Patron::Ansi31)
    }

    fn escala(&self, cx: &Lectura) -> f64 {
        self.escala.unwrap_or_else(|| {
            esp::unidades_por_mm(
                cx.ajustes.escala_lamina,
                cx.dibujo.doc.header.insertion_units,
            )
        })
    }

    fn crear(
        &self,
        cx: &mut Contexto,
        exteriores: &[Anillo],
        islas: &[Anillo],
        semilla: Option<P>,
    ) {
        let s = self.escala(&cx.leer());
        let h = patrones::sombreado(exteriores, islas, self.patron(), s, self.giro, semilla);
        cx.crear("SOMBREADO", vec![h], None);
        cx.decir(format!(
            "Sombreado {} con {} contorno(s) y {} isla(s)",
            self.patron().etiqueta(),
            exteriores.len(),
            islas.len()
        ));
    }

    fn de_objetos(&self, cx: &mut Contexto, hs: &[Handle]) -> Flujo {
        let anillos: Vec<Anillo> = hs
            .iter()
            .filter_map(|h| anillo_de(cx.dibujo.entidad(*h)?))
            .collect();
        if anillos.is_empty() {
            cx.decir("No es un contorno cerrado (polilinea cerrada o circulo)");
        } else {
            self.crear(cx, &anillos, &[], None);
        }
        Flujo::Sigue
    }
}

impl Herramienta for Sombreado {
    fn indicacion(&self, cx: &Lectura) -> String {
        match self.espera {
            Espera::Patron => "Patron [SOLIDO/ANSI31/ANSI37/ARENA]:".into(),
            Espera::Escala => format!("Escala del patron <{}>:", self.escala(cx)),
            Espera::Angulo => format!("Angulo del patron <{}>:", self.giro.to_degrees()),
            Espera::Nada => {
                let pide = if self.objetos {
                    "Elija contornos cerrados [Interior/Patron/Escala/Angulo]"
                } else {
                    "Punto interior [Objetos/Patron/Escala/Angulo]"
                };
                format!(
                    "{pide} ({}, escala {}, {}°):",
                    self.patron().etiqueta(),
                    self.escala(cx),
                    self.giro.to_degrees()
                )
            }
        }
    }

    fn pide_objeto(&self) -> bool {
        self.objetos && self.espera == Espera::Nada
    }

    fn pide_texto(&self) -> bool {
        self.espera == Espera::Patron
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        if let Some(p) = Patron::de(t) {
            self.patron = Some(p);
            self.espera = Espera::Nada;
            return Flujo::Sigue;
        }
        if self.espera == Espera::Patron {
            cx.decir(format!(
                "Patron desconocido «{}»: {}",
                t.trim(),
                Patron::TODOS
                    .iter()
                    .map(|p| p.etiqueta())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            return Flujo::Sigue;
        }
        match crate::ordenes::sin_tildes(t.trim()).to_uppercase().as_str() {
            "O" | "OBJETOS" => self.objetos = true,
            "I" | "INTERIOR" | "PUNTO" => self.objetos = false,
            "P" | "PATRON" => self.espera = Espera::Patron,
            "E" | "ESCALA" => self.espera = Espera::Escala,
            "A" | "ANGULO" => self.espera = Espera::Angulo,
            _ => return Flujo::NoEntiende,
        }
        Flujo::Sigue
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        match self.espera {
            Espera::Escala if v > 0.0 => self.escala = Some(v),
            Espera::Escala => {
                cx.decir("La escala tiene que ser mayor que cero");
                return Flujo::Sigue;
            }
            Espera::Angulo => self.giro = v.to_radians(),
            _ => return Flujo::NoEntiende,
        }
        self.espera = Espera::Nada;
        Flujo::Sigue
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        if self.espera != Espera::Nada {
            return Flujo::NoEntiende;
        }
        match contorno::buscar(cx.dibujo, p) {
            Some(c) => self.crear(cx, &[c.exterior], &c.islas, Some(p)),
            None => cx.decir("No hay un contorno cerrado alrededor del punto"),
        }
        Flujo::Sigue
    }

    fn objeto(&mut self, cx: &mut Contexto, h: Handle, _p: P) -> Flujo {
        self.de_objetos(cx, &[h])
    }

    fn recuadro(&mut self, cx: &mut Contexto, hs: Vec<Handle>, _a: P, _b: P) -> Flujo {
        self.de_objetos(cx, &hs)
    }

    fn enter(&mut self, _cx: &mut Contexto) -> Flujo {
        if self.espera != Espera::Nada {
            self.espera = Espera::Nada;
            return Flujo::Sigue;
        }
        Flujo::Termina
    }
}

#[cfg(test)]
mod pruebas {
    use opencadcodec::EntityType;
    use opencadcodec::entities::{BoundaryEdge, Hatch};

    use crate::{Dibujo, Sesion};

    fn sombreados(s: &Sesion) -> Vec<Hatch> {
        s.dibujo
            .doc
            .model_space_entities()
            .filter_map(|e| match e {
                EntityType::Hatch(h) => Some(h.clone()),
                _ => None,
            })
            .collect()
    }

    fn vertices(h: &Hatch, k: usize) -> usize {
        match &h.paths[k].edges[0] {
            BoundaryEdge::Polyline(p) => p.vertices.len(),
            _ => 0,
        }
    }

    #[test]
    fn dentro_de_un_rectangulo_con_patron_y_escala() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("rec");
        s.escribir("0,0");
        s.escribir("4000,3000");
        s.escribir("so");
        s.escribir("e");
        s.escribir("20");
        s.escribir("a");
        s.escribir("15");
        s.escribir("ansi37");
        s.escribir("1000,1000");
        s.escribir("");
        assert_eq!(s.activa(), None);
        let h = sombreados(&s);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].pattern.name, "ANSI37");
        assert_eq!(h[0].pattern.lines.len(), 2);
        assert!(!h[0].is_solid);
        assert_eq!(h[0].pattern_scale, 20.0);
        assert!((h[0].pattern.lines[0].angle - 60f64.to_radians()).abs() < 1e-12);
        assert_eq!(vertices(&h[0], 0), 4);
        // Se deshace de una vez.
        s.escribir("u");
        assert!(sombreados(&s).is_empty());
    }

    #[test]
    fn dentro_de_un_circulo_solido() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("c");
        s.escribir("0,0");
        s.escribir("500");
        s.escribir("so");
        s.escribir("p");
        s.escribir("solido");
        s.escribir("100,100");
        let h = sombreados(&s);
        assert_eq!(h.len(), 1);
        assert!(h[0].is_solid && h[0].pattern.name == "SOLID");
        assert_eq!(vertices(&h[0], 0), 2);
    }

    #[test]
    fn fuera_de_todo_no_sombrea() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("rec");
        s.escribir("0,0");
        s.escribir("100,100");
        s.escribir("rec");
        s.escribir("200,0");
        s.escribir("300,100");
        s.escribir("so");
        s.escribir("500,500");
        // Entre los dos rectangulos: dentro de lo dibujado pero sin contorno.
        s.escribir("150,50");
        assert!(sombreados(&s).is_empty());
        assert!(s.registro.last().unwrap().contains("No hay un contorno"));
        // Y un patron que no existe se dice.
        s.escribir("p");
        s.escribir("ladrillo");
        assert!(s.registro.last().unwrap().contains("desconocido"));
    }

    #[test]
    fn con_objetos_y_con_islas() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("rec");
        s.escribir("0,0");
        s.escribir("1000,1000");
        s.escribir("c");
        s.escribir("500,500");
        s.escribir("100");
        s.escribir("so");
        // Entre el rectangulo y el circulo: el circulo es una isla.
        s.escribir("100,100");
        let h = sombreados(&s);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].paths.len(), 2);
        s.escribir("");
        // Con objetos: el circulo solo.
        s.escribir("u");
        s.escribir("so");
        s.escribir("o");
        assert!(s.pide_objetos());
        s.clic([600.0, 500.0], 1.0);
        s.escribir("");
        let h = sombreados(&s);
        assert_eq!(h.len(), 1);
        assert_eq!((h[0].paths.len(), vertices(&h[0], 0)), (1, 2));
    }
}
