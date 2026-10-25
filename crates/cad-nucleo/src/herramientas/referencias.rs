//! **Las referencias externas y los planos como bloque**: REFX (XATTACH) y
//! REFSUPERPONER (XOVERLAY) adjuntan un plano, RECARGARREF lo vuelve a leer,
//! UNIRREF lo vuelve bloque, QUITARREF lo quita, LISTAREF dice como esta
//! cada una e INSERTARDWG pone un plano entero como bloque. Lo de dentro
//! esta en [`crate::xref`].

use std::path::PathBuf;

use crate::bloque;
use crate::geo::{self, Forma, P};
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, PedidoApp, Previa, Registro};
use crate::ordenes::{Grupo, Id, Orden};
use crate::xref;

pub const REFX: &str = "REFX";
pub const REFSUPERPONER: &str = "REFSUPERPONER";
pub const RECARGARREF: &str = "RECARGARREF";
pub const UNIRREF: &str = "UNIRREF";
pub const QUITARREF: &str = "QUITARREF";
pub const LISTAREF: &str = "LISTAREF";
pub const INSERTARDWG: &str = "INSERTARDWG";

pub const REGISTROS: &[Registro] = &[
    Registro {
        orden: Orden {
            id: Id::Otra(REFX),
            nombre: REFX,
            alias: &["XA", "XATTACH", "XREF"],
            grupo: Grupo::Bloques,
            ayuda: "Adjunta otro plano DWG o DXF como referencia externa",
            con_objetos: false,
        },
        etiqueta: "Referencia",
        crear: || Box::new(Colocar::nuevo(Que::Adjuntar)),
    },
    Registro {
        orden: Orden {
            id: Id::Otra(REFSUPERPONER),
            nombre: REFSUPERPONER,
            alias: &["XOVERLAY"],
            grupo: Grupo::Bloques,
            ayuda: "Superpone otro plano (no se ve si este se referencia en otro)",
            con_objetos: false,
        },
        etiqueta: "Superponer",
        crear: || Box::new(Colocar::nuevo(Que::Superponer)),
    },
    Registro {
        orden: Orden {
            id: Id::Otra(RECARGARREF),
            nombre: RECARGARREF,
            alias: &["XRELOAD"],
            grupo: Grupo::Bloques,
            ayuda: "Vuelve a leer las referencias externas de su fichero",
            con_objetos: false,
        },
        etiqueta: "Recargar ref.",
        crear: || Box::new(PorNombre::nuevo(Accion::Recargar)),
    },
    Registro {
        orden: Orden {
            id: Id::Otra(UNIRREF),
            nombre: UNIRREF,
            alias: &["XBIND"],
            grupo: Grupo::Bloques,
            ayuda: "Convierte una referencia externa en un bloque del dibujo",
            con_objetos: false,
        },
        etiqueta: "Unir ref.",
        crear: || Box::new(PorNombre::nuevo(Accion::Unir)),
    },
    Registro {
        orden: Orden {
            id: Id::Otra(QUITARREF),
            nombre: QUITARREF,
            alias: &["XDETACH"],
            grupo: Grupo::Bloques,
            ayuda: "Quita una referencia externa y sus inserciones",
            con_objetos: false,
        },
        etiqueta: "Quitar ref.",
        crear: || Box::new(PorNombre::nuevo(Accion::Quitar)),
    },
    Registro {
        orden: Orden {
            id: Id::Otra(LISTAREF),
            nombre: LISTAREF,
            alias: &["LREF", "XREFS"],
            grupo: Grupo::Bloques,
            ayuda: "Las referencias externas del dibujo y si se encuentran",
            con_objetos: false,
        },
        etiqueta: "Lista ref.",
        crear: || Box::new(Listar),
    },
    Registro {
        orden: Orden {
            id: Id::Otra(INSERTARDWG),
            nombre: INSERTARDWG,
            alias: &["IA", "IDWG"],
            grupo: Grupo::Bloques,
            ayuda: "Inserta otro plano entero como un bloque de este",
            con_objetos: false,
        },
        etiqueta: "Insertar plano",
        crear: || Box::new(Colocar::nuevo(Que::Plano)),
    },
];

