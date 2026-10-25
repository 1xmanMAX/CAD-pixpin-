//! **Los datos de un IFC** (2x3, 4, 4.3) con el lector STEP de `ifc-lite`:
//! la estructura espacial (Proyecto ▸ Sitio ▸ Edificio ▸ Nivel ▸ Espacio),
//! cada elemento con sus Psets, Qtos (con su unidad), materiales y capas,
//! clasificacion y tipo, y la caja de su geometria.

use std::collections::{HashMap, HashSet};

use ifc_lite_core::{AttributeValue, DecodedEntity, EntityDecoder, EntityScanner, ProjectUnits};

use crate::modelo::*;

/// Hasta donde se siguen referencias (materiales anidados, piezas de piezas).
const HONDO: usize = 8;

/// **Lee un IFC.** `cajas`: tambien la caja de cada elemento (pide su
/// geometria: tarda mas). `avance` recibe lo que se va haciendo.
pub fn leer(bytes: &[u8], cajas: bool, avance: &mut dyn FnMut(&str)) -> Result<ModeloBim, String> {
    if !es_step(bytes) {
        return Err("no es un IFC (falta la cabecera ISO-10303-21)".into());
    }
    let mut m = ModeloBim::default();
    m.archivo.formato = Formato::Ifc;
    m.archivo.bytes = bytes.len() as u64;
    let esquema = ifc_lite_core::declared_schema_bounded(bytes).unwrap_or_default();
    cabecera(bytes, &esquema, &mut m.archivo);

    avance("Leyendo las entidades del IFC…");
    let mut tipos: HashMap<u32, String> = HashMap::new();
    let mut por_tipo: HashMap<String, Vec<u32>> = HashMap::new();
    let mut sc = EntityScanner::new(bytes);
    while let Some((id, kw, _, _)) = sc.next_entity() {
        let t = kw.trim().to_ascii_uppercase();
        por_tipo.entry(t.clone()).or_default().push(id);
        tipos.insert(id, t);
    }
    if tipos.is_empty() {
        m.avisos.push("El IFC no tiene ninguna entidad.".into());
        return Ok(m);
    }
    let esq = if ifc_lite_core::attribute_names_for_schema(&esquema, "IFCWALL").is_some() {
        esquema.clone()
    } else {
        m.avisos.push(format!(
            "Esquema «{esquema}» desconocido: se lee como IFC4."
        ));
        "IFC4".to_string()
    };
    let mut l = Lector {
        dec: EntityDecoder::new(bytes),
        tipos,
        esquema: esq,
        unidades: ProjectUnits::default(),
        conjuntos: HashMap::new(),
        escala: 1.0,
    };
    let proyecto = por_tipo.get("IFCPROJECT").and_then(|v| v.first().copied());
    if let Some(p) = proyecto {
        l.unidades = ProjectUnits::resolve(&mut l.dec, p);
    }
    l.escala = l
        .unidades
        .resolved_for_unit_type("LENGTHUNIT")
        .map(|u| u.si_scale)
        .unwrap_or(1.0);

    avance("Leyendo las relaciones…");
    let r = Relaciones::leer(&mut l, &por_tipo);

    // La estructura espacial, desde el proyecto, por `IfcRelAggregates`.
    avance("Leyendo la estructura espacial…");
    let mut indice_espacial: HashMap<u32, usize> = HashMap::new();
    let mut pila: Vec<(u32, Option<usize>)> = proyecto.map(|p| vec![(p, None)]).unwrap_or_default();
    let mut vistos: HashSet<u32> = HashSet::new();
    while let Some((id, padre)) = pila.pop() {
        if !vistos.insert(id) {
            continue;
        }
        let Some(e) = l.ent(id) else { continue };
        let clase = clase_canonica(&l.tipo(id));
        let nombre = l
            .texto(&e, 2)
            .or_else(|| l.texto(&e, 7))
            .unwrap_or_else(|| clase.clone());
        let k = m.espacial.len();
        m.espacial.push(NodoEspacial {
            clase,
            nombre,
            global_id: l.texto(&e, 0),
            padre,
        });
        indice_espacial.insert(id, k);
        if let Some(hijos) = r.partes.get(&id) {
            for h in hijos.iter().rev() {
                if l.es_espacial(*h) {
                    pila.push((*h, Some(k)));
                }
            }
        }
    }

    // Los niveles.
    let mut nivel_de_planta: HashMap<u32, usize> = HashMap::new();
    let mut plantas: Vec<(u32, Nivel)> = Vec::new();
    for &id in por_tipo
        .get("IFCBUILDINGSTOREY")
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        let Some(e) = l.ent(id) else { continue };
        let edificio = r
            .padre
            .get(&id)
            .filter(|p| l.tipo(**p) == "IFCBUILDING")
            .and_then(|p| l.ent(*p))
            .and_then(|b| l.texto(&b, 2));
        plantas.push((
            id,
            Nivel {
                nombre: l.texto(&e, 2).unwrap_or_else(|| format!("Nivel #{id}")),
                cota: e.get(9).and_then(numero).map(|c| c * l.escala),
                global_id: l.texto(&e, 0),
                edificio,
                elementos: 0,
            },
        ));
    }
    plantas.sort_by(|a, b| {
        a.1.cota
            .unwrap_or(0.0)
            .partial_cmp(&b.1.cota.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (id, n) in plantas {
        nivel_de_planta.insert(id, m.niveles.len());
        m.niveles.push(n);
    }

    // El proyecto.
    if let Some(p) = proyecto.and_then(|p| l.ent(p)) {
        let nombre = l.texto(&p, 2);
        let largo = l.texto(&p, 5);
        m.proyecto.nombre = largo.clone().or(nombre.clone());
        m.proyecto.numero = largo.and(nombre);
        m.proyecto.descripcion = l.texto(&p, 3);
        m.proyecto.estado = l.texto(&p, 6);
        for c in l.conjuntos_de(p.id, &r) {
            m.proyecto.grupos.push(c);
        }
    }
    if let Some(s) = por_tipo.get("IFCSITE").and_then(|v| v.first()) {
        if let Some(e) = l.ent(*s) {
            m.proyecto.sitio = l.texto(&e, 2);
            if m.proyecto.direccion.is_none() {
                m.proyecto.direccion = e.get_ref(14).and_then(|d| l.direccion(d));
            }
        }
    }
    if let Some(b) = por_tipo.get("IFCBUILDING").and_then(|v| v.first()) {
        if let Some(e) = l.ent(*b) {
            m.proyecto.edificio = l.texto(&e, 2).or_else(|| l.texto(&e, 7));
            if let Some(d) = e.get_ref(11).and_then(|d| l.direccion(d)) {
                m.proyecto.direccion = Some(d);
            }
            for c in l.conjuntos_de(*b, &r) {
                m.proyecto.grupos.push(c);
            }
        }
    }
    for g in &m.proyecto.grupos {
        for p in &g.propiedades {
            let n = p.nombre.to_ascii_lowercase();
            if m.proyecto.cliente.is_none() && n.contains("client") {
                m.proyecto.cliente = Some(p.valor.texto());
            }
            if m.proyecto.fecha_emision.is_none() && n.contains("issue") && n.contains("date") {
                m.proyecto.fecha_emision = Some(p.valor.texto());
            }
            if m.proyecto.organizacion.is_none() && n.contains("organization") {
                m.proyecto.organizacion = Some(p.valor.texto());
            }
        }
    }
    if m.proyecto.organizacion.is_none() {
        m.proyecto.organizacion = m.archivo.organizacion.clone();
    }

    // Los tipos.
    avance("Leyendo los tipos…");
    let mut indice_tipo: HashMap<u32, usize> = HashMap::new();
    let mut ids_tipo: Vec<u32> = r.tipo_de.values().copied().collect();
    ids_tipo.sort_unstable();
    ids_tipo.dedup();
    for t in ids_tipo {
        let Some(e) = l.ent(t) else { continue };
        let nombre = l.texto(&e, 2).unwrap_or_default();
        let (familia, tipo) = partir_familia(&nombre);
        let mut conjuntos = Vec::new();
        if let Some(AttributeValue::List(v)) = e.get(5) {
            for id in v.iter().filter_map(AttributeValue::as_entity_ref) {
                if let Some(mut c) = l.conjunto(id) {
                    c.tipo = if c.tipo == TipoConjunto::Cantidades {
                        TipoConjunto::Cantidades
                    } else {
                        TipoConjunto::Tipo
                    };
                    conjuntos.push(c);
                }
            }
        }
        for c in l.conjuntos_de(t, &r) {
            conjuntos.push(c);
        }
        indice_tipo.insert(t, m.tipos.len());
        m.tipos.push(TipoBim {
            id: format!("#{t}"),
            clase: clase_canonica(&l.tipo(t)),
            familia,
            nombre: tipo,
            elementos: 0,
            conjuntos,
        });
    }

    // Los materiales con nombre.
    let mut nombres_material: Vec<String> = Vec::new();
    for &id in por_tipo
        .get("IFCMATERIAL")
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        if let Some(n) = l.ent(id).and_then(|e| l.texto(&e, 0)) {
            if !nombres_material.contains(&n) {
                nombres_material.push(n);
            }
        }
    }
    m.materiales = nombres_material
        .into_iter()
        .map(|nombre| Material {
            nombre,
            ..Default::default()
        })
        .collect();

    // Los elementos.
    avance("Leyendo los elementos y sus propiedades…");
    let mut ids: Vec<u32> = Vec::new();
    for (t, v) in &por_tipo {
        if l.es_elemento(t) {
            ids.extend(v);
        }
    }
    ids.sort_unstable();
    let total = ids.len();
    let mut indice_elemento: HashMap<u32, usize> = HashMap::new();
    for (k, id) in ids.into_iter().enumerate() {
        if k % 2000 == 0 && k > 0 {
            avance(&format!("Leyendo elementos… {k} de {total}"));
        }
        let Some(e) = l.ent(id) else { continue };
        let clase = clase_canonica(&l.tipo(id));
        let nombre = l.texto(&e, 2).unwrap_or_default();
        let objeto = l.texto(&e, 4);
        let tipo_id = r.tipo_de.get(&id).copied();
        let tipo_nombre = tipo_id
            .and_then(|t| indice_tipo.get(&t))
            .map(|&i| (m.tipos[i].familia.clone(), m.tipos[i].nombre.clone()));
        let (familia, tipo) = match (&objeto, &tipo_nombre) {
            (Some(o), _) if o.contains(':') => partir_familia(o),
            (_, Some((f, t))) => (f.clone(), t.clone()),
            (Some(o), None) => (String::new(), o.clone()),
            (None, None) => (String::new(), String::new()),
        };
        let mut atributos = Conjunto::nuevo("Atributos IFC", TipoConjunto::Atributos);
        atributos.texto("Clase", clase.clone());
        atributos.texto_opcional("GlobalId", l.texto(&e, 0).as_deref());
        atributos.texto("Name", nombre.clone());
        atributos.texto_opcional("Description", l.texto(&e, 3).as_deref());
        atributos.texto_opcional("ObjectType", objeto.as_deref());
        if l.es_subtipo(&l.tipo(id), "IFCELEMENT") {
            atributos.texto_opcional("Tag", l.texto(&e, 7).as_deref());
        } else {
            atributos.texto_opcional("LongName", l.texto(&e, 7).as_deref());
        }
        if let Some(AttributeValue::Enum(p)) = e.attributes.last() {
            atributos.texto("PredefinedType", p.clone());
        }
        if let Some(t) = tipo_id {
            atributos.texto_opcional("Tipo", l.ent(t).and_then(|t| l.texto(&t, 2)).as_deref());
        }
        let mut conjuntos = vec![atributos];
        conjuntos.extend(l.conjuntos_de(id, &r));
        // Lo del tipo que el elemento no trae ya.
        if let Some(&i) = tipo_id.and_then(|t| indice_tipo.get(&t)) {
            for c in &m.tipos[i].conjuntos {
                if !conjuntos.iter().any(|x| x.nombre == c.nombre) {
                    conjuntos.push(c.clone());
                }
            }
        }
        // Materiales: del elemento, o de su tipo.
        let mat_id = r
            .material_de
            .get(&id)
            .or_else(|| tipo_id.and_then(|t| r.material_de.get(&t)))
            .copied();
        let mut materiales = Vec::new();
        if let Some(mid) = mat_id {
            let mut capas = Vec::new();
            l.materiales(mid, 0, &mut materiales, &mut capas);
            if !capas.is_empty() {
                let mut c = Conjunto::nuevo("Capas de material", TipoConjunto::Material);
                for (k, (n, espesor)) in capas.iter().enumerate() {
                    c.medida(&format!("{}. {n}", k + 1), *espesor, "m");
                }
                c.medida("Espesor total", capas.iter().map(|c| c.1).sum(), "m");
                conjuntos.push(c);
            }
        }
        // Clasificacion.
        let mut clas = Conjunto::nuevo("Clasificacion", TipoConjunto::Clasificacion);
        for cid in r
            .clasificacion_de
            .get(&id)
            .into_iter()
            .chain(tipo_id.and_then(|t| r.clasificacion_de.get(&t)))
            .flatten()
        {
            if let Some((fuente, codigo)) = l.clasificacion(*cid) {
                clas.texto(&fuente, codigo);
            }
        }
        if !clas.propiedades.is_empty() {
            conjuntos.push(clas);
        }
        let nivel = l
            .planta_de(id, &r, 0)
            .and_then(|p| nivel_de_planta.get(&p).copied());
        indice_elemento.insert(id, m.elementos.len());
        m.elementos.push(ElementoBim2 {
            id: format!("#{id}"),
            global_id: l.texto(&e, 0),
            categoria: categoria_de_clase(&clase),
            clase,
            familia,
            tipo,
            nombre,
            nivel,
            materiales,
            caja: None,
            conjuntos,
        });
    }
    for t in &mut m.tipos {
        t.elementos = 0;
    }
    for (e, t) in &r.tipo_de {
        if indice_elemento.contains_key(e) {
            if let Some(&i) = indice_tipo.get(t) {
                m.tipos[i].elementos += 1;
            }
        }
    }

    // Los espacios (habitaciones).
    for (i, e) in m.elementos.iter().enumerate() {
        if e.clase != "IfcSpace" {
            continue;
        }
        let num = |n: &str| e.propiedad(None, n).and_then(Propiedad::numero_si);
        let area = num("NetFloorArea")
            .or_else(|| num("GrossFloorArea"))
            .or_else(|| num("Area"));
        let volumen = num("NetVolume")
            .or_else(|| num("GrossVolume"))
            .or_else(|| num("Volume"));
        m.espacios.push(Espacio {
            elemento: i,
            numero: e.nombre.clone(),
            nombre: e
                .propiedad(Some("Atributos IFC"), "LongName")
                .map(|p| p.valor.texto())
                .unwrap_or_default(),
            nivel: e.nivel,
            area,
            volumen,
        });
    }

    if cajas && !m.elementos.is_empty() {
        avance("Midiendo la geometria de cada elemento…");
        match cajas_de(bytes) {
            Ok(c) => {
                for (id, caja) in c {
                    if let Some(&i) = indice_elemento.get(&id) {
                        m.elementos[i].caja = Some(caja);
                    }
                }
            }
            Err(e) => m.avisos.push(format!("No se pudo medir la geometria: {e}")),
        }
    }
    m.recontar();
    Ok(m)
}

/// Si empieza como un fichero STEP.
pub fn es_step(bytes: &[u8]) -> bool {
    let b = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let i = b
        .iter()
        .position(|c| !c.is_ascii_whitespace())
        .unwrap_or(b.len());
    b[i..].starts_with(b"ISO-10303-21")
}

/// Las relaciones del modelo, de una pasada.
#[derive(Default)]
struct Relaciones {
    /// De un todo a sus partes (`IfcRelAggregates`, `IfcRelNests`).
    partes: HashMap<u32, Vec<u32>>,
    /// De una parte a su todo.
    padre: HashMap<u32, u32>,
    /// De un elemento a la estructura que lo contiene.
    contenido_en: HashMap<u32, u32>,
    psets: HashMap<u32, Vec<u32>>,
    tipo_de: HashMap<u32, u32>,
    material_de: HashMap<u32, u32>,
    clasificacion_de: HashMap<u32, Vec<u32>>,
}

impl Relaciones {
    fn leer(l: &mut Lector, por_tipo: &HashMap<String, Vec<u32>>) -> Relaciones {
        let mut r = Relaciones::default();
        let ids = |t: &str| por_tipo.get(t).cloned().unwrap_or_default();
        for t in ["IFCRELAGGREGATES", "IFCRELNESTS"] {
            for id in ids(t) {
                let Some(e) = l.ent(id) else { continue };
                let Some(todo) = e.get_ref(4) else { continue };
                for p in refs(e.get(5)) {
                    r.partes.entry(todo).or_default().push(p);
                    r.padre.entry(p).or_insert(todo);
                }
            }
        }
        for id in ids("IFCRELCONTAINEDINSPATIALSTRUCTURE") {
            let Some(e) = l.ent(id) else { continue };
            let Some(s) = e.get_ref(5) else { continue };
            for p in refs(e.get(4)) {
                r.contenido_en.entry(p).or_insert(s);
            }
        }
        for id in ids("IFCRELDEFINESBYPROPERTIES") {
            let Some(e) = l.ent(id) else { continue };
            let Some(s) = e.get_ref(5) else { continue };
            for p in refs(e.get(4)) {
                r.psets.entry(p).or_default().push(s);
            }
        }
        for id in ids("IFCRELDEFINESBYTYPE") {
            let Some(e) = l.ent(id) else { continue };
            let Some(s) = e.get_ref(5) else { continue };
            for p in refs(e.get(4)) {
                r.tipo_de.entry(p).or_insert(s);
            }
        }
        for id in ids("IFCRELASSOCIATESMATERIAL") {
            let Some(e) = l.ent(id) else { continue };
            let Some(s) = e.get_ref(5) else { continue };
            for p in refs(e.get(4)) {
                r.material_de.entry(p).or_insert(s);
            }
        }
        for id in ids("IFCRELASSOCIATESCLASSIFICATION") {
            let Some(e) = l.ent(id) else { continue };
            let Some(s) = e.get_ref(5) else { continue };
            for p in refs(e.get(4)) {
                r.clasificacion_de.entry(p).or_default().push(s);
            }
        }
        r
    }
}

/// Las referencias de un atributo (una sola o una lista).
fn refs(a: Option<&AttributeValue>) -> Vec<u32> {
    match a {
        Some(AttributeValue::EntityRef(id)) => vec![*id],
        Some(AttributeValue::List(v)) => {
            v.iter().filter_map(AttributeValue::as_entity_ref).collect()
        }
        _ => Vec::new(),
    }
}

fn numero(a: &AttributeValue) -> Option<f64> {
    match a {
        AttributeValue::Float(v) => Some(*v),
        AttributeValue::Integer(v) => Some(*v as f64),
        AttributeValue::List(v) if v.len() == 2 => numero(&v[1]),
        _ => None,
    }
}

/// `Familia:Tipo` (como exporta Revit) en sus dos partes; sin `:`, todo es el tipo.
fn partir_familia(s: &str) -> (String, String) {
    match s.split_once(':') {
        Some((f, t)) => (f.trim().to_string(), t.trim().to_string()),
        None => (String::new(), s.trim().to_string()),
    }
}

struct Lector<'a> {
    dec: EntityDecoder<'a>,
    tipos: HashMap<u32, String>,
    esquema: String,
    unidades: ProjectUnits,
    /// Los conjuntos ya leidos (muchos elementos comparten los de su tipo).
    conjuntos: HashMap<u32, Option<Conjunto>>,
    /// Metros por unidad de longitud del fichero.
    escala: f64,
}

