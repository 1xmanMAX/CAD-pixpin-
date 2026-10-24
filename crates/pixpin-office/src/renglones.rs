//! **Partir un parrafo en renglones.** Lo comparten Word y PowerPoint: les
//! llega una lista de piezas (texto con estilo, tabuladores, saltos y
//! objetos en linea) y devuelve renglones con cada trozo ya en su x.
//!
//! El reparto es el voraz de siempre, que es el que hace Word: se meten
//! palabras mientras quepan; los espacios del final de un renglon cuelgan
//! fuera (no cuentan para caber ni para alinear), y una palabra que no cabe
//! sola en un renglon se parte por letras.

use std::sync::Arc;

use crate::medidor::Medidor;
use crate::pagina::{Campo, Color, Efecto, Letra, Subrayado};

/// El estilo de un trozo de texto, tal como lo necesita la maquetacion.
#[derive(Debug, Clone, PartialEq)]
pub struct Estilo {
    pub letra: Letra,
    pub color: Color,
    pub subrayado: Option<Subrayado>,
    pub tachado: bool,
    /// Interletrado, en puntos.
    pub espaciado: f32,
    /// Subida de la linea base (superindice positivo), en puntos.
    pub subida: f32,
    /// Fondo del texto (resaltado o sombreado de la corrida).
    pub fondo: Option<Color>,
    pub enlace: Option<Arc<str>>,
    pub campo: Option<Campo>,
    /// Efectos del texto (la sombra o el brillo de un `a:rPr/a:effectLst`).
    pub efectos: Option<Arc<[Efecto]>>,
}

impl Estilo {
    pub fn de_letra(letra: Letra) -> Estilo {
        Estilo {
            letra,
            color: Color::NEGRO,
            subrayado: None,
            tachado: false,
            espaciado: 0.0,
            subida: 0.0,
            fondo: None,
            enlace: None,
            campo: None,
            efectos: None,
        }
    }
}

/// Una pieza de la entrada.
#[derive(Debug, Clone, PartialEq)]
pub enum Pieza {
    Texto {
        texto: String,
        estilo: u32,
    },
    Tab {
        estilo: u32,
    },
    /// Salto de renglon (`w:br`, `a:br`).
    Salto,
    /// Un objeto en linea (una imagen, una formula): `alto` por encima de
    /// la linea base y `bajo` por debajo.
    Objeto {
        ancho: f32,
        alto: f32,
        bajo: f32,
        id: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alineacion {
    #[default]
    Izquierda,
    Centro,
    Derecha,
    Justificada,
    /// Justificada tambien el ultimo renglon.
    Distribuida,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoTope {
    Izquierda,
    Centro,
    Derecha,
    Decimal,
}

/// Un tope de tabulador, en puntos desde el borde izquierdo de la caja.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tope {
    pub pos: f32,
    pub tipo: TipoTope,
    /// Caracter de relleno (los puntos de un indice).
    pub relleno: Option<char>,
}

/// La geometria de un parrafo.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Ancho total de la caja (sin sangrias).
    pub ancho: f32,
    pub sangria_izq: f32,
    pub sangria_der: f32,
    /// Desplazamiento del primer renglon respecto a `sangria_izq`
    /// (negativo = sangria francesa).
    pub primera: f32,
    pub topes: Vec<Tope>,
    /// Cada cuanto hay un tope por defecto.
    pub tope_defecto: f32,
    pub alineacion: Alineacion,
    /// Si se parte en renglones (PowerPoint `wrap="none"` no lo hace).
    pub partir: bool,
    /// Cuanto pueden encogerse los espacios de un renglon justificado para
    /// que quepa una palabra mas (Word: hasta un 20 %). 0 = nada.
    pub encoger: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ancho: 0.0,
            sangria_izq: 0.0,
            sangria_der: 0.0,
            primera: 0.0,
            topes: Vec::new(),
            tope_defecto: 36.0,
            alineacion: Alineacion::Izquierda,
            partir: true,
            encoger: 0.0,
        }
    }
}

