//! **La grafica de una funcion, escrita con su formula** (`object Graficas`
//! de `motor/Graficas.kt` del movil).
//!
//! Se teclea `sin(x)/x`, se dan los limites —de donde a donde va la `x` y que
//! ventana de `y` se ensena— y sale dibujada: la curva, los dos ejes con sus
//! marcas y su rotulo, todo como elementos normales del lienzo. Es dibujo, no
//! un cuadro incrustado: se mueve, se escala, se le cambia el color y sale en
//! el PDF, en el SVG, en la pagina web y en el movil como cualquier otra cosa.
//!
//! **Varias curvas en la misma grafica**: cada formula de la lista es una
//! curva con su color, sobre los mismos ejes. Y **funciones por partes**:
//! `x^2 si x < 0; 2x si x >= 0` (ver `formula.rs`).
//!
//! ## La curva
//!
//! Se muestrea la `x` a paso fijo y se **corta la polilinea** donde la funcion
//! se sale de la ventana o no existe: asi `tan(x)` sale como sus ramas y no
//! como una raya vertical que las une, y `1/x` deja el hueco en el cero. Cada
//! rama es una linea del lienzo sin temblor.

use crate::ecuacion::{self, Estilo, Medir, elemento_linea, elemento_texto};
use crate::elemento::{ColorRgba, Elemento, Figura};
use crate::formula::{self, Compilada};
use crate::vector::Punto2;

/// Lo que se pide para una grafica. Los limites son de la ventana que se
/// ensena.
#[derive(Debug, Clone, PartialEq)]
pub struct Peticion {
    /// Una formula por curva, en funcion de `x`.
    pub formulas: Vec<String>,
    pub x_desde: f64,
    pub x_hasta: f64,
    pub y_desde: f64,
    pub y_hasta: f64,
    /// Cuantos pixeles del dibujo mide una unidad.
    pub escala: f64,
}

impl Peticion {
    /// Una formula con la ventana de fabrica del movil: de -5 a 5 en los dos
    /// ejes, a 40 px la unidad.
    pub fn de(formula: &str) -> Peticion {
        Peticion {
            formulas: vec![formula.to_string()],
            x_desde: -5.0,
            x_hasta: 5.0,
            y_desde: -5.0,
            y_hasta: 5.0,
            escala: 40.0,
        }
    }
}

/// Cuantas muestras de `x` entre los dos limites.
pub const MUESTRAS: usize = 400;

/// Los colores de las curvas a partir de la segunda: la primera va con el
/// pincel, y las demas con estos, vivos y distintos entre si.
pub const COLORES_DE_CURVAS: [u32; 6] =
    [0xe03131, 0x1971c2, 0x2f9e44, 0xf08c00, 0x9c36b5, 0x0c8599];

/// Largo de la punta de los ejes, en px del dibujo.
pub const PUNTA: f32 = 8.0;
/// Apertura de cada ala de la punta, en grados.
const ANGULO_DE_LA_PUNTA: f32 = 22.0;

pub fn color_de(rgb: u32) -> ColorRgba {
    ColorRgba::opaco(
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
    )
}

/// Por que no sale una grafica: lo que se le dice al usuario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fallo {
    /// Ninguna formula escrita.
    SinFormula,
    /// Esta formula no se entiende (la primera que falla).
    NoSeEntiende(String),
    /// Usa otra variable que la `x`.
    OtraVariable(String),
    /// Los limites estan del reves, o son enormes.
    Limites,
}

/// Compila las formulas de la peticion, o dice cual falla.
pub fn compilar(p: &Peticion) -> Result<Vec<Compilada>, Fallo> {
    let mut v = Vec::new();
    for f in p
        .formulas
        .iter()
        .map(|f| f.trim())
        .filter(|f| !f.is_empty())
    {
        let c = formula::compilar(f).ok_or_else(|| Fallo::NoSeEntiende(f.to_string()))?;
        if c.variables().iter().any(|&v| v != 'x') {
            return Err(Fallo::OtraVariable(f.to_string()));
        }
        v.push(c);
    }
    if v.is_empty() {
        return Err(Fallo::SinFormula);
    }
    Ok(v)
}

