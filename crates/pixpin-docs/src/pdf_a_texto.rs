//! **Un PDF pasado a texto, para leerlo como un Word** (Android v0.92
//! `800db67` y dos columnas v0.93 `a6153f8`: `motor/PdfAHtml.kt`). El
//! usuario: «el PDF lo puedes transformar como texto… lo extraes, y tambien
//! tengo la opcion de mostrarlo como un PDF normal».
//!
//! Los trozos de texto con su sitio y su tamano los saca `pixpin-pdf`
//! (`texto::trozos_de_bytes`), que ya lee el PDF por dentro para buscar.
//! Aqui, sin saber nada de PDF, se juntan en **lineas** (lo que esta a la
//! misma altura), las lineas en **parrafos** (por el salto entre ellas, el
//! cambio de tamano, la sangria y el punto final), se deshacen los guiones
//! de fin de linea, lo mas grande se marca como titulo y se quitan los
//! numeros de pagina sueltos. **A dos columnas** se busca el pasillo vacio
//! del medio y se lee la izquierda entera y luego la derecha.
//!
//! El resultado es la misma pagina HTML que escribe el movil, asi que el
//! lector de Word (`visor.rs`) la trata como cualquier otra: letra, voz,
//! pagina que sube sola, anotar. Cada hoja empieza con su marca «Hoja N»
//! (`hoja-pdf`), que se ve en gris y no se lee en voz alta.
//!
//! Limites, dichos: un **escaneo** no tiene texto (es una foto) y lo dice
//! quien llama.

use crate::documento::escapar;

/// Un trozo de texto en el papel, en puntos desde arriba a la izquierda:
/// `x`..`fin` a lo ancho, `y` la linea base, `alto` el tamano de la letra.
#[derive(Debug, Clone, PartialEq)]
pub struct Trozo {
    pub x: f64,
    pub y: f64,
    pub fin: f64,
    pub alto: f64,
    pub texto: String,
    pub negrita: bool,
    pub cursiva: bool,
}

impl Trozo {
    pub fn nuevo(x: f64, y: f64, fin: f64, alto: f64, texto: &str) -> Self {
        Self {
            x,
            y,
            fin,
            alto,
            texto: texto.to_string(),
            negrita: false,
            cursiva: false,
        }
    }
}

/// Una linea: `columna` 0 es de ancho completo; 1 y 2, la columna izquierda
/// y la derecha.
#[derive(Debug, Clone, PartialEq)]
pub struct Linea {
    pub y: f64,
    pub x0: f64,
    pub x1: f64,
    pub alto: f64,
    pub texto: String,
    pub negrita: bool,
    pub columna: u8,
}

/// Un parrafo ya hecho: `nivel` 0 es texto; 1, 2 o 3, titulo de ese nivel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parrafo {
    pub texto: String,
    pub nivel: u8,
    pub pagina: usize,
}

/// Las palabras que se cuentan por hoja de un escaneo, lo de una pagina
/// corriente: con ellas la pagina que sube sola tiene velocidad igual.
pub const POR_HOJA_SI_NO_HAY: usize = 250;

/// Los signos con los que acaba una frase.
const FINALES: [char; 8] = ['.', ':', '!', '?', '…', '»', '"', '”'];

fn acaba_en_final(t: &str) -> bool {
    t.chars().last().is_some_and(|c| FINALES.contains(&c))
}

/// La mediana de los tamanos (el de la mitad, ordenados), como el movil.
fn mediana_de_altos(trozos: &[Trozo]) -> f64 {
    let mut altos: Vec<f64> = trozos.iter().map(|t| t.alto).collect();
    altos.sort_by(f64::total_cmp);
    altos[trozos.len() / 2]
}

