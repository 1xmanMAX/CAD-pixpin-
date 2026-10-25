//! **SOMBREADO**: rellenar un recinto con un patron (solido, ANSI31, ANSI37
//! o arena) con un clic dentro, o eligiendo contornos cerrados. El
//! sombreado queda asociado a sus bordes (sigue a lo que lo cierra al
//! moverlo o estirarlo, ver [`crate::asociar`]) y, con un clic dentro, se
//! cierran los huecos pequenos entre extremos (opcion Tolerancia).

use opencadcodec::{EntityType, Handle};

use crate::asociar;
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
    Tolerancia,
}

#[derive(Default)]
struct Sombreado {
    patron: Option<Patron>,
    /// `None`: la de la lamina (el patron se ve igual en el papel).
    escala: Option<f64>,
    /// En radianes.
    giro: f64,
    /// Los huecos que se cierran al buscar el contorno (`None`: segun el
    /// tamano del dibujo, [`contorno::hueco_por_defecto`]).
    hueco: Option<f64>,
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

    fn hueco(&self, cx: &Lectura) -> f64 {
        self.hueco
            .unwrap_or_else(|| contorno::hueco_por_defecto(cx.dibujo))
    }

    /// Crea el sombreado, asociado a `bordes` (las entidades de cada anillo,
    /// los exteriores y luego las islas).
    fn crear(
        &self,
        cx: &mut Contexto,
        exteriores: &[Anillo],
        islas: &[Anillo],
        semilla: Option<P>,
        bordes: &[Vec<Handle>],
    ) {
        let s = self.escala(&cx.leer());
        let hueco = if semilla.is_some() {
            self.hueco(&cx.leer())
        } else {
            0.0
        };
        let h = patrones::sombreado(exteriores, islas, self.patron(), s, self.giro, semilla);
        asociar::asegurar_app(&mut cx.dibujo.doc);
        let app = asociar::app_handle(&cx.dibujo.doc);
        let capa = cx.dibujo.capa_actual();
        cx.dibujo.editar("SOMBREADO", |t| {
            if let Some(h) = t.anadir_en(h, &capa) {
                t.cambiar(h, |e| asociar::asociar_sombreado(app, e, bordes, hueco));
            }
        });
        cx.decir(format!(
            "Sombreado {} con {} contorno(s) y {} isla(s)",
            self.patron().etiqueta(),
            exteriores.len(),
            islas.len()
        ));
    }

