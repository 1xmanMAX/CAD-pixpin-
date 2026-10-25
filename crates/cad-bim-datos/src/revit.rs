//! **Los datos de un Revit** con `rvt-rs`: el fichero (`BasicFileInfo`), la
//! informacion de proyecto o de la familia (`PartAtom`, OmniClass), la
//! miniatura y, en los proyectos de Revit 2023 a 2025 (2026 a prueba), los
//! elementos que `rvt-rs` decodifica, leidos de su modelo IFC en memoria (sin
//! pasar por el texto STEP): ElementId, GlobalId, categoria, familia y tipo,
//! nivel, materiales y capas, caja, y los parametros que trae (Type Mark,
//! Description, Fire Rating, nombre del tipo, numero y nombre de habitacion…).
//!
//! Lo que `rvt-rs` no lee (los elementos de Revit 2022 o anterior, la
//! mayoria de los parametros de ejemplar) se dice en [`ModeloBim::avisos`].

use std::path::Path;

use rvt::ifc::IfcModel;
use rvt::ifc::body_geometry::{Placement, body_mesh, element_body};
use rvt::ifc::entities::{IfcEntity, ProfileDef, PropertySet, PropertyValue};

use crate::modelo::*;

/// Pies a metros.
pub const PIE: f64 = 0.3048;

/// Las extensiones de Revit.
pub fn es_revit(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "rvt" | "rfa" | "rte" | "rft"
    )
}

/// **Lee un Revit.**
pub fn leer(ruta: &Path, avance: &mut dyn FnMut(&str)) -> Result<ModeloBim, String> {
    avance("Abriendo el Revit…");
    let mut rf =
        rvt::RevitFile::open(ruta).map_err(|e| format!("no es un Revit que se pueda leer: {e}"))?;
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mut m = ModeloBim::default();
    let streams = rf.stream_names();
    // Por la extension; sin ella, una familia trae PartAtom y no
    // ProjectInformation.
    let es_familia = match ext.as_str() {
        "rfa" | "rte" | "rft" => true,
        "rvt" => false,
        _ => {
            !streams.iter().any(|s| s == "ProjectInformation")
                && streams.iter().any(|s| s == "PartAtom")
        }
    };
    m.archivo.formato = if es_familia {
        Formato::FamiliaRevit
    } else {
        Formato::Revit
    };
    m.archivo.bytes = std::fs::metadata(ruta).map(|x| x.len()).unwrap_or(0);

    let bfi = rf
        .basic_file_info()
        .map_err(|e| format!("no es un Revit que se pueda leer (BasicFileInfo): {e}"))?;
    let a = &mut m.archivo;
    a.version = Some(format!("Revit {}", bfi.version));
    a.compilacion = bfi.build.clone();
    a.guid = bfi.document_guid().map(str::to_string).or(bfi.guid.clone());
    a.idioma = bfi.locale.clone();
    a.guardado_por = bfi.username().filter(|s| !s.is_empty()).map(str::to_string);
    a.trabajo_compartido = bfi.worksharing().map(str::to_string);
    a.ruta_central = bfi
        .central_model_path()
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    a.autor = bfi.author().filter(|s| !s.is_empty()).map(str::to_string);
    a.aplicacion = bfi
        .client_app_name()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| Some("Autodesk Revit".into()));
    if let Some(p) = &bfi.original_path {
        a.datos.push(Dato {
            clave: "Ruta original".into(),
            valor: p.clone(),
        });
    }
    for p in &bfi.properties {
        a.datos.push(Dato {
            clave: p.key.clone(),
            valor: p.value.clone(),
        });
    }

    // La informacion de proyecto (proyectos) o el PartAtom (familias).
    let atomo = if es_familia {
        rf.part_atom().ok()
    } else {
        rf.project_information()
            .ok()
            .flatten()
            .or_else(|| rf.part_atom().ok())
    };
    if let Some(at) = &atomo {
        m.archivo.fecha = at.updated.clone();
        m.archivo.omniclass = at.omniclass.clone();
        m.proyecto.nombre = at.entry_title.clone().or(at.title.clone());
        let mut tax = Conjunto::nuevo("Taxonomias", TipoConjunto::Clasificacion);
        for t in &at.taxonomies {
            tax.texto(&t.term, t.label.clone());
        }
        for c in &at.categories {
            tax.texto(c.scheme.as_deref().unwrap_or("Categoria"), c.term.clone());
        }
        tax.texto_opcional("OmniClass", at.omniclass.as_deref());
        if !tax.propiedades.is_empty() {
            m.proyecto.grupos.push(tax);
        }
        m.proyecto.grupos.extend(grupos_del_atomo(&at.raw_xml));
    }
    llenar_proyecto(&mut m.proyecto);
    m.archivo.vista_previa = rf.preview_png().ok().filter(|p| p.len() > 8);

    if es_familia {
        m.avisos.push(
            "Es una familia de Revit: trae sus datos, su OmniClass y su miniatura, pero no \
             elementos de un modelo (rvt-rs no lee la geometria de las familias)."
                .into(),
        );
        return Ok(m);
    }

    avance("Leyendo los elementos del Revit (rvt-rs)…");
    let v = bfi.version;
    if v <= 2022 {
        m.avisos.push(format!(
            "Revit {v}: rvt-rs aun no lee los elementos de Revit 2022 o anterior; \
             solo los datos del fichero, del proyecto y su miniatura."
        ));
    } else if v >= 2026 {
        m.avisos.push(format!(
            "Revit {v}: la lectura de elementos de rvt-rs es experimental en esta version."
        ));
    }
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rvt::ifc::RvtDocExporter.export_with_diagnostics(&mut rf)
    }));
    let r = match r {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            m.avisos
                .push(format!("rvt-rs no pudo leer los elementos: {e}"));
            m.recontar();
            return Ok(m);
        }
        Err(_) => {
            m.avisos
                .push("rvt-rs se detuvo leyendo los elementos (fichero raro o dañado).".into());
            m.recontar();
            return Ok(m);
        }
    };
    let d = &r.diagnostics;
    for w in d.unsupported_features.iter().take(10) {
        m.avisos.push(format!("No leido: {w}"));
    }
    for s in d.skipped.iter().take(10) {
        m.avisos
            .push(format!("Omitidos {} ({})", s.count, s.reason));
    }
    for w in d.warnings.iter().take(10) {
        m.avisos.push(w.clone());
    }
    avance("Ordenando los datos del Revit…");
    de_modelo(&r.model, &mut m);
    if m.elementos.is_empty() && v > 2022 {
        m.avisos
            .push("rvt-rs no encontro elementos que pueda leer en este proyecto.".into());
    }
    m.recontar();
    Ok(m)
}

