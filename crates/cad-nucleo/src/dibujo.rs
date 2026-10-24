//! **El dibujo abierto**: el `CadDocument` de opencadcodec tal cual (asi al
//! guardar no se pierde nada de lo que no se toca: bloques, cotas,
//! sombreados, presentaciones…), con deshacer y rehacer y un indice para
//! encontrar rapido lo que hay bajo el cursor.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use opencadcodec::{CadDocument, Color, EntityType, Handle, Layer};

use crate::formas;
use crate::geo::{self, Caja, Forma};

/// En que se guarda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Dwg,
    Dxf,
}

impl Formato {
    pub fn de_ruta(ruta: &Path) -> Formato {
        match ruta
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
        {
            Some(e) if e == "dxf" => Formato::Dxf,
            _ => Formato::Dwg,
        }
    }
}

/// Un cambio de una entidad: como estaba y como quedo (`None`: no estaba).
#[derive(Debug, Clone)]
struct Cambio {
    handle: Handle,
    antes: Option<Arc<EntityType>>,
    despues: Option<Arc<EntityType>>,
}

/// Lo que hizo una orden, para deshacerlo de una vez.
#[derive(Debug, Clone)]
struct Paso {
    nombre: String,
    cambios: Vec<Cambio>,
}

/// Cuantos pasos se pueden deshacer.
const PASOS: usize = 200;

/// Una entidad del espacio modelo, ya en formas simples.
#[derive(Debug, Clone)]
pub struct Entrada {
    pub handle: Handle,
    pub caja: Caja,
    pub formas: Vec<Forma>,
    /// Puntos de insercion (textos, bloques, puntos).
    pub puntos: Vec<geo::P>,
    /// En una capa bloqueada: se ve y se engancha, pero no se elige.
    pub bloqueada: bool,
}

/// **El indice**: lo visible del espacio modelo en una rejilla de celdas.
#[derive(Debug, Default)]
pub struct Indice {
    pub entradas: Vec<Entrada>,
    caja: Caja,
    lado: f64,
    columnas: usize,
    filas: usize,
    celdas: Vec<Vec<u32>>,
}

impl Indice {
    fn de(doc: &CadDocument) -> Indice {
        let mut entradas = Vec::new();
        for e in doc.model_space_entities() {
            let c = e.common();
            let capa = doc.layers.get(&c.layer);
            if c.invisible || capa.is_some_and(|l| !l.is_visible()) {
                continue;
            }
            let f = formas::formas(e);
            let puntos = formas::puntos_de_insercion(e);
            let Some(caja) = formas::caja(e, &f) else {
                continue;
            };
            entradas.push(Entrada {
                handle: c.handle,
                caja,
                formas: f,
                puntos,
                bloqueada: capa.is_some_and(|l| l.is_locked()),
            });
        }
        let mut total = geo::CAJA_VACIA;
        for e in &entradas {
            total = geo::unir_cajas(total, e.caja);
        }
        if entradas.is_empty() || !geo::caja_valida(&total) {
            return Indice {
                entradas,
                ..Default::default()
            };
        }
        // Unas 4 entidades por celda, con un tope de celdas.
        let n = entradas.len().clamp(1, 1 << 20) as f64;
        let (w, h) = (
            (total[2] - total[0]).max(1e-6),
            (total[3] - total[1]).max(1e-6),
        );
        let lado = ((w * h) / (n / 4.0)).sqrt().max(w.max(h) / 1024.0);
        let columnas = ((w / lado).ceil() as usize).clamp(1, 1024);
        let filas = ((h / lado).ceil() as usize).clamp(1, 1024);
        let mut celdas = vec![Vec::new(); columnas * filas];
        let mut ind = Indice {
            entradas: Vec::new(),
            caja: total,
            lado,
            columnas,
            filas,
            celdas: Vec::new(),
        };
        for (k, e) in entradas.iter().enumerate() {
            let (c0, f0, c1, f1) = ind.rango(&e.caja);
            for f in f0..=f1 {
                for c in c0..=c1 {
                    celdas[f * columnas + c].push(k as u32);
                }
            }
        }
        ind.entradas = entradas;
        ind.celdas = celdas;
        ind
    }

