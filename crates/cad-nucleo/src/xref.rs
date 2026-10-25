//! **Las referencias externas** (REFX, el XATTACH de AutoCAD) y **los planos
//! insertados como bloque** (INSERTARDWG).
//!
//! Una referencia es un registro de bloque marcado `is_xref` (y
//! `is_xref_overlay` si es superpuesta) con la ruta del fichero en
//! `xref_path`, relativa al dibujo cuando se puede (`.\planos\base.dwg`,
//! `..\base.dwg`), como la guarda AutoCAD.
//!
//! En memoria su definicion se llena con el espacio modelo del fichero, asi
//! el convertidor la dibuja como cualquier INSERT. Lo que trae entra con el
//! nombre de la referencia delante, como en AutoCAD: las capas como
//! `NOMBRE|CAPA`, los tipos de linea como `NOMBRE|TIPO`, sus bloques como
//! `NOMBRE|BLOQUE` y sus referencias anidadas (las adjuntas, no las
//! superpuestas, como en AutoCAD) como bloques `NOMBRE|SUB` con las capas
//! `NOMBRE|SUB|CAPA`. Los estilos de texto que el dibujo no tiene se copian
//! con su nombre (el convertidor los busca por nombre).
//!
//! Al guardar ([`para_guardar`], lo usa [`crate::Dibujo::guardar_como`]) se
//! guarda una copia sin nada de eso: en el DWG o el DXF queda el registro con
//! su ruta y los INSERT, como lo deja AutoCAD (opencadcodec escribe el
//! registro de referencia de verdad en los dos formatos). Al abrir
//! ([`cargar_todas`]) se vuelve a leer cada fichero. Lo que se pierde: el
//! estado de las capas `NOMBRE|CAPA` cambiado aqui (el VISRETAIN de AutoCAD)
//! vuelve al del fichero al abrir de nuevo.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use opencadcodec::tables::{BlockRecord, Layer};
use opencadcodec::{CadDocument, EntityType, Handle, Vector3};

use crate::dibujo::Dibujo;

/// Lo que separa el nombre de la referencia del de lo que trae.
pub const SEPARADOR: char = '|';

/// Hasta donde se entra en referencias dentro de referencias.
const PROFUNDIDAD: usize = 4;

/// Las capas que no se renombran (las comparte todo dibujo).
const CAPAS_COMUNES: &[&str] = &["0", "DEFPOINTS"];

/// Los tipos de linea que no se renombran.
const TIPOS_COMUNES: &[&str] = &["", "BYLAYER", "BYBLOCK", "CONTINUOUS"];

/// **Como esta una referencia.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Estado {
    /// Su fichero esta y su contenido esta cargado.
    Cargada,
    /// Su fichero esta pero no hay nada cargado (vacio o no se pudo leer).
    SinCargar,
    /// No se encuentra el fichero.
    NoEncontrada,
}

impl Estado {
    pub fn nombre(&self) -> &'static str {
        match self {
            Estado::Cargada => "cargada",
            Estado::SinCargar => "sin cargar",
            Estado::NoEncontrada => "no encontrada",
        }
    }
}

/// **Una referencia del dibujo**, para LISTAREF.
#[derive(Debug, Clone, PartialEq)]
pub struct Referencia {
    pub nombre: String,
    /// La ruta tal cual se guarda.
    pub ruta: String,
    /// Donde se encontro el fichero.
    pub resuelta: Option<PathBuf>,
    pub superpuesta: bool,
    pub estado: Estado,
    /// Cuantos objetos trae cargados.
    pub objetos: usize,
    /// Cuantas veces esta insertada en el espacio modelo.
    pub inserciones: usize,
}

/// **Lo que dejo una carga**: cuantos objetos trajo y lo que hay que decir
/// (referencias anidadas que faltan, circulares…).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Carga {
    pub objetos: usize,
    pub avisos: Vec<String>,
}

/// Si un registro de bloque es una referencia externa.
pub fn es_referencia(br: &BlockRecord) -> bool {
    br.flags.is_xref || br.flags.is_xref_overlay
}

/// **Los nombres de las referencias** del dibujo, en orden alfabetico.
pub fn nombres(doc: &CadDocument) -> Vec<String> {
    let mut v: Vec<String> = doc
        .block_records
        .iter()
        .filter(|b| es_referencia(b))
        .map(|b| b.name.clone())
        .collect();
    v.sort_by_key(|n| n.to_uppercase());
    v
}

/// El nombre de verdad de la referencia `nombre` (sin mirar mayusculas).
pub fn buscar(doc: &CadDocument, nombre: &str) -> Option<String> {
    doc.block_records
        .get(nombre.trim())
        .filter(|b| es_referencia(b))
        .map(|b| b.name.clone())
}

fn prefijo(nombre: &str) -> String {
    format!("{nombre}{SEPARADOR}")
}

/// Si `simbolo` es de los que trajo la referencia de prefijo `p` (`NOMBRE|`).
fn es_de(simbolo: &str, p: &str) -> bool {
    simbolo.len() > p.len()
        && simbolo.is_char_boundary(p.len())
        && simbolo[..p.len()].eq_ignore_ascii_case(p)
}

// ---------------------------------------------------------------- rutas

/// Las partes de una ruta escrita con `\` o con `/`.
fn partes(s: &str) -> Vec<&str> {
    s.split(['\\', '/']).filter(|p| !p.is_empty()).collect()
}

/// Si una ruta guardada es absoluta (`C:\…`, `\\servidor\…` o `/…`).
fn es_absoluta(s: &str) -> bool {
    let b = s.as_bytes();
    s.starts_with(['\\', '/']) || (b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic())
}

/// Quita los `.` y resuelve los `..` sin mirar el disco.
fn normalizar(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            c => out.push(c.as_os_str()),
        }
    }
    out
}

/// La ruta absoluta y normalizada (sin tocar el disco).
fn absoluta(p: &Path) -> PathBuf {
    normalizar(&std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()))
}

