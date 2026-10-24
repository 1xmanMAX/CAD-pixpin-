//! **El texto de PowerPoint** (`p:txBody` / `a:txBody`): propiedades de
//! parrafo por nivel (`a:lvl1pPr`...), heredadas del estilo de texto de la
//! presentacion, del patron y de los marcadores; vinetas y numeros; el
//! cuerpo con sus margenes, su anclaje y su autoajuste.

use std::sync::Arc;

use pixpin_docs::xml::Nodo;

use crate::dml::Contexto;
use crate::medidor::Medidor;
use crate::pagina::{Campo, Color, Efecto, Elemento, Letra, Rect, Subrayado};
use crate::paquete::{attr_num, emu};
use crate::renglones::{self, Alineacion, Config, Estilo, Pieza, TipoTope, Tope};
use crate::tema;

/// Propiedades de corrida de DrawingML (`a:rPr`, `a:defRPr`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropR {
    pub tam: Option<f32>,
    pub negrita: Option<bool>,
    pub cursiva: Option<bool>,
    pub subrayado: Option<Option<Subrayado>>,
    pub tachado: Option<bool>,
    /// 1 = todo mayusculas, 2 = versalitas, 0 = nada.
    pub mayusculas: Option<u8>,
    pub color: Option<Color>,
    pub letra: Option<String>,
    pub letra_ea: Option<String>,
    pub espaciado: Option<f32>,
    /// Subida en fraccion del tamano (0.3 = superindice).
    pub subida: Option<f32>,
    pub resaltado: Option<Color>,
    /// Los efectos del texto (`a:effectLst`); vacio = ninguno (y manda
    /// sobre lo heredado).
    pub efectos: Option<Arc<[Efecto]>>,
}

impl PropR {
    pub fn mezclar(&mut self, o: &PropR) {
        macro_rules! m {
            ($($c:ident),*) => { $( if o.$c.is_some() { self.$c = o.$c.clone(); } )* };
        }
        m!(
            tam, negrita, cursiva, subrayado, tachado, mayusculas, color, letra, letra_ea,
            espaciado, subida, resaltado, efectos
        );
    }

    pub fn leer(n: &Nodo, ctx: Contexto) -> PropR {
        let b = |k: &str| match n.atributo(k) {
            "" => None,
            "1" | "true" => Some(true),
            _ => Some(false),
        };
        let mut p = PropR {
            tam: attr_num(n, "sz", 100.0).map(|v| v / 100.0),
            negrita: b("b"),
            cursiva: b("i"),
            espaciado: attr_num(n, "spc", 100.0).map(|v| v / 100.0),
            subida: attr_num(n, "baseline", 1.0).map(|v| v / 100000.0),
            ..Default::default()
        };
        match n.atributo("u") {
            "" => {}
            "none" => p.subrayado = Some(None),
            "dbl" => p.subrayado = Some(Some(Subrayado::Doble)),
            "heavy" => p.subrayado = Some(Some(Subrayado::Grueso)),
            "dotted" | "dottedHeavy" => p.subrayado = Some(Some(Subrayado::Puntos)),
            "dash" | "dashHeavy" | "dashLong" | "dashLongHeavy" => {
                p.subrayado = Some(Some(Subrayado::Rayas))
            }
            "wavy" | "wavyHeavy" | "wavyDbl" => p.subrayado = Some(Some(Subrayado::Ondulado)),
            _ => p.subrayado = Some(Some(Subrayado::Simple)),
        }
        match n.atributo("strike") {
            "" => {}
            "noStrike" => p.tachado = Some(false),
            _ => p.tachado = Some(true),
        }
        match n.atributo("cap") {
            "" => {}
            "all" => p.mayusculas = Some(1),
            "small" => p.mayusculas = Some(2),
            _ => p.mayusculas = Some(0),
        }
        for e in n.elementos() {
            match e.nombre.as_str() {
                "solidFill" | "gradFill" => {
                    if let Some(Some(r)) = crate::dml::relleno_de(e, ctx, None) {
                        p.color = Some(r.color_medio());
                    }
                }
                "latin" => {
                    p.letra = Some(e.atributo("typeface").to_string()).filter(|s| !s.is_empty())
                }
                "ea" => {
                    p.letra_ea = Some(e.atributo("typeface").to_string()).filter(|s| !s.is_empty())
                }
                "highlight" => p.resaltado = tema::color_hijo(e, ctx.tema, ctx.mapa, None),
                "effectLst" => p.efectos = Some(Arc::from(crate::dml::efectos(e, ctx, None, 0.0))),
                _ => {}
            }
        }
        p
    }
}