    fn rango(&self, c: &Caja) -> (usize, usize, usize, usize) {
        let col = |x: f64| {
            (((x - self.caja[0]) / self.lado).floor().max(0.0) as usize).min(self.columnas - 1)
        };
        let fil = |y: f64| {
            (((y - self.caja[1]) / self.lado).floor().max(0.0) as usize).min(self.filas - 1)
        };
        (col(c[0]), fil(c[1]), col(c[2]), fil(c[3]))
    }

    /// Las entradas cuya caja toca `zona`.
    pub fn en(&self, zona: &Caja) -> Vec<&Entrada> {
        if self.celdas.is_empty() {
            return self
                .entradas
                .iter()
                .filter(|e| geo::se_tocan(&e.caja, zona))
                .collect();
        }
        if !geo::se_tocan(&self.caja, zona) {
            return Vec::new();
        }
        let (c0, f0, c1, f1) = self.rango(zona);
        let mut vistos = std::collections::HashSet::new();
        let mut v = Vec::new();
        for f in f0..=f1 {
            for c in c0..=c1 {
                for &k in &self.celdas[f * self.columnas + c] {
                    let e = &self.entradas[k as usize];
                    if vistos.insert(k) && geo::se_tocan(&e.caja, zona) {
                        v.push(e);
                    }
                }
            }
        }
        v
    }

    pub fn de_handle(&self, h: Handle) -> Option<&Entrada> {
        self.entradas.iter().find(|e| e.handle == h)
    }

    /// La caja de todo lo visible.
    pub fn caja(&self) -> Option<Caja> {
        geo::caja_valida(&self.caja).then_some(self.caja)
    }
}

/// **El dibujo abierto.**
pub struct Dibujo {
    pub doc: CadDocument,
    pub ruta: Option<PathBuf>,
    hecho: Vec<Paso>,
    deshecho: Vec<Paso>,
    /// Sube con cada cambio: quien pinta sabe si tiene que volver a leer.
    version: u64,
    guardada: u64,
    indice: RefCell<Option<(u64, std::rc::Rc<Indice>)>>,
}

impl Dibujo {
    pub fn de_documento(doc: CadDocument, ruta: Option<PathBuf>) -> Dibujo {
        Dibujo {
            doc,
            ruta,
            hecho: Vec::new(),
            deshecho: Vec::new(),
            version: 1,
            guardada: 1,
            indice: RefCell::new(None),
        }
    }

    /// **Un dibujo nuevo**, en milimetros y con las capas de la plantilla.
    pub fn nuevo() -> Dibujo {
        let mut doc = CadDocument::new();
        // 4: milimetros ($INSUNITS).
        doc.header.insertion_units = 4;
        for c in crate::especialidad::CAPAS {
            let _ = doc
                .layers
                .add(Layer::with_color(c.nombre, Color::from_index(c.color)));
        }
        let mut d = Dibujo::de_documento(doc, None);
        d.guardada = 0;
        d
    }

    /// **Abre un DWG o un DXF**: primero estricto y, si se salto algo, en
    /// modo tolerante (lo que hace OpenCADStudio).
    pub fn abrir(ruta: &Path) -> Result<Dibujo, String> {
        Ok(Dibujo::de_documento(leer(ruta)?, Some(ruta.to_path_buf())))
    }

    /// **Guarda** en su ruta (DWG o DXF por la extension).
    pub fn guardar(&mut self) -> Result<(), String> {
        let ruta = self.ruta.clone().ok_or("el dibujo aun no tiene nombre")?;
        self.guardar_como(&ruta)
    }

    pub fn guardar_como(&mut self, ruta: &Path) -> Result<(), String> {
        // Primero a un temporal: un fallo a medias no deja el plano roto.
        let tmp = ruta.with_extension(format!(
            "{}.tmp",
            ruta.extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default()
        ));
        let r = match Formato::de_ruta(ruta) {
            Formato::Dxf => opencadcodec::DxfWriter::new(&self.doc).write_to_file(&tmp),
            Formato::Dwg => opencadcodec::DwgWriter::write_to_file(&tmp, &self.doc),
        };
        r.map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, ruta).map_err(|e| e.to_string())?;
        self.ruta = Some(ruta.to_path_buf());
        self.guardada = self.version;
        Ok(())
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// Si hay cambios sin guardar.
    pub fn sucio(&self) -> bool {
        self.version != self.guardada
    }

