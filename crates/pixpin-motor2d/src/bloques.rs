//! **Reorganizar una hoja por bloques** (25-oct-2026, el usuario: «quiero
//! poner el recuadro que esta arriba abajo y reorganizar la pagina … que
//! utilice el mismo codigo, entre el codigo de esa hoja y lo mueva, que no lo
//! rehaga usando las herramientas del canvas … o que se expanda o le ponga
//! otros elementos, normal que lo edite»).
//!
//! Una hoja (una plantilla de serie, una figura guardada, un dibujo) se lee
//! como **bloques**: cada recuadro con todo lo que lleva dentro y su titulo
//! si va justo encima. Si la hoja no tiene recuadros, sus **franjas**: lo que
//! queda separado por huecos horizontales. Sobre esos bloques se mueve, se
//! intercambia, se duplica, se borra, se estira y se le anaden filas **con
//! las piezas que ya hay**: nada se redibuja, todo conserva su estilo, y la
//! hoja queda ordenada (los bloques de una columna se recolocan uno debajo
//! de otro con los huecos de antes, y la pagina crece si hace falta).
//!
//! [`leer`] y las operaciones son puras (devuelven un [`Plan`]);
//! [`aplicar`] lo lleva a la escena en un solo paso de deshacer.

use crate::elemento::{Elemento, Figura};
use crate::escena::Escena;
use crate::seleccion::Seleccion;

/// Un bloque de la hoja.
#[derive(Debug, Clone, PartialEq)]
pub struct Bloque {
    /// Lo que se lee primero en el (su titulo), o vacio.
    pub titulo: String,
    /// `(x0, y0, x1, y1)`.
    pub caja: (f32, f32, f32, f32),
    /// Lo que lleva, recuadro incluido.
    pub ids: Vec<u64>,
    /// El recuadro que lo encierra, si lo tiene (una franja no).
    pub recuadro: Option<u64>,
}

/// La hoja leida.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Hoja {
    /// El papel de la pagina (el rectangulo que lo encierra todo), si hay.
    pub pagina: Option<(u64, (f32, f32, f32, f32))>,
    pub bloques: Vec<Bloque>,
}

/// Lo que hay que hacer en la escena.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    /// `(id, dx, dy)`.
    pub mover: Vec<(u64, f32, f32)>,
    /// Copias: las piezas, el desplazamiento y cuanto sumar a los textos que
    /// son solo un numero (el «11» de la fila nueva tras el «10»).
    pub copiar: Vec<(Vec<u64>, f32, f32, i64)>,
    pub borrar: Vec<u64>,
    /// `(id, dalto)`: rectangulos que crecen hacia abajo y lineas verticales
    /// que se alargan por abajo.
    pub estirar: Vec<(u64, f32)>,
    /// Lo que se dice al modelo de lo hecho.
    pub que: String,
}

/// Donde va un bloque al moverlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Donde {
    /// El primero de su columna.
    Arriba,
    /// El ultimo de su columna.
    Abajo,
    /// Justo antes (encima) del bloque `n`.
    Antes(usize),
    /// Justo despues (debajo) del bloque `n`.
    Despues(usize),
}

type Caja = (f32, f32, f32, f32);

fn area(c: Caja) -> f32 {
    (c.2 - c.0).max(0.0) * (c.3 - c.1).max(0.0)
}

fn centro(c: Caja) -> (f32, f32) {
    ((c.0 + c.2) / 2.0, (c.1 + c.3) / 2.0)
}

fn dentro(p: (f32, f32), c: Caja) -> bool {
    p.0 >= c.0 && p.0 <= c.2 && p.1 >= c.1 && p.1 <= c.3
}

fn contiene(a: Caja, b: Caja) -> bool {
    b.0 >= a.0 - 0.5 && b.1 >= a.1 - 0.5 && b.2 <= a.2 + 0.5 && b.3 <= a.3 + 0.5
}

fn union(cs: impl Iterator<Item = Caja>) -> Option<Caja> {
    cs.fold(None, |acc, c| {
        Some(match acc {
            None => c,
            Some(a) => (a.0.min(c.0), a.1.min(c.1), a.2.max(c.2), a.3.max(c.3)),
        })
    })
}

fn texto_de(e: &Elemento) -> Option<&str> {
    match &e.figura {
        Figura::Texto { texto, .. } if !texto.trim().is_empty() => Some(texto.trim()),
        _ => None,
    }
}

