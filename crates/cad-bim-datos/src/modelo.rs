//! **El modelo de datos comun** de un Revit y de un IFC: lo que se ve en el
//! explorador y lo que se exporta. Todas las medidas de posicion van en
//! metros; los valores de los parametros van en la unidad que traen (con su
//! simbolo y su factor a la unidad SI).

use serde::{Deserialize, Serialize};

/// **De donde vienen los datos.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Formato {
    #[default]
    Ifc,
    /// Un proyecto de Revit (`.rvt`).
    Revit,
    /// Una familia o plantilla de Revit (`.rfa`, `.rte`, `.rft`).
    FamiliaRevit,
}

impl Formato {
    pub fn nombre(self) -> &'static str {
        match self {
            Formato::Ifc => "IFC",
            Formato::Revit => "Revit",
            Formato::FamiliaRevit => "Familia de Revit",
        }
    }
}

/// **Un dato suelto** (clave y valor en texto): las lineas de
/// `BasicFileInfo`, la cabecera del IFC…
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Dato {
    pub clave: String,
    pub valor: String,
}

/// **El fichero**: version, quien y cuando, trabajo compartido…
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct InfoArchivo {
    pub nombre: String,
    pub ruta: String,
    pub formato: Formato,
    pub bytes: u64,
    /// `Revit 2024`, `IFC4`…
    pub version: Option<String>,
    /// La compilacion de Revit (`20230509_0315(x64)`).
    pub compilacion: Option<String>,
    /// El programa que lo escribio (IFC) o `ClientAppName` (Revit).
    pub aplicacion: Option<String>,
    pub autor: Option<String>,
    pub organizacion: Option<String>,
    /// El usuario del ultimo guardado (Revit).
    pub guardado_por: Option<String>,
    /// Fecha del ultimo guardado o de la exportacion.
    pub fecha: Option<String>,
    /// `Not enabled`, `Local`, `Central`… (Revit).
    pub trabajo_compartido: Option<String>,
    pub ruta_central: Option<String>,
    pub guid: Option<String>,
    pub idioma: Option<String>,
    /// El codigo OmniClass de una familia.
    pub omniclass: Option<String>,
    /// Todo lo demas tal cual (las lineas de `BasicFileInfo`, la cabecera STEP).
    pub datos: Vec<Dato>,
    /// La miniatura que guarda Revit (PNG). No va en el JSON.
    #[serde(skip)]
    pub vista_previa: Option<Vec<u8>>,
}

/// **El proyecto**: lo de «Informacion de proyecto» de Revit o el
/// `IfcProject`, el edificio y su direccion del IFC.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct InfoProyecto {
    pub nombre: Option<String>,
    pub numero: Option<String>,
    pub cliente: Option<String>,
    pub direccion: Option<String>,
    pub estado: Option<String>,
    pub fecha_emision: Option<String>,
    pub descripcion: Option<String>,
    pub organizacion: Option<String>,
    pub sitio: Option<String>,
    pub edificio: Option<String>,
    /// Los grupos de parametros tal como vienen (Revit: los de la
    /// informacion de proyecto o de la familia; IFC: los Psets del proyecto).
    pub grupos: Vec<Conjunto>,
}

/// **Un nodo de la estructura espacial** (Proyecto, Sitio, Edificio, Nivel,
/// Espacio), con su padre.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NodoEspacial {
    /// `IfcBuildingStorey`…
    pub clase: String,
    pub nombre: String,
    pub global_id: Option<String>,
    /// Indice del padre en [`ModeloBim::espacial`].
    pub padre: Option<usize>,
}

/// **Un nivel** (planta).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Nivel {
    pub nombre: String,
    /// Cota en metros.
    pub cota: Option<f64>,
    pub global_id: Option<String>,
    /// El edificio al que pertenece.
    pub edificio: Option<String>,
    /// Cuantos elementos tiene.
    pub elementos: usize,
}

/// **Una categoria** (Muros, Puertas…) y cuantos elementos tiene.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Categoria {
    pub nombre: String,
    /// Las clases IFC que caen en ella.
    pub clases: Vec<String>,
    pub elementos: usize,
}

