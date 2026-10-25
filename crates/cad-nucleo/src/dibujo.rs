//! **El dibujo abierto**: el `CadDocument` de opencadcodec tal cual (asi al
//! guardar no se pierde nada de lo que no se toca: bloques, cotas,
//! sombreados, presentaciones…), con deshacer y rehacer y un indice para
//! encontrar rapido lo que hay bajo el cursor.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
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

impl Paso {
    fn handles(&self) -> Vec<Handle> {
        self.cambios.iter().map(|c| c.handle).collect()
    }
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

/// **Un hash rapido para los handles**, que son numeros casi seguidos: con
/// multiplicar basta (el de siempre, SipHash, es varias veces mas lento).
#[derive(Debug, Clone, Copy, Default)]
pub struct HashDeHandle(u64);

const MEZCLA: u64 = 0x517c_c1b7_2722_0a95;

impl std::hash::Hasher for HashDeHandle {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u64(*b as u64);
        }
    }

    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(MEZCLA);
    }
}

/// Un mapa con handles de llave y el hash rapido.
pub type PorHandle<V> = HashMap<Handle, V, BuildHasherDefault<HashDeHandle>>;

/// Lo que se sabe de cada capa al hacer el indice (`normalize_name` de
/// opencadcodec pide memoria en cada busqueda: mejor una vez por capa).
struct Capas<'a> {
    doc: &'a CadDocument,
    vistas: HashMap<&'a str, (bool, bool)>,
    /// La ultima que se miro (casi siempre la de la entidad siguiente).
    ultima: Option<(&'a str, (bool, bool))>,
    /// Las formas de cada bloque en su sistema (se sacan una vez por indice:
    /// un plano con mil sillas iguales no recorre el bloque mil veces).
    bloques: HashMap<String, std::rc::Rc<Vec<Forma>>>,
}

impl<'a> Capas<'a> {
    fn de(doc: &'a CadDocument) -> Capas<'a> {
        Capas {
            doc,
            vistas: HashMap::new(),
            ultima: None,
            bloques: HashMap::new(),
        }
    }

    /// Si la capa se ve y si esta bloqueada (una capa que no existe se ve).
    fn estado(&mut self, nombre: &'a str) -> (bool, bool) {
        if let Some((n, e)) = self.ultima
            && n == nombre
        {
            return e;
        }
        let doc = self.doc;
        let e = *self.vistas.entry(nombre).or_insert_with(|| {
            let capa = doc.layers.get(nombre);
            (
                capa.is_none_or(|l| l.is_visible()),
                capa.is_some_and(|l| l.is_locked()),
            )
        });
        self.ultima = Some((nombre, e));
        e
    }
}

/// La entrada de una entidad, o `None` si no se ve o no ocupa nada.
fn entrada<'a>(capas: &mut Capas<'a>, e: &'a EntityType) -> Option<Entrada> {
    let c = e.common();
    if c.invisible {
        return None;
    }
    let (visible, bloqueada) = capas.estado(&c.layer);
    if !visible {
        return None;
    }
    let f = match e {
        // Un bloque se elige y se engancha por lo que dibuja, no solo por
        // su punto de insercion.
        EntityType::Insert(i) if !i.is_minsert() => {
            let doc = capas.doc;
            let propias = capas
                .bloques
                .entry(i.block_name.clone())
                .or_insert_with(|| std::rc::Rc::new(crate::bloque::formas_de(doc, &i.block_name)))
                .clone();
            let (s, ins) = (i.x_scale(), formas::p2(&i.insert_point));
            let uniforme = (s - i.y_scale()).abs() < 1e-12 && s > 0.0 && i.normal.z >= 0.0;
            if uniforme {
                propias
                    .iter()
                    .map(|f| crate::bloque::transformar(f, ins, s, i.rotation))
                    .collect()
            } else {
                formas::formas(e)
            }
        }
        _ => formas::formas(e),
    };
    let caja = formas::caja(e, &f)?;
    Some(Entrada {
        handle: c.handle,
        caja,
        formas: f,
        puntos: formas::puntos_de_insercion(e),
        bloqueada,
    })
}

