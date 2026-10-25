//! **BLOQUE, INSERTAR y BIBLIOTECA**: hacer un bloque con lo elegido,
//! ponerlo (o uno de la biblioteca) en su punto, a su escala y con su giro,
//! y ver las piezas que trae la biblioteca.

use crate::biblioteca::{self, Pieza};
use crate::bloque;
use crate::especialidad as esp;
use crate::geo::{self, Forma, P};
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, Previa, Registro};
use crate::ordenes::{Grupo, Id, Orden};

pub const REGISTROS: &[Registro] = &[
    Registro {
        orden: Orden {
            id: Id::Otra("BLOQUE"),
            nombre: "BLOQUE",
            alias: &["B", "BLOCK"],
            grupo: Grupo::Bloques,
            ayuda: "Hace un bloque con lo elegido (nombre y punto base)",
            con_objetos: true,
        },
        etiqueta: "Bloque",
        crear: || Box::new(Bloque::default()),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("INSERTAR"),
            nombre: "INSERTAR",
            alias: &["I", "INSERT"],
            grupo: Grupo::Bloques,
            ayuda: "Inserta un bloque del dibujo o de la biblioteca",
            con_objetos: false,
        },
        etiqueta: "Insertar",
        crear: || Box::new(Insertar::nuevo(false)),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("BIBLIOTECA"),
            nombre: "BIBLIOTECA",
            alias: &["BIB", "LIBRARY"],
            grupo: Grupo::Bloques,
            ayuda: "Las piezas de la biblioteca (sanitarios, mobiliario, simbolos)",
            con_objetos: false,
        },
        etiqueta: "Biblioteca",
        crear: || Box::new(Insertar::nuevo(true)),
    },
];

// ---------------------------------------------------------------- BLOQUE

/// Pide el nombre y el punto base, y cambia lo elegido por el bloque.
#[derive(Default)]
struct Bloque {
    nombre: Option<String>,
    /// Dejar los originales donde estan (y no poner el INSERT).
    conservar: bool,
}

impl Bloque {
    fn hacer(&mut self, cx: &mut Contexto, base: P) -> Flujo {
        let Some(nombre) = self.nombre.clone() else {
            return Flujo::Sigue;
        };
        let elegidas = cx.elegidas();
        if elegidas.is_empty() {
            cx.decir("No hay objetos elegidos");
            return Flujo::Termina;
        }
        let entidades = elegidas.iter().map(|(_, e)| e.clone()).collect();
        if let Err(e) = bloque::definir(&mut cx.dibujo.doc, &nombre, entidades, base) {
            cx.decir(e);
            return Flujo::Termina;
        }
        cx.dibujo.tocar();
        let n = elegidas.len();
        if self.conservar {
            cx.seleccion.clear();
            cx.decir(format!(
                "Bloque «{nombre}» definido con {n} objetos (los originales se quedan)"
            ));
            return Flujo::Termina;
        }
        let capa = cx.dibujo.capa_actual();
        let h = cx.dibujo.editar("BLOQUE", |t| {
            for (h, _) in &elegidas {
                t.quitar(*h);
            }
            t.anadir_en(bloque::insercion(&nombre, base, 1.0, 0.0), &capa)
        });
        *cx.seleccion = h.into_iter().collect();
        cx.decir(format!("Bloque «{nombre}» con {n} objetos"));
        Flujo::Termina
    }
}

impl Herramienta for Bloque {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match self.nombre {
            None => "Nombre del bloque:".into(),
            Some(_) => format!(
                "Punto base [Conservar originales: {}] (C cambia):",
                if self.conservar { "Si" } else { "No" }
            ),
        }
    }

    fn pide_texto(&self) -> bool {
        self.nombre.is_none()
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        if self.nombre.is_none() {
            match bloque::validar_nombre(t) {
                Ok(n) if bloque::existe(&cx.dibujo.doc, &n) => {
                    cx.decir(format!("Ya hay un bloque «{n}»: elija otro nombre"));
                }
                Ok(n) => self.nombre = Some(n),
                Err(e) => cx.decir(e),
            }
            return Flujo::Sigue;
        }
        if t.trim().eq_ignore_ascii_case("C") {
            self.conservar = !self.conservar;
            return Flujo::Sigue;
        }
        Flujo::NoEntiende
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        if self.nombre.is_none() {
            return Flujo::NoEntiende;
        }
        self.hacer(cx, p)
    }
}

