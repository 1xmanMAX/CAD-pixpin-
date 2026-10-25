//! **Los patrones de sombreado** y como se arma un HATCH: el relleno solido,
//! las rayas ANSI31 y ANSI37 y la arena, con sus lineas de definicion ya
//! escaladas y giradas (como las guarda AutoCAD: asi lo ve igual cualquier
//! programa, aunque no tenga el `.pat`).

use opencadcodec::entities::{
    BoundaryEdge, BoundaryPath, Hatch, HatchPattern, HatchPatternLine, HatchPatternType,
    HatchStyleType, PolylineEdge,
};
use opencadcodec::{EntityType, Vector2, Vector3};

use crate::geo::P;

/// Un patron de la lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Patron {
    Solido,
    Ansi31,
    Ansi37,
    Arena,
}

/// Una familia de rayas como en un `.pat`: angulo (grados), origen,
/// desplazamiento entre rayas (a lo largo y de lado) y trazos.
type Familia = (f64, [f64; 2], [f64; 2], &'static [f64]);

const ANSI31: &[Familia] = &[(45.0, [0.0, 0.0], [0.0, 3.175], &[])];
const ANSI37: &[Familia] = &[
    (45.0, [0.0, 0.0], [0.0, 3.175], &[]),
    (135.0, [0.0, 0.0], [0.0, 3.175], &[]),
];
/// Puntos sueltos en tres direcciones que no se alinean (al estilo del
/// `AR-SAND`; se guarda como patron propio, `ARENA`, con sus lineas).
const ARENA: &[Familia] = &[
    (37.5, [0.0, 0.0], [1.123, 1.567], &[0.0, -1.031]),
    (7.5, [0.0, 0.0], [2.108, 2.941], &[0.0, -1.351]),
    (-32.5, [-1.687, 0.0], [1.123, 1.567], &[0.0, -1.031]),
];

impl Patron {
    pub const TODOS: &[Patron] = &[
        Patron::Solido,
        Patron::Ansi31,
        Patron::Ansi37,
        Patron::Arena,
    ];

    /// El nombre que se guarda en el HATCH.
    pub fn nombre(self) -> &'static str {
        match self {
            Patron::Solido => "SOLID",
            Patron::Ansi31 => "ANSI31",
            Patron::Ansi37 => "ANSI37",
            Patron::Arena => "ARENA",
        }
    }

    /// El nombre que se ve y se escribe.
    pub fn etiqueta(self) -> &'static str {
        match self {
            Patron::Solido => "SOLIDO",
            Patron::Ansi31 => "ANSI31",
            Patron::Ansi37 => "ANSI37",
            Patron::Arena => "ARENA",
        }
    }

    /// **El patron por su nombre** (en castellano o como en AutoCAD).
    pub fn de(texto: &str) -> Option<Patron> {
        match crate::ordenes::sin_tildes(texto.trim())
            .to_uppercase()
            .as_str()
        {
            "SOLIDO" | "SOLID" | "RELLENO" => Some(Patron::Solido),
            "ANSI31" => Some(Patron::Ansi31),
            "ANSI37" => Some(Patron::Ansi37),
            "ARENA" | "AR-SAND" | "PUNTOS" => Some(Patron::Arena),
            _ => None,
        }
    }

    fn familias(self) -> &'static [Familia] {
        match self {
            Patron::Solido => &[],
            Patron::Ansi31 => ANSI31,
            Patron::Ansi37 => ANSI37,
            Patron::Arena => ARENA,
        }
    }

    /// **Las lineas de definicion** a `escala` y girado `giro` radianes, en
    /// coordenadas del dibujo (el desplazamiento ya girado con su raya).
    pub fn lineas(self, escala: f64, giro: f64) -> Vec<HatchPatternLine> {
        let girar = |v: [f64; 2], a: f64| {
            let (s, c) = a.sin_cos();
            Vector2::new(v[0] * c - v[1] * s, v[0] * s + v[1] * c)
        };
        self.familias()
            .iter()
            .map(|(ang, base, desp, trazos)| {
                let a = ang.to_radians() + giro;
                HatchPatternLine {
                    angle: a,
                    base_point: girar([base[0] * escala, base[1] * escala], giro),
                    offset: girar([desp[0] * escala, desp[1] * escala], a),
                    dash_lengths: trazos.iter().map(|t| t * escala).collect(),
                }
            })
            .collect()
    }
}

/// Un contorno cerrado: sus vertices con el abombado del tramo que sale de
/// cada uno (como en una polilinea ligera).
pub type Anillo = Vec<(P, f64)>;

/// **Arma el HATCH** con su contorno exterior (o varios) y sus islas.
pub fn sombreado(
    exteriores: &[Anillo],
    islas: &[Anillo],
    patron: Patron,
    escala: f64,
    giro: f64,
    semilla: Option<P>,
) -> EntityType {
    let mut h = if patron == Patron::Solido {
        Hatch::solid()
    } else {
        let mut p = HatchPattern::new(patron.nombre());
        p.lines = patron.lineas(escala, giro);
        let mut h = Hatch::with_pattern(p);
        h.pattern_type = if patron == Patron::Arena {
            HatchPatternType::Custom
        } else {
            HatchPatternType::Predefined
        };
        // Sin `set_pattern_*`: las lineas ya van escaladas y giradas.
        h.pattern_angle = giro;
        h.pattern_scale = escala;
        h
    };
    h.style = HatchStyleType::Normal;
    h.is_associative = false;
    for camino in caminos(exteriores, islas) {
        h.add_path(camino);
    }
    if let Some(s) = semilla {
        h.add_seed_point(Vector2::new(s[0], s[1]));
    }
    EntityType::Hatch(h)
}

/// **Los caminos de un HATCH**: uno por anillo, como polilinea cerrada (los
/// de fuera marcados como exteriores).
pub fn caminos(exteriores: &[Anillo], islas: &[Anillo]) -> Vec<BoundaryPath> {
    exteriores
        .iter()
        .chain(islas)
        .enumerate()
        .map(|(k, a)| {
            let mut camino = if k < exteriores.len() {
                BoundaryPath::external()
            } else {
                BoundaryPath::new()
            };
            let mut pl = PolylineEdge::new(Vec::new(), true);
            for (p, b) in a {
                pl.vertices.push(Vector3::new(p[0], p[1], *b));
            }
            camino.add_edge(BoundaryEdge::Polyline(pl));
            camino
        })
        .collect()
}

/// **Un circulo como anillo**: dos medias vueltas (abombado 1).
pub fn anillo_circulo(c: P, r: f64) -> Anillo {
    vec![([c[0] + r, c[1]], 1.0), ([c[0] - r, c[1]], 1.0)]
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_desplazamiento_gira_con_la_raya() {
        let l = Patron::Ansi31.lineas(10.0, 0.0);
        assert_eq!(l.len(), 1);
        // 3,175 de lado a 45 grados: la separacion entre rayas sigue siendo 31,75.
        let n = [-l[0].angle.sin(), l[0].angle.cos()];
        let paso = l[0].offset.x * n[0] + l[0].offset.y * n[1];
        assert!((paso - 31.75).abs() < 1e-9);
        assert_eq!(Patron::de("solido"), Some(Patron::Solido));
        assert_eq!(Patron::de("ar-sand"), Some(Patron::Arena));
        assert_eq!(Patron::de("nada"), None);
        assert_eq!(Patron::Ansi37.lineas(1.0, 0.0).len(), 2);
    }
}
