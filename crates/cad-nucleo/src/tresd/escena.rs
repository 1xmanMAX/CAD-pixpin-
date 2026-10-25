//! **El modelo 3D de un dibujo**: sus piezas 3D (mallas, caras 3D, lo que
//! tiene altura) con su tipo por la capa, su color y su nivel, y las rayas
//! 2D a su cota para verlas en la ventana 3D.

use opencadcodec::{CadDocument, Color, EntityType, Layer};

use super::geometria::{Malla, P3, Prisma, malla_de_entidad};
use super::{Tipo, modelado, unidades_por_metro};
use crate::formas;
use crate::geo::Forma;

/// Hasta donde se entra en bloques dentro de bloques.
const PROFUNDIDAD: u32 = 4;

/// Donde guarda el dibujo sus niveles (una propiedad del documento, la de
/// PROPSDIBUJO, que va en el DWG y en el DXF).
pub const PROPIEDAD_NIVELES: &str = "CADPIXPIN_NIVELES";

/// **Un nivel del edificio** (`IfcBuildingStorey`): su nombre y su cota en
/// unidades del dibujo.
#[derive(Debug, Clone, PartialEq)]
pub struct Nivel {
    pub nombre: String,
    pub cota: f64,
}

/// **Una pieza del modelo.**
#[derive(Debug, Clone, PartialEq)]
pub struct Elemento3d {
    pub tipo: Tipo,
    pub nombre: String,
    pub capa: String,
    /// RGBA de 0 a 1.
    pub color: [f32; 4],
    /// En que nivel esta (indice en [`Escena::niveles`]).
    pub nivel: Option<usize>,
    /// Su forma: los prismas si lo es entera (asi va al IFC como solido
    /// extruido), y si no la malla.
    pub prismas: Vec<Prisma>,
    pub malla: Malla,
}

impl Elemento3d {
    /// Toda su superficie como malla (la de los prismas o la suya).
    pub fn malla_completa(&self) -> Malla {
        if self.prismas.is_empty() {
            self.malla.clone()
        } else {
            Malla::de_prismas(&self.prismas)
        }
    }
}