impl Lector<'_> {
    fn ent(&mut self, id: u32) -> Option<DecodedEntity> {
        self.dec.decode_by_id(id).ok()
    }

    fn tipo(&self, id: u32) -> String {
        self.tipos.get(&id).cloned().unwrap_or_default()
    }

    fn es_subtipo(&self, t: &str, padre: &str) -> bool {
        ifc_lite_core::is_subtype_of_for_schema(&self.esquema, t, padre)
    }

    fn es_espacial(&self, id: u32) -> bool {
        let t = self.tipo(id);
        t == "IFCPROJECT"
            || self.es_subtipo(&t, "IFCSPATIALSTRUCTUREELEMENT")
            || self.es_subtipo(&t, "IFCSPATIALELEMENT")
    }

    /// Lo que va en la lista de elementos: los `IfcElement` (sin huecos ni
    /// elementos virtuales) y los espacios.
    fn es_elemento(&self, t: &str) -> bool {
        if t == "IFCSPACE" {
            return true;
        }
        self.es_subtipo(t, "IFCELEMENT")
            && !self.es_subtipo(t, "IFCFEATUREELEMENTSUBTRACTION")
            && t != "IFCVIRTUALELEMENT"
    }

    /// Un texto (o una etiqueta tipada) no vacio.
    fn texto(&self, e: &DecodedEntity, i: usize) -> Option<String> {
        let s = match e.get(i)? {
            AttributeValue::String(s) => s.clone(),
            AttributeValue::List(v) if v.len() == 2 => match &v[1] {
                AttributeValue::String(s) => s.clone(),
                _ => return None,
            },
            AttributeValue::Enum(s) => s.clone(),
            _ => return None,
        };
        let s = s.trim().to_string();
        (!s.is_empty()).then_some(s)
    }

    /// La planta (su id) donde esta algo: por lo que lo contiene, o por su
    /// todo (una pieza de una escalera, un espacio en su planta).
    fn planta_de(&self, id: u32, r: &Relaciones, hondo: usize) -> Option<u32> {
        if hondo > HONDO {
            return None;
        }
        if self
            .tipos
            .get(&id)
            .is_some_and(|t| t == "IFCBUILDINGSTOREY")
        {
            return Some(id);
        }
        if let Some(s) = r.contenido_en.get(&id) {
            if let Some(p) = self.planta_de(*s, r, hondo + 1) {
                return Some(p);
            }
        }
        r.padre
            .get(&id)
            .and_then(|p| self.planta_de(*p, r, hondo + 1))
    }

    /// Los conjuntos (Psets y Qtos) de un objeto.
    fn conjuntos_de(&mut self, id: u32, r: &Relaciones) -> Vec<Conjunto> {
        let mut v = Vec::new();
        for s in r.psets.get(&id).cloned().unwrap_or_default() {
            if let Some(c) = self.conjunto(s) {
                v.push(c);
            }
        }
        v
    }

    /// Un `IfcPropertySet`, un `IfcElementQuantity` u otra definicion (las
    /// de IFC2x3 como `IfcDoorLiningProperties`: sus atributos con nombre).
    fn conjunto(&mut self, id: u32) -> Option<Conjunto> {
        if let Some(c) = self.conjuntos.get(&id) {
            return c.clone();
        }
        let c = self.conjunto_sin_cache(id);
        self.conjuntos.insert(id, c.clone());
        c
    }

    fn conjunto_sin_cache(&mut self, id: u32) -> Option<Conjunto> {
        let e = self.ent(id)?;
        let t = self.tipo(id);
        let nombre = self.texto(&e, 2).unwrap_or_else(|| clase_canonica(&t));
        match t.as_str() {
            "IFCPROPERTYSET" => {
                let mut c = Conjunto::nuevo(nombre, TipoConjunto::Propiedades);
                for p in refs(e.get(4)) {
                    self.propiedad(p, "", 0, &mut c.propiedades);
                }
                Some(c)
            }
            "IFCELEMENTQUANTITY" => {
                let mut c = Conjunto::nuevo(nombre, TipoConjunto::Cantidades);
                for q in refs(e.get(5)) {
                    self.cantidad(q, "", 0, &mut c.propiedades);
                }
                Some(c)
            }
            _ if t.starts_with("IFCREL") => None,
            _ => {
                // Una definicion con atributos propios: se listan con su nombre.
                let nombres =
                    ifc_lite_core::attribute_names_for_schema(&self.esquema, &t).unwrap_or(&[]);
                let mut c = Conjunto::nuevo(clase_canonica(&t), TipoConjunto::Propiedades);
                for (k, a) in e.attributes.iter().enumerate().skip(4) {
                    let Some(n) = nombres.get(k) else { break };
                    if let Some((v, u)) = self.valor(a) {
                        c.propiedades
                            .push(self.con_unidad(n, v, u.as_deref(), None));
                    }
                }
                (!c.propiedades.is_empty()).then_some(c)
            }
        }
    }

    /// Un valor de un atributo, con el tipo de medida si viene tipado.
    fn valor(&self, a: &AttributeValue) -> Option<(Valor, Option<String>)> {
        Some(match a {
            AttributeValue::String(s) => (Valor::Texto(s.clone()), None),
            AttributeValue::Integer(n) => (Valor::Entero(*n), None),
            AttributeValue::Float(v) => (Valor::Real(*v), None),
            AttributeValue::Enum(s) => (logico(s).unwrap_or_else(|| Valor::Texto(s.clone())), None),
            AttributeValue::List(v)
                if v.len() == 2
                    && matches!(&v[0], AttributeValue::String(s) if s.starts_with("IFC")) =>
            {
                let medida = v[0].as_string().map(str::to_string);
                let (valor, _) = self.valor(&v[1])?;
                (valor, medida)
            }
            AttributeValue::List(v) => {
                let l: Vec<Valor> = v
                    .iter()
                    .filter_map(|a| self.valor(a))
                    .map(|x| x.0)
                    .collect();
                if l.is_empty() {
                    return None;
                }
                let medida = v.iter().find_map(|a| self.valor(a).and_then(|x| x.1));
                (Valor::Lista(l), medida)
            }
            AttributeValue::EntityRef(r) => (Valor::Texto(format!("#{r}")), None),
            AttributeValue::Null | AttributeValue::Derived => return None,
        })
    }

    /// Una propiedad con su unidad: la de su `Unit` si la trae, o la del
    /// proyecto para su tipo de medida.
    fn con_unidad(
        &mut self,
        nombre: &str,
        valor: Valor,
        medida: Option<&str>,
        unidad: Option<u32>,
    ) -> Propiedad {
        let mut p = Propiedad::nueva(nombre, valor);
        if p.valor.numero().is_none() && !matches!(p.valor, Valor::Lista(_)) {
            return p;
        }
        let r = unidad
            .and_then(|u| ifc_lite_core::resolve_unit_by_ref(&mut self.dec, u).map(|x| x.1))
            .or_else(|| medida.and_then(|m| self.unidades.unit_for_measure(m)));
        if let Some(r) = r {
            if !r.symbol.is_empty() {
                p.unidad = Some(r.symbol);
            }
            p.a_si = r.si_scale;
        }
        p
    }

    fn propiedad(&mut self, id: u32, prefijo: &str, hondo: usize, out: &mut Vec<Propiedad>) {
        if hondo > HONDO {
            return;
        }
        let Some(e) = self.ent(id) else { return };
        let nombre = format!("{prefijo}{}", self.texto(&e, 0).unwrap_or_default());
        let t = self.tipo(id);
        match t.as_str() {
            "IFCPROPERTYSINGLEVALUE" => {
                let Some((v, medida)) = e.get(2).and_then(|a| self.valor(a)) else {
                    out.push(Propiedad::nueva(&nombre, Valor::Texto(String::new())));
                    return;
                };
                let p = self.con_unidad(&nombre, v, medida.as_deref(), e.get_ref(3));
                out.push(p);
            }
            "IFCPROPERTYENUMERATEDVALUE" | "IFCPROPERTYLISTVALUE" => {
                if let Some((v, medida)) = e.get(2).and_then(|a| self.valor(a)) {
                    let u = (t == "IFCPROPERTYLISTVALUE")
                        .then(|| e.get_ref(3))
                        .flatten();
                    let p = self.con_unidad(&nombre, v, medida.as_deref(), u);
                    out.push(p);
                }
            }
            "IFCPROPERTYBOUNDEDVALUE" => {
                let sup = e.get(2).and_then(|a| self.valor(a));
                let inf = e.get(3).and_then(|a| self.valor(a));
                let txt = format!(
                    "{} – {}",
                    inf.as_ref().map(|x| x.0.texto()).unwrap_or_default(),
                    sup.as_ref().map(|x| x.0.texto()).unwrap_or_default()
                );
                out.push(Propiedad::nueva(&nombre, Valor::Texto(txt)));
            }
            "IFCCOMPLEXPROPERTY" => {
                for p in refs(e.get(3)) {
                    self.propiedad(p, &format!("{nombre}."), hondo + 1, out);
                }
            }
            "IFCPROPERTYREFERENCEVALUE" => {
                let txt = e
                    .get_ref(3)
                    .and_then(|r| self.ent(r).map(|x| (r, x)))
                    .map(|(r, x)| {
                        self.texto(&x, 0)
                            .or_else(|| self.texto(&x, 2))
                            .unwrap_or_else(|| clase_canonica(&self.tipo(r)))
                    })
                    .unwrap_or_default();
                out.push(Propiedad::nueva(&nombre, Valor::Texto(txt)));
            }
            _ => out.push(Propiedad::nueva(
                &nombre,
                Valor::Texto(format!("({})", clase_canonica(&t))),
            )),
        }
    }

    fn cantidad(&mut self, id: u32, prefijo: &str, hondo: usize, out: &mut Vec<Propiedad>) {
        if hondo > HONDO {
            return;
        }
        let Some(e) = self.ent(id) else { return };
        let nombre = format!("{prefijo}{}", self.texto(&e, 0).unwrap_or_default());
        let t = self.tipo(id);
        let medida = match t.as_str() {
            "IFCQUANTITYLENGTH" => Some("IFCLENGTHMEASURE"),
            "IFCQUANTITYAREA" => Some("IFCAREAMEASURE"),
            "IFCQUANTITYVOLUME" => Some("IFCVOLUMEMEASURE"),
            "IFCQUANTITYWEIGHT" => Some("IFCMASSMEASURE"),
            "IFCQUANTITYTIME" => Some("IFCTIMEMEASURE"),
            "IFCQUANTITYCOUNT" | "IFCQUANTITYNUMBER" => None,
            "IFCPHYSICALCOMPLEXQUANTITY" => {
                for q in refs(e.get(2)) {
                    self.cantidad(q, &format!("{nombre}."), hondo + 1, out);
                }
                return;
            }
            _ => return,
        };
        let Some((v, _)) = e.get(3).and_then(|a| self.valor(a)) else {
            return;
        };
        let p = self.con_unidad(&nombre, v, medida, e.get_ref(2));
        out.push(p);
    }

    /// Los nombres de los materiales que cuelgan de una asociacion (capas,
    /// listas, constituyentes, perfiles) y las capas con su espesor (m).
    fn materiales(
        &mut self,
        id: u32,
        hondo: usize,
        nombres: &mut Vec<String>,
        capas: &mut Vec<(String, f64)>,
    ) {
        if hondo > HONDO {
            return;
        }
        let Some(e) = self.ent(id) else { return };
        let t = self.tipo(id);
        if t == "IFCMATERIAL" {
            if let Some(n) = self.texto(&e, 0) {
                if !nombres.contains(&n) {
                    nombres.push(n);
                }
            }
            return;
        }
        if t == "IFCMATERIALLAYER" {
            let mut sub = Vec::new();
            if let Some(m) = e.get_ref(0) {
                self.materiales(m, hondo + 1, &mut sub, &mut Vec::new());
            }
            let espesor = e.get(1).and_then(numero).unwrap_or(0.0) * self.escala;
            let n = sub
                .first()
                .cloned()
                .or_else(|| self.texto(&e, 3))
                .unwrap_or_else(|| "(sin material)".into());
            capas.push((n, espesor));
            for s in sub {
                if !nombres.contains(&s) {
                    nombres.push(s);
                }
            }
            return;
        }
        if !t.starts_with("IFCMATERIAL") {
            return;
        }
        for a in e.attributes.iter() {
            for r in refs(Some(a)) {
                if self.tipo(r).starts_with("IFCMATERIAL") {
                    self.materiales(r, hondo + 1, nombres, capas);
                }
            }
        }
    }

    /// `(fuente, "codigo nombre")` de una referencia de clasificacion.
    fn clasificacion(&mut self, id: u32) -> Option<(String, String)> {
        let e = self.ent(id)?;
        let t = self.tipo(id);
        if t == "IFCCLASSIFICATION" {
            let n = self.texto(&e, 3).unwrap_or_else(|| "Clasificacion".into());
            return Some((n, self.texto(&e, 1).unwrap_or_default()));
        }
        let codigo = self.texto(&e, 1).unwrap_or_default();
        let nombre = self.texto(&e, 2).unwrap_or_default();
        let fuente = e
            .get_ref(3)
            .and_then(|f| self.ent(f).map(|x| (f, x)))
            .and_then(|(f, x)| match self.tipo(f).as_str() {
                "IFCCLASSIFICATION" => self.texto(&x, 3),
                _ => self.texto(&x, 2),
            })
            .unwrap_or_else(|| "Clasificacion".into());
        let txt = format!("{codigo} {nombre}").trim().to_string();
        (!txt.is_empty()).then_some((fuente, txt))
    }

    /// Una `IfcPostalAddress` en una linea.
    fn direccion(&mut self, id: u32) -> Option<String> {
        let e = self.ent(id)?;
        let mut partes: Vec<String> = Vec::new();
        if let Some(AttributeValue::List(v)) = e.get(4) {
            partes.extend(v.iter().filter_map(|a| a.as_string()).map(str::to_string));
        }
        for i in [6, 7, 8, 9] {
            if let Some(s) = self.texto(&e, i) {
                partes.push(s);
            }
        }
        let s = partes
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        (!s.is_empty()).then_some(s)
    }
}

