//! **Los comentarios de una nota Markdown** (H12, 30-sep-2026): elegir un
//! trozo del texto, «Comentar», y el comentario queda en un panel a la
//! derecha alineado con su frase, como en Google Docs o en el editor de
//! documentos de Claude. Se responde (hilo), se edita, se borra y se
//! resuelve.
//!
//! # Fuera del `.md`
//!
//! Los comentarios **no van dentro del Markdown**: el `Markdown.kt` del
//! movil pintaria un `<!-- -->` como texto, y una nota con marcas raras ya
//! no se lee igual en otro editor. Van en un fichero hermano, JSON, con el
//! nombre de la nota (ver [`TERMINACION`] y
//! `docs/investigacion/2026-09-30-comentarios-de-notas-android.md`).
//!
//! # El ancla es una cita
//!
//! Un comentario no guarda «de la letra 120 a la 140»: el texto cambia en
//! cualquier aparato y esas cifras dejarian de decir nada. Guarda **la cita**
//! (el texto comentado), un poco de lo que hay antes y despues, y donde
//! estaba mas o menos ([`Ancla`]). Al abrir se busca la cita; si sale mas
//! de una vez, gana la que tiene el mismo contexto y, si empatan, la mas
//! cercana a donde estaba. Si la cita ya no esta tal cual (se escribio
//! dentro de ella) se busca por el contexto de los dos lados. Si tampoco,
//! el comentario queda **sin ancla**: arriba del panel, con su cita, y
//! nunca se pierde.
//!
//! Las posiciones son en unidades UTF-16, las de un `String` de Kotlin y las
//! del `RichEdit`: asi el movil y el PC cuentan lo mismo.
//!
//! Este modulo es puro: cuentas y JSON. La ventana esta en `pixpin-notas`.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// La version del formato. Una mayor se lee igual (lo que no se entiende se
/// conserva en `resto`) y se escribe con su numero.
pub const VERSION: u32 = 1;

/// Cuantas unidades de contexto se guardan a cada lado de la cita.
pub const CONTEXTO: usize = 32;

/// La terminacion del fichero de comentarios, detras del nombre de la nota
/// (`anot-<uid>.comentarios.json`, `apuntes.comentarios.json`).
pub const TERMINACION: &str = ".comentarios.json";

/// **Donde esta lo comentado**: la cita y su contexto.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Ancla {
    /// El texto comentado, tal cual en el Markdown.
    pub cita: String,
    /// Hasta [`CONTEXTO`] unidades justo antes de la cita.
    pub antes: String,
    /// Hasta [`CONTEXTO`] unidades justo despues.
    pub despues: String,
    /// Donde empezaba la cita (UTF-16 del Markdown): solo para desempatar.
    pub pos: usize,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

/// Una respuesta dentro de un hilo.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Respuesta {
    pub id: String,
    /// El nombre del aparato que la escribio, para ensenarlo.
    pub autor: String,
    /// Su codigo de aparato (el de la sincronizacion).
    pub aparato: String,
    /// Milisegundos UTC.
    pub cuando: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editado: Option<i64>,
    pub texto: String,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

/// Un comentario con su ancla y sus respuestas.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Hilo {
    pub id: String,
    pub ancla: Ancla,
    pub autor: String,
    pub aparato: String,
    pub cuando: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editado: Option<i64>,
    pub texto: String,
    pub resuelto: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resuelto_por: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resuelto_cuando: Option<i64>,
    pub respuestas: Vec<Respuesta>,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

/// El fichero entero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Comentarios {
    pub version: u32,
    pub comentarios: Vec<Hilo>,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

impl Default for Comentarios {
    fn default() -> Self {
        Comentarios {
            version: VERSION,
            comentarios: Vec::new(),
            resto: Map::new(),
        }
    }
}

/// Quien escribe: el nombre del aparato y su codigo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Quien {
    pub autor: String,
    pub aparato: String,
}

/// Lee un fichero de comentarios. Vacio es «sin comentarios»; un JSON que
/// no se entiende es un error, y quien lo lee **no debe escribir encima**
/// (se perderian los comentarios que hubiera).
pub fn leer(t: &str) -> Result<Comentarios, String> {
    let t = t.trim_start_matches('\u{feff}');
    if t.trim().is_empty() {
        return Ok(Comentarios::default());
    }
    serde_json::from_str(t).map_err(|e| e.to_string())
}

