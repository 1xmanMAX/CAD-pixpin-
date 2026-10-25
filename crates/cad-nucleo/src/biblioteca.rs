//! **La biblioteca de bloques**: aparatos sanitarios, mobiliario, la puerta,
//! los simbolos electricos y sanitarios y una columna, dibujados aqui mismo
//! (en milimetros, en la capa 0 para que tomen la capa del INSERT). Se crean
//! en el dibujo la primera vez que se insertan.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

use opencadcodec::EntityType;

use crate::especialidad::{arco, circulo, linea, polilinea};
use crate::geo::P;
use crate::patrones::{self, Patron};

/// Una pieza de la biblioteca.
pub struct Pieza {
    /// El nombre del bloque en el dibujo.
    pub nombre: &'static str,
    pub descripcion: &'static str,
    /// La capa en la que se pone el INSERT.
    pub capa: &'static str,
    /// Su dibujo, con el punto base en el origen.
    pub dibujo: fn() -> Vec<EntityType>,
}

const MOB: &str = "A-MOBILIARIO";

/// **Todas las piezas**, en el orden en que se listan.
pub const PIEZAS: &[Pieza] = &[
    Pieza {
        nombre: "INODORO",
        descripcion: "Inodoro con tanque, 500x700 (base: centro del muro)",
        capa: MOB,
        dibujo: inodoro,
    },
    Pieza {
        nombre: "LAVATORIO",
        descripcion: "Lavatorio de 550x450 (base: centro del muro)",
        capa: MOB,
        dibujo: lavatorio,
    },
    Pieza {
        nombre: "DUCHA",
        descripcion: "Ducha de 900x900 con su desague (base: centro del muro)",
        capa: MOB,
        dibujo: ducha,
    },
    Pieza {
        nombre: "TINA",
        descripcion: "Tina de 1500x700 (base: centro del muro)",
        capa: MOB,
        dibujo: tina,
    },
    Pieza {
        nombre: "LAVADERO",
        descripcion: "Lavadero de ropa de 600x500 (base: centro del muro)",
        capa: MOB,
        dibujo: lavadero,
    },
    Pieza {
        nombre: "COCINA",
        descripcion: "Cocina de 4 hornillas, 600x600 (base: centro del muro)",
        capa: MOB,
        dibujo: cocina,
    },
    Pieza {
        nombre: "REFRIGERADORA",
        descripcion: "Refrigeradora de 700x700 (base: centro del muro)",
        capa: MOB,
        dibujo: refrigeradora,
    },
    Pieza {
        nombre: "CAMA-1.5P",
        descripcion: "Cama de 1 1/2 plaza, 1050x1900 (base: centro de la cabecera)",
        capa: MOB,
        dibujo: cama_15,
    },
    Pieza {
        nombre: "CAMA-2P",
        descripcion: "Cama de 2 plazas, 1400x1900 (base: centro de la cabecera)",
        capa: MOB,
        dibujo: cama_2,
    },
    Pieza {
        nombre: "MESA-4S",
        descripcion: "Mesa de 900x900 con 4 sillas (base: centro)",
        capa: MOB,
        dibujo: mesa_4,
    },
    Pieza {
        nombre: "SOFA",
        descripcion: "Sofa de 3 cuerpos, 2000x850 (base: centro del respaldo)",
        capa: MOB,
        dibujo: sofa,
    },
    Pieza {
        nombre: "ESCRITORIO",
        descripcion: "Escritorio de 1200x600 con silla (base: centro del muro)",
        capa: MOB,
        dibujo: escritorio,
    },
    Pieza {
        nombre: "PUERTA-0.90",
        descripcion: "Puerta de 0.90 con hoja y giro (base: la bisagra; el vano va en +X)",
        capa: "A-PUERTAS",
        dibujo: puerta_90,
    },
    Pieza {
        nombre: "TOMACORRIENTE",
        descripcion: "Tomacorriente doble (simbolo, base: centro)",
        capa: "IE-ELECTRICAS",
        dibujo: tomacorriente,
    },
    Pieza {
        nombre: "INTERRUPTOR",
        descripcion: "Interruptor simple (simbolo, base: centro)",
        capa: "IE-ELECTRICAS",
        dibujo: interruptor,
    },
    Pieza {
        nombre: "LUMINARIA",
        descripcion: "Punto de luz en techo (simbolo, base: centro)",
        capa: "IE-ELECTRICAS",
        dibujo: luminaria,
    },
    Pieza {
        nombre: "SUMIDERO",
        descripcion: "Sumidero de 2\" (simbolo, base: centro)",
        capa: "IS-SANITARIAS",
        dibujo: sumidero,
    },
    Pieza {
        nombre: "REGISTRO",
        descripcion: "Registro roscado / caja de registro (simbolo, base: centro)",
        capa: "IS-SANITARIAS",
        dibujo: registro,
    },
    Pieza {
        nombre: "COLUMNA-30X30",
        descripcion: "Columna de 300x300 rellena (base: centro)",
        capa: "E-ESTRUCTURAS",
        dibujo: columna,
    },
];

