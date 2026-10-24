//! **Parrafos y tablas a franjas.** Un bloque del cuerpo (un `w:p` o una
//! `w:tbl`) se convierte en una lista de [`Franja`]s: una por renglon de
//! parrafo o por fila de tabla. La franja ya sabe su alto y lleva sus
//! elementos con x desde el borde de la columna e y desde su propio borde
//! de arriba; quien la coloca en la pagina (el paginador de `mod.rs`) solo
//! decide en que pagina y a que altura cae.

use std::collections::HashMap;
use std::sync::Arc;

use pixpin_docs::xml::Nodo;

use crate::docx::dibujo::{self, Dibujo, Flotante, Objeto};
use crate::docx::estilos::{
    AnchoW, Bordes, ColorW, Estilos, Familia, Interlineado, Margenes, PropCelda, PropCorrida,
    PropParrafo, PropTabla, Vertical,
};
use crate::docx::numeracion::Numeracion;
use crate::medidor::Medidor;
use crate::pagina::{Campo, Color, Elemento, Letra, Rect, Relleno, Texto, Trazo};
use crate::paquete::{Paquete, Relaciones, attr_num, twips};
use crate::renglones::{self, Alineacion, Config, Contenido, Estilo, Pieza, TipoTope, Tope};
use crate::tema::Tema;

/// Todo lo que la maquetacion de un bloque necesita del documento.
pub struct Ctx<'a> {
    pub estilos: &'a Estilos,
    pub tema: &'a Tema,
    pub numeracion: &'a mut Numeracion,
    pub paquete: &'a mut Paquete,
    pub recursos: &'a crate::pagina::Recursos,
    /// Las relaciones de la parte que se esta leyendo (cuerpo, encabezado...).
    pub rels: &'a Relaciones,
    pub medidor: &'a dyn Medidor,
    pub tope_defecto: f32,
    /// `compatibilityMode` de Word (15 = Word 2013 y posteriores).
    pub compat: u32,
    /// El numero de la pagina en curso, para los campos `PAGE`.
    pub pagina: u32,
    /// El total de paginas que se cree que habra (para medir `NUMPAGES`).
    pub total: u32,
    /// Lo que el estilo de la tabla en curso aporta a sus parrafos.
    pub base_tabla: Option<(PropParrafo, PropCorrida)>,
    /// Contador de notas al pie.
    pub notas: u32,
    /// Ids de notas al pie citadas (para pintarlas al pie de la pagina).
    pub notas_citadas: Vec<(String, u32)>,
    /// Letra → alternativas (`fontTable.xml`), por si no esta instalada.
    pub alternas: &'a HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Salto {
    #[default]
    Ninguno,
    Pagina,
    Columna,
}

/// Un trozo de bloque que va entero en una pagina.
#[derive(Debug, Clone, Default)]
pub struct Franja {
    pub alto: f32,
    /// Espacio antes (se deja encima de la franja).
    pub antes: f32,
    /// Espacio despues (debajo; no empuja a la pagina siguiente).
    pub despues: f32,
    pub elementos: Vec<Elemento>,
    /// Tiene que ir en la misma pagina que la siguiente.
    pub pegada: bool,
    pub salto: Salto,
    pub flotantes: Vec<Flotante>,
    /// Una fila de tabla que se puede partir entre paginas.
    pub fila: Option<Box<FilaMaq>>,
    /// Las filas de encabezado de su tabla, para repetirlas.
    pub cabecera: Option<Arc<Vec<Franja>>>,
    pub es_cabecera: bool,
    /// Estilo del parrafo y si tiene espaciado contextual.
    pub estilo: Option<String>,
    pub contextual: bool,
    /// Empieza la seccion `n` (lo pone el paginador).
    pub seccion: Option<usize>,
    /// Ids de notas al pie citadas en esta franja.
    pub notas: Vec<(String, u32)>,
    /// El espacio antes/despues es el automatico de HTML (Word lo quita al
    /// principio y al final de una celda).
    pub auto_antes: bool,
    pub auto_despues: bool,
    /// El salto viene de un `w:br` (no de «salto de pagina antes»): tras
    /// el, Word no deja el espacio antes del parrafo.
    pub salto_manual: bool,
    /// Pegada a la siguiente por «conservar lineas juntas»: solo cuenta
    /// para el propio parrafo, no cuando otro se pega a este.
    pub pegada_lineas: bool,
    /// Primera franja de un bloque.
    pub inicio_bloque: bool,
    /// Espacio de abajo que puede quedar por debajo del margen (el de mas
    /// del interlineado multiple): Word solo mira que quepa el texto.
    pub holgura: f32,
    /// Con que volver a partir este renglon y los que le siguen (los de un
    /// parrafo), si un dibujo flotante se los estrecha.
    pub rehacer: Option<Arc<Rehacer>>,
    /// Que renglon del tramo de parrafo es, y en que unidad empieza.
    pub linea: usize,
    pub unidad: usize,
    /// Los tramos (x de la caja) en que se partio el renglon; vacio = a lo
    /// ancho de la caja.
    pub limites: Vec<(f32, f32)>,
}

/// La geometria de un tramo de parrafo que hace falta para montar sus
/// franjas.
#[derive(Debug, Clone)]
pub struct Geometria {
    interlineado: Interlineado,
    izq: f32,
    der: f32,
    primera: f32,
    ancho: f32,
    sombreado: Option<Color>,
    hay_bordes: bool,
    borde_arriba: Option<Trazo>,
    borde_abajo: Option<Trazo>,
    borde_izq: Option<Trazo>,
    borde_der: Option<Trazo>,
    estilo: Option<String>,
    contextual: bool,
    antes: f32,
    despues: f32,
    auto_antes: bool,
    auto_despues: bool,
    salto_antes: bool,
    /// El salto que abre este tramo (los de despues del primero).
    salto_tramo: Salto,
    primer_tramo: bool,
    ultimo_tramo: bool,
    lineas_juntas: bool,
    viudas: bool,
    con_siguiente: bool,
    /// Las notas al pie citadas en el parrafo (van con su ultimo renglon).
    notas: Vec<(String, u32)>,
}

/// Lo que hace falta para volver a partir un tramo de parrafo ya
/// maquetado: sus unidades medidas, estilos, objetos en linea y geometria.
#[derive(Debug)]
pub struct Rehacer {
    unidades: Vec<renglones::Unidad>,
    estilos: Arc<[Estilo]>,
    objetos: Arc<[Objeto]>,
    pub cfg: Config,
    marca: u32,
    g: Geometria,
}

/// Contenido apilado en una caja (celda, cuadro de texto, encabezado).
#[derive(Debug, Clone, Default)]
pub struct Caja {
    pub elementos: Vec<Elemento>,
    pub alto: f32,
    /// Alturas donde se puede cortar (fin de cada franja).
    pub cortes: Vec<f32>,
    pub flotantes: Vec<(f32, Flotante)>,
}

/// Apila los bloques en una caja de ancho `ancho`, sin paginar.
pub fn caja<'n>(ctx: &mut Ctx, nodos: impl Iterator<Item = &'n Nodo>, ancho: f32) -> Caja {
    let mut franjas = Vec::new();
    let nodos: Vec<&Nodo> = aplanar(nodos);
    for (i, n) in nodos.iter().enumerate() {
        bloque(
            ctx,
            n,
            ancho,
            nodos.get(i.wrapping_sub(1)).copied(),
            nodos.get(i + 1).copied(),
            &mut franjas,
        );
    }
    if let Some(f) = franjas.first_mut()
        && f.auto_antes
    {
        f.antes = 0.0;
    }
    if let Some(f) = franjas.last_mut()
        && f.auto_despues
    {
        f.despues = 0.0;
    }
    apilar(franjas)
}

/// Una caja (celda, cuadro de texto) con sus dibujos anclados ya puestos
/// dentro: en una celda, «columna» y «parrafo» son los de la celda.
pub fn caja_con_flotantes<'n>(
    ctx: &mut Ctx,
    nodos: impl Iterator<Item = &'n Nodo>,
    ancho: f32,
) -> Caja {
    let mut c = caja(ctx, nodos, ancho);
    for (y_parrafo, f) in std::mem::take(&mut c.flotantes) {
        use crate::docx::dibujo::Referencia;
        let o = &f.objeto;
        let x = match f.h.alineacion.as_deref() {
            Some("center") => (ancho - o.ancho) / 2.0,
            Some("right") | Some("outside") => ancho - o.ancho,
            Some(_) => 0.0,
            None => f.h.desplazamiento,
        };
        let y = match f.v.referencia {
            Referencia::Parrafo => y_parrafo + f.v.desplazamiento,
            _ => f.v.desplazamiento.max(0.0),
        };
        let mut els: Vec<Elemento> = f
            .objeto
            .elementos
            .into_iter()
            .map(|mut e| {
                e.trasladar(x, y);
                e
            })
            .collect();
        if f.detras {
            els.append(&mut c.elementos);
            c.elementos = els;
        } else {
            c.elementos.append(&mut els);
        }
    }
    c
}

/// Lo que deja una franja colocada para la siguiente: su estilo, si tiene
/// espaciado contextual y su espacio despues.
pub type Previa = Option<(Option<String>, bool, f32)>;