/// El JSON que se guarda: con sangria (se lee a ojo) y un salto al final.
pub fn escribir(c: &Comentarios) -> String {
    let mut s = serde_json::to_string_pretty(c).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

fn limpio(t: &str) -> Option<String> {
    let t = t.trim();
    (!t.is_empty()).then(|| t.replace("\r\n", "\n"))
}

impl Comentarios {
    pub fn hilo(&self, id: &str) -> Option<&Hilo> {
        self.comentarios.iter().find(|h| h.id == id)
    }

    fn hilo_mut(&mut self, id: &str) -> Option<&mut Hilo> {
        self.comentarios.iter_mut().find(|h| h.id == id)
    }

    /// Los que no estan resueltos: el numero del boton de la cabecera.
    pub fn abiertos(&self) -> usize {
        self.comentarios.iter().filter(|h| !h.resuelto).count()
    }

    /// Un id que no esta en el fichero: el aparato, la hora y un contador.
    /// El aparato lo hace unico entre aparatos; la hora, entre sesiones.
    fn id_nuevo(&self, aparato: &str, cuando: i64) -> String {
        let usados: HashSet<&str> = self
            .comentarios
            .iter()
            .flat_map(|h| {
                std::iter::once(h.id.as_str()).chain(h.respuestas.iter().map(|r| r.id.as_str()))
            })
            .collect();
        let aparato = if aparato.is_empty() { "x" } else { aparato };
        (0..)
            .map(|n| format!("{aparato}-{cuando:x}-{n}"))
            .find(|id| !usados.contains(id.as_str()))
            .unwrap_or_default()
    }

    /// Un comentario nuevo. `None` si el texto esta vacio o la cita tambien.
    pub fn nuevo(
        &mut self,
        ancla: Ancla,
        quien: &Quien,
        cuando: i64,
        texto: &str,
    ) -> Option<String> {
        let texto = limpio(texto)?;
        if ancla.cita.trim().is_empty() {
            return None;
        }
        let id = self.id_nuevo(&quien.aparato, cuando);
        self.comentarios.push(Hilo {
            id: id.clone(),
            ancla,
            autor: quien.autor.clone(),
            aparato: quien.aparato.clone(),
            cuando,
            texto,
            ..Default::default()
        });
        Some(id)
    }

    /// Una respuesta al hilo `hilo`. Responder a uno resuelto lo reabre,
    /// como en Google Docs: si se sigue hablando, no esta resuelto.
    pub fn responder(
        &mut self,
        hilo: &str,
        quien: &Quien,
        cuando: i64,
        texto: &str,
    ) -> Option<String> {
        let texto = limpio(texto)?;
        let id = self.id_nuevo(&quien.aparato, cuando);
        let h = self.hilo_mut(hilo)?;
        h.respuestas.push(Respuesta {
            id: id.clone(),
            autor: quien.autor.clone(),
            aparato: quien.aparato.clone(),
            cuando,
            texto,
            ..Default::default()
        });
        h.resuelto = false;
        Some(id)
    }

    /// Cambia el texto de un comentario o de una respuesta (por su id).
    pub fn editar(&mut self, id: &str, texto: &str, cuando: i64) -> bool {
        let Some(texto) = limpio(texto) else {
            return false;
        };
        for h in &mut self.comentarios {
            if h.id == id {
                if h.texto != texto {
                    h.texto = texto;
                    h.editado = Some(cuando);
                }
                return true;
            }
            if let Some(r) = h.respuestas.iter_mut().find(|r| r.id == id) {
                if r.texto != texto {
                    r.texto = texto;
                    r.editado = Some(cuando);
                }
                return true;
            }
        }
        false
    }

    /// Borra un hilo entero (su id) o solo una respuesta (la suya).
    pub fn borrar(&mut self, id: &str) -> bool {
        let antes = self.comentarios.len();
        self.comentarios.retain(|h| h.id != id);
        if self.comentarios.len() != antes {
            return true;
        }
        for h in &mut self.comentarios {
            let n = h.respuestas.len();
            h.respuestas.retain(|r| r.id != id);
            if h.respuestas.len() != n {
                return true;
            }
        }
        false
    }

    /// Resuelve (`si`) o reabre un hilo.
    pub fn resolver(&mut self, hilo: &str, si: bool, quien: &Quien, cuando: i64) -> bool {
        let Some(h) = self.hilo_mut(hilo) else {
            return false;
        };
        h.resuelto = si;
        h.resuelto_por = si.then(|| quien.autor.clone());
        h.resuelto_cuando = Some(cuando);
        true
    }

    /// Pone al dia el ancla de cada hilo contra `texto` (UTF-16). Devuelve
    /// donde cae cada uno, en el orden de los hilos; `None` es sin ancla (el
    /// ancla vieja se queda como estaba, por si el texto vuelve).
    pub fn reanclar(&mut self, texto: &[u16]) -> Vec<Option<(usize, usize)>> {
        self.comentarios
            .iter_mut()
            .map(|h| reanclar(texto, &mut h.ancla))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// El ancla

fn u16s(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn alta(c: u16) -> bool {
    (0xD800..0xDC00).contains(&c)
}

fn baja(c: u16) -> bool {
    (0xDC00..0xE000).contains(&c)
}

/// Un tramo sin partir una pareja sustituta por ninguno de los lados.
fn entero(t: &[u16], mut a: usize, mut b: usize) -> (usize, usize) {
    b = b.min(t.len());
    a = a.min(b);
    if a > 0 && a < t.len() && baja(t[a]) {
        a += 1;
    }
    if b > 0 && b < t.len() && baja(t[b]) && alta(t[b - 1]) {
        b -= 1;
    }
    (a.min(b), b)
}

fn texto_de(t: &[u16], a: usize, b: usize) -> String {
    String::from_utf16_lossy(&t[a..b])
}

fn es_blanco(c: u16) -> bool {
    char::from_u32(c as u32).is_some_and(char::is_whitespace)
}

/// **El ancla de lo elegido** `desde..hasta` en `texto`: sin los blancos de
/// los bordes. `None` si no queda nada.
pub fn ancla_de(texto: &[u16], desde: usize, hasta: usize) -> Option<Ancla> {
    let (mut a, mut b) = entero(texto, desde.min(hasta), desde.max(hasta));
    while a < b && es_blanco(texto[a]) {
        a += 1;
    }
    while b > a && es_blanco(texto[b - 1]) {
        b -= 1;
    }
    if a >= b {
        return None;
    }
    let (ca, _) = entero(texto, a.saturating_sub(CONTEXTO), a);
    let (_, cb) = entero(texto, b, b + CONTEXTO);
    Some(Ancla {
        cita: texto_de(texto, a, b),
        antes: texto_de(texto, ca, a),
        despues: texto_de(texto, b, cb),
        pos: a,
        resto: Map::new(),
    })
}

fn apariciones(texto: &[u16], aguja: &[u16]) -> Vec<usize> {
    if aguja.is_empty() || aguja.len() > texto.len() {
        return Vec::new();
    }
    let primera = aguja[0];
    (0..=texto.len() - aguja.len())
        .filter(|&i| texto[i] == primera && texto[i..i + aguja.len()] == *aguja)
        .collect()
}

/// Cuantas unidades coinciden contando desde el final de los dos.
fn comun_detras(a: &[u16], b: &[u16]) -> usize {
    a.iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count()
}

fn comun_delante(a: &[u16], b: &[u16]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

/// **Donde esta hoy lo comentado** en `texto`: primero la cita tal cual
/// (con el mismo contexto y lo mas cerca de donde estaba), y si ya no esta,
/// lo que haya entre su contexto de antes y el de despues. `None` si no se
/// encuentra: el comentario queda sin ancla.
pub fn ubicar(texto: &[u16], a: &Ancla) -> Option<(usize, usize)> {
    let cita = u16s(&a.cita);
    if cita.is_empty() {
        return None;
    }
    let antes = u16s(&a.antes);
    let despues = u16s(&a.despues);
    let exacta = apariciones(texto, &cita).into_iter().max_by(|&x, &y| {
        let contexto = |i: usize| {
            comun_detras(&texto[..i], &antes) + comun_delante(&texto[i + cita.len()..], &despues)
        };
        contexto(x)
            .cmp(&contexto(y))
            .then(y.abs_diff(a.pos).cmp(&x.abs_diff(a.pos)))
    });
    if let Some(i) = exacta {
        return Some((i, i + cita.len()));
    }
    // El contexto entero y, si tambien se toco, lo mas pegado a la cita
    // (16 y luego 8 letras de cada lado): escribir un poco mas alla no
    // tiene por que soltar el comentario.
    let largos = |l: usize| {
        let mut v = vec![l];
        v.extend([16, 8].into_iter().filter(|&n| n < l));
        v
    };
    for na in largos(antes.len()) {
        for nd in largos(despues.len()) {
            let r = por_contexto(
                texto,
                a.pos,
                &cita,
                &antes[antes.len() - na..],
                &despues[..nd],
            );
            if r.is_some() {
                return r;
            }
        }
    }
    None
}

/// La cita cambio: lo que hay entre los dos contextos, si los dos siguen y
/// lo de en medio no ha crecido tanto que ya sea otra cosa. Con un lado en
/// el borde del texto (la cita empezaba o acababa la nota), ese lado es el
/// borde. Contextos de menos de 8 letras en total no bastan: encontrarian
/// cualquier cosa.
fn por_contexto(
    texto: &[u16],
    pos: usize,
    cita: &[u16],
    antes: &[u16],
    despues: &[u16],
) -> Option<(usize, usize)> {
    if antes.len() + despues.len() < 8 {
        return None;
    }
    let limite = cita.len() * 2 + 64;
    let mut candidatos: Vec<(usize, usize)> = Vec::new();
    let inicios: Vec<usize> = if antes.is_empty() {
        if pos > limite {
            return None;
        }
        vec![0]
    } else {
        apariciones(texto, antes)
            .into_iter()
            .map(|i| i + antes.len())
            .collect()
    };
    for inicio in inicios {
        let fin = if despues.is_empty() {
            Some(texto.len()).filter(|f| f - inicio <= limite)
        } else {
            let hasta = (inicio + limite + despues.len()).min(texto.len());
            apariciones(&texto[inicio..hasta], despues)
                .first()
                .map(|d| inicio + d)
        };
        if let Some(fin) = fin
            && fin > inicio
            && texto[inicio..fin].iter().any(|&c| !es_blanco(c))
        {
            candidatos.push((inicio, fin));
        }
    }
    candidatos.into_iter().min_by_key(|(i, _)| i.abs_diff(pos))
}

/// Busca lo comentado y, si esta, pone el ancla al dia (cita, contexto y
/// sitio de ahora), para que siga al texto aunque se escriba dentro.
pub fn reanclar(texto: &[u16], a: &mut Ancla) -> Option<(usize, usize)> {
    let (x, y) = ubicar(texto, a)?;
    let resto = std::mem::take(&mut a.resto);
    let nueva = Ancla {
        cita: texto_de(texto, x, y),
        antes: texto_de(texto, entero(texto, x.saturating_sub(CONTEXTO), x).0, x),
        despues: texto_de(texto, y, entero(texto, y, y + CONTEXTO).1),
        pos: x,
        resto,
    };
    *a = nueva;
    Some((x, y))
}

/// La palabra bajo `pos` (comentar sin elegir nada comenta la palabra).
pub fn palabra_en(texto: &[u16], pos: usize) -> Option<(usize, usize)> {
    let letra = |c: u16| {
        alta(c)
            || baja(c)
            || char::from_u32(c as u32).is_some_and(|c| c.is_alphanumeric() || c == '_')
    };
    let pos = pos.min(texto.len());
    let mut a = pos;
    while a > 0 && letra(texto[a - 1]) {
        a -= 1;
    }
    let mut b = pos;
    while b < texto.len() && letra(texto[b]) {
        b += 1;
    }
    (a < b).then_some((a, b))
}

// ---------------------------------------------------------------------------
// Juntar lo de dos sitios

/// Lo ultimo que le paso a un hilo, para elegir entre dos versiones.
fn ultimo(h: &Hilo) -> i64 {
    let r = h
        .respuestas
        .iter()
        .map(|r| r.editado.unwrap_or(r.cuando))
        .max()
        .unwrap_or(0);
    h.cuando
        .max(h.editado.unwrap_or(0))
        .max(h.resuelto_cuando.unwrap_or(0))
        .max(r)
}

/// Junta por id tres listas: lo que habia al abrir (`base`), lo de aqui
/// (`mio`) y lo que hay ahora en disco (`disco`). Lo nuevo de cada lado
/// entra; lo borrado en un lado (estaba en `base`) se va si el otro no lo
/// cambio; lo cambiado en uno solo gana; cambiado en los dos, se mira dentro.
fn juntar<T: Clone + PartialEq>(
    base: &[T],
    mio: &[T],
    disco: &[T],
    id: impl Fn(&T) -> &str,
    los_dos: impl Fn(&T, &T, &T) -> T,
) -> Vec<T> {
    let busca = |l: &[T], k: &str| l.iter().find(|x| id(x) == k).cloned();
    let mut salida: Vec<T> = Vec::new();
    for m in mio {
        let k = id(m);
        match (busca(base, k), busca(disco, k)) {
            // Nuevo aqui.
            (None, _) => salida.push(m.clone()),
            // Borrado alli: se va si aqui no se toco.
            (Some(b), None) => {
                if b != *m {
                    salida.push(m.clone());
                }
            }
            (Some(b), Some(d)) => salida.push(if *m == b {
                d
            } else if d == b {
                m.clone()
            } else {
                los_dos(&b, m, &d)
            }),
        }
    }
    for d in disco {
        let k = id(d);
        let en_mio = mio.iter().any(|x| id(x) == k);
        let en_base = base.iter().any(|x| id(x) == k);
        // Nuevo alli. (Si estaba en la base y aqui no, se borro aqui.)
        if !en_mio && !en_base {
            salida.push(d.clone());
        }
    }
    salida
}

/// **Lo de este aparato y lo que llego mientras tanto, juntos**: la nota
/// estaba abierta y la sincronizacion trajo comentarios del movil; guardar
/// no los pisa. Ver [`juntar`].
pub fn fusionar(base: &Comentarios, mio: &Comentarios, disco: &Comentarios) -> Comentarios {
    let hilos = juntar(
        &base.comentarios,
        &mio.comentarios,
        &disco.comentarios,
        |h| h.id.as_str(),
        |b, m, d| {
            let mut h = if ultimo(d) > ultimo(m) {
                d.clone()
            } else {
                m.clone()
            };
            h.respuestas = juntar(
                &b.respuestas,
                &m.respuestas,
                &d.respuestas,
                |r| r.id.as_str(),
                |_, m, d| {
                    if d.editado.unwrap_or(d.cuando) > m.editado.unwrap_or(m.cuando) {
                        d.clone()
                    } else {
                        m.clone()
                    }
                },
            );
            h
        },
    );
    let mut resto = disco.resto.clone();
    resto.extend(mio.resto.clone());
    Comentarios {
        version: mio.version.max(disco.version),
        comentarios: hilos,
        resto,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn u(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    fn yo() -> Quien {
        Quien {
            autor: "Portátil".into(),
            aparato: "K7Q2".into(),
        }
    }

    fn movil() -> Quien {
        Quien {
            autor: "Teléfono".into(),
            aparato: "MOVI".into(),
        }
    }

    const NOTA: &str =
        "# Obra\nLa losa del segundo piso ya esta hormigonada.\nFalta el curado de la losa.\n";

    fn ancla(texto: &str, cita: &str, n: usize) -> Ancla {
        let t = u(texto);
        let i = texto.match_indices(cita).nth(n).unwrap().0;
        let a = texto[..i].encode_utf16().count();
        ancla_de(&t, a, a + cita.encode_utf16().count()).unwrap()
    }

    #[test]
    fn crear_un_comentario_guarda_la_cita_su_contexto_y_quien() {
        let mut c = Comentarios::default();
        let a = ancla(NOTA, "segundo piso", 0);
        assert_eq!(a.cita, "segundo piso");
        assert_eq!(a.antes, "# Obra\nLa losa del ");
        assert_eq!(a.despues, " ya esta hormigonada.\nFalta el c");
        let id = c
            .nuevo(a, &yo(), 1_000, "  ¿Seguro que es el segundo?  ")
            .unwrap();
        let h = c.hilo(&id).unwrap();
        assert_eq!(h.texto, "¿Seguro que es el segundo?");
        assert_eq!(
            (h.autor.as_str(), h.aparato.as_str(), h.cuando),
            ("Portátil", "K7Q2", 1_000)
        );
        assert_eq!(c.abiertos(), 1);
    }

    #[test]
    fn un_comentario_vacio_o_sin_cita_no_se_crea() {
        let mut c = Comentarios::default();
        assert!(c.nuevo(ancla(NOTA, "losa", 0), &yo(), 1, "   ").is_none());
        assert!(c.nuevo(Ancla::default(), &yo(), 1, "hola").is_none());
        assert!(
            ancla_de(&u("a   b"), 1, 4).is_none(),
            "solo blancos no es cita"
        );
        assert!(ancla_de(&u("abc"), 2, 2).is_none());
        assert!(c.comentarios.is_empty());
    }

    #[test]
    fn la_cita_no_lleva_los_blancos_de_los_bordes_ni_parte_un_emoji() {
        let t = u("ver 😀 la losa ");
        let a = ancla_de(&t, 3, 15).unwrap();
        assert_eq!(a.cita, "😀 la losa");
        // Empezar en mitad del emoji no lo parte: empieza detras.
        let a = ancla_de(&t, 5, 10).unwrap();
        assert_eq!(a.cita, "la");
    }

    #[test]
    fn responder_editar_resolver_y_borrar_un_hilo() {
        let mut c = Comentarios::default();
        let h = c
            .nuevo(ancla(NOTA, "curado", 0), &yo(), 10, "¿Cuantos dias?")
            .unwrap();
        let r = c.responder(&h, &movil(), 20, "Siete").unwrap();
        assert_ne!(r, h, "cada cosa con su id");
        assert!(c.editar(&r, "Siete, regando", 30));
        let hilo = c.hilo(&h).unwrap();
        assert_eq!(hilo.respuestas[0].texto, "Siete, regando");
        assert_eq!(hilo.respuestas[0].editado, Some(30));
        assert!(c.resolver(&h, true, &yo(), 40));
        assert_eq!(c.abiertos(), 0);
        assert_eq!(
            c.hilo(&h).unwrap().resuelto_por.as_deref(),
            Some("Portátil")
        );
        // Contestar a uno resuelto lo reabre.
        c.responder(&h, &yo(), 50, "Al final ocho").unwrap();
        assert!(!c.hilo(&h).unwrap().resuelto);
        // Borrar una respuesta deja el hilo; borrar el hilo, todo.
        assert!(c.borrar(&r));
        assert_eq!(c.hilo(&h).unwrap().respuestas.len(), 1);
        assert!(c.borrar(&h));
        assert!(c.comentarios.is_empty());
    }

    #[test]
    fn lo_que_no_esta_no_se_responde_ni_se_edita_ni_se_borra() {
        let mut c = Comentarios::default();
        let h = c.nuevo(ancla(NOTA, "curado", 0), &yo(), 10, "x").unwrap();
        assert!(c.responder("nadie", &yo(), 1, "hola").is_none());
        assert!(c.responder(&h, &yo(), 1, "  ").is_none());
        assert!(!c.editar("nadie", "y", 1));
        assert!(
            !c.editar(&h, "", 1),
            "un texto vacio no borra el comentario"
        );
        assert!(!c.borrar("nadie"));
        assert!(!c.resolver("nadie", true, &yo(), 1));
        assert_eq!(c.hilo(&h).unwrap().texto, "x");
    }

    #[test]
    fn dos_comentarios_en_el_mismo_milisegundo_no_comparten_id() {
        let mut c = Comentarios::default();
        let a = c.nuevo(ancla(NOTA, "losa", 0), &yo(), 5, "a").unwrap();
        let b = c.nuevo(ancla(NOTA, "losa", 1), &yo(), 5, "b").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn el_json_va_y_vuelve_y_conserva_lo_que_no_entiende() {
        let mut c = Comentarios::default();
        let h = c
            .nuevo(ancla(NOTA, "curado", 0), &yo(), 10, "¿Cuantos dias?")
            .unwrap();
        c.responder(&h, &movil(), 20, "Siete").unwrap();
        let json = escribir(&c);
        assert!(json.contains("\"comentarios\""));
        assert!(json.contains("\"ancla\""));
        assert!(!json.contains("editado"), "lo que no hay no se escribe");
        assert_eq!(leer(&json).unwrap(), c);
        // Un campo que pondra otra version (en el hilo y arriba) se conserva.
        let raro = r#"{"version":2,"queSeYo":1,"comentarios":[{"id":"a","texto":"t","ancla":{"cita":"x","peso":3},"color":"rojo","respuestas":[]}]}"#;
        let l = leer(raro).unwrap();
        assert_eq!(l.version, 2);
        let otra = escribir(&l);
        let v: Value = serde_json::from_str(&otra).unwrap();
        assert_eq!(v["queSeYo"], 1);
        assert_eq!(v["comentarios"][0]["color"], "rojo");
        assert_eq!(v["comentarios"][0]["ancla"]["peso"], 3);
    }

    #[test]
    fn un_fichero_vacio_es_sin_comentarios_y_uno_roto_es_un_error() {
        assert_eq!(leer("").unwrap(), Comentarios::default());
        assert_eq!(leer("\u{feff}  \n").unwrap(), Comentarios::default());
        assert!(leer("{\"comentarios\": [").is_err());
        assert!(leer("[1,2]").is_err());
    }

    #[test]
    fn la_cita_repetida_se_encuentra_por_su_contexto_y_no_por_la_primera() {
        // «losa» sale dos veces; el comentario es de la segunda.
        let a = ancla(NOTA, "losa", 1);
        let t = u(NOTA);
        let (x, _) = ubicar(&t, &a).unwrap();
        assert_eq!(x, a.pos);
        // Escribir algo delante corre las dos: sigue siendo la segunda.
        let nuevo = NOTA.replace("# Obra\n", "# Obra en Lima, segunda fase\n");
        let t2 = u(&nuevo);
        let (x, y) = ubicar(&t2, &a).unwrap();
        assert_eq!(String::from_utf16_lossy(&t2[x..y]), "losa");
        assert!(String::from_utf16_lossy(&t2[..x]).ends_with("curado de la "));
    }

    #[test]
    fn re_anclar_tras_escribir_dentro_de_la_cita_la_sigue() {
        let mut a = ancla(NOTA, "ya esta hormigonada", 0);
        let nuevo = NOTA.replace("ya esta hormigonada", "ya esta casi del todo hormigonada");
        let t = u(&nuevo);
        let (x, y) = reanclar(&t, &mut a).unwrap();
        assert_eq!(
            String::from_utf16_lossy(&t[x..y]),
            "ya esta casi del todo hormigonada"
        );
        assert_eq!(a.cita, "ya esta casi del todo hormigonada");
        assert_eq!(a.pos, x);
        // Y la siguiente vez ya se encuentra tal cual.
        assert_eq!(ubicar(&t, &a), Some((x, y)));
    }

    #[test]
    fn escribir_dentro_de_la_cita_y_en_su_contexto_a_la_vez_no_la_suelta() {
        let mut a = ancla(NOTA, "ya esta hormigonada", 0);
        // El contexto de antes llegaba hasta «# Obra»: tambien cambio.
        let nuevo = NOTA
            .replace("# Obra\n", "# Obra en Lima\nIntro.\n")
            .replace("ya esta hormigonada", "ya esta casi hormigonada");
        let t = u(&nuevo);
        let (x, y) = reanclar(&t, &mut a).unwrap();
        assert_eq!(
            String::from_utf16_lossy(&t[x..y]),
            "ya esta casi hormigonada"
        );
    }

    #[test]
    fn una_cita_borrada_entera_queda_sin_ancla_y_vuelve_si_vuelve_el_texto() {
        let mut a = ancla(NOTA, "del segundo piso", 0);
        let sin = NOTA.replace("del segundo piso ", "");
        assert_eq!(reanclar(&u(&sin), &mut a), None);
        assert_eq!(a.cita, "del segundo piso", "el ancla vieja se queda");
        assert!(
            reanclar(&u(NOTA), &mut a).is_some(),
            "deshacer la trae de vuelta"
        );
    }

    #[test]
    fn un_contexto_corto_no_ancla_en_cualquier_sitio() {
        // Cita cambiada y apenas contexto: mejor sin ancla que en otro sitio.
        let a = Ancla {
            cita: "zzz".into(),
            antes: "a ".into(),
            despues: " b".into(),
            pos: 0,
            resto: Map::new(),
        };
        assert_eq!(ubicar(&u("a xx b a yy b"), &a), None);
    }

    #[test]
    fn una_cita_al_principio_o_al_final_de_la_nota_se_sigue_por_el_borde() {
        let texto = "Primera frase de la nota y algo mas aqui.";
        let mut a = ancla(texto, "Primera", 0);
        assert_eq!(a.antes, "");
        let t = u(&texto.replace("Primera", "Una primerisima"));
        let (x, y) = reanclar(&t, &mut a).unwrap();
        assert_eq!(
            (x, String::from_utf16_lossy(&t[x..y]).as_str()),
            (0, "Una primerisima")
        );
        let mut b = ancla(texto, "aqui.", 0);
        let t = u(&texto.replace("aqui.", "alli, al final."));
        let (x, y) = reanclar(&t, &mut b).unwrap();
        assert_eq!(String::from_utf16_lossy(&t[x..y]), "alli, al final.");
    }

    #[test]
    fn la_palabra_bajo_el_cursor() {
        let t = u("la losa, ya");
        assert_eq!(palabra_en(&t, 4), Some((3, 7)));
        assert_eq!(palabra_en(&t, 7), Some((3, 7)), "justo detras tambien");
        assert_eq!(palabra_en(&u(" , "), 1), None);
        assert_eq!(palabra_en(&u("cañería"), 2), Some((0, 7)));
    }

    #[test]
    fn juntar_conserva_lo_nuevo_de_los_dos_lados_y_respeta_lo_borrado() {
        let mut base = Comentarios::default();
        let a = base.nuevo(ancla(NOTA, "losa", 0), &yo(), 1, "a").unwrap();
        let b = base.nuevo(ancla(NOTA, "curado", 0), &yo(), 2, "b").unwrap();
        // Aqui: se borra `a`, se responde a `b` y se crea `c`.
        let mut mio = base.clone();
        mio.borrar(&a);
        mio.responder(&b, &yo(), 10, "desde el PC").unwrap();
        let c = mio.nuevo(ancla(NOTA, "Obra", 0), &yo(), 11, "c").unwrap();
        // En disco (llego del movil): se respondio a `b`, se resolvio y hay `d`.
        let mut disco = base.clone();
        disco.responder(&b, &movil(), 12, "desde el movil").unwrap();
        disco.resolver(&b, true, &movil(), 13);
        let d = disco
            .nuevo(ancla(NOTA, "piso", 0), &movil(), 14, "d")
            .unwrap();

        let j = fusionar(&base, &mio, &disco);
        let ids: Vec<&str> = j.comentarios.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(
            ids,
            [b.as_str(), c.as_str(), d.as_str()],
            "a se borro aqui y no vuelve"
        );
        let hb = j.hilo(&b).unwrap();
        assert_eq!(hb.respuestas.len(), 2, "las dos respuestas");
        assert!(hb.resuelto, "lo ultimo fue resolverlo en el movil");
    }

    #[test]
    fn juntar_borra_lo_borrado_alli_salvo_que_aqui_se_cambiara() {
        let mut base = Comentarios::default();
        let a = base.nuevo(ancla(NOTA, "losa", 0), &yo(), 1, "a").unwrap();
        let b = base.nuevo(ancla(NOTA, "curado", 0), &yo(), 2, "b").unwrap();
        let mut mio = base.clone();
        mio.editar(&b, "b cambiado", 5);
        let disco = Comentarios::default();
        let j = fusionar(&base, &mio, &disco);
        assert!(j.hilo(&a).is_none(), "borrado alli y sin tocar aqui: fuera");
        assert_eq!(
            j.hilo(&b).unwrap().texto,
            "b cambiado",
            "cambiado aqui: se queda"
        );
        // Sin cambios en ningun lado, juntar no cambia nada.
        assert_eq!(fusionar(&base, &base, &base), base);
    }
}