/// Un espaciado: en porcentaje del renglon o en puntos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Esp {
    Pct(f32),
    Pts(f32),
}

fn esp(n: &Nodo) -> Option<Esp> {
    if let Some(p) = n.hijo("spcPct") {
        return attr_num(p, "val", 1.0).map(|v| Esp::Pct(v / 100000.0));
    }
    if let Some(p) = n.hijo("spcPts") {
        return attr_num(p, "val", 1.0).map(|v| Esp::Pts(v / 100.0));
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub enum Vineta {
    Nada,
    Caracter(char),
    Numero { tipo: String, inicio: u32 },
}

/// Propiedades de parrafo de DrawingML (`a:pPr`, `a:lvlNpPr`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropP {
    pub marl: Option<f32>,
    pub sangria: Option<f32>,
    pub alineacion: Option<Alineacion>,
    pub interlineado: Option<Esp>,
    pub antes: Option<Esp>,
    pub despues: Option<Esp>,
    pub vineta: Option<Vineta>,
    /// `None` = la de la letra del texto.
    pub vineta_letra: Option<Option<String>>,
    pub vineta_color: Option<Option<Color>>,
    /// Fraccion del tamano del texto.
    pub vineta_tam: Option<f32>,
    pub tope_defecto: Option<f32>,
    pub topes: Option<Vec<Tope>>,
    pub def: PropR,
}

impl PropP {
    pub fn mezclar(&mut self, o: &PropP) {
        macro_rules! m {
            ($($c:ident),*) => { $( if o.$c.is_some() { self.$c = o.$c.clone(); } )* };
        }
        m!(
            marl,
            sangria,
            alineacion,
            interlineado,
            antes,
            despues,
            vineta,
            vineta_letra,
            vineta_color,
            vineta_tam,
            tope_defecto,
            topes
        );
        self.def.mezclar(&o.def);
    }

    pub fn leer(n: &Nodo, ctx: Contexto) -> PropP {
        let mut p = PropP {
            marl: attr_num(n, "marL", 1.0).map(emu),
            sangria: attr_num(n, "indent", 1.0).map(emu),
            tope_defecto: attr_num(n, "defTabSz", 1.0).map(emu),
            ..Default::default()
        };
        p.alineacion = match n.atributo("algn") {
            "" => None,
            "ctr" => Some(Alineacion::Centro),
            "r" => Some(Alineacion::Derecha),
            "just" | "justLow" => Some(Alineacion::Justificada),
            "dist" | "thaiDist" => Some(Alineacion::Distribuida),
            _ => Some(Alineacion::Izquierda),
        };
        for e in n.elementos() {
            match e.nombre.as_str() {
                "lnSpc" => p.interlineado = esp(e),
                "spcBef" => p.antes = esp(e),
                "spcAft" => p.despues = esp(e),
                "buNone" => p.vineta = Some(Vineta::Nada),
                "buChar" => p.vineta = e.atributo("char").chars().next().map(Vineta::Caracter),
                "buAutoNum" => {
                    p.vineta = Some(Vineta::Numero {
                        tipo: e.atributo("type").to_string(),
                        inicio: e.atributo("startAt").parse().unwrap_or(1),
                    })
                }
                "buFontTx" => p.vineta_letra = Some(None),
                "buFont" => p.vineta_letra = Some(Some(e.atributo("typeface").to_string())),
                "buClrTx" => p.vineta_color = Some(None),
                "buClr" => p.vineta_color = Some(tema::color_hijo(e, ctx.tema, ctx.mapa, None)),
                "buSzPct" => p.vineta_tam = attr_num(e, "val", 1.0).map(|v| v / 100000.0),
                "buSzTx" => p.vineta_tam = Some(1.0),
                "tabLst" => {
                    p.topes = Some(
                        e.elementos()
                            .filter(|t| t.nombre == "tab")
                            .map(|t| Tope {
                                pos: attr_num(t, "pos", 1.0).map(emu).unwrap_or(0.0),
                                tipo: match t.atributo("algn") {
                                    "ctr" => TipoTope::Centro,
                                    "r" => TipoTope::Derecha,
                                    "dec" => TipoTope::Decimal,
                                    _ => TipoTope::Izquierda,
                                },
                                relleno: None,
                            })
                            .collect(),
                    )
                }
                "defRPr" => p.def = PropR::leer(e, ctx),
                _ => {}
            }
        }
        p
    }
}

/// Los nueve niveles de una lista de estilos (`a:lstStyle`,
/// `p:titleStyle`...).
pub type Niveles = Vec<PropP>;

pub fn niveles_vacios() -> Niveles {
    vec![PropP::default(); 9]
}

/// Mezcla en `base` los niveles de `lista` (un `a:lstStyle` o similar).
pub fn mezclar_lista(base: &mut Niveles, lista: &Nodo, ctx: Contexto) {
    for e in lista.elementos() {
        if let Some(n) = e
            .nombre
            .strip_prefix("lvl")
            .and_then(|r| r.strip_suffix("pPr"))
            .and_then(|n| n.parse::<usize>().ok())
            && (1..=9).contains(&n)
        {
            base[n - 1].mezclar(&PropP::leer(e, ctx));
        }
        if e.nombre == "defPPr" {
            let d = PropP::leer(e, ctx);
            for b in base.iter_mut() {
                let mut x = d.clone();
                x.mezclar(b);
                *b = x;
            }
        }
    }
}

/// Los atributos de un `a:bodyPr`, todos opcionales para heredarlos.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cuerpo {
    pub izq: Option<f32>,
    pub arriba: Option<f32>,
    pub der: Option<f32>,
    pub abajo: Option<f32>,
    pub anclaje: Option<String>,
    pub centrado: Option<bool>,
    pub partir: Option<bool>,
    pub escala: Option<f32>,
    pub reduccion: Option<f32>,
    /// `normAutofit` sin `fontScale`: PowerPoint no guardo cuanto reducir,
    /// asi que se reduce aqui, a pasos, hasta que quepa.
    pub encoger: Option<bool>,
    pub vertical: Option<String>,
}