/// **Las lineas**: lo que esta a la misma altura, de izquierda a derecha,
/// con espacios donde hay hueco.
pub fn lineas(trozos: &[Trozo]) -> Vec<Linea> {
    let mut ordenados: Vec<&Trozo> = trozos.iter().collect();
    ordenados.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let mut grupos: Vec<Vec<&Trozo>> = Vec::new();
    for t in ordenados {
        if let Some(g) = grupos.last_mut() {
            let y_de = g.iter().map(|t| t.y).sum::<f64>() / g.len() as f64;
            let alto_de = g.iter().map(|t| t.alto).fold(f64::MIN, f64::max);
            if (t.y - y_de).abs() <= 0.45 * t.alto.max(alto_de) {
                g.push(t);
                continue;
            }
        }
        grupos.push(vec![t]);
    }
    grupos
        .into_iter()
        .map(|g| {
            let mut en_orden = g.clone();
            en_orden.sort_by(|a, b| a.x.total_cmp(&b.x));
            let mut s = String::new();
            let mut antes: Option<&Trozo> = None;
            for t in &en_orden {
                if let Some(a) = antes {
                    let hueco = t.x - a.fin;
                    let ya_hay = s.ends_with(' ') || t.texto.starts_with(' ');
                    if !ya_hay && (hueco > 0.12 * t.alto || hueco < -0.5 * t.alto) {
                        s.push(' ');
                    }
                }
                s.push_str(&t.texto);
                antes = Some(t);
            }
            // El tamano que mas se repite; con empate, el primero que salio.
            let mut cuentas: Vec<(f64, usize)> = Vec::new();
            for t in &en_orden {
                match cuentas.iter_mut().find(|(a, _)| *a == t.alto) {
                    Some(c) => c.1 += 1,
                    None => cuentas.push((t.alto, 1)),
                }
            }
            let alto = cuentas
                .iter()
                .fold(cuentas[0], |m, c| if c.1 > m.1 { *c } else { m })
                .0;
            Linea {
                y: g.iter().map(|t| t.y).sum::<f64>() / g.len() as f64,
                x0: en_orden[0].x,
                x1: en_orden.iter().map(|t| t.fin).fold(f64::MIN, f64::max),
                alto,
                texto: s.split_whitespace().collect::<Vec<_>>().join(" "),
                negrita: en_orden.iter().all(|t| t.negrita),
                columna: 0,
            }
        })
        .filter(|l| !l.texto.is_empty())
        .collect()
}

