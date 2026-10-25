//! **Las exportaciones**: tablas de planificacion (como las de Revit) en
//! CSV y en Excel (una hoja por categoria), el modelo entero en JSON, y de
//! un Revit su IFC4 y su GLB (con `rvt-rs`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::modelo::*;

/// **Una celda** de una tabla.
#[derive(Debug, Clone, PartialEq)]
pub enum Celda {
    Vacia,
    Texto(String),
    Numero(f64),
}

impl Celda {
    pub fn texto(&self) -> String {
        match self {
            Celda::Vacia => String::new(),
            Celda::Texto(t) => t.clone(),
            Celda::Numero(v) => numero_corto(*v),
        }
    }

    /// Para ordenar: los numeros como numeros, los textos sin mayusculas.
    pub fn comparar(&self, otra: &Celda) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self, otra) {
            (Celda::Numero(a), Celda::Numero(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (Celda::Vacia, Celda::Vacia) => Ordering::Equal,
            (Celda::Vacia, _) => Ordering::Greater,
            (_, Celda::Vacia) => Ordering::Less,
            (Celda::Numero(_), Celda::Texto(_)) => Ordering::Less,
            (Celda::Texto(_), Celda::Numero(_)) => Ordering::Greater,
            (Celda::Texto(a), Celda::Texto(b)) => orden_natural(a, b),
        }
    }
}

/// Compara textos con los numeros por su valor (`P2` antes que `P10`).
pub fn orden_natural(a: &str, b: &str) -> std::cmp::Ordering {
    let clave = |s: &str| -> String {
        let mut out = String::with_capacity(s.len() + 8);
        let mut dig = String::new();
        for c in s.to_lowercase().chars() {
            if c.is_ascii_digit() {
                dig.push(c);
                continue;
            }
            if !dig.is_empty() {
                out.push_str(&format!("{dig:0>20}"));
                dig.clear();
            }
            out.push(c);
        }
        if !dig.is_empty() {
            out.push_str(&format!("{dig:0>20}"));
        }
        out
    };
    clave(a).cmp(&clave(b))
}

/// **Una tabla de planificacion**: columnas y filas (una por elemento).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tabla {
    pub columnas: Vec<String>,
    pub filas: Vec<Vec<Celda>>,
    /// El indice del elemento de cada fila.
    pub elementos: Vec<usize>,
}

impl Tabla {
    /// Ordena por una columna.
    pub fn ordenar(&mut self, columna: usize, ascendente: bool) {
        let mut orden: Vec<usize> = (0..self.filas.len()).collect();
        let vacia = Celda::Vacia;
        orden.sort_by(|&a, &b| {
            let x = self.filas[a].get(columna).unwrap_or(&vacia);
            let y = self.filas[b].get(columna).unwrap_or(&vacia);
            let o = x.comparar(y);
            if ascendente { o } else { o.reverse() }
        });
        self.filas = orden.iter().map(|&i| self.filas[i].clone()).collect();
        self.elementos = orden.iter().map(|&i| self.elementos[i]).collect();
    }
}

/// Las columnas fijas de toda tabla.
pub const COLUMNAS_FIJAS: &[&str] = &[
    "Id",
    "GlobalId",
    "Categoria",
    "Clase",
    "Familia",
    "Tipo",
    "Nombre",
    "Nivel",
    "Materiales",
    "Ancho caja (m)",
    "Fondo caja (m)",
    "Alto caja (m)",
];

/// **La tabla de unos elementos**: las columnas fijas y, si se pide, una
/// por cada propiedad que tenga alguno (`Conjunto.Propiedad (unidad)`).
pub fn tabla(m: &ModeloBim, indices: &[usize], con_propiedades: bool) -> Tabla {
    let mut columnas: Vec<String> = COLUMNAS_FIJAS.iter().map(|s| s.to_string()).collect();
    let mut col_de: HashMap<(String, String), usize> = HashMap::new();
    if con_propiedades {
        for &i in indices {
            for c in &m.elementos[i].conjuntos {
                for p in &c.propiedades {
                    let k = (c.nombre.clone(), p.nombre.clone());
                    if let std::collections::hash_map::Entry::Vacant(v) = col_de.entry(k) {
                        let mut n = format!("{}.{}", c.nombre, p.nombre);
                        if let Some(u) = &p.unidad {
                            n.push_str(&format!(" ({u})"));
                        }
                        v.insert(columnas.len());
                        columnas.push(n);
                    }
                }
            }
        }
    }
    let n = columnas.len();
    let mut filas = Vec::with_capacity(indices.len());
    for &i in indices {
        let e = &m.elementos[i];
        let t = |s: &str| {
            if s.is_empty() {
                Celda::Vacia
            } else {
                Celda::Texto(s.to_string())
            }
        };
        let med = e.caja.map(|c| c.medidas());
        let num = |k: usize| med.map_or(Celda::Vacia, |m| Celda::Numero(m[k]));
        let mut f = vec![
            t(&e.id),
            t(e.global_id.as_deref().unwrap_or("")),
            t(&e.categoria),
            t(&e.clase),
            t(&e.familia),
            t(&e.tipo),
            t(&e.nombre),
            t(m.nivel_de(e).unwrap_or("")),
            t(&e.materiales.join("; ")),
            num(0),
            num(1),
            num(2),
        ];
        f.resize(n, Celda::Vacia);
        if con_propiedades {
            for c in &e.conjuntos {
                for p in &c.propiedades {
                    if let Some(&k) = col_de.get(&(c.nombre.clone(), p.nombre.clone())) {
                        f[k] = match p.valor.numero() {
                            Some(v) => Celda::Numero(v),
                            None => t(&p.valor.texto()),
                        };
                    }
                }
            }
        }
        filas.push(f);
    }
    Tabla {
        columnas,
        filas,
        elementos: indices.to_vec(),
    }
}