/// **Una raya 2D a su cota** (solo para verla en 3D).
#[derive(Debug, Clone, PartialEq)]
pub struct Linea3d {
    pub capa: String,
    pub color: [f32; 4],
    pub puntos: Vec<P3>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Escena {
    pub elementos: Vec<Elemento3d>,
    pub lineas: Vec<Linea3d>,
    /// De abajo arriba.
    pub niveles: Vec<Nivel>,
    /// Las unidades del dibujo (`$INSUNITS`).
    pub insunits: i16,
}

/// **Los niveles guardados en el dibujo**, de abajo arriba.
pub fn niveles(doc: &CadDocument) -> Vec<Nivel> {
    let Some((_, v)) = doc
        .summary_info
        .custom_properties
        .iter()
        .find(|(k, _)| k == PROPIEDAD_NIVELES)
    else {
        return Vec::new();
    };
    let mut n: Vec<Nivel> = v
        .split(';')
        .filter_map(|t| {
            let (nombre, cota) = t.rsplit_once('@')?;
            let cota: f64 = cota.trim().parse().ok()?;
            (cota.is_finite() && !nombre.trim().is_empty()).then(|| Nivel {
                nombre: nombre.trim().to_string(),
                cota,
            })
        })
        .collect();
    n.sort_by(|a, b| a.cota.total_cmp(&b.cota));
    n
}

/// **Guarda los niveles en el dibujo** (vacio: los quita).
pub fn poner_niveles(doc: &mut CadDocument, niveles: &[Nivel]) {
    let props = &mut doc.summary_info.custom_properties;
    props.retain(|(k, _)| k != PROPIEDAD_NIVELES);
    if niveles.is_empty() {
        return;
    }
    let mut v = niveles.to_vec();
    v.sort_by(|a, b| a.cota.total_cmp(&b.cota));
    let texto = v
        .iter()
        .map(|n| format!("{}@{}", n.nombre.replace([';', '@'], " ").trim(), n.cota))
        .collect::<Vec<_>>()
        .join(";");
    props.push((PROPIEDAD_NIVELES.to_string(), texto));
}

/// El color RGBA de una entidad (por capa, por bloque o el suyo); el 7
/// (blanco o negro segun el fondo), gris claro.
pub fn color_de(doc: &CadDocument, e: &EntityType, padre: Option<Color>) -> [f32; 4] {
    let mut c = e.common().color;
    if matches!(c, Color::ByBlock) {
        c = padre.unwrap_or(Color::Index(7));
    }
    if matches!(c, Color::ByLayer | Color::None) {
        c = doc
            .layers
            .get(&e.common().layer)
            .map_or(Color::Index(7), |l| l.color);
    }
    match c {
        Color::Index(7) | Color::Index(255) => [0.82, 0.82, 0.80, 1.0],
        otro => match otro.rgb() {
            Some((r, g, b)) => [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0],
            None => [0.82, 0.82, 0.80, 1.0],
        },
    }
}

fn capa_visible(doc: &CadDocument, capa: &str) -> bool {
    doc.layers
        .get(capa)
        .is_none_or(|l| !l.flags.frozen && !l.flags.off)
}

impl Escena {
    /// **El modelo del espacio modelo de un documento.** Las piezas que
    /// tienen altura (mallas, caras 3D, lo de ALTURA) son elementos; con
    /// `con_lineas`, lo demas que tiene rayas va como lineas a su cota.
    pub fn de_documento(doc: &CadDocument, con_lineas: bool) -> Escena {
        let mut s = Escena {
            niveles: niveles(doc),
            insunits: doc.header.insertion_units,
            ..Default::default()
        };
        for e in doc.model_space_entities() {
            s.entidad(doc, e, None, con_lineas, 0);
        }
        // El nivel de cada pieza: el mas alto que queda por debajo de ella.
        let tol = unidades_por_metro(s.insunits) * 0.01;
        for el in &mut s.elementos {
            let zmin = if el.prismas.is_empty() {
                el.malla.caja().map_or(0.0, |c| c[2])
            } else {
                el.prismas
                    .iter()
                    .map(|p| p.z0)
                    .fold(f64::INFINITY, f64::min)
            };
            el.nivel = s
                .niveles
                .iter()
                .rposition(|n| n.cota <= zmin + tol)
                .or((!s.niveles.is_empty()).then_some(0));
        }
        s
    }