    fn de_objetos(&self, cx: &mut Contexto, hs: &[Handle]) -> Flujo {
        let (anillos, bordes): (Vec<Anillo>, Vec<Vec<Handle>>) = hs
            .iter()
            .filter_map(|h| Some((anillo_de(cx.dibujo.entidad(*h)?)?, vec![*h])))
            .unzip();
        if anillos.is_empty() {
            cx.decir("No es un contorno cerrado (polilinea cerrada o circulo)");
        } else {
            self.crear(cx, &anillos, &[], None, &bordes);
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
            Espera::Tolerancia => format!(
                "Huecos que se cierran (0: ninguno) <{}>:",
                cx.medida(self.hueco(cx))
            ),
            Espera::Nada => {
                let pide = if self.objetos {
                    "Elija contornos cerrados [Interior/Patron/Escala/Angulo]"
                } else {
                    "Punto interior [Objetos/Patron/Escala/Angulo/Tolerancia]"
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
            "T" | "TOLERANCIA" | "HUECO" => self.espera = Espera::Tolerancia,
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
            Espera::Tolerancia if v >= 0.0 && v.is_finite() => self.hueco = Some(v),
            Espera::Tolerancia => {
                cx.decir("La tolerancia no puede ser negativa");
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
        let hueco = self.hueco(&cx.leer());
        match contorno::buscar(cx.dibujo, p, hueco) {
            Some(c) => {
                let mut bordes = vec![c.bordes];
                bordes.extend(c.bordes_islas);
                self.crear(cx, &[c.exterior], &c.islas, Some(p), &bordes)
            }
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

    fn primera(s: &Sesion, f: fn(&EntityType) -> bool) -> opencadcodec::Handle {
        s.dibujo
            .doc
            .model_space_entities()
            .find(|e| f(e))
            .unwrap()
            .common()
            .handle
    }

    /// El area del primer camino de un sombreado (con sus abombados planos).
    fn area(h: &Hatch) -> f64 {
        let BoundaryEdge::Polyline(p) = &h.paths[0].edges[0] else {
            return 0.0;
        };
        let pts: Vec<[f64; 2]> = p.vertices.iter().map(|v| [v.x, v.y]).collect();
        crate::geo::area_poligono(&pts).abs()
    }

    #[test]
    fn el_sombreado_sigue_a_su_borde_al_moverlo_y_estirarlo() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("rec");
        s.escribir("0,0");
        s.escribir("4000,3000");
        s.escribir("so");
        s.escribir("1000,1000");
        s.escribir("");
        let pl = primera(&s, |e| matches!(e, EntityType::LwPolyline(_)));
        let h = sombreados(&s);
        assert!(h[0].is_associative);
        assert_eq!(h[0].paths[0].boundary_handles, vec![pl]);
        // Mover el rectangulo: el sombreado va con el (en el mismo paso).
        s.seleccion = vec![pl];
        s.escribir("m");
        s.escribir("0,0");
        s.escribir("10000,0");
        let h = sombreados(&s);
        assert!(h[0].is_associative);
        let BoundaryEdge::Polyline(p) = &h[0].paths[0].edges[0] else {
            panic!()
        };
        assert!(p.vertices.iter().all(|v| v.x >= 10000.0 - 1e-6));
        assert!((area(&h[0]) - 12e6).abs() < 1.0);
        // Estirar una esquina con un pinzamiento: se rehace el contorno.
        s.seleccion = vec![pl];
        assert!(s.tomar_pinzamiento([14000.0, 3000.0], 2.0));
        s.clic([16000.0, 3000.0], 2.0);
        let h = sombreados(&s);
        assert!(h[0].is_associative);
        assert!((area(&h[0]) - 15e6).abs() < 1.0, "{}", area(&h[0]));
        // Deshacer: el borde y el sombreado como estaban.
        s.escribir("u");
        assert!((area(&sombreados(&s)[0]) - 12e6).abs() < 1.0);
        s.escribir("u");
        let h = sombreados(&s);
        let BoundaryEdge::Polyline(p) = &h[0].paths[0].edges[0] else {
            panic!()
        };
        assert!(p.vertices.iter().all(|v| v.x <= 4000.0 + 1e-6));
    }

    #[test]
    fn borrar_el_borde_o_mover_el_sombreado_solo_lo_suelta() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("c");
        s.escribir("0,0");
        s.escribir("500");
        s.escribir("so");
        s.escribir("o");
        s.clic([500.0, 0.0], 1.0);
        s.escribir("");
        let c = primera(&s, |e| matches!(e, EntityType::Circle(_)));
        let hh = primera(&s, |e| matches!(e, EntityType::Hatch(_)));
        assert!(sombreados(&s)[0].is_associative);
        // Escalar el circulo: el sombreado (de objetos) lo sigue.
        s.seleccion = vec![c];
        s.escribir("es");
        s.escribir("0,0");
        s.escribir("2");
        let h = sombreados(&s);
        let BoundaryEdge::Polyline(p) = &h[0].paths[0].edges[0] else {
            panic!()
        };
        assert!((p.vertices[0].x - 1000.0).abs() < 1e-6, "{:?}", p.vertices);
        // Borrarlo: el sombreado se queda, ya sin asociar.
        s.seleccion = vec![c];
        s.escribir("e");
        let h = sombreados(&s);
        assert_eq!(h.len(), 1);
        assert!(!h[0].is_associative);
        assert!(h[0].paths.iter().all(|p| p.boundary_handles.is_empty()));
        s.escribir("u");
        assert!(sombreados(&s)[0].is_associative);
        // Mover el sombreado solo: tambien se suelta (caso negativo).
        s.seleccion = vec![hh];
        s.escribir("m");
        s.escribir("0,0");
        s.escribir("5000,0");
        assert!(!sombreados(&s)[0].is_associative);
        s.seleccion = vec![c];
        s.escribir("m");
        s.escribir("0,0");
        s.escribir("0,5000");
        let h = sombreados(&s);
        let BoundaryEdge::Polyline(p) = &h[0].paths[0].edges[0] else {
            panic!()
        };
        assert!(p.vertices[0].y.abs() < 1e-6, "ya no lo sigue");
    }

    #[test]
    fn lineas_con_huecos_y_la_opcion_tolerancia() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        // Un cuadrado de 1000 con huecos de 20 en dos esquinas.
        for (a, b) in [
            ("0,0", "980,0"),
            ("1000,20", "1000,1000"),
            ("1000,1000", "0,1000"),
            ("0,1000", "0,0"),
        ] {
            s.escribir("l");
            s.escribir(a);
            s.escribir(b);
            s.escribir("");
        }
        s.escribir("so");
        // Por defecto (una diezmilesima del dibujo) no llega.
        s.escribir("500,500");
        assert!(sombreados(&s).is_empty());
        s.escribir("t");
        assert!(s.indicacion().contains("Huecos"));
        s.escribir("-1");
        assert!(s.registro.last().unwrap().contains("negativa"));
        s.escribir("30");
        s.escribir("500,500");
        s.escribir("");
        let h = sombreados(&s);
        assert_eq!(h.len(), 1);
        assert!((area(&h[0]) - 1e6).abs() < 500.0, "{}", area(&h[0]));
        // Sus bordes son las cuatro lineas y se acuerda de la tolerancia.
        assert_eq!(h[0].paths[0].boundary_handles.len(), 4);
        let e = EntityType::Hatch(h[0].clone());
        assert_eq!(crate::asociar::hueco_de(&e), 30.0);
        // Estirar una linea (con el hueco): se rehace con la misma tolerancia.
        let arriba = s
            .dibujo
            .doc
            .model_space_entities()
            .find(|e| matches!(e, EntityType::Line(l) if l.start.y == 1000.0 && l.end.y == 1000.0))
            .unwrap()
            .common()
            .handle;
        // Subirla 10: abre huecos de 10 en sus extremos, que se cierran.
        s.seleccion = vec![arriba];
        s.escribir("m");
        s.escribir("0,0");
        s.escribir("0,10");
        let h = sombreados(&s);
        assert!(h[0].is_associative, "sigue cerrado");
        assert!((area(&h[0]) - 1.01e6).abs() < 500.0, "{}", area(&h[0]));
        // Caso negativo: 500 mas arriba ya no cierra y se suelta.
        s.seleccion = vec![arriba];
        s.escribir("m");
        s.escribir("0,0");
        s.escribir("0,500");
        let h = sombreados(&s);
        assert!(!h[0].is_associative);
        assert!((area(&h[0]) - 1.01e6).abs() < 500.0, "se queda como estaba");
    }

    #[test]
    fn los_sombreados_asociativos_vuelven_en_dwg_y_dxf_y_siguen_valiendo() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("rec");
        s.escribir("0,0");
        s.escribir("4000,3000");
        s.escribir("so");
        s.escribir("1000,1000");
        s.escribir("");
        let dir = std::env::temp_dir().join(format!("cad-nucleo-somb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for ext in ["dwg", "dxf"] {
            let ruta = dir.join(format!("somb.{ext}"));
            s.dibujo.guardar_como(&ruta).unwrap();
            let mut s2 = Sesion::nueva(Dibujo::abrir(&ruta).unwrap());
            let pl = primera(&s2, |e| matches!(e, EntityType::LwPolyline(_)));
            let h = sombreados(&s2);
            assert!(h[0].is_associative, "{ext}");
            assert_eq!(h[0].paths[0].boundary_handles, vec![pl], "{ext}");
            assert_eq!(h[0].seed_points.len(), 1, "{ext}");
            s2.seleccion = vec![pl];
            assert!(s2.tomar_pinzamiento([4000.0, 3000.0], 2.0), "{ext}");
            s2.clic([6000.0, 3000.0], 2.0);
            assert!(
                (area(&sombreados(&s2)[0]) - 15e6).abs() < 1.0,
                "{ext}: {}",
                area(&sombreados(&s2)[0])
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
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
