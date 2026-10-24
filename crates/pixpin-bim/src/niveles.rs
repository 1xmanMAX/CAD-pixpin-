//! **Los niveles de un IFC**: los `IfcBuildingStorey` (nombre y cota) y en
//! cual esta cada elemento, por `IfcRelContainedInSpatialStructure` (y, para
//! las piezas de otro, por `IfcRelAggregates`: un tramo de escalera esta en
//! el nivel de su escalera). Se lee el texto STEP tal cual: es lo unico que
//! hace falta de la estructura y no pide nada a `ifc-lite`.

use std::collections::HashMap;

/// Un nivel tal como viene: su id en el fichero, su `GlobalId` (la clave
/// que no cambia de un lector a otro), su nombre y su cota (en metros).
#[derive(Debug, Clone, PartialEq)]
pub struct NivelIfc {
    pub id: u32,
    pub global: String,
    pub nombre: String,
    pub cota: f64,
}

/// Los niveles y el nivel (su id) de cada elemento (por su id); y el
/// `GlobalId` de los elementos que se pidieron ([`de_ifc_con`]).
#[derive(Debug, Default)]
pub struct Espacial {
    pub niveles: Vec<NivelIfc>,
    pub de: HashMap<u32, u32>,
    pub globales: HashMap<u32, String>,
}

/// Una entidad: `#id=TIPO(params);`, con los parametros sin partir.
struct Entidad<'a> {
    id: u32,
    tipo: &'a str,
    params: &'a str,
}

/// Recorre las entidades del texto STEP. Los `;` dentro de un texto no cortan.
fn entidades(t: &str) -> impl Iterator<Item = Entidad<'_>> {
    let b = t.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        loop {
            // El siguiente `#` al principio de una entidad.
            while i < b.len() && b[i] != b'#' {
                i += 1;
            }
            if i >= b.len() {
                return None;
            }
            let ini = i;
            let mut en_texto = false;
            let mut j = i;
            while j < b.len() {
                match b[j] {
                    b'\'' => en_texto = !en_texto,
                    b';' if !en_texto => break,
                    _ => {}
                }
                j += 1;
            }
            i = j + 1;
            let s = &t[ini..j.min(t.len())];
            let Some(igual) = s.find('=') else { continue };
            let Ok(id) = s[1..igual].trim().parse::<u32>() else {
                continue;
            };
            let resto = s[igual + 1..].trim_start();
            let Some(abre) = resto.find('(') else {
                continue;
            };
            let tipo = resto[..abre].trim();
            let params = resto[abre + 1..]
                .trim_end()
                .strip_suffix(')')
                .unwrap_or(&resto[abre + 1..]);
            return Some(Entidad { id, tipo, params });
        }
    })
}

/// Parte los parametros de primer nivel (las comas dentro de `(...)` o de un
/// texto no cortan).
fn partir(p: &str) -> Vec<&str> {
    let b = p.as_bytes();
    let (mut out, mut nivel, mut en_texto, mut ini) = (Vec::new(), 0i32, false, 0);
    for (k, &c) in b.iter().enumerate() {
        match c {
            b'\'' => en_texto = !en_texto,
            b'(' if !en_texto => nivel += 1,
            b')' if !en_texto => nivel -= 1,
            b',' if !en_texto && nivel == 0 => {
                out.push(p[ini..k].trim());
                ini = k + 1;
            }
            _ => {}
        }
    }
    out.push(p[ini..].trim());
    out
}

/// `#12` → 12.
fn referencia(s: &str) -> Option<u32> {
    s.trim().strip_prefix('#')?.parse().ok()
}

/// `(#1,#2)` → [1, 2].
fn referencias(s: &str) -> Vec<u32> {
    s.trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .filter_map(referencia)
        .collect()
}

/// Un texto STEP (`'1ER PISO'`) sin comillas, con los `\X2\…\X0\` de los
/// acentos traducidos.
fn texto(s: &str) -> Option<String> {
    let s = s.trim();
    let s = s.strip_prefix('\'')?.strip_suffix('\'')?.replace("''", "'");
    Some(desescapar(&s))
}