/// Cuantas formas se pintan como mucho en la previa (con mas, el recuadro).
const PREVIA_MAX: usize = 20_000;

/// Quita las comillas que trae una ruta pegada del explorador.
fn limpiar_ruta(t: &str) -> PathBuf {
    PathBuf::from(t.trim().trim_matches('"'))
}

/// Pide el dialogo de abrir un plano a la app.
fn pedir_archivo(cx: &mut Contexto, titulo: &str) {
    cx.memoria.pedidos.push(PedidoApp::Archivo {
        guardar: false,
        titulo: titulo.into(),
        filtros: vec![("Planos DWG y DXF".into(), "*.dwg;*.dxf".into())],
        sugerido: String::new(),
    });
}

/// Las cuatro rayas de la caja de unas formas.
fn recuadro(f: &[Forma]) -> Vec<Forma> {
    let mut c = geo::CAJA_VACIA;
    for f in f {
        c = geo::unir_cajas(c, f.caja());
    }
    if !geo::caja_valida(&c) {
        return Vec::new();
    }
    let e = [[c[0], c[1]], [c[2], c[1]], [c[2], c[3]], [c[0], c[3]]];
    (0..4)
        .map(|i| Forma::Segmento {
            a: e[i],
            b: e[(i + 1) % 4],
        })
        .collect()
}

// ------------------------------------------------- REFX / INSERTARDWG

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Que {
    Adjuntar,
    Superponer,
    /// INSERTARDWG: el plano como bloque normal.
    Plano,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paso {
    Archivo,
    Punto,
    Escala,
    Giro,
}

/// Pide el fichero, el punto, la escala y el giro.
struct Colocar {
    que: Que,
    paso: Paso,
    nombre: String,
    /// Si la definicion se hizo en esta orden (se quita si se cancela).
    creada: bool,
    formas: Vec<Forma>,
    punto: P,
    escala: f64,
}

impl Colocar {
    fn nuevo(que: Que) -> Colocar {
        Colocar {
            que,
            paso: Paso::Archivo,
            nombre: String::new(),
            creada: false,
            formas: Vec::new(),
            punto: [0.0, 0.0],
            escala: 1.0,
        }
    }

    fn titulo(&self) -> &'static str {
        match self.que {
            Que::Adjuntar => "Plano para adjuntar como referencia",
            Que::Superponer => "Plano para superponer como referencia",
            Que::Plano => "Plano para insertar como bloque",
        }
    }

    fn cargar(&mut self, cx: &mut Contexto, archivo: PathBuf) -> Flujo {
        let anfitrion = cx.dibujo.ruta.clone();
        let r = match self.que {
            Que::Plano => xref::insertar_archivo(&mut cx.dibujo.doc, &archivo, None),
            q => xref::adjuntar(
                &mut cx.dibujo.doc,
                anfitrion.as_deref(),
                &archivo,
                q == Que::Superponer,
            ),
        };
        let (nombre, creada, carga) = match r {
            Ok(v) => v,
            Err(e) => {
                cx.decir(e);
                return Flujo::Termina;
            }
        };
        for a in &carga.avisos {
            cx.decir(a.clone());
        }
        if creada {
            cx.dibujo.tocar();
            let que = if self.que == Que::Plano {
                "Bloque"
            } else {
                "Referencia"
            };
            cx.decir(format!("{que} «{nombre}» con {} objetos", carga.objetos));
        } else if self.que != Que::Plano {
            cx.decir(format!("«{nombre}» ya estaba adjunta: se inserta otra vez"));
        }
        self.creada = creada;
        self.formas = bloque::formas_de(&cx.dibujo.doc, &nombre);
        if self.formas.len() > PREVIA_MAX {
            self.formas = recuadro(&self.formas);
        }
        self.nombre = nombre;
        self.paso = Paso::Punto;
        Flujo::Sigue
    }

    fn poner(&mut self, cx: &mut Contexto, giro: f64) -> Flujo {
        let e = bloque::insercion(&self.nombre, self.punto, self.escala, giro);
        let orden = match self.que {
            Que::Plano => INSERTARDWG,
            _ => REFX,
        };
        *cx.seleccion = cx.crear(orden, vec![e], None);
        self.creada = false;
        cx.decir(format!("«{}» insertado", self.nombre));
        Flujo::Termina
    }

    fn formas_en(&self, p: P, s: f64, giro: f64) -> Vec<Forma> {
        self.formas
            .iter()
            .map(|f| bloque::transformar(f, p, s, giro))
            .collect()
    }
}