/// **Una tabla en CSV** (RFC 4180, fin de linea CRLF, con BOM para Excel).
/// Un texto que una hoja de calculo tomaria por formula lleva `'` delante.
pub fn csv(t: &Tabla) -> String {
    let mut s = String::from("\u{FEFF}");
    let linea = |s: &mut String, v: &mut dyn Iterator<Item = String>| {
        let v: Vec<String> = v.collect();
        s.push_str(&v.join(","));
        s.push_str("\r\n");
    };
    linea(&mut s, &mut t.columnas.iter().map(|c| escapar(c)));
    for f in &t.filas {
        linea(
            &mut s,
            &mut f.iter().map(|c| match c {
                Celda::Vacia => String::new(),
                Celda::Numero(v) => numero_corto(*v),
                Celda::Texto(x) => {
                    if x.starts_with(['=', '+', '-', '@', '\t', '\r']) {
                        escapar(&format!("'{x}"))
                    } else {
                        escapar(x)
                    }
                }
            }),
        );
    }
    s
}

fn escapar(v: &str) -> String {
    if v.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

/// Un nombre de fichero sin lo que Windows no acepta.
fn nombre_seguro(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let t = t.trim().trim_end_matches('.').to_string();
    if t.is_empty() { "sin nombre".into() } else { t }
}

/// **El CSV de todo** (una fila por elemento, todas las propiedades como columnas).
pub fn csv_todo(m: &ModeloBim) -> String {
    let todos: Vec<usize> = (0..m.elementos.len()).collect();
    csv(&tabla(m, &todos, true))
}

/// **Un CSV por categoria** junto a `base` (`base - Muros.csv`…). Devuelve
/// los ficheros escritos.
pub fn csv_por_categoria(m: &ModeloBim, base: &Path) -> Result<Vec<PathBuf>, String> {
    let carpeta = base.parent().unwrap_or(Path::new("."));
    let raiz = base
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "modelo".into());
    let mut v = Vec::new();
    for c in &m.categorias {
        let t = tabla(m, &m.de_categoria(&c.nombre), true);
        let r = carpeta.join(format!("{raiz} - {}.csv", nombre_seguro(&c.nombre)));
        std::fs::write(&r, csv(&t)).map_err(|e| format!("{}: {e}", r.display()))?;
        v.push(r);
    }
    Ok(v)
}

/// **El modelo entero en JSON** (sin la miniatura).
pub fn json(m: &ModeloBim) -> String {
    serde_json::to_string_pretty(m).unwrap_or_else(|_| "{}".into())
}

/// Un nombre de hoja de Excel: hasta 31 caracteres, sin `[]:*?/\`, unico.
fn nombre_de_hoja(s: &str, usados: &mut Vec<String>) -> String {
    let limpio: String = s
        .chars()
        .map(|c| if "[]:*?/\\".contains(c) { '_' } else { c })
        .collect();
    let limpio = limpio.trim_matches('\'').trim().to_string();
    let base: String = if limpio.is_empty() {
        "Hoja".into()
    } else {
        limpio
    };
    let mut k = 1;
    loop {
        let sufijo = if k == 1 {
            String::new()
        } else {
            format!(" ({k})")
        };
        let corto: String = base.chars().take(31 - sufijo.chars().count()).collect();
        let n = format!("{corto}{sufijo}");
        if !usados.iter().any(|u| u.eq_ignore_ascii_case(&n)) {
            usados.push(n.clone());
            return n;
        }
        k += 1;
    }
}