/// Del proyecto: el nombre, numero, cliente… de sus grupos (los nombres
/// de los parametros de Revit en ingles o en español).
fn llenar_proyecto(p: &mut InfoProyecto) {
    let poner = |campo: &mut Option<String>, nombres: &[&str], grupos: &[Conjunto]| {
        if campo.is_some() {
            return;
        }
        for g in grupos {
            for q in &g.propiedades {
                let n = q.nombre.to_lowercase();
                if nombres.iter().any(|x| n == *x) {
                    let v = q.valor.texto();
                    if !v.trim().is_empty() {
                        *campo = Some(v);
                        return;
                    }
                }
            }
        }
    };
    let g = p.grupos.clone();
    let mut nombre = None;
    poner(
        &mut nombre,
        &["project name", "nombre de proyecto", "nombre del proyecto"],
        &g,
    );
    if nombre.is_some() {
        p.nombre = nombre;
    }
    poner(
        &mut p.numero,
        &["project number", "numero de proyecto", "número de proyecto"],
        &g,
    );
    poner(
        &mut p.cliente,
        &["client name", "nombre de cliente", "nombre del cliente"],
        &g,
    );
    poner(
        &mut p.direccion,
        &[
            "project address",
            "direccion de proyecto",
            "dirección de proyecto",
        ],
        &g,
    );
    poner(
        &mut p.estado,
        &[
            "project status",
            "estado de proyecto",
            "estado del proyecto",
        ],
        &g,
    );
    poner(
        &mut p.fecha_emision,
        &[
            "project issue date",
            "fecha de emision de proyecto",
            "fecha de emisión de proyecto",
        ],
        &g,
    );
    poner(
        &mut p.organizacion,
        &[
            "organization name",
            "nombre de organizacion",
            "nombre de organización",
        ],
        &g,
    );
    poner(
        &mut p.edificio,
        &["building name", "nombre de edificio"],
        &g,
    );
}