/// Si dos rutas son el mismo fichero (sin mayusculas en Windows).
fn misma(a: &Path, b: &Path) -> bool {
    let (a, b) = (absoluta(a), absoluta(b));
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// La ruta guardada `s`, llevada a las de este sistema y pegada a `dir`.
fn pegar_ruta(dir: &Path, s: &str) -> PathBuf {
    let mut p = dir.to_path_buf();
    for parte in partes(s) {
        p.push(parte);
    }
    normalizar(&p)
}

/// **Donde esta el fichero** de una referencia guardada como `guardada`,
/// desde el dibujo `anfitrion`: tal cual si es absoluta, junto al dibujo si
/// es relativa y, si no, con su nombre en la carpeta del dibujo (lo que
/// busca AutoCAD). `None` si no esta.
pub fn resolver(guardada: &str, anfitrion: Option<&Path>) -> Option<PathBuf> {
    let guardada = guardada.trim();
    if guardada.is_empty() {
        return None;
    }
    let dir = anfitrion.and_then(Path::parent);
    let mut candidatas = Vec::new();
    if es_absoluta(guardada) {
        let p = if cfg!(windows) {
            PathBuf::from(guardada)
        } else {
            PathBuf::from(guardada.replace('\\', "/"))
        };
        candidatas.push(p);
    } else if let Some(d) = dir {
        candidatas.push(pegar_ruta(d, guardada));
    }
    if let (Some(d), Some(nombre)) = (dir, partes(guardada).last()) {
        candidatas.push(d.join(nombre));
    }
    candidatas.into_iter().find(|p| p.is_file())
}

/// **La ruta que se guarda** para `archivo` desde el dibujo `anfitrion`:
/// relativa a su carpeta si los dos estan en la misma unidad (con `\`, como
/// AutoCAD), y si no (o el dibujo aun no tiene nombre) absoluta.
pub fn ruta_para_guardar(archivo: &Path, anfitrion: Option<&Path>) -> String {
    let destino = absoluta(archivo);
    if let Some(dir) = anfitrion.and_then(Path::parent)
        && let Some(r) = relativa(&destino, &absoluta(dir))
    {
        return r;
    }
    destino.display().to_string()
}

fn relativa(destino: &Path, dir: &Path) -> Option<String> {
    let a: Vec<Component> = destino.components().collect();
    let b: Vec<Component> = dir.components().collect();
    // La misma raiz (la unidad en Windows).
    let iguales = |x: &Component, y: &Component| {
        if cfg!(windows) {
            x.as_os_str().to_string_lossy().to_lowercase()
                == y.as_os_str().to_string_lossy().to_lowercase()
        } else {
            x == y
        }
    };
    if a.is_empty() || b.is_empty() || !iguales(&a[0], &b[0]) {
        return None;
    }
    let comun = a.iter().zip(&b).take_while(|(x, y)| iguales(x, y)).count();
    let subir = b.len() - comun;
    let mut v: Vec<String> = Vec::new();
    if subir == 0 {
        v.push(".".into());
    }
    v.extend(std::iter::repeat_n("..".to_string(), subir));
    v.extend(
        a[comun..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    Some(v.join("\\"))
}

/// **Al guardar en otra carpeta**: las rutas relativas se rehacen para que
/// sigan llevando al mismo fichero.
pub fn rebasar(doc: &mut CadDocument, viejo: Option<&Path>, nuevo: &Path) {
    let Some(dir_viejo) = viejo.and_then(Path::parent) else {
        return;
    };
    if nuevo.parent().is_some_and(|d| misma(d, dir_viejo)) {
        return;
    }
    for br in doc.block_records.iter_mut().filter(|b| es_referencia(b)) {
        if br.xref_path.trim().is_empty() || es_absoluta(br.xref_path.trim()) {
            continue;
        }
        let archivo = pegar_ruta(&absoluta(dir_viejo), &br.xref_path);
        br.xref_path = ruta_para_guardar(&archivo, Some(nuevo));
    }
}

// ---------------------------------------------------------------- leer

/// **Lee un plano** en un hilo con pila grande (como al abrir: el lector
/// entra hondo en algunos ficheros) y sin dejar caer la app si se rompe.
pub fn leer_archivo(ruta: &Path) -> Result<CadDocument, String> {
    let r = ruta.to_path_buf();
    std::thread::Builder::new()
        .name("referencia-leer".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || crate::dibujo::leer(&r))
        .map_err(|e| e.to_string())?
        .join()
        .unwrap_or_else(|_| Err("el plano esta dañado".into()))
}

/// **Lee un plano y sus referencias**: lo que la app hace al abrir. Da lo
/// que haya que decir de las que no se pudieron cargar.
pub fn leer_con_referencias(ruta: &Path) -> Result<(CadDocument, Vec<String>), String> {
    let mut doc = crate::dibujo::leer(ruta)?;
    let avisos = cargar_todas(&mut doc, Some(ruta));
    Ok((doc, avisos))
}

// ---------------------------------------------------------------- importar

/// Copia lo de un plano `origen` dentro de `destino`, con un prefijo en los
/// nombres (las referencias) o sin el (INSERTARDWG, donde gana lo que el
/// dibujo ya tiene).
struct Importar<'a> {
    destino: &'a mut CadDocument,
    /// `Some("NOMBRE|")` para una referencia.
    prefijo: Option<String>,
    /// El registro de la referencia (para sus capas).
    registro: Handle,
    capas: HashMap<String, String>,
    tipos: HashMap<String, String>,
    bloques: HashMap<String, String>,
    avisos: &'a mut Vec<String>,
}

impl Importar<'_> {
    fn nombre(&self, n: &str) -> String {
        match &self.prefijo {
            Some(p) => format!("{p}{n}"),
            None => n.to_string(),
        }
    }

    /// El tipo de linea `n` de `origen` en el destino (lo crea si falta).
    fn tipo(&mut self, origen: &CadDocument, n: &str) -> String {
        if TIPOS_COMUNES.contains(&n.to_uppercase().as_str()) {
            return n.to_string();
        }
        let clave = n.to_uppercase();
        if let Some(d) = self.tipos.get(&clave) {
            return d.clone();
        }
        let dest = self.nombre(n);
        if !self.destino.line_types.contains(&dest) {
            match origen.line_types.get(n) {
                Some(lt) => {
                    let mut lt = lt.clone();
                    lt.name = dest.clone();
                    lt.handle = self.destino.allocate_handle();
                    lt.xref_dependent = self.prefijo.is_some();
                    lt.xref_block_record_handle = if self.prefijo.is_some() {
                        self.registro
                    } else {
                        Handle::NULL
                    };
                    for el in &mut lt.elements {
                        if let Some(c) = el.complex.as_mut() {
                            c.style_handle = Handle::NULL;
                        }
                    }
                    let _ = self.destino.line_types.add(lt);
                }
                // Un tipo que el plano no define: se queda sin renombrar.
                None => {
                    self.tipos.insert(clave, n.to_string());
                    return n.to_string();
                }
            }
        }
        self.tipos.insert(clave, dest.clone());
        dest
    }

    /// La capa `n` de `origen` en el destino (la crea si falta).
    fn capa(&mut self, origen: &CadDocument, n: &str) -> String {
        if CAPAS_COMUNES.iter().any(|c| c.eq_ignore_ascii_case(n)) || n.is_empty() {
            let n = if n.is_empty() { "0" } else { n };
            if !self.destino.layers.contains(n) {
                let mut l = origen
                    .layers
                    .get(n)
                    .cloned()
                    .unwrap_or_else(|| Layer::new(n));
                l.handle = self.destino.allocate_handle();
                let _ = self.destino.layers.add(l);
            }
            return n.to_string();
        }
        let clave = n.to_uppercase();
        if let Some(d) = self.capas.get(&clave) {
            return d.clone();
        }
        let dest = self.nombre(n);
        if !self.destino.layers.contains(&dest) {
            let mut l = origen
                .layers
                .get(n)
                .cloned()
                .unwrap_or_else(|| Layer::new(n));
            l.line_type = self.tipo(origen, &l.line_type.clone());
            l.name = dest.clone();
            l.handle = self.destino.allocate_handle();
            l.flags.xref_dependent = self.prefijo.is_some();
            l.xref_block_record_handle = if self.prefijo.is_some() {
                self.registro
            } else {
                Handle::NULL
            };
            l.material = Handle::NULL;
            l.plotstyle_handle = Handle::NULL;
            let _ = self.destino.layers.add(l);
        }
        self.capas.insert(clave, dest.clone());
        dest
    }

    /// Un nombre de bloque anonimo (`*U`, `*D`…) libre en el destino.
    fn anonimo_libre(&self, n: &str) -> String {
        let letra: String = n.chars().take(2).collect();
        let mut i = 1usize;
        loop {
            let c = format!("{letra}{i}");
            if !self.destino.block_records.contains(&c)
                && !self.bloques.values().any(|v| v.eq_ignore_ascii_case(&c))
            {
                return c;
            }
            i += 1;
        }
    }

    /// Deja una entidad de `origen` lista para el destino: sus capas, tipos
    /// y bloques con los nombres de aqui, y sin handles de alla.
    fn adaptar(&mut self, origen: &CadDocument, e: &mut EntityType) {
        let capa = self.capa(origen, &e.common().layer.clone());
        let tipo = self.tipo(origen, &e.common().linetype.clone());
        let capa_h = self.destino.layers.get(&capa).map(|l| l.handle);
        let tipo_h = self.destino.line_types.get(&tipo).map(|l| l.handle);
        e.as_entity_mut().set_handle(Handle::NULL);
        let c = e.common_mut();
        c.layer = capa;
        c.layer_handle = capa_h;
        c.linetype = tipo;
        c.linetype_handle = tipo_h;
        c.extended_data = Default::default();
        c.reactors.clear();
        c.xdictionary_handle = None;
        c.color_book_handle = None;
        c.full_visual_style_handle = None;
        c.face_visual_style_handle = None;
        c.edge_visual_style_handle = None;
        c.material_handle = None;
        c.plotstyle_handle = None;
        let bloque = |b: &mut String, m: &HashMap<String, String>| {
            if let Some(d) = m.get(&b.to_uppercase()) {
                *b = d.clone();
            }
        };
        match e {
            EntityType::Insert(i) => {
                bloque(&mut i.block_name, &self.bloques);
                i.seqend_handle = None;
                i.view_rep_handle = None;
                for a in &mut i.attributes {
                    let capa = self.capa(origen, &a.common.layer.clone());
                    a.common.handle = Handle::NULL;
                    a.common.owner_handle = Handle::NULL;
                    a.common.layer_handle = self.destino.layers.get(&capa).map(|l| l.handle);
                    a.common.layer = capa;
                    a.common.reactors.clear();
                    a.common.xdictionary_handle = None;
                    a.attdef_handle = Handle::NULL;
                }
            }
            EntityType::Dimension(d) => {
                let b = d.base_mut();
                bloque(&mut b.block_name, &self.bloques);
            }
            EntityType::Hatch(h) => {
                h.is_associative = false;
                for p in &mut h.paths {
                    p.boundary_handles.clear();
                }
            }
            EntityType::Leader(l) => l.annotation_handle = Handle::NULL,
            EntityType::Polyline3D(p) => {
                for v in &mut p.vertices {
                    v.handle = Handle::NULL;
                }
            }
            _ => {}
        }
    }

    /// Copia las entidades `v` de `origen` al bloque `h` del destino,
    /// corridas `corrimiento`. Da cuantas.
    fn copiar<'o>(
        &mut self,
        origen: &CadDocument,
        v: impl Iterator<Item = &'o EntityType>,
        h: Handle,
        corrimiento: Vector3,
    ) -> usize {
        let mover = corrimiento.x != 0.0 || corrimiento.y != 0.0 || corrimiento.z != 0.0;
        let mut n = 0;
        for e in v {
            if matches!(
                e,
                EntityType::Block(_) | EntityType::BlockEnd(_) | EntityType::Viewport(_)
            ) {
                continue;
            }
            let mut e = e.clone();
            self.adaptar(origen, &mut e);
            if mover {
                e.translate(corrimiento);
            }
            e.common_mut().owner_handle = h;
            if self.destino.add_entity(e).is_ok() {
                n += 1;
            }
        }
        n
    }

    /// **Todo el plano `origen`** (leido de `ruta`) en el bloque `h` del
    /// destino, con sus bloques y sus referencias anidadas. `cadena`: los
    /// ficheros por los que se viene (para no entrar en circulos).
    fn plano(
        &mut self,
        origen: &CadDocument,
        ruta: &Path,
        h: Handle,
        cadena: &mut Vec<PathBuf>,
    ) -> usize {
        cadena.push(absoluta(ruta));
        let mut pendientes: Vec<(String, Handle, Vector3)> = Vec::new();
        let mut anidadas: Vec<(String, Handle, String)> = Vec::new();
        // Primero los registros (con sus nombres nuevos), luego lo de dentro:
        // un bloque puede usar otro que va despues.
        for br in origen.block_records.iter() {
            if br.is_model_space() || br.is_paper_space() || br.is_layout() {
                continue;
            }
            let clave = br.name.to_uppercase();
            if es_referencia(br) {
                if br.flags.is_xref_overlay {
                    // Las superpuestas no entran anidadas (como en AutoCAD).
                    continue;
                }
                let dest = self.nombre(&br.name);
                if self.destino.block_records.contains(&dest) {
                    self.bloques.insert(clave, dest);
                    continue;
                }
                let nh = nuevo_registro(self.destino, &dest, false);
                self.bloques.insert(clave, dest.clone());
                anidadas.push((br.name.clone(), nh, br.xref_path.clone()));
                continue;
            }
            let anonimo = br.name.starts_with('*') || br.flags.anonymous;
            let dest = if self.prefijo.is_none() && anonimo {
                self.anonimo_libre(&br.name)
            } else {
                self.nombre(&br.name)
            };
            if self.destino.block_records.contains(&dest) {
                // Sin prefijo, gana la definicion que ya tiene el dibujo.
                self.bloques.insert(clave, dest);
                continue;
            }
            let nh = nuevo_registro(self.destino, &dest, anonimo);
            if let Some(n) = self.destino.block_records.get_mut(&dest) {
                n.flags.has_attributes = br.flags.has_attributes;
                n.units = br.units;
                n.explodable = br.explodable;
                n.description = br.description.clone();
            }
            self.bloques.insert(clave, dest);
            let base = crate::bloque::base_de(origen, &br.name);
            pendientes.push((br.name.clone(), nh, Vector3::new(-base[0], -base[1], 0.0)));
        }
        for (nombre, nh, corr) in pendientes {
            self.copiar(origen, origen.entities_in_block(&nombre), nh, corr);
        }
        // Las capas del plano, aunque no se usen (salen en la lista).
        for l in origen.layers.iter() {
            self.capa(origen, &l.name.clone());
        }
        for s in origen.text_styles.iter() {
            if !self.destino.text_styles.contains(&s.name) {
                let mut s = s.clone();
                s.handle = self.destino.allocate_handle();
                let _ = self.destino.text_styles.add(s);
            }
        }
        let base = origen.header.model_space_insertion_base;
        let n = self.copiar(
            origen,
            origen.model_space_entities(),
            h,
            Vector3::new(-base.x, -base.y, -base.z),
        );
        for (sub, nh, guardada) in anidadas {
            self.anidada(&sub, nh, &guardada, ruta, cadena);
        }
        cadena.pop();
        n
    }

    /// Una referencia dentro de la que se carga: su espacio modelo en el
    /// bloque `h`, con lo suyo como `PREFIJO|SUB|…`.
    fn anidada(
        &mut self,
        sub: &str,
        h: Handle,
        guardada: &str,
        padre: &Path,
        cadena: &mut Vec<PathBuf>,
    ) {
        if cadena.len() >= PROFUNDIDAD {
            self.avisos.push(format!(
                "«{sub}»: demasiadas referencias una dentro de otra"
            ));
            return;
        }
        let Some(ruta) = resolver(guardada, Some(padre)) else {
            self.avisos.push(format!(
                "Referencia anidada «{sub}»: no se encuentra «{guardada}»"
            ));
            return;
        };
        if cadena.iter().any(|c| misma(c, &ruta)) {
            self.avisos.push(format!(
                "Referencia anidada «{sub}»: es circular, no se carga"
            ));
            return;
        }
        let origen = match leer_archivo(&ruta) {
            Ok(d) => d,
            Err(e) => {
                self.avisos.push(format!("Referencia anidada «{sub}»: {e}"));
                return;
            }
        };
        let mut hija = Importar {
            destino: &mut *self.destino,
            prefijo: Some(format!(
                "{}{sub}{SEPARADOR}",
                self.prefijo.clone().unwrap_or_default()
            )),
            registro: self.registro,
            capas: HashMap::new(),
            tipos: HashMap::new(),
            bloques: HashMap::new(),
            avisos: &mut *self.avisos,
        };
        hija.plano(&origen, &ruta, h, cadena);
    }
}

