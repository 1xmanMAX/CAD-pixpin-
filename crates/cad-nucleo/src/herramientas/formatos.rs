//! **Las ordenes de formatos 2D** (ver [`crate::formatos`]): IMPORTARPDF,
//! IMPORTARSVG, EXPORTARPDF, EXPORTARSVG, IMAGEN, GUARDARVERSION,
//! EXPORTARDXF (WBLOQUE de lo elegido) y ESCALAREF para calibrar lo
//! importado con una medida conocida.
//!
//! Las que leen o escriben un fichero piden a la app su dialogo
//! ([`PedidoApp::Archivo`]) y esperan la ruta como texto (tambien se puede
//! escribir a mano); Esc cancela.

use std::path::{Path, PathBuf};

use opencadcodec::Handle;

use crate::especialidad as esp;
use crate::formatos::escena::{self, Que};
use crate::formatos::pdf::{self, Hoja, OpcionesPdf, Papel};
use crate::formatos::{self, Importado, imagen, svg, version, wbloque};
use crate::geo::{self, Caja, Forma, P};
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, PedidoApp, Previa, Registro};
use crate::operaciones as op;
use crate::ordenes::{self, Grupo, Id, Orden};

pub const REGISTROS: &[Registro] = &[
    Registro {
        orden: Orden {
            id: Id::Otra("IMPORTARPDF"),
            nombre: "IMPORTARPDF",
            alias: &["PDFIMPORT", "IMPPDF"],
            grupo: Grupo::Vista,
            ayuda: "Trae los vectores y textos de una pagina de PDF como lineas, polilineas, sombreados y textos",
            con_objetos: false,
        },
        etiqueta: "Importar PDF",
        crear: || Box::new(Importar::nuevo(Origen::Pdf)),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("IMPORTARSVG"),
            nombre: "IMPORTARSVG",
            alias: &["IMPORTSVG", "SVGIMPORT", "IMPSVG"],
            grupo: Grupo::Vista,
            ayuda: "Trae un dibujo SVG (caminos, lineas, circulos, textos) con sus capas o colores",
            con_objetos: false,
        },
        etiqueta: "Importar SVG",
        crear: || Box::new(Importar::nuevo(Origen::Svg)),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("EXPORTARPDF"),
            nombre: "EXPORTARPDF",
            alias: &["EXPORTPDF", "EXPPDF", "PLOTPDF"],
            grupo: Grupo::Vista,
            ayuda: "PDF vectorial de todo, una ventana o cada lamina, con papel, escala, grosores y capas",
            con_objetos: false,
        },
        etiqueta: "Exportar PDF",
        crear: || Box::new(Exportar::nuevo(Destino::Pdf)),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("EXPORTARSVG"),
            nombre: "EXPORTARSVG",
            alias: &["EXPORTSVG", "EXPSVG"],
            grupo: Grupo::Vista,
            ayuda: "SVG de todo o de una ventana, con una capa de Inkscape por capa",
            con_objetos: false,
        },
        etiqueta: "Exportar SVG",
        crear: || Box::new(Exportar::nuevo(Destino::Svg)),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("IMAGEN"),
            nombre: "IMAGEN",
            alias: &["IMAGEATTACH", "IAT", "ADJUNTARIMAGEN"],
            grupo: Grupo::Vista,
            ayuda: "Enlaza una imagen PNG, JPG, BMP, TIF o GIF con su punto, ancho y giro",
            con_objetos: false,
        },
        etiqueta: "Imagen",
        crear: || Box::new(Imagen::default()),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("GUARDARVERSION"),
            nombre: "GUARDARVERSION",
            alias: &["SAVEASVERSION", "VERSIONDWG"],
            grupo: Grupo::Vista,
            ayuda: "Elige la version de DWG/DXF (2000 a 2018) en que se guarda, y guarda",
            con_objetos: false,
        },
        etiqueta: "Guardar versión",
        crear: || Box::new(GuardarVersion::default()),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("EXPORTARDXF"),
            nombre: "EXPORTARDXF",
            alias: &["W", "WBLOQUE", "WBLOCK"],
            grupo: Grupo::Vista,
            ayuda: "Escribe solo lo elegido (con sus capas y bloques) en un DWG o DXF nuevo",
            con_objetos: true,
        },
        etiqueta: "Exportar elegidos",
        crear: || Box::new(ExportarElegidos::default()),
    },
    Registro {
        orden: Orden {
            id: Id::Otra("ESCALAREF"),
            nombre: "ESCALAREF",
            alias: &["SCALEREF", "ESCREF"],
            grupo: Grupo::Modificar,
            ayuda: "Escala lo elegido para que dos puntos queden a una distancia conocida (calibrar un PDF)",
            con_objetos: true,
        },
        etiqueta: "Escala por referencia",
        crear: || Box::new(EscalaRef::default()),
    },
];

/// Una ruta escrita o elegida (sin comillas).
fn ruta_de(t: &str) -> PathBuf {
    PathBuf::from(t.trim().trim_matches('"').trim())
}

/// La ruta con su extension (si no la trae).
fn con_extension(r: PathBuf, ext: &str) -> PathBuf {
    let tiene = r
        .extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(ext));
    if tiene {
        r
    } else {
        let mut s = r.into_os_string();
        s.push(".");
        s.push(ext);
        PathBuf::from(s)
    }
}

