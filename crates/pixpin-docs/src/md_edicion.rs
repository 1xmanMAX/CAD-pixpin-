//! **Editar el Markdown sin ver sus marcas** (H12, 30-sep): las cuentas de
//! un editor como el de Claude o Notion sobre un texto que sigue siendo el
//! Markdown letra por letra.
//!
//! El editor esconde **siempre** las marcas (`**`, `#`, `- `, `[ ]`…), tambien
//! en el renglon del cursor: el usuario no las ve ni las escribe a mano. Pero
//! siguen en el texto (el `.md` es el mismo que lee el movil), y el
//! `RichEdit` borraria una a ciegas: un Retroceso tras la ultima letra de una
//! negrita se llevaria el `*` escondido y dejaria `**negrit*` a la vista.
//! Aqui se decide que hace cada tecla mirando solo lo que se ve:
//!
//! - [`borrar`] quita las letras **visibles** de un tramo y, con ellas, las
//!   marcas que se quedan sin nada dentro (o la marca de bloque del renglon
//!   que se junta con el de arriba);
//! - [`tecla_borrar`] es Retroceso y Supr sin nada elegido, como en Notion:
//!   al principio de un titulo o una lista lo vuelve parrafo;
//! - [`cursor_valido`] saca el cursor de dentro de una marca;
//! - [`alternar`] y [`quitar_formato`] ponen y quitan las marcas de lo
//!   elegido (los botones B, I, S, codigo de la barra flotante);
//! - [`convertir`] es la conversion al vuelo de `[ ] ` en casilla.
//!
//! Todo en unidades UTF-16 (las del `RichEdit`), y puro: se prueba sin
//! ventanas.

use crate::md_vivo::{self, Estilo};