/// **Un material.**
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Material {
    pub nombre: String,
    /// RGB.
    pub color: Option<[u8; 3]>,
    /// 0 opaco, 1 transparente.
    pub transparencia: Option<f64>,
    /// En cuantos elementos esta.
    pub elementos: usize,
}

/// **Una habitacion o espacio**, con su superficie y volumen si se leen.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Espacio {
    /// Indice en [`ModeloBim::elementos`].
    pub elemento: usize,
    pub numero: String,
    pub nombre: String,
    pub nivel: Option<usize>,
    /// m².
    pub area: Option<f64>,
    /// m³.
    pub volumen: Option<f64>,
}

/// **Un tipo** (Revit: familia y tipo; IFC: `IfcWallType`…), con sus
/// parametros y cuantos elementos lo usan.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TipoBim {
    pub id: String,
    pub clase: String,
    pub familia: String,
    pub nombre: String,
    pub elementos: usize,
    pub conjuntos: Vec<Conjunto>,
}

/// **Una caja** alineada con los ejes, en metros.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Caja {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Caja {
    /// La caja de unos puntos (`None` si no hay ninguno finito).
    pub fn de_puntos(puntos: impl IntoIterator<Item = [f64; 3]>) -> Option<Caja> {
        let mut c: Option<Caja> = None;
        for p in puntos {
            if !p.iter().all(|v| v.is_finite()) {
                continue;
            }
            match &mut c {
                None => c = Some(Caja { min: p, max: p }),
                Some(c) => c.anadir(p),
            }
        }
        c
    }

    pub fn anadir(&mut self, p: [f64; 3]) {
        for (k, v) in p.into_iter().enumerate() {
            self.min[k] = self.min[k].min(v);
            self.max[k] = self.max[k].max(v);
        }
    }

    pub fn unir(&mut self, otra: &Caja) {
        self.anadir(otra.min);
        self.anadir(otra.max);
    }

    /// Ancho, fondo y alto.
    pub fn medidas(&self) -> [f64; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }
}

/// **Que clase de conjunto es.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TipoConjunto {
    /// Los atributos del elemento (GlobalId, Name, Tag…) y lo de Revit
    /// (ElementId, familia, tipo).
    #[default]
    Atributos,
    /// Un `IfcPropertySet` o un grupo de parametros de Revit.
    Propiedades,
    /// Un `IfcElementQuantity` (Qto) o las medidas leidas.
    Cantidades,
    /// Lo que viene del tipo.
    Tipo,
    /// Materiales y capas.
    Material,
    /// Clasificacion (OmniClass, Uniclass…).
    Clasificacion,
}

impl TipoConjunto {
    pub fn nombre(self) -> &'static str {
        match self {
            TipoConjunto::Atributos => "Atributos",
            TipoConjunto::Propiedades => "Propiedades",
            TipoConjunto::Cantidades => "Cantidades",
            TipoConjunto::Tipo => "Tipo",
            TipoConjunto::Material => "Material",
            TipoConjunto::Clasificacion => "Clasificacion",
        }
    }
}

/// **Un conjunto de propiedades** con nombre (`Pset_WallCommon`,
/// `Qto_WallBaseQuantities`, `Datos de identidad`…).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Conjunto {
    pub nombre: String,
    pub tipo: TipoConjunto,
    pub propiedades: Vec<Propiedad>,
}

impl Conjunto {
    pub fn nuevo(nombre: impl Into<String>, tipo: TipoConjunto) -> Conjunto {
        Conjunto {
            nombre: nombre.into(),
            tipo,
            propiedades: Vec::new(),
        }
    }

    /// Anade un texto (si no esta vacio).
    pub fn texto(&mut self, nombre: &str, valor: impl Into<String>) {
        let v = valor.into();
        if !v.trim().is_empty() {
            self.propiedades
                .push(Propiedad::nueva(nombre, Valor::Texto(v)));
        }
    }

