//! **Lo que pide presentar un `.pptx`**: las notas del orador, la transicion
//! de cada diapositiva y los videos y audios incrustados.
//!
//! Nada de esto se lee al abrir: cada cosa se pide cuando hace falta (las
//! notas, cuando se ensenan; la transicion, al pasar a esa diapositiva) y lo
//! guarda quien la pide. Asi abrir una presentacion de cien diapositivas
//! sigue costando lo mismo que antes.
//!
//! Las transiciones son las sencillas: fundido, empuje, barrido, cubrir,
//! descubrir y corte. Las demas (las de `p14`, como «prisma» o «vortice»)
//! se hacen como fundido. Lo que aqui se calcula es solo **que se pinta y
//! donde** en cada instante ([`capas`]): quien pinta mezcla dos mapas de
//! bits ya hechos, sin volver a maquetar en cada fotograma.

// Partes traducidas de deckcraft (https://github.com/storytold/deckcraft), MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team and the Deckcraft contributors.

use pixpin_docs::xml::Nodo;

use super::Presentacion;
use crate::dml;
use crate::pagina::{Color, Elemento, Figura, Rect, Relleno, Tramo};

/// Lo mas que dura una transicion. Un `dur` mayor (un fichero roto, o uno
/// hecho a mano) se recorta: pasar diapositiva no puede dejar la pantalla
/// colgada un minuto.
pub const MS_MAXIMO: u32 = 10_000;
/// Lo mas que se espera para pasar sola (un dia, como PowerPoint).
pub const AVANCE_MAXIMO: u32 = 86_400_000;

// ---------------------------------------------------------------------------
// Notas del orador

impl Presentacion {
    /// Las notas del orador de la diapositiva `n` (desde 0), como texto con
    /// un parrafo por linea. `None` si no tiene, o si estan vacias.
    pub fn notas(&mut self, n: usize) -> Option<String> {
        let ruta = self.diapositivas.get(n)?.clone();
        let rels = self.paquete.relaciones(&ruta);
        let destino = rels
            .values()
            .find(|r| r.tipo == "notesSlide" && !r.externa)?
            .destino
            .clone();
        notas_de(&self.paquete.xml(&destino)?)
    }

    /// La transicion con la que entra la diapositiva `n`, si tiene.
    pub fn transicion(&mut self, n: usize) -> Option<Transicion> {
        let ruta = self.diapositivas.get(n)?.clone();
        transicion_de(&self.paquete.xml(&ruta)?)
    }

    /// Los videos y audios de la diapositiva `n`, con su caja en puntos.
    pub fn medios(&mut self, n: usize) -> Vec<Medio> {
        let Some(ruta) = self.diapositivas.get(n).cloned() else {
            return Vec::new();
        };
        let Some(raiz) = self.paquete.xml(&ruta) else {
            return Vec::new();
        };
        let rels = self.paquete.relaciones(&ruta);
        let mut v = Vec::new();
        if let Some(arbol) = raiz.hijo("cSld").and_then(|c| c.hijo("spTree")) {
            recoger_medios(arbol, &rels, &mut v);
        }
        v
    }

    /// Los bytes de un medio incrustado (`Medio::parte`). `None` si no esta
    /// o pasa del tope de una parte.
    pub fn bytes_de_parte(&mut self, parte: &str) -> Option<Vec<u8>> {
        self.paquete.bytes(parte)
    }
}