fn u16s(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn texto_de(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

fn es_blanco(c: u16) -> bool {
    c == b' ' as u16 || c == b'\t' as u16
}

/// Que unidades del texto son marcas (las que el editor esconde siempre):
/// las `Marca` del analizador, mas el numero de una lista numerada y la
/// casilla, que pinta el parrafo y no las letras.
pub fn marcas(texto: &str) -> Vec<bool> {
    let n = texto.encode_utf16().count();
    let mut v = vec![false; n];
    for t in md_vivo::analizar(texto) {
        if matches!(
            t.estilo,
            Estilo::Marca | Estilo::Numero | Estilo::Casilla { .. }
        ) {
            for x in v.iter_mut().take(t.hasta.min(n)).skip(t.desde) {
                *x = true;
            }
        }
    }
    v
}

/// **Lo que el editor no ensena**: las marcas, el numero y la casilla de
/// una lista, la raya de un separador y el texto de una foto (se ve la foto).
pub fn escondidas(texto: &str) -> Vec<bool> {
    let n = texto.encode_utf16().count();
    let mut v = vec![false; n];
    for t in md_vivo::analizar(texto) {
        if matches!(
            t.estilo,
            Estilo::Marca
                | Estilo::Numero
                | Estilo::Casilla { .. }
                | Estilo::Regla
                | Estilo::Imagen
        ) {
            for x in v.iter_mut().take(t.hasta.min(n)).skip(t.desde) {
                *x = true;
            }
        }
    }
    v
}

/// **Si escribir `letra` en `pos` cambia lo que se esconde** en su renglon
/// (cierra una negrita, abre un titulo o una lista con el espacio, completa
/// un separador): entonces el editor la mete el mismo con el formato ya
/// puesto, y no se ve ni un instante el `**` o el `# ` (H12, 1-oct: «aparecen
/// los asteriscos por un corto tiempo»). Una letra normal la escribe el
/// control, que es lo rapido. Se mira solo el renglon: barato a cada tecla.
pub fn letra_cambia_lo_escondido(texto: &str, pos: usize, letra: char) -> bool {
    let u = u16s(texto);
    let pos = pos.min(u.len());
    let ls = md_vivo::lineas(texto);
    let l = ls[md_vivo::linea_de(&ls, pos)];
    let renglon = &u[l.desde..l.hasta.min(u.len())];
    let k = pos.saturating_sub(l.desde).min(renglon.len());
    let mut buf = [0u16; 2];
    let puesta = letra.encode_utf16(&mut buf);
    let mut nuevo = renglon[..k].to_vec();
    nuevo.extend_from_slice(puesta);
    nuevo.extend_from_slice(&renglon[k..]);
    let antes = escondidas(&texto_de(renglon));
    let despues = escondidas(&texto_de(&nuevo));
    let m = puesta.len();
    if despues.len() != antes.len() + m || despues[k..k + m].iter().any(|e| *e) {
        return true;
    }
    despues[..k] != antes[..k] || despues[k + m..] != antes[k..]
}

/// Lo que es cada renglon para el teclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Renglon {
    /// Texto (titulo, lista, cita, parrafo), con lo que mide su marca de
    /// bloque escondida (0 si es un parrafo).
    Texto(usize),
    /// Una foto o una raya: un bloque entero que no se escribe por dentro.
    Bloque,
    /// La valla de un bloque de codigo (escondida entera).
    Valla,
    /// Dentro de un bloque de codigo: texto tal cual, sin marcas.
    Codigo,
    /// Una fila de una tabla del control: la lleva el `RichEdit`.
    Tabla,
}

/// Un documento, un audio, un mensaje del chat o una hoja enlazada solo en
/// su renglon (`![x](a.pdf)`, `[x](pixpin:mensaje=…)`, `[x](pixpin:hoja=…)`):
/// la nota lo pinta como su tarjeta o su burbuja
/// (`pixpin_notas::incrustados`), un bloque entero como una foto.
pub fn es_incrustado(r: &str) -> bool {
    let t = r.trim();
    let (medio, resto) = match (t.strip_prefix("!["), t.strip_prefix('[')) {
        (Some(x), _) => (true, x),
        (None, Some(x)) => (false, x),
        _ => return false,
    };
    let Some((texto, resto)) = resto.split_once("](") else {
        return false;
    };
    let Some(url) = resto.strip_suffix(')').map(str::trim) else {
        return false;
    };
    if texto.contains(['[', ']']) || url.contains(['(', ')']) || url.is_empty() {
        return false;
    }
    medio || url.starts_with("pixpin:mensaje=") || url.starts_with("pixpin:hoja=")
}

/// Lo que es cada renglon del texto.
pub fn renglones(texto: &str) -> Vec<Renglon> {
    let mut en_codigo = false;
    texto
        .split(['\n', '\r'])
        .map(|r| {
            let l: Vec<char> = r.chars().collect();
            if md_vivo::es_de_tabla(&l) {
                return Renglon::Tabla;
            }
            if md_vivo::es_valla(&l) {
                en_codigo = !en_codigo;
                return Renglon::Valla;
            }
            if en_codigo {
                return Renglon::Codigo;
            }
            if md_vivo::es_regla(&l) || md_vivo::imagen_de(r).is_some() || es_incrustado(r) {
                return Renglon::Bloque;
            }
            // Las marcas de bloque son ASCII: letras y unidades coinciden.
            Renglon::Texto(md_vivo::marca_de_bloque(&l))
        })
        .collect()
}

/// **Donde puede estar el cursor**: nunca dentro de una marca ni delante de
/// la marca de bloque de su renglon (escribir ahi romperia el titulo o la
/// lista). Lo devuelve detras de la marca.
pub fn cursor_valido(texto: &str, pos: usize) -> usize {
    let ls = md_vivo::lineas(texto);
    let n = md_vivo::linea_de(&ls, pos);
    let l = ls[n];
    if let Some(Renglon::Texto(m)) = renglones(texto).get(n).copied()
        && m > 0
        && pos >= l.desde
        && pos < l.desde + m
    {
        return (l.desde + m).min(l.hasta);
    }
    for t in md_vivo::analizar(texto) {
        if t.estilo == Estilo::Marca && t.desde < pos && pos < t.hasta && t.linea == n {
            return t.hasta;
        }
    }
    pos
}

/// Un tramo con formato entre dos marcas: `**`dentro`**`, `[`texto`](url)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Envuelto {
    pub estilo: Estilo,
    pub abre: (usize, usize),
    pub dentro: (usize, usize),
    pub cierra: (usize, usize),
}