    /// Anade un texto si lo hay.
    pub fn texto_opcional(&mut self, nombre: &str, valor: Option<&str>) {
        if let Some(v) = valor {
            self.texto(nombre, v);
        }
    }

    /// Anade un numero con su unidad.
    pub fn medida(&mut self, nombre: &str, v: f64, unidad: &str) {
        if v.is_finite() {
            self.propiedades.push(Propiedad {
                nombre: nombre.into(),
                valor: Valor::Real(v),
                unidad: (!unidad.is_empty()).then(|| unidad.to_string()),
                a_si: 1.0,
            });
        }
    }

    pub fn buscar(&self, nombre: &str) -> Option<&Propiedad> {
        self.propiedades
            .iter()
            .find(|p| p.nombre.eq_ignore_ascii_case(nombre))
    }
}

fn es_uno(v: &f64) -> bool {
    *v == 1.0
}

fn uno() -> f64 {
    1.0
}

/// **Una propiedad**: nombre, valor y unidad.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Propiedad {
    pub nombre: String,
    pub valor: Valor,
    /// El simbolo (`m`, `mm`, `m²`…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unidad: Option<String>,
    /// Por cuanto multiplicar el valor para tenerlo en la unidad SI (`mm`: 0.001).
    #[serde(default = "uno", skip_serializing_if = "es_uno")]
    pub a_si: f64,
}

impl Propiedad {
    pub fn nueva(nombre: &str, valor: Valor) -> Propiedad {
        Propiedad {
            nombre: nombre.into(),
            valor,
            unidad: None,
            a_si: 1.0,
        }
    }

    /// El valor como se lee, con su unidad (`3.05 m`).
    pub fn texto(&self) -> String {
        let v = self.valor.texto();
        match &self.unidad {
            Some(u) if !v.is_empty() && self.valor.numero().is_some() => format!("{v} {u}"),
            _ => v,
        }
    }

    /// El numero en la unidad SI (metros, m², m³, kg…).
    pub fn numero_si(&self) -> Option<f64> {
        self.valor.numero().map(|v| v * self.a_si)
    }
}

/// **Un valor con su tipo.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", content = "valor")]
pub enum Valor {
    Texto(String),
    Entero(i64),
    Real(f64),
    /// Verdadero, falso o desconocido.
    Logico(Option<bool>),
    Lista(Vec<Valor>),
}

impl Valor {
    /// En texto: los reales con hasta 4 decimales, sin ceros de mas.
    pub fn texto(&self) -> String {
        match self {
            Valor::Texto(t) => t.clone(),
            Valor::Entero(n) => n.to_string(),
            Valor::Real(v) => numero_corto(*v),
            Valor::Logico(Some(true)) => "Si".into(),
            Valor::Logico(Some(false)) => "No".into(),
            Valor::Logico(None) => "Desconocido".into(),
            Valor::Lista(v) => v.iter().map(Valor::texto).collect::<Vec<_>>().join("; "),
        }
    }

    pub fn numero(&self) -> Option<f64> {
        match self {
            Valor::Entero(n) => Some(*n as f64),
            Valor::Real(v) if v.is_finite() => Some(*v),
            _ => None,
        }
    }
}

/// Un real con hasta 4 decimales, sin ceros al final.
pub fn numero_corto(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// **Un elemento** del modelo con todo lo que se sabe de el.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ElementoBim2 {
    /// Revit: el ElementId; IFC: el numero de la entidad (`#123`).
    pub id: String,
    pub global_id: Option<String>,
    /// La clase IFC (`IfcWall`).
    pub clase: String,
    /// La categoria legible (`Muros`).
    pub categoria: String,
    pub familia: String,
    pub tipo: String,
    pub nombre: String,
    /// Indice en [`ModeloBim::niveles`].
    pub nivel: Option<usize>,
    pub materiales: Vec<String>,
    pub caja: Option<Caja>,
    pub conjuntos: Vec<Conjunto>,
}