/// Cuanto hay que subir (quitar del espacio despues de la anterior) y
/// cuanto bajar antes de colocar `f`.
///
/// Word no suma el espacio despues de un parrafo y el espacio antes del
/// siguiente: deja el mayor de los dos (medido contra sus PDF). Con el
/// espaciado contextual entre parrafos del mismo estilo no deja ninguno.
pub fn espacio_entre(previa: &Previa, f: &Franja) -> (f32, f32) {
    match previa {
        Some((est, ctxl, desp))
            if f.contextual && *ctxl && *est == f.estilo && f.estilo.is_some() =>
        {
            (*desp, 0.0)
        }
        Some((_, _, desp)) => (0.0, (f.antes - desp).max(0.0)),
        None => (0.0, f.antes),
    }
}

/// Pone una franja tras otra.
pub fn apilar(franjas: Vec<Franja>) -> Caja {
    let mut c = Caja::default();
    let mut y = 0.0;
    let mut previa: Previa = None;
    for f in franjas {
        let (retroceso, antes) = espacio_entre(&previa, &f);
        y -= retroceso;
        y += antes;
        let alto = f.alto;
        for mut e in materializar(&f) {
            e.trasladar(0.0, y);
            c.elementos.push(e);
        }
        for fl in f.flotantes {
            c.flotantes.push((y, fl));
        }
        y += alto;
        c.cortes.push(y);
        y += f.despues;
        previa = Some((f.estilo.clone(), f.contextual, f.despues));
    }
    c.alto = y;
    c
}

/// Los elementos de una franja (las filas de tabla se pintan aqui).
pub fn materializar(f: &Franja) -> Vec<Elemento> {
    match &f.fila {
        Some(fila) => fila.elementos(f.alto),
        None => f.elementos.clone(),
    }
}

/// Los bloques de verdad: un `w:sdt` o un `w:customXml` se abren.
pub fn aplanar<'n>(nodos: impl Iterator<Item = &'n Nodo>) -> Vec<&'n Nodo> {
    let mut v = Vec::new();
    for n in nodos {
        match n.nombre.as_str() {
            "p" | "tbl" => v.push(n),
            "sdt" => {
                if let Some(c) = n.hijo("sdtContent") {
                    v.extend(aplanar(c.elementos()));
                }
            }
            "customXml" | "smartTag" | "ins" => v.extend(aplanar(n.elementos())),
            "AlternateContent" => {
                if let Some(c) = n.hijo("Choice").or_else(|| n.hijo("Fallback")) {
                    v.extend(aplanar(c.elementos()));
                }
            }
            _ => {}
        }
    }
    v
}

/// Un bloque a franjas.
pub fn bloque(
    ctx: &mut Ctx,
    n: &Nodo,
    ancho: f32,
    previo: Option<&Nodo>,
    siguiente: Option<&Nodo>,
    salida: &mut Vec<Franja>,
) {
    let desde = salida.len();
    match n.nombre.as_str() {
        "p" => parrafo(ctx, n, ancho, previo, siguiente, salida),
        "tbl" => tabla(ctx, n, ancho, salida),
        _ => {}
    }
    if let Some(f) = salida.get_mut(desde) {
        f.inicio_bloque = true;
    }
}

// ---------------------------------------------------------------------
// Parrafos
// ---------------------------------------------------------------------

/// Las propiedades efectivas de un parrafo y la base de sus corridas.
pub struct Resuelto {
    pub p: PropParrafo,
    pub r: PropCorrida,
    pub estilo: Option<String>,
}

pub fn resolver_parrafo(ctx: &Ctx, n: &Nodo) -> Resuelto {
    let directo = n
        .hijo("pPr")
        .map(|p| PropParrafo::leer(p, ctx.tema))
        .unwrap_or_default();
    let estilo = directo
        .estilo
        .clone()
        .or_else(|| ctx.estilos.defecto_parrafo.clone());
    let (sp, sr) = estilo
        .as_deref()
        .map(|e| ctx.estilos.de_parrafo(e))
        .unwrap_or_default();
    let mut p = ctx.estilos.doc_ppr.clone();
    let mut r = ctx.estilos.doc_rpr.clone();
    if let Some((tp, tr)) = &ctx.base_tabla {
        p.mezclar(tp);
        r.mezclar(tr);
    }
    let num_directo = directo.num_id.is_some();
    let num_id = directo.num_id.or(sp.num_id).unwrap_or(0);
    let nivel_n = directo.nivel.or(sp.nivel).unwrap_or(0);
    let nivel = if num_id > 0 {
        ctx.numeracion.nivel(num_id, nivel_n)
    } else {
        None
    };
    if num_directo {
        p.mezclar(&sp);
        if let Some(l) = &nivel {
            p.mezclar(&l.ppr);
        }
    } else {
        if let Some(l) = &nivel {
            p.mezclar(&l.ppr);
        }
        p.mezclar(&sp);
    }
    p.mezclar(&directo);
    p.num_id = Some(num_id);
    p.nivel = Some(nivel_n);
    r.mezclar(&sr);
    Resuelto { p, r, estilo }
}

/// Lo que hace falta para convertir propiedades de corrida en estilos.
struct Montador<'t> {
    tema: &'t Tema,
    medidor: &'t dyn Medidor,
    alternas: &'t HashMap<String, String>,
    estilos: Vec<Estilo>,
}

impl Montador<'_> {
    fn familia(&self, f: &Option<Familia>) -> String {
        let n = match f {
            Some(Familia::Nombre(n)) => n.clone(),
            Some(Familia::Tema(true)) => self.tema.letra_titulos.clone(),
            Some(Familia::Tema(false)) => self.tema.letra_texto.clone(),
            None => "Times New Roman".into(),
        };
        // Una letra que no esta instalada: la alternativa que el propio
        // documento apunta en `fontTable.xml` (lo que hace Word).
        if self.medidor.existe(&n) {
            return n;
        }
        // «Arial Bold», «Verdana-Italic»: la familia con el estilo pegado
        // al nombre (la negrita o cursiva ya va en la corrida).
        for sufijo in [
            " Bold",
            " Italic",
            " Bold Italic",
            " Light",
            " Regular",
            " Semibold",
            " Black",
            "-Bold",
            "-Italic",
            "-Regular",
            ",Bold",
        ] {
            if let Some(base) = n.strip_suffix(sufijo)
                && self.medidor.existe(base)
            {
                return base.to_string();
            }
        }
        if let Some(a) = self.alternas.get(&n) {
            for x in a.split(',') {
                if self.medidor.existe(x.trim()) {
                    return x.trim().to_string();
                }
            }
        }
        n
    }

    /// El indice del estilo para unas propiedades (lo crea si no estaba).
    fn estilo(
        &mut self,
        r: &PropCorrida,
        cjk: bool,
        enlace: Option<&Arc<str>>,
        campo: Option<Campo>,
        escala: f32,
    ) -> u32 {
        let familia = if cjk && r.familia_ea.is_some() {
            self.familia(&r.familia_ea)
        } else {
            self.familia(&r.familia)
        };
        let mut tam = r.tam.unwrap_or(10.0) * escala;
        let mut subida = r.posicion.unwrap_or(0.0);
        match r.vertical {
            Some(Vertical::Super) => {
                subida += tam * 0.33;
                tam *= 0.65;
            }
            Some(Vertical::Sub) => {
                subida -= tam * 0.14;
                tam *= 0.65;
            }
            _ => {}
        }
        let color = match r.color {
            Some(ColorW::Fijo(c)) => c,
            _ => Color::NEGRO,
        };
        let e = Estilo {
            letra: Letra {
                familia: Arc::from(familia.as_str()),
                tam: tam.max(1.0),
                negrita: r.negrita.unwrap_or(false),
                cursiva: r.cursiva.unwrap_or(false),
            },
            color,
            subrayado: r.subrayado.flatten(),
            tachado: r.tachado.unwrap_or(false),
            espaciado: r.espaciado.unwrap_or(0.0),
            subida,
            fondo: r.resaltado.flatten().or(r.sombreado.flatten()),
            enlace: enlace.cloned(),
            campo,
            efectos: None,
        };
        if let Some(i) = self.estilos.iter().position(|x| *x == e) {
            return i as u32;
        }
        self.estilos.push(e);
        (self.estilos.len() - 1) as u32
    }
}

fn tiene_cjk(s: &str) -> bool {
    s.chars().any(|c| matches!(c as u32, 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF))
}

/// Lo que va saliendo al recorrer un parrafo.
struct Recogida<'t> {
    m: Montador<'t>,
    /// Trozos de parrafo separados por saltos de pagina o columna.
    tramos: Vec<(Salto, Vec<Pieza>)>,
    objetos: Vec<Objeto>,
    flotantes: Vec<Flotante>,
    /// Campos abiertos: (instruccion, ya en el resultado, ocultar resultado).
    campos: Vec<(String, bool, bool)>,
    topes_extra: Vec<Tope>,
    notas: Vec<(String, u32)>,
    /// El ancho de la caja (para las rayas de lado a lado).
    ancho: f32,
    /// El parrafo cierra una seccion: Word no hace caso a sus saltos de
    /// pagina (la seccion siguiente ya dice como empieza).
    cierra_seccion: bool,
}