    fn entidad(
        &mut self,
        doc: &CadDocument,
        e: &EntityType,
        padre: Option<Color>,
        con_lineas: bool,
        prof: u32,
    ) {
        let capa = e.common().layer.clone();
        if e.common().invisible || !capa_visible(doc, &capa) {
            return;
        }
        let color = color_de(doc, e, padre);
        if let EntityType::Insert(ins) = e {
            if prof >= PROFUNDIDAD {
                return;
            }
            let def: Vec<EntityType> = doc.entities_in_block(&ins.block_name).cloned().collect();
            let c = match e.common().color {
                Color::ByLayer => doc.layers.get(&capa).map(|l| l.color),
                otro => Some(otro),
            };
            for parte in ins.explode(&def) {
                self.entidad(doc, &parte, c, con_lineas, prof + 1);
            }
            return;
        }
        let malla = malla_de_entidad(e).or_else(|| modelado::de_altura(e));
        if let Some(m) = malla {
            if m.caras.is_empty() {
                return;
            }
            let tipo = Tipo::de_capa(&capa);
            let prismas = m.prismas().unwrap_or_default();
            let numero = self.elementos.iter().filter(|x| x.tipo == tipo).count() + 1;
            let mut color = color;
            if tipo == Tipo::Ventana && !matches!(e, EntityType::Face3D(_)) {
                // El vidrio deja ver.
                color[3] = color[3].min(0.45);
            }
            self.elementos.push(Elemento3d {
                tipo,
                nombre: format!("{} {numero}", tipo.nombre()),
                capa,
                color,
                nivel: None,
                malla: if prismas.is_empty() {
                    m
                } else {
                    Malla::default()
                },
                prismas,
            });
            return;
        }
        if !con_lineas {
            return;
        }
        let z = match e {
            EntityType::LwPolyline(p) => p.elevation,
            EntityType::Circle(c) => c.center.z,
            EntityType::Arc(a) => a.center.z,
            _ => 0.0,
        };
        if let EntityType::Line(l) = e {
            self.lineas.push(Linea3d {
                capa,
                color,
                puntos: vec![
                    [l.start.x, l.start.y, l.start.z],
                    [l.end.x, l.end.y, l.end.z],
                ],
            });
            return;
        }
        if let EntityType::Polyline3D(p) = e {
            let mut puntos: Vec<P3> = p
                .vertices
                .iter()
                .map(|v| [v.position.x, v.position.y, v.position.z])
                .collect();
            if p.is_closed()
                && let Some(q) = puntos.first().copied()
            {
                puntos.push(q);
            }
            self.lineas.push(Linea3d {
                capa,
                color,
                puntos,
            });
            return;
        }
        if matches!(
            e,
            EntityType::Text(_) | EntityType::MText(_) | EntityType::Hatch(_)
        ) {
            return;
        }
        for f in formas::formas(e) {
            let pts = match f {
                Forma::Segmento { a, b } => vec![a, b],
                otra => otra.muestras(32),
            };
            self.lineas.push(Linea3d {
                capa: capa.clone(),
                color,
                puntos: pts.iter().map(|p| [p[0], p[1], z]).collect(),
            });
        }
    }

    /// Cuantas unidades del dibujo es un metro.
    pub fn unidades_por_metro(&self) -> f64 {
        unidades_por_metro(self.insunits)
    }

    /// La caja de todo el modelo (piezas y lineas).
    pub fn caja(&self) -> Option<[f64; 6]> {
        let mut todo = Malla::default();
        for e in &self.elementos {
            if let Some(c) = e.malla_completa().caja() {
                todo.puntos.extend([[c[0], c[1], c[2]], [c[3], c[4], c[5]]]);
            }
        }
        for l in &self.lineas {
            todo.puntos.extend(&l.puntos);
        }
        todo.caja()
    }

    /// **Un dibujo con solo las piezas 3D**, como mallas en sus capas (para
    /// guardarlo en DWG o DXF).
    pub fn a_dibujo(&self) -> crate::Dibujo {
        let mut doc = CadDocument::new();
        doc.header.insertion_units = self.insunits;
        let mut capas: Vec<(&str, [f32; 4])> = Vec::new();
        for e in &self.elementos {
            if !capas.iter().any(|c| c.0 == e.capa) {
                capas.push((&e.capa, e.color));
            }
        }
        for (c, col) in &capas {
            let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            let mut l = Layer::new(*c);
            l.color = Color::from_rgb(b(col[0]), b(col[1]), b(col[2]));
            let _ = doc.layers.add(l);
        }
        poner_niveles(&mut doc, &self.niveles);
        let mut d = crate::Dibujo::de_documento(doc, None);
        d.editar("EXPORTAR3D", |t| {
            for e in &self.elementos {
                t.anadir_en(e.malla_completa().a_mesh(), &e.capa);
            }
        });
        d
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_niveles_se_guardan_en_el_documento() {
        let mut doc = CadDocument::new();
        assert!(niveles(&doc).is_empty());
        poner_niveles(
            &mut doc,
            &[
                Nivel {
                    nombre: "Planta 1".into(),
                    cota: 2800.0,
                },
                Nivel {
                    nombre: "Planta baja;".into(),
                    cota: 0.0,
                },
            ],
        );
        let n = niveles(&doc);
        assert_eq!(n.len(), 2);
        assert_eq!(n[0].nombre, "Planta baja");
        assert_eq!(n[1].cota, 2800.0);
        poner_niveles(&mut doc, &[]);
        assert!(niveles(&doc).is_empty());
    }
}