// ------------------------------------------------------- INSERTAR / BIBLIOTECA

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paso {
    Nombre,
    Punto,
    Escala,
    Giro,
}

/// Pide el bloque, el punto, la escala y el giro.
struct Insertar {
    /// BIBLIOTECA: solo las piezas de la biblioteca, y Enter las lista.
    biblioteca: bool,
    paso: Paso,
    nombre: String,
    /// La pieza de la biblioteca que aun no esta en el dibujo (se crea al insertar).
    pendiente: Option<&'static Pieza>,
    capa: Option<&'static str>,
    /// Las formas del bloque, en su sistema: se calculan una vez.
    formas: Vec<Forma>,
    punto: P,
    escala: f64,
    /// La escala de Enter (las piezas de la biblioteca van en mm).
    escala_defecto: f64,
}

/// Cuantas formas se pintan como mucho en la previa.
const PREVIA_MAX: usize = 20_000;

impl Insertar {
    fn nuevo(biblioteca: bool) -> Insertar {
        Insertar {
            biblioteca,
            paso: Paso::Nombre,
            nombre: String::new(),
            pendiente: None,
            capa: None,
            formas: Vec::new(),
            punto: [0.0, 0.0],
            escala: 1.0,
            escala_defecto: 1.0,
        }
    }

    fn listar(&self, cx: &mut Contexto) {
        if !self.biblioteca {
            let v = bloque::nombres(&cx.dibujo.doc);
            if v.is_empty() {
                cx.decir("El dibujo no tiene bloques");
            } else {
                cx.decir(format!("Bloques del dibujo: {}", v.join(", ")));
            }
        }
        cx.decir("Biblioteca:");
        for l in biblioteca::lista() {
            cx.decir(l);
        }
    }

    fn elegir(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        let t = t.trim();
        if t == "?" {
            self.listar(cx);
            return Flujo::Sigue;
        }
        let doc = &cx.dibujo.doc;
        let pieza = biblioteca::buscar(t);
        let mm = esp::unidades_por_mm(1.0, doc.header.insertion_units);
        let del_dibujo = if self.biblioteca {
            pieza.map(|p| p.nombre).filter(|n| bloque::existe(doc, n))
        } else {
            Some(t)
                .filter(|n| bloque::existe(doc, n))
                .or_else(|| pieza.map(|p| p.nombre).filter(|n| bloque::existe(doc, n)))
        };
        if let Some(n) = del_dibujo {
            let br = doc.block_records.get(n).map(|b| b.name.clone());
            self.nombre = br.unwrap_or_else(|| n.to_string());
            self.formas = bloque::formas_de(doc, &self.nombre);
            self.pendiente = None;
        } else if let Some(p) = pieza {
            self.nombre = p.nombre.to_string();
            self.formas = bloque::formas_de_lista(&(p.dibujo)());
            self.pendiente = Some(p);
        } else {
            cx.decir(format!("No hay un bloque «{t}» (escriba ? para la lista)"));
            return Flujo::Sigue;
        }
        // Las de la biblioteca: en su capa y a la escala de los mm del dibujo.
        let de_biblioteca = pieza.is_some_and(|p| p.nombre == self.nombre);
        self.capa = pieza.filter(|_| de_biblioteca).map(|p| p.capa);
        self.escala_defecto = if de_biblioteca { mm } else { 1.0 };
        self.escala = self.escala_defecto;
        self.formas.truncate(PREVIA_MAX);
        self.paso = Paso::Punto;
        Flujo::Sigue
    }