/// Con menos entradas que estas, la rejilla no se rehace por crecer o
/// menguar (sale barato mirarlas todas).
const MINIMO_REJILLA: usize = 64;

/// **El indice**: lo visible del espacio modelo en una rejilla de celdas.
///
/// Se pone al dia por partes ([`Indice::actualizar`]): solo lo que cambio
/// sale y vuelve a entrar. La rejilla se rehace (sin volver a sacar las
/// formas) cuando hay muchas mas o muchas menos entradas que al hacerla, o
/// cuando demasiadas se salen de ella.
#[derive(Debug, Clone)]
pub struct Indice {
    /// Las entradas, sin un orden fijo. Solo para leer: el indice sabe donde
    /// esta cada una ([`Indice::de_handle`]).
    pub entradas: Vec<Entrada>,
    /// Donde esta cada handle en `entradas`.
    lugar: PorHandle<u32>,
    /// La caja de todo lo visible.
    caja: Caja,
    /// Se quito algo que tocaba el borde de `caja`: hay que recalcularla.
    caja_vieja: bool,
    /// La caja que cubre la rejilla (la de todo al hacerla).
    marco: Caja,
    lado: f64,
    columnas: usize,
    filas: usize,
    celdas: Vec<Vec<u32>>,
    /// Cuantas entradas se salen del marco (estan en las celdas del borde).
    fuera: usize,
    /// Cuantas entradas habia al hacer la rejilla.
    al_hacerla: usize,
}

impl Default for Indice {
    fn default() -> Self {
        Indice {
            entradas: Vec::new(),
            lugar: PorHandle::default(),
            caja: geo::CAJA_VACIA,
            caja_vieja: false,
            marco: geo::CAJA_VACIA,
            lado: 1.0,
            columnas: 0,
            filas: 0,
            celdas: Vec::new(),
            fuera: 0,
            al_hacerla: 0,
        }
    }
}

impl Indice {
    /// **El indice entero** de lo visible en el espacio modelo de `doc`.
    pub fn completo(doc: &CadDocument) -> Indice {
        let mut capas = Capas::de(doc);
        let entradas: Vec<Entrada> = doc
            .model_space_entities()
            .filter_map(|e| entrada(&mut capas, e))
            .collect();
        let mut lugar = PorHandle::default();
        lugar.reserve(entradas.len());
        for (k, e) in entradas.iter().enumerate() {
            lugar.insert(e.handle, k as u32);
        }
        let mut ind = Indice {
            entradas,
            lugar,
            ..Default::default()
        };
        ind.rehacer_rejilla();
        ind
    }

    /// **Pone al dia** lo de los handles `hs` (anadidos, cambiados o
    /// quitados) mirando como estan ahora en `doc`.
    pub fn actualizar(&mut self, doc: &CadDocument, hs: &[Handle]) {
        let modelo = doc.block_records.get("*Model_Space").map(|b| b.handle);
        let mut capas = Capas::de(doc);
        for h in hs {
            let estaba = self.quitar(*h);
            let Some(e) = doc.get_entity(*h) else {
                continue;
            };
            // Lo de los bloques y las presentaciones no va al indice.
            if !estaba && Some(e.common().owner_handle) != modelo {
                continue;
            }
            if let Some(en) = entrada(&mut capas, e) {
                self.poner(en);
            }
        }
        let n = self.entradas.len();
        let base = self.al_hacerla.max(MINIMO_REJILLA);
        if (self.celdas.is_empty() && n > 0)
            || n > base * 4
            || (self.al_hacerla > MINIMO_REJILLA && n < self.al_hacerla / 4)
            || self.fuera > (n / 8).max(1024)
        {
            self.rehacer_rejilla();
        } else if self.caja_vieja {
            self.recalcular_caja();
        }
    }