/// **Los grupos de parametros de un Atom de Revit** (`A:group` con su
/// `A:title` y sus parametros como elementos con texto).
pub fn grupos_del_atomo(xml: &str) -> Vec<Conjunto> {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return Vec::new();
    };
    let mut v = Vec::new();
    for g in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "group")
    {
        let titulo = g
            .children()
            .find(|c| c.is_element() && c.tag_name().name() == "title")
            .and_then(|c| c.text())
            .unwrap_or("Parametros")
            .trim()
            .to_string();
        let mut c = Conjunto::nuevo(titulo, TipoConjunto::Propiedades);
        for p in g.children().filter(|c| c.is_element()) {
            let n = p.tag_name().name();
            if n == "title" || n == "group" {
                continue;
            }
            let nombre = p.attribute("displayName").unwrap_or(n).replace('_', " ");
            let texto = p.text().unwrap_or("").trim().to_string();
            let mut q = Propiedad::nueva(&nombre, Valor::Texto(texto));
            if let Some(u) = p.attribute("units") {
                q.unidad = Some(u.to_string());
            }
            c.propiedades.push(q);
        }
        if !c.propiedades.is_empty() {
            v.push(c);
        }
    }
    v
}

/// Un valor de rvt-rs en el modelo comun (todo a metros).
fn valor(v: &PropertyValue) -> (Valor, Option<&'static str>) {
    match v {
        PropertyValue::Text(s) | PropertyValue::Label(s) | PropertyValue::Identifier(s) => {
            (Valor::Texto(s.clone()), None)
        }
        PropertyValue::Integer(n) | PropertyValue::CountValue(n) => (Valor::Entero(*n), None),
        PropertyValue::Real(x) => (Valor::Real(*x), None),
        PropertyValue::Boolean(b) => (Valor::Logico(Some(*b)), None),
        PropertyValue::Logical(b) => (Valor::Logico(*b), None),
        PropertyValue::LengthFeet(x) | PropertyValue::PositiveLengthFeet(x) => {
            (Valor::Real(x * PIE), Some("m"))
        }
        PropertyValue::AngleRadians(x) => (Valor::Real(x.to_degrees()), Some("°")),
        PropertyValue::AreaSquareFeet(x) => (Valor::Real(x * PIE * PIE), Some("m²")),
        PropertyValue::VolumeCubicFeet(x) => (Valor::Real(x * PIE * PIE * PIE), Some("m³")),
        PropertyValue::TimeSeconds(x) => (Valor::Real(*x), Some("s")),
        PropertyValue::MassPounds(x) => (Valor::Real(x * 0.453_592_37), Some("kg")),
        PropertyValue::List(l) => (Valor::Lista(l.iter().map(|x| valor(x).0).collect()), None),
        PropertyValue::Enumerated(l) => (
            Valor::Lista(l.iter().cloned().map(Valor::Texto).collect()),
            None,
        ),
    }
}

fn conjunto(set: &PropertySet) -> Conjunto {
    let mut c = Conjunto::nuevo(set.name.clone(), TipoConjunto::Propiedades);
    for p in &set.properties {
        let (v, u) = valor(&p.value);
        let mut q = Propiedad::nueva(&p.name, v);
        q.unidad = u.map(str::to_string);
        c.propiedades.push(q);
    }
    c
}

/// `Familia:Tipo:ElementId` (como nombra rvt-rs) en familia y tipo.
fn familia_y_tipo(nombre: &str) -> (String, String) {
    let partes: Vec<&str> = nombre.split(':').collect();
    match partes.as_slice() {
        [f, medio @ .., ultimo] if !medio.is_empty() && ultimo.trim().parse::<u64>().is_ok() => {
            (f.trim().to_string(), medio.join(":").trim().to_string())
        }
        [f, t] => (f.trim().to_string(), t.trim().to_string()),
        _ => (String::new(), String::new()),
    }
}

fn anadir(v: &mut Vec<String>, n: &str) {
    if !n.is_empty() && !v.iter().any(|x| x == n) {
        v.push(n.to_string());
    }
}

/// El area en planta de un perfil poligonal (sin sus huecos), en pies².
fn area_del_perfil(p: &ProfileDef) -> Option<f64> {
    let anillo = |pts: &[(f64, f64)]| {
        pts.iter()
            .zip(pts.iter().cycle().skip(1))
            .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
            .sum::<f64>()
            .abs()
            / 2.0
    };
    match p {
        ProfileDef::ArbitraryClosed { points } => Some(anillo(points)),
        ProfileDef::ArbitraryWithVoids { points, voids } => {
            Some(anillo(points) - voids.iter().map(|v| anillo(v)).sum::<f64>())
        }
        ProfileDef::Rectangle {
            width_feet,
            depth_feet,
        } => Some(width_feet * depth_feet),
        _ => None,
    }
}