impl Cuerpo {
    pub fn leer(n: &Nodo) -> Cuerpo {
        let mut c = Cuerpo {
            izq: attr_num(n, "lIns", 1.0).map(emu),
            arriba: attr_num(n, "tIns", 1.0).map(emu),
            der: attr_num(n, "rIns", 1.0).map(emu),
            abajo: attr_num(n, "bIns", 1.0).map(emu),
            anclaje: Some(n.atributo("anchor").to_string()).filter(|s| !s.is_empty()),
            centrado: match n.atributo("anchorCtr") {
                "" => None,
                v => Some(matches!(v, "1" | "true")),
            },
            partir: match n.atributo("wrap") {
                "" => None,
                v => Some(v != "none"),
            },
            vertical: Some(n.atributo("vert").to_string()).filter(|s| !s.is_empty()),
            ..Default::default()
        };
        if let Some(a) = n.hijo("normAutofit") {
            let escala = attr_num(a, "fontScale", 1.0).map(|v| v / 100000.0);
            c.escala = Some(escala.unwrap_or(1.0));
            c.reduccion = Some(
                attr_num(a, "lnSpcReduction", 1.0)
                    .map(|v| v / 100000.0)
                    .unwrap_or(0.0),
            );
            c.encoger = Some(escala.is_none());
        } else if n.hijo("noAutofit").is_some() || n.hijo("spAutoFit").is_some() {
            c.escala = Some(1.0);
            c.reduccion = Some(0.0);
            c.encoger = Some(false);
        }
        c
    }