/// **Lee la hoja** de estos elementos (los de la plantilla, o el lienzo).
pub fn leer(elementos: &[Elemento]) -> Hoja {
    let vivos: Vec<&Elemento> = elementos.iter().filter(|e| !e.borrado).collect();
    if vivos.is_empty() {
        return Hoja::default();
    }
    // La pagina: el rectangulo mas grande que encierra casi todo.
    let pagina = vivos
        .iter()
        .filter(|e| matches!(e.figura, Figura::Rectangulo))
        .filter(|p| {
            let c = p.caja();
            let dentro_n = vivos
                .iter()
                .filter(|o| o.id != p.id && dentro(centro(o.caja()), c))
                .count();
            dentro_n * 10 >= (vivos.len() - 1) * 8
        })
        .max_by(|a, b| area(a.caja()).total_cmp(&area(b.caja())))
        .map(|p| (p.id, p.caja()));
    let resto: Vec<&Elemento> = vivos
        .iter()
        .copied()
        .filter(|e| Some(e.id) != pagina.map(|p| p.0))
        .collect();
    let area_pagina = pagina
        .map(|p| area(p.1))
        .or_else(|| union(resto.iter().map(|e| e.caja())).map(area))
        .unwrap_or(1.0)
        .max(1.0);
    // Los recuadros: rectangulos con algo dentro y de un tamano que cuenta.
    let candidatos: Vec<&Elemento> = resto
        .iter()
        .copied()
        .filter(|r| matches!(r.figura, Figura::Rectangulo))
        .filter(|r| area(r.caja()) >= area_pagina * 0.015)
        .filter(|r| {
            let c = r.caja();
            resto
                .iter()
                .any(|o| o.id != r.id && contiene(c, o.caja()) && area(o.caja()) < area(c))
        })
        .collect();
    // Solo los de fuera: un recuadro dentro de otro es parte de su bloque.
    let exteriores: Vec<&Elemento> = candidatos
        .iter()
        .copied()
        .filter(|r| {
            !candidatos.iter().any(|o| {
                o.id != r.id && contiene(o.caja(), r.caja()) && area(o.caja()) > area(r.caja())
            })
        })
        .collect();
    let bloques = if exteriores.len() >= 2 {
        por_recuadros(&resto, &exteriores)
    } else {
        por_franjas(&resto)
    };
    Hoja { pagina, bloques }
}

fn por_recuadros(resto: &[&Elemento], recuadros: &[&Elemento]) -> Vec<Bloque> {
    let mut usados = std::collections::HashSet::new();
    let mut v: Vec<Bloque> = recuadros
        .iter()
        .map(|r| {
            let c = r.caja();
            let mut ids = vec![r.id];
            usados.insert(r.id);
            for o in resto {
                if o.id != r.id && !usados.contains(&o.id) && dentro(centro(o.caja()), c) {
                    ids.push(o.id);
                    usados.insert(o.id);
                }
            }
            Bloque {
                titulo: String::new(),
                caja: c,
                ids,
                recuadro: Some(r.id),
            }
        })
        .collect();
    // El titulo que va justo encima del recuadro, pegado, es suyo.
    for b in &mut v {
        let alto = b.caja.3 - b.caja.1;
        if let Some(t) = resto
            .iter()
            .filter(|o| !usados.contains(&o.id) && texto_de(o).is_some())
            .filter(|o| {
                let c = o.caja();
                let solapa = c.0.max(b.caja.0) < c.2.min(b.caja.2);
                solapa && c.3 <= b.caja.1 + 2.0 && b.caja.1 - c.3 <= (alto * 0.25).clamp(8.0, 40.0)
            })
            .min_by(|a, c| (b.caja.1 - a.caja().3).total_cmp(&(b.caja.1 - c.caja().3)))
        {
            usados.insert(t.id);
            b.ids.push(t.id);
            let c = t.caja();
            b.caja = (
                b.caja.0.min(c.0),
                c.1.min(b.caja.1),
                b.caja.2.max(c.2),
                b.caja.3,
            );
        }
    }
    for b in &mut v {
        b.titulo = titulo_de(resto, &b.ids);
    }
    v.sort_by(|a, b| {
        a.caja
            .1
            .total_cmp(&b.caja.1)
            .then(a.caja.0.total_cmp(&b.caja.0))
    });
    v
}

/// Sin recuadros: lo que queda entre huecos horizontales.
fn por_franjas(resto: &[&Elemento]) -> Vec<Bloque> {
    let mut orden: Vec<&Elemento> = resto.to_vec();
    orden.sort_by(|a, b| a.caja().1.total_cmp(&b.caja().1));
    let mut v: Vec<Bloque> = Vec::new();
    for e in orden {
        let c = e.caja();
        match v.last_mut() {
            Some(b) if c.1 <= b.caja.3 + 10.0 => {
                b.ids.push(e.id);
                b.caja = (
                    b.caja.0.min(c.0),
                    b.caja.1.min(c.1),
                    b.caja.2.max(c.2),
                    b.caja.3.max(c.3),
                );
            }
            _ => v.push(Bloque {
                titulo: String::new(),
                caja: c,
                ids: vec![e.id],
                recuadro: None,
            }),
        }
    }
    for b in &mut v {
        b.titulo = titulo_de(resto, &b.ids);
    }
    v
}