impl Recogida<'_> {
    fn piezas(&mut self) -> &mut Vec<Pieza> {
        if self.tramos.is_empty() {
            self.tramos.push((Salto::Ninguno, Vec::new()));
        }
        &mut self.tramos.last_mut().expect("hay al menos un tramo").1
    }

    /// Si lo que llega ahora se ve (no es la instruccion de un campo ni el
    /// resultado de uno que se calcula aqui).
    fn visible(&self) -> bool {
        self.campos
            .iter()
            .all(|(_, res, ocultar)| *res && !*ocultar)
    }

    fn texto(&mut self, t: &str, r: &PropCorrida, enlace: Option<&Arc<str>>) {
        if t.is_empty() || !self.visible() {
            return;
        }
        let cjk = tiene_cjk(t);
        if r.versalitas == Some(true) && r.mayusculas != Some(true) {
            // Versalitas: las minusculas en mayuscula y mas pequenas.
            let mut actual = String::new();
            let mut minus = false;
            let flush = |s: &mut Self, txt: &mut String, minus: bool| {
                if !txt.is_empty() {
                    let e =
                        s.m.estilo(r, cjk, enlace, None, if minus { 0.8 } else { 1.0 });
                    let texto = std::mem::take(txt);
                    s.piezas().push(Pieza::Texto { texto, estilo: e });
                }
            };
            for c in t.chars() {
                let es_min = c.is_lowercase();
                if es_min != minus {
                    flush(self, &mut actual, minus);
                    minus = es_min;
                }
                actual.extend(c.to_uppercase());
            }
            flush(self, &mut actual, minus);
            return;
        }
        let texto = if r.mayusculas == Some(true) {
            t.to_uppercase()
        } else {
            t.to_string()
        };
        let e = self.m.estilo(r, cjk, enlace, None, 1.0);
        self.piezas().push(Pieza::Texto { texto, estilo: e });
    }
}

/// Evalua la instruccion de un campo que depende de la paginacion.
fn campo_de(instr: &str) -> Option<Campo> {
    let palabra = instr
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    match palabra.as_str() {
        "PAGE" => Some(Campo::Pagina),
        "NUMPAGES" | "SECTIONPAGES" => Some(Campo::TotalPaginas),
        _ => None,
    }
}

fn texto_campo(ctx: &Ctx, c: Campo, instr: &str) -> String {
    let n = match c {
        Campo::Pagina => ctx.pagina,
        Campo::TotalPaginas => ctx.total.max(1),
    };
    let up = instr.to_ascii_uppercase();
    if up.contains("ROMAN") {
        let r = crate::docx::numeracion::formatear(n as i64, "upperRoman");
        if instr.contains("roman") {
            r.to_lowercase()
        } else {
            r
        }
    } else if up.contains("ALPHABETIC") {
        crate::docx::numeracion::formatear(
            n as i64,
            if instr.contains("alphabetic") {
                "lowerLetter"
            } else {
                "upperLetter"
            },
        )
    } else {
        n.to_string()
    }
}

/// Recorre el contenido de un parrafo (o de un hipervinculo, un campo...).
fn recorrer(
    ctx: &mut Ctx,
    rc: &mut Recogida,
    n: &Nodo,
    base: &PropCorrida,
    enlace: Option<&Arc<str>>,
) {
    for e in n.elementos() {
        match e.nombre.as_str() {
            "r" => corrida(ctx, rc, e, base, enlace),
            "hyperlink" => {
                let rid = e.atributo_con_prefijo("r", "id");
                let destino = ctx.rels.get(rid).map(|r| r.destino.clone()).or_else(|| {
                    (!e.atributo("anchor").is_empty()).then(|| format!("#{}", e.atributo("anchor")))
                });
                let destino = destino.map(|d| Arc::from(d.as_str()));
                recorrer(ctx, rc, e, base, destino.as_ref().or(enlace));
            }
            "fldSimple" => {
                let instr = e.atributo("instr").to_string();
                if let Some(c) = campo_de(&instr) {
                    let r = primera_rpr(ctx, e, base);
                    let t = texto_campo(ctx, c, &instr);
                    if rc.visible() {
                        let est = rc.m.estilo(&r, false, enlace, Some(c), 1.0);
                        rc.piezas().push(Pieza::Texto {
                            texto: t,
                            estilo: est,
                        });
                    }
                } else {
                    recorrer(ctx, rc, e, base, enlace);
                }
            }
            "smartTag" | "customXml" | "ins" | "moveTo" | "bdo" | "dir" => {
                recorrer(ctx, rc, e, base, enlace)
            }
            "sdt" => {
                if let Some(c) = e.hijo("sdtContent") {
                    recorrer(ctx, rc, c, base, enlace);
                }
            }
            "AlternateContent" => {
                if let Some(c) = e.hijo("Choice").or_else(|| e.hijo("Fallback")) {
                    recorrer(ctx, rc, c, base, enlace);
                }
            }
            "oMath" | "oMathPara" => {
                // Las formulas de Office (OMML), maquetadas como objeto en
                // la linea.
                if !rc.visible() {
                    continue;
                }
                let tam = base.tam.unwrap_or(11.0);
                let color = match base.color {
                    Some(ColorW::Fijo(c)) => c,
                    _ => Color::NEGRO,
                };
                let formulas: Vec<&Nodo> = if e.nombre == "oMathPara" {
                    e.elementos().filter(|x| x.nombre == "oMath").collect()
                } else {
                    vec![e]
                };
                for (k, f) in formulas.into_iter().enumerate() {
                    if k > 0 {
                        rc.piezas().push(Pieza::Salto);
                    }
                    let o =
                        crate::docx::mate::formula(f, tam, color, e.nombre == "oMath", ctx.medidor);
                    poner_dibujo(rc, Dibujo::EnLinea(o));
                }
            }
            _ => {}
        }
    }
}

/// Las propiedades de la primera corrida dentro de `n` (para los campos
/// simples, que llevan el formato en su resultado).
fn primera_rpr(ctx: &Ctx, n: &Nodo, base: &PropCorrida) -> PropCorrida {
    let mut r = base.clone();
    if let Some(rpr) = n.descendientes("rPr").first() {
        let d = PropCorrida::leer(rpr, ctx.tema);
        if let Some(s) = &d.estilo {
            r.mezclar(&ctx.estilos.de_caracter(s));
        }
        r.mezclar(&d);
    }
    r
}

fn corrida(
    ctx: &mut Ctx,
    rc: &mut Recogida,
    r: &Nodo,
    base: &PropCorrida,
    enlace: Option<&Arc<str>>,
) {
    let mut props = base.clone();
    if let Some(rpr) = r.hijo("rPr") {
        let d = PropCorrida::leer(rpr, ctx.tema);
        let mut estilo = d
            .estilo
            .clone()
            .or_else(|| ctx.estilos.defecto_caracter.clone());
        // Los vinculos de un indice (`_Toc...`) se ven como texto normal en
        // Word aunque lleven el estilo «Hyperlink».
        if enlace.is_some_and(|e| e.starts_with("#_Toc")) && estilo.as_deref() == Some("Hyperlink")
        {
            estilo = None;
        }
        if let Some(s) = estilo {
            props.mezclar(&ctx.estilos.de_caracter(&s));
        }
        props.mezclar(&d);
    }
    if props.oculto == Some(true) {
        // Un campo puede abrirse o cerrarse en una corrida oculta: eso si
        // cuenta.
        for e in r.elementos() {
            if e.nombre == "fldChar" {
                campo_char(ctx, rc, e, &props, enlace);
            }
        }
        return;
    }
    for e in r.elementos() {
        match e.nombre.as_str() {
            "t" => rc.texto(&e.texto(), &props, enlace),
            "tab" => {
                if rc.visible() {
                    let est = rc.m.estilo(&props, false, enlace, None, 1.0);
                    rc.piezas().push(Pieza::Tab { estilo: est });
                }
            }
            "ptab" => {
                if rc.visible() {
                    let est = rc.m.estilo(&props, false, enlace, None, 1.0);
                    // Un tabulador absoluto: alineado al margen.
                    let tipo = match e.atributo("alignment") {
                        "center" => TipoTope::Centro,
                        "right" => TipoTope::Derecha,
                        _ => TipoTope::Izquierda,
                    };
                    rc.topes_extra.push(Tope {
                        pos: f32::NAN,
                        tipo,
                        relleno: None,
                    });
                    rc.piezas().push(Pieza::Tab { estilo: est });
                }
            }
            "br" => {
                if !rc.visible() {
                    continue;
                }
                match e.atributo("type") {
                    "page" if rc.cierra_seccion => {}
                    "page" => {
                        rc.piezas();
                        rc.tramos.push((Salto::Pagina, Vec::new()))
                    }
                    "column" => {
                        rc.piezas();
                        rc.tramos.push((Salto::Columna, Vec::new()))
                    }
                    _ => rc.piezas().push(Pieza::Salto),
                }
            }
            "cr" => {
                if rc.visible() {
                    rc.piezas().push(Pieza::Salto)
                }
            }
            "noBreakHyphen" => rc.texto("\u{2011}", &props, enlace),
            "softHyphen" => rc.texto("\u{00AD}", &props, enlace),
            "sym" => {
                let c = u32::from_str_radix(e.atributo("char"), 16)
                    .ok()
                    .and_then(char::from_u32);
                if let Some(c) = c {
                    let mut p2 = props.clone();
                    let f = e.atributo("font");
                    if !f.is_empty() {
                        p2.familia = Some(Familia::Nombre(f.to_string()));
                    }
                    // Los simbolos de Symbol/Wingdings vienen en F0xx.
                    let c = if (c as u32) < 0x100 && !f.is_empty() {
                        char::from_u32(0xF000 + c as u32).unwrap_or(c)
                    } else {
                        c
                    };
                    rc.texto(&c.to_string(), &p2, enlace);
                }
            }
            "drawing" => {
                if rc.visible()
                    && let Some(d) = dibujo::dibujo(ctx, e)
                {
                    poner_dibujo(rc, d);
                }
            }
            "pict" | "object" => {
                if rc.visible()
                    && let Some(d) = dibujo::vml(ctx, e)
                {
                    poner_dibujo(rc, d);
                }
            }
            "AlternateContent" => {
                // La primera rama, salvo un grafico nuevo con respaldo.
                if let Some(c) = crate::grafico::rama_alternativa(e) {
                    for x in c.elementos() {
                        let d = match x.nombre.as_str() {
                            "drawing" => dibujo::dibujo(ctx, x),
                            "pict" | "object" => dibujo::vml(ctx, x),
                            _ => None,
                        };
                        if let Some(d) = d
                            && rc.visible()
                        {
                            poner_dibujo(rc, d);
                        }
                    }
                }
            }
            "fldChar" => campo_char(ctx, rc, e, &props, enlace),
            "instrText" => {
                if let Some(c) = rc.campos.last_mut()
                    && !c.1
                {
                    c.0.push_str(&e.texto());
                }
            }
            "footnoteReference" | "endnoteReference" => {
                if rc.visible() {
                    ctx.notas += 1;
                    let mut p2 = props.clone();
                    if p2.vertical.is_none() {
                        p2.vertical = Some(Vertical::Super);
                    }
                    let n = ctx.notas.to_string();
                    rc.texto(&n, &p2, enlace);
                    if e.nombre == "footnoteReference" {
                        rc.notas.push((e.atributo("id").to_string(), ctx.notas));
                    }
                }
            }
            "footnoteRef" | "endnoteRef" => {
                // La marca dentro de la propia nota: el numero que le toco.
                let n = ctx.notas.max(1).to_string();
                rc.texto(&n, &props, enlace);
            }
            "separator" | "continuationSeparator" => {}
            _ => {}
        }
    }
}