    pub fn mezclar(&mut self, o: &Cuerpo) {
        macro_rules! m {
            ($($c:ident),*) => { $( if o.$c.is_some() { self.$c = o.$c.clone(); } )* };
        }
        m!(
            izq, arriba, der, abajo, anclaje, centrado, partir, escala, reduccion, encoger,
            vertical
        );
    }
}

/// Lo que el texto necesita saber de fuera.
pub struct Entorno<'a> {
    pub ctx: Contexto<'a>,
    pub medidor: &'a dyn Medidor,
    /// Color del texto si nada lo dice (el `fontRef` del estilo, o `tx1`).
    pub color: Color,
    pub numero: u32,
}

fn numero_auto(tipo: &str, n: u32) -> String {
    use crate::docx::numeracion::formatear;
    let fmt = match tipo {
        t if t.starts_with("romanUc") => "upperRoman",
        t if t.starts_with("romanLc") => "lowerRoman",
        t if t.starts_with("alphaUc") => "upperLetter",
        t if t.starts_with("alphaLc") => "lowerLetter",
        _ => "decimal",
    };
    let base = formatear(n as i64, fmt);
    if tipo.ends_with("ParenBoth") {
        format!("({base})")
    } else if tipo.ends_with("ParenR") {
        format!("{base})")
    } else if tipo.ends_with("Period") {
        format!("{base}.")
    } else if tipo.ends_with("Minus") {
        format!("- {base}")
    } else {
        base
    }
}