fn titulo_de(resto: &[&Elemento], ids: &[u64]) -> String {
    resto
        .iter()
        .filter(|e| ids.contains(&e.id))
        .filter_map(|e| texto_de(e).map(|t| (e.caja(), t)))
        .min_by(|a, b| a.0.1.total_cmp(&b.0.1).then(a.0.0.total_cmp(&b.0.0)))
        .map(|(_, t)| t.lines().next().unwrap_or("").chars().take(60).collect())
        .unwrap_or_default()
}

/// La lista de bloques para el modelo: `B1 «Prioridades» arriba izquierda`.
pub fn describir(h: &Hoja) -> String {
    if h.bloques.is_empty() {
        return "la hoja no tiene bloques que mover".into();
    }
    let caja = h
        .pagina
        .map(|p| p.1)
        .or_else(|| union(h.bloques.iter().map(|b| b.caja)))
        .unwrap_or((0.0, 0.0, 1.0, 1.0));
    let (w, alto) = ((caja.2 - caja.0).max(1.0), (caja.3 - caja.1).max(1.0));
    h.bloques
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let (cx, cy) = centro(b.caja);
            let v = match (cy - caja.1) / alto {
                f if f < 0.34 => "arriba",
                f if f < 0.67 => "en medio",
                _ => "abajo",
            };
            let hz = match (cx - caja.0) / w {
                f if f < 0.34 => " a la izquierda",
                f if f < 0.67 => "",
                _ => " a la derecha",
            };
            let titulo = if b.titulo.is_empty() {
                "(sin titulo)".to_string()
            } else {
                format!("«{}»", b.titulo)
            };
            format!(
                "B{} {titulo} {v}{hz}, {:.0}×{:.0}",
                i + 1,
                b.caja.2 - b.caja.0,
                b.caja.3 - b.caja.1
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// **El bloque nombrado**: `B3`, `3` o parte de su titulo (sin tildes ni
/// mayusculas).
pub fn buscar(h: &Hoja, que: &str) -> Option<usize> {
    let q = que.trim().trim_start_matches(['B', 'b', '#']);
    if let Ok(n) = q.parse::<usize>() {
        return (n >= 1 && n <= h.bloques.len()).then(|| n - 1);
    }
    let q = plegar(que.trim());
    if q.is_empty() {
        return None;
    }
    h.bloques
        .iter()
        .position(|b| plegar(&b.titulo) == q)
        .or_else(|| {
            h.bloques
                .iter()
                .position(|b| plegar(&b.titulo).contains(&q))
        })
}

fn plegar(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            o => o,
        })
        .collect()
}

/// Los bloques de la columna de `i` (los que comparten con el la mitad de su
/// ancho), de arriba abajo.
fn columna(h: &Hoja, i: usize) -> Vec<usize> {
    let c = h.bloques[i].caja;
    let mut v: Vec<usize> = (0..h.bloques.len())
        .filter(|j| {
            let o = h.bloques[*j].caja;
            let comun = c.2.min(o.2) - c.0.max(o.0);
            comun > 0.5 * (c.2 - c.0).min(o.2 - o.0)
        })
        .collect();
    v.sort_by(|a, b| h.bloques[*a].caja.1.total_cmp(&h.bloques[*b].caja.1));
    v
}

/// Recoloca `orden` (bloques de una columna) uno debajo de otro desde el
/// arriba de la columna, con los huecos que habia; `x` es la izquierda de
/// cada uno (la suya, salvo que cambie de columna).
fn apilar(h: &Hoja, antes: &[usize], orden: &[(usize, f32)], plan: &mut Plan) {
    let Some(arriba) = antes
        .iter()
        .chain(orden.iter().map(|(i, _)| i))
        .map(|i| h.bloques[*i].caja.1)
        .reduce(f32::min)
    else {
        return;
    };
    let huecos: Vec<f32> = antes
        .windows(2)
        .map(|p| (h.bloques[p[1]].caja.1 - h.bloques[p[0]].caja.3).max(0.0))
        .collect();
    let hueco_medio = if huecos.is_empty() {
        16.0
    } else {
        huecos.iter().sum::<f32>() / huecos.len() as f32
    };
    let mut y = arriba;
    for (k, (i, x)) in orden.iter().enumerate() {
        let b = &h.bloques[*i];
        let (dx, dy) = (x - b.caja.0, y - b.caja.1);
        if dx.abs() > 0.01 || dy.abs() > 0.01 {
            for id in &b.ids {
                plan.mover.push((*id, dx, dy));
            }
        }
        y += b.caja.3 - b.caja.1 + huecos.get(k).copied().unwrap_or(hueco_medio);
    }
}