    fn poner(&mut self, cx: &mut Contexto, giro: f64) -> Flujo {
        if let Some(p) = self.pendiente.take() {
            if let Err(e) = bloque::definir(&mut cx.dibujo.doc, p.nombre, (p.dibujo)(), [0.0, 0.0])
            {
                cx.decir(e);
                return Flujo::Termina;
            }
            cx.dibujo.tocar();
        }
        let e = bloque::insercion(&self.nombre, self.punto, self.escala, giro);
        let hs = cx.crear("INSERTAR", vec![e], self.capa);
        *cx.seleccion = hs;
        cx.decir(format!("Bloque «{}» insertado", self.nombre));
        Flujo::Termina
    }

    fn formas_en(&self, p: P, s: f64, giro: f64) -> Vec<Forma> {
        self.formas
            .iter()
            .map(|f| bloque::transformar(f, p, s, giro))
            .collect()
    }
}

impl Herramienta for Insertar {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match self.paso {
            Paso::Nombre if self.biblioteca => "Pieza de la biblioteca (Enter: la lista):".into(),
            Paso::Nombre => "Nombre del bloque (? la lista):".into(),
            Paso::Punto => format!("Punto de insercion de «{}»:", self.nombre),
            Paso::Escala => format!("Escala <{}>:", self.escala_defecto),
            Paso::Giro => "Angulo de giro <0>:".into(),
        }
    }

    fn base(&self) -> Option<P> {
        matches!(self.paso, Paso::Escala | Paso::Giro).then_some(self.punto)
    }

    fn pide_texto(&self) -> bool {
        self.paso == Paso::Nombre
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        match self.paso {
            Paso::Nombre => self.elegir(cx, t),
            _ => Flujo::NoEntiende,
        }
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        match self.paso {
            Paso::Nombre => Flujo::NoEntiende,
            Paso::Punto => {
                self.punto = p;
                self.paso = Paso::Escala;
                Flujo::Sigue
            }
            Paso::Escala => {
                let d = geo::dist(self.punto, p);
                if d > geo::EPS {
                    self.escala = d;
                    self.paso = Paso::Giro;
                }
                Flujo::Sigue
            }
            Paso::Giro => self.poner(cx, geo::angulo(self.punto, p)),
        }
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        match self.paso {
            Paso::Escala if v > 0.0 => {
                self.escala = v;
                self.paso = Paso::Giro;
                Flujo::Sigue
            }
            Paso::Escala => {
                cx.decir("La escala tiene que ser mayor que cero");
                Flujo::Sigue
            }
            Paso::Giro => self.poner(cx, v.to_radians()),
            _ => Flujo::NoEntiende,
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        match self.paso {
            Paso::Nombre => {
                if self.biblioteca {
                    self.listar(cx);
                }
                Flujo::Termina
            }
            Paso::Punto => Flujo::Termina,
            Paso::Escala => {
                self.escala = self.escala_defecto;
                self.paso = Paso::Giro;
                Flujo::Sigue
            }
            Paso::Giro => self.poner(cx, 0.0),
        }
    }

    fn previa(&self, _cx: &Lectura, c: P) -> Previa {
        let formas = match self.paso {
            Paso::Nombre => Vec::new(),
            Paso::Punto => self.formas_en(c, self.escala_defecto, 0.0),
            Paso::Escala => self.formas_en(self.punto, self.escala_defecto, 0.0),
            Paso::Giro => self.formas_en(self.punto, self.escala, geo::angulo(self.punto, c)),
        };
        Previa {
            formas,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod pruebas {
    use opencadcodec::EntityType;

    use crate::ordenes::Id;
    use crate::{Dibujo, Sesion};

    fn cuantas(s: &Sesion) -> usize {
        s.dibujo.doc.model_space_entities().count()
    }

    fn inserts(s: &Sesion) -> Vec<opencadcodec::entities::Insert> {
        s.dibujo
            .doc
            .model_space_entities()
            .filter_map(|e| match e {
                EntityType::Insert(i) => Some(i.clone()),
                _ => None,
            })
            .collect()
    }

    /// Un rectangulo y un circulo, elegidos, hechos bloque «MESA» con base en 100,100.
    fn con_bloque() -> Sesion {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("rec");
        s.escribir("100,100");
        s.escribir("300,200");
        s.escribir("c");
        s.escribir("200,150");
        s.escribir("30");
        s.elegir_todo();
        s.escribir("b");
        s.escribir("Mesa");
        s.escribir("100,100");
        assert_eq!(s.activa(), None);
        s
    }

    #[test]
    fn bloque_cambia_lo_elegido_por_el_insert_y_se_deshace() {
        let mut s = con_bloque();
        assert_eq!(cuantas(&s), 1);
        let i = &inserts(&s)[0];
        assert_eq!(i.block_name, "Mesa");
        assert_eq!((i.insert_point.x, i.insert_point.y), (100.0, 100.0));
        // La definicion, relativa al punto base.
        let def: Vec<_> = s.dibujo.doc.entities_in_block("Mesa").cloned().collect();
        assert_eq!(def.len(), 2);
        assert!(def.iter().any(|e| matches!(e,
            EntityType::Circle(c) if (c.center.x, c.center.y) == (100.0, 50.0))));
        assert_eq!(s.seleccion.len(), 1);
        // Deshacer devuelve los originales (la definicion se queda).
        s.escribir("u");
        assert_eq!(cuantas(&s), 2);
        assert!(inserts(&s).is_empty());
    }

    #[test]
    fn bloque_rechaza_nombres_repetidos_o_malos_y_puede_conservar() {
        let mut s = con_bloque();
        s.escribir("l");
        s.escribir("0,0");
        s.escribir("10,0");
        s.escribir("");
        s.elegir_todo();
        s.escribir("b");
        s.escribir("MESA");
        assert!(s.registro.last().unwrap().contains("Ya hay"));
        s.escribir("a/b");
        assert!(s.registro.last().unwrap().contains("«/»"));
        s.escribir("Otro");
        s.escribir("c");
        assert!(s.indicacion().contains("Si"));
        s.escribir("0,0");
        assert_eq!(s.activa(), None);
        // Conservando: siguen los dos (el insert y la linea) y no hay insert nuevo.
        assert_eq!(cuantas(&s), 2);
        assert_eq!(s.dibujo.doc.entities_in_block("Otro").count(), 2);
    }

    #[test]
    fn insertar_con_escala_y_giro_y_descomponer() {
        let mut s = con_bloque();
        s.escribir("i");
        s.escribir("nada");
        assert!(s.registro.last().unwrap().contains("No hay"));
        s.escribir("mesa");
        // La previa sale en el cursor, sin tocar el dibujo.
        let v = s.previa([1000.0, 0.0]);
        assert_eq!(v.formas.len(), 5);
        s.escribir("1000,0");
        s.escribir("2");
        s.escribir("90");
        assert_eq!(s.activa(), None);
        let ins = inserts(&s);
        assert_eq!(ins.len(), 2);
        let i = ins.iter().find(|i| i.insert_point.x == 1000.0).unwrap();
        assert_eq!(i.x_scale(), 2.0);
        assert!((i.rotation - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        // Descomponer (X) lo deja en sus piezas, en su sitio.
        s.seleccion = vec![i.common.handle];
        s.escribir("x");
        assert_eq!(cuantas(&s), 3);
        let c = s
            .dibujo
            .doc
            .model_space_entities()
            .find_map(|e| match e {
                EntityType::Circle(c) => Some(c.clone()),
                _ => None,
            })
            .expect("el circulo sale del bloque");
        // (100,50) del bloque, por 2 y girado 90: (-100,200) desde (1000,0).
        assert!((c.center.x - 900.0).abs() < 1e-6 && (c.center.y - 200.0).abs() < 1e-6);
        assert!((c.radius - 60.0).abs() < 1e-9);
    }

    #[test]
    fn insertar_de_la_biblioteca_lo_crea_una_vez() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("i");
        s.escribir("Inodoro");
        assert!(!s.dibujo.doc.block_records.contains("INODORO"));
        assert!(!s.previa([0.0, 0.0]).formas.is_empty());
        s.escribir("0,0");
        s.escribir("");
        s.escribir("");
        assert_eq!(s.activa(), None);
        assert!(s.dibujo.doc.block_records.contains("INODORO"));
        let n = s.dibujo.doc.entities_in_block("INODORO").count();
        assert!(n > 0);
        // Otra vez: usa la que ya esta.
        s.escribir("bib");
        s.escribir("inodoro");
        s.escribir("1000,0");
        s.escribir("");
        s.escribir("45");
        assert_eq!(s.dibujo.doc.entities_in_block("INODORO").count(), n);
        let ins = inserts(&s);
        assert_eq!(ins.len(), 2);
        assert!(ins.iter().all(|i| i.common.layer == "A-MOBILIARIO"));
    }

    #[test]
    fn biblioteca_lista_sus_piezas() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("bib");
        assert_eq!(s.activa(), Some(Id::Otra("BIBLIOTECA")));
        s.escribir("");
        assert_eq!(s.activa(), None);
        for n in ["INODORO", "CAMA-1.5P", "PUERTA-0.90", "COLUMNA-30X30"] {
            assert!(s.registro.iter().any(|l| l.contains(n)), "{n}");
        }
    }

    #[test]
    fn todas_las_piezas_se_insertan() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        for (k, p) in crate::biblioteca::PIEZAS.iter().enumerate() {
            s.escribir("i");
            s.escribir(p.nombre);
            s.escribir(&format!("{},0", k as f64 * 3000.0));
            s.escribir("");
            s.escribir("");
        }
        assert_eq!(inserts(&s).len(), crate::biblioteca::PIEZAS.len());
    }

    #[test]
    fn bloques_y_sombreados_vuelven_en_dwg_y_dxf() {
        let mut s = con_bloque();
        // Una pieza con un sombreado dentro y un sombreado en el modelo.
        s.escribir("i");
        s.escribir("columna-30x30");
        s.escribir("5000,0");
        s.escribir("");
        s.escribir("");
        s.escribir("rec");
        s.escribir("0,1000");
        s.escribir("2000,2000");
        s.escribir("so");
        s.escribir("arena");
        s.escribir("1000,1500");
        s.escribir("");
        let dir = std::env::temp_dir().join(format!("cad-nucleo-bloques-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for ext in ["dwg", "dxf"] {
            let ruta = dir.join(format!("bloques.{ext}"));
            s.dibujo.guardar_como(&ruta).unwrap();
            let d = Dibujo::abrir(&ruta).unwrap();
            let s2 = Sesion::nueva(d);
            let ins = inserts(&s2);
            assert_eq!(ins.len(), 2, "{ext}");
            assert!(ins.iter().any(|i| i.block_name == "Mesa"), "{ext}");
            assert_eq!(s2.dibujo.doc.entities_in_block("Mesa").count(), 2, "{ext}");
            let col: Vec<_> = s2.dibujo.doc.entities_in_block("COLUMNA-30X30").collect();
            assert!(
                col.iter()
                    .any(|e| matches!(e, EntityType::Hatch(h) if h.is_solid)),
                "{ext}: el relleno de la columna"
            );
            let h: Vec<_> = s2
                .dibujo
                .doc
                .model_space_entities()
                .filter_map(|e| match e {
                    EntityType::Hatch(h) => Some(h.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(h.len(), 1, "{ext}");
            assert_eq!(h[0].pattern.name, "ARENA", "{ext}");
            assert_eq!(h[0].pattern.lines.len(), 3, "{ext}");
            assert!(!h[0].is_solid, "{ext}");
            assert_eq!(h[0].paths.len(), 1, "{ext}");
            // Y el bloque leido se sigue pudiendo insertar y descomponer.
            let mut s2 = s2;
            s2.escribir("i");
            s2.escribir("mesa");
            s2.escribir("0,5000");
            s2.escribir("");
            s2.escribir("");
            assert_eq!(inserts(&s2).len(), 3, "{ext}");
            s2.escribir("x");
            assert_eq!(inserts(&s2).len(), 2, "{ext}");
            assert_eq!(s2.seleccion.len(), 2, "{ext}: el rectangulo y el circulo");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