    fn recalcular_caja(&mut self) {
        self.caja = self
            .entradas
            .iter()
            .fold(geo::CAJA_VACIA, |c, e| geo::unir_cajas(c, e.caja));
        self.caja_vieja = false;
    }

    /// Reparte otra vez todas las entradas en una rejilla nueva a su medida.
    fn rehacer_rejilla(&mut self) {
        self.recalcular_caja();
        let total = self.caja;
        self.celdas = Vec::new();
        self.columnas = 0;
        self.filas = 0;
        self.fuera = 0;
        self.al_hacerla = self.entradas.len();
        self.marco = total;
        if self.entradas.is_empty() || !geo::caja_valida(&total) {
            return;
        }
        // Unas 4 entidades por celda, con un tope de celdas.
        let n = self.entradas.len().clamp(1, 1 << 20) as f64;
        let (w, h) = (
            (total[2] - total[0]).max(1e-6),
            (total[3] - total[1]).max(1e-6),
        );
        let lado = ((w * h) / (n / 4.0)).sqrt().max(w.max(h) / 1024.0);
        self.lado = lado;
        self.columnas = ((w / lado).ceil() as usize).clamp(1, 1024);
        self.filas = ((h / lado).ceil() as usize).clamp(1, 1024);
        let mut celdas = vec![Vec::new(); self.columnas * self.filas];
        for (k, e) in self.entradas.iter().enumerate() {
            let (c0, f0, c1, f1) = self.rango(&e.caja);
            for f in f0..=f1 {
                for c in c0..=c1 {
                    celdas[f * self.columnas + c].push(k as u32);
                }
            }
        }
        self.celdas = celdas;
    }

    /// Mete la entrada `k` en sus celdas.
    fn meter_en_celdas(&mut self, k: usize) {
        if self.celdas.is_empty() {
            return;
        }
        let caja = self.entradas[k].caja;
        if !geo::contiene(&self.marco, &caja) {
            self.fuera += 1;
        }
        let (c0, f0, c1, f1) = self.rango(&caja);
        for f in f0..=f1 {
            for c in c0..=c1 {
                self.celdas[f * self.columnas + c].push(k as u32);
            }
        }
    }

    /// En las celdas de la entrada `k`, cambia `k` por `nuevo` (o lo quita si
    /// `None`).
    fn cambiar_en_celdas(&mut self, k: usize, nuevo: Option<u32>) {
        if self.celdas.is_empty() {
            return;
        }
        let caja = self.entradas[k].caja;
        if nuevo.is_none() && !geo::contiene(&self.marco, &caja) {
            self.fuera = self.fuera.saturating_sub(1);
        }
        let (c0, f0, c1, f1) = self.rango(&caja);
        for f in f0..=f1 {
            for c in c0..=c1 {
                let celda = &mut self.celdas[f * self.columnas + c];
                if let Some(i) = celda.iter().position(|x| *x as usize == k) {
                    match nuevo {
                        Some(n) => celda[i] = n,
                        None => {
                            celda.swap_remove(i);
                        }
                    }
                }
            }
        }
    }

    /// Anade una entrada nueva (su handle no puede estar ya).
    fn poner(&mut self, e: Entrada) {
        let k = self.entradas.len();
        self.caja = geo::unir_cajas(self.caja, e.caja);
        self.lugar.insert(e.handle, k as u32);
        self.entradas.push(e);
        self.meter_en_celdas(k);
    }

    /// Quita la entrada del handle `h`; dice si estaba.
    fn quitar(&mut self, h: Handle) -> bool {
        let Some(k) = self.lugar.remove(&h) else {
            return false;
        };
        let k = k as usize;
        let c = self.entradas[k].caja;
        let t = self.caja;
        if c[0] <= t[0] || c[1] <= t[1] || c[2] >= t[2] || c[3] >= t[3] {
            self.caja_vieja = true;
        }
        self.cambiar_en_celdas(k, None);
        // La ultima pasa al hueco.
        let ultima = self.entradas.len() - 1;
        if k != ultima {
            self.cambiar_en_celdas(ultima, Some(k as u32));
            self.lugar.insert(self.entradas[ultima].handle, k as u32);
        }
        self.entradas.swap_remove(k);
        true
    }