fn campo_char(
    ctx: &Ctx,
    rc: &mut Recogida,
    e: &Nodo,
    props: &PropCorrida,
    enlace: Option<&Arc<str>>,
) {
    match e.atributo("fldCharType") {
        "begin" => rc.campos.push((String::new(), false, false)),
        "separate" => {
            let Some(top) = rc.campos.last().cloned() else {
                return;
            };
            let calculado = campo_de(&top.0);
            if let Some(c) = rc.campos.last_mut() {
                c.1 = true;
                c.2 = calculado.is_some();
            }
            if let Some(c) = calculado {
                // Visible si los de fuera lo son.
                let n = rc.campos.len();
                let fuera_visible = rc.campos[..n - 1].iter().all(|(_, res, oc)| *res && !*oc);
                if fuera_visible {
                    let t = texto_campo(ctx, c, &top.0);
                    let est = rc.m.estilo(props, false, enlace, Some(c), 1.0);
                    rc.piezas().push(Pieza::Texto {
                        texto: t,
                        estilo: est,
                    });
                }
            }
        }
        "end" => {
            if let Some((instr, separado, _)) = rc.campos.pop()
                && !separado
                && let Some(c) = campo_de(&instr)
                && rc.visible()
            {
                let t = texto_campo(ctx, c, &instr);
                let est = rc.m.estilo(props, false, enlace, Some(c), 1.0);
                rc.piezas().push(Pieza::Texto {
                    texto: t,
                    estilo: est,
                });
            }
        }
        _ => {}
    }
}

fn poner_dibujo(rc: &mut Recogida, d: Dibujo) {
    match d {
        Dibujo::EnLinea(o) => {
            let id = rc.objetos.len() as u32;
            let (ancho, alto, bajo) = (o.ancho, o.alto, o.bajo);
            rc.objetos.push(o);
            rc.piezas().push(Pieza::Objeto {
                ancho,
                alto,
                bajo,
                id,
            });
        }
        Dibujo::Anclado(f) => rc.flotantes.push(f),
        Dibujo::Regla { alto, color } => {
            let ancho = (rc.ancho - 1.0).max(1.0);
            let o = Objeto {
                ancho,
                alto,
                bajo: 0.0,
                elementos: vec![Elemento::Rect {
                    rect: Rect::nuevo(0.0, 0.0, ancho, alto),
                    relleno: Some(Relleno::Solido(color)),
                    borde: None,
                }],
            };
            let id = rc.objetos.len() as u32;
            rc.objetos.push(o);
            rc.piezas().push(Pieza::Objeto {
                ancho,
                alto,
                bajo: 0.0,
                id,
            });
        }
    }
}