    /// Marca un cambio que no es de entidades (una capa, un ajuste).
    pub fn tocar(&mut self) {
        self.version += 1;
    }

    pub fn entidad(&self, h: Handle) -> Option<&EntityType> {
        self.doc.get_entity(h)
    }

    /// **El indice de ahora** (se rehace solo si el dibujo cambio).
    pub fn indice(&self) -> std::rc::Rc<Indice> {
        let mut i = self.indice.borrow_mut();
        if let Some((v, ind)) = i.as_ref()
            && *v == self.version
        {
            return ind.clone();
        }
        let ind = std::rc::Rc::new(Indice::de(&self.doc));
        *i = Some((self.version, ind.clone()));
        ind
    }

    /// **Hace una orden de una vez**: lo que cambie dentro se deshace junto.
    /// Si no cambio nada, no deja paso.
    pub fn editar<R>(&mut self, nombre: &str, f: impl FnOnce(&mut Transaccion) -> R) -> R {
        let mut t = Transaccion {
            doc: &mut self.doc,
            cambios: Vec::new(),
        };
        let r = f(&mut t);
        let mut cambios = std::mem::take(&mut t.cambios);
        for c in &mut cambios {
            c.despues = self.doc.get_entity_arc(c.handle);
        }
        cambios.retain(|c| match (&c.antes, &c.despues) {
            (None, None) => false,
            (Some(a), Some(d)) => !Arc::ptr_eq(a, d) && a != d,
            _ => true,
        });
        if !cambios.is_empty() {
            self.hecho.push(Paso {
                nombre: nombre.to_string(),
                cambios,
            });
            if self.hecho.len() > PASOS {
                self.hecho.remove(0);
            }
            self.deshecho.clear();
            self.version += 1;
        }
        r
    }

    pub fn se_puede_deshacer(&self) -> bool {
        !self.hecho.is_empty()
    }

    pub fn se_puede_rehacer(&self) -> bool {
        !self.deshecho.is_empty()
    }

    /// Deshace la ultima orden; dice cual era.
    pub fn deshacer(&mut self) -> Option<String> {
        let paso = self.hecho.pop()?;
        for c in paso.cambios.iter().rev() {
            poner(&mut self.doc, c.handle, c.antes.clone());
        }
        let nombre = paso.nombre.clone();
        self.deshecho.push(paso);
        self.version += 1;
        Some(nombre)
    }

    pub fn rehacer(&mut self) -> Option<String> {
        let paso = self.deshecho.pop()?;
        for c in &paso.cambios {
            poner(&mut self.doc, c.handle, c.despues.clone());
        }
        let nombre = paso.nombre.clone();
        self.hecho.push(paso);
        self.version += 1;
        Some(nombre)
    }

    // ------------------------------------------------------------- capas

    /// La capa en la que se dibuja.
    pub fn capa_actual(&self) -> String {
        let n = &self.doc.header.current_layer_name;
        if n.is_empty() || !self.doc.layers.contains(n) {
            "0".into()
        } else {
            n.clone()
        }
    }

    pub fn poner_capa_actual(&mut self, nombre: &str) {
        self.asegurar_capa(nombre, 7);
        self.doc.header.current_layer_name = nombre.to_string();
        if let Some(l) = self.doc.layers.get(nombre) {
            self.doc.header.current_layer_handle = l.handle;
        }
        self.tocar();
    }

    /// Crea la capa si no esta.
    pub fn asegurar_capa(&mut self, nombre: &str, color: i16) {
        if !self.doc.layers.contains(nombre) {
            let _ = self
                .doc
                .layers
                .add(Layer::with_color(nombre, Color::from_index(color)));
            self.tocar();
        }
    }

    /// Cambia algo de una capa (encender, bloquear, color…).
    pub fn con_capa(&mut self, nombre: &str, f: impl FnOnce(&mut Layer)) -> bool {
        let Some(l) = self.doc.layers.get_mut(nombre) else {
            return false;
        };
        f(l);
        self.tocar();
        true
    }
}

/// Deja la entidad `h` como `e` (o la quita si `None`).
fn poner(doc: &mut CadDocument, h: Handle, e: Option<Arc<EntityType>>) {
    let esta = doc.get_entity(h).is_some();
    match (esta, e) {
        (true, Some(e)) => {
            doc.replace_entity_arc(h, e);
        }
        (true, None) => {
            doc.remove_entity_arc(h);
        }
        (false, Some(e)) => {
            doc.restore_entity_arc(e);
        }
        (false, None) => {}
    }
}