/// El nombre que se sugiere: el del dibujo con otra extension.
fn sugerido(cx: &Contexto, ext: &str) -> String {
    cx.dibujo
        .ruta
        .as_deref()
        .and_then(Path::file_stem)
        .map(|s| format!("{}.{ext}", s.to_string_lossy()))
        .unwrap_or_else(|| format!("plano.{ext}"))
}

fn pedir_archivo(
    cx: &mut Contexto,
    guardar: bool,
    titulo: &str,
    filtros: &[(&str, &str)],
    sug: String,
) {
    cx.memoria.pedidos.push(PedidoApp::Archivo {
        guardar,
        titulo: titulo.into(),
        filtros: filtros
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect(),
        sugerido: sug,
    });
}

/// Unidades del dibujo por mm de papel a 1:`e`.
fn unidades_por_mm(cx: &Lectura, e: f64) -> f64 {
    esp::unidades_por_mm(e, cx.dibujo.doc.header.insertion_units)
}

fn rectangulo(a: P, b: P) -> Vec<Forma> {
    let e = [a, [b[0], a[1]], b, [a[0], b[1]]];
    (0..4)
        .map(|i| Forma::Segmento {
            a: e[i],
            b: e[(i + 1) % 4],
        })
        .collect()
}

/// La opcion escrita, sin tildes y en mayusculas.
fn opcion(t: &str) -> String {
    ordenes::sin_tildes(t.trim()).to_uppercase()
}

// ------------------------------------------------------------ IMPORTAR

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origen {
    Pdf,
    Svg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasoImportar {
    Archivo,
    Pagina,
    Escala,
    Punto,
}

/// IMPORTARPDF e IMPORTARSVG: archivo, pagina (PDF), escala y punto.
struct Importar {
    origen: Origen,
    paso: PasoImportar,
    pdf: Option<pdf::Documento>,
    svg: Option<Importado>,
    paginas: usize,
    pagina: usize,
    /// El plano del PDF esta a 1:`escala`.
    escala: f64,
    /// El tamano de la pagina en mm (para la previa).
    papel: (f64, f64),
}

impl Importar {
    fn nuevo(origen: Origen) -> Importar {
        Importar {
            origen,
            paso: PasoImportar::Archivo,
            pdf: None,
            svg: None,
            paginas: 0,
            pagina: 1,
            escala: 1.0,
            papel: (0.0, 0.0),
        }
    }

    fn prefijo(&self) -> &'static str {
        match self.origen {
            Origen::Pdf => "PDF_",
            Origen::Svg => "SVG_",
        }
    }

    fn abrir(&mut self, cx: &mut Contexto, ruta: &Path) -> Flujo {
        match self.origen {
            Origen::Pdf => {
                let doc = match pdf::importar::abrir(ruta) {
                    Ok(d) => d,
                    Err(e) => {
                        cx.decir(format!("No se pudo importar {}: {e}", ruta.display()));
                        return Flujo::Termina;
                    }
                };
                let paginas = doc.paginas();
                self.paginas = paginas.len();
                if self.paginas == 0 {
                    cx.decir("El PDF no tiene paginas");
                    return Flujo::Termina;
                }
                let c = paginas[0].caja;
                self.papel = (
                    (c[2] - c[0]) * pdf::importar::MM_POR_PT,
                    (c[3] - c[1]) * pdf::importar::MM_POR_PT,
                );
                drop(paginas);
                self.pdf = Some(doc);
                cx.decir(format!(
                    "PDF de {} pagina(s), {:.0} x {:.0} mm",
                    self.paginas, self.papel.0, self.papel.1
                ));
                self.paso = if self.paginas > 1 {
                    PasoImportar::Pagina
                } else {
                    PasoImportar::Escala
                };
            }
            Origen::Svg => {
                let texto = match std::fs::read(ruta) {
                    Ok(b) => String::from_utf8_lossy(&b).into_owned(),
                    Err(e) => {
                        cx.decir(format!("No se pudo abrir {}: {e}", ruta.display()));
                        return Flujo::Termina;
                    }
                };
                match svg::importar(&texto) {
                    Ok(imp) => {
                        self.papel = imp.pagina;
                        self.svg = Some(imp);
                        self.paso = PasoImportar::Escala;
                    }
                    Err(e) => {
                        cx.decir(format!("No se pudo importar {}: {e}", ruta.display()));
                        return Flujo::Termina;
                    }
                }
            }
        }
        Flujo::Sigue
    }

    fn importar(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        let imp = match self.origen {
            Origen::Pdf => {
                let Some(doc) = &self.pdf else {
                    return Flujo::Termina;
                };
                match pdf::importar::importar(doc, self.pagina, Default::default()) {
                    Ok(i) => i,
                    Err(e) => {
                        cx.decir(format!(
                            "No se pudo importar la pagina {}: {e}",
                            self.pagina
                        ));
                        return Flujo::Termina;
                    }
                }
            }
            Origen::Svg => self.svg.take().unwrap_or_default(),
        };
        let k = unidades_por_mm(&cx.leer(), self.escala);
        let r = formatos::a_entidades(&imp, self.prefijo(), p, k);
        if r.entidades.is_empty() {
            cx.decir("No hay nada que importar (pagina vacia, o solo imagenes)");
            for a in &imp.avisos {
                cx.decir(format!("  {a}"));
            }
            return Flujo::Termina;
        }
        let hs = poner_importado(cx, r, &format!("IMPORTAR{:?}", self.origen).to_uppercase());
        let textos = imp.rotulos.len();
        cx.decir(format!(
            "Importadas {} entidades ({} textos) a 1:{}",
            hs.len(),
            textos,
            esp::texto_de_medida(self.escala, 2)
        ));
        for a in &imp.avisos {
            cx.decir(format!("  {a}"));
        }
        cx.decir("Para calibrar con una medida conocida: ESCALAREF (quedan elegidas)");
        *cx.seleccion = hs;
        Flujo::Termina
    }
}