/// Un parrafo a franjas (una por renglon).
pub fn parrafo(
    ctx: &mut Ctx,
    n: &Nodo,
    ancho: f32,
    previo: Option<&Nodo>,
    siguiente: Option<&Nodo>,
    salida: &mut Vec<Franja>,
) {
    let Resuelto { p, r: base, estilo } = resolver_parrafo(ctx, n);
    let mut marca = base.clone();
    if let Some(m) = &p.marca {
        marca.mezclar(m);
    }
    let tema = ctx.tema;
    let mut rc = Recogida {
        m: Montador {
            tema,
            medidor: ctx.medidor,
            alternas: ctx.alternas,
            estilos: Vec::new(),
        },
        tramos: Vec::new(),
        objetos: Vec::new(),
        flotantes: Vec::new(),
        campos: Vec::new(),
        topes_extra: Vec::new(),
        notas: Vec::new(),
        ancho,
        cierra_seccion: n.hijo("pPr").and_then(|p| p.hijo("sectPr")).is_some(),
    };
    let estilo_marca = rc.m.estilo(&marca, false, None, None, 1.0);

    // El numero o la vineta de la lista.
    let num_id = p.num_id.unwrap_or(0);
    if num_id > 0
        && let Some((texto, nivel)) = ctx.numeracion.siguiente(num_id, p.nivel.unwrap_or(0))
    {
        let mut rn = marca.clone();
        rn.subrayado = None;
        rn.mezclar(&nivel.rpr);
        if !texto.is_empty() || nivel.formato == "bullet" {
            let texto = if nivel.formato == "bullet" {
                nivel.texto.clone()
            } else {
                texto
            };
            rc.texto(&texto, &rn, None);
            match nivel.sufijo.as_str() {
                "space" => rc.texto(" ", &rn, None),
                "nothing" => {}
                _ => {
                    let e = rc.m.estilo(&rn, false, None, None, 1.0);
                    rc.piezas().push(Pieza::Tab { estilo: e });
                }
            }
        }
    }
    recorrer(ctx, &mut rc, n, &base, None);

    // La geometria.
    let izq = p.izq.unwrap_or(0.0);
    let der = p.der.unwrap_or(0.0);
    let primera = p.primera.unwrap_or(0.0);
    let mut topes: Vec<Tope> = p
        .topes
        .iter()
        .filter(|t| !t.quitar)
        .filter_map(|t| {
            t.tipo.map(|tipo| Tope {
                pos: t.pos,
                tipo,
                relleno: t.relleno,
            })
        })
        .collect();
    if primera < 0.0
        && !topes
            .iter()
            .any(|t| t.pos > izq + primera + 0.01 && t.pos < izq - 0.01)
    {
        // La sangria francesa hace de tope (lo que separa el numero del
        // texto en las listas).
        topes.push(Tope {
            pos: izq,
            tipo: TipoTope::Izquierda,
            relleno: None,
        });
    }
    for t in &rc.topes_extra {
        let pos = match t.tipo {
            TipoTope::Centro => (ancho - der + izq) / 2.0,
            TipoTope::Derecha => ancho - der,
            _ => izq,
        };
        topes.push(Tope { pos, ..*t });
    }
    topes.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    let cfg = Config {
        ancho,
        sangria_izq: izq,
        sangria_der: der,
        primera,
        topes,
        tope_defecto: ctx.tope_defecto,
        alineacion: p.alineacion.unwrap_or_default(),
        partir: true,
        // Word 2013 y posteriores encogen los espacios al justificar.
        encoger: if p.alineacion == Some(Alineacion::Justificada) && ctx.compat >= 15 {
            0.2
        } else {
            0.0
        },
    };

    let antes = if p.antes_auto == Some(true) {
        14.0
    } else {
        p.antes.unwrap_or(0.0)
    };
    let despues = if p.despues_auto == Some(true) {
        14.0
    } else {
        p.despues.unwrap_or(0.0)
    };
    let interlineado = p.interlineado.unwrap_or(Interlineado::Auto(1.0));

    // Bordes de parrafo; los de un grupo de parrafos con los mismos bordes
    // se juntan (sin el de en medio).
    let mismo = |otro: Option<&Nodo>| -> bool {
        let Some(o) = otro.filter(|o| o.nombre == "p") else {
            return false;
        };
        let r = resolver_parrafo(ctx, o);
        r.p.bordes == p.bordes && r.p.izq == p.izq && r.p.der == p.der
    };
    let hay_bordes = p.bordes != Bordes::default();
    let (junto_antes, junto_despues) = if hay_bordes {
        (mismo(previo), mismo(siguiente))
    } else {
        (false, false)
    };
    let borde_arriba = if junto_antes {
        p.bordes.entre.flatten()
    } else {
        p.bordes.arriba.flatten()
    };
    let borde_abajo = if junto_despues {
        None
    } else {
        p.bordes.abajo.flatten()
    };

    let n_tramos = rc.tramos.len().max(1);
    if rc.tramos.is_empty() {
        rc.tramos.push((Salto::Ninguno, Vec::new()));
    }
    let tramos = std::mem::take(&mut rc.tramos);
    let estilos: Arc<[Estilo]> = Arc::from(std::mem::take(&mut rc.m.estilos));
    let objetos: Arc<[Objeto]> = Arc::from(std::mem::take(&mut rc.objetos));
    let ultimo_tramo = n_tramos - 1;
    let geometria = Geometria {
        interlineado,
        izq,
        der,
        primera,
        ancho,
        sombreado: p.sombreado.flatten(),
        hay_bordes,
        borde_arriba,
        borde_abajo,
        borde_izq: p.bordes.izq.flatten(),
        borde_der: p.bordes.der.flatten(),
        estilo: estilo.clone(),
        contextual: p.contextual.unwrap_or(false),
        antes,
        despues,
        auto_antes: p.antes_auto == Some(true),
        auto_despues: p.despues_auto == Some(true),
        salto_antes: p.salto_antes == Some(true),
        salto_tramo: Salto::Ninguno,
        primer_tramo: true,
        ultimo_tramo: false,
        lineas_juntas: p.lineas_juntas == Some(true),
        viudas: p.viudas.unwrap_or(true),
        con_siguiente: p.con_siguiente == Some(true),
        notas: Vec::new(),
    };
    for (k, (salto, piezas)) in tramos.into_iter().enumerate() {
        // Un salto de pagina al final del parrafo no deja renglon vacio.
        if k > 0 && k == ultimo_tramo && piezas.is_empty() {
            salida.push(Franja {
                salto,
                salto_manual: true,
                ..Default::default()
            });
            continue;
        }
        let cfg_k = if k > 0 {
            Config {
                primera: 0.0,
                ..cfg.clone()
            }
        } else {
            cfg.clone()
        };
        let mut unidades = renglones::medir(&piezas, &estilos, estilo_marca, ctx.medidor);
        let renglones = renglones::partir_unidades(
            &mut unidades,
            0,
            true,
            &estilos,
            estilo_marca,
            &cfg_k,
            ctx.medidor,
        );
        let total = renglones.len();
        let rh = Arc::new(Rehacer {
            unidades,
            estilos: estilos.clone(),
            objetos: objetos.clone(),
            cfg: cfg_k,
            marca: estilo_marca,
            g: Geometria {
                salto_tramo: salto,
                primer_tramo: k == 0,
                ultimo_tramo: k == ultimo_tramo,
                notas: if k == ultimo_tramo {
                    rc.notas.clone()
                } else {
                    Vec::new()
                },
                ..geometria.clone()
            },
        });
        let desde = salida.len();
        for (i, (r, unidad)) in renglones.iter().enumerate() {
            let mut f = franja_de_renglon(&rh, r, i, total, *unidad, Vec::new(), ctx.medidor);
            if i == 0 && k == 0 {
                f.flotantes = std::mem::take(&mut rc.flotantes);
            }
            salida.push(f);
        }
        // Lineas viudas y huerfanas, y lo que tiene que ir junto.
        marcar_juntas(&rh.g, &mut salida[desde..], 0, total);
    }
    if !rc.notas.is_empty()
        && let Some(f) = salida.last_mut()
    {
        f.notas = rc.notas.clone();
    }
    ctx.notas_citadas.extend(rc.notas);
    if p.con_siguiente == Some(true)
        && let Some(f) = salida.last_mut()
    {
        f.pegada = true;
    }
}

/// La franja de un renglon de un tramo de parrafo (el `linea` de `total`).
fn franja_de_renglon(
    rh: &Arc<Rehacer>,
    r: &renglones::Renglon,
    linea: usize,
    total: usize,
    unidad: usize,
    limites: Vec<(f32, f32)>,
    medidor: &dyn Medidor,
) -> Franja {
    let g = &rh.g;
    let espacio_borde = |t: Option<Trazo>| t.map(|t| t.grosor + 1.0).unwrap_or(0.0);
    let natural = r.alto_natural;
    let natural_texto = r.alto_texto;
    let (alto, base_y) = match g.interlineado {
        Interlineado::Auto(m) => {
            let h = (natural + natural_texto * (m - 1.0)).max(natural * 0.5);
            // Word deja el espacio de mas DEBAJO del texto: la linea base
            // queda donde estaria con interlineado sencillo.
            (h, natural - r.descenso)
        }
        Interlineado::Exacto(v) => (v, v - r.descenso.min(v * 0.2)),
        Interlineado::AlMenos(v) => {
            let h = natural.max(v);
            (h, h - r.descenso)
        }
    };
    let es_primero = linea == 0 && g.primer_tramo;
    let es_ultimo = linea + 1 == total && g.ultimo_tramo;
    let arriba = if es_primero {
        espacio_borde(g.borde_arriba)
    } else {
        0.0
    };
    let abajo = if es_ultimo {
        espacio_borde(g.borde_abajo)
    } else {
        0.0
    };
    let (izq, der, ancho) = (g.izq, g.der, g.ancho);
    let mut els = Vec::new();
    // Sombreado y bordes del parrafo.
    let caja_p = Rect::nuevo(
        izq.min(izq + if es_primero { g.primera } else { 0.0 }),
        0.0,
        ancho - der - izq,
        alto + arriba + abajo,
    );
    if let Some(c) = g.sombreado {
        els.push(Elemento::Rect {
            rect: Rect { x: izq, ..caja_p },
            relleno: Some(Relleno::Solido(c)),
            borde: None,
        });
    }
    if g.hay_bordes {
        let (x0, x1) = (izq - 4.0, ancho - der + 4.0);
        if es_primero && let Some(t) = g.borde_arriba {
            els.push(Elemento::Linea {
                desde: (x0, t.grosor / 2.0),
                hasta: (x1, t.grosor / 2.0),
                trazo: t,
            });
        }
        if es_ultimo && let Some(t) = g.borde_abajo {
            let y = alto + arriba + abajo - t.grosor / 2.0;
            els.push(Elemento::Linea {
                desde: (x0, y),
                hasta: (x1, y),
                trazo: t,
            });
        }
        if let Some(t) = g.borde_izq {
            els.push(Elemento::Linea {
                desde: (x0, 0.0),
                hasta: (x0, alto + arriba + abajo),
                trazo: t,
            });
        }
        if let Some(t) = g.borde_der {
            els.push(Elemento::Linea {
                desde: (x1, 0.0),
                hasta: (x1, alto + arriba + abajo),
                trazo: t,
            });
        }
    }
    renglon_a_elementos(
        r,
        &rh.estilos,
        &rh.objetos,
        base_y + arriba,
        medidor,
        &mut els,
    );
    let mut f = Franja {
        holgura: if let Interlineado::Auto(m) = g.interlineado {
            (natural_texto * (m - 1.0)).max(0.0)
        } else {
            0.0
        },
        alto: alto + arriba + abajo,
        elementos: els,
        estilo: g.estilo.clone(),
        contextual: g.contextual,
        rehacer: Some(rh.clone()),
        linea,
        unidad,
        limites,
        ..Default::default()
    };
    if es_primero {
        f.antes = g.antes;
        f.auto_antes = g.auto_antes;
        if g.salto_antes {
            f.salto = Salto::Pagina;
        }
    }
    if linea == 0 && !g.primer_tramo {
        f.salto = g.salto_tramo;
        f.salto_manual = true;
    }
    if es_ultimo {
        f.despues = g.despues;
        f.auto_despues = g.auto_despues;
    }
    f
}

/// Lineas viudas y huerfanas, y «conservar lineas juntas»: que renglones
/// tienen que ir con el siguiente. `franjas` son los renglones `desde..` de
/// un tramo de `total`.
fn marcar_juntas(g: &Geometria, franjas: &mut [Franja], desde: usize, total: usize) {
    for (k, f) in franjas.iter_mut().enumerate() {
        let i = desde + k;
        if g.lineas_juntas {
            f.pegada_lineas = i + 1 < total;
            f.pegada = g.viudas && total >= 2 && i == 0;
        } else {
            f.pegada = g.viudas && total >= 2 && (i == 0 || i + 2 == total);
        }
    }
}