impl Herramienta for Colocar {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match self.paso {
            Paso::Archivo => format!("{} (ruta del DWG o DXF):", self.titulo()),
            Paso::Punto => format!("Punto de insercion de «{}»:", self.nombre),
            Paso::Escala => "Escala <1>:".into(),
            Paso::Giro => "Angulo de giro <0>:".into(),
        }
    }

    fn base(&self) -> Option<P> {
        matches!(self.paso, Paso::Escala | Paso::Giro).then_some(self.punto)
    }

    fn pide_texto(&self) -> bool {
        self.paso == Paso::Archivo
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        pedir_archivo(cx, self.titulo());
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        match self.paso {
            Paso::Archivo => self.cargar(cx, limpiar_ruta(t)),
            _ => Flujo::NoEntiende,
        }
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        match self.paso {
            Paso::Archivo => Flujo::NoEntiende,
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
            Paso::Archivo | Paso::Punto => {
                self.cancelar(cx);
                Flujo::Termina
            }
            Paso::Escala => {
                self.escala = 1.0;
                self.paso = Paso::Giro;
                Flujo::Sigue
            }
            Paso::Giro => self.poner(cx, 0.0),
        }
    }

    fn cancelar(&mut self, cx: &mut Contexto) {
        // Lo que se cargo para nada no se queda en el dibujo.
        if self.creada && xref::inserciones(&cx.dibujo.doc, &self.nombre) == 0 {
            xref::quitar_definicion(&mut cx.dibujo.doc, &self.nombre);
            cx.dibujo.tocar();
        }
        self.creada = false;
    }

    fn previa(&self, _cx: &Lectura, c: P) -> Previa {
        let formas = match self.paso {
            Paso::Archivo => Vec::new(),
            Paso::Punto => self.formas_en(c, 1.0, 0.0),
            Paso::Escala => self.formas_en(self.punto, 1.0, 0.0),
            Paso::Giro => self.formas_en(self.punto, self.escala, geo::angulo(self.punto, c)),
        };
        Previa {
            formas,
            ..Default::default()
        }
    }
}

// ------------------------------------------- RECARGARREF / UNIRREF / QUITARREF

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Recargar,
    Unir,
    Quitar,
}

/// Pide el nombre de la referencia (Enter: todas para recargar, o la unica).
struct PorNombre {
    accion: Accion,
}

impl PorNombre {
    fn nuevo(accion: Accion) -> PorNombre {
        PorNombre { accion }
    }

    fn hacer(&self, cx: &mut Contexto, nombres: Vec<String>) -> Flujo {
        let anfitrion = cx.dibujo.ruta.clone();
        for n in nombres {
            match self.accion {
                Accion::Recargar => {
                    match xref::cargar(&mut cx.dibujo.doc, &n, anfitrion.as_deref()) {
                        Ok(c) => {
                            for a in c.avisos {
                                cx.decir(a);
                            }
                            cx.decir(format!("«{n}» recargada: {} objetos", c.objetos));
                        }
                        Err(e) => cx.decir(format!("«{n}»: {e}")),
                    }
                }
                Accion::Unir => match xref::unir(&mut cx.dibujo.doc, &n) {
                    Ok(k) => cx.decir(format!("«{n}» es ahora un bloque del dibujo ({k} objetos)")),
                    Err(e) => cx.decir(e),
                },
                Accion::Quitar => {
                    cx.seleccion.clear();
                    match xref::quitar(cx.dibujo, &n) {
                        Ok(k) => cx.decir(format!("«{n}» quitada ({k} inserciones)")),
                        Err(e) => cx.decir(e),
                    }
                }
            }
        }
        cx.dibujo.tocar();
        Flujo::Termina
    }
}