/// Un registro de bloque nuevo (normal) llamado `nombre`. Da su handle.
fn nuevo_registro(doc: &mut CadDocument, nombre: &str, anonimo: bool) -> Handle {
    let mut br = BlockRecord::new(nombre);
    br.handle = doc.allocate_handle();
    br.block_entity_handle = doc.allocate_handle();
    br.block_end_handle = doc.allocate_handle();
    br.flags.anonymous = anonimo;
    let h = br.handle;
    let _ = doc.block_records.add(br);
    h
}

/// Quita las entidades del bloque `nombre` (no el registro).
fn vaciar_bloque(doc: &mut CadDocument, nombre: &str) {
    let hs = match doc.block_records.get_mut(nombre) {
        Some(br) => std::mem::take(&mut br.entity_handles),
        None => return,
    };
    for h in hs {
        doc.remove_entity_arc(h);
    }
}

/// **Quita lo cargado** de la referencia `nombre`: su contenido y los
/// bloques que trajo (`NOMBRE|…`). Deja el registro y las capas.
fn vaciar(doc: &mut CadDocument, nombre: &str) {
    vaciar_bloque(doc, nombre);
    let p = prefijo(nombre);
    let suyos: Vec<String> = doc
        .block_records
        .iter()
        .filter(|b| es_de(&b.name, &p))
        .map(|b| b.name.clone())
        .collect();
    for b in suyos {
        vaciar_bloque(doc, &b);
        doc.block_records.remove(&b);
    }
}