/// **Lo que puede hacer una orden** con el dibujo, apuntando cada cambio.
pub struct Transaccion<'a> {
    doc: &'a mut CadDocument,
    cambios: Vec<Cambio>,
}

impl Transaccion<'_> {
    fn apuntar(&mut self, h: Handle) {
        if !self.cambios.iter().any(|c| c.handle == h) {
            let antes = self.doc.get_entity_arc(h);
            self.cambios.push(Cambio {
                handle: h,
                antes,
                despues: None,
            });
        }
    }

    pub fn doc(&self) -> &CadDocument {
        self.doc
    }

    pub fn entidad(&self, h: Handle) -> Option<&EntityType> {
        self.doc.get_entity(h)
    }

    /// Anade una entidad nueva (sin handle: se le da uno).
    pub fn anadir(&mut self, mut e: EntityType) -> Option<Handle> {
        e.as_entity_mut().set_handle(Handle::NULL);
        let h = self.doc.add_entity(e).ok()?;
        self.cambios.push(Cambio {
            handle: h,
            antes: None,
            despues: None,
        });
        Some(h)
    }

    /// Anade en la capa actual (si la entidad no dice otra).
    pub fn anadir_en(&mut self, mut e: EntityType, capa: &str) -> Option<Handle> {
        e.common_mut().layer = capa.to_string();
        e.common_mut().layer_handle = self.doc.layers.get(capa).map(|l| l.handle);
        self.anadir(e)
    }

    /// Cambia una entidad que ya esta.
    pub fn cambiar(&mut self, h: Handle, f: impl FnOnce(&mut EntityType)) -> bool {
        if self.doc.get_entity(h).is_none() {
            return false;
        }
        self.apuntar(h);
        match self.doc.get_entity_mut(h) {
            Some(e) => {
                f(e);
                true
            }
            None => false,
        }
    }

    /// Pone `nueva` en el sitio de `h` (mismo handle).
    pub fn reemplazar(&mut self, h: Handle, mut nueva: EntityType) -> bool {
        if self.doc.get_entity(h).is_none() {
            return false;
        }
        self.apuntar(h);
        nueva.as_entity_mut().set_handle(h);
        self.doc.replace_entity_arc(h, Arc::new(nueva)).is_some()
    }

    pub fn quitar(&mut self, h: Handle) -> bool {
        if self.doc.get_entity(h).is_none() {
            return false;
        }
        self.apuntar(h);
        self.doc.remove_entity_arc(h).is_some()
    }

    /// Desmonta una entidad en sus piezas (handles nuevos) y la quita.
    pub fn descomponer(&mut self, h: Handle) -> Vec<Handle> {
        let Some(e) = self.doc.get_entity(h).cloned() else {
            return Vec::new();
        };
        let partes = self.doc.explode_entity(&e);
        if partes.is_empty() {
            return Vec::new();
        }
        self.quitar(h);
        partes.into_iter().filter_map(|p| self.anadir(p)).collect()
    }
}

/// Si los primeros bytes son de un DWG (`AC10xx`).
pub fn es_dwg(cabeza: &[u8]) -> bool {
    cabeza.len() >= 6 && cabeza.starts_with(b"AC10")
}