    fn rango(&self, c: &Caja) -> (usize, usize, usize, usize) {
        let col = |x: f64| {
            (((x - self.marco[0]) / self.lado).floor().max(0.0) as usize).min(self.columnas - 1)
        };
        let fil = |y: f64| {
            (((y - self.marco[1]) / self.lado).floor().max(0.0) as usize).min(self.filas - 1)
        };
        (col(c[0]), fil(c[1]), col(c[2]), fil(c[3]))
    }

    /// **Recorre las entradas cuya caja toca `zona`**, cada una una vez y
    /// sin pedir memoria: solo las celdas de la zona.
    pub fn en_cada<'a>(&'a self, zona: &Caja, mut f: impl FnMut(&'a Entrada)) {
        if !geo::se_tocan(&self.caja, zona) {
            return;
        }
        let de_corrido = |f: &mut dyn FnMut(&'a Entrada)| {
            for e in &self.entradas {
                if geo::se_tocan(&e.caja, zona) {
                    f(e);
                }
            }
        };
        if self.celdas.is_empty() {
            return de_corrido(&mut f);
        }
        let (c0, f0, c1, f1) = self.rango(zona);
        // Una zona de media rejilla o mas: mejor de corrido.
        if (c1 - c0 + 1) * (f1 - f0 + 1) * 2 >= self.celdas.len() {
            return de_corrido(&mut f);
        }
        for fi in f0..=f1 {
            for co in c0..=c1 {
                for &k in &self.celdas[fi * self.columnas + co] {
                    let e = &self.entradas[k as usize];
                    if !geo::se_tocan(&e.caja, zona) {
                        continue;
                    }
                    // Sin repetir: cada entrada sale solo en la primera celda
                    // que comparte con la zona.
                    let (ec0, ef0, _, _) = self.rango(&e.caja);
                    if co == ec0.max(c0) && fi == ef0.max(f0) {
                        f(e);
                    }
                }
            }
        }
    }

    /// Las entradas cuya caja toca `zona`.
    pub fn en(&self, zona: &Caja) -> Vec<&Entrada> {
        let mut v = Vec::new();
        self.en_cada(zona, |e| v.push(e));
        v
    }

    /// La entrada de un handle (al instante).
    pub fn de_handle(&self, h: Handle) -> Option<&Entrada> {
        self.lugar.get(&h).map(|k| &self.entradas[*k as usize])
    }