/// Como se compara un nombre: sin tildes, en mayusculas y con guiones por espacios.
fn clave(s: &str) -> String {
    crate::ordenes::sin_tildes(s.trim())
        .to_uppercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// **La pieza por su nombre** (`cama-2p`, `Cama 2P`, `inodoro`…).
pub fn buscar(nombre: &str) -> Option<&'static Pieza> {
    let k = clave(nombre);
    PIEZAS.iter().find(|p| p.nombre == k)
}

/// La lista para la linea de ordenes, una pieza por linea.
pub fn lista() -> Vec<String> {
    PIEZAS
        .iter()
        .map(|p| format!("  {:<14} {}", p.nombre, p.descripcion))
        .collect()
}

// --------------------------------------------------------------- dibujos

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> EntityType {
    polilinea(&[[x0, y0], [x1, y0], [x1, y1], [x0, y1]], true)
}

/// Una polilinea cerrada con abombados (`(punto, abombado del tramo que sale)`).
fn con_curvas(v: &[(P, f64)]) -> EntityType {
    let mut e = polilinea(&v.iter().map(|(p, _)| *p).collect::<Vec<_>>(), true);
    if let EntityType::LwPolyline(p) = &mut e {
        for (vx, (_, b)) in p.vertices.iter_mut().zip(v) {
            vx.bulge = *b;
        }
    }
    e
}

/// Una taza en U: recta desde `y0` hasta `y1` y media vuelta al frente.
fn taza(medio_ancho: f64, y0: f64, y1: f64) -> EntityType {
    let a = medio_ancho;
    con_curvas(&[
        ([-a, y0], 0.0),
        ([a, y0], 0.0),
        ([a, y1], 1.0),
        ([-a, y1], 0.0),
    ])
}

fn inodoro() -> Vec<EntityType> {
    vec![
        // El tanque contra el muro y la taza.
        rect(-250.0, 0.0, 250.0, 200.0),
        taza(180.0, 200.0, 470.0),
        taza(130.0, 250.0, 470.0),
    ]
}

fn lavatorio() -> Vec<EntityType> {
    vec![
        rect(-275.0, 0.0, 275.0, 450.0),
        circulo([0.0, 250.0], 170.0),
        circulo([0.0, 250.0], 20.0),
        circulo([0.0, 50.0], 20.0),
    ]
}

fn ducha() -> Vec<EntityType> {
    vec![
        rect(-450.0, 0.0, 450.0, 900.0),
        linea([-450.0, 0.0], [450.0, 900.0]),
        linea([450.0, 0.0], [-450.0, 900.0]),
        circulo([0.0, 450.0], 40.0),
    ]
}

fn tina() -> Vec<EntityType> {
    vec![
        rect(-750.0, 0.0, 750.0, 700.0),
        con_curvas(&[
            ([-450.0, 70.0], 0.0),
            ([680.0, 70.0], 0.0),
            ([680.0, 630.0], 0.0),
            ([-450.0, 630.0], 1.0),
        ]),
        circulo([-600.0, 350.0], 25.0),
    ]
}

fn lavadero() -> Vec<EntityType> {
    vec![
        rect(-300.0, 0.0, 300.0, 500.0),
        rect(-250.0, 50.0, 250.0, 450.0),
        circulo([0.0, 250.0], 25.0),
    ]
}

fn cocina() -> Vec<EntityType> {
    let mut v = vec![rect(-300.0, 0.0, 300.0, 600.0)];
    for (x, y) in [
        (-140.0, 180.0),
        (140.0, 180.0),
        (-140.0, 440.0),
        (140.0, 440.0),
    ] {
        v.push(circulo([x, y], 100.0));
        v.push(circulo([x, y], 40.0));
    }
    v
}

fn refrigeradora() -> Vec<EntityType> {
    vec![
        rect(-350.0, 0.0, 350.0, 700.0),
        linea([-350.0, 640.0], [350.0, 640.0]),
        linea([-350.0, 0.0], [350.0, 640.0]),
    ]
}

/// Una cama de `ancho`, de 1900 de largo, con una o dos almohadas.
fn cama(ancho: f64, almohadas: usize) -> Vec<EntityType> {
    let a = ancho / 2.0;
    let mut v = vec![
        rect(-a, 0.0, a, 1900.0),
        // La cabecera y el doblez de la sabana.
        linea([-a, 50.0], [a, 50.0]),
        linea([-a, 650.0], [a, 650.0]),
        linea([-a, 650.0], [-a + 250.0, 450.0]),
    ];
    let hueco = 80.0;
    let w = (ancho - hueco * (almohadas as f64 + 1.0)) / almohadas as f64;
    for i in 0..almohadas {
        let x0 = -a + hueco + i as f64 * (w + hueco);
        v.push(rect(x0, 120.0, x0 + w, 400.0));
    }
    v
}

fn cama_15() -> Vec<EntityType> {
    cama(1050.0, 1)
}