impl ElementoBim2 {
    /// El valor de una propiedad por su nombre, en cualquier conjunto (o en
    /// el que se diga).
    pub fn propiedad(&self, conjunto: Option<&str>, nombre: &str) -> Option<&Propiedad> {
        self.conjuntos
            .iter()
            .filter(|c| conjunto.is_none_or(|n| c.nombre.eq_ignore_ascii_case(n)))
            .find_map(|c| c.buscar(nombre))
    }

    /// Si algo del elemento contiene `buscado` (en minusculas): su nombre,
    /// categoria, familia, tipo, id, material o el valor de una propiedad.
    pub fn coincide(&self, buscado: &str) -> bool {
        if buscado.is_empty() {
            return true;
        }
        let tiene = |s: &str| s.to_lowercase().contains(buscado);
        tiene(&self.nombre)
            || tiene(&self.categoria)
            || tiene(&self.clase)
            || tiene(&self.familia)
            || tiene(&self.tipo)
            || tiene(&self.id)
            || self.global_id.as_deref().is_some_and(tiene)
            || self.materiales.iter().any(|m| tiene(m))
            || self
                .conjuntos
                .iter()
                .flat_map(|c| &c.propiedades)
                .any(|p| tiene(&p.valor.texto()))
    }
}

/// **Todo lo leido de un modelo.**
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ModeloBim {
    pub archivo: InfoArchivo,
    pub proyecto: InfoProyecto,
    pub espacial: Vec<NodoEspacial>,
    pub niveles: Vec<Nivel>,
    pub categorias: Vec<Categoria>,
    pub elementos: Vec<ElementoBim2>,
    pub materiales: Vec<Material>,
    pub espacios: Vec<Espacio>,
    pub tipos: Vec<TipoBim>,
    /// Lo que no se pudo leer o se leyo a medias (dicho para el usuario).
    pub avisos: Vec<String>,
}

impl ModeloBim {
    /// **Rehace las cuentas**: las categorias (por orden de nombre) y
    /// cuantos elementos hay por nivel, material y tipo.
    pub fn recontar(&mut self) {
        let mut cats: std::collections::BTreeMap<String, Categoria> = Default::default();
        for n in &mut self.niveles {
            n.elementos = 0;
        }
        for m in &mut self.materiales {
            m.elementos = 0;
        }
        let mut por_material: std::collections::HashMap<&str, usize> = Default::default();
        for e in &self.elementos {
            let c = cats
                .entry(e.categoria.clone())
                .or_insert_with(|| Categoria {
                    nombre: e.categoria.clone(),
                    ..Default::default()
                });
            c.elementos += 1;
            if !c.clases.contains(&e.clase) {
                c.clases.push(e.clase.clone());
            }
            if let Some(n) = e.nivel.and_then(|i| self.niveles.get_mut(i)) {
                n.elementos += 1;
            }
            for m in &e.materiales {
                *por_material.entry(m.as_str()).or_default() += 1;
            }
        }
        for m in &mut self.materiales {
            m.elementos = por_material.get(m.nombre.as_str()).copied().unwrap_or(0);
        }
        self.categorias = cats.into_values().collect();
    }

    /// Los indices de los elementos de una categoria.
    pub fn de_categoria(&self, categoria: &str) -> Vec<usize> {
        (0..self.elementos.len())
            .filter(|&i| self.elementos[i].categoria == categoria)
            .collect()
    }

    /// **Lo que coincide con una busqueda** (sin distinguir mayusculas).
    pub fn buscar(&self, texto: &str) -> Vec<usize> {
        let t = texto.trim().to_lowercase();
        (0..self.elementos.len())
            .filter(|&i| self.elementos[i].coincide(&t))
            .collect()
    }

    /// El nombre del nivel de un elemento.
    pub fn nivel_de(&self, e: &ElementoBim2) -> Option<&str> {
        e.nivel
            .and_then(|i| self.niveles.get(i))
            .map(|n| n.nombre.as_str())
    }
}