/// **Excel**: una hoja «Proyecto» (el fichero y el proyecto), «Niveles»,
/// una por categoria (sus elementos con todas sus propiedades),
/// «Materiales» y «Tipos».
pub fn xlsx(m: &ModeloBim) -> Result<Vec<u8>, String> {
    use rust_xlsxwriter::{Format, Workbook};
    let err = |e: rust_xlsxwriter::XlsxError| e.to_string();
    let mut libro = Workbook::new();
    let negrita = Format::new().set_bold();
    let mut usados = Vec::new();

    // Proyecto.
    {
        let h = libro.add_worksheet();
        h.set_name(nombre_de_hoja("Proyecto", &mut usados))
            .map_err(err)?;
        let a = &m.archivo;
        let p = &m.proyecto;
        let filas: Vec<(&str, Option<String>)> = vec![
            ("Fichero", Some(a.nombre.clone())),
            ("Formato", Some(a.formato.nombre().to_string())),
            ("Version", a.version.clone()),
            ("Compilacion", a.compilacion.clone()),
            ("Programa", a.aplicacion.clone()),
            ("Autor", a.autor.clone()),
            ("Guardado por", a.guardado_por.clone()),
            ("Fecha", a.fecha.clone()),
            ("Trabajo compartido", a.trabajo_compartido.clone()),
            ("Modelo central", a.ruta_central.clone()),
            ("GUID", a.guid.clone()),
            ("Proyecto", p.nombre.clone()),
            ("Numero", p.numero.clone()),
            ("Cliente", p.cliente.clone()),
            ("Direccion", p.direccion.clone()),
            ("Estado", p.estado.clone()),
            ("Edificio", p.edificio.clone()),
            ("Elementos", Some(m.elementos.len().to_string())),
        ];
        let mut r = 0u32;
        for (k, v) in filas {
            if let Some(v) = v.filter(|v| !v.is_empty()) {
                h.write_string_with_format(r, 0, k, &negrita).map_err(err)?;
                h.write_string(r, 1, recortar(&v)).map_err(err)?;
                r += 1;
            }
        }
        for d in &a.datos {
            h.write_string(r, 0, recortar(&d.clave)).map_err(err)?;
            h.write_string(r, 1, recortar(&d.valor)).map_err(err)?;
            r += 1;
        }
        for aviso in &m.avisos {
            h.write_string_with_format(r, 0, "Aviso", &negrita)
                .map_err(err)?;
            h.write_string(r, 1, recortar(aviso)).map_err(err)?;
            r += 1;
        }
        h.set_column_width(0, 24).map_err(err)?;
        h.set_column_width(1, 70).map_err(err)?;
    }
    // Niveles.
    {
        let h = libro.add_worksheet();
        h.set_name(nombre_de_hoja("Niveles", &mut usados))
            .map_err(err)?;
        for (c, t) in ["Nivel", "Cota (m)", "Edificio", "Elementos"]
            .iter()
            .enumerate()
        {
            h.write_string_with_format(0, c as u16, *t, &negrita)
                .map_err(err)?;
        }
        for (i, n) in m.niveles.iter().enumerate() {
            let r = i as u32 + 1;
            h.write_string(r, 0, recortar(&n.nombre)).map_err(err)?;
            if let Some(c) = n.cota {
                h.write_number(r, 1, c).map_err(err)?;
            }
            h.write_string(r, 2, recortar(n.edificio.as_deref().unwrap_or("")))
                .map_err(err)?;
            h.write_number(r, 3, n.elementos as f64).map_err(err)?;
        }
    }
    // Una por categoria.
    for c in &m.categorias {
        let t = tabla(m, &m.de_categoria(&c.nombre), true);
        let h = libro.add_worksheet();
        h.set_name(nombre_de_hoja(&c.nombre, &mut usados))
            .map_err(err)?;
        escribir_tabla(h, &t, &negrita).map_err(err)?;
    }
    // Materiales y tipos.
    {
        let h = libro.add_worksheet();
        h.set_name(nombre_de_hoja("Materiales", &mut usados))
            .map_err(err)?;
        for (c, t) in ["Material", "Color", "Transparencia", "Elementos"]
            .iter()
            .enumerate()
        {
            h.write_string_with_format(0, c as u16, *t, &negrita)
                .map_err(err)?;
        }
        for (i, x) in m.materiales.iter().enumerate() {
            let r = i as u32 + 1;
            h.write_string(r, 0, recortar(&x.nombre)).map_err(err)?;
            if let Some([a, b, c]) = x.color {
                h.write_string(r, 1, format!("#{a:02X}{b:02X}{c:02X}"))
                    .map_err(err)?;
            }
            if let Some(t) = x.transparencia {
                h.write_number(r, 2, t).map_err(err)?;
            }
            h.write_number(r, 3, x.elementos as f64).map_err(err)?;
        }
        let h = libro.add_worksheet();
        h.set_name(nombre_de_hoja("Tipos", &mut usados))
            .map_err(err)?;
        for (c, t) in ["Id", "Clase", "Familia", "Tipo", "Elementos"]
            .iter()
            .enumerate()
        {
            h.write_string_with_format(0, c as u16, *t, &negrita)
                .map_err(err)?;
        }
        for (i, x) in m.tipos.iter().enumerate() {
            let r = i as u32 + 1;
            h.write_string(r, 0, recortar(&x.id)).map_err(err)?;
            h.write_string(r, 1, recortar(&x.clase)).map_err(err)?;
            h.write_string(r, 2, recortar(&x.familia)).map_err(err)?;
            h.write_string(r, 3, recortar(&x.nombre)).map_err(err)?;
            h.write_number(r, 4, x.elementos as f64).map_err(err)?;
        }
    }
    libro.save_to_buffer().map_err(err)
}