fn nombre(h: &Hoja, i: usize) -> String {
    let b = &h.bloques[i];
    if b.titulo.is_empty() {
        format!("B{}", i + 1)
    } else {
        format!("B{} «{}»", i + 1, b.titulo)
    }
}

/// **Mueve el bloque `a`** dentro de su columna, o a la de `n` si `Antes(n)`
/// / `Despues(n)` esta en otra (entonces se alinea con ella y la de origen se
/// cierra).
pub fn mover(h: &Hoja, a: usize, donde: Donde) -> Result<Plan, String> {
    let destino = match donde {
        Donde::Antes(n) | Donde::Despues(n) => n,
        _ => a,
    };
    if a >= h.bloques.len() || destino >= h.bloques.len() {
        return Err("ese bloque no existe".into());
    }
    if destino == a && matches!(donde, Donde::Antes(_) | Donde::Despues(_)) {
        return Err("un bloque no se pone junto a si mismo".into());
    }
    let mut plan = Plan::default();
    let col_a = columna(h, a);
    let col_d = columna(h, destino);
    let misma = col_d.contains(&a);
    let x_destino = h.bloques[destino].caja.0;
    let mut nueva: Vec<(usize, f32)> = col_d
        .iter()
        .filter(|i| **i != a)
        .map(|i| (*i, h.bloques[*i].caja.0))
        .collect();
    let x_a = if misma {
        h.bloques[a].caja.0
    } else {
        x_destino
    };
    let pos = match donde {
        Donde::Arriba => 0,
        Donde::Abajo => nueva.len(),
        Donde::Antes(n) => nueva.iter().position(|(i, _)| *i == n).unwrap_or(0),
        Donde::Despues(n) => nueva
            .iter()
            .position(|(i, _)| *i == n)
            .map_or(nueva.len(), |p| p + 1),
    };
    nueva.insert(pos, (a, x_a));
    let mut antes = col_d.clone();
    if !misma {
        antes.push(a);
        antes.sort_by(|x, y| h.bloques[*x].caja.1.total_cmp(&h.bloques[*y].caja.1));
    }
    apilar(h, &col_d, &nueva, &mut plan);
    if !misma {
        let resto: Vec<(usize, f32)> = col_a
            .iter()
            .filter(|i| **i != a)
            .map(|i| (*i, h.bloques[*i].caja.0))
            .collect();
        apilar(h, &col_a, &resto, &mut plan);
    }
    plan.que = format!(
        "{} {}",
        nombre(h, a),
        match donde {
            Donde::Arriba => "al principio de su columna".to_string(),
            Donde::Abajo => "al final de su columna".to_string(),
            Donde::Antes(n) => format!("encima de {}", nombre(h, n)),
            Donde::Despues(n) => format!("debajo de {}", nombre(h, n)),
        }
    );
    crecer_pagina(h, &mut plan);
    Ok(plan)
}

/// **Intercambia dos bloques**: cada uno va al sitio del otro (su esquina de
/// arriba a la izquierda) y sus columnas se recolocan.
pub fn intercambiar(h: &Hoja, a: usize, b: usize) -> Result<Plan, String> {
    if a >= h.bloques.len() || b >= h.bloques.len() || a == b {
        return Err("hacen falta dos bloques distintos".into());
    }
    let mut plan = Plan::default();
    let col_a = columna(h, a);
    let col_b = columna(h, b);
    if col_a.contains(&b) {
        let orden: Vec<(usize, f32)> = col_a
            .iter()
            .map(|i| {
                let j = if *i == a {
                    b
                } else if *i == b {
                    a
                } else {
                    *i
                };
                (j, h.bloques[j].caja.0)
            })
            .collect();
        apilar(h, &col_a, &orden, &mut plan);
    } else {
        let cambia = |col: &[usize], de: usize, por: usize| -> Vec<(usize, f32)> {
            col.iter()
                .map(|i| {
                    if *i == de {
                        (por, h.bloques[de].caja.0)
                    } else {
                        (*i, h.bloques[*i].caja.0)
                    }
                })
                .collect()
        };
        apilar(h, &col_a, &cambia(&col_a, a, b), &mut plan);
        apilar(h, &col_b, &cambia(&col_b, b, a), &mut plan);
    }
    plan.que = format!("{} y {} intercambiados", nombre(h, a), nombre(h, b));
    crecer_pagina(h, &mut plan);
    Ok(plan)
}