impl Herramienta for PorNombre {
    fn indicacion(&self, cx: &Lectura) -> String {
        let v = xref::nombres(&cx.dibujo.doc);
        let lista = if v.is_empty() {
            "ninguna".to_string()
        } else {
            v.join(", ")
        };
        let defecto = match (self.accion, v.len()) {
            (Accion::Recargar, _) => " <* todas>".to_string(),
            (_, 1) => format!(" <{}>", v[0]),
            _ => String::new(),
        };
        let que = match self.accion {
            Accion::Recargar => "recargar",
            Accion::Unir => "unir",
            Accion::Quitar => "quitar",
        };
        format!("Referencia a {que} ({lista}){defecto}:")
    }

    fn pide_texto(&self) -> bool {
        true
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        if xref::nombres(&cx.dibujo.doc).is_empty() {
            cx.decir("El dibujo no tiene referencias externas");
            return Flujo::Termina;
        }
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        let t = t.trim();
        let todas = xref::nombres(&cx.dibujo.doc);
        if t == "*" {
            return self.hacer(cx, todas);
        }
        match xref::buscar(&cx.dibujo.doc, t) {
            Some(n) => self.hacer(cx, vec![n]),
            None => {
                cx.decir(format!("No hay una referencia «{t}»"));
                Flujo::Sigue
            }
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        let todas = xref::nombres(&cx.dibujo.doc);
        match (self.accion, todas.len()) {
            (Accion::Recargar, _) | (_, 1) => self.hacer(cx, todas),
            _ => Flujo::Termina,
        }
    }
}

// ---------------------------------------------------------------- LISTAREF

/// Dice las referencias, de donde vienen y como estan.
struct Listar;

impl Herramienta for Listar {
    fn indicacion(&self, _cx: &Lectura) -> String {
        String::new()
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        let v = xref::listar(&cx.dibujo.doc, cx.dibujo.ruta.as_deref());
        if v.is_empty() {
            cx.decir("El dibujo no tiene referencias externas");
            return Flujo::Termina;
        }
        cx.decir(format!("{} referencias externas:", v.len()));
        for r in v {
            let tipo = if r.superpuesta {
                "superpuesta"
            } else {
                "adjunta"
            };
            let donde = r
                .resuelta
                .map(|p| format!(" → {}", p.display()))
                .unwrap_or_default();
            cx.decir(format!(
                "  {} ({tipo}, {}): {}{donde} · {} objetos · {} inserciones",
                r.nombre,
                r.estado.nombre(),
                r.ruta,
                r.objetos,
                r.inserciones
            ));
        }
        Flujo::Termina
    }
}

#[cfg(test)]
mod pruebas {
    use std::path::Path;

    use opencadcodec::EntityType;

    use crate::especialidad as esp;
    use crate::herramienta::PedidoApp;
    use crate::{Dibujo, Sesion};