/// Los tramos con formato de letra del texto, con sus dos marcas: la que
/// acaba justo donde empieza lo de dentro y la que empieza donde acaba.
pub fn envueltos(texto: &str) -> Vec<Envuelto> {
    let tramos = md_vivo::analizar(texto);
    let marcas: Vec<&md_vivo::Tramo> = tramos
        .iter()
        .filter(|t| t.estilo == Estilo::Marca)
        .collect();
    tramos
        .iter()
        .filter(|t| {
            matches!(
                t.estilo,
                Estilo::Negrita
                    | Estilo::Cursiva
                    | Estilo::Tachado
                    | Estilo::Codigo
                    | Estilo::Formula
                    | Estilo::Enlace
            )
        })
        .filter_map(|t| {
            let abre = marcas
                .iter()
                .find(|m| m.linea == t.linea && m.hasta == t.desde)?;
            let cierra = marcas
                .iter()
                .find(|m| m.linea == t.linea && m.desde == t.hasta)?;
            Some(Envuelto {
                estilo: t.estilo,
                abre: (abre.desde, abre.hasta),
                dentro: (t.desde, t.hasta),
                cierra: (cierra.desde, cierra.hasta),
            })
        })
        .collect()
}

/// **Borra lo visible de `desde..hasta`**: las marcas se quedan, salvo las
/// de un tramo que se queda sin nada que se vea (borrar «negrita» entera no
/// deja `****`), la barra que escapaba una letra borrada y la marca de
/// bloque de un renglon cuyo salto se borra (se junta con el de arriba como
/// texto, como en Notion). Devuelve el texto nuevo y donde queda el cursor.
pub fn borrar(texto: &str, desde: usize, hasta: usize) -> (String, usize) {
    let u = u16s(texto);
    let (desde, hasta) = (desde.min(hasta).min(u.len()), hasta.max(desde).min(u.len()));
    let marcas = marcas(texto);
    let mut fuera = vec![false; u.len()];
    for i in desde..hasta {
        fuera[i] = !marcas[i];
    }
    // El renglon que se junta con el de arriba pierde su marca de bloque.
    let ls = md_vivo::lineas(texto);
    let tipos = renglones(texto);
    for (n, l) in ls.iter().enumerate().skip(1) {
        if fuera[l.desde - 1]
            && let Some(Renglon::Texto(m)) = tipos.get(n).copied()
        {
            for x in fuera.iter_mut().skip(l.desde).take(m) {
                *x = true;
            }
        }
    }
    // Los tramos que se quedan sin nada visible dentro, con sus marcas (de
    // dentro afuera: `***x***` pierde la cursiva y luego la negrita).
    let vacio =
        |fuera: &[bool], e: &Envuelto| (e.dentro.0..e.dentro.1).all(|i| fuera[i] || marcas[i]);
    let envs = envueltos(texto);
    loop {
        let mut cambio = false;
        for e in &envs {
            let tocado = (e.dentro.0..e.dentro.1).any(|i| fuera[i]) || e.dentro.0 == e.dentro.1;
            let ya = fuera[e.abre.0];
            if !ya && tocado && vacio(&fuera, e) {
                for i in (e.abre.0..e.abre.1).chain(e.cierra.0..e.cierra.1) {
                    fuera[i] = true;
                }
                cambio = true;
            }
        }
        if !cambio {
            break;
        }
    }
    // Una barra `\` que escapaba una letra borrada.
    for i in 0..u.len().saturating_sub(1) {
        if u[i] == b'\\' as u16 && marcas[i] && fuera[i + 1] {
            fuera[i] = true;
        }
    }
    let cursor = (0..desde).filter(|&i| !fuera[i]).count();
    let quedan: Vec<u16> = u
        .iter()
        .zip(&fuera)
        .filter(|(_, f)| !**f)
        .map(|(c, _)| *c)
        .collect();
    (texto_de(&quedan), cursor)
}