/// Crea las capas y las entidades de una importacion (un paso de deshacer).
fn poner_importado(cx: &mut Contexto, r: formatos::Entidades, nombre: &str) -> Vec<Handle> {
    for c in &r.capas {
        if !cx.dibujo.doc.layers.contains(&c.nombre) {
            cx.dibujo.asegurar_capa(&c.nombre, 7);
            let (color, apagada) = (c.color, c.apagada);
            cx.dibujo.con_capa(&c.nombre, |l| {
                l.color = color;
                if apagada {
                    l.turn_off();
                }
            });
        }
    }
    cx.dibujo.editar(nombre, |t| {
        r.entidades
            .into_iter()
            .filter_map(|(e, capa)| t.anadir_en(e, &capa))
            .collect()
    })
}

impl Herramienta for Importar {
    fn indicacion(&self, cx: &Lectura) -> String {
        match self.paso {
            PasoImportar::Archivo => match self.origen {
                Origen::Pdf => "Archivo PDF que importar:".into(),
                Origen::Svg => "Archivo SVG que importar:".into(),
            },
            PasoImportar::Pagina => format!("Pagina (1 a {}) <{}>:", self.paginas, self.pagina),
            PasoImportar::Escala => format!(
                "Escala del plano 1:<{}> (100 si esta a 1/100; {}):",
                esp::texto_de_medida(self.escala, 2),
                if cx.dibujo.doc.header.insertion_units == 4 {
                    "el dibujo esta en mm"
                } else {
                    "en las unidades del dibujo"
                }
            ),
            PasoImportar::Punto => "Punto de insercion (esquina inferior izquierda) <0,0>:".into(),
        }
    }

