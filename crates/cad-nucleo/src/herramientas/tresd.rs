//! **Las ordenes del 3D**: EXTRUIR, ALTURA, MURO3D, LOSA y NIVELES para
//! levantar el modelo desde la planta; EXPORTAR3D (IFC4, glTF, OBJ, STL,
//! DXF y DWG), IMPORTAR3D (STL, OBJ, glTF, FBX y COLLADA) y VER3D, que pide
//! a la app la ventana 3D con el dibujo de ahora. Lo de dentro esta en
//! [`crate::tresd`].

use std::path::PathBuf;

use opencadcodec::{Color, EntityType, Handle, Layer};

use crate::especialidad as esp;
use crate::geo::P;
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, PedidoApp, Registro};
use crate::ordenes::{self, Grupo, Id, Orden};
use crate::tresd::escena::{self, Nivel};
use crate::tresd::{self, Escena, Malla, Tipo, importar, modelado};

/// La capa de las losas (gris, como el hormigon en los planos).
pub const CAPA_LOSAS: &str = "A-LOSAS";

macro_rules! orden {
    ($n:literal, [$($a:literal),*], $g:expr, $obj:literal, $et:literal, $ay:literal, $crear:expr) => {
        Registro {
            orden: Orden {
                id: Id::Otra($n),
                nombre: $n,
                alias: &[$($a),*],
                grupo: $g,
                ayuda: $ay,
                con_objetos: $obj,
            },
            etiqueta: $et,
            crear: $crear,
        }
    };
}

pub const REGISTROS: &[Registro] = &[
    orden!(
        "EXTRUIR",
        ["EXT", "EXTRUDE"],
        Grupo::Arquitectura,
        true,
        "Extruir",
        "Levanta polilineas cerradas y circulos con una altura (malla 3D en su capa)",
        || Box::new(Extruir::default())
    ),
    orden!(
        "ALTURA",
        ["ALT", "THICKNESS"],
        Grupo::Arquitectura,
        true,
        "Altura",
        "Da altura y elevacion a lineas, polilineas, arcos y circulos (2D extruido)",
        || Box::new(Altura::default())
    ),
    orden!(
        "MURO3D",
        ["M3D", "WALL3D"],
        Grupo::Arquitectura,
        true,
        "Muro 3D",
        "Levanta los muros elegidos (sus dos caras o su eje) con los vanos de puertas y ventanas",
        || Box::new(Muro3d::default())
    ),
    orden!(
        "LOSA",
        ["SLAB"],
        Grupo::Arquitectura,
        true,
        "Losa",
        "Una losa desde un contorno cerrado, con su espesor y su nivel",
        || Box::new(Losa::default())
    ),
    orden!(
        "NIVELES",
        ["NIVS", "STOREYS"],
        Grupo::Arquitectura,
        false,
        "Niveles",
        "Los niveles del edificio (nombre y cota) para el IFC y la vista 3D",
        || Box::new(Niveles)
    ),
    orden!(
        "EXPORTAR3D",
        ["EXP3D", "3DEXPORT"],
        Grupo::Vista,
        false,
        "Exportar 3D",
        "Guarda el modelo 3D en IFC4, glTF (.glb), OBJ, STL, DXF o DWG",
        || Box::new(Exportar3d)
    ),
    orden!(
        "IMPORTAR3D",
        ["IMP3D", "3DIMPORT"],
        Grupo::Vista,
        false,
        "Importar 3D",
        "Trae mallas de STL, OBJ, glTF/GLB, FBX o COLLADA, a su escala",
        || Box::new(Importar3d::default())
    ),
    orden!(
        "VER3D",
        ["3D", "V3"],
        Grupo::Vista,
        false,
        "Ver en 3D",
        "Abre el dibujo en la ventana 3D (orbitar, cortar, medir)",
        || Box::new(Ver3d)
    ),
];

/// Un metro en unidades del dibujo.
fn metro(cx: &Lectura) -> f64 {
    tresd::unidades_por_metro(cx.dibujo.doc.header.insertion_units)
}

/// Anade mallas, cada una en su capa, de una vez (un paso de deshacer), y
/// las deja elegidas.
fn poner(cx: &mut Contexto, nombre: &str, v: Vec<(Malla, String)>) -> usize {
    for (_, capa) in &v {
        cx.dibujo.asegurar_capa(capa, esp::color_de_capa(capa));
    }
    let hs: Vec<Handle> = cx.dibujo.editar(nombre, |t| {
        v.into_iter()
            .filter_map(|(m, capa)| t.anadir_en(m.a_mesh(), &capa))
            .collect()
    });
    let n = hs.len();
    *cx.seleccion = hs;
    n
}

// ---------------------------------------------------------------- EXTRUIR

#[derive(Default)]
struct Extruir {
    objetos: Vec<(Handle, EntityType)>,
}

impl Extruir {
    fn hacer(&mut self, cx: &mut Contexto, alto: f64) -> Flujo {
        let mut v = Vec::new();
        let mut malos = 0;
        let mut error = String::new();
        for (_, e) in &self.objetos {
            match modelado::extruir(e, alto) {
                Ok(p) => v.push((p.malla(), e.common().layer.clone())),
                Err(t) => {
                    malos += 1;
                    error = t;
                }
            }
        }
        if v.is_empty() {
            cx.decir(error);
            // Con la altura en cero se puede escribir otra.
            return if alto.abs() < 1e-9 {
                Flujo::Sigue
            } else {
                Flujo::Termina
            };
        }
        let n = poner(cx, "EXTRUIR", v);
        let mut t = format!("{n} solido(s) de {} de alto", cx.medida(alto.abs()));
        if malos > 0 {
            t.push_str(&format!(" ({malos} objeto(s) no son contornos cerrados)"));
        }
        cx.decir(t);
        Flujo::Termina
    }
}

impl Herramienta for Extruir {
    fn indicacion(&self, cx: &Lectura) -> String {
        format!("Altura de extrusion <{}>:", cx.medida(metro(cx) * 2.6))
    }

