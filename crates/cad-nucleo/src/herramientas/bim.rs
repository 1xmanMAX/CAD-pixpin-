//! **Las ordenes de los modelos BIM**: EXPLORARBIM (abre el explorador de
//! datos de un IFC o un Revit) y REVITAPLANTA (dibuja la planta de un nivel
//! de un Revit, con una capa por categoria). Los datos los lee
//! `cad-bim-datos` (que no depende de este crate).

use cad_bim_datos::planta::{self, Planta};

use crate::especialidad as esp;
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, PedidoApp, Registro};
use crate::herramientas::vista::PedidoVista;
use crate::ordenes::{Grupo, Id, Orden};

pub const REGISTROS: &[Registro] = &[
    Registro {
        orden: Orden {
            id: Id::Otra("EXPLORARBIM"),
            nombre: "EXPLORARBIM",
            alias: &["EB", "BIMDATOS"],
            grupo: Grupo::Medir,
            ayuda: "Ver los datos de un modelo IFC o Revit: proyecto, niveles, elementos y sus parametros",
            con_objetos: false,
        },
        etiqueta: "Explorar BIM",
        crear: || Box::new(Explorar),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("REVITAPLANTA"),
            nombre: "REVITAPLANTA",
            alias: &["RVTPLANTA", "REVITPLANTA"],
            grupo: Grupo::Medir,
            ayuda: "Dibujar la planta de un nivel de un Revit, una capa por categoria",
            con_objetos: false,
        },
        etiqueta: "Planta de Revit",
        crear: || Box::new(RevitPlanta { planta: None }),
    },
];

/// Una ruta como la escribe el dialogo o alguien (quizas entre comillas).
fn ruta(t: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(t.trim().trim_matches('"'))
}

struct Explorar;

impl Herramienta for Explorar {
    fn indicacion(&self, _: &Lectura) -> String {
        "Modelo a explorar (IFC o Revit)".into()
    }

    fn pide_texto(&self) -> bool {
        true
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        cx.memoria.pedidos.push(PedidoApp::Archivo {
            guardar: false,
            titulo: "Explorar los datos de un modelo".into(),
            filtros: vec![
                ("Modelos BIM".into(), "*.ifc;*.rvt;*.rfa".into()),
                ("Todos".into(), "*.*".into()),
            ],
            sugerido: String::new(),
        });
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        let r = ruta(t);
        if !cad_bim_datos::se_lee(&r) {
            cx.decir(format!("{}: no es un IFC ni un Revit", r.display()));
            return Flujo::Sigue;
        }
        cx.memoria.pedidos.push(PedidoApp::ExplorarBim(r));
        Flujo::Termina
    }
}

struct RevitPlanta {
    planta: Option<Planta>,
}

impl RevitPlanta {
    /// Dibuja los contornos de un nivel (`None`: todos, con el nivel en la capa).
    fn dibujar(&self, cx: &mut Contexto, nivel: Option<usize>) {
        let Some(p) = &self.planta else { return };
        // De metros a unidades del dibujo.
        let k = 1000.0 * esp::unidades_por_mm(1.0, cx.dibujo.doc.header.insertion_units);
        let mut por_capa: Vec<(String, Vec<opencadcodec::EntityType>)> = Vec::new();
        for t in p.del_nivel(nivel) {
            let capa = match (nivel, t.nivel.and_then(|i| p.niveles.get(i))) {
                (None, Some((n, _))) => format!("{}-{}", t.capa, nombre_de_capa(n)),
                _ => t.capa.clone(),
            };
            let pts: Vec<[f64; 2]> = t.puntos.iter().map(|q| [q[0] * k, q[1] * k]).collect();
            let e = esp::polilinea(&pts, true);
            match por_capa.iter_mut().find(|(c, _)| *c == capa) {
                Some((_, v)) => v.push(e),
                None => por_capa.push((capa, vec![e])),
            }
        }
        if por_capa.is_empty() {
            cx.decir("Ese nivel no tiene nada que dibujar en planta");
            return;
        }
        let n: usize = por_capa.iter().map(|(_, v)| v.len()).sum();
        for (c, _) in &por_capa {
            cx.dibujo.asegurar_capa(c, esp::color_de_capa(c));
        }
        cx.dibujo.editar("REVITAPLANTA", |t| {
            for (c, v) in por_capa {
                for e in v {
                    t.anadir_en(e, &c);
                }
            }
        });
        cx.decir(format!("Planta de Revit: {n} contornos"));
        cx.memoria.vista = Some(PedidoVista::Extension);
    }
}