fn logico(s: &str) -> Option<Valor> {
    Some(match s {
        "T" | "TRUE" => Valor::Logico(Some(true)),
        "F" | "FALSE" => Valor::Logico(Some(false)),
        "U" | "UNKNOWN" => Valor::Logico(None),
        _ => return None,
    })
}

/// La cabecera STEP: `FILE_NAME` (nombre, fecha, autor, organizacion,
/// preprocesador, programa, autorizacion) y `FILE_DESCRIPTION`.
fn cabecera(bytes: &[u8], esquema: &str, a: &mut InfoArchivo) {
    let fin = bytes.len().min(256 * 1024);
    let t = String::from_utf8_lossy(&bytes[..fin]);
    let t = match t.find("ENDSEC") {
        Some(i) => &t[..i],
        None => &t,
    };
    if !esquema.is_empty() {
        a.version = Some(esquema.to_string());
        a.datos.push(Dato {
            clave: "Esquema".into(),
            valor: esquema.into(),
        });
    }
    if let Some(args) = argumentos(t, "FILE_DESCRIPTION") {
        let d = args.first().map(|v| v.join("; ")).unwrap_or_default();
        if !d.is_empty() {
            a.datos.push(Dato {
                clave: "Descripcion".into(),
                valor: d,
            });
        }
    }
    if let Some(args) = argumentos(t, "FILE_NAME") {
        let campo = |i: usize| -> Option<String> {
            let v = args.get(i)?.join("; ");
            (!v.trim().is_empty()).then_some(v)
        };
        let claves = [
            "Nombre",
            "Fecha",
            "Autor",
            "Organizacion",
            "Preprocesador",
            "Programa de origen",
            "Autorizacion",
        ];
        for (i, c) in claves.iter().enumerate() {
            if let Some(v) = campo(i) {
                a.datos.push(Dato {
                    clave: (*c).into(),
                    valor: v,
                });
            }
        }
        a.fecha = campo(1);
        a.autor = campo(2);
        a.organizacion = campo(3);
        a.aplicacion = campo(5).or_else(|| campo(4));
    }
}