    fn al_tener_objetos(&mut self, cx: &mut Contexto) -> Flujo {
        self.objetos = cx.elegidas();
        Flujo::Sigue
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        self.hacer(cx, v)
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        let alto = metro(&cx.leer()) * 2.6;
        self.hacer(cx, alto)
    }
}

// ----------------------------------------------------------------- ALTURA

#[derive(Default)]
struct Altura {
    objetos: Vec<Handle>,
    alto: Option<f64>,
}

impl Altura {
    fn hacer(&mut self, cx: &mut Contexto, elevacion: f64) -> Flujo {
        let alto = self.alto.unwrap_or(0.0);
        let hs = self.objetos.clone();
        let n = cx.dibujo.editar("ALTURA", |t| {
            hs.iter()
                .filter(|h| {
                    t.cambiar(**h, |e| match e {
                        EntityType::Line(l) => {
                            l.thickness = alto;
                            l.start.z = elevacion;
                            l.end.z = elevacion;
                        }
                        EntityType::LwPolyline(p) => {
                            p.thickness = alto;
                            p.elevation = elevacion;
                        }
                        EntityType::Circle(c) => {
                            c.thickness = alto;
                            c.center.z = elevacion;
                        }
                        EntityType::Arc(a) => {
                            a.thickness = alto;
                            a.center.z = elevacion;
                        }
                        _ => {}
                    })
                })
                .count()
        });
        let a = cx.medida(alto);
        let z = cx.medida(elevacion);
        cx.decir(format!("{n} objeto(s) con altura {a} desde {z}"));
        Flujo::Termina
    }
}

fn admite_altura(e: &EntityType) -> bool {
    matches!(
        e,
        EntityType::Line(_)
            | EntityType::LwPolyline(_)
            | EntityType::Circle(_)
            | EntityType::Arc(_)
    )
}

impl Herramienta for Altura {
    fn indicacion(&self, cx: &Lectura) -> String {
        match self.alto {
            None => format!("Altura (espesor 3D) <{}>:", cx.medida(metro(cx) * 2.6)),
            Some(_) => "Elevacion (cota de la base) <0>:".into(),
        }
    }

    fn al_tener_objetos(&mut self, cx: &mut Contexto) -> Flujo {
        let v = cx.elegidas();
        self.objetos = v
            .iter()
            .filter(|(_, e)| admite_altura(e))
            .map(|(h, _)| *h)
            .collect();
        if self.objetos.is_empty() {
            cx.decir("Solo las lineas, polilineas, arcos y circulos llevan altura");
            return Flujo::Termina;
        }
        Flujo::Sigue
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        match self.alto {
            None => {
                self.alto = Some(v);
                Flujo::Sigue
            }
            Some(_) => self.hacer(cx, v),
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        match self.alto {
            None => {
                self.alto = Some(metro(&cx.leer()) * 2.6);
                Flujo::Sigue
            }
            Some(_) => self.hacer(cx, 0.0),
        }
    }
}

// ----------------------------------------------------------------- MURO3D

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PasoMuro {
    #[default]
    Alto,
    Base,
    Espesor,
}

#[derive(Default)]
struct Muro3d {
    objetos: Vec<(Handle, EntityType)>,
    paso: PasoMuro,
    base: f64,
    /// Levantar por el eje con este espesor (si no, se buscan las caras).
    espesor: Option<f64>,
}

/// **Los muros 3D** de unas entidades: los tramos (por sus caras o, si no
/// hay parejas o se dio `espesor`, por su eje), sus vanos y las hojas de
/// las puertas y ventanas. Devuelve las mallas con su capa, y cuantos
/// tramos y vanos salieron.
pub fn muros_3d(
    doc: &opencadcodec::CadDocument,
    objetos: &[EntityType],
    espesor: Option<f64>,
    espesor_defecto: f64,
    base: f64,
    alto: f64,
) -> (Vec<(Malla, String)>, usize, usize) {
    let m = tresd::unidades_por_metro(doc.header.insertion_units);
    let tramos = match espesor {
        Some(e) => objetos
            .iter()
            .flat_map(|o| modelado::tramos_por_eje(o, e))
            .collect(),
        None => {
            let t = modelado::tramos_por_caras(objetos, (0.6 * m).max(espesor_defecto * 2.5));
            if t.is_empty() {
                objetos
                    .iter()
                    .flat_map(|o| modelado::tramos_por_eje(o, espesor_defecto))
                    .collect()
            } else {
                t
            }
        }
    };
    let capa = objetos
        .iter()
        .map(|e| e.common().layer.clone())
        .find(|c| Tipo::de_capa(c) == Tipo::Muro)
        .unwrap_or_else(|| esp::CAPA_MUROS.to_string());
    let (mut puertas, mut ventanas) = (Vec::new(), Vec::new());
    for e in doc.model_space_entities() {
        match Tipo::de_capa(&e.common().layer) {
            Tipo::Puerta => puertas.push(e.clone()),
            Tipo::Ventana => ventanas.push(e.clone()),
            _ => {}
        }
    }
    let al = modelado::Alturas::en(m);
    let mut v = Vec::new();
    let mut huecos = 0;
    for t in &tramos {
        let vanos = modelado::vanos(t, &puertas, &ventanas, base, al);
        let prismas = modelado::muro_3d(t, base, alto, &vanos);
        if prismas.is_empty() {
            continue;
        }
        v.push((Malla::de_prismas(&prismas), capa.clone()));
        for h in &vanos {
            huecos += 1;
            let (grueso, capa) = if h.puerta {
                (0.04 * m, esp::CAPA_PUERTAS)
            } else {
                (0.02 * m, esp::CAPA_VENTANAS)
            };
            if let Some(p) = modelado::hoja(t, h, grueso) {
                v.push((p.malla(), capa.to_string()));
            }
        }
    }
    (v, tramos.len(), huecos)
}

impl Muro3d {
    fn hacer(&mut self, cx: &mut Contexto, alto: f64) -> Flujo {
        if !alto.is_finite() || alto <= 0.0 {
            cx.decir("La altura tiene que ser mayor que cero");
            return Flujo::Sigue;
        }
        let objetos: Vec<EntityType> = self.objetos.iter().map(|(_, e)| e.clone()).collect();
        let (v, tramos, vanos) = muros_3d(
            &cx.dibujo.doc,
            &objetos,
            self.espesor,
            cx.ajustes.espesor_muro,
            self.base,
            alto,
        );
        if v.is_empty() {
            cx.decir(
                "No hay muros: elija las dos caras de un muro, o su eje (lineas o polilineas)",
            );
            return Flujo::Termina;
        }
        poner(cx, "MURO3D", v);
        let a = cx.medida(alto);
        cx.decir(format!(
            "{tramos} tramo(s) de muro de {a} de alto, con {vanos} vano(s)"
        ));
        Flujo::Termina
    }
}

impl Herramienta for Muro3d {
    fn indicacion(&self, cx: &Lectura) -> String {
        match self.paso {
            PasoMuro::Alto => format!(
                "Altura del muro <{}> [Base: {}/Espesor (por el eje)]:",
                cx.medida(metro(cx) * 2.6),
                cx.medida(self.base)
            ),
            PasoMuro::Base => "Cota de la base del muro <0>:".into(),
            PasoMuro::Espesor => {
                format!("Espesor del muro <{}>:", cx.medida(cx.ajustes.espesor_muro))
            }
        }
    }