/// Los elementos de la grafica, con su esquina en el `(0, 0)`, o el fallo.
pub fn elementos(p: &Peticion, estilo: &Estilo, medir: Medir<'_>) -> Result<Vec<Elemento>, Fallo> {
    let (mut trazado, leyenda) = partes(p, estilo, medir)?;
    trazado.extend(leyenda);
    Ok(trazado)
}

/// **Lo trazado y la leyenda, por separado.** La leyenda —la ecuacion de
/// cada curva, tipografiada y de su color— va en una columna **a la derecha
/// del area de los ejes**, apilada, y no dentro como en el movil (que la pone
/// en la esquina de arriba a la izquierda del trazado): alli una ecuacion por
/// partes, alta por su llave, bajaba hasta el eje x y tapaba sus numeros, y
/// cualquier leyenda se montaba sobre las curvas. Fuera no pisa nada y se
/// sigue leyendo junto a su curva por el color.
// `!(a > b)` es a proposito: tambien rechaza los NaN.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn partes(
    p: &Peticion,
    estilo: &Estilo,
    medir: Medir<'_>,
) -> Result<(Vec<Elemento>, Vec<Elemento>), Fallo> {
    let funciones = compilar(p)?;
    if !(p.x_hasta > p.x_desde) || !(p.y_hasta > p.y_desde) || !(p.escala > 0.0) {
        return Err(Fallo::Limites);
    }
    let ancho = (p.x_hasta - p.x_desde) * p.escala;
    let alto = (p.y_hasta - p.y_desde) * p.escala;
    if !ancho.is_finite() || !alto.is_finite() || ancho > 100_000.0 || alto > 100_000.0 {
        return Err(Fallo::Limites);
    }
    let px = |x: f64| ((x - p.x_desde) * p.escala) as f32;
    let py = |y: f64| ((p.y_hasta - y) * p.escala) as f32;
    let (ancho, alto) = (ancho as f32, alto as f32);

    let mut salida = Vec::new();
    // Los ejes, finos: son referencia, no dibujo.
    // La letra sale de la del pincel, como en el movil (`fontSize * 0.7`),
    // pero **sin pasar de la de fabrica** (`LETRA_MAXIMA`): con el pincel en
    // 36 los numeros salian de 25 y los rotulos se comian la grafica. Y los
    // numeros de las marcas, un punto mas pequenos que en el movil
    // (`PARTE_DE_LOS_NUMEROS`): en Excalifont se veian tan grandes como la
    // ecuacion, y el usuario los pidio mas pequenos.
    let base = estilo.tam.min(LETRA_MAXIMA);
    let de_letra = estilo.con_tam((base * 0.7).max(8.0));
    let tam_numeros = (base * PARTE_DE_LOS_NUMEROS).max(8.0);
    let hay_eje_x = p.y_desde <= 0.0 && 0.0 <= p.y_hasta;
    let hay_eje_y = p.x_desde <= 0.0 && 0.0 <= p.x_hasta;
    let y_eje_x = if hay_eje_x { py(0.0) } else { py(p.y_desde) };
    let x_eje_y = if hay_eje_y { px(0.0) } else { px(p.x_desde) };

    salida.extend(flecha((0.0, y_eje_x), (ancho, y_eje_x), estilo));
    salida.extend(flecha((x_eje_y, alto), (x_eje_y, 0.0), estilo));
    // El nombre de cada eje, en cursiva como las variables de la ecuacion,
    // pegado a la punta donde no choca con los numeros de las marcas.
    let tam_variable = de_letra.tam * 1.15;
    salida.push(rotulo(
        "x",
        (ancho + 4.0, y_eje_x),
        tam_variable,
        true,
        estilo,
        medir,
        Lado::Derecha,
        false,
    ));
    salida.push(rotulo(
        "y",
        (x_eje_y + 5.0, 0.0),
        tam_variable,
        true,
        estilo,
        medir,
        Lado::Derecha,
        true,
    ));

    // Las marcas, a un paso «bonito»: ni dos ni doscientas.
    let paso_x = paso_bonito(p.x_hasta - p.x_desde);
    let paso_y = paso_bonito(p.y_hasta - p.y_desde);
    let marca = 4.0f32;
    let mut x = (p.x_desde / paso_x).ceil() * paso_x;
    while x <= p.x_hasta + 1e-9 {
        if x.abs() > 1e-9 || !hay_eje_y {
            salida.push(raya(
                (px(x), y_eje_x - marca),
                (px(x), y_eje_x + marca),
                estilo,
            ));
            salida.push(rotulo(
                &numero(x),
                (px(x), y_eje_x + marca + 2.0),
                tam_numeros,
                false,
                estilo,
                medir,
                Lado::Centro,
                true,
            ));
        }
        x += paso_x;
    }
    let mut y = (p.y_desde / paso_y).ceil() * paso_y;
    while y <= p.y_hasta + 1e-9 {
        if y.abs() > 1e-9 || !hay_eje_x {
            salida.push(raya(
                (x_eje_y - marca, py(y)),
                (x_eje_y + marca, py(y)),
                estilo,
            ));
            salida.push(rotulo(
                &numero(y),
                (x_eje_y - marca - 2.0, py(y)),
                tam_numeros,
                false,
                estilo,
                medir,
                Lado::Izquierda,
                false,
            ));
        }
        y += paso_y;
    }

    // Las curvas, por ramas, cada una con su color, y su rotulo tipografiado
    // en la columna de la leyenda, pasada la «x» del eje.
    let x_leyenda = ancho + 4.0 + medir("x", tam_variable).0 + SEPARACION_DE_LA_LEYENDA;
    let mut leyenda = Vec::new();
    let mut altura_del_rotulo = 0.0f32;
    for (k, f) in funciones.iter().enumerate() {
        let color = if k == 0 {
            estilo.color
        } else {
            color_de(COLORES_DE_CURVAS[(k - 1) % COLORES_DE_CURVAS.len()])
        };
        let de_curva = estilo.con_color(color);
        let mut rama: Vec<Punto2> = Vec::new();
        let cerrar = |rama: &mut Vec<Punto2>, salida: &mut Vec<Elemento>| {
            if rama.len() >= 2 {
                salida.push(elemento_linea(
                    std::mem::take(rama),
                    grosor_de_curva(estilo),
                    &de_curva,
                ));
            }
            rama.clear();
        };
        for i in 0..=MUESTRAS {
            let xv = p.x_desde + (p.x_hasta - p.x_desde) * i as f64 / MUESTRAS as f64;
            let yv = f.en(xv);
            if !yv.is_finite() || yv < p.y_desde || yv > p.y_hasta {
                cerrar(&mut rama, &mut salida);
                continue;
            }
            rama.push(Punto2::nuevo(px(xv), py(yv)));
        }
        cerrar(&mut rama, &mut salida);

        let letra = estilo.con_color(color);
        let caja = ecuacion::caja_de_la_ecuacion("y", f, de_letra.tam * 1.15, medir);
        leyenda.extend(caja.elementos(x_leyenda, altura_del_rotulo, &letra));
        altura_del_rotulo += caja.alto + 8.0;
    }
    Ok((salida, leyenda))
}