/// **Retroceso (`atras`) o Supr sin nada elegido** en `cursor`. `None`: que
/// lo haga el control (dentro de una tabla, o al principio de la nota).
pub fn tecla_borrar(texto: &str, cursor: usize, atras: bool) -> Option<(String, usize)> {
    let u = u16s(texto);
    let ls = md_vivo::lineas(texto);
    let tipos = renglones(texto);
    let marcas = marcas(texto);
    let n = md_vivo::linea_de(&ls, cursor.min(u.len()));
    let l = ls[n];
    let m = match tipos[n] {
        Renglon::Tabla => return None,
        Renglon::Texto(m) => m,
        _ => 0,
    };
    let inicio = l.desde + m;
    // Un bloque entero (foto, raya) se quita de una vez, con su salto.
    let quitar_renglon = |k: usize| -> (String, usize) {
        let r = ls[k];
        let (a, b) = if r.hasta < u.len() {
            (r.desde, r.hasta + 1)
        } else {
            (r.desde.saturating_sub(1), r.hasta)
        };
        let mut v = u[..a].to_vec();
        v.extend_from_slice(&u[b..]);
        (texto_de(&v), a)
    };
    if atras {
        let visible = (inicio..cursor.min(l.hasta)).rev().find(|&i| !marcas[i]);
        match visible {
            Some(p) => {
                let p = if p > 0 && is_baja(u[p]) && is_alta(u[p - 1]) {
                    p - 1
                } else {
                    p
                };
                let largo = if is_alta(u[p]) && p + 1 < u.len() {
                    2
                } else {
                    1
                };
                Some(borrar(texto, p, p + largo))
            }
            // Al principio de lo que se ve.
            None if m > 0 => {
                // El titulo o la lista vuelve a parrafo: se va su marca.
                let mut v = u[..l.desde].to_vec();
                v.extend_from_slice(&u[l.desde + m..]);
                Some((texto_de(&v), l.desde))
            }
            None if n == 0 => None,
            None => match tipos[n - 1] {
                Renglon::Tabla => None,
                // No se mete texto en la valla de un bloque de codigo.
                Renglon::Valla => Some((texto.to_string(), cursor)),
                Renglon::Bloque => {
                    let (t, _) = quitar_renglon(n - 1);
                    Some((t, ls[n - 1].desde))
                }
                _ => Some(borrar(texto, l.desde - 1, l.desde)),
            },
        }
    } else {
        let visible = (cursor.max(inicio)..l.hasta).find(|&i| !marcas[i]);
        match visible {
            Some(q) => {
                let largo = if is_alta(u[q]) && q + 1 < u.len() {
                    2
                } else {
                    1
                };
                let (t, _) = borrar(texto, q, q + largo);
                Some((t, cursor.min(q)))
            }
            None if n + 1 >= ls.len() => None,
            None => match tipos[n + 1] {
                Renglon::Tabla => None,
                Renglon::Valla => Some((texto.to_string(), cursor)),
                Renglon::Bloque => {
                    let (t, _) = quitar_renglon(n + 1);
                    Some((t, cursor))
                }
                _ => {
                    let (t, _) = borrar(texto, l.hasta, l.hasta + 1);
                    Some((t, cursor.min(l.hasta)))
                }
            },
        }
    }
}

fn is_alta(c: u16) -> bool {
    (0xD800..0xDC00).contains(&c)
}

fn is_baja(c: u16) -> bool {
    (0xDC00..0xE000).contains(&c)
}

/// El estilo de cada marca que envuelve.
fn estilo_de(marca: &str) -> Option<Estilo> {
    Some(match marca {
        "**" | "__" => Estilo::Negrita,
        "*" | "_" => Estilo::Cursiva,
        "~~" => Estilo::Tachado,
        "`" => Estilo::Codigo,
        "$" => Estilo::Formula,
        _ => return None,
    })
}

/// Un cambio en el texto: en `pos`, quitar `quitar` unidades y meter `poner`.
#[derive(Debug, Clone)]
struct Cambio {
    pos: usize,
    quitar: usize,
    poner: Vec<u16>,
}

/// Aplica los cambios (que no se pisan) y lleva por ellos unas posiciones.
fn aplicar(
    u: &[u16],
    mut cambios: Vec<Cambio>,
    posiciones: &[(usize, bool)],
) -> (String, Vec<usize>) {
    // De atras adelante; en la misma posicion, quitar antes que meter.
    cambios.sort_by(|a, b| {
        b.pos
            .cmp(&a.pos)
            .then_with(|| (a.quitar == 0).cmp(&(b.quitar == 0)))
    });
    let mut v = u.to_vec();
    for c in &cambios {
        v.splice(c.pos..c.pos + c.quitar, c.poner.iter().copied());
    }
    // Una posicion se corre con lo que cambia antes que ella; con `derecha`,
    // lo que se mete justo en ella queda delante.
    let llevar = |p: usize, derecha: bool| -> usize {
        let mut q = p as isize;
        for c in &cambios {
            let (pos, quitar, poner) = (c.pos as isize, c.quitar as isize, c.poner.len() as isize);
            let p = p as isize;
            if quitar > 0 && pos + quitar <= p {
                q += poner - quitar;
            } else if quitar > 0 && pos < p {
                // Dentro de lo quitado: al principio de lo que se puso.
                q += pos - p + if derecha { poner } else { 0 };
            } else if quitar == 0 && (pos < p || (pos == p && derecha)) {
                q += poner;
            }
        }
        q.max(0) as usize
    };
    let sal = posiciones.iter().map(|&(p, d)| llevar(p, d)).collect();
    (texto_de(&v), sal)
}