    fn al_tener_objetos(&mut self, cx: &mut Contexto) -> Flujo {
        self.objetos = cx.elegidas();
        Flujo::Sigue
    }

    fn texto(&mut self, _cx: &mut Contexto, t: &str) -> Flujo {
        if self.paso != PasoMuro::Alto {
            return Flujo::NoEntiende;
        }
        match ordenes::sin_tildes(t.trim()).to_uppercase().as_str() {
            "B" | "BASE" => self.paso = PasoMuro::Base,
            "E" | "ESPESOR" | "EJE" => self.paso = PasoMuro::Espesor,
            _ => return Flujo::NoEntiende,
        }
        Flujo::Sigue
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        match self.paso {
            PasoMuro::Alto => self.hacer(cx, v),
            PasoMuro::Base => {
                self.base = v;
                self.paso = PasoMuro::Alto;
                Flujo::Sigue
            }
            PasoMuro::Espesor => {
                if v <= 0.0 {
                    cx.decir("El espesor tiene que ser mayor que cero");
                } else {
                    self.espesor = Some(v);
                    self.paso = PasoMuro::Alto;
                }
                Flujo::Sigue
            }
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        match self.paso {
            PasoMuro::Alto => {
                let alto = metro(&cx.leer()) * 2.6;
                self.hacer(cx, alto)
            }
            PasoMuro::Base => {
                self.base = 0.0;
                self.paso = PasoMuro::Alto;
                Flujo::Sigue
            }
            PasoMuro::Espesor => {
                self.espesor = Some(cx.ajustes.espesor_muro);
                self.paso = PasoMuro::Alto;
                Flujo::Sigue
            }
        }
    }
}

// ------------------------------------------------------------------- LOSA

#[derive(Default)]
struct Losa {
    objetos: Vec<(Handle, EntityType)>,
    espesor: Option<f64>,
}

impl Losa {
    fn hacer(&mut self, cx: &mut Contexto, nivel: f64) -> Flujo {
        let espesor = self.espesor.unwrap_or(0.0);
        let mut v = Vec::new();
        let mut error = String::new();
        for (_, e) in &self.objetos {
            match modelado::losa(e, espesor, nivel) {
                Ok(p) => v.push((p.malla(), CAPA_LOSAS.to_string())),
                Err(t) => error = t,
            }
        }
        if v.is_empty() {
            cx.decir(error);
            return Flujo::Termina;
        }
        cx.dibujo.asegurar_capa(CAPA_LOSAS, 8);
        let n = poner(cx, "LOSA", v);
        let (e, z) = (cx.medida(espesor), cx.medida(nivel));
        cx.decir(format!("{n} losa(s) de {e} con la cara de arriba a {z}"));
        Flujo::Termina
    }
}

impl Herramienta for Losa {
    fn indicacion(&self, cx: &Lectura) -> String {
        match self.espesor {
            None => format!("Espesor de la losa <{}>:", cx.medida(metro(cx) * 0.2)),
            Some(_) => "Nivel de la cara de arriba <0>:".into(),
        }
    }

    fn al_tener_objetos(&mut self, cx: &mut Contexto) -> Flujo {
        self.objetos = cx.elegidas();
        Flujo::Sigue
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        match self.espesor {
            None if v <= 0.0 => {
                cx.decir("El espesor tiene que ser mayor que cero");
                Flujo::Sigue
            }
            None => {
                self.espesor = Some(v);
                Flujo::Sigue
            }
            Some(_) => self.hacer(cx, v),
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        match self.espesor {
            None => {
                self.espesor = Some(metro(&cx.leer()) * 0.2);
                Flujo::Sigue
            }
            Some(_) => self.hacer(cx, 0.0),
        }
    }
}

// ---------------------------------------------------------------- NIVELES

/// Pide «nombre cota» hasta Enter; Lista los dice y Borrar los quita.
struct Niveles;

/// Lo escrito como nivel: el nombre y, al final, la cota (`Planta 1 2800`,
/// `Planta 1 = 2800`).
pub fn leer_nivel(t: &str) -> Option<Nivel> {
    let t = t.trim();
    let (nombre, cota) = t.rsplit_once(['=', ' ', '\t'])?;
    let cota: f64 = cota.trim().replace(',', ".").parse().ok()?;
    let nombre = nombre.trim().trim_end_matches('=').trim();
    (cota.is_finite() && !nombre.is_empty()).then(|| Nivel {
        nombre: nombre.to_string(),
        cota,
    })
}

impl Niveles {
    fn listar(cx: &mut Contexto) {
        let v = escena::niveles(&cx.dibujo.doc);
        if v.is_empty() {
            cx.decir("El dibujo no tiene niveles");
        }
        for n in v {
            let c = cx.medida(n.cota);
            cx.decir(format!("  {}: {c}", n.nombre));
        }
    }
}

impl Herramienta for Niveles {
    fn indicacion(&self, _cx: &Lectura) -> String {
        "Nivel: nombre y cota («Planta 1 2800») [Lista/Borrar] <terminar>:".into()
    }

