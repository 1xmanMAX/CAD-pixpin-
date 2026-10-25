//! **IFC4** (ISO 16739, texto STEP): proyecto, terreno, edificio y sus
//! niveles; cada pieza con su clase por la capa (`IfcWall`, `IfcSlab`,
//! `IfcDoor`, `IfcWindow`…), su forma como solidos extruidos (los prismas)
//! o como triangulos, su color, su material (la capa) y un juego de
//! propiedades con su nombre y su capa. Como lo escriben IfcOpenShell o el
//! banco BIM de FreeCAD, en metros.

use std::fmt::Write as _;

use super::Tipo;
use super::escena::{Elemento3d, Escena};
use super::geometria::Prisma;

/// **Un texto STEP** entre comillas: la comilla doble y lo que no es ASCII
/// en `\X2\…\X0\`.
pub fn texto_step(s: &str) -> String {
    let mut r = String::from("'");
    let mut ancho: Vec<u16> = Vec::new();
    let vaciar = |r: &mut String, a: &mut Vec<u16>| {
        if !a.is_empty() {
            r.push_str("\\X2\\");
            for u in a.drain(..) {
                let _ = write!(r, "{u:04X}");
            }
            r.push_str("\\X0\\");
        }
    };
    for c in s.chars() {
        if c.is_ascii() && !c.is_ascii_control() {
            vaciar(&mut r, &mut ancho);
            match c {
                '\'' => r.push_str("''"),
                '\\' => r.push_str("\\\\"),
                _ => r.push(c),
            }
        } else if !c.is_control() {
            let mut b = [0u16; 2];
            ancho.extend_from_slice(c.encode_utf16(&mut b));
        }
    }
    vaciar(&mut r, &mut ancho);
    r.push('\'');
    r
}

/// Un real STEP (siempre con punto).
pub fn real(v: f64) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let v = if v.abs() < 1e-12 { 0.0 } else { v };
    let mut s = format!("{v}");
    if !s.contains('.') {
        s.push('.');
    }
    s
}