/// Lee el plano (copia de `pixpin_cad::leer::leer`, que no compila fuera de
/// Windows).
pub fn leer(ruta: &Path) -> Result<CadDocument, String> {
    let mut cabeza = [0u8; 6];
    {
        use std::io::Read;
        let mut f = std::fs::File::open(ruta).map_err(|e| e.to_string())?;
        let _ = f.read(&mut cabeza);
    }
    if es_dwg(&cabeza) {
        use opencadcodec::{DwgReadOptions, DwgReader};
        let estricto = DwgReader::from_file(ruta)
            .map_err(|e| e.to_string())?
            .read_with_stats();
        match estricto {
            Ok(r) if r.stats.skipped_source_records == 0 && r.stats.stream_completed => {
                Ok(r.document)
            }
            _ => DwgReader::from_file_with_options(ruta, DwgReadOptions::failsafe())
                .map_err(|e| e.to_string())?
                .read()
                .map_err(|e| e.to_string()),
        }
    } else {
        use opencadcodec::{DxfReader, DxfReaderConfiguration};
        let estricto = DxfReader::from_file(ruta)
            .map_err(|e| e.to_string())?
            .read_with_stats();
        match estricto {
            Ok(r) if r.stats.skipped_source_records == 0 && r.stats.stream_completed => {
                Ok(r.document)
            }
            _ => DxfReader::from_file(ruta)
                .map_err(|e| e.to_string())?
                .with_configuration(DxfReaderConfiguration {
                    failsafe: true,
                    default_encoding: None,
                })
                .read()
                .map_err(|e| e.to_string()),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use opencadcodec::Line;

    fn linea(x0: f64, y0: f64, x1: f64, y1: f64) -> EntityType {
        EntityType::Line(Line::from_coords(x0, y0, 0.0, x1, y1, 0.0))
    }

    #[test]
    fn deshacer_y_rehacer_anadir_cambiar_y_quitar() {
        let mut d = Dibujo::nuevo();
        let h = d
            .editar("LINEA", |t| t.anadir(linea(0.0, 0.0, 10.0, 0.0)))
            .unwrap();
        assert!(d.entidad(h).is_some());
        d.editar("MOVER", |t| {
            t.cambiar(h, |e| {
                e.translate(opencadcodec::Vector3::new(0.0, 5.0, 0.0))
            });
        });
        let EntityType::Line(l) = d.entidad(h).unwrap() else {
            panic!()
        };
        assert_eq!(l.start.y, 5.0);
        d.editar("BORRAR", |t| t.quitar(h));
        assert!(d.entidad(h).is_none());
        assert_eq!(d.deshacer().as_deref(), Some("BORRAR"));
        assert!(d.entidad(h).is_some());
        assert_eq!(d.deshacer().as_deref(), Some("MOVER"));
        let EntityType::Line(l) = d.entidad(h).unwrap() else {
            panic!()
        };
        assert_eq!(l.start.y, 0.0);
        assert_eq!(d.deshacer().as_deref(), Some("LINEA"));
        assert!(d.entidad(h).is_none());
        assert!(d.deshacer().is_none());
        d.rehacer();
        d.rehacer();
        let EntityType::Line(l) = d.entidad(h).unwrap() else {
            panic!()
        };
        assert_eq!(l.start.y, 5.0);
        // Una orden que no cambia nada no deja paso.
        d.editar("NADA", |_| {});
        assert_eq!(d.deshacer().as_deref(), Some("MOVER"));
    }

    #[test]
    fn el_indice_encuentra_lo_cercano_y_se_rehace_al_cambiar() {
        let mut d = Dibujo::nuevo();
        let mut hs = Vec::new();
        d.editar("LINEAS", |t| {
            for i in 0..100 {
                hs.push(
                    t.anadir(linea(i as f64 * 10.0, 0.0, i as f64 * 10.0, 5.0))
                        .unwrap(),
                );
            }
        });
        let ind = d.indice();
        let cerca = ind.en(&[495.0, 1.0, 505.0, 2.0]);
        assert_eq!(cerca.len(), 1);
        assert_eq!(cerca[0].handle, hs[50]);
        d.editar("BORRAR", |t| t.quitar(hs[50]));
        assert!(d.indice().en(&[495.0, 1.0, 505.0, 2.0]).is_empty());
    }

    #[test]
    fn guarda_y_vuelve_a_abrir_un_dxf() {
        let dir = std::env::temp_dir().join(format!("cad-nucleo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("prueba.dxf");
        let mut d = Dibujo::nuevo();
        d.editar("LINEA", |t| {
            t.anadir_en(linea(0.0, 0.0, 3.0, 4.0), "A-MUROS")
        });
        d.guardar_como(&ruta).unwrap();
        assert!(!d.sucio());
        let d2 = Dibujo::abrir(&ruta).unwrap();
        let lineas: Vec<_> = d2
            .doc
            .model_space_entities()
            .filter_map(|e| match e {
                EntityType::Line(l) => Some(l.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(lineas.len(), 1);
        assert_eq!(lineas[0].common.layer, "A-MUROS");
        assert!((lineas[0].length() - 5.0).abs() < 1e-9);
        assert!(d2.doc.layers.contains("A-COTAS"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