/// **Duplica el bloque** justo debajo, empujando lo de debajo.
pub fn duplicar(h: &Hoja, a: usize) -> Result<Plan, String> {
    let b = h.bloques.get(a).ok_or("ese bloque no existe")?;
    let col = columna(h, a);
    let alto = b.caja.3 - b.caja.1;
    let hueco = hueco_de(h, &col).unwrap_or(16.0);
    let mut plan = Plan::default();
    let dy = alto + hueco;
    plan.copiar.push((b.ids.clone(), 0.0, dy, 0));
    empujar_debajo(h, &col, a, dy, &mut plan);
    plan.que = format!("{} duplicado debajo", nombre(h, a));
    crecer_pagina(h, &mut plan);
    Ok(plan)
}

/// **Borra el bloque** y cierra su hueco.
pub fn borrar(h: &Hoja, a: usize) -> Result<Plan, String> {
    let b = h.bloques.get(a).ok_or("ese bloque no existe")?;
    let col = columna(h, a);
    let mut plan = Plan {
        borrar: b.ids.clone(),
        ..Default::default()
    };
    let resto: Vec<(usize, f32)> = col
        .iter()
        .filter(|i| **i != a)
        .map(|i| (*i, h.bloques[*i].caja.0))
        .collect();
    apilar(h, &col, &resto, &mut plan);
    plan.que = format!("{} borrado", nombre(h, a));
    Ok(plan)
}

/// **Estira el bloque** `cuanto` hacia abajo: su recuadro y sus lineas
/// verticales que llegan al fondo crecen; lo de debajo baja.
pub fn estirar(h: &Hoja, elementos: &[Elemento], a: usize, cuanto: f32) -> Result<Plan, String> {
    let b = h.bloques.get(a).ok_or("ese bloque no existe")?;
    if !(1.0..=5000.0).contains(&cuanto) {
        return Err("«cuanto» tiene que ir de 1 a 5000".into());
    }
    let mut plan = Plan::default();
    estirar_en(b, elementos, cuanto, &mut plan);
    empujar_debajo(h, &columna(h, a), a, cuanto, &mut plan);
    plan.que = format!("{} estirado {cuanto:.0}", nombre(h, a));
    crecer_pagina(h, &mut plan);
    Ok(plan)
}

fn estirar_en(b: &Bloque, elementos: &[Elemento], cuanto: f32, plan: &mut Plan) {
    for e in elementos.iter().filter(|e| b.ids.contains(&e.id)) {
        let c = e.caja();
        let al_fondo = (c.3 - b.caja.3).abs() < 4.0 + e.grosor;
        let es_recuadro = Some(e.id) == b.recuadro;
        let vertical = matches!(e.figura, Figura::Linea { .. }) && (c.2 - c.0) < 3.0 + e.grosor;
        let rect_alto = matches!(e.figura, Figura::Rectangulo) && al_fondo;
        if es_recuadro || (vertical && al_fondo) || rect_alto {
            plan.estirar.push((e.id, cuanto));
        }
    }
}