/// Un nombre de nivel que valga como parte de un nombre de capa.
fn nombre_de_capa(s: &str) -> String {
    s.chars()
        .map(|c| {
            if "<>/\\\":;?*|=,`".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect()
}

impl Herramienta for RevitPlanta {
    fn indicacion(&self, _: &Lectura) -> String {
        match &self.planta {
            None => "Revit (.rvt) del que dibujar la planta".into(),
            Some(p) => {
                let lista: Vec<String> = p
                    .niveles
                    .iter()
                    .enumerate()
                    .map(|(i, (n, _))| format!("{} {n}", i + 1))
                    .collect();
                format!("Nivel [{} / Todos] <1>", lista.join(" / "))
            }
        }
    }

    fn pide_texto(&self) -> bool {
        true
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        cx.memoria.pedidos.push(PedidoApp::Archivo {
            guardar: false,
            titulo: "Dibujar la planta de un Revit".into(),
            filtros: vec![("Revit".into(), "*.rvt".into())],
            sugerido: String::new(),
        });
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        if self.planta.is_none() {
            let r = ruta(t);
            return match planta::de_revit(&r) {
                Err(e) => {
                    cx.decir(format!("No se pudo leer {}: {e}", r.display()));
                    Flujo::Termina
                }
                Ok(p) if p.trazos.is_empty() => {
                    cx.decir("rvt-rs no encontro nada que dibujar en planta en ese Revit");
                    Flujo::Termina
                }
                Ok(p) => {
                    let unico = p.niveles.len() <= 1;
                    self.planta = Some(p);
                    if unico {
                        self.dibujar(cx, None);
                        Flujo::Termina
                    } else {
                        Flujo::Sigue
                    }
                }
            };
        }
        let t = t.trim();
        if t.eq_ignore_ascii_case("todos") || t.eq_ignore_ascii_case("t") || t == "*" {
            self.dibujar(cx, None);
            return Flujo::Termina;
        }
        match self.planta.as_ref().and_then(|p| p.buscar_nivel(t)) {
            Some(n) => {
                self.dibujar(cx, Some(n));
                Flujo::Termina
            }
            None => {
                cx.decir(format!("No hay un nivel «{t}»"));
                Flujo::Sigue
            }
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        if self.planta.is_some() {
            self.dibujar(cx, Some(0));
        }
        Flujo::Termina
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::{Dibujo, Sesion};

    fn rvt() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cad-bim-datos/tests/datos/Revit_IFC5_Einhoven.rvt")
    }

    #[test]
    fn explorar_pide_el_fichero_y_luego_la_ventana() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("eb");
        assert!(matches!(
            s.memoria.pedidos.last(),
            Some(PedidoApp::Archivo { guardar: false, .. })
        ));
        s.escribir("modelo.txt");
        assert!(s.activa().is_some(), "no es un modelo: sigue pidiendo");
        s.escribir("C:\\obra\\modelo.ifc");
        assert_eq!(
            s.memoria.pedidos.last(),
            Some(&PedidoApp::ExplorarBim("C:\\obra\\modelo.ifc".into()))
        );
        assert_eq!(s.activa(), None);
    }

    #[test]
    fn planta_de_revit_por_nivel() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("revitaplanta");
        assert!(matches!(
            s.memoria.pedidos.last(),
            Some(PedidoApp::Archivo { .. })
        ));
        s.escribir(&rvt().to_string_lossy());
        assert!(s.activa().is_some(), "pide el nivel");
        s.escribir("no hay tal");
        assert!(s.activa().is_some());
        s.escribir("1");
        assert_eq!(s.activa(), None);
        let capas: Vec<String> = s
            .dibujo
            .doc
            .model_space_entities()
            .map(|e| e.common().layer.clone())
            .collect();
        assert!(!capas.is_empty());
        assert!(capas.iter().all(|c| c.starts_with("BIM-")), "{capas:?}");
        // Deshacer quita todo de una vez.
        s.dibujo.deshacer();
        assert_eq!(s.dibujo.doc.model_space_entities().count(), 0);
    }

    #[test]
    fn planta_de_revit_roto() {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        s.escribir("rvtplanta");
        s.escribir("/no/existe.rvt");
        assert_eq!(s.activa(), None);
    }
}