/// El texto de una diapositiva de notas (`p:notes`): el del marcador del
/// cuerpo. La imagen de la diapositiva (`sldImg`), el numero, la fecha y el
/// pie no son notas.
pub fn notas_de(raiz: &Nodo) -> Option<String> {
    let arbol = raiz.hijo("cSld").and_then(|c| c.hijo("spTree"))?;
    for sp in arbol.elementos().filter(|e| e.nombre == "sp") {
        let ph = sp
            .hijo("nvSpPr")
            .and_then(|n| n.hijo("nvPr"))
            .and_then(|n| n.hijo("ph"));
        let Some(ph) = ph else { continue };
        if !matches!(ph.atributo("type"), "" | "body") {
            continue;
        }
        let Some(tx) = sp.hijo("txBody") else {
            continue;
        };
        let mut lineas: Vec<String> = Vec::new();
        for p in tx.elementos().filter(|e| e.nombre == "p") {
            let mut linea = String::new();
            for r in p.elementos() {
                match r.nombre.as_str() {
                    "r" | "fld" => {
                        if let Some(t) = r.hijo("t") {
                            linea.push_str(&t.texto());
                        }
                    }
                    "br" => linea.push('\n'),
                    _ => {}
                }
            }
            lineas.push(linea);
        }
        // Sin las lineas vacias del principio ni del final (PowerPoint deja
        // una siempre).
        while lineas.first().is_some_and(|l| l.trim().is_empty()) {
            lineas.remove(0);
        }
        while lineas.last().is_some_and(|l| l.trim().is_empty()) {
            lineas.pop();
        }
        let texto = lineas.join("\n");
        if !texto.trim().is_empty() {
            return Some(texto);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Transiciones: leerlas

/// Hacia donde se mueve una transicion (`dir`): `dx` y `dy` valen -1, 0 o
/// 1. «l» se mueve hacia la izquierda: la nueva entra por la derecha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sentido {
    pub dx: i8,
    pub dy: i8,
}

impl Sentido {
    pub const IZQUIERDA: Sentido = Sentido { dx: -1, dy: 0 };

    /// El `dir` de OOXML (`l`, `r`, `u`, `d`, `lu`, `rd`...). Sin `dir`, o
    /// con uno que no se entiende, hacia la izquierda (el de por defecto).
    pub fn de(dir: &str) -> Sentido {
        let mut s = Sentido { dx: 0, dy: 0 };
        for c in dir.chars() {
            match c {
                'l' => s.dx = -1,
                'r' => s.dx = 1,
                'u' => s.dy = -1,
                'd' => s.dy = 1,
                _ => return Sentido::IZQUIERDA,
            }
        }
        if s.dx == 0 && s.dy == 0 {
            Sentido::IZQUIERDA
        } else {
            s
        }
    }
}

/// Las transiciones que se pintan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Efecto {
    /// Sin efecto: la nueva de golpe (o tras un instante en negro).
    Corte {
        por_negro: bool,
    },
    Fundido {
        por_negro: bool,
    },
    /// La vieja sale y la nueva entra empujandola.
    Empuje(Sentido),
    /// La nueva aparece en su sitio tras un borde que barre.
    Barrido(Sentido),
    /// La nueva entra por encima de la vieja.
    Cubrir(Sentido),
    /// La vieja se va y deja ver la nueva debajo (`pull`).
    Descubrir(Sentido),
}

/// La transicion de una diapositiva (`p:transition`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transicion {
    pub efecto: Efecto,
    /// Lo que dura, en milisegundos (0 si es un corte).
    pub ms: u32,
    /// Si pasa sola: tras cuantos milisegundos de verse (`advTm`).
    pub avance_ms: Option<u32>,
    /// Si un clic la pasa (`advClick`, si por defecto).
    pub avance_clic: bool,
}

/// La transicion de una diapositiva ya leida (`p:sld`). Mira la
/// `p:transition` de la raiz y la de dentro de un `mc:AlternateContent`
/// (PowerPoint 2010 escribe ahi las suyas, con `p14:dur`, y deja en
/// `Fallback` una que entienden los demas).
pub fn transicion_de(sld: &Nodo) -> Option<Transicion> {
    // En orden de preferencia: lo que pide la rama nueva antes que el
    // respaldo.
    let mut candidatas: Vec<&Nodo> = Vec::new();
    for h in sld.elementos() {
        match h.nombre.as_str() {
            "transition" => candidatas.push(h),
            "AlternateContent" => {
                for rama in ["Choice", "Fallback"] {
                    for r in h.elementos().filter(|e| e.nombre == rama) {
                        candidatas.extend(r.hijo("transition"));
                    }
                }
            }
            _ => {}
        }
    }
    if candidatas.is_empty() {
        return None;
    }
    let tiene_efecto = candidatas.iter().any(|t| efecto_de(t).is_some());
    let efecto = candidatas
        .iter()
        .find_map(|t| efecto_de(t).and_then(conocido))
        .unwrap_or(if tiene_efecto {
            // Una que no se sabe pintar (prisma, vortice...): como fundido.
            Efecto::Fundido { por_negro: false }
        } else {
            Efecto::Corte { por_negro: false }
        });
    let ms = match efecto {
        Efecto::Corte { por_negro: false } => 0,
        _ => candidatas
            .iter()
            .find_map(|t| ms_valido(t.atributo("dur")))
            .unwrap_or_else(|| ms_de_velocidad(candidatas[0].atributo("spd")))
            .min(MS_MAXIMO),
    };
    let avance_ms = candidatas
        .iter()
        .find_map(|t| ms_valido(t.atributo("advTm")))
        .map(|m| m.min(AVANCE_MAXIMO));
    let avance_clic = !candidatas
        .iter()
        .any(|t| matches!(t.atributo("advClick"), "0" | "false"));
    Some(Transicion {
        efecto,
        ms,
        avance_ms,
        avance_clic,
    })
}