/// La letra del pincel de la que sale la grafica, como mucho: la de fabrica
/// del movil (`ItemStyle.fontSize` = 20). Mas grande no la agranda.
pub const LETRA_MAXIMA: f32 = 20.0;

/// Los numeros de las marcas, respecto a esa letra: 12 con la de fabrica,
/// frente a los 16 de la ecuacion (el movil los pone a 0,7: 14).
pub const PARTE_DE_LOS_NUMEROS: f32 = 0.6;

/// Lo que se aparta la columna de la leyenda del final del eje x y su letra.
pub const SEPARACION_DE_LA_LEYENDA: f32 = 24.0;

/// El grueso de la curva: el del pincel, como en el movil (`strokeWidth` del
/// estilo), y los ejes a uno.
fn grosor_de_curva(estilo: &Estilo) -> f32 {
    // `Estilo` no lleva grueso: la curva va a dos, el `strokeWidth` de
    // fabrica de Excalidraw, que es lo que lleva el pincel del movil.
    let _ = estilo;
    2.0
}

/// Un paso de marca redondo (1, 2, 5, 10...) que deje entre cinco y diez
/// marcas.
pub fn paso_bonito(rango: f64) -> f64 {
    if rango <= 0.0 || !rango.is_finite() {
        return 1.0;
    }
    let crudo = rango / 8.0;
    let potencia = 10f64.powf(crudo.log10().floor());
    let mantisa = crudo / potencia;
    let redonda = if mantisa < 1.5 {
        1.0
    } else if mantisa < 3.5 {
        2.0
    } else if mantisa < 7.5 {
        5.0
    } else {
        10.0
    };
    redonda * potencia
}