    fn pide_texto(&self) -> bool {
        true
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        Niveles::listar(cx);
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        match ordenes::sin_tildes(t.trim()).to_uppercase().as_str() {
            "L" | "LISTA" => {
                Niveles::listar(cx);
                return Flujo::Sigue;
            }
            "B" | "BORRAR" => {
                escena::poner_niveles(&mut cx.dibujo.doc, &[]);
                cx.dibujo.tocar();
                cx.decir("Niveles borrados");
                return Flujo::Sigue;
            }
            _ => {}
        }
        let Some(n) = leer_nivel(t) else {
            cx.decir("Escriba el nombre y la cota, por ejemplo «Planta 1 2800»");
            return Flujo::Sigue;
        };
        let mut v = escena::niveles(&cx.dibujo.doc);
        v.retain(|x| !x.nombre.eq_ignore_ascii_case(&n.nombre));
        let c = cx.medida(n.cota);
        cx.decir(format!("Nivel «{}» a {c}", n.nombre));
        v.push(n);
        escena::poner_niveles(&mut cx.dibujo.doc, &v);
        cx.dibujo.tocar();
        Flujo::Sigue
    }
}

// ------------------------------------------------------------- EXPORTAR3D

struct Exportar3d;

impl Herramienta for Exportar3d {
    fn indicacion(&self, _cx: &Lectura) -> String {
        "Fichero (.ifc, .glb, .obj, .stl, .dxf o .dwg):".into()
    }

    fn pide_texto(&self) -> bool {
        true
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        let sugerido = cx
            .dibujo
            .ruta
            .as_ref()
            .and_then(|r| r.file_stem())
            .map_or_else(|| "modelo".into(), |n| n.to_string_lossy().into_owned());
        cx.memoria.pedidos.push(PedidoApp::Archivo {
            guardar: true,
            titulo: "Exportar el modelo 3D".into(),
            filtros: tresd::FORMATOS
                .iter()
                .map(|(n, e)| (n.to_string(), format!("*.{e}")))
                .collect(),
            sugerido: format!("{sugerido}.ifc"),
        });
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        let mut ruta = PathBuf::from(t.trim().trim_matches('"'));
        if ruta.extension().is_none() {
            ruta.set_extension("ifc");
        }
        let escena = Escena::de_documento(&cx.dibujo.doc, false);
        match tresd::exportar(&escena, &ruta) {
            Ok(r) => cx.decir(format!("Exportado {}: {r}", ruta.display())),
            Err(e) => cx.decir(format!("No se pudo exportar: {e}")),
        }
        Flujo::Termina
    }
}

// ------------------------------------------------------------- IMPORTAR3D

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PasoImportar {
    #[default]
    Fichero,
    Unidades,
    Punto,
}

#[derive(Default)]
struct Importar3d {
    paso: PasoImportar,
    ruta: PathBuf,
    /// Cuantas unidades del dibujo es una del fichero.
    escala: f64,
    y_arriba: bool,
}

impl Importar3d {
    fn extension(&self) -> String {
        self.ruta
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    }

    fn hacer(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        let r = importar::importar(&self.ruta, self.escala, self.y_arriba, [p[0], p[1], 0.0]);
        let im = match r {
            Ok(im) => im,
            Err(e) => {
                cx.decir(format!("No se pudo importar: {e}"));
                return Flujo::Termina;
            }
        };
        for (nombre, color) in &im.capas {
            if !cx.dibujo.doc.layers.contains(nombre) {
                let c = if matches!(color, Color::ByLayer | Color::ByBlock | Color::None) {
                    Color::Index(7)
                } else {
                    *color
                };
                let _ = cx
                    .dibujo
                    .doc
                    .layers
                    .add(Layer::with_color(nombre.clone(), c));
                cx.dibujo.tocar();
            }
        }
        let actual = cx.dibujo.capa_actual();
        let hs: Vec<Handle> = cx.dibujo.editar("IMPORTAR3D", |t| {
            im.entidades
                .into_iter()
                .filter_map(|e| {
                    let capa = e.common().layer.clone();
                    let capa = if capa.is_empty() {
                        actual.clone()
                    } else {
                        capa
                    };
                    t.anadir_en(e, &capa)
                })
                .collect()
        });
        cx.decir(format!(
            "{} malla(s) de {} a escala {}",
            hs.len(),
            self.ruta.display(),
            self.escala
        ));
        *cx.seleccion = hs;
        Flujo::Termina
    }
}

impl Herramienta for Importar3d {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match self.paso {
            PasoImportar::Fichero => "Fichero 3D (.stl, .obj, .gltf, .glb, .fbx o .dae):".into(),
            PasoImportar::Unidades => format!(
                "Unidades del fichero [Metros/Centimetros/Milimetros/Pulgadas/Dibujo] o escala <{}> [Y arriba: {}]:",
                self.escala,
                if self.y_arriba { "Si" } else { "No" }
            ),
            PasoImportar::Punto => "Punto de insercion <0,0>:".into(),
        }
    }