/// Quita las capas y los tipos de linea `NOMBRE|…`.
fn quitar_simbolos(doc: &mut CadDocument, nombre: &str) {
    let p = prefijo(nombre);
    let capas: Vec<String> = doc
        .layers
        .iter()
        .filter(|l| es_de(&l.name, &p))
        .map(|l| l.name.clone())
        .collect();
    for c in capas {
        doc.layers.remove(&c);
    }
    let tipos: Vec<String> = doc
        .line_types
        .iter()
        .filter(|l| es_de(&l.name, &p))
        .map(|l| l.name.clone())
        .collect();
    for t in tipos {
        doc.line_types.remove(&t);
    }
    if !doc.layers.contains(&doc.header.current_layer_name.clone()) {
        doc.header.current_layer_name = "0".into();
        if let Some(l) = doc.layers.get("0") {
            doc.header.current_layer_handle = l.handle;
        }
    }
}

// ---------------------------------------------------------------- ordenes

/// **Carga (o vuelve a cargar) la referencia `nombre`** desde su fichero,
/// buscado desde el dibujo `anfitrion`. Si no esta, queda vacia y da el
/// error (nunca rompe nada).
pub fn cargar(
    doc: &mut CadDocument,
    nombre: &str,
    anfitrion: Option<&Path>,
) -> Result<Carga, String> {
    let (nombre, guardada, h) = match doc.block_records.get(nombre) {
        Some(br) if es_referencia(br) => (br.name.clone(), br.xref_path.clone(), br.handle),
        _ => return Err(format!("No hay una referencia «{nombre}»")),
    };
    vaciar(doc, &nombre);
    let ruta =
        resolver(&guardada, anfitrion).ok_or_else(|| format!("no se encuentra «{guardada}»"))?;
    if anfitrion.is_some_and(|a| misma(a, &ruta)) {
        return Err("apunta al propio dibujo".into());
    }
    let origen = leer_archivo(&ruta)?;
    let mut avisos = Vec::new();
    let mut cadena: Vec<PathBuf> = anfitrion.map(absoluta).into_iter().collect();
    let mut imp = Importar {
        destino: doc,
        prefijo: Some(prefijo(&nombre)),
        registro: h,
        capas: HashMap::new(),
        tipos: HashMap::new(),
        bloques: HashMap::new(),
        avisos: &mut avisos,
    };
    let objetos = imp.plano(&origen, &ruta, h, &mut cadena);
    Ok(Carga { objetos, avisos })
}

/// **Carga todas las referencias** (al abrir el dibujo `anfitrion`). Da lo
/// que haya que decir de las que no se pudieron cargar.
pub fn cargar_todas(doc: &mut CadDocument, anfitrion: Option<&Path>) -> Vec<String> {
    let mut avisos = Vec::new();
    for n in nombres(doc) {
        match cargar(doc, &n, anfitrion) {
            Ok(c) => avisos.extend(c.avisos),
            Err(e) => avisos.push(format!("Referencia «{n}»: {e}")),
        }
    }
    avisos
}

/// Un nombre de bloque sacado de un nombre de fichero (sin las letras que
/// AutoCAD no admite), que no este ya en el dibujo.
fn nombre_libre(doc: &CadDocument, archivo: &Path) -> String {
    let base: String = archivo
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| match crate::bloque::validar_nombre(&c.to_string()) {
            Ok(_) => c,
            Err(_) if c == ' ' => c,
            Err(_) => '_',
        })
        .collect();
    let base = match base.trim() {
        "" => "REFERENCIA".to_string(),
        b => b.to_string(),
    };
    if !doc.block_records.contains(&base) {
        return base;
    }
    (2..)
        .map(|i| format!("{base}_{i}"))
        .find(|n| !doc.block_records.contains(n))
        .unwrap_or(base)
}