/// El elemento del efecto: el primer hijo que no es el sonido ni una
/// extension.
fn efecto_de(t: &Nodo) -> Option<&Nodo> {
    t.elementos()
        .find(|e| !matches!(e.nombre.as_str(), "sndAc" | "extLst"))
}

/// El efecto, si es de los que se pintan tal cual.
fn conocido(e: &Nodo) -> Option<Efecto> {
    let negro = matches!(e.atributo("thruBlk"), "1" | "true");
    let dir = Sentido::de(e.atributo("dir"));
    Some(match e.nombre.as_str() {
        "cut" => Efecto::Corte { por_negro: negro },
        "fade" => Efecto::Fundido { por_negro: negro },
        "push" => Efecto::Empuje(dir),
        "wipe" => Efecto::Barrido(dir),
        "cover" => Efecto::Cubrir(dir),
        "pull" | "uncover" => Efecto::Descubrir(dir),
        _ => return None,
    })
}

/// Un tiempo en milisegundos, si es un numero con sentido.
fn ms_valido(v: &str) -> Option<u32> {
    let v = v.trim();
    if v.is_empty() || v == "indefinite" {
        return None;
    }
    let n: f64 = v.parse().ok()?;
    if !n.is_finite() || n < 0.0 {
        return None;
    }
    Some(n.min(u32::MAX as f64) as u32)
}

/// `spd`: rapida medio segundo, media tres cuartos y lenta uno (lo que
/// hace PowerPoint). Sin `spd`, rapida (el de por defecto de la norma).
fn ms_de_velocidad(spd: &str) -> u32 {
    match spd {
        "slow" => 1000,
        "med" => 750,
        _ => 500,
    }
}

// ---------------------------------------------------------------------------
// Transiciones: pintarlas

/// Por donde va una transicion que empezo hace `transcurrido` ms y dura
/// `ms`: de 0 a 1. Una de cero ms ya ha acabado.
pub fn progreso(transcurrido_ms: u64, ms: u32) -> f32 {
    if ms == 0 {
        return 1.0;
    }
    (transcurrido_ms as f64 / ms as f64).clamp(0.0, 1.0) as f32
}

/// Arranca y frena suave (lo de los movimientos).
pub fn suave(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// De donde sale lo que pinta una capa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fuente {
    Vieja,
    Nueva,
    Negro,
}

/// Una capa de un fotograma de transicion: el trozo `origen` de la fuente
/// en `destino`, con su opacidad. Todo en fracciones de la diapositiva
/// (0..1; izquierda, arriba, derecha, abajo). Se pintan en orden sobre
/// negro, recortadas a la diapositiva.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capa {
    pub fuente: Fuente,
    pub origen: [f32; 4],
    pub destino: [f32; 4],
    pub alfa: f32,
}

const ENTERA: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

impl Capa {
    fn entera(fuente: Fuente, alfa: f32) -> Capa {
        Capa {
            fuente,
            origen: ENTERA,
            destino: ENTERA,
            alfa,
        }
    }

    fn movida(fuente: Fuente, dx: f32, dy: f32) -> Capa {
        Capa {
            fuente,
            origen: ENTERA,
            destino: [dx, dy, 1.0 + dx, 1.0 + dy],
            alfa: 1.0,
        }
    }

    fn en_su_sitio(fuente: Fuente, r: [f32; 4], alfa: f32) -> Capa {
        Capa {
            fuente,
            origen: r,
            destino: r,
            alfa,
        }
    }
}

