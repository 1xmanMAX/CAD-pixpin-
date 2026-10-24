//! **Los enganches** (referencias a objetos): que el cursor caiga justo en
//! el extremo, el medio, el centro o el cruce de lo dibujado; y el modo orto
//! y el rastreo polar para ir en angulos exactos.

use crate::dibujo::{Entrada, Indice};
use crate::geo::{self, Forma, P};

/// A que se engancho.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tipo {
    Extremo,
    Medio,
    Centro,
    Cuadrante,
    Interseccion,
    Insercion,
    Perpendicular,
    Cercano,
}

impl Tipo {
    pub const TODOS: [Tipo; 8] = [
        Tipo::Extremo,
        Tipo::Medio,
        Tipo::Centro,
        Tipo::Cuadrante,
        Tipo::Interseccion,
        Tipo::Insercion,
        Tipo::Perpendicular,
        Tipo::Cercano,
    ];

    pub fn bit(self) -> u16 {
        1 << self as u16
    }

    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::Extremo => "Punto final",
            Tipo::Medio => "Punto medio",
            Tipo::Centro => "Centro",
            Tipo::Cuadrante => "Cuadrante",
            Tipo::Interseccion => "Interseccion",
            Tipo::Insercion => "Insercion",
            Tipo::Perpendicular => "Perpendicular",
            Tipo::Cercano => "Cercano",
        }
    }

    /// Cuanto manda: con dos candidatos igual de cerca gana el que mas.
    fn prioridad(self) -> u8 {
        match self {
            Tipo::Extremo | Tipo::Interseccion => 4,
            Tipo::Centro | Tipo::Medio | Tipo::Insercion => 3,
            Tipo::Cuadrante => 2,
            Tipo::Perpendicular => 1,
            Tipo::Cercano => 0,
        }
    }
}

/// Los modos encendidos (como `OSMODE`): uno por bit de [`Tipo::bit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modos(pub u16);

impl Default for Modos {
    /// Lo de siempre: extremo, medio, centro, interseccion e insercion.
    fn default() -> Self {
        Modos(
            Tipo::Extremo.bit()
                | Tipo::Medio.bit()
                | Tipo::Centro.bit()
                | Tipo::Interseccion.bit()
                | Tipo::Insercion.bit()
                | Tipo::Perpendicular.bit(),
        )
    }
}

impl Modos {
    pub fn tiene(self, t: Tipo) -> bool {
        self.0 & t.bit() != 0
    }