/// **Adjunta (o superpone) el plano `archivo`** al dibujo `anfitrion` como
/// referencia y lo carga. Si ya estaba adjunto, da la que habia sin volver a
/// leerla. Da su nombre, si es nueva y lo cargado.
pub fn adjuntar(
    doc: &mut CadDocument,
    anfitrion: Option<&Path>,
    archivo: &Path,
    superpuesta: bool,
) -> Result<(String, bool, Carga), String> {
    if !archivo.is_file() {
        return Err(format!("No existe «{}»", archivo.display()));
    }
    if anfitrion.is_some_and(|a| misma(a, archivo)) {
        return Err("Un dibujo no puede referenciarse a si mismo".into());
    }
    for n in nombres(doc) {
        let guardada = doc.block_records.get(&n).map(|b| b.xref_path.clone());
        if guardada
            .and_then(|g| resolver(&g, anfitrion))
            .is_some_and(|r| misma(&r, archivo))
        {
            return Ok((n, false, Carga::default()));
        }
    }
    let nombre = nombre_libre(doc, archivo);
    let mut br = BlockRecord::new(nombre.clone());
    br.handle = doc.allocate_handle();
    br.block_entity_handle = doc.allocate_handle();
    br.block_end_handle = doc.allocate_handle();
    br.flags.is_xref = true;
    br.flags.is_xref_overlay = superpuesta;
    br.xref_path = ruta_para_guardar(archivo, anfitrion);
    doc.block_records.add(br)?;
    match cargar(doc, &nombre, anfitrion) {
        Ok(c) => Ok((nombre, true, c)),
        Err(e) => {
            quitar_definicion(doc, &nombre);
            Err(e)
        }
    }
}

/// **Quita la definicion** de una referencia (o de un bloque): su
/// contenido, lo que trajo y su registro. No toca los INSERT.
pub fn quitar_definicion(doc: &mut CadDocument, nombre: &str) {
    vaciar(doc, nombre);
    quitar_simbolos(doc, nombre);
    doc.block_records.remove(nombre);
}

/// Cuantos INSERT de `nombre` hay en el espacio modelo.
pub fn inserciones(doc: &CadDocument, nombre: &str) -> usize {
    doc.model_space_entities()
        .filter(|e| matches!(e, EntityType::Insert(i) if i.block_name.eq_ignore_ascii_case(nombre)))
        .count()
}

/// **QUITARREF**: borra los INSERT de la referencia (un paso de deshacer) y
/// su definicion (esa no se deshace, como la de un bloque). Da cuantos
/// INSERT quito.
pub fn quitar(d: &mut Dibujo, nombre: &str) -> Result<usize, String> {
    let nombre =
        buscar(&d.doc, nombre).ok_or_else(|| format!("No hay una referencia «{nombre}»"))?;
    let en_modelo: Vec<Handle> = d
        .doc
        .model_space_entities()
        .filter_map(|e| match e {
            EntityType::Insert(i) if i.block_name.eq_ignore_ascii_case(&nombre) => {
                Some(i.common.handle)
            }
            _ => None,
        })
        .collect();
    d.editar("QUITARREF", |t| {
        for h in &en_modelo {
            t.quitar(*h);
        }
    });
    // Los de las presentaciones o de otros bloques, sin deshacer.
    let otros: Vec<Handle> = d
        .doc
        .entities()
        .filter_map(|e| match e {
            EntityType::Insert(i) if i.block_name.eq_ignore_ascii_case(&nombre) => {
                Some(i.common.handle)
            }
            _ => None,
        })
        .collect();
    for h in otros {
        d.doc.remove_entity_arc(h);
    }
    quitar_definicion(&mut d.doc, &nombre);
    d.tocar();
    Ok(en_modelo.len())
}

/// El nombre que toma un simbolo `NOMBRE|X` al unir: `NOMBRE$0$X` (con el
/// primer numero libre, como el XBIND de AutoCAD).
fn nombre_unido(n: &str, ocupado: impl Fn(&str) -> bool) -> String {
    let mut i = 0;
    loop {
        let c = n.replace(SEPARADOR, &format!("${i}$"));
        if !ocupado(&c) {
            return c;
        }
        i += 1;
    }
}

/// **UNIRREF** (XBIND, «Unir»): la referencia pasa a ser un bloque normal
/// del dibujo, con sus capas, tipos y bloques renombrados `NOMBRE$0$X`.
/// Tiene que estar cargada. Da cuantos objetos trae.
pub fn unir(doc: &mut CadDocument, nombre: &str) -> Result<usize, String> {
    let nombre = buscar(doc, nombre).ok_or_else(|| format!("No hay una referencia «{nombre}»"))?;
    let objetos = doc.entities_in_block(&nombre).count();
    if objetos == 0 {
        return Err(format!("La referencia «{nombre}» no esta cargada"));
    }
    let p = prefijo(&nombre);
    let mut capas = HashMap::new();
    for n in doc
        .layers
        .iter()
        .filter(|l| es_de(&l.name, &p))
        .map(|l| l.name.clone())
        .collect::<Vec<_>>()
    {
        let nuevo = nombre_unido(&n, |c| doc.layers.contains(c));
        if doc.layers.rename(&n, nuevo.clone()).is_ok()
            && let Some(l) = doc.layers.get_mut(&nuevo)
        {
            l.flags.xref_dependent = false;
            l.xref_block_record_handle = Handle::NULL;
            capas.insert(n.to_uppercase(), nuevo);
        }
    }
    let mut tipos = HashMap::new();
    for n in doc
        .line_types
        .iter()
        .filter(|l| es_de(&l.name, &p))
        .map(|l| l.name.clone())
        .collect::<Vec<_>>()
    {
        let nuevo = nombre_unido(&n, |c| doc.line_types.contains(c));
        if doc.line_types.rename(&n, nuevo.clone()).is_ok()
            && let Some(l) = doc.line_types.get_mut(&nuevo)
        {
            l.xref_dependent = false;
            l.xref_block_record_handle = Handle::NULL;
            tipos.insert(n.to_uppercase(), nuevo);
        }
    }
    for l in doc.layers.iter_mut() {
        if let Some(t) = tipos.get(&l.line_type.to_uppercase()) {
            l.line_type = t.clone();
        }
    }
    let mut bloques = HashMap::new();
    for n in doc
        .block_records
        .iter()
        .filter(|b| es_de(&b.name, &p))
        .map(|b| b.name.clone())
        .collect::<Vec<_>>()
    {
        let nuevo = nombre_unido(&n, |c| doc.block_records.contains(c));
        if doc.block_records.rename(&n, nuevo.clone()).is_ok() {
            bloques.insert(n.to_uppercase(), nuevo);
        }
    }
    // Las entidades que usan lo renombrado (las de la referencia, sus
    // bloques y lo que alguien dejo en sus capas).
    let tocar: Vec<Handle> = doc
        .entities()
        .filter(|e| {
            let c = e.common();
            es_de(&c.layer, &p)
                || es_de(&c.linetype, &p)
                || match e {
                    EntityType::Insert(i) => es_de(&i.block_name, &p),
                    EntityType::Dimension(d) => es_de(&d.base().block_name, &p),
                    _ => false,
                }
        })
        .map(|e| e.common().handle)
        .collect();
    for h in tocar {
        let lh = |n: &str| doc.layers.get(n).map(|l| l.handle);
        let Some(e) = doc.get_entity(h) else {
            continue;
        };
        let capa = capas.get(&e.common().layer.to_uppercase()).cloned();
        let capa_h = capa.as_deref().and_then(lh);
        let tipo = tipos.get(&e.common().linetype.to_uppercase()).cloned();
        let tipo_h = tipo
            .as_deref()
            .and_then(|t| doc.line_types.get(t).map(|l| l.handle));
        let Some(e) = doc.get_entity_mut(h) else {
            continue;
        };
        let c = e.common_mut();
        if let Some(n) = capa {
            c.layer = n;
            c.layer_handle = capa_h;
        }
        if let Some(n) = tipo {
            c.linetype = n;
            c.linetype_handle = tipo_h;
        }
        let renombrar = |b: &mut String| {
            if let Some(n) = bloques.get(&b.to_uppercase()) {
                *b = n.clone();
            }
        };
        match e {
            EntityType::Insert(i) => {
                renombrar(&mut i.block_name);
                for a in &mut i.attributes {
                    if let Some(n) = capas.get(&a.common.layer.to_uppercase()) {
                        a.common.layer = n.clone();
                    }
                }
            }
            EntityType::Dimension(d) => renombrar(&mut d.base_mut().block_name),
            _ => {}
        }
    }
    if let Some(br) = doc.block_records.get_mut(&nombre) {
        br.flags.is_xref = false;
        br.flags.is_xref_overlay = false;
        br.flags.is_xref_unloaded = false;
        br.xref_path.clear();
    }
    Ok(objetos)
}