/// **Pone o quita un formato de letra** (`**`, `*`, `~~`, `` ` ``, `$`) a lo
/// elegido, con las marcas escondidas: lo elegido se recorta de blancos y
/// marcas (un doble clic elige la palabra con su espacio, y `**hola **` no
/// seria negrita); si todo esta ya dentro de un tramo con ese formato se le
/// quita (partiendo el tramo si hace falta); si no, se pone, juntando con
/// los tramos de ese formato que toque. Un renglon cada vez: un formato de
/// letra no cruza renglones.
///
/// Sin nada elegido, con el cursor dentro de un tramo con ese formato, se
/// lo quita a todo el tramo. `None`: no hay nada que hacer (el editor deja
/// el formato «pendiente» para lo que se escriba).
pub fn alternar(
    texto: &str,
    desde: usize,
    hasta: usize,
    marca: &str,
) -> Option<(String, usize, usize)> {
    let estilo = estilo_de(marca)?;
    let u = u16s(texto);
    let (a, b) = (desde.min(hasta).min(u.len()), hasta.max(desde).min(u.len()));
    let envs: Vec<Envuelto> = envueltos(texto)
        .into_iter()
        .filter(|e| e.estilo == estilo)
        .collect();
    if a == b {
        let e = envs.iter().find(|e| e.dentro.0 <= a && a <= e.dentro.1)?;
        let cambios = vec![
            Cambio {
                pos: e.abre.0,
                quitar: e.abre.1 - e.abre.0,
                poner: Vec::new(),
            },
            Cambio {
                pos: e.cierra.0,
                quitar: e.cierra.1 - e.cierra.0,
                poner: Vec::new(),
            },
        ];
        let (t, p) = aplicar(&u, cambios, &[(a, false)]);
        return Some((t, p[0], p[0]));
    }
    let marcas = marcas(texto);
    let ls = md_vivo::lineas(texto);
    let tipos = renglones(texto);
    let m = u16s(marca);
    // Los trozos de cada renglon, recortados.
    let mut trozos: Vec<(usize, usize)> = Vec::new();
    for (n, l) in ls.iter().enumerate() {
        let inicio = match tipos[n] {
            Renglon::Texto(k) => l.desde + k,
            _ => continue,
        };
        let (mut s, mut t) = (a.max(inicio), b.min(l.hasta));
        while s < t && (marcas[s] || es_blanco(u[s])) {
            s += 1;
        }
        while t > s && (marcas[t - 1] || es_blanco(u[t - 1])) {
            t -= 1;
        }
        if s < t {
            trozos.push((s, t));
        }
    }
    if trozos.is_empty() {
        return None;
    }
    // Quitar si todo lo elegido ya lo tiene.
    let dentro_de = |s: usize, t: usize| {
        envs.iter()
            .find(|e| e.dentro.0 <= s && t <= e.dentro.1)
            .copied()
    };
    let quitar = trozos.iter().all(|&(s, t)| dentro_de(s, t).is_some());
    let mut cambios = Vec::new();
    let mut extremos = Vec::new();
    for &(s, t) in &trozos {
        if quitar {
            let e = dentro_de(s, t).expect("mirado arriba");
            // Se cierra antes de lo elegido y se reabre detras, sin dejar un
            // blanco pegado por dentro de una marca.
            let mut cierre = s;
            while cierre > e.dentro.0 && es_blanco(u[cierre - 1]) {
                cierre -= 1;
            }
            let mut abre = t;
            while abre < e.dentro.1 && es_blanco(u[abre]) {
                abre += 1;
            }
            if cierre == e.dentro.0 {
                cambios.push(Cambio {
                    pos: e.abre.0,
                    quitar: e.abre.1 - e.abre.0,
                    poner: Vec::new(),
                });
            } else {
                cambios.push(Cambio {
                    pos: cierre,
                    quitar: 0,
                    poner: u[e.cierra.0..e.cierra.1].to_vec(),
                });
            }
            if abre == e.dentro.1 {
                cambios.push(Cambio {
                    pos: e.cierra.0,
                    quitar: e.cierra.1 - e.cierra.0,
                    poner: Vec::new(),
                });
            } else {
                cambios.push(Cambio {
                    pos: abre,
                    quitar: 0,
                    poner: u[e.abre.0..e.abre.1].to_vec(),
                });
            }
            extremos.push(((s, true), (t, false)));
        } else {
            // Se junta con los tramos de ese formato que toca.
            let (mut s2, mut t2) = (s, t);
            let tocados: Vec<&Envuelto> = envs
                .iter()
                .filter(|e| e.abre.0 < t && s < e.cierra.1)
                .collect();
            for e in &tocados {
                s2 = s2.min(e.dentro.0);
                t2 = t2.max(e.dentro.1);
                cambios.push(Cambio {
                    pos: e.abre.0,
                    quitar: e.abre.1 - e.abre.0,
                    poner: Vec::new(),
                });
                cambios.push(Cambio {
                    pos: e.cierra.0,
                    quitar: e.cierra.1 - e.cierra.0,
                    poner: Vec::new(),
                });
            }
            cambios.push(Cambio {
                pos: s2,
                quitar: 0,
                poner: m.clone(),
            });
            cambios.push(Cambio {
                pos: t2,
                quitar: 0,
                poner: m.clone(),
            });
            extremos.push(((s2, true), (t2, false)));
        }
    }
    let primero = extremos.first().map(|x| x.0).unwrap_or((a, true));
    let ultimo = extremos.last().map(|x| x.1).unwrap_or((b, false));
    let (t, p) = aplicar(&u, cambios, &[primero, ultimo]);
    Some((t, p[0], p[1]))
}