fn desescapar(s: &str) -> String {
    let mut out = String::new();
    let mut resto = s;
    while let Some(k) = resto.find("\\X2\\") {
        out.push_str(&resto[..k]);
        let tras = &resto[k + 4..];
        let Some(fin) = tras.find("\\X0\\") else {
            out.push_str(&resto[k..]);
            return out;
        };
        let hex = &tras[..fin];
        for c in hex.as_bytes().chunks(4) {
            if let Some(ch) = std::str::from_utf8(c)
                .ok()
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .and_then(char::from_u32)
            {
                out.push(ch);
            }
        }
        resto = &tras[fin + 4..];
    }
    // `\X\E9` (un byte latin-1).
    let mut final_ = String::new();
    let mut r = resto;
    while let Some(k) = r.find("\\X\\") {
        final_.push_str(&r[..k]);
        let h = r.get(k + 3..k + 5).unwrap_or("");
        match u8::from_str_radix(h, 16) {
            Ok(v) => {
                final_.push(v as char);
                r = &r[k + 5..];
            }
            Err(_) => {
                final_.push_str("\\X\\");
                r = &r[k + 3..];
            }
        }
    }
    final_.push_str(r);
    out.push_str(&final_);
    out
}

/// Lo que mide la unidad de longitud del proyecto, en metros.
fn metros_de_la_unidad(ifc: &str) -> f64 {
    for e in entidades(ifc) {
        if e.tipo.eq_ignore_ascii_case("IFCSIUNIT") && e.params.contains(".LENGTHUNIT.") {
            let p = partir(e.params);
            let prefijo = p.get(2).copied().unwrap_or("$");
            return match prefijo {
                ".MILLI." => 0.001,
                ".CENTI." => 0.01,
                ".DECI." => 0.1,
                ".KILO." => 1000.0,
                _ => 1.0,
            };
        }
        if e.tipo.eq_ignore_ascii_case("IFCCONVERSIONBASEDUNIT")
            && e.params.contains(".LENGTHUNIT.")
        {
            let nombre = partir(e.params)
                .get(2)
                .and_then(|s| texto(s))
                .unwrap_or_default()
                .to_ascii_uppercase();
            if nombre.contains("FOOT") {
                return 0.3048;
            }
            if nombre.contains("INCH") {
                return 0.0254;
            }
        }
    }
    1.0
}

/// El `GlobalId`: el primer parametro de toda entidad con raiz
/// (`IfcRoot`). No lleva comas (son 22 caracteres de base 64).
fn global_de(params: &str) -> String {
    params.split(',').next().and_then(texto).unwrap_or_default()
}

/// Lee los niveles de un IFC.
pub fn de_ifc(ifc: &[u8]) -> Espacial {
    de_ifc_con(ifc, &std::collections::HashSet::new())
}