/// Excel no acepta celdas de mas de 32 767 caracteres.
fn recortar(s: &str) -> String {
    s.chars().take(32_000).collect()
}

fn escribir_tabla(
    h: &mut rust_xlsxwriter::Worksheet,
    t: &Tabla,
    negrita: &rust_xlsxwriter::Format,
) -> Result<(), rust_xlsxwriter::XlsxError> {
    // Excel tiene 16 384 columnas y 1 048 576 filas.
    let ncol = t.columnas.len().min(16_384);
    for (c, n) in t.columnas.iter().take(ncol).enumerate() {
        h.write_string_with_format(0, c as u16, recortar(n), negrita)?;
    }
    for (i, f) in t.filas.iter().take(1_048_575).enumerate() {
        let r = i as u32 + 1;
        for (c, celda) in f.iter().take(ncol).enumerate() {
            match celda {
                Celda::Vacia => {}
                Celda::Texto(x) => {
                    h.write_string(r, c as u16, recortar(x))?;
                }
                Celda::Numero(v) => {
                    h.write_number(r, c as u16, *v)?;
                }
            }
        }
    }
    if ncol > 0 {
        h.set_freeze_panes(1, 0)?;
        let ultima = t.filas.len().min(1_048_575) as u32;
        h.autofilter(0, 0, ultima, (ncol - 1) as u16)?;
        for c in 0..ncol.min(12) {
            h.set_column_width(c as u16, if c == 1 { 24 } else { 16 })?;
        }
    }
    Ok(())
}

/// **Exporta segun la extension** de `destino`: `.csv` (todo en uno),
/// `.json` o `.xlsx`.
pub fn exportar(m: &ModeloBim, destino: &Path) -> Result<(), String> {
    let ext = destino
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let datos = match ext.as_str() {
        "csv" => csv_todo(m).into_bytes(),
        "json" => json(m).into_bytes(),
        "xlsx" => xlsx(m)?,
        _ => return Err(format!("no se exporta a .{ext}")),
    };
    std::fs::write(destino, datos).map_err(|e| format!("{}: {e}", destino.display()))
}

/// El modelo IFC de un Revit como lo exporta `rvt-rs` (con los muebles y
/// demas familias con las mallas que guardo Revit).
fn modelo_de_revit(ruta: &Path) -> Result<rvt::ifc::IfcModel, String> {
    use rvt::ifc::{ExportQualityMode, RvtDocExporter};
    let mut rf =
        rvt::RevitFile::open(ruta).map_err(|e| format!("no es un Revit que se pueda leer: {e}"))?;
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        RvtDocExporter.export_with_diagnostics_mode_and_limits(
            &mut rf,
            ExportQualityMode::Scaffold,
            rvt::walker::WalkerLimits::default(),
        )
    }))
    .map_err(|_| "rvt-rs se detuvo leyendo el Revit".to_string())?
    .map_err(|e| e.to_string())?;
    let mut modelo = r.model;
    let _ = rvt::ifc::saved_meshes::attach(&mut rf, &mut modelo);
    Ok(modelo)
}

/// **Revit a IFC4** (texto STEP).
pub fn revit_a_ifc(ruta: &Path) -> Result<String, String> {
    Ok(rvt::ifc::write_step(&modelo_de_revit(ruta)?))
}

/// **Revit a GLB** (glTF binario).
pub fn revit_a_glb(ruta: &Path) -> Result<Vec<u8>, String> {
    Ok(rvt::ifc::gltf::model_to_glb(&modelo_de_revit(ruta)?))
}