/// Vuelve a partir un tramo de parrafo desde su renglon `linea` (que
/// empieza en la unidad `unidad`): ese renglon en los `tramos` dados (x de
/// la caja, de izquierda a derecha; `None` = a lo ancho) y los que le
/// siguen a lo ancho de la caja. Las unidades ya estan medidas: no se mide
/// nada otra vez.
pub fn rehacer(
    rh: &Arc<Rehacer>,
    linea: usize,
    unidad: usize,
    tramos: Option<&[(f32, f32)]>,
    medidor: &dyn Medidor,
) -> Vec<Franja> {
    let mut u = rh.unidades.clone();
    // Cada renglon rehecho, la unidad donde empieza y sus tramos.
    type Rehecho = (renglones::Renglon, usize, Vec<(f32, f32)>);
    let mut v: Vec<Rehecho> = Vec::new();
    let resto = match tramos {
        Some(t) => {
            let (r, fin, forzado) = renglones::renglon_en_tramos(
                &mut u,
                unidad,
                t,
                &rh.estilos,
                rh.marca,
                &rh.cfg,
                medidor,
            );
            v.push((r, unidad, t.to_vec()));
            (fin < u.len() || forzado).then_some(fin)
        }
        None => Some(unidad),
    };
    if let Some(desde) = resto {
        let primero = tramos.is_none() && linea == 0;
        for (r, i) in renglones::partir_unidades(
            &mut u,
            desde,
            primero,
            &rh.estilos,
            rh.marca,
            &rh.cfg,
            medidor,
        ) {
            v.push((r, i, Vec::new()));
        }
    }
    let total = linea + v.len();
    let mut franjas: Vec<Franja> = v
        .into_iter()
        .enumerate()
        .map(|(k, (r, i, lim))| franja_de_renglon(rh, &r, linea + k, total, i, lim, medidor))
        .collect();
    marcar_juntas(&rh.g, &mut franjas, linea, total);
    if rh.g.ultimo_tramo
        && let Some(f) = franjas.last_mut()
    {
        f.notas = rh.g.notas.clone();
        f.pegada |= rh.g.con_siguiente;
    }
    franjas
}

/// Pinta un renglon con la linea base en `base_y` (desde arriba de la
/// franja).
pub fn renglon_a_elementos(
    r: &renglones::Renglon,
    estilos: &[Estilo],
    objetos: &[Objeto],
    base_y: f32,
    medidor: &dyn Medidor,
    salida: &mut Vec<Elemento>,
) {
    for t in &r.trozos {
        match &t.contenido {
            Contenido::Texto {
                texto,
                estilo,
                extra_espacio,
            } => {
                let Some(e) = estilos.get(*estilo as usize) else {
                    continue;
                };
                if let Some(f) = e.fondo {
                    let m = medidor.metricas(&e.letra);
                    salida.push(Elemento::Rect {
                        rect: Rect::nuevo(
                            t.x,
                            base_y - e.subida - m.ascenso,
                            t.ancho,
                            m.ascenso + m.descenso,
                        ),
                        relleno: Some(Relleno::Solido(f)),
                        borde: None,
                    });
                }
                if texto.trim().is_empty() && e.subrayado.is_none() {
                    continue;
                }
                let el = Elemento::Texto(Texto {
                    x: t.x,
                    y: base_y - e.subida,
                    letra: e.letra.clone(),
                    color: e.color,
                    texto: texto.clone(),
                    subrayado: e.subrayado,
                    tachado: e.tachado,
                    espaciado: e.espaciado,
                    extra_espacio: *extra_espacio,
                    ancho: t.ancho,
                    campo: e.campo,
                    enlace: e.enlace.clone(),
                });
                salida.push(match &e.efectos {
                    Some(ef) => el.con_efectos(ef.to_vec()),
                    None => el,
                });
            }
            Contenido::Objeto { id, alto } => {
                if let Some(o) = objetos.get(*id as usize) {
                    for mut el in o.elementos.iter().cloned() {
                        el.trasladar(t.x, base_y - alto);
                        salida.push(el);
                    }
                }
            }
            Contenido::Relleno { caracter, estilo } => {
                let Some(e) = estilos.get(*estilo as usize) else {
                    continue;
                };
                let w = medidor.ancho(&caracter.to_string(), &e.letra).max(0.5);
                // Los puntos alineados a una rejilla, como Word.
                let n = ((t.ancho - w * 0.5) / w).floor().max(0.0) as usize;
                if n == 0 {
                    continue;
                }
                let texto: String = std::iter::repeat_n(*caracter, n).collect();
                let x = t.x + t.ancho - n as f32 * w - w * 0.25;
                salida.push(Elemento::Texto(Texto {
                    x,
                    y: base_y - e.subida,
                    letra: e.letra.clone(),
                    color: e.color,
                    texto,
                    subrayado: None,
                    tachado: false,
                    espaciado: 0.0,
                    extra_espacio: 0.0,
                    ancho: n as f32 * w,
                    campo: None,
                    enlace: None,
                }));
            }
        }
    }
}

// ---------------------------------------------------------------------
// Tablas
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fusion {
    Nada,
    /// Empieza y abarca `n` filas.
    Inicio(usize),
    Sigue,
}

/// Una celda ya maquetada.
#[derive(Debug, Clone)]
pub struct CeldaMaq {
    x: f32,
    ancho: f32,
    contenido: Vec<Elemento>,
    cortes: Vec<f32>,
    alto_contenido: f32,
    m_arriba: f32,
    m_abajo: f32,
    m_izq: f32,
    fondo: Option<Color>,
    /// Arriba, derecha, abajo, izquierda.
    bordes: [Option<Trazo>; 4],
    vertical: u8,
    fusion: Fusion,
    /// Alto total de la fusion (para centrar en vertical).
    alto_fusion: f32,
}

/// Una fila de tabla maquetada, que se pinta al colocarla.
#[derive(Debug, Clone)]
pub struct FilaMaq {
    celdas: Vec<CeldaMaq>,
    pub no_partir: bool,
    pub exacta: bool,
}

impl FilaMaq {
    /// Los elementos de la fila con alto `alto`.
    pub fn elementos(&self, alto: f32) -> Vec<Elemento> {
        let mut v = Vec::new();
        for c in &self.celdas {
            if let Some(f) = c.fondo {
                v.push(Elemento::Rect {
                    rect: Rect::nuevo(c.x, 0.0, c.ancho, alto),
                    relleno: Some(Relleno::Solido(f)),
                    borde: None,
                });
            }
        }
        for c in &self.celdas {
            if c.fusion != Fusion::Sigue {
                let alto_util = if let Fusion::Inicio(_) = c.fusion {
                    c.alto_fusion.max(alto)
                } else {
                    alto
                };
                let libre = (alto_util - c.m_arriba - c.m_abajo - c.alto_contenido).max(0.0);
                let dy = match c.vertical {
                    1 => libre / 2.0,
                    2 => libre,
                    _ => 0.0,
                };
                for e in &c.contenido {
                    let mut e = e.clone();
                    e.trasladar(c.x + c.m_izq, c.m_arriba + dy);
                    v.push(e);
                }
            }
        }
        for c in &self.celdas {
            let (x0, x1) = (c.x, c.x + c.ancho);
            let lado = |t: &Option<Trazo>, a: (f32, f32), b: (f32, f32), v: &mut Vec<Elemento>| {
                if let Some(t) = t {
                    v.push(Elemento::Linea {
                        desde: a,
                        hasta: b,
                        trazo: *t,
                    });
                }
            };
            if c.fusion != Fusion::Sigue {
                lado(&c.bordes[0], (x0, 0.0), (x1, 0.0), &mut v);
            }
            lado(&c.bordes[1], (x1, 0.0), (x1, alto), &mut v);
            if !matches!(c.fusion, Fusion::Inicio(n) if n > 1) {
                lado(&c.bordes[2], (x0, alto), (x1, alto), &mut v);
            }
            lado(&c.bordes[3], (x0, 0.0), (x0, alto), &mut v);
        }
        v
    }

    /// Parte la fila para que lo primero quepa en `alto_max`. `None` si no
    /// cabe ni un renglon.
    pub fn partir(&self, alto_max: f32) -> Option<(FilaMaq, f32, FilaMaq, f32)> {
        if self.no_partir {
            return None;
        }
        let mut a = self.clone();
        let mut b = self.clone();
        let mut alto_b: f32 = 0.0;
        let mut algo = false;
        for (k, c) in self.celdas.iter().enumerate() {
            if c.fusion == Fusion::Sigue {
                b.celdas[k].contenido.clear();
                continue;
            }
            let limite = alto_max - c.m_arriba - c.m_abajo;
            if c.alto_contenido <= limite {
                // Cabe entera en la primera parte.
                b.celdas[k].contenido.clear();
                b.celdas[k].cortes.clear();
                b.celdas[k].alto_contenido = 0.0;
                if !c.contenido.is_empty() {
                    algo = true;
                }
                continue;
            }
            let corte = c
                .cortes
                .iter()
                .copied()
                .filter(|&y| y <= limite + 0.01)
                .fold(0.0f32, f32::max);
            if corte > 0.0 {
                algo = true;
            }
            a.celdas[k].contenido = c
                .contenido
                .iter()
                .filter(|e| e.y_referencia() < corte)
                .cloned()
                .collect();
            a.celdas[k].alto_contenido = corte;
            a.celdas[k].vertical = 0;
            b.celdas[k].contenido = c
                .contenido
                .iter()
                .filter(|e| e.y_referencia() >= corte)
                .cloned()
                .map(|mut e| {
                    e.trasladar(0.0, -corte);
                    e
                })
                .collect();
            b.celdas[k].cortes = c
                .cortes
                .iter()
                .filter(|&&y| y > corte)
                .map(|y| y - corte)
                .collect();
            b.celdas[k].alto_contenido = c.alto_contenido - corte;
            b.celdas[k].vertical = 0;
            alto_b = alto_b.max(b.celdas[k].alto_contenido + c.m_arriba + c.m_abajo);
        }
        if !algo {
            return None;
        }
        for c in &mut a.celdas {
            if let Fusion::Inicio(_) = c.fusion {
                c.alto_fusion = 0.0;
            }
        }
        Some((a, alto_max, b, alto_b))
    }
}