/// **Las referencias del dibujo** con su estado (LISTAREF).
pub fn listar(doc: &CadDocument, anfitrion: Option<&Path>) -> Vec<Referencia> {
    nombres(doc)
        .into_iter()
        .filter_map(|n| {
            let br = doc.block_records.get(&n)?;
            let resuelta = resolver(&br.xref_path, anfitrion);
            let objetos = doc.entities_in_block(&n).count();
            let estado = match (&resuelta, objetos) {
                (None, _) => Estado::NoEncontrada,
                (Some(_), 0) => Estado::SinCargar,
                _ => Estado::Cargada,
            };
            Some(Referencia {
                ruta: br.xref_path.clone(),
                superpuesta: br.flags.is_xref_overlay,
                inserciones: inserciones(doc, &n),
                nombre: n,
                resuelta,
                estado,
                objetos,
            })
        })
        .collect()
}

/// **La copia que se guarda**: sin el contenido de las referencias ni lo que
/// trajeron (sus bloques, capas y tipos), para que el fichero las tenga
/// como referencias y no unidas. `None` si el dibujo no tiene ninguna (se
/// guarda tal cual, sin copiar).
pub fn para_guardar(doc: &CadDocument) -> Option<CadDocument> {
    let refs = nombres(doc);
    if refs.is_empty() {
        return None;
    }
    let mut c = doc.clone();
    for n in refs {
        vaciar(&mut c, &n);
        quitar_simbolos(&mut c, &n);
    }
    Some(c)
}

// ---------------------------------------------------------------- INSERTARDWG

/// **Inserta un plano como bloque** (el INSERT de un fichero de AutoCAD): su
/// espacio modelo pasa a ser la definicion `nombre` (o la del nombre del
/// fichero), con sus capas, tipos y bloques; lo que el dibujo ya tiene con
/// el mismo nombre gana. Si ya hay un bloque con ese nombre, se usa ese.
/// Da el nombre, si se creo ahora y lo cargado.
pub fn insertar_archivo(
    doc: &mut CadDocument,
    archivo: &Path,
    nombre: Option<&str>,
) -> Result<(String, bool, Carga), String> {
    if !archivo.is_file() {
        return Err(format!("No existe «{}»", archivo.display()));
    }
    let nombre = match nombre {
        Some(n) => crate::bloque::validar_nombre(n)?,
        None => archivo
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .and_then(|s| crate::bloque::validar_nombre(&s).ok())
            .unwrap_or_else(|| nombre_libre(doc, archivo)),
    };
    if let Some(br) = doc.block_records.get(&nombre) {
        let n = br.name.clone();
        return Ok((
            n,
            false,
            Carga {
                objetos: 0,
                avisos: vec![format!("Ya hay un bloque «{nombre}»: se usa el del dibujo")],
            },
        ));
    }
    let origen = leer_archivo(archivo)?;
    let h = nuevo_registro(doc, &nombre, false);
    let mut avisos = Vec::new();
    let mut cadena = Vec::new();
    let mut imp = Importar {
        destino: doc,
        prefijo: None,
        registro: Handle::NULL,
        capas: HashMap::new(),
        tipos: HashMap::new(),
        bloques: HashMap::new(),
        avisos: &mut avisos,
    };
    let objetos = imp.plano(&origen, archivo, h, &mut cadena);
    Ok((nombre, true, Carga { objetos, avisos }))
}

// ---------------------------------------------------------------- vigia

/// **Mira si cambiaron en el disco** los ficheros de las referencias (la
/// fecha de cambio). La app lo llama cada pocos segundos, nunca en cada
/// fotograma.
#[derive(Debug, Default, Clone)]
pub struct Vigia {
    vistas: HashMap<String, (PathBuf, Option<SystemTime>)>,
}