/// Los argumentos de `CLAVE(...)` de la cabecera: cada uno, sus textos.
fn argumentos(t: &str, clave: &str) -> Option<Vec<Vec<String>>> {
    let i = t.find(&format!("{clave}("))? + clave.len() + 1;
    let b = &t.as_bytes()[i..];
    let mut args: Vec<Vec<String>> = vec![Vec::new()];
    let mut nivel = 0usize;
    let mut k = 0;
    while k < b.len() {
        match b[k] {
            b'\'' => {
                let mut s = Vec::new();
                k += 1;
                while k < b.len() {
                    if b[k] == b'\'' {
                        if b.get(k + 1) == Some(&b'\'') {
                            s.push(b'\'');
                            k += 2;
                            continue;
                        }
                        break;
                    }
                    s.push(b[k]);
                    k += 1;
                }
                let s = String::from_utf8_lossy(&s).into_owned();
                let s = ifc_lite_core::decode_ifc_string(&s).into_owned();
                if let Some(a) = args.last_mut() {
                    if !s.trim().is_empty() {
                        a.push(s);
                    }
                }
            }
            b'(' => nivel += 1,
            b')' => {
                if nivel == 0 {
                    break;
                }
                nivel -= 1;
            }
            b',' if nivel == 0 => args.push(Vec::new()),
            _ => {}
        }
        k += 1;
    }
    Some(args)
}