    fn pide_texto(&self) -> bool {
        self.paso == PasoImportar::Fichero
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        let pat = importar::EXTENSIONES
            .iter()
            .map(|e| format!("*.{e}"))
            .collect::<Vec<_>>()
            .join(";");
        cx.memoria.pedidos.push(PedidoApp::Archivo {
            guardar: false,
            titulo: "Importar un modelo 3D".into(),
            filtros: vec![("Modelos 3D".into(), pat)],
            sugerido: String::new(),
        });
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        let m = metro(&cx.leer());
        match self.paso {
            PasoImportar::Fichero => {
                self.ruta = PathBuf::from(t.trim().trim_matches('"'));
                let ext = self.extension();
                if !importar::EXTENSIONES.contains(&ext.as_str()) {
                    cx.decir(format!("No se importar «.{ext}»"));
                    return Flujo::Sigue;
                }
                if !self.ruta.is_file() {
                    cx.decir(format!("No existe {}", self.ruta.display()));
                    return Flujo::Sigue;
                }
                self.escala = importar::metros_por_defecto(&ext).map_or(1.0, |k| k * m);
                self.y_arriba = importar::y_arriba_por_defecto(&ext);
                self.paso = PasoImportar::Unidades;
                Flujo::Sigue
            }
            PasoImportar::Unidades => {
                let metros = match ordenes::sin_tildes(t.trim()).to_uppercase().as_str() {
                    "M" | "METROS" => Some(1.0),
                    "C" | "CM" | "CENTIMETROS" => Some(0.01),
                    "MM" | "MILIMETROS" => Some(0.001),
                    "P" | "PULGADAS" | "IN" => Some(0.0254),
                    "D" | "DIBUJO" => {
                        self.escala = 1.0;
                        self.paso = PasoImportar::Punto;
                        return Flujo::Sigue;
                    }
                    "Y" => {
                        self.y_arriba = !self.y_arriba;
                        return Flujo::Sigue;
                    }
                    _ => None,
                };
                match metros {
                    Some(k) => {
                        self.escala = k * m;
                        self.paso = PasoImportar::Punto;
                        Flujo::Sigue
                    }
                    None => Flujo::NoEntiende,
                }
            }
            PasoImportar::Punto => Flujo::NoEntiende,
        }
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        if self.paso != PasoImportar::Unidades {
            return Flujo::NoEntiende;
        }
        if !(v.is_finite() && v > 0.0) {
            cx.decir("La escala tiene que ser mayor que cero");
            return Flujo::Sigue;
        }
        self.escala = v;
        self.paso = PasoImportar::Punto;
        Flujo::Sigue
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        if self.paso != PasoImportar::Punto {
            return Flujo::NoEntiende;
        }
        self.hacer(cx, p)
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        match self.paso {
            PasoImportar::Fichero => Flujo::Termina,
            PasoImportar::Unidades => {
                self.paso = PasoImportar::Punto;
                Flujo::Sigue
            }
            PasoImportar::Punto => self.hacer(cx, [0.0, 0.0]),
        }
    }
}

// ------------------------------------------------------------------ VER3D

struct Ver3d;

impl Herramienta for Ver3d {
    fn indicacion(&self, _cx: &Lectura) -> String {
        String::new()
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        cx.memoria.pedidos.push(PedidoApp::Ver3d(None));
        cx.decir("Abriendo el dibujo en 3D…");
        Flujo::Termina
    }
}

#[cfg(test)]
mod pruebas {
    use std::collections::{BTreeMap, HashSet};
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::{Dibujo, Sesion};

    fn sesion() -> Sesion {
        Sesion::nueva(Dibujo::nuevo())
    }

    fn carpeta(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cad-nucleo-3d-{nombre}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn anadir(s: &mut Sesion, v: Vec<EntityType>, capa: &str) -> Vec<Handle> {
        s.dibujo.asegurar_capa(capa, esp::color_de_capa(capa));
        s.dibujo.editar("PRUEBA", |t| {
            v.into_iter().filter_map(|e| t.anadir_en(e, capa)).collect()
        })
    }

    fn mallas(s: &Sesion) -> Vec<(String, Malla)> {
        s.dibujo
            .doc
            .model_space_entities()
            .filter(|e| matches!(e, EntityType::Mesh(_)))
            .map(|e| {
                (
                    e.common().layer.clone(),
                    crate::tresd::geometria::malla_de_entidad(e).unwrap(),
                )
            })
            .collect()
    }

    fn ultimo(s: &Sesion) -> &str {
        s.registro.last().map_or("", |t| t.as_str())
    }

    /// Un plano con un muro de MURO (5 m de 150) con una puerta y una
    /// ventana, levantado, y una losa.
    fn casita() -> Sesion {
        let mut s = sesion();
        let muro = anadir(
            &mut s,
            esp::muro(&[[0.0, 0.0], [5000.0, 0.0]], 150.0, false),
            esp::CAPA_MUROS,
        );
        anadir(
            &mut s,
            esp::puerta([1000.0, 0.0], [2000.0, 0.0], 900.0, true),
            esp::CAPA_PUERTAS,
        );
        anadir(
            &mut s,
            esp::ventana([3000.0, 0.0], [4200.0, 0.0], 150.0),
            esp::CAPA_VENTANAS,
        );
        s.seleccion = muro;
        s.escribir("muro3d");
        s.escribir("");
        assert!(s.activa().is_none(), "{:?}", s.registro);
        assert!(ultimo(&s).contains("1 tramo"), "{}", ultimo(&s));
        let piso = anadir(
            &mut s,
            vec![esp::polilinea(
                &[[0.0, -3000.0], [5000.0, -3000.0], [5000.0, 0.0], [0.0, 0.0]],
                true,
            )],
            "0",
        );
        s.seleccion = piso;
        s.escribir("losa");
        s.escribir("");
        s.escribir("0");
        assert!(ultimo(&s).contains("1 losa"), "{}", ultimo(&s));
        s
    }

    #[test]
    fn extruir_contornos_y_rechazar_lo_abierto() {
        let mut s = sesion();
        let mut hs = anadir(
            &mut s,
            vec![
                esp::polilinea(
                    &[[0.0, 0.0], [4000.0, 0.0], [4000.0, 3000.0], [0.0, 3000.0]],
                    true,
                ),
                esp::circulo([8000.0, 0.0], 500.0),
            ],
            "VOLUMENES",
        );
        hs.extend(anadir(
            &mut s,
            vec![esp::polilinea(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], false)],
            "VOLUMENES",
        ));
        s.seleccion = hs;
        s.escribir("ext");
        // La altura cero no vale: sigue pidiendo.
        s.escribir("0");
        assert!(ultimo(&s).contains("cero"), "{}", ultimo(&s));
        assert!(s.activa().is_some());
        s.escribir("3000");
        assert!(s.activa().is_none());
        assert!(ultimo(&s).contains("2 solido"), "{}", ultimo(&s));
        assert!(ultimo(&s).contains("1 objeto(s) no son"), "{}", ultimo(&s));
        let m = mallas(&s);
        assert_eq!(m.len(), 2);
        assert!(m.iter().all(|(c, _)| c == "VOLUMENES"));
        let p = m[0].1.prismas().unwrap();
        assert!((p[0].volumen() - 4000.0 * 3000.0 * 3000.0).abs() < 1e-3);
        // Lo creado queda elegido y se deshace de una vez.
        assert_eq!(s.seleccion.len(), 2);
        s.dibujo.deshacer();
        assert!(mallas(&s).is_empty());

        // Solo lo abierto: ninguno.
        let h = anadir(
            &mut s,
            vec![esp::polilinea(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], false)],
            "0",
        );
        s.seleccion = h;
        s.escribir("extruir");
        s.escribir("");
        assert!(ultimo(&s).contains("contorno cerrado"), "{}", ultimo(&s));
        assert!(mallas(&s).is_empty());
    }