    fn pide_texto(&self) -> bool {
        self.paso == PasoImportar::Archivo
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        let (titulo, filtros): (&str, &[(&str, &str)]) = match self.origen {
            Origen::Pdf => ("Importar PDF", &[("PDF", "*.pdf")]),
            Origen::Svg => ("Importar SVG", &[("SVG", "*.svg")]),
        };
        pedir_archivo(cx, false, titulo, filtros, String::new());
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        match self.paso {
            PasoImportar::Archivo => self.abrir(cx, &ruta_de(t)),
            _ => Flujo::NoEntiende,
        }
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        match self.paso {
            PasoImportar::Pagina => {
                let n = v.round();
                if v.fract().abs() > 1e-9 || n < 1.0 || n as usize > self.paginas {
                    cx.decir(format!("La pagina va de 1 a {}", self.paginas));
                } else {
                    self.pagina = n as usize;
                    self.paso = PasoImportar::Escala;
                }
                Flujo::Sigue
            }
            PasoImportar::Escala => {
                if v.is_finite() && v > 0.0 {
                    self.escala = v;
                    self.paso = PasoImportar::Punto;
                } else {
                    cx.decir("La escala tiene que ser mayor que cero");
                }
                Flujo::Sigue
            }
            _ => Flujo::NoEntiende,
        }
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        if self.paso == PasoImportar::Punto {
            self.importar(cx, p)
        } else {
            Flujo::NoEntiende
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        match self.paso {
            PasoImportar::Archivo => Flujo::Termina,
            PasoImportar::Pagina => {
                self.paso = PasoImportar::Escala;
                Flujo::Sigue
            }
            PasoImportar::Escala => {
                self.paso = PasoImportar::Punto;
                Flujo::Sigue
            }
            PasoImportar::Punto => self.importar(cx, [0.0, 0.0]),
        }
    }

    fn previa(&self, cx: &Lectura, c: P) -> Previa {
        if self.paso != PasoImportar::Punto {
            return Previa::default();
        }
        let k = unidades_por_mm(cx, self.escala);
        let b = [c[0] + self.papel.0 * k, c[1] + self.papel.1 * k];
        Previa {
            formas: rectangulo(c, b),
            ..Default::default()
        }
    }
}

// ------------------------------------------------------------ EXPORTAR

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Destino {
    Pdf,
    Svg,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PasoExportar {
    Zona,
    Papel,
    Escala,
    Esquina(Option<P>),
    Archivo,
}

/// Lo que se exporta.
#[derive(Debug, Clone, PartialEq)]
enum Zona {
    Todo,
    Ventana(Caja),
    Laminas(Vec<Caja>),
}

/// EXPORTARPDF y EXPORTARSVG.
struct Exportar {
    destino: Destino,
    paso: PasoExportar,
    zona: Zona,
    papel: Papel,
    nombre_papel: String,
    /// 1:`escala`; `None` ajusta al papel.
    escala: Option<f64>,
    opciones: OpcionesPdf,
}

impl Exportar {
    fn nuevo(destino: Destino) -> Exportar {
        Exportar {
            destino,
            paso: PasoExportar::Zona,
            zona: Zona::Todo,
            papel: Papel::Ajustar,
            nombre_papel: "Ajustar".into(),
            escala: None,
            opciones: OpcionesPdf::default(),
        }
    }

    fn pedir(&mut self, cx: &mut Contexto, zona: Zona) -> Flujo {
        self.zona = zona;
        self.paso = PasoExportar::Archivo;
        let (titulo, filtros, ext): (&str, &[(&str, &str)], &str) = match self.destino {
            Destino::Pdf => ("Exportar PDF", &[("PDF", "*.pdf")], "pdf"),
            Destino::Svg => ("Exportar SVG", &[("SVG", "*.svg")], "svg"),
        };
        let s = sugerido(cx, ext);
        pedir_archivo(cx, true, titulo, filtros, s);
        Flujo::Sigue
    }

    fn escribir(&self, cx: &mut Contexto, ruta: PathBuf) -> Flujo {
        let ext = match self.destino {
            Destino::Pdf => "pdf",
            Destino::Svg => "svg",
        };
        let ruta = con_extension(ruta, ext);
        let lect = cx.leer();
        let insunits = lect.dibujo.doc.header.insertion_units;
        let mm_por_unidad_lamina = 1.0 / unidades_por_mm(&lect, lect.ajustes.escala_lamina);
        let filtro = match &self.zona {
            Zona::Ventana(c) => Some(*c),
            _ => None,
        };
        let esc = escena::escena(&lect.dibujo.doc, Que::Todo, filtro);
        if esc.elementos.is_empty() {
            cx.decir("No hay nada visible que exportar");
            return Flujo::Termina;
        }
        let todo = esc.caja().unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let k = self.escala.map(|e| 1.0 / esp::unidades_por_mm(e, insunits));
        let (bytes, hojas) = match self.destino {
            Destino::Pdf => {
                let hojas: Vec<Hoja> = match &self.zona {
                    Zona::Todo => vec![pdf::exportar::hoja_para(
                        todo,
                        self.papel,
                        k.or((self.papel == Papel::Ajustar).then_some(mm_por_unidad_lamina)),
                    )],
                    Zona::Ventana(c) => vec![pdf::exportar::hoja_para(
                        *c,
                        self.papel,
                        k.or((self.papel == Papel::Ajustar).then_some(mm_por_unidad_lamina)),
                    )],
                    Zona::Laminas(v) => v
                        .iter()
                        .map(|m| {
                            pdf::exportar::hoja_de_lamina(*m, k.unwrap_or(mm_por_unidad_lamina))
                        })
                        .collect(),
                };
                let titulo = ruta
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let n = hojas.len();
                (
                    pdf::exportar::escribir(&esc, &hojas, &self.opciones, &titulo),
                    n,
                )
            }
            Destino::Svg => {
                // Con un poco de margen: lo del borde no pierde medio grosor.
                let zona = match &self.zona {
                    Zona::Ventana(c) => *c,
                    _ => geo::ensanchar(todo, (todo[2] - todo[0]).max(todo[3] - todo[1]) * 0.01),
                };
                let mm_por_unidad = 1.0 / esp::unidades_por_mm(1.0, insunits);
                let grosor = unidades_por_mm(&lect, lect.ajustes.escala_lamina);
                (
                    svg::escribir(&esc, zona, mm_por_unidad, grosor, self.opciones.monocromo)
                        .into_bytes(),
                    1,
                )
            }
        };
        let n = esc.elementos.len();
        let (imagenes, omitidas) = (esc.imagenes, esc.omitidas);
        match std::fs::write(&ruta, bytes) {
            Ok(()) => {
                cx.decir(format!(
                    "{} escrito: {} ({} hoja(s), {} elementos)",
                    ext.to_uppercase(),
                    ruta.display(),
                    hojas,
                    n
                ));
                if imagenes > 0 {
                    cx.decir(format!("  {imagenes} imagen(es) no se exportan"));
                }
                if omitidas > 0 {
                    cx.decir(format!(
                        "  {omitidas} objeto(s) que no se saben dibujar (3D, proxys)"
                    ));
                }
            }
            Err(e) => cx.decir(format!("No se pudo escribir {}: {e}", ruta.display())),
        }
        Flujo::Termina
    }

    fn resumen(&self) -> String {
        let escala = self.escala.map_or("ajustada".to_string(), |e| {
            format!("1:{}", esp::texto_de_medida(e, 2))
        });
        format!(
            "{}, {escala}, {}, {}{}",
            self.nombre_papel,
            if self.opciones.monocromo {
                "monocromo"
            } else {
                "color"
            },
            if self.opciones.trazos {
                "textos en rayas"
            } else {
                "textos como texto"
            },
            if self.opciones.sin_capas {
                ", sin capas"
            } else {
                ""
            }
        )
    }
}

impl Herramienta for Exportar {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match (self.paso, self.destino) {
            (PasoExportar::Zona, Destino::Pdf) => format!(
                "Exportar [Todo/Ventana/Laminas] o cambiar [Papel/Escala/Monocromo/Rayas/Capas] ({}) <Todo>:",
                self.resumen()
            ),
            (PasoExportar::Zona, Destino::Svg) => format!(
                "Exportar [Todo/Ventana] o [Monocromo] ({}) <Todo>:",
                if self.opciones.monocromo {
                    "monocromo"
                } else {
                    "color"
                }
            ),
            (PasoExportar::Papel, _) => {
                format!("Papel [A4/A3/A2/A1/A0/Ajustar] <{}>:", self.nombre_papel)
            }
            (PasoExportar::Escala, _) => "Escala 1: (Enter: ajustar al papel):".into(),
            (PasoExportar::Esquina(None), _) => "Primera esquina de la ventana:".into(),
            (PasoExportar::Esquina(Some(_)), _) => "Esquina opuesta:".into(),
            (PasoExportar::Archivo, _) => "Archivo donde escribir:".into(),
        }
    }

    fn pide_texto(&self) -> bool {
        self.paso == PasoExportar::Archivo
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        let o = opcion(t);
        match self.paso {
            PasoExportar::Archivo => self.escribir(cx, ruta_de(t)),
            PasoExportar::Papel => {
                if o == "AJUSTAR" || o == "AJ" || o == "FIT" {
                    self.papel = Papel::Ajustar;
                    self.nombre_papel = "Ajustar".into();
                } else if let Some((a, b)) = esp::formato(&o) {
                    self.papel = Papel::Formato(a, b);
                    self.nombre_papel = o;
                } else {
                    return Flujo::NoEntiende;
                }
                self.paso = PasoExportar::Zona;
                Flujo::Sigue
            }
            PasoExportar::Zona => match o.as_str() {
                "T" | "TODO" | "ALL" | "EXTENSION" => self.pedir(cx, Zona::Todo),
                "V" | "VENTANA" | "W" | "WINDOW" => {
                    self.paso = PasoExportar::Esquina(None);
                    Flujo::Sigue
                }
                "L" | "LAMINA" | "LAMINAS" | "SHEET" | "SHEETS" if self.destino == Destino::Pdf => {
                    let m = escena::marcos_de_lamina(&cx.dibujo.doc);
                    if m.is_empty() {
                        cx.decir("No hay laminas (marcos de LAMINA en la capa L-LAMINA)");
                        return Flujo::Sigue;
                    }
                    cx.decir(format!("{} lamina(s): una pagina cada una", m.len()));
                    self.pedir(cx, Zona::Laminas(m))
                }
                "P" | "PAPEL" | "PAPER" if self.destino == Destino::Pdf => {
                    self.paso = PasoExportar::Papel;
                    Flujo::Sigue
                }
                "E" | "ESCALA" | "SCALE" if self.destino == Destino::Pdf => {
                    self.paso = PasoExportar::Escala;
                    Flujo::Sigue
                }
                "M" | "MONOCROMO" | "COLOR" | "C" => {
                    self.opciones.monocromo = !self.opciones.monocromo;
                    Flujo::Sigue
                }
                "R" | "RAYAS" | "TEXTOS" | "TRAZOS" if self.destino == Destino::Pdf => {
                    self.opciones.trazos = !self.opciones.trazos;
                    Flujo::Sigue
                }
                "CAPAS" | "CA" | "LAYERS" if self.destino == Destino::Pdf => {
                    self.opciones.sin_capas = !self.opciones.sin_capas;
                    Flujo::Sigue
                }
                _ => Flujo::NoEntiende,
            },
            _ => Flujo::NoEntiende,
        }
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        if self.paso != PasoExportar::Escala {
            return Flujo::NoEntiende;
        }
        if v.is_finite() && v > 0.0 {
            self.escala = Some(v);
            self.paso = PasoExportar::Zona;
        } else {
            cx.decir("La escala tiene que ser mayor que cero");
        }
        Flujo::Sigue
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        match self.paso {
            PasoExportar::Zona => {
                self.paso = PasoExportar::Esquina(Some(p));
                Flujo::Sigue
            }
            PasoExportar::Esquina(None) => {
                self.paso = PasoExportar::Esquina(Some(p));
                Flujo::Sigue
            }
            PasoExportar::Esquina(Some(a)) => {
                let c = geo::caja_de(&[a, p]);
                if c[2] - c[0] < 1e-9 || c[3] - c[1] < 1e-9 {
                    cx.decir("La ventana no tiene area: elija otra esquina");
                    return Flujo::Sigue;
                }
                self.pedir(cx, Zona::Ventana(c))
            }
            _ => Flujo::NoEntiende,
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        match self.paso {
            PasoExportar::Zona => self.pedir(cx, Zona::Todo),
            PasoExportar::Escala => {
                self.escala = None;
                self.paso = PasoExportar::Zona;
                Flujo::Sigue
            }
            PasoExportar::Papel => {
                self.paso = PasoExportar::Zona;
                Flujo::Sigue
            }
            _ => Flujo::Termina,
        }
    }

    fn previa(&self, _cx: &Lectura, c: P) -> Previa {
        match self.paso {
            PasoExportar::Esquina(Some(a)) => Previa {
                formas: rectangulo(a, c),
                ..Default::default()
            },
            _ => Previa::default(),
        }
    }
}

// --------------------------------------------------------------- IMAGEN

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum PasoImagen {
    #[default]
    Archivo,
    Punto,
    Ancho,
    Giro,
}

/// IMAGEN: archivo, punto, ancho y giro.
#[derive(Default)]
struct Imagen {
    paso: PasoImagen,
    ruta: PathBuf,
    pixeles: (u32, u32),
    punto: P,
    ancho: f64,
}

impl Imagen {
    /// El ancho que se propone: a 96 ppp en el papel de la lamina.
    fn ancho_propuesto(&self, cx: &Lectura) -> f64 {
        self.pixeles.0 as f64 * 25.4 / 96.0 * unidades_por_mm(cx, cx.ajustes.escala_lamina)
    }

    fn crear(&mut self, cx: &mut Contexto, giro: f64) -> Flujo {
        match imagen::adjuntar(
            cx.dibujo,
            &self.ruta,
            self.pixeles,
            self.punto,
            self.ancho,
            giro,
        ) {
            Ok(h) => {
                *cx.seleccion = vec![h];
                cx.decir(format!(
                    "Imagen adjunta: {} ({} x {} px), enlazada (no se copia en el plano)",
                    self.ruta.display(),
                    self.pixeles.0,
                    self.pixeles.1
                ));
            }
            Err(e) => cx.decir(format!("No se pudo adjuntar la imagen: {e}")),
        }
        Flujo::Termina
    }
}

impl Herramienta for Imagen {
    fn indicacion(&self, cx: &Lectura) -> String {
        match self.paso {
            PasoImagen::Archivo => "Imagen que adjuntar (PNG, JPG, BMP, TIF, GIF):".into(),
            PasoImagen::Punto => "Punto de insercion (esquina inferior izquierda):".into(),
            PasoImagen::Ancho => format!(
                "Ancho de la imagen <{}>:",
                cx.medida(self.ancho_propuesto(cx))
            ),
            PasoImagen::Giro => "Angulo de giro <0>:".into(),
        }
    }

    fn base(&self) -> Option<P> {
        matches!(self.paso, PasoImagen::Ancho | PasoImagen::Giro).then_some(self.punto)
    }

    fn pide_texto(&self) -> bool {
        self.paso == PasoImagen::Archivo
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        pedir_archivo(
            cx,
            false,
            "Adjuntar imagen",
            &[
                ("Imagenes", "*.png;*.jpg;*.jpeg;*.bmp;*.tif;*.tiff;*.gif"),
                ("Todos", "*.*"),
            ],
            String::new(),
        );
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        if self.paso != PasoImagen::Archivo {
            return Flujo::NoEntiende;
        }
        let ruta = ruta_de(t);
        let ruta = std::fs::canonicalize(&ruta)
            .map(sin_prefijo_largo)
            .unwrap_or(ruta);
        match imagen::tamano_de(&ruta) {
            Ok(px) => {
                self.ruta = ruta;
                self.pixeles = px;
                self.paso = PasoImagen::Punto;
                Flujo::Sigue
            }
            Err(e) => {
                cx.decir(format!("No se pudo adjuntar {}: {e}", ruta.display()));
                Flujo::Termina
            }
        }
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        match self.paso {
            PasoImagen::Punto => {
                self.punto = p;
                self.paso = PasoImagen::Ancho;
                Flujo::Sigue
            }
            // Un punto para el ancho: la distancia en x, como el lado de abajo.
            PasoImagen::Ancho => {
                let d = geo::dist(self.punto, p);
                if d > 1e-9 {
                    self.ancho = d;
                    self.paso = PasoImagen::Giro;
                }
                Flujo::Sigue
            }
            PasoImagen::Giro => {
                if geo::dist(self.punto, p) < 1e-12 {
                    return Flujo::Sigue;
                }
                self.crear(cx, geo::angulo(self.punto, p))
            }
            _ => Flujo::NoEntiende,
        }
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        match self.paso {
            PasoImagen::Ancho => {
                if v.is_finite() && v > 0.0 {
                    self.ancho = v;
                    self.paso = PasoImagen::Giro;
                } else {
                    cx.decir("El ancho tiene que ser mayor que cero");
                }
                Flujo::Sigue
            }
            PasoImagen::Giro if v.is_finite() => self.crear(cx, v.to_radians()),
            _ => Flujo::NoEntiende,
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        match self.paso {
            PasoImagen::Ancho => {
                self.ancho = self.ancho_propuesto(&cx.leer());
                self.paso = PasoImagen::Giro;
                Flujo::Sigue
            }
            PasoImagen::Giro => self.crear(cx, 0.0),
            _ => Flujo::Termina,
        }
    }

    fn previa(&self, cx: &Lectura, c: P) -> Previa {
        let (w, giro) = match self.paso {
            PasoImagen::Punto => (self.ancho_propuesto(cx), 0.0),
            PasoImagen::Ancho => (geo::dist(self.punto, c).max(1e-9), 0.0),
            PasoImagen::Giro => (self.ancho, geo::angulo(self.punto, c)),
            _ => return Previa::default(),
        };
        let o = if self.paso == PasoImagen::Punto {
            c
        } else {
            self.punto
        };
        let h = w * self.pixeles.1 as f64 / self.pixeles.0.max(1) as f64;
        let e: Vec<P> = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]
            .iter()
            .map(|q| geo::sumar(o, geo::rotar(*q, [0.0, 0.0], giro)))
            .collect();
        let mut formas: Vec<Forma> = (0..4)
            .map(|i| Forma::Segmento {
                a: e[i],
                b: e[(i + 1) % 4],
            })
            .collect();
        formas.push(Forma::Segmento { a: e[0], b: e[2] });
        Previa {
            formas,
            ..Default::default()
        }
    }
}

// ------------------------------------------------------- GUARDARVERSION

/// GUARDARVERSION: la version y, si se elige archivo, guardar ya.
#[derive(Default)]
struct GuardarVersion {
    elegida: bool,
}

impl Herramienta for GuardarVersion {
    fn indicacion(&self, cx: &Lectura) -> String {
        if self.elegida {
            return "Archivo DWG o DXF donde guardar (Esc: solo la version, para el proximo Guardar):"
                .into();
        }
        let lista: Vec<&str> = version::VERSIONES.iter().map(|(n, _)| *n).collect();
        format!(
            "Version de DWG/DXF [{}] <{}>:",
            lista.join("/"),
            version::nombre(cx.dibujo.doc.version)
        )
    }

    fn pide_texto(&self) -> bool {
        self.elegida
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        if !self.elegida {
            return match version::de_texto(t) {
                Some(v) => self.elegir(cx, v),
                None => Flujo::NoEntiende,
            };
        }
        let mut ruta = ruta_de(t);
        if ruta.extension().is_none() {
            ruta = con_extension(ruta, "dwg");
        }
        let v = cx.dibujo.doc.version;
        match version::guardar_en(cx.dibujo, &ruta, v) {
            Ok(aviso) => {
                cx.decir(format!(
                    "Guardado en {} (AutoCAD {})",
                    ruta.display(),
                    version::nombre(v)
                ));
                if let Some(a) = aviso {
                    cx.decir(a);
                }
            }
            Err(e) => cx.decir(format!("No se pudo guardar {}: {e}", ruta.display())),
        }
        Flujo::Termina
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        if self.elegida {
            return Flujo::NoEntiende;
        }
        match version::de_texto(&format!("{}", v.round() as i64)) {
            Some(x) if v.fract() == 0.0 => self.elegir(cx, x),
            _ => Flujo::NoEntiende,
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        if self.elegida {
            return Flujo::Termina;
        }
        let v = cx.dibujo.doc.version;
        let v = if version::VERSIONES.iter().any(|(_, x)| *x == v) {
            v
        } else {
            opencadcodec::DxfVersion::AC1032
        };
        self.elegir(cx, v)
    }

    fn cancelar(&mut self, cx: &mut Contexto) {
        if self.elegida {
            cx.decir(format!(
                "Se guardara en AutoCAD {} con Guardar",
                version::nombre(cx.dibujo.doc.version)
            ));
        }
    }
}

impl GuardarVersion {
    fn elegir(&mut self, cx: &mut Contexto, v: opencadcodec::DxfVersion) -> Flujo {
        if let Some(a) = version::poner(cx.dibujo, v) {
            cx.decir(a);
        }
        cx.decir(format!(
            "Version para guardar: AutoCAD {}",
            version::nombre(v)
        ));
        self.elegida = true;
        let s = cx
            .dibujo
            .ruta
            .as_deref()
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "plano.dwg".into());
        pedir_archivo(
            cx,
            true,
            &format!("Guardar como AutoCAD {}", version::nombre(v)),
            &[("Dibujo DWG", "*.dwg"), ("Intercambio DXF", "*.dxf")],
            s,
        );
        Flujo::Sigue
    }
}

// ---------------------------------------------------------- EXPORTARDXF

/// EXPORTARDXF / WBLOQUE: lo elegido, su punto base y el archivo.
#[derive(Default)]
struct ExportarElegidos {
    base: Option<P>,
    objetos: Vec<Handle>,
}

impl ExportarElegidos {
    fn con_base(&mut self, cx: &mut Contexto, b: P) -> Flujo {
        self.base = Some(b);
        let s = sugerido(cx, "dwg").replace(".dwg", "_parte.dwg");
        pedir_archivo(
            cx,
            true,
            "Escribir lo elegido",
            &[("Dibujo DWG", "*.dwg"), ("Intercambio DXF", "*.dxf")],
            s,
        );
        Flujo::Sigue
    }
}

impl Herramienta for ExportarElegidos {
    fn indicacion(&self, _cx: &Lectura) -> String {
        if self.base.is_none() {
            "Punto base <0,0>:".into()
        } else {
            "Archivo DWG o DXF nuevo:".into()
        }
    }

    fn pide_texto(&self) -> bool {
        self.base.is_some()
    }

    fn al_tener_objetos(&mut self, cx: &mut Contexto) -> Flujo {
        self.objetos = cx.seleccion.clone();
        Flujo::Sigue
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        if self.base.is_some() {
            return Flujo::NoEntiende;
        }
        self.con_base(cx, p)
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        if self.base.is_none() {
            return self.con_base(cx, [0.0, 0.0]);
        }
        Flujo::Termina
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        if self.base.is_none() {
            return Flujo::NoEntiende;
        }
        let mut ruta = ruta_de(t);
        if ruta.extension().is_none() {
            ruta = con_extension(ruta, "dwg");
        }
        let objetos = if self.objetos.is_empty() {
            cx.seleccion.clone()
        } else {
            self.objetos.clone()
        };
        match wbloque::escribir(cx.dibujo, &objetos, self.base, &ruta) {
            Ok(n) => cx.decir(format!(
                "{n} objetos escritos en {} (AutoCAD {})",
                ruta.display(),
                version::nombre(cx.dibujo.doc.version)
            )),
            Err(e) => cx.decir(format!("No se pudo escribir {}: {e}", ruta.display())),
        }
        Flujo::Termina
    }
}

// ------------------------------------------------------------ ESCALAREF

/// ESCALAREF: base, dos puntos de referencia y la distancia de verdad.
#[derive(Default)]
struct EscalaRef {
    base: Option<P>,
    r1: Option<P>,
    medida: Option<f64>,
}

impl EscalaRef {
    fn escalar(&mut self, cx: &mut Contexto, real: f64) -> Flujo {
        let (Some(b), Some(m)) = (self.base, self.medida) else {
            return Flujo::Sigue;
        };
        if !(real.is_finite() && real > 0.0) {
            cx.decir("La distancia tiene que ser mayor que cero");
            return Flujo::Sigue;
        }
        let k = real / m;
        let hs = cx.seleccion.clone();
        let n = cx.dibujo.editar("ESCALAREF", |t| {
            hs.iter()
                .filter(|h| t.cambiar(**h, |e| op::escalar(e, b, k)))
                .count()
        });
        cx.decir(format!(
            "{n} objetos escalados {} veces (de {} a {})",
            esp::texto_de_medida(k, 6),
            cx.medida(m),
            cx.medida(real)
        ));
        Flujo::Termina
    }
}

impl Herramienta for EscalaRef {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match (self.base, self.r1, self.medida) {
            (None, _, _) => "Punto base (el que no se mueve):".into(),
            (Some(_), None, None) => {
                "Primer punto de la medida conocida (o escriba la longitud de referencia):".into()
            }
            (Some(_), Some(_), None) => "Segundo punto de la medida conocida:".into(),
            _ => "Longitud de verdad entre esos puntos:".into(),
        }
    }

    fn base(&self) -> Option<P> {
        self.r1.or(self.base)
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        match (self.base, self.r1, self.medida) {
            (None, _, _) => self.base = Some(p),
            (Some(_), None, None) => self.r1 = Some(p),
            (Some(_), Some(a), None) => {
                let d = geo::dist(a, p);
                if d < 1e-12 {
                    cx.decir("Los dos puntos son el mismo: elija otro");
                } else {
                    self.medida = Some(d);
                    cx.decir(format!("Medida en el dibujo: {}", cx.medida(d)));
                }
            }
            _ => return Flujo::NoEntiende,
        }
        Flujo::Sigue
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        match (self.base, self.r1, self.medida) {
            (Some(_), None, None) if v > 0.0 => {
                self.medida = Some(v);
                Flujo::Sigue
            }
            (Some(_), _, Some(_)) => self.escalar(cx, v),
            _ => Flujo::NoEntiende,
        }
    }

    fn previa(&self, _cx: &Lectura, c: P) -> Previa {
        match (self.base, self.r1, self.medida) {
            (Some(_), Some(a), None) => Previa {
                guias: vec![(a, c)],
                etiquetas: vec![(c, format!("{:.3}", geo::dist(a, c)))],
                ..Default::default()
            },
            _ => Previa::default(),
        }
    }
}

#[cfg(test)]
mod pruebas;

/// Quita el `\\?\` que pone `canonicalize` en Windows: AutoCAD y los demas
/// programas no entienden esa forma de la ruta.
fn sin_prefijo_largo(r: std::path::PathBuf) -> std::path::PathBuf {
    let t = r.to_string_lossy();
    match t.strip_prefix(r"\\?\UNC\") {
        Some(resto) => std::path::PathBuf::from(format!(r"\\{resto}")),
        None => match t.strip_prefix(r"\\?\") {
            Some(resto) => std::path::PathBuf::from(resto),
            None => r,
        },
    }
}

#[cfg(test)]
mod pruebas_ruta {
    use super::sin_prefijo_largo;
    use std::path::PathBuf;

    #[test]
    fn rutas_sin_el_prefijo_largo_de_windows() {
        assert_eq!(
            sin_prefijo_largo(PathBuf::from(r"\\?\C:\a\b.png")),
            PathBuf::from(r"C:\a\b.png")
        );
        assert_eq!(
            sin_prefijo_largo(PathBuf::from(r"\\?\UNC\srv\x\b.png")),
            PathBuf::from(r"\\srv\x\b.png")
        );
        // Caso negativo: una ruta normal no cambia.
        assert_eq!(
            sin_prefijo_largo(PathBuf::from("/tmp/b.png")),
            PathBuf::from("/tmp/b.png")
        );
    }
}