const BASE64_IFC: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// **Un GlobalId** (128 bits en los 22 caracteres del IFC), sacado de una
/// semilla: el mismo dibujo da los mismos.
pub fn guid(semilla: u64, n: u64) -> String {
    let mezcla = |mut z: u64| {
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let alto = mezcla(semilla ^ n.wrapping_mul(0xA24B_AED4_963E_E407));
    let bajo = mezcla(alto ^ n);
    let mut v = ((alto as u128) << 64) | bajo as u128;
    let mut s = [0u8; 22];
    for c in s.iter_mut().rev() {
        *c = BASE64_IFC[(v & 63) as usize];
        v >>= 6;
    }
    // 22 x 6 = 132: al primero solo le quedan 2 bits (de 0 a 3).
    String::from_utf8_lossy(&s).into_owned()
}

struct Step {
    datos: String,
    n: u32,
    semilla: u64,
    guids: u64,
}

impl Step {
    fn poner(&mut self, e: impl AsRef<str>) -> u32 {
        self.n += 1;
        let _ = writeln!(self.datos, "#{}={};", self.n, e.as_ref());
        self.n
    }

    fn guid(&mut self) -> String {
        self.guids += 1;
        texto_step(&guid(self.semilla, self.guids))
    }

    fn punto3(&mut self, p: [f64; 3]) -> u32 {
        self.poner(format!(
            "IFCCARTESIANPOINT(({},{},{}))",
            real(p[0]),
            real(p[1]),
            real(p[2])
        ))
    }

    fn colocacion(&mut self, z: f64) -> u32 {
        let p = self.punto3([0.0, 0.0, z]);
        self.poner(format!("IFCAXIS2PLACEMENT3D(#{p},$,$)"))
    }
}

/// El estilo y el material de una capa (y color), y sus piezas.
struct CapaIfc {
    nombre: String,
    color: [f32; 4],
    estilo: u32,
    material: u32,
    elementos: Vec<u32>,
}

/// Referencias `#1,#2` para una lista.
fn lista(v: &[u32]) -> String {
    v.iter()
        .map(|i| format!("#{i}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// **El IFC4 del modelo** (en metros).
pub fn escribir(escena: &Escena, nombre: &str) -> String {
    let k = 1.0 / escena.unidades_por_metro();
    let semilla = escena.elementos.len() as u64
        ^ nombre
            .bytes()
            .fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    let mut s = Step {
        datos: String::new(),
        n: 0,
        semilla,
        guids: 0,
    };
    // Unidades y contexto.
    let metro = s.poner("IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.)");
    let m2 = s.poner("IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.)");
    let m3 = s.poner("IFCSIUNIT(*,.VOLUMEUNIT.,$,.CUBIC_METRE.)");
    let rad = s.poner("IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.)");
    let unidades = s.poner(format!("IFCUNITASSIGNMENT((#{metro},#{m2},#{m3},#{rad}))"));
    let origen = s.colocacion(0.0);
    let contexto = s.poner(format!(
        "IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#{origen},$)"
    ));
    let cuerpo = s.poner(format!(
        "IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#{contexto},$,.MODEL_VIEW.,$)"
    ));
    let arriba = s.poner("IFCDIRECTION((0.,0.,1.))");
    // Proyecto, terreno y edificio.
    let g = s.guid();
    let proyecto = s.poner(format!(
        "IFCPROJECT({g},$,{},$,$,$,$,(#{contexto}),#{unidades})",
        texto_step(nombre)
    ));
    let o = s.colocacion(0.0);
    let lugar_terreno = s.poner(format!("IFCLOCALPLACEMENT($,#{o})"));
    let g = s.guid();
    let terreno = s.poner(format!(
        "IFCSITE({g},$,'Terreno',$,$,#{lugar_terreno},$,$,.ELEMENT.,$,$,$,$,$)"
    ));
    let o = s.colocacion(0.0);
    let lugar_edificio = s.poner(format!("IFCLOCALPLACEMENT(#{lugar_terreno},#{o})"));
    let g = s.guid();
    let edificio = s.poner(format!(
        "IFCBUILDING({g},$,'Edificio',$,$,#{lugar_edificio},$,$,.ELEMENT.,$,$,$)"
    ));
    let g = s.guid();
    s.poner(format!(
        "IFCRELAGGREGATES({g},$,$,$,#{proyecto},(#{terreno}))"
    ));
    let g = s.guid();
    s.poner(format!(
        "IFCRELAGGREGATES({g},$,$,$,#{terreno},(#{edificio}))"
    ));
    // Los niveles (al menos uno).
    let niveles: Vec<(String, f64)> = if escena.niveles.is_empty() {
        vec![("Nivel 0".into(), 0.0)]
    } else {
        escena
            .niveles
            .iter()
            .map(|n| (n.nombre.clone(), n.cota))
            .collect()
    };
    let mut plantas = Vec::new();
    for (nombre, cota) in &niveles {
        let o = s.colocacion(cota * k);
        let lugar = s.poner(format!("IFCLOCALPLACEMENT(#{lugar_edificio},#{o})"));
        let g = s.guid();
        let p = s.poner(format!(
            "IFCBUILDINGSTOREY({g},$,{},$,$,#{lugar},$,$,.ELEMENT.,{})",
            texto_step(nombre),
            real(cota * k)
        ));
        plantas.push((p, lugar, cota * k));
    }
    let ids: Vec<u32> = plantas.iter().map(|p| p.0).collect();
    let g = s.guid();
    s.poner(format!(
        "IFCRELAGGREGATES({g},$,$,$,#{edificio},({}))",
        lista(&ids)
    ));
    // Un estilo y un material por capa.
    let mut capas: Vec<CapaIfc> = Vec::new();
    let mut por_planta: Vec<Vec<u32>> = vec![Vec::new(); plantas.len()];
    for el in &escena.elementos {
        let ip = el.nivel.unwrap_or(0).min(plantas.len() - 1);
        let (_, lugar_planta, cota) = plantas[ip];
        let ic = match capas
            .iter()
            .position(|c| c.nombre == el.capa && c.color == el.color)
        {
            Some(i) => i,
            None => {
                let c = el.color;
                let rgb = s.poner(format!(
                    "IFCCOLOURRGB($,{},{},{})",
                    real(c[0] as f64),
                    real(c[1] as f64),
                    real(c[2] as f64)
                ));
                let sombra = s.poner(format!(
                    "IFCSURFACESTYLESHADING(#{rgb},{})",
                    real((1.0 - c[3] as f64).clamp(0.0, 1.0))
                ));
                let estilo = s.poner(format!(
                    "IFCSURFACESTYLE({},.BOTH.,(#{sombra}))",
                    texto_step(&el.capa)
                ));
                let material = s.poner(format!("IFCMATERIAL({},$,$)", texto_step(&el.capa)));
                capas.push(CapaIfc {
                    nombre: el.capa.clone(),
                    color: c,
                    estilo,
                    material,
                    elementos: Vec::new(),
                });
                capas.len() - 1
            }
        };
        let estilo = capas[ic].estilo;
        let forma = forma(&mut s, el, k, cota, cuerpo, arriba, estilo);
        let o = s.colocacion(0.0);
        let lugar = s.poner(format!("IFCLOCALPLACEMENT(#{lugar_planta},#{o})"));
        let g = s.guid();
        let nombre = texto_step(&el.nombre);
        let clase = el.tipo.clase_ifc().to_uppercase();
        let comun = format!("{g},$,{nombre},$,$,#{lugar},#{forma},$");
        let caja = el.malla_completa().caja().unwrap_or([0.0; 6]);
        let alto = real((caja[5] - caja[2]) * k);
        let ancho = real((caja[3] - caja[0]).max(caja[4] - caja[1]) * k);
        let e = s.poner(match el.tipo {
            Tipo::Muro => format!("{clase}({comun},.STANDARD.)"),
            Tipo::Losa => format!("{clase}({comun},.FLOOR.)"),
            Tipo::Columna => format!("{clase}({comun},.COLUMN.)"),
            Tipo::Viga => format!("{clase}({comun},.BEAM.)"),
            Tipo::Puerta => format!("{clase}({comun},{alto},{ancho},.DOOR.,.NOTDEFINED.,$)"),
            Tipo::Ventana => format!("{clase}({comun},{alto},{ancho},.WINDOW.,.NOTDEFINED.,$)"),
            Tipo::Otro => format!("{clase}({comun},.NOTDEFINED.)"),
        });
        // Sus propiedades.
        let pn = s.poner(format!(
            "IFCPROPERTYSINGLEVALUE('Nombre',$,IFCLABEL({nombre}),$)"
        ));
        let pc = s.poner(format!(
            "IFCPROPERTYSINGLEVALUE('Capa',$,IFCLABEL({}),$)",
            texto_step(&el.capa)
        ));
        let g = s.guid();
        let pset = s.poner(format!(
            "IFCPROPERTYSET({g},$,'Pset_CADpixpin',$,(#{pn},#{pc}))"
        ));
        let g = s.guid();
        s.poner(format!(
            "IFCRELDEFINESBYPROPERTIES({g},$,$,$,(#{e}),#{pset})"
        ));
        capas[ic].elementos.push(e);
        por_planta[ip].push(e);
    }
    for c in &capas {
        let g = s.guid();
        s.poner(format!(
            "IFCRELASSOCIATESMATERIAL({g},$,$,$,({}),#{})",
            lista(&c.elementos),
            c.material
        ));
    }
    for (i, v) in por_planta.iter().enumerate() {
        if v.is_empty() {
            continue;
        }
        let g = s.guid();
        s.poner(format!(
            "IFCRELCONTAINEDINSPATIALSTRUCTURE({g},$,$,$,({}),#{})",
            lista(v),
            plantas[i].0
        ));
    }
    let mut r = String::new();
    r.push_str("ISO-10303-21;\nHEADER;\n");
    r.push_str("FILE_DESCRIPTION(('ViewDefinition [ReferenceView_V1.2]'),'2;1');\n");
    let _ = writeln!(
        r,
        "FILE_NAME({},'',(''),(''),'CAD-pixpin','CAD-pixpin','');",
        texto_step(&format!("{nombre}.ifc"))
    );
    r.push_str("FILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n");
    r.push_str(&s.datos);
    r.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    r
}

/// La forma de una pieza (relativa a su nivel): un solido extruido por
/// prisma o, si no los tiene, sus triangulos. Devuelve el
/// `IfcProductDefinitionShape`.
fn forma(
    s: &mut Step,
    el: &Elemento3d,
    k: f64,
    cota: f64,
    cuerpo: u32,
    arriba: u32,
    estilo: u32,
) -> u32 {
    let mut items = Vec::new();
    let tipo = if el.prismas.is_empty() {
        let m = &el.malla;
        let coords = m
            .puntos
            .iter()
            .map(|p| {
                format!(
                    "({},{},{})",
                    real(p[0] * k),
                    real(p[1] * k),
                    real(p[2] * k - cota)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let lista_p = s.poner(format!("IFCCARTESIANPOINTLIST3D(({coords}))"));
        let tri = m
            .triangulos()
            .iter()
            .map(|t| format!("({},{},{})", t[0] + 1, t[1] + 1, t[2] + 1))
            .collect::<Vec<_>>()
            .join(",");
        items.push(s.poner(format!("IFCTRIANGULATEDFACESET(#{lista_p},$,$,({tri}),$)")));
        "Tessellation"
    } else {
        for p in &el.prismas {
            items.push(extruido(s, p, k, cota, arriba));
        }
        "SweptSolid"
    };
    for i in &items {
        s.poner(format!("IFCSTYLEDITEM(#{i},(#{estilo}),$)"));
    }
    let rep = s.poner(format!(
        "IFCSHAPEREPRESENTATION(#{cuerpo},'Body','{tipo}',({}))",
        lista(&items)
    ));
    s.poner(format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))"))
}

fn extruido(s: &mut Step, p: &Prisma, k: f64, cota: f64, arriba: u32) -> u32 {
    let mut pts: Vec<u32> = p
        .planta
        .iter()
        .map(|q| {
            s.poner(format!(
                "IFCCARTESIANPOINT(({},{}))",
                real(q[0] * k),
                real(q[1] * k)
            ))
        })
        .collect();
    pts.push(pts[0]);
    let linea = s.poner(format!("IFCPOLYLINE(({}))", lista(&pts)));
    let perfil = s.poner(format!("IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#{linea})"));
    let pos = s.colocacion(p.z0 * k - cota);
    s.poner(format!(
        "IFCEXTRUDEDAREASOLID(#{perfil},#{pos},#{arriba},{})",
        real(p.alto * k)
    ))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn textos_y_reales_step() {
        assert_eq!(texto_step("Muro d'A"), "'Muro d''A'");
        assert_eq!(texto_step("Baño"), "'Ba\\X2\\00F1\\X0\\o'");
        assert_eq!(real(2.0), "2.");
        assert_eq!(real(-0.5), "-0.5");
        assert_eq!(real(f64::NAN), "0.");
    }

    #[test]
    fn los_guid_son_de_22_y_distintos() {
        let a = guid(7, 1);
        let b = guid(7, 2);
        assert_eq!(a.len(), 22);
        assert_ne!(a, b);
        assert!(matches!(a.as_bytes()[0], b'0'..=b'3'));
        assert_eq!(guid(7, 1), a);
    }
}