    fn carpeta(nombre: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("cad-nucleo-refs-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn plano(ruta: &Path) {
        let mut d = Dibujo::nuevo();
        d.editar("x", |t| {
            t.anadir_en(esp::linea([0.0, 0.0], [100.0, 0.0]), "A-MUROS");
            t.anadir_en(esp::linea([0.0, 0.0], [0.0, 100.0]), "A-MUROS");
        });
        d.guardar_como(ruta).unwrap();
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

    fn ultimo(s: &Sesion) -> String {
        s.registro.last().cloned().unwrap_or_default()
    }

    #[test]
    fn refx_pide_el_fichero_y_lo_pone() {
        let dir = carpeta("refx");
        let base = dir.join("Base.dxf");
        plano(&base);
        let mut d = Dibujo::nuevo();
        d.ruta = Some(dir.join("anf.dwg"));
        let mut s = Sesion::nueva(d);
        s.escribir("xa");
        assert!(matches!(
            s.memoria.pedidos.last(),
            Some(PedidoApp::Archivo { guardar: false, .. })
        ));
        s.memoria.pedidos.clear();
        // La app escribe la ruta (con comillas, como al pegarla).
        s.escribir(&format!("\"{}\"", base.display()));
        s.escribir("500,500");
        s.escribir("2");
        s.escribir("90");
        assert_eq!(s.activa(), None);
        let v = inserts(&s);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].block_name, "Base");
        assert!((v[0].x_scale() - 2.0).abs() < 1e-9);
        assert!(s.dibujo.doc.layers.contains("Base|A-MUROS"));
        // No se descompone.
        s.elegir_todo();
        s.escribir("x");
        assert_eq!(inserts(&s).len(), 1);
        s.escape();
        s.escribir("listaref");
        assert!(
            s.registro
                .iter()
                .any(|l| l.contains("Base (adjunta, cargada)"))
        );
        s.escribir("recargarref");
        s.escribir("");
        assert!(
            s.registro
                .iter()
                .any(|l| l.contains("«Base» recargada: 2 objetos"))
        );
        s.escribir("xbind");
        s.escribir("");
        assert!(ultimo(&s).contains("bloque del dibujo"), "{}", ultimo(&s));
        assert!(crate::xref::nombres(&s.dibujo.doc).is_empty());
        assert!(s.dibujo.doc.layers.contains("Base$0$A-MUROS"));
    }

    #[test]
    fn cancelar_no_deja_la_definicion_y_quitarref_la_borra() {
        let dir = carpeta("cancelar");
        let base = dir.join("base.dxf");
        plano(&base);
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("xoverlay");
        s.escribir(&base.display().to_string());
        assert!(s.dibujo.doc.block_records.contains("base"));
        s.escape();
        assert!(!s.dibujo.doc.block_records.contains("base"));
        assert!(!s.dibujo.doc.layers.contains("base|A-MUROS"));
        // El dialogo cancelado (la app manda Esc): nada.
        s.escribir("xa");
        s.escape();
        assert_eq!(s.activa(), None);
        // Un fichero que no esta.
        s.escribir("xa");
        s.escribir(&dir.join("nada.dwg").display().to_string());
        assert!(ultimo(&s).contains("No existe"), "{}", ultimo(&s));
        assert_eq!(s.activa(), None);
        // Ahora si, superpuesta, y luego quitarla.
        s.escribir("xoverlay");
        s.escribir(&base.display().to_string());
        s.escribir("0,0");
        s.escribir("");
        s.escribir("");
        assert!(
            s.dibujo
                .doc
                .block_records
                .get("base")
                .unwrap()
                .flags
                .is_xref_overlay
        );
        s.escribir("quitarref");
        s.escribir("base");
        assert!(inserts(&s).is_empty());
        assert!(crate::xref::nombres(&s.dibujo.doc).is_empty());
        // Y con nada que quitar.
        s.escribir("xdetach");
        assert!(ultimo(&s).contains("no tiene referencias"));
    }

    #[test]
    fn insertardwg_pone_el_plano_como_bloque() {
        let dir = carpeta("ia");
        let base = dir.join("Mesa de trabajo.dwg");
        plano(&base);
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("ia");
        s.escribir(&base.display().to_string());
        s.escribir("100,0");
        s.escribir("");
        s.escribir("");
        let v = inserts(&s);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].block_name, "Mesa de trabajo");
        assert_eq!(s.dibujo.doc.entities_in_block("Mesa de trabajo").count(), 2);
        assert!(crate::xref::nombres(&s.dibujo.doc).is_empty());
        // Se descompone como cualquier bloque.
        s.elegir_todo();
        s.escribir("x");
        assert!(inserts(&s).is_empty());
        assert_eq!(s.dibujo.doc.model_space_entities().count(), 2);
    }
}