    /// Cuantas entradas hay.
    pub fn len(&self) -> usize {
        self.entradas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entradas.is_empty()
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
    /// Que cambio en cada version (`None`: algo que no es de entidades, como
    /// una capa, que obliga a mirarlo todo). Las ultimas [`BITACORA`].
    bitacora: std::collections::VecDeque<(u64, Option<Vec<Handle>>)>,
}

/// Cuantas versiones recuerda [`Dibujo::cambios_desde`].
const BITACORA: usize = 256;

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
            bitacora: std::collections::VecDeque::new(),
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
        self.apuntar_version(None);
    }

    pub fn entidad(&self, h: Handle) -> Option<&EntityType> {
        self.doc.get_entity(h)
    }

    /// **El indice de ahora**: si el dibujo cambio, se pone al dia solo con
    /// lo que cambio ([`Dibujo::cambios_desde`]); si no se sabe que cambio
    /// (o cambio casi todo), se rehace entero.
    pub fn indice(&self) -> std::rc::Rc<Indice> {
        let mut i = self.indice.borrow_mut();
        if let Some((v, ind)) = i.as_ref()
            && *v == self.version
        {
            return ind.clone();
        }
        let ind = match i.take() {
            Some((v, mut ind)) => match self.cambios_desde(v) {
                Some(hs) if hs.len() <= ind.len() / 2 => {
                    // Si alguien aun tiene el viejo, se copia (sin volver a
                    // sacar las formas, que es lo caro).
                    std::rc::Rc::make_mut(&mut ind).actualizar(&self.doc, &hs);
                    ind
                }
                _ => std::rc::Rc::new(Indice::completo(&self.doc)),
            },
            None => std::rc::Rc::new(Indice::completo(&self.doc)),
        };
        *i = Some((self.version, ind.clone()));
        ind
    }

    /// **Hace una orden de una vez**: lo que cambie dentro se deshace junto.
    /// Si no cambio nada, no deja paso.
    pub fn editar<R>(&mut self, nombre: &str, f: impl FnOnce(&mut Transaccion) -> R) -> R {
        let mut t = Transaccion {
            doc: &mut self.doc,
            cambios: Vec::new(),
            apuntados: std::collections::HashSet::new(),
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
            let hs = self.hecho.last().map(Paso::handles);
            self.apuntar_version(hs);
        }
        r
    }

    fn apuntar_version(&mut self, hs: Option<Vec<Handle>>) {
        self.bitacora.push_back((self.version, hs));
        while self.bitacora.len() > BITACORA {
            self.bitacora.pop_front();
        }
    }

    /// **Que entidades cambiaron** (anadidas, cambiadas o quitadas) desde la
    /// version `v`, sin repetir. `None` si no se sabe (paso algo que no es de
    /// entidades, como apagar una capa, o `v` es demasiado vieja): entonces
    /// hay que mirarlo todo.
    pub fn cambios_desde(&self, v: u64) -> Option<Vec<Handle>> {
        if v == self.version {
            return Some(Vec::new());
        }
        if v > self.version {
            return None;
        }
        // La bitacora tiene que llegar hasta la version siguiente a `v`.
        let primera = self.bitacora.front()?.0;
        if primera > v + 1 {
            return None;
        }
        let mut vistos = std::collections::HashSet::new();
        let mut out = Vec::new();
        for (ver, hs) in &self.bitacora {
            if *ver <= v {
                continue;
            }
            for h in hs.as_ref()? {
                if vistos.insert(*h) {
                    out.push(*h);
                }
            }
        }
        Some(out)
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
        let hs = paso.handles();
        self.deshecho.push(paso);
        self.version += 1;
        self.apuntar_version(Some(hs));
        Some(nombre)
    }

    pub fn rehacer(&mut self) -> Option<String> {
        let paso = self.deshecho.pop()?;
        for c in &paso.cambios {
            poner(&mut self.doc, c.handle, c.despues.clone());
        }
        let nombre = paso.nombre.clone();
        let hs = paso.handles();
        self.hecho.push(paso);
        self.version += 1;
        self.apuntar_version(Some(hs));
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
    /// Los handles ya apuntados en `cambios`: mirarlo en la lista era
    /// cuadratico (mover o borrar 200 000 objetos tardaba 15 a 20 s).
    apuntados: std::collections::HashSet<Handle>,
}