/// **El pasillo entre dos columnas**, si lo hay: la x de su centro. Se
/// busca, hacia el medio de lo escrito, una franja vertical por la que casi
/// no pasa texto —solo algun titulo que la cruza— con bastante texto a los
/// dos lados. Nada si la hoja va a una columna.
pub fn pasillo_de_columnas(trozos: &[Trozo]) -> Option<f64> {
    if trozos.len() < 8 {
        return None;
    }
    let x0 = trozos.iter().map(|t| t.x).fold(f64::MAX, f64::min);
    let x1 = trozos.iter().map(|t| t.fin).fold(f64::MIN, f64::max);
    let ancho = x1 - x0;
    if ancho <= 0.0 {
        return None;
    }
    const CUBOS: usize = 200;
    let paso = ancho / CUBOS as f64;
    let mut cobertura = [0usize; CUBOS];
    let cubo = |v: f64| (((v - x0) / paso) as i64).clamp(0, CUBOS as i64 - 1) as usize;
    for t in trozos {
        for c in &mut cobertura[cubo(t.x)..=cubo(t.fin).max(cubo(t.x))] {
            *c += 1;
        }
    }
    let maximo = *cobertura.iter().max().unwrap_or(&0);
    // «Casi no pasa texto»: hasta un quinto de lo que pasa por lo mas lleno
    // (el movil, un decimo). Con los renglones ya juntos ([`corridos`]) una
    // hoja de LaTeX da 19 por columna y el titulo, el autor y un encabezado
    // cruzan el pasillo: 3, mas de un decimo (medido).
    let vacio = (maximo / 5).max(1);
    let cuerpo = mediana_de_altos(trozos);
    // La franja vacia mas ancha con el centro entre el 30 % y el 70 % de lo
    // escrito. Al menos 0,6 letras de ancho (el movil pide 0,8): el pasillo
    // de LaTeX es de 10 puntos con letra de 10, y al contarlo en cubos se
    // pierde un cubo por cada lado (medido: 7 puntos).
    let mut mejor: Option<(usize, usize)> = None;
    let mut i = 0;
    while i < CUBOS {
        if cobertura[i] > vacio {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < CUBOS && cobertura[j + 1] <= vacio {
            j += 1;
        }
        let centro = (i + j) as f64 / 2.0 / CUBOS as f64;
        if (0.3..=0.7).contains(&centro)
            && (j - i + 1) as f64 * paso >= cuerpo * 0.6
            && mejor.is_none_or(|(a, b)| j - i > b - a)
        {
            mejor = Some((i, j));
        }
        i = j + 1;
    }
    let (a, b) = mejor?;
    let pasillo = x0 + (a + b + 1) as f64 / 2.0 * paso;
    // Y texto de verdad a los dos lados: una columna de cuatro palabras no es una columna.
    let letras = |t: &Trozo| t.texto.chars().count();
    let izquierda: usize = trozos.iter().filter(|t| t.fin <= pasillo).map(letras).sum();
    let derecha: usize = trozos.iter().filter(|t| t.x >= pasillo).map(letras).sum();
    let total = trozos.iter().map(letras).sum::<usize>().max(1) as f64;
    (izquierda as f64 > total * 0.2 && derecha as f64 > total * 0.2).then_some(pasillo)
}

/// **Las palabras seguidas de un renglon, en un trozo**: misma linea base,
/// mismo tamano y un hueco menor que media letra (lo que mide un espacio,
/// aunque el renglon este justificado). El pasillo entre dos columnas es mas
/// ancho que eso, asi que nunca se juntan las dos columnas.
pub fn corridos(trozos: &[Trozo]) -> Vec<Trozo> {
    let mut ordenados: Vec<&Trozo> = trozos.iter().collect();
    ordenados.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let mut sale: Vec<Trozo> = Vec::with_capacity(ordenados.len());
    // El ultimo trozo de cada renglon abierto: el que se puede seguir.
    let mut abiertos: Vec<usize> = Vec::new();
    for t in ordenados {
        let sigue = abiertos.iter().position(|&k| {
            let a = &sale[k];
            let alto = a.alto.max(t.alto);
            let hueco = t.x - a.fin;
            (t.y - a.y).abs() <= 0.2 * alto
                && (t.alto - a.alto).abs() <= 0.15 * alto
                && hueco < 0.5 * alto
                && hueco > -0.5 * alto
        });
        match sigue {
            Some(p) => {
                let a = &mut sale[abiertos[p]];
                let ya_hay = a.texto.ends_with(' ') || t.texto.starts_with(' ');
                if !ya_hay && t.x - a.fin > 0.12 * t.alto {
                    a.texto.push(' ');
                }
                a.texto.push_str(&t.texto);
                a.fin = a.fin.max(t.fin);
                a.negrita &= t.negrita;
                a.cursiva &= t.cursiva;
            }
            None => {
                // Los renglones de mas arriba ya no se siguen.
                abiertos.retain(|&k| (sale[k].y - t.y).abs() <= sale[k].alto);
                sale.push(t.clone());
                abiertos.push(sale.len() - 1);
            }
        }
    }
    sale
}

/// **Las lineas en el orden en que se leen**. A una columna, [`lineas`] tal
/// cual. A dos, lo que cruza el pasillo (un titulo de ancho completo) va en
/// su sitio, y entre medias **toda la columna izquierda y despues la
/// derecha**.
///
/// **Distinto del movil, a proposito**: antes se juntan las palabras seguidas
/// de un mismo renglon ([`corridos`]). LaTeX, Word y casi todo lo que no es
/// una prueba escriben cada palabra en su propio trozo, y entonces el titulo
/// centrado no «cruza» el pasillo (ninguna palabra suya lo hace) y sus
/// palabras de en medio tapan el pasillo: la hoja se leia a una columna,
/// mezclada. Medido con un articulo de LaTeX a dos columnas.
pub fn lineas_en_orden(trozos: &[Trozo]) -> Vec<Linea> {
    let juntos = corridos(trozos);
    let trozos = juntos.as_slice();
    let Some(pasillo) = pasillo_de_columnas(trozos) else {
        return lineas(trozos);
    };
    let holgura = mediana_de_altos(trozos) * 0.3;
    let cruza = |t: &Trozo| t.x < pasillo - holgura && t.fin > pasillo + holgura;
    let de = |filtro: &dyn Fn(&Trozo) -> bool| -> Vec<Trozo> {
        trozos.iter().filter(|t| filtro(t)).cloned().collect()
    };
    let mut anchas = lineas(&de(&cruza));
    let con_columna = |mut v: Vec<Linea>, c: u8| {
        for l in &mut v {
            l.columna = c;
        }
        v
    };
    let izquierda = con_columna(
        lineas(&de(&|t: &Trozo| t.fin <= pasillo + holgura && !cruza(t))),
        1,
    );
    let derecha = con_columna(
        lineas(&de(&|t: &Trozo| {
            t.x >= pasillo - holgura && t.fin > pasillo + holgura
        })),
        2,
    );
    anchas.sort_by(|a, b| a.y.total_cmp(&b.y));
    let mut sale = Vec::new();
    let mut desde = f64::NEG_INFINITY;
    let mut tramos: Vec<Option<Linea>> = anchas.into_iter().map(Some).collect();
    tramos.push(None);
    for tramo in tramos {
        let hasta = tramo.as_ref().map_or(f64::INFINITY, |l| l.y);
        let entre = |l: &&Linea| l.y >= desde && l.y < hasta;
        sale.extend(izquierda.iter().filter(entre).cloned());
        sale.extend(derecha.iter().filter(entre).cloned());
        if let Some(l) = tramo {
            sale.push(l);
        }
        desde = hasta;
    }
    sale
}

/// El tamano de la letra del cuerpo: el que mas letras lleva.
pub fn alto_del_cuerpo(lineas: &[Linea]) -> f64 {
    let mut grupos: Vec<(f64, usize)> = Vec::new();
    for l in lineas {
        // `Math.round(alto * 2) / 2.0`: al medio punto.
        let clave = (l.alto * 2.0 + 0.5).floor() / 2.0;
        let n = l.texto.chars().count();
        match grupos.iter_mut().find(|(k, _)| *k == clave) {
            Some(g) => g.1 += n,
            None => grupos.push((clave, n)),
        }
    }
    grupos
        .iter()
        .fold(None::<(f64, usize)>, |m, g| match m {
            Some(m) if m.1 >= g.1 => Some(m),
            _ => Some(*g),
        })
        .map_or(10.0, |g| g.0)
}

/// **Los parrafos de una pagina.** Se corta cuando la linea siguiente esta
/// mas lejos de lo normal, cambia de tamano de letra, entra con sangria, o
/// la anterior acaba corta y en punto.
pub fn parrafos(lineas: &[Linea], cuerpo: f64, pagina: usize) -> Vec<Parrafo> {
    if lineas.is_empty() {
        return Vec::new();
    }
    let del_cuerpo = |l: &Linea| (l.alto - cuerpo).abs() < cuerpo * 0.15;
    // El interlineado normal: la mediana de los saltos entre lineas del tamano del cuerpo.
    let mut saltos: Vec<f64> = lineas
        .windows(2)
        .filter_map(|w| {
            let d = w[1].y - w[0].y;
            (d > 0.0 && del_cuerpo(&w[0]) && del_cuerpo(&w[1]) && d < cuerpo * 3.0).then_some(d)
        })
        .collect();
    saltos.sort_by(f64::total_cmp);
    let normal = saltos
        .get(saltos.len() / 2)
        .copied()
        .unwrap_or(cuerpo * 1.3);
    // El borde izquierdo y derecho del texto, **en cada columna**: a dos
    // columnas, la derecha empieza en medio de la hoja y eso no es sangria.
    let izquierda_de = |c: u8| {
        let de_la_columna: Vec<&Linea> = lineas.iter().filter(|l| l.columna == c).collect();
        let mut x0s: Vec<f64> = de_la_columna
            .iter()
            .filter(|l| del_cuerpo(l))
            .map(|l| l.x0)
            .collect();
        x0s.sort_by(f64::total_cmp);
        x0s.get(x0s.len() / 4)
            .copied()
            .unwrap_or_else(|| de_la_columna.iter().map(|l| l.x0).fold(f64::MAX, f64::min))
    };
    let derecha_de = |c: u8| {
        lineas
            .iter()
            .filter(|l| l.columna == c)
            .map(|l| l.x1)
            .fold(f64::MIN, f64::max)
    };

    let mut sale: Vec<Parrafo> = Vec::new();
    let mut actual = String::new();
    let mut primera: Option<&Linea> = None;
    let mut anterior: Option<&Linea> = None;
    let cerrar = |actual: &mut String, primera: &mut Option<&Linea>, sale: &mut Vec<Parrafo>| {
        if let Some(p) = primera.take() {
            let texto = actual.trim().to_string();
            if !texto.is_empty() {
                let nivel = nivel_de(p, &texto, cuerpo);
                sale.push(Parrafo {
                    texto,
                    nivel,
                    pagina,
                });
            }
        }
        actual.clear();
    };
    for l in lineas {
        let nuevo = match anterior {
            None => true,
            Some(a) => {
                let (izq_a, der_a) = (izquierda_de(a.columna), derecha_de(a.columna));
                let izq_l = izquierda_de(l.columna);
                let corta =
                    a.x1 < izq_a + (der_a - izq_a) * 0.75 && acaba_en_final(a.texto.trim_end());
                // Al pasar de una columna a la otra la linea sube: eso no es salto.
                let misma_columna = a.columna == l.columna;
                (misma_columna && l.y - a.y > normal * 1.45)
                    || (a.columna == 0) != (l.columna == 0)
                    || (l.alto - a.alto).abs() > cuerpo * 0.2
                    || (l.x0 - izq_l > l.alto * 1.2 && a.x0 - izq_a < l.alto * 0.5)
                    || corta
            }
        };
        if nuevo {
            cerrar(&mut actual, &mut primera, &mut sale);
        }
        if primera.is_none() {
            primera = Some(l);
            actual.push_str(&l.texto);
        } else if actual.ends_with('-') && l.texto.chars().next().is_some_and(char::is_lowercase) {
            // Un guion al final y minuscula al empezar la siguiente: palabra cortada.
            actual.pop();
            actual.push_str(&l.texto);
        } else {
            actual.push(' ');
            actual.push_str(&l.texto);
        }
        anterior = Some(l);
    }
    cerrar(&mut actual, &mut primera, &mut sale);
    if sale.is_empty() {
        return sale;
    }
    // Fuera el numero de pagina: un parrafo suelto de solo numeros al principio o al final.
    let ultimo = sale.len() - 1;
    sale.into_iter()
        .enumerate()
        .filter(|(i, p)| {
            !((*i == 0 || *i == ultimo)
                && p.texto.chars().count() <= 12
                && es_numero_de_pagina(&p.texto))
        })
        .map(|(_, p)| p)
        .collect()
}

fn nivel_de(primera: &Linea, texto: &str, cuerpo: f64) -> u8 {
    let largo = texto.chars().count();
    if primera.alto >= cuerpo * 1.6 && largo < 200 {
        1
    } else if primera.alto >= cuerpo * 1.25 && largo < 250 {
        2
    } else if primera.negrita && largo < 120 && !acaba_en_final(texto) {
        3
    } else {
        0
    }
}

/// `^[\p{Punct}\s–—]*(\d{1,4}|[ivxlcdmIVXLCDM]{1,6})([\s/–—-]*(de|of)?\s*\d{0,4})[\p{Punct}\s–—]*$`
/// del movil, escrito a mano: «12», «- 3 -», «iv», «3 de 10», «4/20».
pub fn es_numero_de_pagina(texto: &str) -> bool {
    let cs: Vec<char> = texto.chars().collect();
    let relleno = |c: char| c.is_ascii_punctuation() || c.is_whitespace() || c == '–' || c == '—';
    let mut i = 0;
    while i < cs.len() && relleno(cs[i]) {
        i += 1;
    }
    let cifras = cs[i..].iter().take_while(|c| c.is_ascii_digit()).count();
    let romanos = cs[i..]
        .iter()
        .take_while(|c| "ivxlcdmIVXLCDM".contains(**c))
        .count();
    let numero = if cifras > 0 {
        cifras.min(4)
    } else {
        romanos.min(6)
    };
    if numero == 0 {
        return false;
    }
    i += numero;
    while i < cs.len() && (cs[i].is_whitespace() || matches!(cs[i], '/' | '–' | '—' | '-')) {
        i += 1;
    }
    if cs.len() >= i + 2 && matches!((cs[i], cs[i + 1]), ('d', 'e') | ('o', 'f')) {
        i += 2;
    }
    while i < cs.len() && cs[i].is_whitespace() {
        i += 1;
    }
    let mas = cs[i..]
        .iter()
        .take_while(|c| c.is_ascii_digit())
        .count()
        .min(4);
    i += mas;
    cs[i..].iter().all(|c| relleno(*c))
}

/// Los parrafos de todas las hojas: cada hoja en el orden en que se lee y
/// con el tamano del cuerpo del documento entero.
pub fn parrafos_de_hojas(por_hoja: &[Vec<Trozo>]) -> Vec<Parrafo> {
    let lineas: Vec<Vec<Linea>> = por_hoja.iter().map(|t| lineas_en_orden(t)).collect();
    let todas: Vec<Linea> = lineas.iter().flatten().cloned().collect();
    let cuerpo = alto_del_cuerpo(&todas);
    lineas
        .iter()
        .enumerate()
        .flat_map(|(i, l)| parrafos(l, cuerpo, i))
        .collect()
}

/// El idioma del texto (para las voces), de los parrafos de cuerpo; si no
/// queda claro, `del_aparato`.
pub fn idioma_de(parrafos: &[Parrafo], del_aparato: &str) -> String {
    let muestra: String = parrafos
        .iter()
        .filter(|p| p.nivel == 0)
        .take(60)
        .map(|p| p.texto.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(5000)
        .collect();
    let por_defecto = del_aparato
        .split('-')
        .next()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("es");
    crate::voz_alta::idioma_del_texto(&muestra, por_defecto)
}

/// La clase de la marca de cada hoja en la pagina.
pub const CLASE_DE_HOJA: &str = "hoja-pdf";

/// Si el texto de un bloque es la marca «Hoja N» (o «Page N»): se pinta en
/// gris y no se lee en voz alta (en el movil va con `aria-hidden`).
pub fn es_marca_de_hoja(texto: &str, palabra: &str) -> bool {
    texto
        .strip_prefix(palabra)
        .and_then(|r| r.strip_prefix(' '))
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// **La pagina entera**, de los parrafos de todas las hojas. `idioma` va en
/// `lang`, para las voces; `hoja` es la palabra de la marca («Hoja»).
pub fn html(titulo: &str, parrafos: &[Parrafo], idioma: &str, hoja: &str) -> String {
    let mut s = String::new();
    let mut pagina = usize::MAX;
    for p in parrafos {
        // Cada hoja empieza con su marca: para saber por donde se va.
        if p.pagina != pagina {
            pagina = p.pagina;
            let n = pagina + 1;
            s.push_str(&format!(
                "<p class=\"{CLASE_DE_HOJA}\" id=\"hoja-{n}\" aria-hidden=\"true\">{} {n}</p>\n",
                escapar(hoja)
            ));
        }
        let etiqueta = if p.nivel == 0 {
            "p".to_string()
        } else {
            format!("h{}", p.nivel)
        };
        s.push_str(&format!(
            "<{etiqueta} data-hoja=\"{}\">{}</{etiqueta}>\n",
            pagina + 1,
            escapar(&p.texto)
        ));
    }
    format!(
        "<!DOCTYPE html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{}</title><style>{}\
         .hoja-pdf{{margin:28px 0 8px;color:#888;font-size:.75em;letter-spacing:.08em;text-transform:uppercase;border-top:1px solid #ddd;padding-top:6px}}\
         </style></head><body><main>\n{s}</main></body></html>\n",
        escapar(idioma),
        escapar(titulo),
        crate::documento::ESTILO,
    )
}

/// **Cuantas palabras tiene el PDF**, para la pagina que sube sola. Un
/// escaneo no tiene texto: entonces se cuentan [`POR_HOJA_SI_NO_HAY`] por hoja.
pub fn contar_palabras(por_hoja: &[Vec<Trozo>]) -> usize {
    let cuenta: usize = por_hoja
        .iter()
        .flat_map(|t| lineas(t))
        .map(|l| l.texto.split(' ').filter(|p| !p.trim().is_empty()).count())
        .sum();
    if cuenta > 0 {
        cuenta
    } else {
        por_hoja.len() * POR_HOJA_SI_NO_HAY
    }
}

pub fn es_pdf(nombre: &str) -> bool {
    crate::extension(nombre) == "pdf"
}

#[cfg(test)]
mod pruebas {
    use super::*;

    // Los casos de `PdfAHtmlTest.kt`, con los mismos numeros.

    #[test]
    fn los_trozos_a_la_misma_altura_hacen_una_linea_con_espacio_solo_donde_hay_hueco() {
        let l = lineas(&[
            Trozo::nuevo(100.0, 200.3, 130.0, 10.0, "mun"),
            Trozo::nuevo(72.0, 200.0, 95.0, 10.0, "Hola"),
            Trozo::nuevo(130.2, 200.1, 140.0, 10.0, "do"),
            Trozo::nuevo(72.0, 214.0, 120.0, 10.0, "Otra línea"),
        ]);
        let textos: Vec<&str> = l.iter().map(|l| l.texto.as_str()).collect();
        assert_eq!(textos, ["Hola mundo", "Otra línea"]);
    }

    fn dos_columnas() -> Vec<Trozo> {
        let mut trozos = vec![Trozo::nuevo(
            72.0,
            60.0,
            520.0,
            20.0,
            "Un título que cruza las dos columnas",
        )];
        for k in 0..10 {
            let y = 100.0 + k as f64 * 14.0;
            trozos.push(Trozo::nuevo(
                72.0,
                y,
                280.0,
                11.0,
                &format!("izquierda {k} de la columna que va primero"),
            ));
            trozos.push(Trozo::nuevo(
                310.0,
                y,
                520.0,
                11.0,
                &format!("derecha {k} de la columna que va después"),
            ));
        }
        trozos
    }

    #[test]
    fn a_dos_columnas_se_lee_la_izquierda_entera_y_luego_la_derecha_con_el_titulo_delante() {
        let trozos = dos_columnas();
        let p = pasillo_de_columnas(&trozos).expect("hay pasillo");
        assert!((p - 305.0).abs() <= 12.0, "pasillo en {p}");
        let l = lineas_en_orden(&trozos);
        assert_eq!(l[0].texto, "Un título que cruza las dos columnas");
        let esperado: Vec<String> = (0..10)
            .map(|k| format!("izquierda {k} de la columna que va primero"))
            .chain((0..10).map(|k| format!("derecha {k} de la columna que va después")))
            .collect();
        let vistos: Vec<String> = l[1..].iter().map(|l| l.texto.clone()).collect();
        assert_eq!(vistos, esperado);
        // Y los parrafos no se cortan en cada linea de la columna derecha por estar a la derecha.
        let p = parrafos(&l, 11.0, 0);
        assert_eq!(p.len(), 2);
        assert!(p[1].texto.starts_with("izquierda 0"));
        assert!(
            p[1].texto
                .ends_with("derecha 9 de la columna que va después")
        );
        // A una columna, nada de pasillo.
        let sola: Vec<Trozo> = trozos.into_iter().filter(|t| t.x < 300.0).collect();
        assert_eq!(pasillo_de_columnas(&sola), None);
    }

    /// Como lo escribe LaTeX: cada palabra en su trozo, el titulo centrado
    /// encima del pasillo y un pasillo de 10 puntos.
    fn palabra_a_palabra() -> Vec<Trozo> {
        let mut t = Vec::new();
        let mut renglon = |x0: f64, y: f64, alto: f64, frase: &str| {
            let mut x = x0;
            for p in frase.split(' ') {
                let ancho = p.chars().count() as f64 * alto * 0.5;
                t.push(Trozo::nuevo(x, y, x + ancho, alto, p));
                x += ancho + alto * 0.33;
            }
        };
        renglon(200.0, 60.0, 17.0, "Un título centrado");
        for k in 0..12 {
            let y = 100.0 + k as f64 * 12.0;
            renglon(
                72.0,
                y,
                10.0,
                &format!("izq {k} uno dos tres cuatro cinco seis siete"),
            );
            renglon(
                72.0 + 238.0,
                y,
                10.0,
                &format!("der {k} uno dos tres cuatro cinco seis siete"),
            );
        }
        t
    }

    #[test]
    fn palabra_a_palabra_tambien_se_leen_las_dos_columnas_en_orden() {
        let t = palabra_a_palabra();
        let juntos = corridos(&t);
        // El titulo es un trozo; cada renglon, dos (uno por columna).
        assert_eq!(juntos.len(), 1 + 12 * 2, "{juntos:?}");
        assert_eq!(juntos[0].texto, "Un título centrado");
        let l = lineas_en_orden(&t);
        assert_eq!(l[0].texto, "Un título centrado");
        assert!(
            l[1].texto.starts_with("izq 0") && l[12].texto.starts_with("izq 11"),
            "{l:?}"
        );
        assert!(
            l[13].texto.starts_with("der 0") && l[24].texto.starts_with("der 11"),
            "{l:?}"
        );
        // Caso negativo: dos palabras con un hueco de pasillo no se juntan.
        let lejos = corridos(&[
            Trozo::nuevo(0.0, 0.0, 20.0, 10.0, "a"),
            Trozo::nuevo(30.0, 0.0, 40.0, 10.0, "b"),
        ]);
        assert_eq!(lejos.len(), 2);
        // Ni las de otro renglon u otro tamano.
        let otras = corridos(&[
            Trozo::nuevo(0.0, 0.0, 20.0, 10.0, "a"),
            Trozo::nuevo(21.0, 12.0, 40.0, 10.0, "b"),
            Trozo::nuevo(41.0, 12.0, 50.0, 20.0, "c"),
        ]);
        assert_eq!(otras.len(), 3);
    }

    #[test]
    fn las_lineas_se_juntan_en_parrafos_y_el_guion_de_fin_de_linea_se_deshace() {
        let linea = |y: f64, texto: &str, x1: f64, x0: f64, alto: f64| Linea {
            y,
            x0,
            x1,
            alto,
            texto: texto.into(),
            negrita: false,
            columna: 0,
        };
        let lineas = [
            linea(100.0, "Título grande", 300.0, 72.0, 20.0),
            linea(
                140.0,
                "Esto es el primer párrafo, que ocupa varias líneas y habla de la inter-",
                520.0,
                72.0,
                11.0,
            ),
            linea(154.0, "pretación de un documento.", 520.0, 72.0, 11.0),
            linea(168.0, "Sigue igual porque no hay salto.", 300.0, 72.0, 11.0),
            linea(
                196.0,
                "Segundo párrafo tras un salto grande.",
                520.0,
                72.0,
                11.0,
            ),
            linea(780.0, "12", 300.0, 290.0, 11.0),
        ];
        let p = parrafos(&lineas, 11.0, 0);
        assert_eq!(p[0].nivel, 1);
        assert_eq!(
            p[1].texto,
            "Esto es el primer párrafo, que ocupa varias líneas y habla de la interpretación de un documento. Sigue igual porque no hay salto."
        );
        assert_eq!(p[2].texto, "Segundo párrafo tras un salto grande.");
        // El numero de pagina suelto se va.
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn un_guion_ante_mayuscula_no_junta_palabras() {
        let l = |y: f64, t: &str| Linea {
            y,
            x0: 72.0,
            x1: 520.0,
            alto: 11.0,
            texto: t.into(),
            negrita: false,
            columna: 0,
        };
        let p = parrafos(
            &[l(100.0, "Norte-"), l(114.0, "Sur es una frontera")],
            11.0,
            0,
        );
        assert_eq!(p[0].texto, "Norte- Sur es una frontera");
    }

    #[test]
    fn los_numeros_de_pagina_se_reconocen_y_las_palabras_no() {
        for si in [
            "12",
            "- 3 -",
            "iv",
            "3 de 10",
            "4/20",
            "Page 2 of 9".trim_start_matches("Page "),
            "(7)",
            "1234",
        ] {
            assert!(es_numero_de_pagina(si), "{si} es numero de pagina");
        }
        for no in ["Hola", "12 manzanas", "", "Capítulo 3", "3.5 kg"] {
            assert!(!es_numero_de_pagina(no), "{no} no es numero de pagina");
        }
    }

    #[test]
    fn la_pagina_lleva_marcas_de_hoja_titulos_e_idioma() {
        let parrafos = [
            Parrafo {
                texto: "Capítulo <uno>".into(),
                nivel: 1,
                pagina: 0,
            },
            Parrafo {
                texto: "Texto".into(),
                nivel: 0,
                pagina: 0,
            },
            Parrafo {
                texto: "Más".into(),
                nivel: 0,
                pagina: 1,
            },
        ];
        let h = html("Apuntes", &parrafos, "es", "Hoja");
        assert!(h.contains("<html lang=\"es\">"));
        assert!(h.contains("<h1 data-hoja=\"1\">Capítulo &lt;uno&gt;</h1>"));
        assert!(h.contains("<p class=\"hoja-pdf\" id=\"hoja-2\" aria-hidden=\"true\">Hoja 2</p>"));
        assert_eq!(h.matches("class=\"hoja-pdf\"").count(), 2);
        assert!(es_marca_de_hoja("Hoja 12", "Hoja"));
        assert!(!es_marca_de_hoja("Hoja suelta", "Hoja"));
        assert!(!es_marca_de_hoja("Hoja ", "Hoja"));
    }

    #[test]
    fn un_escaneo_cuenta_palabras_por_hoja_y_un_pdf_con_texto_las_suyas() {
        assert_eq!(contar_palabras(&[vec![], vec![]]), 2 * POR_HOJA_SI_NO_HAY);
        assert_eq!(contar_palabras(&[dos_columnas()]), 7 + 10 * 8 * 2);
        assert!(parrafos_de_hojas(&[vec![], vec![]]).is_empty());
    }

    #[test]
    fn el_idioma_sale_del_texto_y_si_no_del_aparato() {
        let en = [Parrafo {
            texto: "The quick brown fox jumps over the lazy dog and this is the text that the user reads.".into(),
            nivel: 0,
            pagina: 0,
        }];
        assert_eq!(idioma_de(&en, "es-PE"), "en");
        assert_eq!(idioma_de(&[], "es-PE"), "es");
        assert_eq!(idioma_de(&[], ""), "es");
    }
}