/// Que lleva un trozo de un renglon.
#[derive(Debug, Clone, PartialEq)]
pub enum Contenido {
    Texto {
        texto: String,
        estilo: u32,
        extra_espacio: f32,
    },
    Objeto {
        id: u32,
        alto: f32,
    },
    /// El relleno de un tabulador (`....`), con el estilo del tabulador.
    Relleno {
        caracter: char,
        estilo: u32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Trozo {
    /// Desde el borde izquierdo de la caja.
    pub x: f32,
    pub ancho: f32,
    pub contenido: Contenido,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Renglon {
    pub trozos: Vec<Trozo>,
    pub ascenso: f32,
    pub descenso: f32,
    /// El mayor `lineGap` de las letras del renglon.
    pub hueco: f32,
    /// El mayor tamano de letra del renglon (para el interlineado de
    /// PowerPoint, que va en proporcion al tamano).
    pub tam_max: f32,
    /// El ascenso solo de las letras (sin los objetos en linea): el
    /// interlineado multiple de Word solo multiplica el texto.
    pub ascenso_texto: f32,
    /// Alto del renglon a interlineado sencillo, y el de solo el texto.
    pub alto_natural: f32,
    pub alto_texto: f32,
    /// Hasta donde llega lo escrito (sin los espacios del final), desde el
    /// borde de la caja.
    pub fin: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tipo {
    Letra(char),
    Tab,
    Salto,
    Objeto { id: u32, alto: f32, bajo: f32 },
}

/// Una letra, tabulador, salto u objeto ya medido: lo que se reparte en
/// renglones. Se guardan para volver a partir un parrafo sin medirlo otra
/// vez (cuando un dibujo flotante le estrecha unos renglones).
#[derive(Debug, Clone, Copy)]
pub struct Unidad {
    tipo: Tipo,
    avance: f32,
    estilo: u32,
}

fn es_espacio(c: char) -> bool {
    c == ' ' || c == '\u{3000}'
}

fn es_cjk(c: char) -> bool {
    matches!(c as u32, 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF | 0x20000..=0x2FFFF)
}

/// Se puede partir despues de la unidad `i`?
fn corte_tras(u: &[Unidad], i: usize) -> bool {
    let sig = u.get(i + 1).map(|x| x.tipo);
    match u[i].tipo {
        Tipo::Letra(c) if es_espacio(c) => !matches!(sig, Some(Tipo::Letra(s)) if es_espacio(s)),
        Tipo::Letra('-')
        | Tipo::Letra('\u{2010}')
        | Tipo::Letra('\u{2013}')
        | Tipo::Letra('\u{2014}') => {
            matches!(sig, Some(Tipo::Letra(s)) if s.is_alphanumeric())
        }
        Tipo::Letra('\u{200B}') | Tipo::Letra('\u{00AD}') | Tipo::Letra('/') => true,
        Tipo::Letra(c) if es_cjk(c) => true,
        Tipo::Tab | Tipo::Objeto { .. } => true,
        _ => {
            matches!(sig, Some(Tipo::Letra(s)) if es_cjk(s))
                || matches!(sig, Some(Tipo::Objeto { .. }))
        }
    }
}

/// Parte las piezas en renglones.
pub fn partir(
    piezas: &[Pieza],
    estilos: &[Estilo],
    marca: u32,
    cfg: &Config,
    medidor: &dyn Medidor,
) -> Vec<Renglon> {
    let mut u = medir(piezas, estilos, marca, medidor);
    partir_unidades(&mut u, 0, true, estilos, marca, cfg, medidor)
        .into_iter()
        .map(|(r, _)| r)
        .collect()
}

/// Las piezas medidas, listas para partir.
pub fn medir(
    piezas: &[Pieza],
    estilos: &[Estilo],
    marca: u32,
    medidor: &dyn Medidor,
) -> Vec<Unidad> {
    let mut u: Vec<Unidad> = Vec::new();
    let mut avances = Vec::new();
    for p in piezas {
        match p {
            Pieza::Texto { texto, estilo } => {
                let Some(e) = estilos.get(*estilo as usize) else {
                    continue;
                };
                avances.clear();
                medidor.avances(texto, &e.letra, &mut avances);
                for (k, c) in texto.chars().enumerate() {
                    let a = avances.get(k).copied().unwrap_or(0.0);
                    let a = if c == '\u{00AD}' || c == '\u{200B}' {
                        0.0
                    } else {
                        a + e.espaciado
                    };
                    u.push(Unidad {
                        tipo: Tipo::Letra(c),
                        avance: a,
                        estilo: *estilo,
                    });
                }
            }
            Pieza::Tab { estilo } => u.push(Unidad {
                tipo: Tipo::Tab,
                avance: 0.0,
                estilo: *estilo,
            }),
            Pieza::Salto => u.push(Unidad {
                tipo: Tipo::Salto,
                avance: 0.0,
                estilo: marca,
            }),
            Pieza::Objeto {
                ancho,
                alto,
                bajo,
                id,
            } => u.push(Unidad {
                tipo: Tipo::Objeto {
                    id: *id,
                    alto: *alto,
                    bajo: *bajo,
                },
                avance: *ancho,
                estilo: marca,
            }),
        }
    }
    u
}

/// Parte en renglones a lo ancho de la caja desde la unidad `desde`
/// (`primero`: el renglon que sale es el primero del parrafo, con su
/// sangria). Cada renglon va con la unidad en que empieza.
pub fn partir_unidades(
    u: &mut [Unidad],
    desde: usize,
    primero: bool,
    estilos: &[Estilo],
    marca: u32,
    cfg: &Config,
    medidor: &dyn Medidor,
) -> Vec<(Renglon, usize)> {
    let mut renglones = Vec::new();
    let mut xs = vec![0.0f32; u.len()];
    let mut rellenos: Vec<Option<char>> = vec![None; u.len()];
    let mut i = desde.min(u.len());
    let derecha = cfg.ancho - cfg.sangria_der;
    let mut primero = primero;
    loop {
        let inicio_x = cfg.sangria_izq + if primero { cfg.primera } else { 0.0 };
        primero = false;
        let (renglon, fin_j, forzado) = un_renglon(
            u,
            i,
            inicio_x,
            derecha,
            true,
            &mut xs,
            &mut rellenos,
            estilos,
            marca,
            cfg,
            medidor,
        );
        renglones.push((renglon, i));
        i = fin_j;
        if i >= u.len() {
            // Un salto de renglon al final deja la marca de parrafo sola en
            // un renglon nuevo, vacio pero con su alto.
            if forzado {
                renglones.push((montar(&[], &[], &[], estilos, marca, true, medidor), i));
            }
            break;
        }
    }
    renglones
}

/// Un renglon hecho de tramos uno al lado del otro (el texto a los dos lados
/// de un dibujo flotante), de la unidad `i` en adelante. `tramos` van de
/// izquierda a derecha en x de la caja. Devuelve el renglon, la unidad en
/// que empieza el siguiente y si acaba en un salto.
pub fn renglon_en_tramos(
    u: &mut [Unidad],
    i: usize,
    tramos: &[(f32, f32)],
    estilos: &[Estilo],
    marca: u32,
    cfg: &Config,
    medidor: &dyn Medidor,
) -> (Renglon, usize, bool) {
    let mut xs = vec![0.0f32; u.len()];
    let mut rellenos: Vec<Option<char>> = vec![None; u.len()];
    let mut total: Option<Renglon> = None;
    let mut pos = i.min(u.len());
    let mut forzado = false;
    for (k, &(a, b)) in tramos.iter().enumerate() {
        // El ultimo tramo tiene que llevarse algo si ninguno se llevo nada
        // (si no, el parrafo no avanzaria).
        let forzar = k + 1 == tramos.len() && pos == i;
        let (r, fin, fz) = un_renglon(
            u,
            pos,
            a,
            b,
            forzar,
            &mut xs,
            &mut rellenos,
            estilos,
            marca,
            cfg,
            medidor,
        );
        if fin > pos || fz || total.is_none() && forzar {
            total = Some(match total {
                None => r,
                Some(mut t) => {
                    t.trozos.extend(r.trozos);
                    t.ascenso = t.ascenso.max(r.ascenso);
                    t.ascenso_texto = t.ascenso_texto.max(r.ascenso_texto);
                    t.descenso = t.descenso.max(r.descenso);
                    t.hueco = t.hueco.max(r.hueco);
                    t.tam_max = t.tam_max.max(r.tam_max);
                    t.alto_texto = t.ascenso_texto + t.descenso + t.hueco;
                    t.alto_natural = t.ascenso.max(t.ascenso_texto + t.hueco) + t.descenso;
                    t.fin = r.fin;
                    t
                }
            });
        }
        pos = fin;
        if fz {
            forzado = true;
            break;
        }
        if pos >= u.len() {
            break;
        }
    }
    let r = total.unwrap_or_else(|| montar(&[], &[], &[], estilos, marca, true, medidor));
    (r, pos, forzado)
}

/// Un renglon de la unidad `i` en adelante, de `inicio_x` a `derecha`.
/// `forzar`: lleva al menos una unidad aunque no quepa (partiendo una
/// palabra por letras); sin forzar, una palabra que no cabe deja el renglon
/// vacio. Devuelve el renglon, donde empieza el siguiente y si acaba en un
/// salto.
#[allow(clippy::too_many_arguments)]
fn un_renglon(
    u: &mut [Unidad],
    i: usize,
    inicio_x: f32,
    derecha: f32,
    forzar: bool,
    xs: &mut [f32],
    rellenos: &mut [Option<char>],
    estilos: &[Estilo],
    marca: u32,
    cfg: &Config,
    medidor: &dyn Medidor,
) -> (Renglon, usize, bool) {
    let mut x = inicio_x;
    let mut j = i;
    // Donde se puede cortar: indice de la primera unidad del renglon
    // siguiente.
    let mut ultimo_corte: Option<usize> = None;
    let mut forzado = false;
    // Lo que miden los espacios del renglon (tras el ultimo tabulador).
    let mut espacios = 0.0f32;
    let mut fin_j = u.len();
    while j < u.len() {
        match u[j].tipo {
            Tipo::Salto => {
                xs[j] = x;
                fin_j = j + 1;
                forzado = true;
                break;
            }
            Tipo::Tab => {
                let (nx, tope) = tabulador(u, j, x, cfg);
                if cfg.partir
                    && nx > derecha + 0.01
                    && (j > i || !forzar)
                    && !matches!(tope.map(|t| t.tipo), Some(TipoTope::Derecha))
                {
                    // El tabulador no cabe: pasa con lo que le sigue al
                    // renglon siguiente.
                    fin_j = j;
                    break;
                }
                xs[j] = x;
                rellenos[j] = tope.and_then(|t| t.relleno);
                u[j].avance = (nx - x).max(0.0);
                x = nx;
                espacios = 0.0;
                ultimo_corte = Some(j + 1);
            }
            Tipo::Letra(c) if es_espacio(c) => {
                xs[j] = x;
                x += u[j].avance;
                espacios += u[j].avance;
                if corte_tras(u, j) {
                    ultimo_corte = Some(j + 1);
                }
            }
            _ => {
                if cfg.partir
                    && x + u[j].avance - cfg.encoger * espacios > derecha + 0.01
                    && (j > i || !forzar)
                {
                    fin_j = match ultimo_corte {
                        Some(c) if c > i => c,
                        _ if forzar => j,
                        _ => i,
                    };
                    break;
                }
                xs[j] = x;
                x += u[j].avance;
                if corte_tras(u, j) {
                    ultimo_corte = Some(j + 1);
                }
            }
        }
        j += 1;
    }
    let ultimo = fin_j >= u.len();
    let renglon = montar(
        &u[i..fin_j],
        &xs[i..fin_j],
        &rellenos[i..fin_j],
        estilos,
        marca,
        ultimo,
        medidor,
    );
    let renglon = alinear(
        renglon,
        &u[i..fin_j],
        &xs[i..fin_j],
        cfg,
        derecha,
        ultimo || forzado,
    );
    (renglon, fin_j, forzado)
}

/// Donde acaba un tabulador que empieza en `x`, y de que tipo es su tope.
fn tabulador(u: &[Unidad], j: usize, x: f32, cfg: &Config) -> (f32, Option<Tope>) {
    let tope = cfg
        .topes
        .iter()
        .filter(|t| t.pos > x + 0.01)
        .min_by(|a, b| a.pos.total_cmp(&b.pos))
        .copied();
    let Some(t) = tope else {
        let d = cfg.tope_defecto.max(1.0);
        // El siguiente multiplo del tope por defecto (contados desde el
        // borde de la caja).
        return (((x + 0.01) / d).floor() * d + d, None);
    };
    // Lo que va detras hasta el siguiente tabulador o salto.
    let mut ancho = 0.0;
    let mut hasta_decimal = None;
    for un in &u[j + 1..] {
        match un.tipo {
            Tipo::Tab | Tipo::Salto => break,
            Tipo::Letra(c) => {
                if (c == '.' || c == ',') && hasta_decimal.is_none() {
                    hasta_decimal = Some(ancho);
                }
                ancho += un.avance;
            }
            Tipo::Objeto { .. } => ancho += un.avance,
        }
    }
    let nx = match t.tipo {
        TipoTope::Izquierda => t.pos,
        TipoTope::Derecha => (t.pos - ancho).max(x),
        TipoTope::Centro => (t.pos - ancho / 2.0).max(x),
        TipoTope::Decimal => (t.pos - hasta_decimal.unwrap_or(ancho)).max(x),
    };
    (nx, Some(t))
}

/// Junta las unidades de un renglon en trozos.
#[allow(clippy::needless_range_loop)] // `u` y `xs` van a la par
fn montar(
    u: &[Unidad],
    xs: &[f32],
    rellenos: &[Option<char>],
    estilos: &[Estilo],
    marca: u32,
    _ultimo: bool,
    medidor: &dyn Medidor,
) -> Renglon {
    let mut r = Renglon::default();
    let mut usados: Vec<u32> = Vec::new();
    let mut k = 0;
    while k < u.len() {
        match u[k].tipo {
            Tipo::Letra(_) => {
                let estilo = u[k].estilo;
                let x0 = xs[k];
                let mut texto = String::new();
                let mut ancho = 0.0;
                while k < u.len() {
                    let Tipo::Letra(c) = u[k].tipo else { break };
                    if u[k].estilo != estilo {
                        break;
                    }
                    if c != '\u{00AD}' && c != '\u{200B}' {
                        texto.push(c);
                    }
                    ancho += u[k].avance;
                    k += 1;
                }
                if !usados.contains(&estilo) {
                    usados.push(estilo);
                }
                r.trozos.push(Trozo {
                    x: x0,
                    ancho,
                    contenido: Contenido::Texto {
                        texto,
                        estilo,
                        extra_espacio: 0.0,
                    },
                });
            }
            Tipo::Tab => {
                let e = u[k].estilo;
                if let Some(rel) = rellenos[k]
                    && u[k].avance > 0.0
                {
                    r.trozos.push(Trozo {
                        x: xs[k],
                        ancho: u[k].avance,
                        contenido: Contenido::Relleno {
                            caracter: rel,
                            estilo: e,
                        },
                    });
                }
                k += 1;
            }
            Tipo::Salto => k += 1,
            Tipo::Objeto { id, alto, bajo } => {
                r.trozos.push(Trozo {
                    x: xs[k],
                    ancho: u[k].avance,
                    contenido: Contenido::Objeto { id, alto },
                });
                r.ascenso = r.ascenso.max(alto);
                r.descenso = r.descenso.max(bajo);
                k += 1;
            }
        }
    }
    if usados.is_empty() {
        // Un renglon vacio mide con la marca de parrafo (Word no la cuenta
        // en un renglon con texto).
        usados.push(marca);
    }
    // El interlineado multiple de Word no multiplica lo que suben las
    // letras de simbolos (las vinetas en Symbol o Wingdings).
    let es_simbolo = |e: &u32| {
        estilos.get(*e as usize).is_some_and(|s| {
            matches!(
                &*s.letra.familia,
                "Symbol" | "Wingdings" | "Wingdings 2" | "Wingdings 3" | "Webdings"
            )
        })
    };
    let solo_simbolos = !usados.iter().any(|e| !es_simbolo(e));
    for e in usados {
        let Some(est) = estilos.get(e as usize) else {
            continue;
        };
        let m = medidor.metricas(&est.letra);
        r.ascenso = r.ascenso.max(m.ascenso + est.subida.max(0.0));
        if solo_simbolos || !es_simbolo(&e) {
            r.ascenso_texto = r.ascenso_texto.max(m.ascenso + est.subida.max(0.0));
        }
        r.descenso = r.descenso.max(m.descenso - est.subida.min(0.0));
        if solo_simbolos || !es_simbolo(&e) {
            r.hueco = r.hueco.max(m.hueco);
        }
        r.tam_max = r.tam_max.max(est.letra.tam);
    }
    // El alto sencillo del renglon: el hueco entre renglones solo cuenta
    // encima de las letras de texto; lo que sube de mas (simbolos, objetos)
    // lo absorbe.
    r.alto_texto = r.ascenso_texto + r.descenso + r.hueco;
    r.alto_natural = r.ascenso.max(r.ascenso_texto + r.hueco) + r.descenso;
    // Hasta donde llega lo escrito, sin los espacios colgados.
    let mut fin = xs.first().copied().unwrap_or(0.0);
    for k in 0..u.len() {
        match u[k].tipo {
            Tipo::Letra(c) if es_espacio(c) => {}
            Tipo::Salto => {}
            _ => fin = fin.max(xs[k] + u[k].avance),
        }
    }
    r.fin = fin;
    r
}

fn alinear(
    mut r: Renglon,
    u: &[Unidad],
    xs: &[f32],
    cfg: &Config,
    derecha: f32,
    ultimo: bool,
) -> Renglon {
    // Negativo si el renglon solo cabe encogiendo los espacios.
    let libre = derecha - r.fin;
    let desplazar = match cfg.alineacion {
        Alineacion::Centro => libre.max(0.0) / 2.0,
        Alineacion::Derecha => libre.max(0.0),
        _ => 0.0,
    };
    let justificar = match cfg.alineacion {
        Alineacion::Justificada => !ultimo || libre < 0.0,
        Alineacion::Distribuida => true,
        _ => false,
    };
    let libre = if justificar { libre } else { libre.max(0.0) };
    if justificar && libre.abs() > 0.001 {
        // Los espacios que se estiran: los de despues del ultimo tabulador y
        // antes del final escrito.
        let desde = u
            .iter()
            .rposition(|x| x.tipo == Tipo::Tab)
            .map(|p| p + 1)
            .unwrap_or(0);
        let fin = r.fin;
        let cuantos = (desde..u.len())
            .filter(|&k| {
                matches!(u[k].tipo, Tipo::Letra(c) if es_espacio(c)) && xs[k] + 0.001 < fin
            })
            .count();
        if cuantos > 0 {
            let extra = libre / cuantos as f32;
            // Rehacer las x de los trozos: cada trozo se desplaza lo que
            // sumen los espacios estirados que tenga delante.
            let x_corte = if desde > 0 {
                xs[desde - 1]
            } else {
                f32::NEG_INFINITY
            };
            for t in &mut r.trozos {
                if t.x <= x_corte {
                    continue;
                }
                let antes = (desde..u.len())
                    .filter(|&k| {
                        matches!(u[k].tipo, Tipo::Letra(c) if es_espacio(c)) && xs[k] + 0.001 < t.x
                    })
                    .count();
                t.x += extra * antes as f32;
                if let Contenido::Texto {
                    texto,
                    extra_espacio,
                    ..
                } = &mut t.contenido
                {
                    // Los espacios de dentro del trozo, sin los colgados.
                    let dentro = texto.trim_end().chars().filter(|c| es_espacio(*c)).count();
                    *extra_espacio = extra;
                    t.ancho += extra * dentro as f32;
                }
            }
            r.fin = derecha;
        }
    }
    if desplazar != 0.0 {
        for t in &mut r.trozos {
            t.x += desplazar;
        }
        r.fin += desplazar;
    }
    r
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::medidor::MedidorFijo;

    fn estilos() -> Vec<Estilo> {
        vec![Estilo::de_letra(Letra::nueva("X", 10.0))]
    }

    fn texto(r: &Renglon) -> String {
        r.trozos
            .iter()
            .filter_map(|t| match &t.contenido {
                Contenido::Texto { texto, .. } => Some(texto.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn las_palabras_se_reparten_y_los_espacios_cuelgan() {
        // Cada letra mide 5: «uno dos tres» en 40 puntos (8 letras).
        let p = vec![Pieza::Texto {
            texto: "uno dos tres".into(),
            estilo: 0,
        }];
        let cfg = Config {
            ancho: 40.0,
            ..Default::default()
        };
        let r = partir(&p, &estilos(), 0, &cfg, &MedidorFijo::default());
        assert_eq!(r.len(), 2);
        assert_eq!(texto(&r[0]), "uno dos ");
        assert_eq!(texto(&r[1]), "tres");
        assert_eq!(r[0].fin, 35.0, "el espacio del final no cuenta");
        assert_eq!(r[0].ascenso, 8.0);
    }

    #[test]
    fn una_palabra_mas_larga_que_el_renglon_se_parte_por_letras() {
        let p = vec![Pieza::Texto {
            texto: "abcdefghij".into(),
            estilo: 0,
        }];
        let cfg = Config {
            ancho: 20.0,
            ..Default::default()
        };
        let r = partir(&p, &estilos(), 0, &cfg, &MedidorFijo::default());
        assert_eq!(
            r.iter().map(texto).collect::<Vec<_>>(),
            vec!["abcd", "efgh", "ij"]
        );
    }

    #[test]
    fn el_centrado_y_la_derecha_desplazan() {
        let p = vec![Pieza::Texto {
            texto: "ab".into(),
            estilo: 0,
        }];
        let mut cfg = Config {
            ancho: 100.0,
            alineacion: Alineacion::Centro,
            ..Default::default()
        };
        let r = partir(&p, &estilos(), 0, &cfg, &MedidorFijo::default());
        assert_eq!(r[0].trozos[0].x, 45.0);
        cfg.alineacion = Alineacion::Derecha;
        let r = partir(&p, &estilos(), 0, &cfg, &MedidorFijo::default());
        assert_eq!(r[0].trozos[0].x, 90.0);
    }

    #[test]
    fn el_justificado_llena_todos_menos_el_ultimo() {
        let p = vec![Pieza::Texto {
            texto: "aa bb cc dd".into(),
            estilo: 0,
        }];
        let cfg = Config {
            ancho: 50.0,
            alineacion: Alineacion::Justificada,
            ..Default::default()
        };
        let r = partir(&p, &estilos(), 0, &cfg, &MedidorFijo::default());
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].fin, 50.0);
        match &r[0].trozos[0].contenido {
            Contenido::Texto { extra_espacio, .. } => assert!(*extra_espacio > 0.0),
            _ => panic!(),
        }
        assert!(r[1].fin < 50.0, "el ultimo renglon no se estira");
    }

    #[test]
    fn el_tabulador_salta_al_tope() {
        let p = vec![
            Pieza::Texto {
                texto: "a".into(),
                estilo: 0,
            },
            Pieza::Tab { estilo: 0 },
            Pieza::Texto {
                texto: "b".into(),
                estilo: 0,
            },
            Pieza::Tab { estilo: 0 },
            Pieza::Texto {
                texto: "9".into(),
                estilo: 0,
            },
        ];
        let cfg = Config {
            ancho: 200.0,
            topes: vec![Tope {
                pos: 100.0,
                tipo: TipoTope::Derecha,
                relleno: Some('.'),
            }],
            tope_defecto: 36.0,
            ..Default::default()
        };
        let r = partir(&p, &estilos(), 0, &cfg, &MedidorFijo::default());
        let xs: Vec<f32> = r[0]
            .trozos
            .iter()
            .filter(|t| matches!(t.contenido, Contenido::Texto { .. }))
            .map(|t| t.x)
            .collect();
        // Un tope propio anula los de por defecto de su izquierda: «b» acaba en
        // 100 y «9» va al siguiente tope por defecto (108).
        assert_eq!(xs, vec![0.0, 95.0, 108.0]);
        assert!(
            r[0].trozos
                .iter()
                .any(|t| matches!(t.contenido, Contenido::Relleno { caracter: '.', .. }))
        );
    }

    #[test]
    fn el_salto_parte_y_un_parrafo_vacio_tiene_alto() {
        let p = vec![
            Pieza::Texto {
                texto: "a".into(),
                estilo: 0,
            },
            Pieza::Salto,
            Pieza::Texto {
                texto: "b".into(),
                estilo: 0,
            },
        ];
        let cfg = Config {
            ancho: 200.0,
            ..Default::default()
        };
        let r = partir(&p, &estilos(), 0, &cfg, &MedidorFijo::default());
        assert_eq!(r.len(), 2);
        let r = partir(&[], &estilos(), 0, &cfg, &MedidorFijo::default());
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].ascenso + r[0].descenso, 10.0);
    }

    #[test]
    fn la_sangria_francesa_mueve_el_primer_renglon() {
        let p = vec![Pieza::Texto {
            texto: "aaaa bbbb".into(),
            estilo: 0,
        }];
        let cfg = Config {
            ancho: 40.0,
            sangria_izq: 10.0,
            primera: -10.0,
            ..Default::default()
        };
        let r = partir(&p, &estilos(), 0, &cfg, &MedidorFijo::default());
        assert_eq!(r[0].trozos[0].x, 0.0);
        assert_eq!(r[1].trozos[0].x, 10.0);
    }
}