fn cama_2() -> Vec<EntityType> {
    cama(1400.0, 2)
}

/// Una silla de 450x450 con el respaldo hacia `-y`, girada y llevada a `c`.
fn silla(c: P, giro: f64) -> Vec<EntityType> {
    let mut v = vec![
        rect(-225.0, -225.0, 225.0, 225.0),
        linea([-225.0, -160.0], [225.0, -160.0]),
    ];
    for e in &mut v {
        crate::operaciones::girar(e, [0.0, 0.0], giro);
        e.translate(opencadcodec::Vector3::new(c[0], c[1], 0.0));
    }
    v
}

fn mesa_4() -> Vec<EntityType> {
    let mut v = vec![rect(-450.0, -450.0, 450.0, 450.0)];
    let d = 450.0 + 100.0 + 225.0;
    // Cada silla mira a la mesa: el respaldo, del lado de fuera.
    for (c, giro) in [
        ([0.0, d], std::f64::consts::PI),
        ([0.0, -d], 0.0),
        ([d, 0.0], FRAC_PI_2),
        ([-d, 0.0], -FRAC_PI_2),
    ] {
        v.extend(silla(c, giro));
    }
    v
}

fn sofa() -> Vec<EntityType> {
    let mut v = vec![
        rect(-1000.0, 0.0, 1000.0, 850.0),
        // El respaldo, los brazos y los cojines.
        linea([-800.0, 200.0], [800.0, 200.0]),
        linea([-800.0, 200.0], [-800.0, 850.0]),
        linea([800.0, 200.0], [800.0, 850.0]),
    ];
    for x in [-1600.0 / 6.0, 1600.0 / 6.0] {
        v.push(linea([x, 200.0], [x, 850.0]));
    }
    v
}

fn escritorio() -> Vec<EntityType> {
    vec![
        rect(-600.0, 0.0, 600.0, 600.0),
        // La silla delante.
        circulo([0.0, 850.0], 250.0),
        linea([-200.0, 650.0], [200.0, 650.0]),
    ]
}

fn puerta_90() -> Vec<EntityType> {
    vec![
        // La hoja abierta (40 de espesor) y su giro hasta cerrar el vano.
        rect(0.0, 0.0, 40.0, 900.0),
        arco([0.0, 0.0], 900.0, 0.0, FRAC_PI_2),
    ]
}

fn tomacorriente() -> Vec<EntityType> {
    vec![
        circulo([0.0, 0.0], 100.0),
        linea([-40.0, -55.0], [-40.0, 55.0]),
        linea([40.0, -55.0], [40.0, 55.0]),
        linea([-100.0, 0.0], [100.0, 0.0]),
    ]
}

fn interruptor() -> Vec<EntityType> {
    let (s, c) = FRAC_PI_4.sin_cos();
    let a = [75.0 * c, 75.0 * s];
    let b = [250.0 * c, 250.0 * s];
    let t = [b[0] + 60.0 * s, b[1] - 60.0 * c];
    vec![circulo([0.0, 0.0], 75.0), linea(a, b), linea(b, t)]
}

fn luminaria() -> Vec<EntityType> {
    let k = 150.0 * FRAC_PI_4.cos();
    vec![
        circulo([0.0, 0.0], 150.0),
        linea([-k, -k], [k, k]),
        linea([-k, k], [k, -k]),
    ]
}

fn sumidero() -> Vec<EntityType> {
    vec![
        circulo([0.0, 0.0], 100.0),
        circulo([0.0, 0.0], 60.0),
        linea([-60.0, 0.0], [60.0, 0.0]),
        linea([0.0, -60.0], [0.0, 60.0]),
    ]
}

fn registro() -> Vec<EntityType> {
    vec![
        rect(-150.0, -150.0, 150.0, 150.0),
        circulo([0.0, 0.0], 100.0),
        linea([-150.0, -150.0], [150.0, 150.0]),
        linea([-150.0, 150.0], [150.0, -150.0]),
    ]
}

fn columna() -> Vec<EntityType> {
    let anillo = vec![
        ([-150.0, -150.0], 0.0),
        ([150.0, -150.0], 0.0),
        ([150.0, 150.0], 0.0),
        ([-150.0, 150.0], 0.0),
    ];
    vec![
        rect(-150.0, -150.0, 150.0, 150.0),
        patrones::sombreado(&[anillo], &[], Patron::Solido, 1.0, 0.0, None),
    ]
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn todas_las_piezas_tienen_dibujo_y_nombre_valido() {
        for p in PIEZAS {
            assert!(!(p.dibujo)().is_empty(), "{}", p.nombre);
            assert_eq!(
                crate::bloque::validar_nombre(p.nombre).as_deref(),
                Ok(p.nombre)
            );
            assert_eq!(buscar(p.nombre).unwrap().nombre, p.nombre);
        }
        assert_eq!(PIEZAS.len(), 19);
        assert_eq!(buscar("cama 2p").unwrap().nombre, "CAMA-2P");
        assert!(buscar("nave espacial").is_none());
    }
}