/// Lee los niveles de un IFC y, de paso, el `GlobalId` de las entidades
/// `quien` (los elementos a los que `ifc-lite` no se lo dio).
pub fn de_ifc_con(ifc: &[u8], quien: &std::collections::HashSet<u32>) -> Espacial {
    let t = String::from_utf8_lossy(ifc);
    let escala = metros_de_la_unidad(&t);
    let mut esp = Espacial::default();
    let mut contenido: Vec<(Vec<u32>, u32)> = Vec::new();
    let mut padre: HashMap<u32, u32> = HashMap::new();
    for e in entidades(&t) {
        if quien.contains(&e.id) {
            let g = global_de(e.params);
            if !g.is_empty() {
                esp.globales.insert(e.id, g);
            }
        }
        let tipo = e.tipo.to_ascii_uppercase();
        match tipo.as_str() {
            "IFCBUILDINGSTOREY" => {
                // (GlobalId, OwnerHistory, Name, Description, ObjectType,
                // ObjectPlacement, Representation, LongName, CompositionType, Elevation)
                let p = partir(e.params);
                let nombre = p
                    .get(2)
                    .and_then(|s| texto(s))
                    .or_else(|| p.get(7).and_then(|s| texto(s)))
                    .unwrap_or_default();
                let cota = p.get(9).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0) * escala;
                esp.niveles.push(NivelIfc {
                    id: e.id,
                    global: global_de(e.params),
                    nombre,
                    cota,
                });
            }
            "IFCRELCONTAINEDINSPATIALSTRUCTURE" => {
                // (GlobalId, OwnerHistory, Name, Description, RelatedElements, RelatingStructure)
                let p = partir(e.params);
                if let (Some(hijos), Some(sitio)) = (p.get(4), p.get(5).and_then(|s| referencia(s)))
                {
                    contenido.push((referencias(hijos), sitio));
                }
            }
            "IFCRELAGGREGATES" | "IFCRELNESTS" => {
                // (GlobalId, OwnerHistory, Name, Description, RelatingObject, RelatedObjects)
                let p = partir(e.params);
                if let (Some(de), Some(hijos)) = (p.get(4).and_then(|s| referencia(s)), p.get(5)) {
                    for h in referencias(hijos) {
                        padre.insert(h, de);
                    }
                }
            }
            _ => {}
        }
    }
    let es_nivel: std::collections::HashSet<u32> = esp.niveles.iter().map(|n| n.id).collect();
    for (hijos, sitio) in contenido {
        // Lo que esta en un espacio (habitacion) cuelga del nivel del espacio.
        let mut s = sitio;
        for _ in 0..8 {
            if es_nivel.contains(&s) {
                break;
            }
            match padre.get(&s) {
                Some(p) => s = *p,
                None => break,
            }
        }
        if es_nivel.contains(&s) {
            for h in hijos {
                esp.de.insert(h, s);
            }
        }
    }
    // Las piezas de algo (tramos de una escalera, paneles de un muro
    // cortina) van al nivel de ese algo.
    let piezas: Vec<u32> = padre
        .keys()
        .copied()
        .filter(|h| !esp.de.contains_key(h))
        .collect();
    for h in piezas {
        let mut p = h;
        for _ in 0..8 {
            match padre.get(&p) {
                Some(q) => p = *q,
                None => break,
            }
            if let Some(n) = esp.de.get(&p) {
                let n = *n;
                esp.de.insert(h, n);
                break;
            }
        }
    }
    esp
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const IFC: &str = "ISO-10303-21;\nDATA;\n\
#5=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
#38=IFCBUILDINGSTOREY('3Zu',#18,'1ER PISO',$,'Nivel:8mm Head',#37,$,'1ER PISO',.ELEMENT.,3400.);\n\
#42=IFCBUILDINGSTOREY('15Z',#18,'Planta \\X2\\00F1\\X0\\o; dos',$,$,#41,$,$,.ELEMENT.,6650.);\n\
#100=IFCWALL('a',#18,'Muro',$,$,#1,#2,'1',$);\n\
#101=IFCSTAIR('b',#18,'Escalera',$,$,#1,#2,'1',$);\n\
#102=IFCSTAIRFLIGHT('c',#18,'Tramo',$,$,#1,#2,'1',$);\n\
#200=IFCRELCONTAINEDINSPATIALSTRUCTURE('r',#18,$,$,(#100,#101),#42);\n\
#201=IFCRELAGGREGATES('s',#18,$,$,#101,(#102));\n\
ENDSEC;\n";

    #[test]
    fn lee_niveles_y_quien_esta_en_cada_uno() {
        let e = de_ifc(IFC.as_bytes());
        assert_eq!(e.niveles.len(), 2);
        assert_eq!(e.niveles[0].nombre, "1ER PISO");
        assert!((e.niveles[0].cota - 3.4).abs() < 1e-9, "en metros");
        // El `;` dentro del nombre no corta y la ñ se traduce.
        assert_eq!(e.niveles[1].nombre, "Planta ño; dos");
        // Su GlobalId, la clave estable.
        assert_eq!(
            (e.niveles[0].global.as_str(), e.niveles[1].global.as_str()),
            ("3Zu", "15Z")
        );
        assert_eq!(e.de.get(&100), Some(&42));
        // El tramo, por su escalera.
        assert_eq!(e.de.get(&102), Some(&42));
        // Caso negativo: lo que no esta en ningun nivel, no sale.
        assert_eq!(e.de.get(&38), None);
        assert!(
            e.globales.is_empty(),
            "sin pedirlos no se guarda ningun GlobalId"
        );
    }

    #[test]
    fn da_el_global_id_de_lo_que_se_pide() {
        let quien: std::collections::HashSet<u32> = [100, 102, 999].into_iter().collect();
        let e = de_ifc_con(IFC.as_bytes(), &quien);
        assert_eq!(e.globales.get(&100).map(String::as_str), Some("a"));
        assert_eq!(e.globales.get(&102).map(String::as_str), Some("c"));
        // Caso negativo: lo que no se pidio o no existe, nada.
        assert_eq!(e.globales.get(&101), None);
        assert_eq!(e.globales.get(&999), None);
    }
}