/// **Del modelo IFC en memoria de rvt-rs al modelo comun.**
pub fn de_modelo(im: &IfcModel, m: &mut ModeloBim) {
    if m.proyecto.nombre.is_none() {
        m.proyecto.nombre = im.project_name.clone();
    }
    if m.proyecto.descripcion.is_none() {
        m.proyecto.descripcion = im.description.clone();
    }
    for (i, s) in im.building_storeys.iter().enumerate() {
        m.niveles.push(Nivel {
            nombre: s.name.clone(),
            cota: Some(s.elevation_feet * PIE),
            global_id: im.global_ids.storeys.get(&i).cloned(),
            edificio: None,
            elementos: 0,
        });
    }
    // La estructura espacial: Proyecto ▸ Sitio ▸ Edificio ▸ Niveles.
    m.espacial.push(NodoEspacial {
        clase: "IfcProject".into(),
        nombre: m
            .proyecto
            .nombre
            .clone()
            .unwrap_or_else(|| "Proyecto".into()),
        global_id: None,
        padre: None,
    });
    m.espacial.push(NodoEspacial {
        clase: "IfcSite".into(),
        nombre: "Sitio".into(),
        global_id: None,
        padre: Some(0),
    });
    m.espacial.push(NodoEspacial {
        clase: "IfcBuilding".into(),
        nombre: m
            .proyecto
            .edificio
            .clone()
            .unwrap_or_else(|| "Edificio".into()),
        global_id: None,
        padre: Some(1),
    });
    for n in &m.niveles {
        m.espacial.push(NodoEspacial {
            clase: "IfcBuildingStorey".into(),
            nombre: n.nombre.clone(),
            global_id: n.global_id.clone(),
            padre: Some(2),
        });
    }
    for x in &im.materials {
        m.materiales.push(Material {
            nombre: x.name.clone(),
            color: x.color_packed.map(|c| {
                [
                    (c & 0xFF) as u8,
                    ((c >> 8) & 0xFF) as u8,
                    ((c >> 16) & 0xFF) as u8,
                ]
            }),
            transparencia: x.transparency,
            elementos: 0,
        });
    }
    // Los conjuntos de mas de cada elemento (Pset_StairCommon…).
    let mut mas: std::collections::HashMap<usize, Vec<&PropertySet>> = Default::default();
    for e in &im.entities {
        if let IfcEntity::ElementPropertySet { element, set } = e {
            mas.entry(*element).or_default().push(set);
        }
    }
    let mut tipos: std::collections::BTreeMap<(String, String, String), TipoBim> =
        Default::default();
    for (indice, ent) in im.entities.iter().enumerate() {
        let IfcEntity::BuildingElement {
            ifc_type,
            name,
            type_guid,
            predefined_type,
            storey_index,
            material_index,
            property_set,
            location_feet,
            rotation_radians,
            extrusion,
            host_element_index,
            material_layer_set_index,
            material_profile_set_index,
            ..
        } = ent
        else {
            continue;
        };
        if ifc_type == "IFCOPENINGELEMENT" {
            continue;
        }
        let clase = clase_canonica(ifc_type);
        let id = type_guid.clone().unwrap_or_else(|| format!("e{indice}"));
        let id_num: Option<u32> = type_guid.as_deref().and_then(|g| g.parse().ok());
        let (mut familia, mut tipo) = familia_y_tipo(name);
        let pset = property_set.as_ref();
        let prop = |k: &str| {
            pset.and_then(|s| s.properties.iter().find(|p| p.name == k))
                .map(|p| valor(&p.value).0.texto())
                .filter(|s| !s.is_empty())
        };
        if tipo.is_empty() {
            if let Some(t) = prop("TypeName") {
                tipo = t;
            }
        }
        if familia.is_empty() {
            if let Some(f) = im
                .element_layers
                .get(&id_num.unwrap_or(u32::MAX))
                .and_then(|l| l.system_family.clone())
            {
                familia = f;
            }
        }
        // Los materiales, como los escribe rvt-rs: perfil > capas > material >
        // los de la geometria del tipo.
        let mut materiales: Vec<String> = Vec::new();
        let mut capas = Conjunto::nuevo("Capas de material", TipoConjunto::Material);
        if let Some(set) = material_profile_set_index.and_then(|i| im.material_profile_sets.get(i))
        {
            for p in &set.profiles {
                if let Some(x) = im.materials.get(p.material_index) {
                    anadir(&mut materiales, &x.name);
                }
                capas.texto("Perfil", p.profile_name.clone());
            }
        }
        if let Some(set) = material_layer_set_index.and_then(|i| im.material_layer_sets.get(i)) {
            for (k, c) in set.layers.iter().enumerate() {
                let n = c
                    .material_index
                    .and_then(|i| im.materials.get(i))
                    .map(|x| x.name.clone())
                    .or(c.name.clone())
                    .unwrap_or_else(|| "(sin material)".into());
                anadir(&mut materiales, &n);
                capas.medida(&format!("{}. {n}", k + 1), c.thickness_feet * PIE, "m");
            }
            if !set.layers.is_empty() {
                capas.medida(
                    "Espesor total",
                    set.layers.iter().map(|c| c.thickness_feet).sum::<f64>() * PIE,
                    "m",
                );
            }
        }
        if let Some(x) = material_index.and_then(|i| im.materials.get(i)) {
            anadir(&mut materiales, &x.name);
        }
        if materiales.is_empty() {
            for set in im
                .material_constituent_sets
                .iter()
                .filter(|s| s.elements.contains(&indice))
            {
                for x in set
                    .material_indices
                    .iter()
                    .filter_map(|&i| im.materials.get(i))
                {
                    anadir(&mut materiales, &x.name);
                }
            }
            if let Some(v) = id_num.and_then(|i| im.element_type_materials.get(&i)) {
                for n in v {
                    anadir(&mut materiales, n);
                }
            }
        }

        let mut ident = Conjunto::nuevo("Identidad", TipoConjunto::Atributos);
        ident.texto("ElementId", id.clone());
        ident.texto_opcional(
            "GlobalId",
            im.global_ids.elements.get(&indice).map(String::as_str),
        );
        ident.texto("Clase IFC", clase.clone());
        ident.texto_opcional("PredefinedType", predefined_type.as_deref());
        ident.texto("Familia", familia.clone());
        ident.texto("Tipo", tipo.clone());
        let tipo_id = id_num.and_then(|i| im.element_type_ids.get(&i)).copied();
        if let Some(t) = tipo_id {
            ident.texto("Id del tipo", t.to_string());
        }
        ident.texto_opcional(
            "Nivel",
            storey_index
                .and_then(|i| im.building_storeys.get(i))
                .map(|s| s.name.as_str()),
        );
        if let Some(IfcEntity::BuildingElement {
            type_guid: Some(h), ..
        }) = host_element_index.and_then(|i| im.entities.get(i))
        {
            ident.texto("Anfitrion (ElementId)", h.clone());
        }

        let mut conjuntos = vec![ident];
        if let Some(s) = pset {
            conjuntos.push(conjunto(s));
        }
        for s in mas.get(&indice).into_iter().flatten() {
            conjuntos.push(conjunto(s));
        }
        if !capas.propiedades.is_empty() {
            conjuntos.push(capas);
        }

        // Las medidas: el cuerpo extruido, el sitio y la caja.
        let mut cant = Conjunto::nuevo("Medidas (rvt-rs)", TipoConjunto::Cantidades);
        let mut area = None;
        let mut volumen = None;
        if let Some(e) = extrusion {
            match &e.profile_override {
                None | Some(ProfileDef::Rectangle { .. }) => {
                    let (w, d) = match &e.profile_override {
                        Some(ProfileDef::Rectangle {
                            width_feet,
                            depth_feet,
                        }) => (*width_feet, *depth_feet),
                        _ => (e.width_feet, e.depth_feet),
                    };
                    cant.medida("Ancho", w * PIE, "m");
                    cant.medida("Fondo", d * PIE, "m");
                }
                _ => {}
            }
            cant.medida("Alto", e.height_feet * PIE, "m");
            let a = e
                .profile_override
                .as_ref()
                .and_then(area_del_perfil)
                .or_else(|| {
                    e.profile_override
                        .is_none()
                        .then_some(e.width_feet * e.depth_feet)
                });
            if let Some(a) = a.filter(|a| *a > 0.0) {
                area = Some(a * PIE * PIE);
                volumen = Some(a * e.height_feet * PIE * PIE * PIE);
                cant.medida("Area en planta", a * PIE * PIE, "m²");
                cant.medida("Volumen", a * e.height_feet * PIE * PIE * PIE, "m³");
            }
        }
        if let Some(p) = location_feet {
            cant.medida("X", p[0] * PIE, "m");
            cant.medida("Y", p[1] * PIE, "m");
            cant.medida("Z", p[2] * PIE, "m");
        }
        if let Some(r) = rotation_radians.filter(|_| location_feet.is_some()) {
            cant.medida("Rotacion", r.to_degrees(), "°");
        }
        let caja = element_body(ent, &im.representation_maps)
            .and_then(body_mesh)
            .and_then(|malla| {
                let pl = Placement::new(*location_feet, *rotation_radians);
                Caja::de_puntos(malla.vertices.iter().map(|v| {
                    let p = pl.apply(*v);
                    [p[0] * PIE, p[1] * PIE, p[2] * PIE]
                }))
            });
        if !cant.propiedades.is_empty() {
            conjuntos.push(cant);
        }

        let categoria = categoria_de_clase(&clase);
        let k = m.elementos.len();
        if clase == "IfcSpace" {
            m.espacios.push(Espacio {
                elemento: k,
                numero: prop(rvt::ifc::export_content::ROOM_NUMBER_PROPERTY).unwrap_or_default(),
                nombre: prop(rvt::ifc::export_content::ROOM_NAME_PROPERTY).unwrap_or_default(),
                nivel: *storey_index,
                area,
                volumen,
            });
        }
        if !tipo.is_empty() || tipo_id.is_some() {
            let t = tipos
                .entry((clase.clone(), familia.clone(), tipo.clone()))
                .or_insert_with(|| TipoBim {
                    id: tipo_id.map(|t| t.to_string()).unwrap_or_default(),
                    clase: clase.clone(),
                    familia: familia.clone(),
                    nombre: tipo.clone(),
                    elementos: 0,
                    conjuntos: Vec::new(),
                });
            t.elementos += 1;
            // Los parametros de tipo que lee rvt-rs.
            if t.conjuntos.is_empty() {
                let mut c = Conjunto::nuevo("Parametros de tipo", TipoConjunto::Tipo);
                for n in ["Type Mark", "Description", "Fire Rating", "TypeName"] {
                    if let Some(v) = prop(n) {
                        c.texto(n, v);
                    }
                }
                if !c.propiedades.is_empty() {
                    t.conjuntos.push(c);
                }
            }
        }
        let nombre = if clase == "IfcSpace" {
            let n = prop(rvt::ifc::export_content::ROOM_NUMBER_PROPERTY).unwrap_or_default();
            let t = prop(rvt::ifc::export_content::ROOM_NAME_PROPERTY).unwrap_or_default();
            format!("{n} {t}").trim().to_string()
        } else {
            name.clone()
        };
        m.elementos.push(ElementoBim2 {
            id,
            global_id: im.global_ids.elements.get(&indice).cloned(),
            clase,
            categoria,
            familia,
            tipo,
            nombre,
            nivel: *storey_index,
            materiales,
            caja,
            conjuntos,
        });
    }
    m.tipos = tipos.into_values().collect();
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn familia_y_tipo_del_nombre() {
        assert_eq!(
            familia_y_tipo("Basic Wall:Generic - 200mm:12345"),
            ("Basic Wall".into(), "Generic - 200mm".into())
        );
        assert_eq!(familia_y_tipo("A:B"), ("A".into(), "B".into()));
        assert_eq!(familia_y_tipo("Muro"), (String::new(), String::new()));
    }

    #[test]
    fn grupos_de_un_atomo() {
        let xml = r#"<entry xmlns="http://www.w3.org/2005/Atom" xmlns:A="urn:schemas-autodesk-com:partatom"><title>Tower</title><A:features><A:feature><A:title>Project Information</A:title><A:group><A:title>Identity Data</A:title><Project_Name type="text">Torre Sur</Project_Name><Client_Name type="text">ACME</Client_Name></A:group><A:group><A:title>Other</A:title><Project_Number>P-17</Project_Number></A:group></A:feature></A:features></entry>"#;
        let g = grupos_del_atomo(xml);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].nombre, "Identity Data");
        assert_eq!(g[0].propiedades[0].nombre, "Project Name");
        let mut p = InfoProyecto {
            grupos: g,
            ..Default::default()
        };
        llenar_proyecto(&mut p);
        assert_eq!(p.nombre.as_deref(), Some("Torre Sur"));
        assert_eq!(p.cliente.as_deref(), Some("ACME"));
        assert_eq!(p.numero.as_deref(), Some("P-17"));
    }
}