/// Maqueta un cuerpo de texto en una caja de `ancho` x `alto` (origen 0,0)
/// y devuelve sus elementos. `niveles` ya trae toda la herencia de fuera
/// (estilo de la presentacion, patron, marcadores); aqui se suma el
/// `a:lstStyle` del propio cuerpo.
pub fn maquetar(
    tx: &Nodo,
    ancho: f32,
    alto: f32,
    niveles: &Niveles,
    cuerpo: &Cuerpo,
    e: &Entorno,
) -> Vec<Elemento> {
    let mut niveles = niveles.clone();
    if let Some(l) = tx.hijo("lstStyle") {
        mezclar_lista(&mut niveles, l, e.ctx);
    }
    let mut cuerpo = cuerpo.clone();
    if let Some(b) = tx.hijo("bodyPr") {
        cuerpo.mezclar(&Cuerpo::leer(b));
    }
    let izq = cuerpo.izq.unwrap_or(7.2);
    let der = cuerpo.der.unwrap_or(7.2);
    let arriba = cuerpo.arriba.unwrap_or(3.6);
    let abajo = cuerpo.abajo.unwrap_or(3.6);
    let partir = cuerpo.partir.unwrap_or(true);
    let escala_leida = cuerpo.escala.unwrap_or(1.0).clamp(0.05, 1.0);
    let reduccion_leida = cuerpo.reduccion.unwrap_or(0.0).clamp(0.0, 0.9);
    // Sin partir renglones el ancho solo sirve para alinear.
    let ancho_util = (ancho - izq - der).max(1.0);

    // Cada parrafo: sus renglones, ya con los altos.
    struct Bloque {
        renglones: Vec<renglones::Renglon>,
        estilos: Vec<Estilo>,
        antes: f32,
        despues: f32,
        alturas: Vec<(f32, f32)>,
    }
    // Maqueta todos los parrafos con una escala de letra y una reduccion
    // del interlineado (las del autoajuste).
    let armar = |escala: f32, reduccion: f32| -> Vec<Bloque> {
        let mut bloques = Vec::new();
        let mut contadores = [0u32; 9];
        let mut nivel_previo: Option<usize> = None;
        for p in tx.elementos().filter(|p| p.nombre == "p") {
            let ppr_nodo = p.hijo("pPr");
            let nivel = ppr_nodo
                .and_then(|x| x.atributo("lvl").parse::<usize>().ok())
                .unwrap_or(0)
                .min(8);
            let mut pp = niveles[nivel].clone();
            if let Some(x) = ppr_nodo {
                pp.mezclar(&PropP::leer(x, e.ctx));
            }
            let mut estilos: Vec<Estilo> = Vec::new();
            let mut estilo_de =
                |r: &PropR, campo: Option<Campo>, enlace: Option<Arc<str>>| -> u32 {
                    let tam = r.tam.unwrap_or(18.0) * escala;
                    let letra = r
                        .letra
                        .as_deref()
                        .map(|l| e.ctx.tema.letra(l))
                        .unwrap_or_else(|| e.ctx.tema.letra_texto.clone());
                    let subida = r.subida.unwrap_or(0.0) * tam;
                    let tam_real = if r.subida.is_some_and(|s| s.abs() > 0.01) {
                        tam * 0.67
                    } else {
                        tam
                    };
                    let est = Estilo {
                        letra: Letra {
                            familia: Arc::from(letra.as_str()),
                            tam: tam_real.max(1.0),
                            negrita: r.negrita.unwrap_or(false),
                            cursiva: r.cursiva.unwrap_or(false),
                        },
                        color: r.color.unwrap_or(e.color),
                        subrayado: r.subrayado.flatten(),
                        tachado: r.tachado.unwrap_or(false),
                        espaciado: r.espaciado.unwrap_or(0.0) * escala,
                        subida,
                        fondo: r.resaltado,
                        enlace,
                        campo,
                        efectos: r.efectos.clone().filter(|e| !e.is_empty()),
                    };
                    if let Some(i) = estilos.iter().position(|x| *x == est) {
                        return i as u32;
                    }
                    estilos.push(est);
                    (estilos.len() - 1) as u32
                };
            let mut piezas = Vec::new();
            let mut primer_rpr: Option<PropR> = None;
            for r in p.elementos() {
                match r.nombre.as_str() {
                    "r" | "fld" => {
                        let mut rp = pp.def.clone();
                        let mut enlace = None;
                        if let Some(x) = r.hijo("rPr") {
                            rp.mezclar(&PropR::leer(x, e.ctx));
                            if x.hijo("hlinkClick").is_some() {
                                enlace = Some(Arc::from("#"));
                                if rp.color.is_none() || x.hijo("solidFill").is_none() {
                                    rp.color = e.ctx.tema.esquema("hlink", e.ctx.mapa).or(rp.color);
                                }
                                if rp.subrayado.is_none() {
                                    rp.subrayado = Some(Some(Subrayado::Simple));
                                }
                            }
                        }
                        let mut texto = r.hijo("t").map(|t| t.texto()).unwrap_or_default();
                        let mut campo = None;
                        if r.nombre == "fld" && r.atributo("type") == "slidenum" {
                            texto = e.numero.to_string();
                            campo = Some(Campo::Pagina);
                        }
                        match rp.mayusculas {
                            Some(1) => texto = texto.to_uppercase(),
                            Some(2) => texto = texto.to_uppercase(),
                            _ => {}
                        }
                        if primer_rpr.is_none() {
                            primer_rpr = Some(rp.clone());
                        }
                        // Los tabuladores dentro del texto.
                        let mut trozos = texto.split('\t').peekable();
                        while let Some(t) = trozos.next() {
                            if !t.is_empty() {
                                let est = estilo_de(&rp, campo, enlace.clone());
                                piezas.push(Pieza::Texto {
                                    texto: t.replace(['\u{b}', '\r', '\n'], " "),
                                    estilo: est,
                                });
                            }
                            if trozos.peek().is_some() {
                                let est = estilo_de(&rp, None, None);
                                piezas.push(Pieza::Tab { estilo: est });
                            }
                        }
                    }
                    "br" => piezas.push(Pieza::Salto),
                    _ => {}
                }
            }
            // El final de parrafo mide el renglon vacio.
            let mut fin = pp.def.clone();
            if let Some(x) = p.hijo("endParaRPr") {
                fin.mezclar(&PropR::leer(x, e.ctx));
            }
            let marca = estilo_de(primer_rpr.as_ref().unwrap_or(&fin), None, None);

            // La vineta o el numero.
            let hay_texto = piezas
                .iter()
                .any(|x| matches!(x, Pieza::Texto { texto, .. } if !texto.trim().is_empty()));
            if nivel_previo.is_some_and(|n| n < nivel) {
                contadores[nivel] = 0;
            }
            if let Some(np) = nivel_previo
                && np > nivel
            {
                for c in contadores.iter_mut().skip(nivel + 1) {
                    *c = 0;
                }
            }
            nivel_previo = Some(nivel);
            let marl = pp.marl.unwrap_or(0.0);
            let sangria = pp.sangria.unwrap_or(0.0);
            let mut topes = pp.topes.clone().unwrap_or_default();
            if hay_texto {
                let base = primer_rpr.clone().unwrap_or_else(|| fin.clone());
                let vineta = match &pp.vineta {
                    Some(Vineta::Caracter(c)) => Some(c.to_string()),
                    Some(Vineta::Numero { tipo, inicio }) => {
                        let n = *inicio + contadores[nivel];
                        contadores[nivel] += 1;
                        Some(numero_auto(tipo, n))
                    }
                    _ => None,
                };
                if let Some(v) = vineta {
                    let mut rb = base.clone();
                    rb.subrayado = Some(None);
                    if let Some(Some(f)) = &pp.vineta_letra {
                        rb.letra = Some(f.clone());
                    }
                    if let Some(Some(c)) = pp.vineta_color {
                        rb.color = Some(c);
                    }
                    if let Some(t) = pp.vineta_tam {
                        rb.tam = Some(base.tam.unwrap_or(18.0) * t);
                    }
                    let est = estilo_de(&rb, None, None);
                    let est_tab = estilo_de(&base, None, None);
                    let mut delante = vec![Pieza::Texto {
                        texto: v,
                        estilo: est,
                    }];
                    // Con sangria francesa el texto empieza en el margen; sin
                    // ella va pegado a la vineta (como PowerPoint).
                    if sangria < 0.0 {
                        delante.push(Pieza::Tab { estilo: est_tab });
                        topes.push(Tope {
                            pos: marl,
                            tipo: TipoTope::Izquierda,
                            relleno: None,
                        });
                    }
                    delante.append(&mut piezas);
                    piezas = delante;
                }
            }
            topes.sort_by(|a, b| a.pos.total_cmp(&b.pos));
            let cfg = Config {
                ancho: ancho_util,
                sangria_izq: marl,
                sangria_der: 0.0,
                primera: sangria,
                topes,
                tope_defecto: pp.tope_defecto.unwrap_or(72.0),
                alineacion: pp.alineacion.unwrap_or_default(),
                partir,
                encoger: 0.0,
            };
            let renglones = renglones::partir(&piezas, &estilos, marca, &cfg, e.medidor);
            let tam_ref = estilos
                .get(marca as usize)
                .map(|s| s.letra.tam)
                .unwrap_or(18.0);
            let alturas: Vec<(f32, f32)> = renglones
                .iter()
                .map(|r| {
                    let natural = r.ascenso + r.descenso;
                    // PowerPoint: el 100 % es 1,2 veces el tamano de la letra.
                    let base = (r.tam_max.max(1.0) * 1.2).max(natural);
                    match pp.interlineado {
                        Some(Esp::Pts(v)) => (v, v - r.descenso),
                        Some(Esp::Pct(pc)) => {
                            let pc = (pc - reduccion).max(0.1);
                            let h = base * pc;
                            (h, h - r.descenso - (base - natural) * pc * 0.0)
                        }
                        None => {
                            let h = base * (1.0 - reduccion);
                            (h, h - r.descenso)
                        }
                    }
                })
                .collect();
            let resolver = |x: Option<Esp>| match x {
                Some(Esp::Pts(v)) => v,
                Some(Esp::Pct(p)) => p * tam_ref * 1.2,
                None => 0.0,
            };
            bloques.push(Bloque {
                renglones,
                estilos,
                antes: resolver(pp.antes),
                despues: resolver(pp.despues),
                alturas,
            });
        }
        bloques
    };

    // El alto total, para el anclaje.
    let alto_total = |bloques: &[Bloque]| -> f32 {
        let mut total = 0.0;
        for (i, b) in bloques.iter().enumerate() {
            if i > 0 {
                total += b.antes;
            }
            total += b.alturas.iter().map(|a| a.0).sum::<f32>();
            if i + 1 < bloques.len() {
                total += b.despues;
            }
        }
        total
    };
    let mut bloques = armar(escala_leida, reduccion_leida);
    let mut total = alto_total(&bloques);
    let disponible = alto - arriba - abajo;
    if cuerpo.encoger == Some(true) && alto > 0.0 && total > disponible + 0.5 {
        // Como PowerPoint: primero se aprietan los renglones y despues se
        // reduce la letra, a pasos, hasta que quepa (o hasta el minimo).
        for (escala, reduccion) in PASOS_AUTOAJUSTE {
            bloques = armar(escala, reduccion);
            total = alto_total(&bloques);
            if total <= disponible + 0.5 {
                break;
            }
        }
    }
    let libre = disponible - total;
    let dy = match cuerpo.anclaje.as_deref() {
        Some("ctr") => libre / 2.0,
        Some("b") => libre,
        _ => 0.0,
    };
    let mut salida = Vec::new();
    let mut y = arriba + dy;
    // Centrado horizontal del bloque entero (`anchorCtr`).
    let dx = if cuerpo.centrado == Some(true) && partir {
        let mas_ancho = bloques
            .iter()
            .flat_map(|b| b.renglones.iter().map(|r| r.fin))
            .fold(0.0f32, f32::max);
        ((ancho_util - mas_ancho) / 2.0).max(0.0)
    } else {
        0.0
    };
    for (i, b) in bloques.into_iter().enumerate() {
        if i > 0 {
            y += b.antes;
        }
        for (r, (h, base)) in b.renglones.iter().zip(b.alturas.iter()) {
            let mut els = Vec::new();
            crate::docx::bloques::renglon_a_elementos(
                r,
                &b.estilos,
                &[],
                *base,
                e.medidor,
                &mut els,
            );
            for mut el in els {
                el.trasladar(izq + dx, y);
                salida.push(el);
            }
            y += h;
        }
        y += b.despues;
    }
    salida
}

/// Los pasos del autoajuste cuando el archivo no trae `fontScale`: escala
/// de la letra y reduccion del interlineado, como los que guarda
/// PowerPoint (`fontScale="92500" lnSpcReduction="10000"`...).
const PASOS_AUTOAJUSTE: [(f32, f32); 10] = [
    (0.925, 0.1),
    (0.85, 0.2),
    (0.775, 0.2),
    (0.7, 0.2),
    (0.625, 0.2),
    (0.55, 0.2),
    (0.475, 0.2),
    (0.4, 0.2),
    (0.325, 0.2),
    (0.25, 0.2),
];

/// La caja de texto de un rectangulo, para quien la necesite.
pub fn caja_util(caja: Rect, c: &Cuerpo) -> Rect {
    Rect::nuevo(
        caja.x + c.izq.unwrap_or(7.2),
        caja.y + c.arriba.unwrap_or(3.6),
        caja.ancho - c.izq.unwrap_or(7.2) - c.der.unwrap_or(7.2),
        caja.alto - c.arriba.unwrap_or(3.6) - c.abajo.unwrap_or(3.6),
    )
}