impl Transaccion<'_> {
    fn apuntar(&mut self, h: Handle) {
        if self.apuntados.insert(h) {
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
        self.apuntados.insert(h);
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
        let partes = match &e {
            // Un bloque se desmonta en lo que tiene su definicion, ya puesto
            // en su sitio, y queda donde estaba el bloque (no en la definicion).
            EntityType::Insert(i) => {
                let mut v = crate::bloque::descomponer(self.doc, i);
                for p in &mut v {
                    p.common_mut().owner_handle = e.common().owner_handle;
                }
                v
            }
            _ => self.doc.explode_entity(&e),
        };
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
    fn cambios_desde_una_version() {
        let mut d = Dibujo::nuevo();
        let v0 = d.version();
        let a = d
            .editar("A", |t| t.anadir(linea(0.0, 0.0, 1.0, 0.0)))
            .unwrap();
        let v1 = d.version();
        let b = d
            .editar("B", |t| t.anadir(linea(0.0, 1.0, 1.0, 1.0)))
            .unwrap();
        assert_eq!(d.cambios_desde(v1), Some(vec![b]));
        assert_eq!(d.cambios_desde(v0), Some(vec![a, b]));
        assert_eq!(d.cambios_desde(d.version()), Some(vec![]));
        d.deshacer();
        assert_eq!(d.cambios_desde(v1), Some(vec![b]));
        // Algo que no es de entidades: no se sabe.
        d.con_capa("0", |l| l.turn_off());
        assert_eq!(d.cambios_desde(v1), None);
        // Caso negativo: una version que aun no existe.
        assert_eq!(d.cambios_desde(d.version() + 5), None);
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

    /// Un generador pseudoaleatorio sencillo (siempre la misma serie).
    struct Azar(u64);

    impl Azar {
        fn sig(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }

        fn hasta(&mut self, n: usize) -> usize {
            ((self.sig() * n as f64) as usize).min(n.saturating_sub(1))
        }
    }

    /// Comprueba que el indice al dia de `d` es el mismo que uno hecho de
    /// cero: mismas entradas, misma caja y lo mismo en cada zona.
    fn igual_que_de_cero(d: &Dibujo, az: &mut Azar) {
        let ind = d.indice();
        let cero = Indice::completo(&d.doc);
        assert_eq!(ind.len(), cero.len());
        for e in &cero.entradas {
            let o = ind.de_handle(e.handle).expect("la entrada esta");
            assert_eq!(o.handle, e.handle);
            assert_eq!(o.caja, e.caja);
            assert_eq!(o.formas, e.formas);
            assert_eq!(o.bloqueada, e.bloqueada);
        }
        assert_eq!(ind.caja(), cero.caja());
        let Some(k) = cero.caja() else { return };
        for _ in 0..40 {
            let x = k[0] - 50.0 + az.sig() * (k[2] - k[0] + 100.0);
            let y = k[1] - 50.0 + az.sig() * (k[3] - k[1] + 100.0);
            let r = az.sig() * 80.0;
            let zona = [x - r, y - r, x + r, y + r];
            let mut a: Vec<_> = ind.en(&zona).iter().map(|e| e.handle).collect();
            let mut b: Vec<_> = cero.en(&zona).iter().map(|e| e.handle).collect();
            let n = a.len();
            a.sort();
            a.dedup();
            assert_eq!(a.len(), n, "una entrada salio dos veces");
            b.sort();
            assert_eq!(a, b, "en la zona {zona:?}");
        }
    }

    #[test]
    fn el_indice_al_dia_es_igual_que_rehecho_tras_ediciones_al_azar() {
        let mut d = Dibujo::nuevo();
        let mut az = Azar(3);
        let mut vivas: Vec<Handle> = Vec::new();
        // Empieza vacio: la rejilla se hace al crecer.
        for ronda in 0..300 {
            let tirada = az.hasta(10);
            match tirada {
                0..=3 => {
                    let n = 1 + az.hasta(40);
                    let mut nuevas = Vec::new();
                    d.editar("ANADIR", |t| {
                        for _ in 0..n {
                            let (x, y) = (az.sig() * 1000.0, az.sig() * 1000.0);
                            let e = if az.sig() < 0.8 {
                                linea(x, y, x + az.sig() * 50.0, y + az.sig() * 50.0)
                            } else {
                                crate::especialidad::circulo([x, y], 1.0 + az.sig() * 20.0)
                            };
                            nuevas.extend(t.anadir(e));
                        }
                    });
                    vivas.extend(nuevas);
                }
                4 | 5 if !vivas.is_empty() => {
                    let h = vivas[az.hasta(vivas.len())];
                    // A veces muy lejos: se sale de la rejilla.
                    let lejos = if az.sig() < 0.2 { 5000.0 } else { 10.0 };
                    let (dx, dy) = (az.sig() * lejos, -az.sig() * lejos);
                    d.editar("MOVER", |t| {
                        t.cambiar(h, |e| e.translate(opencadcodec::Vector3::new(dx, dy, 0.0)))
                    });
                }
                6 if !vivas.is_empty() => {
                    let i = az.hasta(vivas.len());
                    let h = vivas.swap_remove(i);
                    d.editar("BORRAR", |t| t.quitar(h));
                }
                7 => {
                    d.deshacer();
                }
                8 => {
                    d.rehacer();
                }
                _ => {
                    // Se sigue usando el indice viejo mientras cambia el dibujo.
                    let viejo = d.indice();
                    if let Some(h) = vivas.first().copied() {
                        d.editar("MOVER", |t| {
                            t.cambiar(h, |e| {
                                e.translate(opencadcodec::Vector3::new(1.0, 1.0, 0.0))
                            })
                        });
                    }
                    let _ = d.indice();
                    drop(viejo);
                }
            }
            // Lo deshecho o rehecho cambia que handles viven.
            vivas = d.indice().entradas.iter().map(|e| e.handle).collect();
            vivas.sort();
            if ronda % 7 == 0 {
                igual_que_de_cero(&d, &mut az);
            }
        }
        igual_que_de_cero(&d, &mut az);
        assert!(d.indice().len() > 100);
        // Apagar una capa no se sabe por handles: se rehace entero.
        d.con_capa("0", |l| l.turn_off());
        assert_eq!(d.indice().len(), 0);
        igual_que_de_cero(&d, &mut az);
    }

    #[test]
    fn el_indice_encoge_su_caja_al_borrar_lo_del_borde() {
        let mut d = Dibujo::nuevo();
        let (a, b) = d.editar("LINEAS", |t| {
            (
                t.anadir(linea(0.0, 0.0, 10.0, 0.0)).unwrap(),
                t.anadir(linea(0.0, 5.0, 1000.0, 5.0)).unwrap(),
            )
        });
        assert_eq!(d.indice().caja(), Some([0.0, 0.0, 1000.0, 5.0]));
        d.editar("BORRAR", |t| t.quitar(b));
        assert_eq!(d.indice().caja(), Some([0.0, 0.0, 10.0, 0.0]));
        assert!(d.indice().de_handle(b).is_none());
        assert!(d.indice().de_handle(a).is_some());
        // Caso negativo: un handle que nunca existio.
        assert!(d.indice().de_handle(Handle::new(999_999)).is_none());
        d.editar("BORRAR", |t| t.quitar(a));
        assert_eq!(d.indice().caja(), None);
        assert!(d.indice().en(&[-1e9, -1e9, 1e9, 1e9]).is_empty());
    }

    #[test]
    fn guarda_y_vuelve_a_abrir_un_dwg_con_lo_editado() {
        let dir = std::env::temp_dir().join(format!("cad-nucleo-dwg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("prueba.dwg");
        let mut d = Dibujo::nuevo();
        let h = d
            .editar("LINEA", |t| {
                t.anadir_en(linea(0.0, 0.0, 3.0, 4.0), "A-MUROS")
            })
            .unwrap();
        d.editar("MOVER", |t| {
            t.cambiar(h, |e| {
                e.translate(opencadcodec::Vector3::new(10.0, 0.0, 0.0))
            })
        });
        d.guardar_como(&ruta).unwrap();
        let d2 = Dibujo::abrir(&ruta).unwrap();
        let l = d2
            .doc
            .model_space_entities()
            .find_map(|e| match e {
                EntityType::Line(l) => Some(l.clone()),
                _ => None,
            })
            .expect("la linea vuelve");
        assert_eq!(l.start.x, 10.0);
        assert_eq!(l.common.layer, "A-MUROS");
        // Y se puede seguir editando lo abierto.
        let mut d2 = d2;
        let hl = l.common.handle;
        assert!(d2.editar("BORRAR", |t| t.quitar(hl)));
        assert_eq!(d2.doc.model_space_entities().count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
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