fn numero(v: f64) -> String {
    let r = if v.abs() < 1e-9 { 0.0 } else { v };
    if r == r.floor() && r.abs() < 1e9 {
        format!("{}", r as i64)
    } else {
        let s = format!("{r:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn raya(a: (f32, f32), b: (f32, f32), estilo: &Estilo) -> Elemento {
    elemento_linea(
        vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
        1.0,
        estilo,
    )
}

/// Un eje: la raya de `a` a `b` y una uve pequena en `b`. No es una flecha
/// del lienzo porque su punta mide 25 px fijos —lo que pide una flecha de
/// diagrama— y en un eje fino se comia las marcas y los numeros.
fn flecha(a: (f32, f32), b: (f32, f32), estilo: &Estilo) -> Vec<Elemento> {
    let largo = (b.0 - a.0).hypot(b.1 - a.1);
    if largo == 0.0 {
        return Vec::new();
    }
    let (nx, ny) = ((b.0 - a.0) / largo, (b.1 - a.1) / largo);
    let base = Punto2::nuevo(b.0 - nx * PUNTA, b.1 - ny * PUNTA);
    let punta = Punto2::nuevo(b.0, b.1);
    let rad = ANGULO_DE_LA_PUNTA.to_radians();
    let ala1 = base.girar(punta, -rad);
    let ala2 = base.girar(punta, rad);
    vec![
        raya(a, b, estilo),
        elemento_linea(vec![ala1, punta, ala2], 1.0, estilo),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lado {
    Izquierda,
    Centro,
    Derecha,
}

/// Una letra colocada respecto de un punto: encima o centrada en alto, a un
/// lado u otro.
#[allow(clippy::too_many_arguments)]
fn rotulo(
    texto: &str,
    donde: (f32, f32),
    tam: f32,
    cursiva: bool,
    estilo: &Estilo,
    medir: Medir<'_>,
    lado: Lado,
    arriba: bool,
) -> Elemento {
    let (ancho, alto) = medir(texto, tam);
    let x = match lado {
        Lado::Izquierda => donde.0 - ancho,
        Lado::Derecha => donde.0,
        Lado::Centro => donde.0 - ancho / 2.0,
    };
    let y = if arriba {
        donde.1
    } else {
        donde.1 - alto / 2.0
    };
    elemento_texto(texto, x, y, (ancho, alto), tam, cursiva, estilo)
}

/// Si un elemento es una curva (y no un eje, una marca o una punta): lo usan
/// las pruebas y quien quiera recolorear las curvas.
pub fn es_curva(e: &Elemento) -> bool {
    matches!(&e.figura, Figura::Linea { puntos } if puntos.len() > 3)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn medir(texto: &str, tam: f32) -> (f32, f32) {
        (texto.chars().count() as f32 * tam * 0.6, tam * 1.2)
    }

    fn estilo() -> Estilo {
        Estilo {
            color: ColorRgba::opaco(0.12, 0.12, 0.12),
            tam: 20.0,
            opacidad: 1.0,
            familia: "Segoe UI".into(),
        }
    }

    fn texto_de(e: &Elemento) -> Option<&str> {
        match &e.figura {
            Figura::Texto { texto, .. } => Some(texto),
            _ => None,
        }
    }

    fn caja(v: &[&Elemento]) -> (f32, f32, f32, f32) {
        v.iter()
            .fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |c, e| {
                let (a, b, x, y) = e.caja();
                (c.0.min(a), c.1.min(b), c.2.max(x), c.3.max(y))
            })
    }

    #[test]
    fn una_grafica_trae_ejes_marcas_rotulo_y_curva() {
        let p = Peticion {
            formulas: vec!["x^2".into()],
            x_desde: -3.0,
            x_hasta: 3.0,
            y_desde: -1.0,
            y_hasta: 9.0,
            escala: 40.0,
        };
        let e = elementos(&p, &estilo(), &medir).unwrap();
        // Las puntas de los ejes: una uve de tres puntos, pequena.
        let puntas: Vec<_> = e
            .iter()
            .filter(|x| matches!(&x.figura, Figura::Linea { puntos } if puntos.len() == 3))
            .collect();
        assert_eq!(puntas.len(), 2);
        assert!(
            puntas
                .iter()
                .all(|x| x.ancho <= PUNTA + 0.01 && x.alto <= PUNTA + 0.01)
        );
        let letras: Vec<_> = e
            .iter()
            .filter(|x| matches!(texto_de(x), Some("x" | "y")) && x.extras.cursiva)
            .collect();
        assert!(
            letras
                .iter()
                .any(|x| texto_de(x) == Some("x") && x.x >= 240.0)
        );
        assert!(
            letras
                .iter()
                .any(|x| texto_de(x) == Some("y") && x.y >= 0.0 && x.y < 4.0)
        );
        // El rotulo va tipografiado: «y =» y el 2 de exponente, sin «^».
        assert!(e.iter().any(|x| texto_de(x) == Some("y =")));
        assert!(e.iter().all(|x| !texto_de(x).unwrap_or("").contains('^')));
        // La parabola entra entera: una sola rama, con todas sus muestras.
        let curvas: Vec<_> = e.iter().filter(|x| es_curva(x)).collect();
        assert_eq!(curvas.len(), 1);
        match &curvas[0].figura {
            Figura::Linea { puntos } => assert_eq!(puntos.len(), MUESTRAS + 1),
            _ => unreachable!(),
        }
        assert!(curvas.iter().all(|c| c.rugosidad == 0.0));
        let c = caja(&curvas);
        assert!(
            c.0 >= -1.0 && c.1 >= -1.0 && c.2 <= 241.0 && c.3 <= 401.0,
            "{c:?}"
        );
        let (trazado, _) = partes(&p, &estilo(), &medir).unwrap();
        let todo = caja(&trazado.iter().collect::<Vec<_>>());
        assert!(
            todo.0 > -60.0 && todo.1 > -20.0 && todo.2 < 300.0 && todo.3 < 440.0,
            "{todo:?}"
        );
    }

    #[test]
    fn una_asintota_parte_la_curva_en_ramas() {
        let p = Peticion {
            formulas: vec!["1/x".into()],
            x_desde: -4.0,
            x_hasta: 4.0,
            y_desde: -4.0,
            y_hasta: 4.0,
            escala: 30.0,
        };
        let e = elementos(&p, &estilo(), &medir).unwrap();
        let curvas: Vec<_> = e.iter().filter(|x| es_curva(x)).collect();
        assert_eq!(curvas.len(), 2);
        for c in curvas {
            if let Figura::Linea { puntos } = &c.figura {
                assert!(puntos.windows(2).all(|w| (w[0].y - w[1].y).abs() < 240.0));
            }
        }
    }

    #[test]
    fn varias_formulas_son_varias_curvas_con_su_color() {
        let p = Peticion {
            formulas: vec!["x".into(), "-x".into()],
            x_desde: -2.0,
            x_hasta: 2.0,
            y_desde: -2.0,
            y_hasta: 2.0,
            escala: 30.0,
        };
        let e = elementos(&p, &estilo(), &medir).unwrap();
        let curvas: Vec<_> = e.iter().filter(|x| es_curva(x)).collect();
        assert_eq!(curvas.len(), 2);
        assert_eq!(curvas[0].trazo, estilo().color);
        assert_eq!(curvas[1].trazo, color_de(COLORES_DE_CURVAS[0]));
        assert_eq!(e.iter().filter(|x| texto_de(x) == Some("y =")).count(), 2);
    }

    #[test]
    fn sin_formula_con_otra_variable_o_con_limites_del_reves_no_hay_grafica() {
        assert_eq!(
            elementos(&Peticion::de("nada(x)"), &estilo(), &medir).unwrap_err(),
            Fallo::NoSeEntiende("nada(x)".into())
        );
        assert_eq!(
            elementos(&Peticion::de("x + y"), &estilo(), &medir).unwrap_err(),
            Fallo::OtraVariable("x + y".into())
        );
        assert_eq!(
            elementos(&Peticion::de("  "), &estilo(), &medir).unwrap_err(),
            Fallo::SinFormula
        );
        let mut p = Peticion::de("x");
        p.x_desde = 5.0;
        p.x_hasta = -5.0;
        assert_eq!(
            elementos(&p, &estilo(), &medir).unwrap_err(),
            Fallo::Limites
        );
        assert!(elementos(&Peticion::de("x"), &estilo(), &medir).is_ok());
    }

    fn se_tocan(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
        a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
    }

    #[test]
    fn ninguna_leyenda_pisa_el_area_de_los_ejes_ni_sus_marcas_ni_sus_numeros() {
        // El caso que lo destapo: una por partes, alta por su llave, que bajaba
        // hasta el eje x y tapaba el -4 y el -2; y tres curvas apiladas.
        let p = Peticion {
            formulas: vec![
                "sin(x)/x".into(),
                "sqrt(x+4) - 1".into(),
                "x^2/4 si x < 0; -x/2 si x >= 0".into(),
            ],
            x_desde: -6.0,
            x_hasta: 6.0,
            y_desde: -3.0,
            y_hasta: 3.0,
            escala: 50.0,
        };
        let (trazado, leyenda) = partes(&p, &estilo(), &medir).unwrap();
        assert!(
            leyenda
                .iter()
                .filter(|e| texto_de(e) == Some("y ="))
                .count()
                == 3
        );
        let area = (0.0, 0.0, 600.0, 300.0);
        for l in &leyenda {
            let c = l.caja();
            assert!(
                !se_tocan(c, area),
                "la leyenda {:?} cruza el area de los ejes",
                l.figura
            );
            for t in &trazado {
                assert!(!se_tocan(c, t.caja()), "la leyenda pisa {:?}", t.figura);
            }
        }
        // Y las leyendas no se pisan entre si: van apiladas.
        let ys: Vec<f32> = leyenda
            .iter()
            .filter(|e| texto_de(e) == Some("y ="))
            .map(|e| e.y)
            .collect();
        assert!(ys.windows(2).all(|w| w[1] > w[0] + 10.0), "{ys:?}");
    }

    #[test]
    fn el_paso_de_las_marcas_es_redondo() {
        for (r, p) in [(10.0, 1.0), (4.0, 0.5), (100.0, 10.0), (0.2, 0.02)] {
            assert!((paso_bonito(r) - p).abs() < 1e-12, "{r}");
        }
    }

    #[test]
    fn los_numeros_de_las_marcas_salen_sin_ceros_de_mas() {
        assert_eq!(numero(2.0), "2");
        assert_eq!(numero(-0.5), "-0.5");
        assert_eq!(numero(1e-12), "0");
        assert_eq!(numero(0.125), "0.125");
    }

    /// El tamano de letra de los numeros de los ejes y el de la ecuacion.
    fn tamanos(tam_pincel: f32) -> (f32, f32) {
        let p = Peticion {
            formulas: vec!["x^2/4".into()],
            x_desde: -5.0,
            x_hasta: 5.0,
            y_desde: -3.0,
            y_hasta: 3.0,
            escala: 40.0,
        };
        let (trazado, leyenda) = partes(
            &p,
            &Estilo {
                tam: tam_pincel,
                ..estilo()
            },
            &medir,
        )
        .unwrap();
        let tam = |e: &Elemento| match &e.figura {
            Figura::Texto { tam, .. } => Some(*tam),
            _ => None,
        };
        let numeros = trazado
            .iter()
            .filter(|e| texto_de(e).is_some_and(|t| t.parse::<f64>().is_ok()))
            .filter_map(tam)
            .fold(0.0f32, f32::max);
        let ecuacion = leyenda.iter().filter_map(tam).fold(0.0f32, f32::max);
        (numeros, ecuacion)
    }

    #[test]
    fn los_numeros_de_los_ejes_son_mas_pequenos_que_la_ecuacion() {
        // Pedido del usuario: «el de los numeros tiene que ser mas pequeno».
        let (numeros, ecuacion) = tamanos(20.0);
        assert!((numeros - 12.0).abs() < 0.01, "{numeros}");
        assert!(ecuacion > numeros * 1.25, "{ecuacion} frente a {numeros}");
    }

    #[test]
    fn un_pincel_de_letra_gorda_no_agranda_la_grafica_de_mas() {
        // Con la letra del pincel en 36 salian numeros de 25: la grafica se
        // comia con sus rotulos. Pasado el 20 de fabrica ya no crecen.
        assert_eq!(tamanos(36.0), tamanos(20.0));
        // Caso negativo: con un pincel pequeno si se achica.
        let (numeros, _) = tamanos(16.0);
        assert!(numeros < 12.0, "{numeros}");
    }
}