    pub fn alternar(&mut self, t: Tipo) {
        self.0 ^= t.bit();
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enganche {
    pub p: P,
    pub tipo: Tipo,
}

/// **El enganche mas cercano al cursor** dentro de `radio` (unidades del
/// dibujo). `base` es el punto anterior de la orden (para la perpendicular).
pub fn buscar(
    ind: &Indice,
    cursor: P,
    radio: f64,
    modos: Modos,
    base: Option<P>,
) -> Option<Enganche> {
    if modos.0 == 0 || radio <= 0.0 {
        return None;
    }
    let zona = geo::ensanchar([cursor[0], cursor[1], cursor[0], cursor[1]], radio);
    let cerca: Vec<&Entrada> = ind.en(&zona);
    let formas: Vec<&Forma> = cerca
        .iter()
        .flat_map(|e| e.formas.iter())
        .filter(|f| geo::se_tocan(&f.caja(), &zona))
        .collect();
    let mut mejor: Option<(f64, Enganche)> = None;
    let mut probar = |p: P, tipo: Tipo| {
        if !modos.tiene(tipo) {
            return;
        }
        let d = geo::dist(p, cursor);
        if d > radio {
            return;
        }
        // Un poco de ventaja a lo que manda mas: el extremo gana al cercano
        // aunque el cercano este a un pixel menos.
        let peso = d - tipo.prioridad() as f64 * radio * 0.2;
        if mejor.as_ref().is_none_or(|(m, _)| peso < *m) {
            mejor = Some((peso, Enganche { p, tipo }));
        }
    };
    for f in &formas {
        for p in f.extremos() {
            probar(p, Tipo::Extremo);
        }
        if let Some(m) = f.punto_medio() {
            probar(m, Tipo::Medio);
        }
        if let Some(c) = f.centro() {
            probar(c, Tipo::Centro);
        }
        for q in f.cuadrantes() {
            probar(q, Tipo::Cuadrante);
        }
        if let Some(b) = base
            && let Some(pie) = f.pie_perpendicular(b)
            && f.contiene_punto(pie, radio * 0.01)
        {
            probar(pie, Tipo::Perpendicular);
        }
        probar(f.cercano(cursor), Tipo::Cercano);
    }
    for e in &cerca {
        for p in &e.puntos {
            probar(*p, Tipo::Insercion);
        }
    }
    if modos.tiene(Tipo::Interseccion) {
        // Solo entre formas cerca del cursor: pocas.
        let n = formas.len().min(64);
        for i in 0..n {
            for j in i + 1..n {
                for p in geo::cortes(formas[i], formas[j]) {
                    probar(p, Tipo::Interseccion);
                }
            }
        }
    }
    mejor.map(|(_, e)| e)
}

/// **Orto**: de `base` a `p` solo en horizontal o vertical.
pub fn orto(base: P, p: P) -> P {
    let d = geo::restar(p, base);
    if d[0].abs() >= d[1].abs() {
        [p[0], base[1]]
    } else {
        [base[0], p[1]]
    }
}

/// **Rastreo polar**: si `p` cae a menos de `tolerancia` (radianes) de un
/// multiplo de `paso`, lo lleva a esa direccion (misma distancia).
pub fn polar(base: P, p: P, paso: f64, tolerancia: f64) -> Option<(P, f64)> {
    let d = geo::dist(base, p);
    if d < geo::EPS || paso <= 0.0 {
        return None;
    }
    let a = geo::angulo(base, p);
    let k = (a / paso).round() * paso;
    ((a - k).abs() <= tolerancia).then(|| (geo::polar(base, d, k), k))
}

/// **La rejilla**: el punto de la rejilla de lado `paso` mas cercano.
pub fn rejilla(p: P, paso: f64) -> P {
    if paso <= 0.0 {
        return p;
    }
    [(p[0] / paso).round() * paso, (p[1] / paso).round() * paso]
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::Dibujo;
    use crate::especialidad::{circulo, linea};

    fn dibujo() -> Dibujo {
        let mut d = Dibujo::nuevo();
        d.editar("x", |t| {
            t.anadir(linea([0.0, 0.0], [100.0, 0.0]));
            t.anadir(linea([50.0, -50.0], [50.0, 50.0]));
            t.anadir(circulo([200.0, 0.0], 20.0));
        });
        d
    }

    #[test]
    fn engancha_extremo_medio_cruce_y_centro() {
        let d = dibujo();
        let ind = d.indice();
        let m = Modos::default();
        let e = buscar(&ind, [99.0, 1.0], 5.0, m, None).unwrap();
        assert_eq!((e.tipo, e.p), (Tipo::Extremo, [100.0, 0.0]));
        // En el cruce de las dos lineas, que tambien es el medio de la
        // horizontal: gana la interseccion (manda mas).
        let e = buscar(&ind, [51.0, 1.0], 5.0, m, None).unwrap();
        assert_eq!(e.p, [50.0, 0.0]);
        assert_eq!(e.tipo, Tipo::Interseccion);
        let e = buscar(&ind, [201.0, 1.0], 5.0, m, None).unwrap();
        assert_eq!(e.tipo, Tipo::Centro);
        // Lejos de todo, nada.
        assert!(buscar(&ind, [150.0, 80.0], 5.0, m, None).is_none());
    }

    #[test]
    fn perpendicular_desde_el_punto_anterior() {
        let d = dibujo();
        let ind = d.indice();
        let m = Modos(Tipo::Perpendicular.bit());
        let e = buscar(&ind, [20.0, 2.0], 5.0, m, Some([22.0, 30.0])).unwrap();
        assert_eq!(e.tipo, Tipo::Perpendicular);
        assert!(geo::dist(e.p, [22.0, 0.0]) < 1e-9);
    }

    #[test]
    fn orto_polar_y_rejilla() {
        assert_eq!(orto([0.0, 0.0], [10.0, 3.0]), [10.0, 0.0]);
        assert_eq!(orto([0.0, 0.0], [2.0, -9.0]), [0.0, -9.0]);
        let (p, a) = polar([0.0, 0.0], [10.0, 10.5], std::f64::consts::PI / 4.0, 0.05).unwrap();
        assert!((a - std::f64::consts::PI / 4.0).abs() < 1e-12);
        assert!((p[0] - p[1]).abs() < 1e-9);
        assert!(polar([0.0, 0.0], [10.0, 5.0], std::f64::consts::PI / 4.0, 0.05).is_none());
        assert_eq!(rejilla([12.4, -7.6], 5.0), [10.0, -10.0]);
    }
}
