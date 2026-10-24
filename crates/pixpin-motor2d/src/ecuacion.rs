//! **La ecuacion tipografiada dentro del dibujo** (`motor/Ecuacion.kt` del
//! movil): raices con su signo y su raya, fracciones con su barra, exponentes
//! en alto, parentesis, valor absoluto y las partes con su llave. Nada de
//! `sqrt(2x)` escrito a maquina: lo que sale es la ecuacion como se escribe
//! en la pizarra.
//!
//! Todo son **textos y lineas del lienzo** —vectoriales— y no una imagen: se
//! mueven con la grafica, se les cambia el color y salen igual en el PDF, en
//! el SVG, en la pagina web y en el movil (que las lee como lo que son). La
//! raya de la fraccion, el signo de la raiz y la llave se dibujan con lineas,
//! que es lo que permite que crezcan con lo que envuelven; una letra no
//! crece.
//!
//! **Sin KaTeX ni MathJax**, ni en el movil ni aqui: el movil compone con sus
//! propias cajas, y esto es el mismo compositor en Rust puro. Un WebView2
//! para escribir una fraccion seria cien megas de memoria por nada.
//!
//! ## Como se compone
//!
//! Cada trozo del arbol de [`crate::formula`] se convierte en una **caja**
//! —ancho, alto y donde esta su eje, que es la linea por la que se alinean el
//! `+` y el `=`— y las cajas se ponen en fila o en columna. Una fraccion es
//! una columna con el eje en la barra; un exponente, una fila con la segunda
//! caja mas pequena y subida; una raiz, su argumento con el signo delante y la
//! raya encima. Aqui cada caja guarda sus piezas ya colocadas respecto de su
//! esquina (el movil guardaba un cierre que las pintaba): componer es correr
//! las piezas de los hijos, y pintar la ecuacion es convertir las piezas de la
//! caja de fuera en elementos.
//!
//! Las letras se miden con lo que pase quien llama (`Medir`): el editor pasa
//! DirectWrite; las pruebas, una cuenta fija.

use crate::elemento::{ColorRgba, Elemento, Figura};
use crate::formula::{Compilada, Nodo};
use crate::vector::Punto2;

/// Lo que ocupa un texto de un tamano: ancho y alto. Es `MedidaDeTexto` del
/// movil.
pub type Medir<'a> = &'a dyn Fn(&str, f32) -> (f32, f32);

/// Cuanto encoge un exponente.
const ENCOGE: f32 = 0.7;
/// Lo menos que se deja encoger una letra, o la tercera planta no se lee.
const MINIMO: f32 = 7.0;

/// Con que se pinta: el color del pincel y la letra.
#[derive(Debug, Clone, PartialEq)]
pub struct Estilo {
    pub color: ColorRgba,
    pub tam: f32,
    pub opacidad: f32,
    pub familia: String,
}

impl Estilo {
    pub fn con_tam(&self, tam: f32) -> Estilo {
        Estilo {
            tam,
            ..self.clone()
        }
    }

    pub fn con_color(&self, color: ColorRgba) -> Estilo {
        Estilo {
            color,
            ..self.clone()
        }
    }
}

/// Una pieza ya colocada respecto de la esquina de su caja.
///
/// `color: None` es «el del estilo de quien la pinta»; un tramo de color
/// (`\color{red}{X}`, un tramo rojo de un texto rico) lo lleva puesto, y el
/// de dentro gana al de fuera ([`Caja::tenida`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Pieza {
    Texto {
        x: f32,
        y: f32,
        ancho: f32,
        alto: f32,
        texto: String,
        tam: f32,
        cursiva: bool,
        negrita: bool,
        color: Option<ColorRgba>,
    },
    Raya {
        puntos: Vec<Punto2>,
        grosor: f32,
        color: Option<ColorRgba>,
    },
}

impl Pieza {
    fn corrida(mut self, dx: f32, dy: f32) -> Pieza {
        match &mut self {
            Pieza::Texto { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Pieza::Raya { puntos, .. } => {
                for p in puntos {
                    p.x += dx;
                    p.y += dy;
                }
            }
        }
        self
    }
}

/// Una caja compuesta: lo que mide, donde esta su eje y sus piezas.
#[derive(Debug, Clone, PartialEq)]
pub struct Caja {
    pub ancho: f32,
    pub alto: f32,
    /// A que altura desde arriba pasa el eje: donde va el «=» y la barra de
    /// una fraccion.
    pub eje: f32,
    pub piezas: Vec<Pieza>,
}

impl Caja {
    fn piezas_en(&self, dx: f32, dy: f32) -> impl Iterator<Item = Pieza> + '_ {
        self.piezas.iter().cloned().map(move |p| p.corrida(dx, dy))
    }

    /// La caja pintada con su esquina en `(x, y)`, como elementos del lienzo.
    pub fn elementos(&self, x: f32, y: f32, estilo: &Estilo) -> Vec<Elemento> {
        self.piezas_en(x, y)
            .map(|p| a_elemento(p, estilo))
            .collect()
    }

    /// Las piezas con su esquina en `(x, y)`, para quien las pinta sin
    /// hacerlas elementos (el texto rico del lienzo).
    pub fn piezas_colocadas(&self, x: f32, y: f32) -> Vec<Pieza> {
        self.piezas_en(x, y).collect()
    }

    /// **De un color**, salvo lo que ya tenia el suyo: en
    /// `\color{red}{a + \color{blue}{b}}` la `b` sigue azul.
    pub fn tenida(mut self, c: ColorRgba) -> Caja {
        for p in &mut self.piezas {
            match p {
                Pieza::Texto { color, .. } | Pieza::Raya { color, .. } => {
                    color.get_or_insert(c);
                }
            }
        }
        self
    }