/// Lee una tabla y la deja en franjas (una por fila).
#[allow(clippy::needless_range_loop)] // las fusiones miran varias filas a la vez
pub fn tabla(ctx: &mut Ctx, t: &Nodo, ancho_disp: f32, salida: &mut Vec<Franja>) {
    let directo = t
        .hijo("tblPr")
        .map(|p| PropTabla::leer(p, ctx.tema))
        .unwrap_or_default();
    let id_estilo = directo
        .estilo
        .clone()
        .or_else(|| ctx.estilos.defecto_tabla.clone());
    let (mut tp, tc_estilo, pp_estilo, rp_estilo, cond) = id_estilo
        .as_deref()
        .map(|e| ctx.estilos.de_tabla(e))
        .unwrap_or_default();
    tp.mezclar(&directo);

    // La rejilla.
    let mut rejilla: Vec<f32> = t
        .hijo("tblGrid")
        .map(|g| {
            g.elementos()
                .filter(|c| c.nombre == "gridCol")
                .map(|c| attr_num(c, "w", 20.0).map(twips).unwrap_or(0.0))
                .collect()
        })
        .unwrap_or_default();
    let filas: Vec<&Nodo> = filas_de(t);
    let max_celdas = filas
        .iter()
        .map(|f| {
            celdas_de(f).iter().map(|c| tramo_de(c)).sum::<usize>()
                + rejilla_antes(f)
                + rejilla_despues(f)
        })
        .max()
        .unwrap_or(0);
    if rejilla.len() < max_celdas {
        let falta = max_celdas - rejilla.len();
        let usado: f32 = rejilla.iter().sum();
        let resto = ((ancho_disp - usado) / falta as f32).max(10.0);
        rejilla.extend(std::iter::repeat_n(resto, falta));
    }
    if rejilla.iter().all(|w| *w <= 0.0) && !rejilla.is_empty() {
        let n = rejilla.len() as f32;
        rejilla.iter_mut().for_each(|w| *w = ancho_disp / n);
    }
    // El ancho pedido en porcentaje estira la rejilla.
    let suma: f32 = rejilla.iter().sum();
    let objetivo = match tp.ancho {
        Some(AnchoW::Porcentaje(p)) => Some(p * ancho_disp),
        Some(AnchoW::Puntos(w))
            if w > 0.0 && suma > 0.0 && (w - suma).abs() > 2.0 && suma < w * 0.5 =>
        {
            Some(w)
        }
        _ => None,
    };
    if let Some(obj) = objetivo
        && suma > 0.0
    {
        let k = obj / suma;
        rejilla.iter_mut().for_each(|w| *w *= k);
    }
    let ancho_tabla: f32 = rejilla.iter().sum();
    let mut xs = vec![0.0f32];
    for w in &rejilla {
        xs.push(xs.last().copied().unwrap_or(0.0) + w);
    }

    // Margenes de celda por defecto: 5,4 pt a los lados.
    let mut m_tabla = Margenes {
        arriba: Some(0.0),
        abajo: Some(0.0),
        izq: Some(5.4),
        der: Some(5.4),
    };
    m_tabla.mezclar(&tp.margenes);

    let mut x0 = tp.sangria.unwrap_or(0.0);
    match tp.alineacion {
        Some(Alineacion::Centro) => x0 = (ancho_disp - ancho_tabla) / 2.0,
        Some(Alineacion::Derecha) => x0 = ancho_disp - ancho_tabla,
        _ => {
            // Antes de Word 2013 la sangria mide hasta el texto de la primera
            // celda: el borde sale hacia fuera.
            if ctx.compat < 15 {
                x0 -= m_tabla.izq.unwrap_or(5.4);
            }
        }
    }

    let n_filas = filas.len();
    let look = |o: Option<bool>, def: bool| o.unwrap_or(def);
    let primera_fila = look(tp.look_primera_fila, true);
    let ultima_fila = look(tp.look_ultima_fila, false);
    let primera_col = look(tp.look_primera_col, true);
    let ultima_col = look(tp.look_ultima_col, false);
    let bandas_h = !look(tp.look_sin_bandas_h, false);
    let bandas_v = !look(tp.look_sin_bandas_v, true);

    // Primero, la estructura: que celda empieza donde y cuantas filas abarca.
    struct CeldaLeida<'n> {
        nodo: &'n Nodo,
        col: usize,
        tramo: usize,
        tcpr: PropCelda,
    }
    let mut estructura: Vec<Vec<CeldaLeida>> = Vec::new();
    for f in &filas {
        let mut col = rejilla_antes(f);
        let mut v = Vec::new();
        for c in celdas_de(f) {
            let tcpr = c
                .hijo("tcPr")
                .map(|p| PropCelda::leer(p, ctx.tema))
                .unwrap_or_default();
            let tramo = tcpr.tramo.unwrap_or(1).max(1) as usize;
            v.push(CeldaLeida {
                nodo: c,
                col,
                tramo,
                tcpr,
            });
            col += tramo;
        }
        estructura.push(v);
    }
    // Cuantas filas abarca cada inicio de fusion vertical.
    let abarca = |fi: usize, col: usize| -> usize {
        let mut n = 1;
        for fila in estructura.iter().skip(fi + 1) {
            match fila.iter().find(|c| c.col == col) {
                Some(c) if c.tcpr.fusion_v == Some(false) => n += 1,
                _ => break,
            }
        }
        n
    };

    let mut filas_maq: Vec<(FilaMaq, f32, bool, f32, bool)> = Vec::new(); // fila, alto, cabecera, alto minimo, exacta
    let mut cabecera_continua = true;
    for (fi, fila_nodo) in filas.iter().enumerate() {
        let trpr = fila_nodo.hijo("trPr");
        let es_cabecera = trpr
            .and_then(|p| p.hijo("tblHeader"))
            .is_some_and(crate::paquete::interruptor)
            && cabecera_continua;
        if !es_cabecera {
            cabecera_continua = false;
        }
        let no_partir = trpr
            .and_then(|p| p.hijo("cantSplit"))
            .is_some_and(crate::paquete::interruptor);
        let (alto_fila, regla) = trpr
            .and_then(|p| p.hijo("trHeight"))
            .map(|h| {
                (
                    attr_num(h, "val", 20.0).map(twips).unwrap_or(0.0),
                    h.atributo("hRule").to_string(),
                )
            })
            .unwrap_or((0.0, String::new()));
        let exacta = regla == "exact";
        let mut celdas = Vec::new();
        let n_celdas = estructura[fi].len();
        let ultima_col_idx = rejilla.len();
        for (ci, cl) in estructura[fi].iter().enumerate() {
            let col = cl.col.min(rejilla.len());
            let fin = (cl.col + cl.tramo).min(rejilla.len());
            let x = x0 + xs[col];
            let ancho = (xs[fin] - xs[col]).max(1.0);
            // Las partes condicionales que tocan a esta celda.
            let mut tipos: Vec<&str> = Vec::new();
            let fila_dato = if primera_fila { fi.wrapping_sub(1) } else { fi };
            let col_dato = if primera_col {
                cl.col.wrapping_sub(1)
            } else {
                cl.col
            };
            if bandas_v && !(primera_col && cl.col == 0) {
                tipos.push(if col_dato % 2 == 0 {
                    "band1Vert"
                } else {
                    "band2Vert"
                });
            }
            if bandas_h && !(primera_fila && fi == 0) {
                tipos.push(if fila_dato % 2 == 0 {
                    "band1Horz"
                } else {
                    "band2Horz"
                });
            }
            if primera_col && cl.col == 0 {
                tipos.push("firstCol");
            }
            if ultima_col && fin == ultima_col_idx {
                tipos.push("lastCol");
            }
            if primera_fila && (fi == 0 || es_cabecera) {
                tipos.push("firstRow");
            }
            if ultima_fila && fi == n_filas - 1 {
                tipos.push("lastRow");
            }
            if primera_fila && fi == 0 && primera_col && cl.col == 0 {
                tipos.push("nwCell");
            }
            if primera_fila && fi == 0 && ultima_col && fin == ultima_col_idx {
                tipos.push("neCell");
            }
            if ultima_fila && fi == n_filas - 1 && primera_col && cl.col == 0 {
                tipos.push("swCell");
            }
            if ultima_fila && fi == n_filas - 1 && ultima_col && fin == ultima_col_idx {
                tipos.push("seCell");
            }
            let mut tcpr = tc_estilo.clone();
            let mut pp = pp_estilo.clone();
            let mut rp = rp_estilo.clone();
            let mut bordes_t = tp.bordes.clone();
            if let Some(c) = cond.get("wholeTable") {
                tcpr.mezclar(&c.tcpr);
                pp.mezclar(&c.ppr);
                rp.mezclar(&c.rpr);
                bordes_t.mezclar(&c.tblpr.bordes);
            }
            for tipo in &tipos {
                if let Some(c) = cond.get(*tipo) {
                    tcpr.mezclar(&c.tcpr);
                    pp.mezclar(&c.ppr);
                    rp.mezclar(&c.rpr);
                }
            }
            tcpr.mezclar(&cl.tcpr);
            // Los bordes: los de la celda, y si no, los de la tabla segun
            // donde caiga.
            let borde_t = |b: &Option<Option<Trazo>>,
                           interior: &Option<Option<Trazo>>,
                           exterior: bool|
             -> Option<Trazo> {
                if exterior {
                    b.flatten()
                } else {
                    interior.flatten()
                }
            };
            let primera_f = fi == 0;
            let fusion = match cl.tcpr.fusion_v {
                Some(true) => Fusion::Inicio(abarca(fi, cl.col)),
                Some(false) => Fusion::Sigue,
                None => Fusion::Nada,
            };
            let ultima_f = match fusion {
                Fusion::Inicio(n) => fi + n >= n_filas,
                _ => fi == n_filas - 1,
            };
            let mut bordes = [
                borde_t(&bordes_t.arriba, &bordes_t.dentro_h, primera_f),
                borde_t(&bordes_t.der, &bordes_t.dentro_v, fin >= ultima_col_idx),
                borde_t(&bordes_t.abajo, &bordes_t.dentro_h, ultima_f),
                borde_t(&bordes_t.izq, &bordes_t.dentro_v, cl.col == 0),
            ];
            let b = &tcpr.bordes;
            for (k, propio) in [&b.arriba, &b.der, &b.abajo, &b.izq]
                .into_iter()
                .enumerate()
            {
                if let Some(p) = propio {
                    bordes[k] = *p;
                }
            }
            let mut m = m_tabla;
            m.mezclar(&tcpr.margenes);
            let (mi, md, ma, mb) = (
                m.izq.unwrap_or(5.4),
                m.der.unwrap_or(5.4),
                m.arriba.unwrap_or(0.0),
                m.abajo.unwrap_or(0.0),
            );
            // El contenido.
            let previa = ctx.base_tabla.replace((pp, rp));
            let ancho_util = (ancho - mi - md).max(1.0);
            let contenido = if fusion == Fusion::Sigue {
                Caja::default()
            } else {
                caja_con_flotantes(ctx, cl.nodo.elementos(), ancho_util)
            };
            ctx.base_tabla = previa;
            let fondo = tcpr.sombreado.flatten().or(tp.sombreado.flatten());
            let _ = (ci, n_celdas);
            celdas.push(CeldaMaq {
                x,
                ancho,
                contenido: contenido.elementos,
                cortes: contenido.cortes,
                alto_contenido: contenido.alto,
                m_arriba: ma,
                m_abajo: mb,
                m_izq: mi,
                fondo,
                bordes,
                vertical: tcpr.vertical.unwrap_or(0),
                fusion,
                alto_fusion: 0.0,
            });
        }
        // Word suma el grueso del borde de arriba de la fila (y el de abajo en
        // la ultima) al alto de la fila: el texto empieza debajo del borde.
        let b_arriba = celdas
            .iter()
            .filter_map(|c| c.bordes[0].map(|t| t.grosor))
            .fold(0.0f32, f32::max);
        let b_abajo = if fi + 1 == n_filas {
            celdas
                .iter()
                .filter_map(|c| c.bordes[2].map(|t| t.grosor))
                .fold(0.0f32, f32::max)
        } else {
            0.0
        };
        for c in &mut celdas {
            c.m_arriba += b_arriba;
            c.m_abajo += b_abajo;
        }
        let alto_contenido = celdas
            .iter()
            .filter(|c| c.fusion == Fusion::Nada || c.fusion == Fusion::Inicio(1))
            .map(|c| c.alto_contenido + c.m_arriba + c.m_abajo)
            .fold(0.0f32, f32::max);
        let alto = if exacta {
            alto_fila
        } else {
            alto_contenido.max(alto_fila)
        };
        filas_maq.push((
            FilaMaq {
                celdas,
                no_partir: no_partir || exacta,
                exacta,
            },
            alto.max(1.0),
            es_cabecera,
            alto_fila,
            exacta,
        ));
    }

    // Las fusiones verticales: el inicio sabe el alto total; si su
    // contenido no cabe, crece la ultima fila de la fusion.
    for fi in 0..filas_maq.len() {
        for ci in 0..filas_maq[fi].0.celdas.len() {
            if let Fusion::Inicio(n) = filas_maq[fi].0.celdas[ci].fusion {
                let n = n.max(1).min(filas_maq.len() - fi);
                let c = &filas_maq[fi].0.celdas[ci];
                let necesita = c.alto_contenido + c.m_arriba + c.m_abajo;
                let tiene: f32 = (fi..fi + n).map(|k| filas_maq[k].1).sum();
                if necesita > tiene {
                    filas_maq[fi + n - 1].1 += necesita - tiene;
                }
                let total: f32 = (fi..fi + n).map(|k| filas_maq[k].1).sum();
                filas_maq[fi].0.celdas[ci].alto_fusion = total;
                // Las celdas que siguen la fusion copian fondo y bordes de
                // los lados.
                let (fondo, bordes) = (
                    filas_maq[fi].0.celdas[ci].fondo,
                    filas_maq[fi].0.celdas[ci].bordes,
                );
                let x = filas_maq[fi].0.celdas[ci].x;
                for k in fi + 1..fi + n {
                    if let Some(s) = filas_maq[k]
                        .0
                        .celdas
                        .iter_mut()
                        .find(|s| (s.x - x).abs() < 0.5 && s.fusion == Fusion::Sigue)
                    {
                        if s.fondo.is_none() {
                            s.fondo = fondo;
                        }
                        if k == fi + n - 1 && s.bordes[2].is_none() {
                            s.bordes[2] = bordes[2];
                        }
                    }
                }
            }
        }
    }

    let cabeceras: Vec<Franja> = filas_maq
        .iter()
        .filter(|f| f.2)
        .map(|(fila, alto, _, _, _)| Franja {
            alto: *alto,
            fila: Some(Box::new(fila.clone())),
            es_cabecera: true,
            ..Default::default()
        })
        .collect();
    let cabeceras = (!cabeceras.is_empty()).then(|| Arc::new(cabeceras));
    let n = filas_maq.len();
    let no_partir: Vec<bool> = filas_maq.iter().map(|f| f.0.no_partir).collect();
    for (k, (fila, alto, es_cab, _, _)) in filas_maq.into_iter().enumerate() {
        salida.push(Franja {
            alto,
            fila: Some(Box::new(fila)),
            cabecera: if es_cab { None } else { cabeceras.clone() },
            es_cabecera: es_cab,
            // La cabecera va con la primera fila de datos.
            // La cabecera va con la primera fila de datos (si esa no se puede
            // partir; si se puede, Word la empieza aqui y la parte).
            pegada: es_cab && k + 1 < n && no_partir[k + 1],
            ..Default::default()
        });
    }
}