/// **Quita el formato de letra** de los tramos que toca lo elegido (o del
/// tramo del cursor): negrita, cursiva, tachado, codigo y formula. Los
/// enlaces se quedan (son lo que son, no una pinta).
pub fn quitar_formato(texto: &str, desde: usize, hasta: usize) -> Option<(String, usize, usize)> {
    let u = u16s(texto);
    let (a, b) = (desde.min(hasta).min(u.len()), hasta.max(desde).min(u.len()));
    let mut cambios = Vec::new();
    for e in envueltos(texto)
        .into_iter()
        .filter(|e| e.estilo != Estilo::Enlace)
    {
        let toca = if a == b {
            e.dentro.0 <= a && a <= e.dentro.1
        } else {
            e.abre.0 < b && a < e.cierra.1
        };
        if toca {
            cambios.push(Cambio {
                pos: e.abre.0,
                quitar: e.abre.1 - e.abre.0,
                poner: Vec::new(),
            });
            cambios.push(Cambio {
                pos: e.cierra.0,
                quitar: e.cierra.1 - e.cierra.0,
                poner: Vec::new(),
            });
        }
    }
    if cambios.is_empty() {
        return None;
    }
    let (t, p) = aplicar(&u, cambios, &[(a, true), (b, false)]);
    Some((t, p[0], p[1]))
}

/// **Conversion al vuelo** tras escribir un espacio en `cursor`: `[] `,
/// `[ ] ` o `[x] ` al principio de un renglon es una casilla (`- [ ] `),
/// como en Notion. Los demas (`# `, `- `, `1. `, `> `) ya son Markdown y el
/// formato los recoge solo. `None` si no hay nada que convertir.
pub fn convertir(texto: &str, cursor: usize) -> Option<(String, usize)> {
    let u = u16s(texto);
    let ls = md_vivo::lineas(texto);
    let n = md_vivo::linea_de(&ls, cursor.min(u.len()));
    let l = ls[n];
    if !matches!(renglones(texto).get(n), Some(Renglon::Texto(0))) || cursor > l.hasta {
        return None;
    }
    let antes = texto_de(&u[l.desde..cursor]);
    let nuevo = match antes.as_str() {
        "[] " | "[ ] " => "- [ ] ",
        "[x] " | "[X] " => "- [x] ",
        _ => return None,
    };
    let mut v = u[..l.desde].to_vec();
    v.extend(nuevo.encode_utf16());
    v.extend_from_slice(&u[cursor..]);
    Some((texto_de(&v), l.desde + nuevo.len()))
}

/// La marca de bloque que tiene el renglon `linea` (`"# "`, `"- [ ] "`…),
/// vacia si es un parrafo: lo que dice el «Aa ▾» de la barra flotante.
pub fn bloque_de(texto: &str, linea: usize) -> String {
    let Some(r) = texto.split(['\n', '\r']).nth(linea) else {
        return String::new();
    };
    match renglones(texto).get(linea) {
        Some(Renglon::Texto(m)) => r.chars().take(*m).collect(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod pruebas;