    /// Con las letras en negrita y las rayas un poco mas gordas.
    pub fn en_negrita(mut self) -> Caja {
        for p in &mut self.piezas {
            match p {
                Pieza::Texto { negrita, .. } => *negrita = true,
                Pieza::Raya { grosor, .. } => *grosor *= 1.4,
            }
        }
        self
    }
}

/// Un texto del lienzo con su caja ya medida.
pub fn elemento_texto(
    texto: &str,
    x: f32,
    y: f32,
    (ancho, alto): (f32, f32),
    tam: f32,
    cursiva: bool,
    estilo: &Estilo,
) -> Elemento {
    Elemento {
        figura: Figura::Texto {
            texto: texto.to_string(),
            tam,
            familia: estilo.familia.clone(),
        },
        x,
        y,
        ancho,
        alto,
        trazo: estilo.color,
        opacidad: estilo.opacidad,
        grosor: 1.0,
        rugosidad: 0.0,
        extras: crate::elemento::Extras {
            cursiva,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Una linea del lienzo por esos puntos, recta y sin temblor: una ecuacion,
/// una grafica o una tabla se quieren limpias, no a mano alzada.
pub fn elemento_linea(puntos: Vec<Punto2>, grosor: f32, estilo: &Estilo) -> Elemento {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &puntos {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    let (x, y) = puntos.first().map_or((0.0, 0.0), |p| (p.x, p.y));
    Elemento {
        figura: Figura::Linea { puntos },
        x,
        y,
        ancho: (x1 - x0).max(0.0),
        alto: (y1 - y0).max(0.0),
        trazo: estilo.color,
        opacidad: estilo.opacidad,
        grosor,
        rugosidad: 0.0,
        ..Default::default()
    }
}

fn a_elemento(p: Pieza, estilo: &Estilo) -> Elemento {
    match p {
        Pieza::Texto {
            x,
            y,
            ancho,
            alto,
            texto,
            tam,
            cursiva,
            negrita,
            color,
        } => {
            let mut e = elemento_texto(
                &texto,
                x,
                y,
                (ancho, alto),
                tam,
                cursiva,
                &estilo.con_color(color.unwrap_or(estilo.color)),
            );
            e.extras.negrita = negrita;
            e
        }
        Pieza::Raya {
            puntos,
            grosor,
            color,
        } => elemento_linea(
            puntos,
            grosor,
            &estilo.con_color(color.unwrap_or(estilo.color)),
        ),
    }
}

/// La ecuacion `nombre = formula` entera, para saber cuanto ocupa antes de
/// colocarla. Con varias partes, una llave las abarca y cada una lleva su
/// condicion.
pub fn caja_de_la_ecuacion(nombre: &str, formula: &Compilada, t: f32, medir: Medir<'_>) -> Caja {
    let cabeza = fila(vec![letra(&format!("{nombre} ="), t, false, medir)]);
    if formula.partes.len() == 1 && formula.partes[0].condicion.is_none() {
        return fila(vec![
            cabeza,
            hueco(t * 0.3),
            componer(&formula.partes[0].expresion, t, medir),
        ]);
    }
    // Por partes: una columna de «expresion  si  condicion» y una llave delante.
    let filas: Vec<Caja> = formula
        .partes
        .iter()
        .map(|p| match &p.condicion {
            None => componer(&p.expresion, t, medir),
            Some(c) => fila(vec![
                componer(&p.expresion, t, medir),
                hueco(t * 0.6),
                letra("si", t * 0.85, false, medir),
                hueco(t * 0.4),
                componer(c, t * 0.85, medir),
            ]),
        })
        .collect();
    let col = columna(filas, t * 0.35);
    let alto = col.alto;
    fila(vec![
        cabeza,
        hueco(t * 0.3),
        llave(alto, t),
        hueco(t * 0.2),
        col,
    ])
}

/// La ecuacion como elementos, con su esquina en `(x, y)`.
pub fn elementos(
    nombre: &str,
    formula: &Compilada,
    x: f32,
    y: f32,
    estilo: &Estilo,
    medir: Medir<'_>,
) -> Vec<Elemento> {
    caja_de_la_ecuacion(nombre, formula, estilo.tam, medir).elementos(x, y, estilo)
}

// ---------------------------------------------------------------------
// Del arbol a cajas
// ---------------------------------------------------------------------

fn componer(n: &Nodo, t: f32, medir: Medir<'_>) -> Caja {
    match n {
        Nodo::Numero { texto, .. } => letra(texto, t, false, medir),
        Nodo::Variable(c) => letra(&c.to_string(), t, true, medir),
        Nodo::Constante { nombre, .. } => {
            let texto = if *nombre == "pi" { "π" } else { nombre };
            letra(texto, t, *nombre == "e", medir)
        }
        Nodo::Grupo(a) => entre_parentesis(componer(a, t, medir), t, medir),
        Nodo::Negado(a) => fila(vec![
            letra("−", t, false, medir),
            con_parentesis_si_suma(a, t, medir),
        ]),
        Nodo::Binaria {
            op,
            a,
            b,
            implicita,
        } => binaria(*op, a, b, *implicita, t, medir),
        Nodo::Funcion { nombre, args } => funcion(nombre, args, t, medir),
    }
}

fn binaria(op: char, a: &Nodo, b: &Nodo, implicita: bool, t: f32, medir: Medir<'_>) -> Caja {
    let signo = |s: &str| {
        fila(vec![
            hueco(t * 0.22),
            letra(s, t, false, medir),
            hueco(t * 0.22),
        ])
    };
    let entre = |s: &str| fila(vec![componer(a, t, medir), signo(s), componer(b, t, medir)]);
    match op {
        '+' => entre("+"),
        '-' => fila(vec![
            componer(a, t, medir),
            signo("−"),
            con_parentesis_si_suma(b, t, medir),
        ]),
        '*' if implicita => fila(vec![
            componer(a, t, medir),
            hueco(t * 0.08),
            componer(b, t, medir),
        ]),
        '*' => entre("·"),
        '/' => fraccion(componer(a, t, medir), componer(b, t, medir), t),
        '%' => entre("mod"),
        '^' => potencia(
            con_parentesis_si_compuesto(a, t, medir),
            componer(b, menor(t), medir),
            t,
        ),
        '<' => entre("<"),
        '>' => entre(">"),
        'l' => entre("≤"),
        'g' => entre("≥"),
        '=' => entre("="),
        _ => entre("≠"),
    }
}

fn funcion(nombre: &str, args: &[Nodo], t: f32, medir: Medir<'_>) -> Caja {
    match nombre {
        "sqrt" => raiz(componer(&args[0], t, medir), None, t),
        "cbrt" => raiz(
            componer(&args[0], t, medir),
            Some(letra("3", menor(t), false, medir)),
            t,
        ),
        "abs" => entre_barras(componer(&args[0], t, medir), t),
        "exp" => potencia(
            letra("e", t, true, medir),
            componer(&args[0], menor(t), medir),
            t,
        ),
        "pow" => potencia(
            con_parentesis_si_compuesto(&args[0], t, medir),
            componer(&args[1], menor(t), medir),
            t,
        ),
        "si" => fila(vec![
            componer(&args[1], t, medir),
            hueco(t * 0.5),
            letra("si", t * 0.85, false, medir),
            hueco(t * 0.3),
            componer(&args[0], t * 0.85, medir),
            hueco(t * 0.5),
            letra("si no", t * 0.85, false, medir),
            hueco(t * 0.3),
            componer(&args[2], t, medir),
        ]),
        "log2" => fila(vec![
            subindice(
                letra("log", t, false, medir),
                letra("2", menor(t), false, medir),
                t,
            ),
            entre_parentesis(componer(&args[0], t, medir), t, medir),
        ]),
        _ => {
            let visible = match nombre {
                "asin" => "arcsin",
                "acos" => "arccos",
                "atan" => "arctan",
                n => n,
            };
            let dentro = if args.len() == 1 {
                componer(&args[0], t, medir)
            } else {
                let mut v = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        v.push(letra(",", t, false, medir));
                        v.push(hueco(t * 0.2));
                    }
                    v.push(componer(a, t, medir));
                }
                fila(v)
            };
            fila(vec![
                letra(visible, t, false, medir),
                hueco(t * 0.05),
                entre_parentesis(dentro, t, medir),
            ])
        }
    }
}

fn con_parentesis_si_suma(n: &Nodo, t: f32, medir: Medir<'_>) -> Caja {
    let c = componer(n, t, medir);
    if matches!(n, Nodo::Binaria { op: '+' | '-', .. }) {
        entre_parentesis(c, t, medir)
    } else {
        c
    }
}

fn con_parentesis_si_compuesto(n: &Nodo, t: f32, medir: Medir<'_>) -> Caja {
    let c = componer(n, t, medir);
    if matches!(n, Nodo::Binaria { .. } | Nodo::Negado(_)) {
        entre_parentesis(c, t, medir)
    } else {
        c
    }
}

// ---------------------------------------------------------------------
// Las cajas
// ---------------------------------------------------------------------
//
// Publicas: el asistente (`pixpin-ia::ecuacion`) compone con ellas las
// ecuaciones que escribe en LaTeX o a mano, con letras que no son `x`, `y`,
// `t`. Un solo compositor para las dos entradas, no una copia.

/// El tamano de un exponente o un indice.
pub fn menor(t: f32) -> f32 {
    (t * ENCOGE).max(MINIMO)
}

/// Un texto suelto. Su eje esta donde una minuscula tiene la mitad de su
/// cuerpo.
pub fn letra(texto: &str, t: f32, cursiva: bool, medir: Medir<'_>) -> Caja {
    let (ancho, alto) = medir(texto, t);
    Caja {
        ancho,
        alto,
        eje: alto * 0.55,
        piezas: vec![Pieza::Texto {
            x: 0.0,
            y: 0.0,
            ancho,
            alto,
            texto: texto.to_string(),
            tam: t,
            cursiva,
            negrita: false,
            color: None,
        }],
    }
}

pub fn hueco(ancho: f32) -> Caja {
    Caja {
        ancho,
        alto: 0.0,
        eje: 0.0,
        piezas: Vec::new(),
    }
}

/// Cajas en fila, alineadas por su eje.
pub fn fila(cajas: Vec<Caja>) -> Caja {
    let arriba = cajas.iter().map(|c| c.eje).fold(0.0f32, f32::max);
    let abajo = cajas.iter().map(|c| c.alto - c.eje).fold(0.0f32, f32::max);
    let mut piezas = Vec::new();
    let mut cursor = 0.0;
    for c in &cajas {
        piezas.extend(c.piezas_en(cursor, arriba - c.eje));
        cursor += c.ancho;
    }
    Caja {
        ancho: cursor,
        alto: arriba + abajo,
        eje: arriba,
        piezas,
    }
}

/// Cajas en columna, alineadas a la izquierda; el eje en medio.
pub fn columna(cajas: Vec<Caja>, separacion: f32) -> Caja {
    let ancho = cajas.iter().map(|c| c.ancho).fold(0.0f32, f32::max);
    let alto = cajas.iter().map(|c| c.alto).sum::<f32>()
        + separacion * cajas.len().saturating_sub(1) as f32;
    let mut piezas = Vec::new();
    let mut cursor = 0.0;
    for c in &cajas {
        piezas.extend(c.piezas_en(0.0, cursor));
        cursor += c.alto + separacion;
    }
    Caja {
        ancho,
        alto,
        eje: alto / 2.0,
        piezas,
    }
}

/// El grosor de las rayas de la ecuacion: proporcional a la letra.
fn trazo(t: f32) -> f32 {
    (t / 14.0).clamp(0.8, 3.0)
}

fn raya(puntos: &[(f32, f32)], t: f32) -> Pieza {
    Pieza::Raya {
        puntos: puntos.iter().map(|&(x, y)| Punto2::nuevo(x, y)).collect(),
        grosor: trazo(t),
        color: None,
    }
}

/// Numerador sobre denominador, con la barra en el eje.
pub fn fraccion(arriba: Caja, abajo: Caja, t: f32) -> Caja {
    let margen = t * 0.15;
    let ancho = arriba.ancho.max(abajo.ancho) + margen * 2.0;
    let hueco = t * 0.18;
    let alto = arriba.alto + hueco * 2.0 + abajo.alto;
    let eje = arriba.alto + hueco;
    let mut piezas: Vec<Pieza> = arriba
        .piezas_en((ancho - arriba.ancho) / 2.0, 0.0)
        .collect();
    piezas.push(raya(&[(0.0, eje), (ancho, eje)], t));
    piezas.extend(abajo.piezas_en((ancho - abajo.ancho) / 2.0, eje + hueco));
    Caja {
        ancho,
        alto,
        eje,
        piezas,
    }
}

/// La base con el exponente pequeno y subido: su pie queda a media altura
/// de la base.
pub fn potencia(base: Caja, exponente: Caja, t: f32) -> Caja {
    let pie = base.alto * 0.45;
    let sobresale = (exponente.alto - pie).max(0.0);
    let hueco = t * 0.05;
    let mut piezas: Vec<Pieza> = base.piezas_en(0.0, sobresale).collect();
    piezas.extend(exponente.piezas_en(base.ancho + hueco, sobresale + pie - exponente.alto));
    Caja {
        ancho: base.ancho + hueco + exponente.ancho,
        alto: base.alto + sobresale,
        eje: base.eje + sobresale,
        piezas,
    }
}

/// La base con un indice pequeno y bajado.
pub fn subindice(base: Caja, indice: Caja, t: f32) -> Caja {
    // (Su centro algo por encima del borde de abajo de la base, 18-oct: con
    // la mitad por debajo caia tan bajo que en un renglon de texto pisaba el
    // de abajo, «P₍ₓ₎=x⁴+6 es un polinomio…».)
    let bajada = indice.alto * 0.72;
    let alto = base
        .alto
        .max(base.alto - base.eje + indice.alto - bajada + base.eje);
    let mut piezas: Vec<Pieza> = base.piezas_en(0.0, 0.0).collect();
    piezas.extend(indice.piezas_en(base.ancho + t * 0.05, base.alto - bajada));
    Caja {
        ancho: base.ancho + indice.ancho + t * 0.05,
        alto,
        eje: base.eje,
        piezas,
    }
}

/// El signo de la raiz dibujado con lineas —crece con lo que envuelve— y la
/// raya encima.
pub fn raiz(dentro: Caja, indice: Option<Caja>, t: f32) -> Caja {
    let margen = t * 0.15;
    let alto_dentro = dentro.alto + margen;
    let ancho_signo = t * 0.6;
    let ancho_indice = indice.as_ref().map_or(0.0, |i| i.ancho * 0.6);
    let ancho = ancho_indice + ancho_signo + dentro.ancho + margen;
    let alto = alto_dentro + margen;
    let x0 = ancho_indice;
    let mut piezas = vec![raya(
        &[
            (x0, alto * 0.6),
            (x0 + ancho_signo * 0.4, alto),
            (x0 + ancho_signo, 0.0),
            (x0 + ancho_signo + dentro.ancho + margen, 0.0),
        ],
        t,
    )];
    piezas.extend(dentro.piezas_en(x0 + ancho_signo + margen / 2.0, margen * 2.0));
    if let Some(i) = &indice {
        piezas.extend(i.piezas_en(0.0, alto * 0.25 - i.alto / 2.0));
    }
    Caja {
        ancho,
        alto,
        eje: dentro.eje + margen * 2.0,
        piezas,
    }
}

/// Entre parentesis del alto de lo de dentro: de letra si cabe, y si no
/// dibujados.
pub fn entre_parentesis(dentro: Caja, t: f32, medir: Medir<'_>) -> Caja {
    let abre = letra("(", t, false, medir);
    // Un exponente dentro (`(x^2 + 1)`) apenas lo sube: con la letra
    // todavia se lee mejor que dibujado.
    if dentro.alto <= abre.alto * 1.6 {
        return fila(vec![abre, dentro, letra(")", t, false, medir)]);
    }
    // Altos: dos arcos suaves (con tres puntos salian picos, como ⟨ ⟩).
    let ap = t * 0.35;
    let ancho = dentro.ancho + ap * 2.0 + t * 0.2;
    let h = dentro.alto;
    let xd = ancho - ap;
    let arco = |x0: f64, hacia: f64| -> Vec<(f32, f32)> {
        (0..=10)
            .map(|k| {
                let s = k as f64 / 10.0;
                (
                    (x0 + hacia * (std::f64::consts::PI * s).sin()) as f32,
                    (h as f64 * s) as f32,
                )
            })
            .collect()
    };
    let mut piezas = vec![raya(&arco(ap as f64, -(ap as f64) * 0.7), t)];
    piezas.extend(dentro.piezas_en(ap + t * 0.1, 0.0));
    piezas.push(raya(&arco(xd as f64, ap as f64 * 0.7), t));
    Caja {
        ancho,
        alto: dentro.alto,
        eje: dentro.eje,
        piezas,
    }
}

/// Entre barras verticales: el valor absoluto.
pub fn entre_barras(dentro: Caja, t: f32) -> Caja {
    let margen = t * 0.2;
    let ancho = dentro.ancho + margen * 4.0;
    let h = dentro.alto;
    let mut piezas = vec![raya(&[(margen, 0.0), (margen, h)], t)];
    piezas.extend(dentro.piezas_en(margen * 2.0, 0.0));
    piezas.push(raya(&[(ancho - margen, 0.0), (ancho - margen, h)], t));
    Caja {
        ancho,
        alto: dentro.alto,
        eje: dentro.eje,
        piezas,
    }
}

/// Entre corchetes del alto de lo de dentro, dibujados (una matriz).
pub fn entre_corchetes(dentro: Caja, t: f32) -> Caja {
    let m = t * 0.12;
    let pata = t * 0.22;
    let ancho = dentro.ancho + (m * 2.0 + pata) * 2.0;
    let h = dentro.alto;
    let xd = ancho - m;
    let mut piezas = vec![raya(&[(m + pata, 0.0), (m, 0.0), (m, h), (m + pata, h)], t)];
    piezas.extend(dentro.piezas_en(m * 2.0 + pata, 0.0));
    piezas.push(raya(
        &[(xd - pata, 0.0), (xd, 0.0), (xd, h), (xd - pata, h)],
        t,
    ));
    Caja {
        ancho,
        alto: h,
        eje: dentro.eje,
        piezas,
    }
}

/// **Una matriz o un sistema**: las celdas en filas y columnas, cada fila
/// alineada por su eje y cada columna del ancho de su celda mas ancha
/// (centradas, o a la izquierda en un sistema). Sin delimitadores: los pone
/// quien llama (`entre_corchetes`, `entre_parentesis`, `llave`...).
pub fn matriz(filas: Vec<Vec<Caja>>, t: f32, a_la_izquierda: bool) -> Caja {
    let columnas = filas.iter().map(Vec::len).max().unwrap_or(0);
    let mut anchos = vec![0.0f32; columnas];
    for f in &filas {
        for (j, c) in f.iter().enumerate() {
            anchos[j] = anchos[j].max(c.ancho);
        }
    }
    let entre_columnas = t * 0.9;
    let entre_filas = t * 0.3;
    let mut piezas = Vec::new();
    let mut y = 0.0f32;
    for (i, f) in filas.iter().enumerate() {
        if i > 0 {
            y += entre_filas;
        }
        let arriba = f.iter().map(|c| c.eje).fold(0.0f32, f32::max);
        let abajo = f.iter().map(|c| c.alto - c.eje).fold(0.0f32, f32::max);
        let mut x = 0.0f32;
        for (j, c) in f.iter().enumerate() {
            let dx = if a_la_izquierda {
                0.0
            } else {
                (anchos[j] - c.ancho) / 2.0
            };
            piezas.extend(c.piezas_en(x + dx, y + arriba - c.eje));
            x += anchos[j] + entre_columnas;
        }
        y += arriba + abajo;
    }
    let ancho = anchos.iter().sum::<f32>() + entre_columnas * columnas.saturating_sub(1) as f32;
    Caja {
        ancho,
        alto: y,
        eje: y / 2.0,
        piezas,
    }
}

/// Una llave de la altura de la columna de partes, dibujada con lineas.
pub fn llave(alto: f32, t: f32) -> Caja {
    let ancho = t * 0.45;
    let pico = t * 0.25;
    Caja {
        ancho,
        alto,
        eje: alto / 2.0,
        piezas: vec![raya(
            &[
                (ancho, 0.0),
                (ancho * 0.5, pico),
                (ancho * 0.5, alto / 2.0 - pico),
                (0.0, alto / 2.0),
                (ancho * 0.5, alto / 2.0 + pico),
                (ancho * 0.5, alto - pico),
                (ancho, alto),
            ],
            t,
        )],
    }
}

// ---------------------------------------------------------------------
// Adornos, recuadros y lo pequeno encima (15-oct-2026)
// ---------------------------------------------------------------------

/// Donde empieza y donde acaba **la tinta** de una caja en vertical, no su
/// caja: la caja de una letra lleva el interlineado, y una raya encima de
/// `AB` puesta sobre la caja quedaba a media letra de distancia.
///
/// Las letras se cuentan por su linea base estimada (el eje de la letra
/// mas un cuarto de su tamano) y su altura de mayuscula (siete decimos).
fn tinta_vertical(c: &Caja) -> (f32, f32) {
    let (mut arriba, mut abajo) = (f32::MAX, f32::MIN);
    for p in &c.piezas {
        match p {
            Pieza::Texto { y, alto, tam, .. } => {
                let base = y + alto * 0.55 + tam * 0.25;
                arriba = arriba.min(base - tam * 0.72);
                abajo = abajo.max(base);
            }
            Pieza::Raya { puntos, .. } => {
                for q in puntos {
                    arriba = arriba.min(q.y);
                    abajo = abajo.max(q.y);
                }
            }
        }
    }
    if arriba > abajo {
        (0.0, c.alto)
    } else {
        (arriba, abajo)
    }
}

/// **Una marca encima (o debajo) del ancho de lo que cubre**: la raya de
/// `AB`, el arco de `AC`, la flecha de un vector, el gorro, la tilde, el
/// subrayado. Pegada a la tinta y no a la caja; si no cabe, la caja crece.
pub fn adornada(dentro: Caja, adorno: crate::formulas::Adorno, t: f32) -> Caja {
    use crate::formulas::Adorno;
    let (arriba, abajo) = tinta_vertical(&dentro);
    let w = dentro.ancho;
    let m = t * 0.06;
    let (x0, x1) = (m, (w - m).max(m + 1.0));
    let separa = t * 0.1;
    let h = match adorno {
        Adorno::ArcoEncima => (t * 0.16 + w * 0.04).min(t * 0.32),
        Adorno::Gorro | Adorno::Tilde => t * 0.16,
        Adorno::Flecha => t * 0.12,
        Adorno::RayaEncima | Adorno::RayaDebajo | Adorno::LlaveDebajo => 0.0,
    };
    // La llave de `\underbrace`: dos tramos que bajan, van rectos y se juntan
    // en un pico en medio, bajo la tinta.
    if adorno == Adorno::LlaveDebajo {
        let y0 = abajo + t * 0.1;
        let alto_llave = t * 0.22;
        let (xm, y1) = ((x0 + x1) / 2.0, y0 + alto_llave);
        let medio = y0 + alto_llave * 0.55;
        let lado = |de: f32, a: f32| -> Vec<(f32, f32)> {
            (0..=10)
                .map(|k| {
                    let s = k as f32 / 10.0;
                    let y = if s < 0.15 {
                        y0 + (medio - y0) * (s / 0.15)
                    } else if s < 0.85 {
                        medio
                    } else {
                        medio + (y1 - medio) * ((s - 0.85) / 0.15)
                    };
                    (de + (a - de) * s, y)
                })
                .collect()
        };
        let mut piezas = dentro.piezas.clone();
        piezas.push(raya(&lado(x0, xm), t));
        piezas.push(raya(&lado(x1, xm), t));
        let alto = dentro.alto.max(y1 + trazo(t));
        return Caja {
            ancho: w,
            alto,
            eje: dentro.eje,
            piezas,
        };
    }
    if adorno == Adorno::RayaDebajo {
        let y = abajo + t * 0.12;
        let alto = dentro.alto.max(y + trazo(t));
        let mut piezas = dentro.piezas.clone();
        piezas.push(raya(&[(x0, y), (x1, y)], t));
        return Caja {
            ancho: w,
            alto,
            eje: dentro.eje,
            piezas,
        };
    }
    // La marca acaba `separa` por encima de la tinta; si se sale de la
    // caja, todo baja lo que falte.
    let fondo = arriba - separa;
    let baja = (h + trazo(t) - fondo).max(0.0);
    let fondo = fondo + baja;
    let tope = fondo - h;
    let mut piezas: Vec<Pieza> = dentro.piezas_en(0.0, baja).collect();
    let marca: Vec<(f32, f32)> = match adorno {
        Adorno::RayaEncima => vec![(x0, fondo), (x1, fondo)],
        Adorno::ArcoEncima => (0..=20)
            .map(|k| {
                let s = k as f32 / 20.0;
                // Un trozo de circulo achatado: sube rapido y se aplana arriba.
                // `max(0)`: el seno de pi en f32 sale un pelo negativo, y su
                // potencia es NaN (la raya entera desaparecia).
                (
                    x0 + (x1 - x0) * s,
                    fondo - h * (std::f32::consts::PI * s).sin().max(0.0).powf(0.8),
                )
            })
            .collect(),
        Adorno::Gorro => vec![(x0, fondo), ((x0 + x1) / 2.0, tope), (x1, fondo)],
        Adorno::Tilde => (0..=16)
            .map(|k| {
                let s = k as f32 / 16.0;
                (
                    x0 + (x1 - x0) * s,
                    tope + h / 2.0 - h / 2.0 * (2.0 * std::f32::consts::PI * s).sin(),
                )
            })
            .collect(),
        Adorno::Flecha => {
            let y = fondo - h / 2.0;
            piezas.push(raya(
                &[
                    (x1 - t * 0.18, y - t * 0.1),
                    (x1, y),
                    (x1 - t * 0.18, y + t * 0.1),
                ],
                t,
            ));
            vec![(x0, y), (x1, y)]
        }
        Adorno::RayaDebajo | Adorno::LlaveDebajo => Vec::new(),
    };
    piezas.push(raya(&marca, t));
    Caja {
        ancho: w,
        alto: dentro.alto + baja,
        eje: dentro.eje + baja,
        piezas,
    }
}

/// **Dentro de un recuadro** (`\boxed`): un margen alrededor de la tinta y
/// el marco de cuatro rayas.
pub fn recuadro(dentro: Caja, t: f32) -> Caja {
    let (arriba, abajo) = tinta_vertical(&dentro);
    let m = t * 0.25;
    let dy = (m - arriba).max(0.0);
    let ancho = dentro.ancho + m * 2.0;
    let (y0, y1) = (arriba + dy - m, abajo + dy + m);
    let mut piezas: Vec<Pieza> = dentro.piezas_en(m, dy).collect();
    piezas.push(raya(
        &[(0.0, y0), (ancho, y0), (ancho, y1), (0.0, y1), (0.0, y0)],
        t,
    ));
    Caja {
        ancho,
        alto: (dentro.alto + dy).max(y1 + trazo(t)),
        eje: dentro.eje + dy,
        piezas,
    }
}

/// Lo pequeno centrado encima de la base (`\overset{?}{=}`).
pub fn encima(base: Caja, arriba: Caja, t: f32) -> Caja {
    let ancho = base.ancho.max(arriba.ancho);
    let hueco = t * 0.02;
    let mut piezas: Vec<Pieza> = arriba
        .piezas_en((ancho - arriba.ancho) / 2.0, 0.0)
        .collect();
    let dy = arriba.alto * 0.7 + hueco;
    piezas.extend(base.piezas_en((ancho - base.ancho) / 2.0, dy));
    Caja {
        ancho,
        alto: base.alto + dy,
        eje: base.eje + dy,
        piezas,
    }
}

/// Los signos de angulo que se dibujan con rayas en vez de con la letra.
pub const ANGULOS: [char; 3] = ['∢', '∡', '∠'];

/// **Un signo de angulo dibujado** (`∢` de «m∢AOB», `∡`, `∠`). Con rayas y
/// no con la letra: casi ninguna fuente de texto los trae, y el de reserva
/// de Windows salia diminuto y torcido junto a las letras (lamina del
/// 15-oct); asi ademas sale igual en el PNG, el SVG y el PDF.
///
/// La caja es la de una letra del mismo tamano (`medir("m")`), con la base
/// donde la estiman los adornos (`tinta_vertical`).
pub fn angulo(signo: char, t: f32, medir: Medir<'_>) -> Caja {
    let (_, alto) = medir("m", t);
    let eje = alto * 0.55;
    let base = eje + t * 0.25;
    let w = t * 0.62;
    let (x0, x1) = (w * 0.1, w * 0.9);
    let arco = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| -> Vec<(f32, f32)> {
        (0..=10)
            .map(|k| {
                let a = a0 + (a1 - a0) * k as f32 / 10.0;
                (cx + r * a.cos(), cy - r * a.sin())
            })
            .collect()
    };
    let mut piezas = Vec::new();
    match signo {
        // Esferico: un «<» con un arco que corta los dos lados.
        '∢' => {
            let v = (x0, base - t * 0.3);
            let (arriba, abajo) = ((x1, base - t * 0.66), (x1, base + t * 0.04));
            piezas.push(raya(&[arriba, v, abajo], t));
            let a = ((arriba.1 - v.1) / (arriba.0 - v.0)).atan().abs();
            piezas.push(raya(&arco(v.0, v.1, w * 0.48, -a * 1.25, a * 1.25), t));
        }
        // Medido y plano: la base horizontal y el lado subiendo.
        _ => {
            let v = (x0, base);
            let sube = (x1 * 0.85, base - t * 0.68);
            piezas.push(raya(&[sube, v, (x1, base)], t));
            if signo == '∡' {
                let a = ((v.1 - sube.1) / (sube.0 - v.0)).atan();
                piezas.push(raya(&arco(v.0, v.1, w * 0.5, 0.0, a), t));
            }
        }
    }
    Caja {
        ancho: w,
        alto,
        eje,
        piezas,
    }
}

/// **De un arbol de formula de las notas** (`formulas::leer`, el LaTeX de
/// bolsillo y la escritura corriente) **a la caja compuesta**. Es el unico
/// compositor: lo usan la herramienta `escribir_ecuacion` del asistente
/// (antes vivia alli, en `pixpin-ia`) y el texto rico del lienzo.
///
/// `en_linea`: la formula va dentro de un renglon de texto, como `$...$`:
/// sus fracciones encogen para no abrir el renglon (las de `\dfrac` no).
pub fn de_formula(p: &crate::formulas::Pieza, t: f32, medir: Medir<'_>, en_linea: bool) -> Caja {
    use crate::formulas::Pieza as F;
    let sigue = |p: &F, t: f32| de_formula(p, t, medir, en_linea);
    match p {
        F::Texto { texto, cursiva } => {
            if texto.is_empty() {
                hueco(0.0)
            } else if texto.chars().any(|c| ANGULOS.contains(&c)) {
                // Los angulos, dibujados; lo de alrededor, en letra.
                let mut cajas = Vec::new();
                let mut trozo = String::new();
                for c in texto.chars() {
                    if ANGULOS.contains(&c) {
                        if !trozo.is_empty() {
                            cajas.push(letra(&std::mem::take(&mut trozo), t, *cursiva, medir));
                        }
                        cajas.push(angulo(c, t, medir));
                    } else {
                        trozo.push(c);
                    }
                }
                if !trozo.is_empty() {
                    cajas.push(letra(&trozo, t, *cursiva, medir));
                }
                fila(cajas)
            } else {
                letra(texto, t, *cursiva, medir)
            }
        }
        F::Fila(l) => fila(l.iter().map(|p| sigue(p, t)).collect()),
        F::Fraccion(a, b) => {
            // Una fraccion dentro de otra, mas pequena (como en un libro);
            // en un renglon de texto, ya de entrada mas pequena.
            let menor_ = if en_linea {
                (t * 0.78).max(MINIMO)
            } else {
                menor(t).max(t * 0.85)
            };
            fraccion(sigue(a, menor_), sigue(b, menor_), t)
        }
        F::Grande(d) => de_formula(d, t, medir, false),
        F::ConIndices {
            base,
            arriba,
            abajo,
        } => {
            let mut c = sigue(base, t);
            if let Some(b) = abajo {
                c = subindice(c, sigue(b, menor(t)), t);
            }
            if let Some(a) = arriba {
                c = potencia(c, sigue(a, menor(t)), t);
            }
            c
        }
        F::Raiz { dentro, indice } => raiz(
            sigue(dentro, t),
            indice.as_ref().map(|i| sigue(i, menor(menor(t)))),
            t,
        ),
        F::Agrupado {
            dentro,
            abre,
            cierra,
        } => {
            let d = sigue(dentro, t);
            match abre.as_str() {
                "(" => entre_parentesis(d, t, medir),
                "|" => entre_barras(d, t),
                _ => fila(vec![
                    letra(abre, t, false, medir),
                    d,
                    letra(cierra, t, false, medir),
                ]),
            }
        }
        // Matrices y sistemas: la rejilla de celdas y su delimitador
        // dibujado del alto de la rejilla.
        F::Tabla {
            filas,
            abre,
            cierra,
            izquierda,
        } => {
            let celdas = filas
                .iter()
                .map(|f| f.iter().map(|p| sigue(p, t)).collect())
                .collect();
            let m = matriz(celdas, t, *izquierda);
            match (abre.as_str(), cierra.as_str()) {
                ("[", _) => entre_corchetes(m, t),
                ("(", _) => entre_parentesis(m, t, medir),
                ("|", _) => entre_barras(m, t),
                ("‖", _) => entre_barras(entre_barras(m, t), t),
                ("{", "") => {
                    let alto = m.alto;
                    fila(vec![llave(alto, t), hueco(t * 0.3), m])
                }
                ("{", _) => entre_corchetes(m, t),
                _ => m,
            }
        }
        F::Adorno { dentro, adorno } => adornada(sigue(dentro, t), *adorno, t),
        F::Color { color, dentro } => {
            let c = sigue(dentro, t);
            match crate::texto_rico::color_de_nombre(color) {
                Some(col) => c.tenida(col),
                None => c,
            }
        }
        F::Negrita(d) => sigue(d, t).en_negrita(),
        F::Recuadro(d) => recuadro(sigue(d, t), t),
        F::Encima { base, arriba } => encima(sigue(base, t), sigue(arriba, menor(t)), t),
        F::Debajo { base, abajo } => debajo(sigue(base, t), sigue(abajo, menor(t)), t),
    }
}

/// Lo pequeno centrado debajo de la base (el rotulo de `\underbrace`).
pub fn debajo(base: Caja, abajo: Caja, t: f32) -> Caja {
    let ancho = base.ancho.max(abajo.ancho);
    let mut piezas: Vec<Pieza> = base.piezas_en((ancho - base.ancho) / 2.0, 0.0).collect();
    let dy = base.alto + t * 0.02;
    piezas.extend(abajo.piezas_en((ancho - abajo.ancho) / 2.0, dy));
    Caja {
        ancho,
        alto: dy + abajo.alto,
        eje: base.eje,
        piezas,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::formula::compilar;

    fn medir(texto: &str, tam: f32) -> (f32, f32) {
        (texto.chars().count() as f32 * tam * 0.6, tam * 1.2)
    }

    fn estilo() -> Estilo {
        Estilo {
            color: ColorRgba::opaco(0.0, 0.0, 0.0),
            tam: 20.0,
            opacidad: 1.0,
            familia: "Segoe UI".into(),
        }
    }

    fn textos(v: &[Elemento]) -> Vec<(String, f32, bool)> {
        v.iter()
            .filter_map(|e| match &e.figura {
                Figura::Texto { texto, tam, .. } => Some((texto.clone(), *tam, e.extras.cursiva)),
                _ => None,
            })
            .collect()
    }

    fn rayas(v: &[Elemento]) -> usize {
        v.iter()
            .filter(|e| matches!(e.figura, Figura::Linea { .. }))
            .count()
    }

    #[test]
    fn una_fraccion_lleva_su_barra_y_el_denominador_debajo() {
        let f = compilar("1/x").unwrap();
        let v = elementos("y", &f, 0.0, 0.0, &estilo(), &medir);
        assert_eq!(rayas(&v), 1, "la barra");
        let uno = v
            .iter()
            .find(|e| {
                textos(std::slice::from_ref(e))
                    .first()
                    .is_some_and(|t| t.0 == "1")
            })
            .unwrap();
        let x = v
            .iter()
            .find(|e| {
                textos(std::slice::from_ref(e))
                    .first()
                    .is_some_and(|t| t.0 == "x")
            })
            .unwrap();
        assert!(x.y > uno.y + 10.0, "la x va debajo del 1");
        // La x de la ecuacion va en cursiva y el numero no.
        assert!(x.extras.cursiva && !uno.extras.cursiva);
    }

    #[test]
    fn el_exponente_sale_pequeno_y_subido_y_no_se_escribe_el_circunflejo() {
        let f = compilar("x^2").unwrap();
        let v = elementos("y", &f, 0.0, 0.0, &estilo(), &medir);
        let t = textos(&v);
        assert!(t.iter().all(|(s, ..)| !s.contains('^')));
        let dos = v
            .iter()
            .find(|e| matches!(&e.figura, Figura::Texto { texto, .. } if texto == "2"))
            .unwrap();
        let x = v
            .iter()
            .find(|e| matches!(&e.figura, Figura::Texto { texto, .. } if texto == "x"))
            .unwrap();
        assert!(matches!(dos.figura, Figura::Texto { tam, .. } if tam < 20.0));
        assert!(dos.y < x.y, "el 2 va mas alto que la x");
        assert!(dos.x > x.x);
    }

    #[test]
    fn la_raiz_se_dibuja_con_una_raya_que_abarca_lo_de_dentro() {
        let f = compilar("sqrt(x+1)").unwrap();
        let c = caja_de_la_ecuacion("y", &f, 20.0, &medir);
        let v = c.elementos(0.0, 0.0, &estilo());
        assert_eq!(rayas(&v), 1);
        let signo = v
            .iter()
            .find(|e| matches!(e.figura, Figura::Linea { .. }))
            .unwrap();
        let mas_a_la_derecha = v
            .iter()
            .filter(|e| matches!(e.figura, Figura::Texto { .. }))
            .map(|e| e.x + e.ancho)
            .fold(0.0f32, f32::max);
        assert!(signo.x + signo.ancho >= mas_a_la_derecha - 0.5);
        // Caso negativo: nada de «sqrt» a maquina.
        assert!(textos(&v).iter().all(|(s, ..)| s != "sqrt"));
    }

    #[test]
    fn por_partes_lleva_llave_y_cada_parte_su_condicion() {
        let f = compilar("x^2 si x < 0; 2x si x >= 0").unwrap();
        let v = elementos("y", &f, 0.0, 0.0, &estilo(), &medir);
        assert_eq!(textos(&v).iter().filter(|(s, ..)| s == "si").count(), 2);
        assert!(textos(&v).iter().any(|(s, ..)| s == "≥"));
        // La llave: una raya de siete puntos.
        assert!(
            v.iter()
                .any(|e| matches!(&e.figura, Figura::Linea { puntos } if puntos.len() == 7))
        );
    }

    #[test]
    fn las_rayas_de_la_ecuacion_van_sin_temblor() {
        let f = compilar("|x|/2").unwrap();
        let v = elementos("y", &f, 10.0, 10.0, &estilo(), &medir);
        assert!(v.iter().all(|e| e.rugosidad == 0.0));
        assert!(rayas(&v) >= 3);
    }
}