    #[test]
    fn altura_y_elevacion_como_autocad() {
        let mut s = sesion();
        let hs = anadir(
            &mut s,
            vec![
                esp::linea([0.0, 0.0], [1000.0, 0.0]),
                esp::texto("x", [0.0, 0.0], 1.0, 0.0, false),
            ],
            "0",
        );
        s.seleccion = hs.clone();
        s.escribir("alt");
        s.escribir("2500");
        s.escribir("100");
        assert!(ultimo(&s).contains("1 objeto"), "{}", ultimo(&s));
        let EntityType::Line(l) = s.dibujo.entidad(hs[0]).unwrap() else {
            panic!()
        };
        assert_eq!((l.thickness, l.start.z, l.end.z), (2500.0, 100.0, 100.0));
        let e = Escena::de_documento(&s.dibujo.doc, true);
        assert_eq!(e.elementos.len(), 1);
        assert_eq!(e.elementos[0].malla.caja().unwrap()[5], 2600.0);
    }

    #[test]
    fn muro_con_vanos_losa_y_niveles() {
        let mut s = casita();
        let e = Escena::de_documento(&s.dibujo.doc, false);
        let cuenta = |t: Tipo| e.elementos.iter().filter(|x| x.tipo == t).count();
        assert_eq!(cuenta(Tipo::Muro), 1);
        assert_eq!(cuenta(Tipo::Puerta), 1);
        assert_eq!(cuenta(Tipo::Ventana), 1);
        assert_eq!(cuenta(Tipo::Losa), 1);
        let muro = e.elementos.iter().find(|x| x.tipo == Tipo::Muro).unwrap();
        assert_eq!(muro.prismas.len(), 6);
        let vol: f64 = muro.prismas.iter().map(|p| p.volumen()).sum();
        let esperado = 150.0 * (5000.0 * 2600.0 - 900.0 * 2100.0 - 1200.0 * 1200.0);
        assert!((vol - esperado).abs() < 1.0, "{vol}");
        // El vidrio deja ver.
        let v = e
            .elementos
            .iter()
            .find(|x| x.tipo == Tipo::Ventana)
            .unwrap();
        assert!(v.color[3] < 1.0);

        // NIVELES: se guardan en el dibujo y reparten las piezas.
        s.escribir("niveles");
        assert!(s.pide_texto());
        s.escribir("Planta baja 0");
        s.escribir("Planta 1 = 2800");
        s.escribir("nada");
        assert!(ultimo(&s).contains("Escriba"));
        s.escribir("");
        assert!(s.activa().is_none());
        let n = escena::niveles(&s.dibujo.doc);
        assert_eq!(n.len(), 2);
        assert_eq!(n[1].nombre, "Planta 1");
        let e = Escena::de_documento(&s.dibujo.doc, false);
        // La losa cuelga del 0 (su base esta en -200): al primer nivel.
        assert!(e.elementos.iter().all(|x| x.nivel == Some(0)));
        assert_eq!(leer_nivel("Azotea 5,6").unwrap().cota, 5.6);
        assert!(leer_nivel("solo").is_none());
    }

    #[test]
    fn muro_por_su_eje_con_espesor() {
        let mut s = sesion();
        let h = anadir(
            &mut s,
            vec![esp::polilinea(
                &[[0.0, 0.0], [4000.0, 0.0], [4000.0, 3000.0]],
                false,
            )],
            "EJES",
        );
        s.seleccion = h;
        s.escribir("m3d");
        s.escribir("e");
        s.escribir("200");
        s.escribir("b");
        s.escribir("100");
        s.escribir("3000");
        assert!(ultimo(&s).contains("2 tramo"), "{}", ultimo(&s));
        let m = mallas(&s);
        assert_eq!(m.len(), 2);
        // Sin capa de muros elegida: van a A-MUROS.
        assert!(m.iter().all(|(c, _)| c == esp::CAPA_MUROS));
        let caja = m[0].1.caja().unwrap();
        assert_eq!((caja[2], caja[5]), (100.0, 3100.0));
    }

    fn exportar_a(s: &Sesion, ruta: &Path) {
        let e = Escena::de_documento(&s.dibujo.doc, false);
        crate::tresd::exportar(&e, ruta).unwrap();
    }