/// **Anade `n` filas** al bloque: copia la ultima fila (lo que hay entre sus
/// dos ultimas rayas horizontales) `n` veces, con el mismo paso, y lo estira.
pub fn anadir_filas(h: &Hoja, elementos: &[Elemento], a: usize, n: u32) -> Result<Plan, String> {
    let b = h.bloques.get(a).ok_or("ese bloque no existe")?;
    if !(1..=200).contains(&n) {
        return Err("«filas» tiene que ir de 1 a 200".into());
    }
    let ancho = b.caja.2 - b.caja.0;
    let mut rayas: Vec<(f32, u64)> = elementos
        .iter()
        .filter(|e| b.ids.contains(&e.id) && matches!(e.figura, Figura::Linea { .. }))
        .filter(|e| {
            let c = e.caja();
            (c.3 - c.1) < 3.0 + e.grosor && (c.2 - c.0) > ancho * 0.5
        })
        .map(|e| (centro(e.caja()).1, e.id))
        .collect();
    rayas.sort_by(|x, y| x.0.total_cmp(&y.0));
    rayas.dedup_by(|x, y| (x.0 - y.0).abs() < 1.0);
    if rayas.len() < 2 {
        return Err(format!(
            "{} no tiene filas que repetir (rayas horizontales); usa «estirar»",
            nombre(h, a)
        ));
    }
    let pasos: Vec<f32> = rayas.windows(2).map(|p| p[1].0 - p[0].0).collect();
    let paso = {
        let mut p = pasos.clone();
        p.sort_by(f32::total_cmp);
        p[p.len() / 2]
    };
    let (mut y_pen, mut y_ult) = (rayas[rayas.len() - 2].0, rayas[rayas.len() - 1].0);
    // La ultima fila suele ir entre la ultima raya y el borde de abajo del
    // recuadro: ese borde cuenta como una raya mas (la copia de la ultima
    // raya cae justo en el, y el recuadro crece por debajo).
    let fondo = b.caja.3;
    if b.recuadro.is_some() && ((fondo - y_ult) - paso).abs() <= paso * 0.3 {
        // Desde un poco antes de la ultima raya: tambien se copia ella.
        y_pen = y_ult - 1.0;
        y_ult = fondo - 1.0;
    }
    // La fila: lo que cae entre las dos ultimas rayas, y la ultima raya.
    let fila: Vec<u64> = elementos
        .iter()
        .filter(|e| b.ids.contains(&e.id) && Some(e.id) != b.recuadro)
        .filter(|e| {
            let (_, cy) = centro(e.caja());
            let c = e.caja();
            let alto_bloque = (c.3 - c.1) > (b.caja.3 - b.caja.1) * 0.5;
            !alto_bloque && cy > y_pen + 0.5 && cy <= y_ult + 0.5
        })
        .map(|e| e.id)
        .collect();
    let mut plan = Plan::default();
    // Lo que va debajo de la ultima raya dentro del bloque (un pie) baja.
    for e in elementos.iter().filter(|e| b.ids.contains(&e.id)) {
        if Some(e.id) != b.recuadro && centro(e.caja()).1 > y_ult + 0.5 && !fila.contains(&e.id) {
            let c = e.caja();
            if (c.3 - c.1) <= (b.caja.3 - b.caja.1) * 0.5 {
                plan.mover.push((e.id, 0.0, paso * n as f32));
            }
        }
    }
    for k in 1..=n {
        plan.copiar
            .push((fila.clone(), 0.0, paso * k as f32, i64::from(k)));
    }
    let cuanto = paso * n as f32;
    estirar_en(b, elementos, cuanto, &mut plan);
    empujar_debajo(h, &columna(h, a), a, cuanto, &mut plan);
    plan.que = format!("{n} filas mas en {}", nombre(h, a));
    crecer_pagina(h, &mut plan);
    Ok(plan)
}

fn hueco_de(h: &Hoja, col: &[usize]) -> Option<f32> {
    let v: Vec<f32> = col
        .windows(2)
        .map(|p| (h.bloques[p[1]].caja.1 - h.bloques[p[0]].caja.3).max(0.0))
        .collect();
    (!v.is_empty()).then(|| v.iter().sum::<f32>() / v.len() as f32)
}

/// Lo que va debajo de `a` en su columna, `dy` mas abajo.
fn empujar_debajo(h: &Hoja, col: &[usize], a: usize, dy: f32, plan: &mut Plan) {
    let abajo = h.bloques[a].caja.3;
    for i in col
        .iter()
        .filter(|i| **i != a && h.bloques[**i].caja.1 >= abajo - 1.0)
    {
        for id in &h.bloques[*i].ids {
            plan.mover.push((*id, 0.0, dy));
        }
    }
}

/// Si lo movido o estirado se sale por abajo de la pagina, la pagina crece
/// (con el mismo margen de abajo que tenia).
fn crecer_pagina(h: &Hoja, plan: &mut Plan) {
    let Some((pid, pc)) = h.pagina else {
        return;
    };
    let fondo_antes = h.bloques.iter().map(|b| b.caja.3).fold(f32::MIN, f32::max);
    let margen = (pc.3 - fondo_antes).max(0.0);
    let mut fondo = fondo_antes;
    for (i, b) in h.bloques.iter().enumerate() {
        let _ = i;
        let dy = b
            .ids
            .iter()
            .filter_map(|id| plan.mover.iter().find(|m| m.0 == *id).map(|m| m.2))
            .fold(0.0f32, f32::max);
        let crece = b
            .ids
            .iter()
            .filter_map(|id| plan.estirar.iter().find(|m| m.0 == *id).map(|m| m.1))
            .fold(0.0f32, f32::max);
        fondo = fondo.max(b.caja.3 + dy + crece);
    }
    for (ids, _, dy, _) in &plan.copiar {
        if let Some(c) = h
            .bloques
            .iter()
            .filter(|b| b.ids.iter().any(|i| ids.contains(i)))
            .map(|b| b.caja.3)
            .reduce(f32::max)
        {
            fondo = fondo.max(c + dy);
        }
    }
    let falta = fondo + margen - pc.3;
    if falta > 0.5 {
        plan.estirar.push((pid, falta));
    }
}