/// Pila grande: las operaciones booleanas de IFC recurren hondo.
const PILA: usize = 256 * 1024 * 1024;

/// **La caja de cada elemento** (por su id en el fichero), de las mallas de
/// `ifc-lite`, en metros y con Z hacia arriba.
pub fn cajas_de(bytes: &[u8]) -> Result<HashMap<u32, Caja>, String> {
    let datos = bytes.to_vec();
    std::thread::Builder::new()
        .stack_size(PILA)
        .name("bim-cajas".into())
        .spawn(move || {
            std::panic::catch_unwind(|| {
                let r = ifc_lite_processing::process_geometry_filtered_with_quality_and_ids(
                    &datos,
                    ifc_lite_processing::OpeningFilterMode::Default,
                    ifc_lite_processing::TessellationQuality::default(),
                    None,
                );
                let mut cajas: HashMap<u32, Caja> = HashMap::new();
                for m in &r.meshes {
                    if matches!(m.geometry_class, 1 | 2) || m.positions.is_empty() {
                        continue;
                    }
                    let o = m.origin;
                    // ifc-lite da las mallas en metros y con Z hacia arriba.
                    let c = Caja::de_puntos(m.positions.chunks_exact(3).map(|p| {
                        let x = p[0] as f64 + o[0];
                        let y = p[1] as f64 + o[1];
                        let z = p[2] as f64 + o[2];
                        [x, y, z]
                    }));
                    if let Some(c) = c {
                        cajas
                            .entry(m.express_id)
                            .and_modify(|x| x.unir(&c))
                            .or_insert(c);
                    }
                }
                cajas
            })
            .map_err(|_| "la geometria esta dañada".to_string())
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "la geometria esta dañada".to_string())?
}