fn filas_de(t: &Nodo) -> Vec<&Nodo> {
    let mut v = Vec::new();
    for e in t.elementos() {
        match e.nombre.as_str() {
            "tr" => v.push(e),
            "sdt" => {
                if let Some(c) = e.hijo("sdtContent") {
                    v.extend(c.elementos().filter(|x| x.nombre == "tr"));
                }
            }
            "customXml" => v.extend(e.elementos().filter(|x| x.nombre == "tr")),
            _ => {}
        }
    }
    v
}

fn celdas_de(f: &Nodo) -> Vec<&Nodo> {
    let mut v = Vec::new();
    for e in f.elementos() {
        match e.nombre.as_str() {
            "tc" => v.push(e),
            "sdt" => {
                if let Some(c) = e.hijo("sdtContent") {
                    v.extend(c.elementos().filter(|x| x.nombre == "tc"));
                }
            }
            "customXml" => v.extend(e.elementos().filter(|x| x.nombre == "tc")),
            _ => {}
        }
    }
    v
}

fn tramo_de(c: &Nodo) -> usize {
    c.hijo("tcPr")
        .and_then(|p| p.hijo("gridSpan"))
        .and_then(|g| g.valor().parse().ok())
        .unwrap_or(1usize)
        .max(1)
}

fn rejilla_antes(f: &Nodo) -> usize {
    f.hijo("trPr")
        .and_then(|p| p.hijo("gridBefore"))
        .and_then(|g| g.valor().parse().ok())
        .unwrap_or(0)
}

fn rejilla_despues(f: &Nodo) -> usize {
    f.hijo("trPr")
        .and_then(|p| p.hijo("gridAfter"))
        .and_then(|g| g.valor().parse().ok())
        .unwrap_or(0)
}

/// Lo que se ve de un mapa vacio: para los colores de DrawingML en Word.
pub fn mapa_vacio() -> HashMap<String, String> {
    HashMap::new()
}