/// **Lleva el plan a la escena**, en un solo paso de deshacer. Devuelve los
/// ids tocados y creados.
pub fn aplicar(escena: &mut Escena, plan: &Plan) -> Vec<u64> {
    let mut tocados: Vec<u64> = Vec::new();
    escena.abrir_paso();
    // Primero las copias, desde donde estaban las piezas (antes de moverlas).
    for (ids, dx, dy, sumar) in &plan.copiar {
        let mut sel = Seleccion::nueva();
        sel.poner_todos(ids.iter().copied());
        let copias = crate::portapapeles::copiar(escena, &sel);
        let nuevos = crate::portapapeles::pegar(escena, &copias, *dx, *dy);
        if *sumar != 0 {
            for id in &nuevos {
                if let Some(e) = escena.buscar_mut(*id)
                    && let Figura::Texto { texto, .. } = &mut e.figura
                    && let Ok(n) = texto.trim().parse::<i64>()
                {
                    *texto = (n + sumar).to_string();
                    e.tocar();
                }
            }
        }
        tocados.extend(nuevos);
    }
    let mut juntos: Vec<(u64, f32, f32)> = Vec::new();
    for (id, dx, dy) in &plan.mover {
        match juntos.iter_mut().find(|m| m.0 == *id) {
            Some(m) => {
                m.1 += dx;
                m.2 += dy;
            }
            None => juntos.push((*id, *dx, *dy)),
        }
    }
    for (id, dx, dy) in juntos {
        escena.apuntar_edicion(id);
        if let Some(e) = escena.buscar_mut(id) {
            e.mover(dx, dy);
            e.tocar();
            tocados.push(id);
        }
    }
    for (id, cuanto) in &plan.estirar {
        escena.apuntar_edicion(*id);
        if let Some(e) = escena.buscar_mut(*id) {
            match &mut e.figura {
                Figura::Linea { puntos } => {
                    // El punto de mas abajo baja.
                    if let Some(p) = puntos.iter_mut().max_by(|a, b| a.y.total_cmp(&b.y)) {
                        p.y += cuanto;
                    }
                    e.alto += cuanto;
                }
                _ => e.alto += cuanto,
            }
            e.tocar();
            tocados.push(*id);
        }
    }
    for id in &plan.borrar {
        escena.borrar_apuntando(*id);
    }
    escena.cerrar_paso();
    tocados.sort_unstable();
    tocados.dedup();
    tocados
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::figuras_de_serie::Lapiz;

    fn negro() -> crate::elemento::ColorRgba {
        crate::elemento::ColorRgba::opaco(0.0, 0.0, 0.0)
    }

    /// Una pagina con tres recuadros apilados, cada uno con su titulo dentro
    /// y rayas de filas.
    fn hoja() -> Escena {
        let mut l = Lapiz::default();
        l.rect(0.0, 0.0, 600.0, 900.0, Some(negro()), None, 1.0, false);
        for (k, (y, alto, t)) in [
            (40.0, 200.0, "Prioridades"),
            (280.0, 260.0, "Tareas"),
            (620.0, 160.0, "Notas"),
        ]
        .into_iter()
        .enumerate()
        {
            l.rect(40.0, y, 520.0, alto, Some(negro()), None, 1.0, false);
            l.texto(
                60.0,
                y + 10.0,
                t,
                18.0,
                "Work Sans",
                negro(),
                true,
                crate::figuras_de_serie::Al::Izq,
            );
            if k == 1 {
                for f in 0..5 {
                    let fy = y + 60.0 + 40.0 * f as f32;
                    l.linea(40.0, fy, 560.0, fy, negro(), 1.0);
                    l.texto(
                        50.0,
                        fy + 8.0,
                        &(f + 1).to_string(),
                        12.0,
                        "Work Sans",
                        negro(),
                        false,
                        crate::figuras_de_serie::Al::Izq,
                    );
                }
                l.linea(100.0, y, 100.0, y + alto, negro(), 1.0);
            }
        }
        let mut e = Escena::nueva();
        for x in l.v {
            e.anadir(x);
        }
        e
    }

    fn vivos(e: &Escena) -> Vec<Elemento> {
        e.visibles().cloned().collect()
    }

    #[test]
    fn la_hoja_se_lee_en_sus_recuadros_con_su_titulo() {
        let e = hoja();
        let h = leer(&vivos(&e));
        assert!(h.pagina.is_some(), "el papel es la pagina, no un bloque");
        let t: Vec<&str> = h.bloques.iter().map(|b| b.titulo.as_str()).collect();
        assert_eq!(t, ["Prioridades", "Tareas", "Notas"]);
        assert_eq!(buscar(&h, "tareas"), Some(1));
        assert_eq!(buscar(&h, "B3"), Some(2));
        assert_eq!(buscar(&h, "nada de eso"), None);
        assert!(describir(&h).contains("B1 «Prioridades» arriba"));
    }

    #[test]
    fn el_de_arriba_va_abajo_y_los_demas_suben_sin_redibujar_nada() {
        let mut e = hoja();
        let antes = e.visibles().count();
        let h = leer(&vivos(&e));
        let plan = mover(&h, 0, Donde::Abajo).unwrap();
        aplicar(&mut e, &plan);
        assert_eq!(e.visibles().count(), antes, "las mismas piezas, movidas");
        let h2 = leer(&vivos(&e));
        let t: Vec<&str> = h2.bloques.iter().map(|b| b.titulo.as_str()).collect();
        assert_eq!(t, ["Tareas", "Notas", "Prioridades"]);
        assert!(
            (h2.bloques[0].caja.1 - 40.0).abs() < 0.5,
            "empieza donde empezaba la columna"
        );
        // Caso negativo: no se solapan.
        for p in h2.bloques.windows(2) {
            assert!(p[1].caja.1 >= p[0].caja.3 - 0.5);
        }
        // Un Ctrl+Z lo devuelve todo.
        e.deshacer();
        let t: Vec<String> = leer(&vivos(&e))
            .bloques
            .iter()
            .map(|b| b.titulo.clone())
            .collect();
        assert_eq!(t, ["Prioridades", "Tareas", "Notas"]);
    }

    #[test]
    fn intercambiar_y_anadir_filas_con_su_numero() {
        let mut e = hoja();
        let h = leer(&vivos(&e));
        aplicar(&mut e, &intercambiar(&h, 0, 2).unwrap());
        let t: Vec<String> = leer(&vivos(&e))
            .bloques
            .iter()
            .map(|b| b.titulo.clone())
            .collect();
        assert_eq!(t, ["Notas", "Tareas", "Prioridades"]);

        let mut e = hoja();
        let todos = vivos(&e);
        let h = leer(&todos);
        let plan = anadir_filas(&h, &todos, 1, 3).unwrap();
        aplicar(&mut e, &plan);
        let numeros: Vec<String> = e
            .visibles()
            .filter_map(|x| texto_de(x).map(str::to_string))
            .filter(|t| t.parse::<i64>().is_ok())
            .collect();
        assert!(numeros.contains(&"8".to_string()), "{numeros:?}");
        let rayas = e
            .visibles()
            .filter(|x| matches!(x.figura, Figura::Linea { .. }))
            .filter(|x| {
                let c = x.caja();
                c.3 - c.1 < 3.0 && c.2 - c.0 > 400.0
            })
            .count();
        assert_eq!(rayas, 5 + 3, "cada fila nueva lleva su raya");
        let h2 = leer(&vivos(&e));
        let tareas = &h2.bloques[1];
        assert!(
            (tareas.caja.3 - tareas.caja.1) > 260.0 + 100.0,
            "el recuadro crecio"
        );
        assert!(
            h2.bloques[2].caja.1 >= tareas.caja.3 - 0.5,
            "lo de debajo bajo"
        );
        let pagina = h2.pagina.unwrap().1;
        assert!(
            pagina.3 >= h2.bloques[2].caja.3,
            "la pagina crece si hace falta"
        );
        // Caso negativo: un bloque sin rayas no tiene filas que repetir.
        let todos = vivos(&e);
        let h = leer(&todos);
        assert!(anadir_filas(&h, &todos, 0, 2).is_err());
    }

    #[test]
    fn duplicar_y_borrar_cierran_y_abren_hueco() {
        let mut e = hoja();
        let h = leer(&vivos(&e));
        aplicar(&mut e, &duplicar(&h, 2).unwrap());
        assert_eq!(leer(&vivos(&e)).bloques.len(), 4);
        let mut e = hoja();
        let h = leer(&vivos(&e));
        aplicar(&mut e, &borrar(&h, 0).unwrap());
        let h2 = leer(&vivos(&e));
        assert_eq!(h2.bloques.len(), 2);
        assert!(
            (h2.bloques[0].caja.1 - 40.0).abs() < 0.5,
            "sube a ocupar el hueco"
        );
    }
}