/// **La categoria legible de una clase IFC** (`IfcWallStandardCase` ->
/// `Muros`). Lo que no esta en la tabla queda con su clase sin `Ifc`.
pub fn categoria_de_clase(clase: &str) -> String {
    let t = clase.to_ascii_lowercase();
    let t = t.strip_prefix("ifc").unwrap_or(&t);
    let s = match t {
        "wall" | "wallstandardcase" | "wallelementedcase" => "Muros",
        "curtainwall" => "Muros cortina",
        "slab" | "slabstandardcase" | "slabelementedcase" => "Losas",
        "column" | "columnstandardcase" => "Columnas",
        "beam" | "beamstandardcase" => "Vigas",
        "footing" => "Cimentaciones",
        "pile" => "Pilotes",
        "door" | "doorstandardcase" => "Puertas",
        "window" | "windowstandardcase" => "Ventanas",
        "stair" | "stairflight" => "Escaleras",
        "ramp" | "rampflight" => "Rampas",
        "railing" => "Barandas",
        "roof" => "Techos",
        "covering" => "Revestimientos",
        "plate" => "Placas",
        "member" => "Montantes y perfiles",
        "furnishingelement" | "furniture" | "systemfurnitureelement" => "Mobiliario",
        "flowterminal" | "sanitaryterminal" => "Aparatos sanitarios",
        "pipesegment" | "pipefitting" => "Tuberias",
        "ductsegment" | "ductfitting" => "Ductos",
        "airterminal" => "Rejillas y difusores",
        "cablecarriersegment" | "cablecarrierfitting" | "cablesegment" => "Bandejas y cables",
        "lightfixture" => "Luminarias",
        "electricappliance" | "electricdistributionboard" | "outlet" | "switchingdevice" => {
            "Equipos electricos"
        }
        "alarm" => "Alarmas",
        "space" => "Habitaciones",
        "buildingelementproxy" => "Modelos genericos",
        "transportelement" => "Ascensores",
        "reinforcingbar" | "reinforcingmesh" | "tendon" => "Armaduras",
        "opening" | "openingelement" => "Huecos",
        "site" => "Sitio",
        "geographicelement" => "Elementos del terreno",
        "flowsegment" | "flowfitting" | "distributionelement" | "flowcontroller" => "Instalaciones",
        "unitaryequipment" | "pump" | "fan" | "boiler" | "chiller" | "tank" => "Equipos mecanicos",
        "valve" => "Valvulas",
        "shadingdevice" => "Parasoles",
        _ => {
            return clase
                .strip_prefix("Ifc")
                .or_else(|| clase.strip_prefix("IFC"))
                .unwrap_or(clase)
                .to_string();
        }
    };
    s.to_string()
}

/// La clase IFC bien escrita (`IFCWALL` -> `IfcWall`).
pub fn clase_canonica(clase: &str) -> String {
    let t = ifc_lite_core::IfcType::from_str(clase.trim());
    let n = t.name();
    if n.starts_with("Ifc") {
        n.to_string()
    } else if clase.len() > 3 && clase[..3].eq_ignore_ascii_case("ifc") {
        // Desconocida: `IFCFOO` -> `IfcFOO` (se respeta lo demas).
        format!("Ifc{}", &clase[3..])
    } else {
        clase.to_string()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn categorias_y_clases() {
        assert_eq!(categoria_de_clase("IFCWALL"), "Muros");
        assert_eq!(categoria_de_clase("IfcWallStandardCase"), "Muros");
        assert_eq!(categoria_de_clase("IfcCoolingTower"), "CoolingTower");
        assert_eq!(clase_canonica("IFCWALL"), "IfcWall");
        assert_eq!(clase_canonica("IFCBUILDINGSTOREY"), "IfcBuildingStorey");
    }

    #[test]
    fn valores_en_texto() {
        assert_eq!(Valor::Real(3.04800001).texto(), "3.048");
        assert_eq!(Valor::Real(-0.00001).texto(), "0");
        let p = Propiedad {
            nombre: "Largo".into(),
            valor: Valor::Real(2500.0),
            unidad: Some("mm".into()),
            a_si: 0.001,
        };
        assert_eq!(p.texto(), "2500 mm");
        assert_eq!(p.numero_si(), Some(2.5));
    }
}