/// Las capas del fotograma de `efecto` en `t` (0..1). En 0 solo la vieja;
/// en 1 solo la nueva.
pub fn capas(efecto: Efecto, t: f32) -> Vec<Capa> {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    if t >= 1.0 {
        return vec![Capa::entera(Fuente::Nueva, 1.0)];
    }
    if t <= 0.0 && !matches!(efecto, Efecto::Corte { .. }) {
        return vec![Capa::entera(Fuente::Vieja, 1.0)];
    }
    let s = suave(t);
    let v = match efecto {
        Efecto::Corte { por_negro } => {
            if por_negro && t < 0.5 {
                vec![Capa::entera(Fuente::Negro, 1.0)]
            } else {
                vec![Capa::entera(Fuente::Nueva, 1.0)]
            }
        }
        Efecto::Fundido { por_negro: false } => vec![
            Capa::entera(Fuente::Vieja, 1.0),
            Capa::entera(Fuente::Nueva, t),
        ],
        Efecto::Fundido { por_negro: true } => {
            if t < 0.5 {
                vec![
                    Capa::entera(Fuente::Negro, 1.0),
                    Capa::entera(Fuente::Vieja, 1.0 - 2.0 * t),
                ]
            } else {
                vec![
                    Capa::entera(Fuente::Negro, 1.0),
                    Capa::entera(Fuente::Nueva, 2.0 * t - 1.0),
                ]
            }
        }
        Efecto::Empuje(d) => {
            let (dx, dy) = (d.dx as f32, d.dy as f32);
            vec![
                Capa::movida(Fuente::Vieja, dx * s, dy * s),
                Capa::movida(Fuente::Nueva, dx * (s - 1.0), dy * (s - 1.0)),
            ]
        }
        Efecto::Cubrir(d) => {
            let (dx, dy) = (d.dx as f32, d.dy as f32);
            vec![
                Capa::entera(Fuente::Vieja, 1.0),
                Capa::movida(Fuente::Nueva, dx * (s - 1.0), dy * (s - 1.0)),
            ]
        }
        Efecto::Descubrir(d) => {
            let (dx, dy) = (d.dx as f32, d.dy as f32);
            vec![
                Capa::entera(Fuente::Nueva, 1.0),
                Capa::movida(Fuente::Vieja, dx * s, dy * s),
            ]
        }
        Efecto::Barrido(d) => barrido(d, t),
    };
    v.into_iter().filter(|c| c.alfa > 0.0).collect()
}