    #[test]
    fn exportar_obj_stl_y_glb() {
        let mut s = casita();
        // Una cara 3D suelta: un elemento sin prismas.
        let cara = opencadcodec::entities::Face3D::triangle(
            opencadcodec::Vector3::new(0.0, 0.0, 3000.0),
            opencadcodec::Vector3::new(1000.0, 0.0, 3000.0),
            opencadcodec::Vector3::new(0.0, 1000.0, 3500.0),
        );
        anadir(&mut s, vec![EntityType::Face3D(cara)], "CUBIERTA");
        let e = Escena::de_documento(&s.dibujo.doc, false);
        assert_eq!(e.elementos.len(), 5);
        let caras: usize = e
            .elementos
            .iter()
            .map(|x| x.malla_completa().caras.len())
            .sum();
        let tris: usize = e
            .elementos
            .iter()
            .map(|x| x.malla_completa().triangulos().len())
            .sum();
        let d = carpeta("exportar");

        let obj = d.join("casa.obj");
        exportar_a(&s, &obj);
        let t = std::fs::read_to_string(&obj).unwrap();
        assert_eq!(t.lines().filter(|l| l.starts_with("f ")).count(), caras);
        assert_eq!(t.lines().filter(|l| l.starts_with("o ")).count(), 5);
        assert!(t.contains("mtllib casa.mtl"));
        let mtl = std::fs::read_to_string(d.join("casa.mtl")).unwrap();
        assert!(mtl.contains("newmtl A-MUROS"));
        assert!(mtl.contains("newmtl A-VENTANAS") && mtl.contains("d 0.45"));
        // En metros y con la Y arriba: el muro sube hasta 2,6.
        assert!(t.contains(" 2.6 "), "{}", &t[..300]);

        let stl = d.join("casa.stl");
        exportar_a(&s, &stl);
        let b = std::fs::read(&stl).unwrap();
        assert!(!b.starts_with(b"solid"));
        assert_eq!(crate::tresd::stl::triangulos(&b) as usize, tris);
        assert_eq!(b.len(), 84 + 50 * tris);

        let glb = d.join("casa.glb");
        exportar_a(&s, &glb);
        let b = std::fs::read(&glb).unwrap();
        assert_eq!(&b[0..4], b"glTF");
        let u = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
        assert_eq!(u(4), 2);
        assert_eq!(u(8), b.len());
        assert_eq!(&b[16..20], b"JSON");
        let largo_json = u(12);
        assert_eq!(largo_json % 4, 0);
        let j: serde_json::Value = serde_json::from_slice(&b[20..20 + largo_json]).unwrap();
        assert_eq!(j["asset"]["version"], "2.0");
        assert_eq!(j["nodes"].as_array().unwrap().len(), 6);
        assert_eq!(j["meshes"].as_array().unwrap().len(), 5);
        assert_eq!(j["nodes"][1]["extras"]["ifc"], "IfcWall");
        let bin = 20 + largo_json;
        assert_eq!(&b[bin + 4..bin + 8], b"BIN\0");
        assert_eq!(
            j["buffers"][0]["byteLength"].as_u64().unwrap() as usize,
            tris * 3 * 24
        );
        let suma: u64 = j["accessors"]
            .as_array()
            .unwrap()
            .iter()
            .step_by(2)
            .map(|a| a["count"].as_u64().unwrap())
            .sum();
        assert_eq!(suma as usize, tris * 3);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn exportar_ifc4_y_leerlo_con_ifc_lite() {
        let mut s = casita();
        escena::poner_niveles(
            &mut s.dibujo.doc,
            &[
                Nivel {
                    nombre: "Planta baja".into(),
                    cota: -200.0,
                },
                Nivel {
                    nombre: "Planta 1".into(),
                    cota: 2800.0,
                },
            ],
        );
        // Una caja sin capa de oficio: un IfcBuildingElementProxy.
        let caja = anadir(
            &mut s,
            vec![esp::polilinea(
                &[
                    [6000.0, 0.0],
                    [7000.0, 0.0],
                    [7000.0, 800.0],
                    [6000.0, 800.0],
                ],
                true,
            )],
            "MOBILIARIO",
        );
        s.seleccion = caja;
        s.escribir("ext");
        s.escribir("750");
        let d = carpeta("ifc");
        let ruta = d.join("casa.ifc");
        exportar_a(&s, &ruta);
        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert!(texto.starts_with("ISO-10303-21;"));
        assert!(texto.contains("FILE_SCHEMA(('IFC4'))"));
        assert_eq!(texto.matches("IFCBUILDINGSTOREY(").count(), 2);
        assert!(texto.contains("IFCEXTRUDEDAREASOLID("));
        assert!(texto.contains("IFCMATERIAL('A-MUROS'"));
        assert!(texto.contains("'Pset_CADpixpin'"));

        let r = ifc_lite_processing::process_geometry_filtered_with_quality_and_ids(
            texto.as_bytes(),
            ifc_lite_processing::OpeningFilterMode::Default,
            ifc_lite_processing::TessellationQuality::default(),
            None,
        );
        let mut por_tipo: BTreeMap<String, HashSet<u32>> = BTreeMap::new();
        for m in &r.meshes {
            if m.indices.is_empty() {
                continue;
            }
            por_tipo
                .entry(m.ifc_type.clone())
                .or_default()
                .insert(m.express_id);
        }
        let n = |t: &str| por_tipo.get(t).map_or(0, |v| v.len());
        assert_eq!(n("IfcWall"), 1, "{por_tipo:?}");
        assert_eq!(n("IfcSlab"), 1, "{por_tipo:?}");
        assert_eq!(n("IfcDoor"), 1, "{por_tipo:?}");
        assert_eq!(n("IfcWindow"), 1, "{por_tipo:?}");
        assert_eq!(n("IfcBuildingElementProxy"), 1, "{por_tipo:?}");
        // El muro, en metros: 5 x 0,15 x 2,6 sin los vanos (el volumen de
        // sus triangulos).
        let vol: f64 = r
            .meshes
            .iter()
            .filter(|m| m.ifc_type == "IfcWall")
            .map(|m| {
                let p = |i: u32| {
                    let k = i as usize * 3;
                    [
                        m.positions[k] as f64 + m.origin[0],
                        m.positions[k + 1] as f64 + m.origin[1],
                        m.positions[k + 2] as f64 + m.origin[2],
                    ]
                };
                m.indices
                    .chunks_exact(3)
                    .map(|t| {
                        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                        let x = crate::tresd::geometria::cruz3(b, c);
                        (a[0] * x[0] + a[1] * x[1] + a[2] * x[2]) / 6.0
                    })
                    .sum::<f64>()
            })
            .sum();
        let esperado = 0.15 * (5.0 * 2.6 - 0.9 * 2.1 - 1.2 * 1.2);
        assert!((vol.abs() - esperado).abs() < 1e-3, "{vol} {esperado}");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn exportar_dxf_y_dwg_con_mallas() {
        let s = casita();
        let d = carpeta("cad");
        for ext in ["dxf", "dwg"] {
            let ruta = d.join(format!("casa.{ext}"));
            exportar_a(&s, &ruta);
            let doc = crate::dibujo::leer(&ruta).unwrap();
            let e = Escena::de_documento(&doc, false);
            assert_eq!(e.elementos.len(), 4, "{ext}");
            assert!(
                e.elementos
                    .iter()
                    .any(|x| x.tipo == Tipo::Muro && x.prismas.len() == 6),
                "{ext}"
            );
        }
        // Sin nada en 3D, o a un formato que no es, no se exporta.
        assert!(crate::tresd::exportar(&Escena::default(), &d.join("x.ifc")).is_err());
        let e = Escena::de_documento(&s.dibujo.doc, false);
        assert!(crate::tresd::exportar(&e, &d.join("x.png")).is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn exportar3d_pide_el_fichero_a_la_app() {
        let mut s = casita();
        s.escribir("exp3d");
        let Some(PedidoApp::Archivo {
            guardar, filtros, ..
        }) = s.memoria.pedidos.pop()
        else {
            panic!("{:?}", s.memoria.pedidos)
        };
        assert!(guardar);
        assert!(filtros.iter().any(|f| f.1 == "*.ifc"));
        assert!(s.pide_texto());
        let d = carpeta("orden");
        let ruta = d.join("con espacios");
        s.escribir(&ruta.to_string_lossy());
        assert!(s.activa().is_none());
        assert!(ultimo(&s).contains("IFC4 con 4"), "{}", ultimo(&s));
        assert!(ruta.with_extension("ifc").is_file());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn importar_stl_y_obj_a_escala() {
        let d = carpeta("importar");
        let stl = d.join("tetra.stl");
        std::fs::write(
            &stl,
            "solid t\n\
             facet normal 0 0 -1\nouter loop\nvertex 0 0 0\nvertex 0 10 0\nvertex 10 0 0\nendloop\nendfacet\n\
             facet normal 0 -1 0\nouter loop\nvertex 0 0 0\nvertex 10 0 0\nvertex 0 0 10\nendloop\nendfacet\n\
             facet normal -1 0 0\nouter loop\nvertex 0 0 0\nvertex 0 0 10\nvertex 0 10 0\nendloop\nendfacet\n\
             facet normal 1 1 1\nouter loop\nvertex 10 0 0\nvertex 0 10 0\nvertex 0 0 10\nendloop\nendfacet\n\
             endsolid t\n",
        )
        .unwrap();
        let mut s = sesion();
        s.escribir("imp3d");
        assert!(matches!(
            s.memoria.pedidos.pop(),
            Some(PedidoApp::Archivo { guardar: false, .. })
        ));
        s.escribir(&stl.to_string_lossy());
        // STL: en las unidades del dibujo; x2 escrito a mano.
        s.escribir("2");
        s.escribir("100,0");
        assert!(ultimo(&s).contains("1 malla"), "{}", ultimo(&s));
        let m = mallas(&s);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].1.triangulos().len(), 4);
        let c = m[0].1.caja().unwrap();
        assert_eq!((c[0], c[3], c[5]), (100.0, 120.0, 20.0));

        // OBJ: en metros y con la Y arriba (como Blender).
        let obj = d.join("caja.obj");
        std::fs::write(&obj, "v 0 0 0\nv 1 0 0\nv 1 2 0\nv 0 2 0\nf 1 2 3 4\n").unwrap();
        let mut s = sesion();
        s.escribir("importar3d");
        s.escribir(&obj.to_string_lossy());
        s.escribir("");
        s.escribir("");
        let m = mallas(&s);
        assert_eq!(m.len(), 1, "{:?}", s.registro);
        let c = m[0].1.caja().unwrap();
        // La Y del fichero (2 m) es la Z del dibujo (2000 mm).
        assert_eq!((c[3], c[5]), (1000.0, 2000.0));

        // Lo que no es 3D o no existe.
        let mut s = sesion();
        s.escribir("imp3d");
        s.escribir("plano.pdf");
        assert!(ultimo(&s).contains(".pdf"));
        s.escribir(&d.join("no-esta.stl").to_string_lossy());
        assert!(ultimo(&s).contains("No existe"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn ver3d_pide_la_ventana() {
        let mut s = sesion();
        s.escribir("3d");
        assert_eq!(s.memoria.pedidos.pop(), Some(PedidoApp::Ver3d(None)));
        assert!(s.activa().is_none());
        s.escribir("v3");
        assert_eq!(s.memoria.pedidos.pop(), Some(PedidoApp::Ver3d(None)));
    }

    #[test]
    fn la_escena_lleva_las_rayas_a_su_cota() {
        let mut s = casita();
        let EntityType::LwPolyline(mut p) =
            esp::polilinea(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], false)
        else {
            panic!()
        };
        p.elevation = 500.0;
        anadir(&mut s, vec![EntityType::LwPolyline(p)], "0");
        let e = Escena::de_documento(&s.dibujo.doc, true);
        assert!(
            e.lineas
                .iter()
                .any(|l| l.puntos.len() == 2 && l.puntos.iter().all(|q| q[2] == 500.0))
        );
        // Sin lineas, solo las piezas.
        assert!(Escena::de_documento(&s.dibujo.doc, false).lineas.is_empty());
    }
}