impl Vigia {
    /// Las referencias cuyo fichero cambio desde la ultima vez que se miro
    /// (la primera vez que se ve una, solo se apunta). Cada cambio se dice
    /// una vez.
    pub fn revisar(&mut self, doc: &CadDocument, anfitrion: Option<&Path>) -> Vec<String> {
        let mut cambiadas = Vec::new();
        let mut siguen = HashMap::new();
        for n in nombres(doc) {
            let Some(br) = doc.block_records.get(&n) else {
                continue;
            };
            let Some(ruta) = resolver(&br.xref_path, anfitrion) else {
                continue;
            };
            let fecha = std::fs::metadata(&ruta).and_then(|m| m.modified()).ok();
            let clave = n.to_uppercase();
            if let Some((r, f)) = self.vistas.get(&clave)
                && *r == ruta
                && *f != fecha
            {
                cambiadas.push(n.clone());
            }
            siguen.insert(clave, (ruta, fecha));
        }
        self.vistas = siguen;
        cambiadas
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::especialidad as esp;

    /// Una carpeta limpia para cada prueba.
    fn carpeta(nombre: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("cad-nucleo-xref-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Un plano con `n` lineas en la capa `capa` y un bloque «PIEZA» puesto
    /// una vez.
    fn plano(ruta: &Path, n: usize, capa: &str) {
        let mut d = Dibujo::nuevo();
        d.asegurar_capa(capa, 3);
        d.editar("x", |t| {
            for i in 0..n {
                let y = i as f64 * 10.0;
                t.anadir_en(esp::linea([0.0, y], [100.0, y]), capa);
            }
        });
        crate::bloque::definir(
            &mut d.doc,
            "PIEZA",
            vec![esp::circulo([0.0, 0.0], 5.0)],
            [0.0, 0.0],
        )
        .unwrap();
        d.editar("x", |t| {
            t.anadir_en(
                crate::bloque::insercion("PIEZA", [50.0, 50.0], 1.0, 0.0),
                capa,
            )
        });
        d.guardar_como(ruta).unwrap();
    }

    fn insertar(d: &mut Dibujo, nombre: &str) {
        d.editar("x", |t| {
            t.anadir(crate::bloque::insercion(nombre, [1000.0, 0.0], 1.0, 0.0))
        });
    }

    #[test]
    fn rutas_relativas_y_absolutas() {
        let dir = carpeta("rutas");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let base = sub.join("base.dxf");
        std::fs::write(&base, b"x").unwrap();
        let anf = dir.join("anfitrion.dwg");
        assert_eq!(ruta_para_guardar(&base, Some(&anf)), ".\\sub\\base.dxf");
        let otro = sub.join("otro.dwg");
        assert_eq!(ruta_para_guardar(&base, Some(&otro)), ".\\base.dxf");
        let arriba = dir.join("a").join("b.dwg");
        assert_eq!(ruta_para_guardar(&base, Some(&arriba)), "..\\sub\\base.dxf");
        assert_eq!(resolver(".\\sub\\base.dxf", Some(&anf)), Some(base.clone()));
        assert_eq!(
            resolver("..\\sub\\base.dxf", Some(&arriba)),
            Some(base.clone())
        );
        // Sin nombre: absoluta, y se resuelve tal cual.
        let abs = ruta_para_guardar(&base, None);
        assert!(es_absoluta(&abs));
        assert_eq!(resolver(&abs, None), Some(base.clone()));
        // Una ruta vieja que ya no vale: se busca el nombre junto al dibujo.
        assert_eq!(
            resolver("C:\\otro\\sitio\\base.dxf", Some(&otro)),
            Some(sub.join("base.dxf"))
        );
        // Caso negativo: no esta.
        assert_eq!(resolver(".\\nada.dwg", Some(&anf)), None);
        assert_eq!(resolver("", Some(&anf)), None);
    }

    #[test]
    fn adjuntar_carga_con_prefijo_y_guardar_la_deja_como_referencia() {
        let dir = carpeta("adjuntar");
        let base = dir.join("Base.dxf");
        plano(&base, 3, "MUROS");
        for ext in ["dwg", "dxf"] {
            let anf = dir.join(format!("anfitrion.{ext}"));
            let mut d = Dibujo::nuevo();
            d.ruta = Some(anf.clone());
            let (n, nueva, c) = adjuntar(&mut d.doc, Some(&anf), &base, false).unwrap();
            assert!(nueva);
            assert_eq!(n, "Base");
            // Tres lineas y el INSERT de la pieza.
            assert_eq!(c.objetos, 4);
            assert!(d.doc.layers.contains("Base|MUROS"));
            assert!(d.doc.block_records.contains("Base|PIEZA"));
            assert_eq!(d.doc.entities_in_block("Base|PIEZA").count(), 1);
            let ins: Vec<_> = d
                .doc
                .entities_in_block("Base")
                .filter_map(|e| match e {
                    EntityType::Insert(i) => Some(i.block_name.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(ins, ["Base|PIEZA"]);
            assert!(
                d.doc
                    .entities_in_block("Base")
                    .all(|e| e.common().layer == "Base|MUROS")
            );
            // Adjuntar otra vez el mismo fichero: la misma referencia.
            let (n2, nueva2, _) = adjuntar(&mut d.doc, Some(&anf), &base, false).unwrap();
            assert_eq!((n2.as_str(), nueva2), ("Base", false));
            insertar(&mut d, "Base");
            assert_eq!(listar(&d.doc, Some(&anf))[0].estado, Estado::Cargada);
            d.guardar_como(&anf).unwrap();
            // Lo de memoria sigue cargado.
            assert_eq!(d.doc.entities_in_block("Base").count(), 4);
            // El fichero: solo el registro, con su ruta relativa.
            let crudo = crate::dibujo::leer(&anf).unwrap();
            let br = crudo.block_records.get("Base").expect("{ext}: el registro");
            assert!(br.flags.is_xref, "{ext}");
            assert!(!br.flags.is_xref_overlay, "{ext}");
            assert_eq!(br.xref_path, ".\\Base.dxf", "{ext}");
            assert_eq!(crudo.entities_in_block("Base").count(), 0, "{ext}");
            assert!(!crudo.block_records.contains("Base|PIEZA"), "{ext}");
            assert!(!crudo.layers.contains("Base|MUROS"), "{ext}");
            assert!(
                !crudo.entities().any(|e| matches!(e, EntityType::Line(_))),
                "{ext}: las lineas de la referencia no se unen al guardar"
            );
            assert_eq!(inserciones(&crudo, "Base"), 1, "{ext}");
            // Y al abrir se vuelve a cargar.
            let d2 = Dibujo::abrir(&anf).unwrap();
            assert_eq!(d2.doc.entities_in_block("Base").count(), 4, "{ext}");
            assert!(d2.doc.layers.contains("Base|MUROS"), "{ext}");
        }
    }

    #[test]
    fn superpuesta_se_guarda_como_tal_y_no_entra_anidada() {
        let dir = carpeta("superpuesta");
        let b = dir.join("b.dxf");
        plano(&b, 2, "B");
        let a = dir.join("a.dxf");
        let mut da = Dibujo::nuevo();
        da.ruta = Some(a.clone());
        adjuntar(&mut da.doc, Some(&a), &b, true).unwrap();
        insertar(&mut da, "b");
        da.guardar_como(&a).unwrap();
        let crudo = crate::dibujo::leer(&a).unwrap();
        assert!(crudo.block_records.get("b").unwrap().flags.is_xref_overlay);
        // Al adjuntar «a», la superpuesta «b» no viene.
        let anf = dir.join("anf.dxf");
        let mut d = Dibujo::nuevo();
        d.ruta = Some(anf.clone());
        let (_, _, c) = adjuntar(&mut d.doc, Some(&anf), &a, false).unwrap();
        assert!(!d.doc.block_records.contains("a|b"));
        // Solo el INSERT de «b» (que no se dibuja).
        assert_eq!(c.objetos, 1);
    }

    #[test]
    fn anidada_un_nivel_y_circular() {
        let dir = carpeta("anidada");
        let b = dir.join("b.dxf");
        plano(&b, 2, "B");
        let a = dir.join("a.dwg");
        let mut da = Dibujo::nuevo();
        da.ruta = Some(a.clone());
        adjuntar(&mut da.doc, Some(&a), &b, false).unwrap();
        insertar(&mut da, "b");
        da.guardar_como(&a).unwrap();
        let anf = dir.join("anf.dxf");
        let mut d = Dibujo::nuevo();
        d.ruta = Some(anf.clone());
        let (n, _, c) = adjuntar(&mut d.doc, Some(&anf), &a, false).unwrap();
        assert!(c.avisos.is_empty(), "{:?}", c.avisos);
        assert_eq!(n, "a");
        assert!(d.doc.block_records.contains("a|b"));
        assert_eq!(d.doc.entities_in_block("a|b").count(), 3);
        assert!(d.doc.layers.contains("a|b|B"));
        assert!(d.doc.block_records.contains("a|b|PIEZA"));
        // Circular: «b» referencia a «a», que referencia a «b».
        let mut db = Dibujo::abrir(&b).unwrap();
        adjuntar(&mut db.doc, Some(&b), &a, false).unwrap();
        insertar(&mut db, "a");
        db.guardar_como(&b).unwrap();
        let mut d = Dibujo::nuevo();
        d.ruta = Some(anf.clone());
        let (_, _, c) = adjuntar(&mut d.doc, Some(&anf), &a, false).unwrap();
        assert!(
            c.avisos.iter().any(|a| a.contains("circular")),
            "{:?}",
            c.avisos
        );
    }

    #[test]
    fn recargar_ve_los_cambios_y_el_vigia_los_avisa_una_vez() {
        let dir = carpeta("recargar");
        let base = dir.join("base.dxf");
        plano(&base, 2, "X");
        let anf = dir.join("anf.dwg");
        let mut d = Dibujo::nuevo();
        d.ruta = Some(anf.clone());
        adjuntar(&mut d.doc, Some(&anf), &base, false).unwrap();
        let mut v = Vigia::default();
        assert!(v.revisar(&d.doc, Some(&anf)).is_empty());
        // Cambia el fichero (con otra fecha: algunos sistemas tienen 1 s).
        std::thread::sleep(std::time::Duration::from_millis(1100));
        plano(&base, 5, "X");
        assert_eq!(v.revisar(&d.doc, Some(&anf)), ["base"]);
        assert!(v.revisar(&d.doc, Some(&anf)).is_empty());
        let c = cargar(&mut d.doc, "BASE", Some(&anf)).unwrap();
        assert_eq!(c.objetos, 6);
        assert_eq!(d.doc.entities_in_block("base").count(), 6);
        // Solo una definicion de la pieza (la vieja se fue).
        assert_eq!(d.doc.entities_in_block("base|PIEZA").count(), 1);
    }

    #[test]
    fn el_fichero_que_falta_se_dice_y_no_rompe() {
        let dir = carpeta("falta");
        let base = dir.join("base.dxf");
        plano(&base, 2, "X");
        let anf = dir.join("anf.dxf");
        let mut d = Dibujo::nuevo();
        d.ruta = Some(anf.clone());
        adjuntar(&mut d.doc, Some(&anf), &base, false).unwrap();
        insertar(&mut d, "base");
        d.guardar_como(&anf).unwrap();
        std::fs::remove_file(&base).unwrap();
        let (doc, avisos) = leer_con_referencias(&anf).unwrap();
        assert_eq!(avisos.len(), 1);
        assert!(avisos[0].contains("no se encuentra"), "{avisos:?}");
        let l = listar(&doc, Some(&anf));
        assert_eq!(l[0].estado, Estado::NoEncontrada);
        assert_eq!(l[0].inserciones, 1);
        // Adjuntar algo que no existe: error, y nada a medias.
        let mut d = Dibujo::nuevo();
        assert!(adjuntar(&mut d.doc, Some(&anf), &dir.join("nada.dwg"), false).is_err());
        assert!(nombres(&d.doc).is_empty());
        // Ni a si mismo.
        assert!(adjuntar(&mut d.doc, Some(&anf), &anf, false).is_err());
        // Un fichero que no es un plano: el lector tolerante lo da vacio (o
        // falla); en ningun caso rompe ni trae nada.
        let malo = dir.join("malo.dwg");
        std::fs::write(&malo, b"AC1032 esto no es un dwg").unwrap();
        match adjuntar(&mut d.doc, Some(&anf), &malo, false) {
            Ok((_, _, c)) => assert_eq!(c.objetos, 0),
            Err(_) => assert!(nombres(&d.doc).is_empty()),
        }
    }

    #[test]
    fn unir_la_vuelve_bloque_y_quitar_la_borra() {
        let dir = carpeta("unir");
        let base = dir.join("base.dxf");
        plano(&base, 2, "X");
        let anf = dir.join("anf.dxf");
        let mut d = Dibujo::nuevo();
        d.ruta = Some(anf.clone());
        adjuntar(&mut d.doc, Some(&anf), &base, false).unwrap();
        insertar(&mut d, "base");
        assert_eq!(unir(&mut d.doc, "base").unwrap(), 3);
        assert!(nombres(&d.doc).is_empty());
        assert!(d.doc.layers.contains("base$0$X"));
        assert!(!d.doc.layers.contains("base|X"));
        assert!(d.doc.block_records.contains("base$0$PIEZA"));
        assert!(
            d.doc
                .entities_in_block("base")
                .any(|e| matches!(e, EntityType::Insert(i) if i.block_name == "base$0$PIEZA"))
        );
        assert!(
            d.doc
                .entities_in_block("base")
                .all(|e| e.common().layer == "base$0$X")
        );
        // Guardado: un bloque normal con todo dentro.
        d.guardar_como(&anf).unwrap();
        let crudo = crate::dibujo::leer(&anf).unwrap();
        assert!(!crudo.block_records.get("base").unwrap().flags.is_xref);
        assert_eq!(crudo.entities_in_block("base").count(), 3);
        // QUITARREF de otra.
        let otra = dir.join("otra.dxf");
        plano(&otra, 1, "Y");
        adjuntar(&mut d.doc, Some(&anf), &otra, false).unwrap();
        insertar(&mut d, "otra");
        insertar(&mut d, "otra");
        assert_eq!(quitar(&mut d, "OTRA").unwrap(), 2);
        assert!(!d.doc.block_records.contains("otra"));
        assert!(!d.doc.layers.contains("otra|Y"));
        assert_eq!(inserciones(&d.doc, "otra"), 0);
        // Casos negativos.
        assert!(quitar(&mut d, "nada").is_err());
        assert!(unir(&mut d.doc, "base").is_err(), "ya no es referencia");
    }

    #[test]
    fn insertar_archivo_trae_el_plano_como_bloque() {
        let dir = carpeta("insertardwg");
        let base = dir.join("Mueble.dwg");
        plano(&base, 2, "MADERA");
        let mut d = Dibujo::nuevo();
        let (n, nuevo, c) = insertar_archivo(&mut d.doc, &base, None).unwrap();
        assert_eq!((n.as_str(), nuevo), ("Mueble", true));
        assert_eq!(c.objetos, 3);
        assert!(d.doc.layers.contains("MADERA"));
        assert!(d.doc.block_records.contains("PIEZA"));
        assert!(!d.doc.block_records.get("Mueble").unwrap().flags.is_xref);
        // Otra vez: se usa el que hay.
        let (_, nuevo, c) = insertar_archivo(&mut d.doc, &base, None).unwrap();
        assert!(!nuevo && !c.avisos.is_empty());
        insertar(&mut d, "Mueble");
        let r = dir.join("con-mueble.dxf");
        d.guardar_como(&r).unwrap();
        let d2 = Dibujo::abrir(&r).unwrap();
        assert_eq!(d2.doc.entities_in_block("Mueble").count(), 3);
        assert_eq!(d2.doc.entities_in_block("PIEZA").count(), 1);
        assert!(insertar_archivo(&mut d.doc, &dir.join("nada.dwg"), None).is_err());
    }

    #[test]
    fn guardar_en_otra_carpeta_rehace_la_ruta() {
        let dir = carpeta("rebasar");
        let base = dir.join("base.dxf");
        plano(&base, 1, "X");
        let anf = dir.join("anf.dxf");
        let mut d = Dibujo::nuevo();
        d.ruta = Some(anf.clone());
        adjuntar(&mut d.doc, Some(&anf), &base, false).unwrap();
        let otra = dir.join("dentro");
        std::fs::create_dir_all(&otra).unwrap();
        let nueva = otra.join("anf.dxf");
        d.guardar_como(&nueva).unwrap();
        assert_eq!(
            d.doc.block_records.get("base").unwrap().xref_path,
            "..\\base.dxf"
        );
        let d2 = Dibujo::abrir(&nueva).unwrap();
        assert_eq!(d2.doc.entities_in_block("base").count(), 2);
    }
}