/// El barrido: la nueva en su sitio hasta el borde, con el borde difuminado
/// en unas tiras (como el de PowerPoint, que no es un corte seco).
fn barrido(d: Sentido, t: f32) -> Vec<Capa> {
    // Se mueve hacia `d`: empieza en el lado contrario. Un tramo [a, b] a
    // lo largo del recorrido, en la diapositiva.
    let tramo = |a: f32, b: f32| -> [f32; 4] {
        if d.dx > 0 {
            [a, 0.0, b, 1.0]
        } else if d.dx < 0 {
            [1.0 - b, 0.0, 1.0 - a, 1.0]
        } else if d.dy > 0 {
            [0.0, a, 1.0, b]
        } else {
            [0.0, 1.0 - b, 1.0, 1.0 - a]
        }
    };
    let mut v = vec![Capa::entera(Fuente::Vieja, 1.0)];
    const BORDE: f32 = 0.12;
    const TIRAS: usize = 6;
    let e = t * (1.0 + BORDE);
    let lleno = (e - BORDE).clamp(0.0, 1.0);
    if lleno > 0.0 {
        v.push(Capa::en_su_sitio(Fuente::Nueva, tramo(0.0, lleno), 1.0));
    }
    for k in 0..TIRAS {
        let a = (e - BORDE + BORDE * k as f32 / TIRAS as f32).clamp(0.0, 1.0);
        let b = (e - BORDE + BORDE * (k + 1) as f32 / TIRAS as f32).clamp(0.0, 1.0);
        if b > a {
            v.push(Capa::en_su_sitio(
                Fuente::Nueva,
                tramo(a, b),
                1.0 - (k as f32 + 0.5) / TIRAS as f32,
            ));
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Videos y audios

/// Un video o un audio de una diapositiva.
#[derive(Debug, Clone, PartialEq)]
pub struct Medio {
    /// Su caja en la diapositiva, en puntos.
    pub rect: Rect,
    /// La parte del paquete con el fichero, si va dentro (`None` si es un
    /// enlace a fuera o falta).
    pub parte: Option<String>,
    pub audio: bool,
}

/// Extensiones de audio (para un `p14:media` que no dice si es audio).
fn es_audio(parte: &str) -> bool {
    let ext = parte.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "mp3"
            | "m4a"
            | "wav"
            | "wma"
            | "aac"
            | "aif"
            | "aiff"
            | "mid"
            | "midi"
            | "flac"
            | "ogg"
            | "opus"
    )
}

/// Si un `p:pic` es un video o un audio (su `nvPr` lleva el fichero), y de
/// cual: `(id de la relacion, si es audio)`.
pub fn medio_de(pic: &Nodo) -> Option<(String, Option<bool>)> {
    let nvpr = pic.hijo("nvPicPr").and_then(|n| n.hijo("nvPr"))?;
    // El incrustado de PowerPoint 2010 (`p14:media r:embed`) manda: el
    // `videoFile` de al lado suele ser un enlace al mismo.
    for ext in nvpr.hijo("extLst").into_iter().flat_map(|l| l.elementos()) {
        if let Some(m) = ext.hijo("media") {
            let id = m.atributo_con_prefijo("r", "embed");
            if !id.is_empty() {
                return Some((id.to_string(), None));
            }
        }
    }
    for h in nvpr.elementos() {
        let audio = match h.nombre.as_str() {
            "videoFile" | "quickTimeFile" => false,
            "audioFile" | "wavAudioFile" => true,
            _ => continue,
        };
        let id = [("r", "link"), ("r", "embed")]
            .iter()
            .map(|(p, n)| h.atributo_con_prefijo(p, n))
            .find(|s| !s.is_empty())
            .unwrap_or("");
        return Some((id.to_string(), Some(audio)));
    }
    None
}

fn recoger_medios(arbol: &Nodo, rels: &crate::paquete::Relaciones, v: &mut Vec<Medio>) {
    for n in arbol.elementos() {
        match n.nombre.as_str() {
            "pic" => {
                let Some((id, audio)) = medio_de(n) else {
                    continue;
                };
                let Some(x) = n.hijo("spPr").and_then(|s| s.hijo("xfrm")).map(dml::xfrm) else {
                    continue;
                };
                let rel = rels.get(&id).filter(|r| !r.externa);
                let parte = rel.map(|r| r.destino.clone());
                let audio = audio.unwrap_or_else(|| parte.as_deref().is_some_and(es_audio));
                v.push(Medio {
                    rect: Rect::nuevo(x.x, x.y, x.ancho, x.alto),
                    parte,
                    audio,
                });
            }
            "AlternateContent" => {
                if let Some(r) = n.hijo("Choice").or_else(|| n.hijo("Fallback")) {
                    recoger_medios(r, rels, v);
                }
            }
            _ => {}
        }
    }
}

/// El boton de reproducir que se pone encima del poster de un video o un
/// audio (`ancho` x `alto` en puntos, en el origen): un circulo oscuro
/// translucido con el triangulo blanco.
pub fn icono_reproducir(ancho: f32, alto: f32) -> Vec<Elemento> {
    let d = (ancho.min(alto) * 0.3)
        .clamp(14.0, 72.0)
        .min(ancho.min(alto));
    if d <= 0.0 || !d.is_finite() {
        return Vec::new();
    }
    let (cx, cy, r) = (ancho / 2.0, alto / 2.0, d / 2.0);
    // Un circulo en cuatro curvas.
    let k = 0.552_284_8 * r;
    let circulo = Figura {
        inicio: (cx + r, cy),
        tramos: vec![
            Tramo::Bezier((cx + r, cy + k), (cx + k, cy + r), (cx, cy + r)),
            Tramo::Bezier((cx - k, cy + r), (cx - r, cy + k), (cx - r, cy)),
            Tramo::Bezier((cx - r, cy - k), (cx - k, cy - r), (cx, cy - r)),
            Tramo::Bezier((cx + k, cy - r), (cx + r, cy - k), (cx + r, cy)),
        ],
        cerrada: true,
        rellenar: true,
        ..Default::default()
    };
    // El triangulo, algo a la derecha para que se vea centrado.
    let l = r * 0.5;
    let triangulo = Figura {
        inicio: (cx - l * 0.7, cy - l),
        tramos: vec![
            Tramo::Linea(cx + l * 1.1, cy),
            Tramo::Linea(cx - l * 0.7, cy + l),
        ],
        cerrada: true,
        rellenar: true,
        ..Default::default()
    };
    vec![
        Elemento::Trayecto {
            figuras: vec![circulo],
            relleno: Some(Relleno::Solido(Color::NEGRO.con_alfa(0.55))),
            borde: None,
        },
        Elemento::Trayecto {
            figuras: vec![triangulo],
            relleno: Some(Relleno::Solido(Color::BLANCO)),
            borde: None,
        },
    ]
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn xml(s: &str) -> Nodo {
        pixpin_docs::xml::leer(s).expect("xml")
    }

    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main""#;

    fn sld(dentro: &str) -> Nodo {
        xml(&format!(
            r#"<p:sld {NS}><p:cSld><p:spTree/></p:cSld>{dentro}</p:sld>"#
        ))
    }

    #[test]
    fn las_notas_son_el_cuerpo_sin_la_imagen_ni_el_numero() {
        let n = xml(&format!(
            r#"<p:notes {NS}><p:cSld><p:spTree>
            <p:sp><p:nvSpPr><p:cNvPr id="2" name="I"/><p:cNvSpPr/><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr></p:sp>
            <p:sp><p:nvSpPr><p:cNvPr id="3" name="N"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr>
              <p:txBody><a:bodyPr/><a:p/><a:p><a:r><a:t>Saludar</a:t></a:r><a:r><a:t> al publico</a:t></a:r></a:p><a:p><a:r><a:t>Luego</a:t></a:r><a:br/><a:r><a:t>la cifra</a:t></a:r></a:p><a:p/></p:txBody></p:sp>
            <p:sp><p:nvSpPr><p:cNvPr id="4" name="S"/><p:cNvSpPr/><p:nvPr><p:ph type="sldNum" idx="5"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:fld type="slidenum"><a:t>3</a:t></a:fld></a:p></p:txBody></p:sp>
            </p:spTree></p:cSld></p:notes>"#
        ));
        assert_eq!(
            notas_de(&n).as_deref(),
            Some("Saludar al publico\nLuego\nla cifra")
        );
    }

    #[test]
    fn sin_cuerpo_o_con_el_cuerpo_vacio_no_hay_notas() {
        let vacio = xml(&format!(
            r#"<p:notes {NS}><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="3" name="N"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:txBody><a:bodyPr/><a:p><a:endParaRPr/></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:notes>"#
        ));
        assert_eq!(notas_de(&vacio), None);
        let sin_arbol = xml(&format!(r#"<p:notes {NS}/>"#));
        assert_eq!(notas_de(&sin_arbol), None);
    }

    #[test]
    fn una_transicion_sencilla_con_su_direccion_y_su_velocidad() {
        let t = transicion_de(&sld(
            r#"<p:transition spd="slow"><p:push dir="u"/></p:transition>"#,
        ))
        .unwrap();
        assert_eq!(t.efecto, Efecto::Empuje(Sentido { dx: 0, dy: -1 }));
        assert_eq!(t.ms, 1000);
        assert_eq!(t.avance_ms, None);
        assert!(t.avance_clic);
        // Sin dir: hacia la izquierda. Sin spd: rapida.
        let t = transicion_de(&sld(r#"<p:transition><p:wipe/></p:transition>"#)).unwrap();
        assert_eq!(t.efecto, Efecto::Barrido(Sentido::IZQUIERDA));
        assert_eq!(t.ms, 500);
        let t = transicion_de(&sld(
            r#"<p:transition spd="med"><p:cover dir="rd"/></p:transition>"#,
        ))
        .unwrap();
        assert_eq!(t.efecto, Efecto::Cubrir(Sentido { dx: 1, dy: 1 }));
        assert_eq!(t.ms, 750);
        let t = transicion_de(&sld(r#"<p:transition><p:pull dir="r"/></p:transition>"#)).unwrap();
        assert_eq!(t.efecto, Efecto::Descubrir(Sentido { dx: 1, dy: 0 }));
        let t = transicion_de(&sld(
            r#"<p:transition><p:fade thruBlk="1"/></p:transition>"#,
        ))
        .unwrap();
        assert_eq!(t.efecto, Efecto::Fundido { por_negro: true });
    }

    #[test]
    fn la_de_powerpoint_2010_manda_su_duracion_y_lo_que_no_se_sabe_es_fundido() {
        // p14:prism no se pinta: su respaldo es un fundido; la duracion es la
        // de la rama nueva.
        let t = transicion_de(&sld(
            r#"<mc:AlternateContent><mc:Choice Requires="p14"><p:transition spd="slow" p14:dur="2000" advTm="3000"><p14:prism/></p:transition></mc:Choice><mc:Fallback><p:transition spd="slow" advTm="3000"><p:fade/></p:transition></mc:Fallback></mc:AlternateContent>"#,
        ))
        .unwrap();
        assert_eq!(t.efecto, Efecto::Fundido { por_negro: false });
        assert_eq!(t.ms, 2000);
        assert_eq!(t.avance_ms, Some(3000));
        // Una desconocida sin respaldo, tambien fundido.
        let t = transicion_de(&sld(
            r#"<p:transition><p14:vortex dir="r"/></p:transition>"#,
        ))
        .unwrap();
        assert_eq!(t.efecto, Efecto::Fundido { por_negro: false });
        // Un empuje en la rama nueva se respeta.
        let t = transicion_de(&sld(
            r#"<mc:AlternateContent><mc:Choice Requires="p14"><p:transition p14:dur="1250"><p:push dir="d"/></p:transition></mc:Choice><mc:Fallback><p:transition><p:fade/></p:transition></mc:Fallback></mc:AlternateContent>"#,
        ))
        .unwrap();
        assert_eq!(t.efecto, Efecto::Empuje(Sentido { dx: 0, dy: 1 }));
        assert_eq!(t.ms, 1250);
    }

    #[test]
    fn un_dur_absurdo_se_recorta_o_se_ignora() {
        let t = transicion_de(&sld(
            r#"<p:transition p14:dur="99999999999"><p:fade/></p:transition>"#,
        ))
        .unwrap();
        assert_eq!(t.ms, MS_MAXIMO);
        let t = transicion_de(&sld(
            r#"<p:transition spd="med" p14:dur="-5"><p:fade/></p:transition>"#,
        ))
        .unwrap();
        assert_eq!(t.ms, 750, "negativo: cuenta la velocidad");
        let t = transicion_de(&sld(
            r#"<p:transition p14:dur="NaN" advTm="mucho"><p:fade/></p:transition>"#,
        ))
        .unwrap();
        assert_eq!(t.ms, 500);
        assert_eq!(t.avance_ms, None);
        let t = transicion_de(&sld(r#"<p:transition advTm="1e30" advClick="0"/>"#)).unwrap();
        assert_eq!(t.avance_ms, Some(AVANCE_MAXIMO));
        assert!(!t.avance_clic);
        assert_eq!(
            t.efecto,
            Efecto::Corte { por_negro: false },
            "sin efecto, un corte"
        );
        assert_eq!(t.ms, 0);
    }

    #[test]
    fn sin_transicion_no_hay_nada() {
        assert_eq!(transicion_de(&sld("")), None);
        assert_eq!(Sentido::de("x"), Sentido::IZQUIERDA);
    }

    #[test]
    fn el_progreso_va_por_el_tiempo_y_no_se_pasa() {
        assert_eq!(progreso(0, 1000), 0.0);
        assert!((progreso(250, 1000) - 0.25).abs() < 1e-6);
        assert_eq!(progreso(5000, 1000), 1.0);
        assert_eq!(progreso(0, 0), 1.0, "una de cero ms ya acabo");
        assert_eq!(suave(0.0), 0.0);
        assert_eq!(suave(1.0), 1.0);
        assert!((suave(0.5) - 0.5).abs() < 1e-6);
        assert!(suave(0.25) < 0.25, "arranca despacio");
    }

    #[test]
    fn la_mezcla_empieza_en_la_vieja_y_acaba_en_la_nueva() {
        let todas = [
            Efecto::Fundido { por_negro: false },
            Efecto::Fundido { por_negro: true },
            Efecto::Empuje(Sentido::IZQUIERDA),
            Efecto::Barrido(Sentido { dx: 0, dy: 1 }),
            Efecto::Cubrir(Sentido { dx: 1, dy: -1 }),
            Efecto::Descubrir(Sentido::IZQUIERDA),
        ];
        for e in todas {
            assert_eq!(
                capas(e, 0.0),
                vec![Capa::entera(Fuente::Vieja, 1.0)],
                "{e:?}"
            );
            assert_eq!(
                capas(e, 1.0),
                vec![Capa::entera(Fuente::Nueva, 1.0)],
                "{e:?}"
            );
            assert_eq!(
                capas(e, f32::NAN),
                vec![Capa::entera(Fuente::Vieja, 1.0)],
                "{e:?}"
            );
            for c in capas(e, 0.4) {
                assert!(c.alfa > 0.0 && c.alfa <= 1.0, "{e:?} {c:?}");
            }
        }
        // Un corte por negro: negro la primera mitad.
        assert_eq!(
            capas(Efecto::Corte { por_negro: true }, 0.2)[0].fuente,
            Fuente::Negro
        );
        assert_eq!(
            capas(Efecto::Corte { por_negro: false }, 0.0)[0].fuente,
            Fuente::Nueva
        );
    }

    #[test]
    fn el_empuje_hacia_la_izquierda_mete_la_nueva_por_la_derecha() {
        let c = capas(Efecto::Empuje(Sentido::IZQUIERDA), 0.5);
        assert_eq!(c.len(), 2);
        let (vieja, nueva) = (c[0], c[1]);
        assert_eq!(vieja.fuente, Fuente::Vieja);
        assert!(
            (vieja.destino[0] + 0.5).abs() < 1e-6,
            "la vieja a media salida: {vieja:?}"
        );
        assert!(
            (nueva.destino[0] - 0.5).abs() < 1e-6,
            "la nueva a media entrada: {nueva:?}"
        );
        // El fundido va lineal: a la mitad, la nueva a medias.
        let f = capas(Efecto::Fundido { por_negro: false }, 0.5);
        assert!((f[1].alfa - 0.5).abs() < 1e-6);
    }

    #[test]
    fn el_barrido_hacia_la_izquierda_destapa_desde_la_derecha() {
        let c = capas(Efecto::Barrido(Sentido::IZQUIERDA), 0.5);
        let llena = c
            .iter()
            .find(|c| c.fuente == Fuente::Nueva && c.alfa == 1.0)
            .unwrap();
        assert!(
            (llena.destino[2] - 1.0).abs() < 1e-6,
            "pegada a la derecha: {llena:?}"
        );
        assert!(
            llena.destino[0] > 0.4 && llena.destino[0] < 0.6,
            "{llena:?}"
        );
        assert_eq!(llena.origen, llena.destino, "en su sitio");
    }

    #[test]
    fn un_video_y_un_audio_se_reconocen_y_lo_demas_no() {
        let video = xml(&format!(
            r#"<p:pic {NS}><p:nvPicPr><p:cNvPr id="4" name="V"/><p:cNvPicPr/><p:nvPr><a:videoFile r:link="rV"/><p:extLst><p:ext uri="x"><p14:media r:embed="rM"/></p:ext></p:extLst></p:nvPr></p:nvPicPr></p:pic>"#
        ));
        assert_eq!(medio_de(&video), Some(("rM".into(), None)));
        let audio = xml(&format!(
            r#"<p:pic {NS}><p:nvPicPr><p:cNvPr id="4" name="A"/><p:cNvPicPr/><p:nvPr><a:audioFile r:link="rA"/></p:nvPr></p:nvPicPr></p:pic>"#
        ));
        assert_eq!(medio_de(&audio), Some(("rA".into(), Some(true))));
        let foto = xml(&format!(
            r#"<p:pic {NS}><p:nvPicPr><p:cNvPr id="4" name="F"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr></p:pic>"#
        ));
        assert_eq!(medio_de(&foto), None);
        assert!(es_audio("ppt/media/media1.M4A") && !es_audio("ppt/media/media1.mp4"));
    }

    #[test]
    fn el_icono_de_reproducir_cabe_en_el_poster() {
        let els = icono_reproducir(200.0, 100.0);
        assert_eq!(els.len(), 2);
        for e in &els {
            let Elemento::Trayecto { figuras, .. } = e else {
                panic!("{e:?}")
            };
            for f in figuras {
                assert!(
                    f.inicio.0 >= 0.0
                        && f.inicio.0 <= 200.0
                        && f.inicio.1 >= 0.0
                        && f.inicio.1 <= 100.0
                );
            }
        }
        assert!(
            icono_reproducir(0.0, 0.0).is_empty(),
            "un poster sin tamano no lleva icono"
        );
    }
}
