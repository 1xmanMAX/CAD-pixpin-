//! **Las formulas de Office** (OMML: `m:oMath`). Una maquetacion compacta de
//! las construcciones de siempre —fracciones, indices, delimitadores que
//! crecen, matrices, raices, sumatorios e integrales, funciones, acentos—
//! en Cambria Math, sin pretender la tipografia completa de TeX: lo justo
//! para que una hoja de calculo de ingenieria (Calcpad, Mathcad exportado a
//! Word) se lea como en Word.
//!
//! Cada pieza es una [`Caja`] con su linea base en y = 0 (lo de arriba es
//! negativo); al final la formula entera se entrega como un [`Objeto`] en
//! linea con su alto sobre la base y lo que baja.

use std::sync::Arc;

use pixpin_docs::xml::Nodo;

use crate::docx::dibujo::Objeto;
use crate::medidor::Medidor;
use crate::pagina::{Color, Elemento, Figura, Letra, Texto, Tramo, Trazo};

const LETRA: &str = "Cambria Math";

#[derive(Debug, Clone, Default)]
struct Caja {
    ancho: f32,
    asc: f32,
    desc: f32,
    els: Vec<Elemento>,
}

impl Caja {
    fn mover(mut self, dx: f32, dy: f32) -> Vec<Elemento> {
        for e in &mut self.els {
            e.trasladar(dx, dy);
        }
        self.els
    }
}

struct Maquina<'a> {
    medidor: &'a dyn Medidor,
    color: Color,
    /// Formula dentro de un renglon (no de bloque): Word encoge las
    /// fracciones, como el estilo de texto de TeX.
    en_linea: bool,
}

/// Maqueta un `m:oMath` a `tam` puntos.
pub fn formula(n: &Nodo, tam: f32, color: Color, en_linea: bool, medidor: &dyn Medidor) -> Objeto {
    let m = Maquina {
        medidor,
        color,
        en_linea,
    };
    let c = m.fila(n, tam);
    let alto = c.asc.max(tam * 0.7);
    let bajo = c.desc.max(tam * 0.2);
    let ancho = c.ancho;
    Objeto {
        ancho,
        alto,
        bajo,
        elementos: c.mover(0.0, alto),
    }
}

fn es_operador(c: char) -> bool {
    matches!(
        c,
        '=' | '<'
            | '>'
            | '≤'
            | '≥'
            | '≈'
            | '≠'
            | '+'
            | '−'
            | '±'
            | '×'
            | '·'
            | '÷'
            | '→'
            | '⇒'
            | '∈'
            | '≡'
            | '∼'
            | '≅'
            | '∝'
    )
}

impl Maquina<'_> {
    fn letra(&self, tam: f32, cursiva: bool, negrita: bool) -> Letra {
        Letra {
            familia: Arc::from(LETRA),
            tam,
            negrita,
            cursiva,
        }
    }

    /// Un texto suelto.
    fn texto(&self, s: &str, tam: f32, cursiva: bool, negrita: bool) -> Caja {
        self.texto_color(s, tam, cursiva, negrita, self.color)
    }

    fn texto_color(&self, s: &str, tam: f32, cursiva: bool, negrita: bool, color: Color) -> Caja {
        let letra = self.letra(tam, cursiva, negrita);
        let ancho = self.medidor.ancho(s, &letra);
        let m = self.medidor.metricas(&letra);
        let t = Texto {
            x: 0.0,
            y: 0.0,
            letra,
            color,
            texto: s.to_string(),
            subrayado: None,
            tachado: false,
            espaciado: 0.0,
            extra_espacio: 0.0,
            ancho,
            campo: None,
            enlace: None,
        };
        // El ascenso real de las letras (las metricas de Cambria Math son
        // enormes: suben casi un em mas de la cuenta).
        // Lo que baja depende de las letras: sin rabos (g, p, y, parentesis...)
        // casi nada.
        let con_rabo = s.chars().any(|c| {
            matches!(
                c,
                'g' | 'j'
                    | 'p'
                    | 'q'
                    | 'y'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '|'
                    | ','
                    | ';'
                    | 'β'
                    | 'γ'
                    | 'μ'
                    | 'ρ'
                    | 'φ'
                    | 'ψ'
                    | '∫'
                    | '∑'
            )
        });
        let desc = if con_rabo {
            m.descenso.min(tam * 0.22)
        } else {
            tam * 0.05
        };
        Caja {
            ancho,
            asc: m.ascenso.min(tam * 0.75),
            desc,
            els: vec![Elemento::Texto(t)],
        }
    }

    /// Pone cajas una detras de otra.
    fn juntar(&self, cajas: Vec<Caja>) -> Caja {
        let mut c = Caja::default();
        for k in cajas {
            c.asc = c.asc.max(k.asc);
            c.desc = c.desc.max(k.desc);
            let x = c.ancho;
            c.ancho += k.ancho;
            c.els.extend(k.mover(x, 0.0));
        }
        c
    }

    /// Los hijos de un nodo, en fila.
    fn fila(&self, n: &Nodo, tam: f32) -> Caja {
        let cajas = n
            .elementos()
            .filter(|e| !e.nombre.ends_with("Pr"))
            .map(|e| self.elemento(e, tam))
            .collect();
        self.juntar(cajas)
    }

    fn hijo(&self, n: &Nodo, nombre: &str, tam: f32) -> Caja {
        n.hijo(nombre)
            .map(|h| self.fila(h, tam))
            .unwrap_or_default()
    }

    fn propiedad<'n>(n: &'n Nodo, pr: &str, nombre: &str) -> Option<&'n str> {
        n.hijo(pr).and_then(|p| p.hijo(nombre)).map(|x| x.valor())
    }

    fn elemento(&self, n: &Nodo, tam: f32) -> Caja {
        match n.nombre.as_str() {
            "r" => self.corrida(n, tam),
            "f" => self.fraccion(n, tam),
            "sSub" | "sSup" | "sSubSup" | "sPre" => self.indices(n, tam),
            "d" => self.delimitador(n, tam),
            "m" => self.matriz(n, tam),
            "rad" => self.raiz(n, tam),
            "nary" => self.nario(n, tam),
            "func" => {
                let nombre = self.hijo(n, "fName", tam);
                let esp = Caja {
                    ancho: tam * 0.17,
                    ..Default::default()
                };
                let arg = self.hijo(n, "e", tam);
                self.juntar(vec![nombre, esp, arg])
            }
            "acc" => {
                let e = self.hijo(n, "e", tam);
                let chr = Self::propiedad(n, "accPr", "chr").unwrap_or("\u{0302}");
                let chr = match chr {
                    "\u{0302}" => "^",
                    "\u{0303}" => "~",
                    "\u{0307}" => "˙",
                    "\u{0308}" => "¨",
                    "\u{20D7}" => "→",
                    "\u{0305}" | "\u{0304}" => "¯",
                    otro => otro,
                };
                let a = self.texto(chr, tam * 0.8, false, false);
                let x = (e.ancho - a.ancho) / 2.0;
                // El acento (casi siempre una flecha o un gorro, que se
                // dibujan a media altura de la letra) justo encima.
                let asc = e.asc + tam * 0.25;
                let mut c = Caja {
                    ancho: e.ancho,
                    asc,
                    desc: e.desc,
                    els: Vec::new(),
                };
                c.els.extend(a.mover(x, -e.asc + tam * 0.1));
                c.els.extend(e.els);
                c
            }
            "bar" => {
                let e = self.hijo(n, "e", tam);
                let abajo = Self::propiedad(n, "barPr", "pos") == Some("bot");
                let g = tam * 0.05;
                let y = if abajo {
                    e.desc + g * 2.0
                } else {
                    -e.asc - g * 2.0
                };
                let mut c = e.clone();
                c.els.push(Elemento::Linea {
                    desde: (0.0, y),
                    hasta: (e.ancho, y),
                    trazo: Trazo::solido(self.color, g),
                });
                if abajo {
                    c.desc += g * 3.0;
                } else {
                    c.asc += g * 3.0;
                }
                c
            }
            "limLow" | "limUpp" | "groupChr" => {
                let e = self.hijo(n, "e", tam);
                let lim = if n.nombre == "groupChr" {
                    let chr = Self::propiedad(n, "groupChrPr", "chr").unwrap_or("⏟");
                    self.texto(chr, tam, false, false)
                } else {
                    self.hijo(n, "lim", tam * 0.7)
                };
                self.apilar(e, lim, n.nombre == "limUpp", tam)
            }
            "eqArr" => {
                let filas: Vec<Caja> = n
                    .elementos()
                    .filter(|e| e.nombre == "e")
                    .map(|e| self.fila(e, tam))
                    .collect();
                self.columna(filas, tam, false)
            }
            "t" => self.texto(&n.texto(), tam, false, false),
            "box" | "borderBox" | "phant" | "e" | "num" | "den" | "sub" | "sup" | "deg"
            | "fName" | "lim" | "oMath" | "oMathPara" => self.fila(n, tam),
            _ => self.fila(n, tam),
        }
    }

    fn corrida(&self, n: &Nodo, tam: f32) -> Caja {
        let texto: String = n
            .elementos()
            .filter(|e| e.nombre == "t")
            .map(|t| t.texto())
            .collect();
        // `m:rPr` (estilo matematico) y `w:rPr` (color, tamano) se llaman
        // igual: los separa el prefijo.
        let mrpr = n
            .elementos()
            .find(|e| e.nombre == "rPr" && e.prefijo != "w");
        let wrpr = n
            .elementos()
            .find(|e| e.nombre == "rPr" && e.prefijo == "w");
        let color = wrpr
            .and_then(|r| r.hijo("color"))
            .and_then(|c| Color::hex(c.valor()))
            .unwrap_or(self.color);
        let sty = mrpr
            .and_then(|p| p.hijo("sty"))
            .map(|s| s.valor().to_string());
        let normal = mrpr.and_then(|p| p.hijo("nor")).is_some();
        let negrita = matches!(sty.as_deref(), Some("b") | Some("bi"));
        // Por defecto las letras van en cursiva y lo demas derecho.
        let mut cajas = Vec::new();
        let mut actual = String::new();
        let mut cursiva_actual: Option<bool> = None;
        let cursiva_de = |c: char| -> bool {
            match sty.as_deref() {
                Some("p") | Some("b") => false,
                Some("i") | Some("bi") => true,
                _ => !normal && c.is_alphabetic(),
            }
        };
        let volcar = |s: &mut String, cur: Option<bool>, cajas: &mut Vec<Caja>| {
            if !s.is_empty() {
                cajas.push(self.texto_color(s, tam, cur.unwrap_or(false), negrita, color));
                s.clear();
            }
        };
        for (i, c) in texto.chars().enumerate() {
            if es_operador(c) {
                volcar(&mut actual, cursiva_actual, &mut cajas);
                let c = if c == '-' { '−' } else { c };
                let unario = i == 0 && matches!(c, '−' | '+');
                let esp = if unario { 0.0 } else { tam * 0.22 };
                cajas.push(Caja {
                    ancho: esp,
                    ..Default::default()
                });
                cajas.push(self.texto_color(&c.to_string(), tam, false, negrita, color));
                cajas.push(Caja {
                    ancho: esp,
                    ..Default::default()
                });
                cursiva_actual = None;
                continue;
            }
            let cur = cursiva_de(c);
            if cursiva_actual.is_some_and(|x| x != cur) {
                volcar(&mut actual, cursiva_actual, &mut cajas);
            }
            cursiva_actual = Some(cur);
            actual.push(if c == '-' { '−' } else { c });
        }
        volcar(&mut actual, cursiva_actual, &mut cajas);
        self.juntar(cajas)
    }

    fn fraccion(&self, n: &Nodo, tam_fuera: f32) -> Caja {
        let tipo = Self::propiedad(n, "fPr", "type").unwrap_or("bar");
        let tam = if self.en_linea {
            (tam_fuera * 0.75).max(6.0)
        } else {
            tam_fuera
        };
        let num = self.hijo(n, "num", tam);
        let den = self.hijo(n, "den", tam);
        if tipo == "lin" || tipo == "skw" {
            let barra = self.texto("/", tam, false, false);
            return self.juntar(vec![num, barra, den]);
        }
        let g = tam * 0.06;
        let eje = -tam * 0.25;
        let ancho = num.ancho.max(den.ancho) + tam * 0.2;
        let base_num = eje - g * 2.0 - num.desc;
        let base_den = eje + g * 2.0 + den.asc;
        let mut c = Caja {
            ancho,
            asc: -(base_num - num.asc),
            desc: base_den + den.desc,
            els: Vec::new(),
        };
        let (wn, wd) = (num.ancho, den.ancho);
        c.els.extend(num.mover((ancho - wn) / 2.0, base_num));
        c.els.extend(den.mover((ancho - wd) / 2.0, base_den));
        if tipo != "noBar" {
            c.els.push(Elemento::Linea {
                desde: (tam * 0.05, eje),
                hasta: (ancho - tam * 0.05, eje),
                trazo: Trazo::solido(self.color, g),
            });
        }
        c
    }

    fn indices(&self, n: &Nodo, tam: f32) -> Caja {
        let t2 = (tam * 0.7).max(5.0);
        let base = self.hijo(n, "e", tam);
        let sub = n.hijo("sub").map(|s| self.fila(s, t2));
        let sup = n.hijo("sup").map(|s| self.fila(s, t2));
        let ancho_ind = sub
            .as_ref()
            .map(|s| s.ancho)
            .unwrap_or(0.0)
            .max(sup.as_ref().map(|s| s.ancho).unwrap_or(0.0));
        let mut c = Caja {
            asc: base.asc,
            desc: base.desc,
            ..Default::default()
        };
        let previo = n.nombre == "sPre";
        let x_base = if previo { ancho_ind } else { 0.0 };
        let x_ind = if previo { 0.0 } else { base.ancho + tam * 0.03 };
        c.ancho = base.ancho + ancho_ind + tam * 0.05;
        let bajada = (tam * 0.15).max(base.desc + t2 * 0.05);
        let subida = (tam * 0.4).max(base.asc - t2 * 0.45);
        c.els.extend(base.mover(x_base, 0.0));
        if let Some(s) = sub {
            c.desc = c.desc.max(bajada + s.desc);
            c.els.extend(s.mover(x_ind, bajada));
        }
        if let Some(s) = sup {
            c.asc = c.asc.max(subida + s.asc);
            c.els.extend(s.mover(x_ind, -subida));
        }
        c
    }

    /// Un caracter de delimitador estirado al alto `alto`, centrado en el
    /// eje.
    fn delimitador_alto(&self, chr: &str, tam: f32, asc: f32, desc: f32) -> Caja {
        if chr.is_empty() {
            return Caja::default();
        }
        let normal = self.texto(chr, tam, false, false);
        let alto_letra = normal.asc + normal.desc;
        let alto = asc + desc + tam * 0.1;
        if alto <= alto_letra * 1.15 {
            return normal;
        }
        // Se estira en vertical con una transformada.
        let k = alto / alto_letra.max(0.1);
        let y0 = -asc - tam * 0.05;
        let ancho = normal.ancho * (1.0 + (k - 1.0) * 0.15).min(1.6);
        let sx = ancho / normal.ancho.max(0.1);
        let na = normal.asc;
        let els = normal.mover(0.0, na);
        Caja {
            ancho,
            asc: asc + tam * 0.05,
            desc: desc + tam * 0.05,
            els: vec![Elemento::Grupo {
                transformada: [sx, 0.0, 0.0, k, 0.0, y0],
                recorte: None,
                elementos: els,
            }],
        }
    }

    fn delimitador(&self, n: &Nodo, tam: f32) -> Caja {
        let pr = n.hijo("dPr");
        let lee = |k: &str, def: &str| -> String {
            match pr.and_then(|p| p.hijo(k)) {
                Some(x) => x.valor().to_string(),
                None => def.to_string(),
            }
        };
        let (beg, end, sep) = (lee("begChr", "("), lee("endChr", ")"), lee("sepChr", "|"));
        let partes: Vec<Caja> = n
            .elementos()
            .filter(|e| e.nombre == "e")
            .map(|e| self.fila(e, tam))
            .collect();
        let asc = partes.iter().map(|p| p.asc).fold(tam * 0.7, f32::max);
        let desc = partes.iter().map(|p| p.desc).fold(tam * 0.2, f32::max);
        let mut v = vec![self.delimitador_alto(&beg, tam, asc, desc)];
        let n_partes = partes.len();
        for (i, p) in partes.into_iter().enumerate() {
            v.push(p);
            if i + 1 < n_partes {
                v.push(self.delimitador_alto(&sep, tam, asc, desc));
            }
        }
        v.push(self.delimitador_alto(&end, tam, asc, desc));
        self.juntar(v)
    }

    /// Apila cajas en columna (centradas o a la izquierda), centradas en el
    /// eje.
    fn columna(&self, filas: Vec<Caja>, tam: f32, centrar: bool) -> Caja {
        let hueco = tam * 0.25;
        let ancho = filas.iter().map(|f| f.ancho).fold(0.0, f32::max);
        let total: f32 = filas.iter().map(|f| f.asc + f.desc).sum::<f32>()
            + hueco * (filas.len().saturating_sub(1)) as f32;
        let eje = -tam * 0.25;
        let mut y = eje - total / 2.0;
        let mut c = Caja {
            ancho,
            asc: -(eje - total / 2.0),
            desc: eje + total / 2.0,
            els: Vec::new(),
        };
        for f in filas {
            let base = y + f.asc;
            let x = if centrar {
                (ancho - f.ancho) / 2.0
            } else {
                0.0
            };
            y = base + f.desc + hueco;
            c.els.extend(f.mover(x, base));
        }
        c
    }

    fn matriz(&self, n: &Nodo, tam: f32) -> Caja {
        let filas: Vec<Vec<Caja>> = n
            .elementos()
            .filter(|e| e.nombre == "mr")
            .map(|r| {
                r.elementos()
                    .filter(|e| e.nombre == "e")
                    .map(|e| self.fila(e, tam))
                    .collect()
            })
            .collect();
        let columnas = filas.iter().map(|f| f.len()).max().unwrap_or(0);
        let mut anchos = vec![0.0f32; columnas];
        for f in &filas {
            for (k, c) in f.iter().enumerate() {
                anchos[k] = anchos[k].max(c.ancho);
            }
        }
        let separacion = tam * 0.6;
        let mut renglones = Vec::new();
        for f in filas {
            let asc = f.iter().map(|c| c.asc).fold(tam * 0.7, f32::max);
            let desc = f.iter().map(|c| c.desc).fold(tam * 0.2, f32::max);
            let mut r = Caja {
                asc,
                desc,
                ..Default::default()
            };
            let mut x = 0.0;
            for (k, c) in f.into_iter().enumerate() {
                let w = c.ancho;
                r.els.extend(c.mover(x + (anchos[k] - w) / 2.0, 0.0));
                x += anchos[k] + separacion;
            }
            r.ancho = (x - separacion).max(0.0);
            renglones.push(r);
        }
        self.columna(renglones, tam, true)
    }

    fn apilar(&self, e: Caja, otro: Caja, arriba: bool, tam: f32) -> Caja {
        let ancho = e.ancho.max(otro.ancho);
        let g = tam * 0.1;
        let mut c = Caja {
            ancho,
            asc: e.asc,
            desc: e.desc,
            els: Vec::new(),
        };
        let (we, wo) = (e.ancho, otro.ancho);
        let (e_asc, e_desc) = (e.asc, e.desc);
        c.els.extend(e.mover((ancho - we) / 2.0, 0.0));
        if arriba {
            let base = -e_asc - g - otro.desc;
            c.asc = -(base - otro.asc);
            c.els.extend(otro.mover((ancho - wo) / 2.0, base));
        } else {
            let base = e_desc + g + otro.asc;
            c.desc = base + otro.desc;
            c.els.extend(otro.mover((ancho - wo) / 2.0, base));
        }
        c
    }

    fn raiz(&self, n: &Nodo, tam: f32) -> Caja {
        let e = self.hijo(n, "e", tam);
        let oculto = Self::propiedad(n, "radPr", "degHide")
            .is_some_and(|v| !matches!(v, "0" | "off" | "false"));
        let grado = if oculto {
            None
        } else {
            n.hijo("deg")
                .map(|d| self.fila(d, tam * 0.6))
                .filter(|d| d.ancho > 0.0)
        };
        let g = tam * 0.05;
        let arriba = -e.asc - g * 3.0;
        let abajo = e.desc;
        let w_signo = tam * 0.5;
        let x0 = grado
            .as_ref()
            .map(|d| (d.ancho - w_signo * 0.4).max(0.0))
            .unwrap_or(0.0);
        let mut c = Caja {
            ancho: x0 + w_signo + e.ancho + g * 2.0,
            asc: -arriba + g,
            desc: abajo,
            els: Vec::new(),
        };
        let medio = (arriba + abajo) / 2.0 + tam * 0.1;
        let figura = Figura {
            inicio: (x0, medio),
            tramos: vec![
                Tramo::Linea(x0 + w_signo * 0.25, medio - tam * 0.05),
                Tramo::Linea(x0 + w_signo * 0.5, abajo),
                Tramo::Linea(x0 + w_signo, arriba),
                Tramo::Linea(c.ancho, arriba),
            ],
            cerrada: false,
            rellenar: false,
            trazar: true,
            ..Default::default()
        };
        c.els.push(Elemento::Trayecto {
            figuras: vec![figura],
            relleno: None,
            borde: Some(Trazo::solido(self.color, g)),
        });
        if let Some(d) = grado {
            c.els.extend(d.mover(0.0, medio - tam * 0.1));
        }
        c.els.extend(e.mover(x0 + w_signo + g, 0.0));
        c
    }

    fn nario(&self, n: &Nodo, tam: f32) -> Caja {
        let chr = Self::propiedad(n, "naryPr", "chr")
            .unwrap_or("∫")
            .to_string();
        let lim_loc = Self::propiedad(n, "naryPr", "limLoc").map(|s| s.to_string());
        let integral = matches!(chr.as_str(), "∫" | "∬" | "∭" | "∮");
        let debajo = lim_loc
            .as_deref()
            .map(|l| l == "undOvr")
            .unwrap_or(!integral);
        let ocultar_sub =
            Self::propiedad(n, "naryPr", "subHide").is_some_and(|v| !matches!(v, "0" | "off"));
        let ocultar_sup =
            Self::propiedad(n, "naryPr", "supHide").is_some_and(|v| !matches!(v, "0" | "off"));
        let signo = self.texto(&chr, tam * 1.4, false, false);
        let t2 = tam * 0.7;
        let sub = if ocultar_sub {
            None
        } else {
            n.hijo("sub")
                .map(|s| self.fila(s, t2))
                .filter(|s| s.ancho > 0.0)
        };
        let sup = if ocultar_sup {
            None
        } else {
            n.hijo("sup")
                .map(|s| self.fila(s, t2))
                .filter(|s| s.ancho > 0.0)
        };
        let mut op = signo;
        if debajo {
            if let Some(s) = sub {
                op = self.apilar(op, s, false, tam);
            }
            if let Some(s) = sup {
                op = self.apilar(op, s, true, tam);
            }
        } else {
            let mut c = Caja {
                asc: op.asc,
                desc: op.desc,
                ..Default::default()
            };
            let w = op.ancho;
            let (a, d) = (op.asc, op.desc);
            c.els.extend(op.mover(0.0, 0.0));
            let mut extra: f32 = 0.0;
            if let Some(s) = sub {
                c.desc = c.desc.max(d * 0.6 + s.asc + s.desc);
                extra = extra.max(s.ancho);
                c.els.extend(s.mover(w * 0.8, d * 0.6 + tam * 0.2));
            }
            if let Some(s) = sup {
                c.asc = c.asc.max(a);
                extra = extra.max(s.ancho);
                c.els.extend(s.mover(w, -a + tam * 0.3));
            }
            c.ancho = w + extra + tam * 0.05;
            op = c;
        }
        let esp = Caja {
            ancho: tam * 0.15,
            ..Default::default()
        };
        let e = self.hijo(n, "e", tam);
        self.juntar(vec![op, esp, e])
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::medidor::MedidorFijo;
    use pixpin_docs::xml::leer;

    #[test]
    fn una_fraccion_queda_encima_y_debajo_de_la_raya() {
        let n = leer(r#"<m:oMath><m:f><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>bb</m:t></m:r></m:den></m:f></m:oMath>"#).unwrap();
        let o = formula(&n, 10.0, Color::NEGRO, false, &MedidorFijo::default());
        assert!(o.alto > 10.0 && o.bajo > 5.0, "{} {}", o.alto, o.bajo);
        let ys: Vec<f32> = o
            .elementos
            .iter()
            .filter_map(|e| {
                if let Elemento::Texto(t) = e {
                    Some(t.y)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(ys.len(), 2);
        assert!(ys[0] < ys[1], "el numerador va arriba");
        assert!(
            o.elementos
                .iter()
                .any(|e| matches!(e, Elemento::Linea { .. }))
        );
    }

    #[test]
    fn los_indices_suben_y_bajan_y_lo_raro_no_rompe() {
        let n = leer(r#"<m:oMath><m:sSubSup><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sub><m:r><m:t>i</m:t></m:r></m:sub><m:sup><m:r><m:t>2</m:t></m:r></m:sup></m:sSubSup><m:d><m:e><m:m><m:mr><m:e><m:r><m:t>1</m:t></m:r></m:e></m:mr><m:mr><m:e/></m:mr></m:m></m:e></m:d><m:rad><m:e/></m:rad><m:nary><m:e/></m:nary><m:loQueSea/></m:oMath>"#).unwrap();
        let o = formula(&n, 10.0, Color::NEGRO, true, &MedidorFijo::default());
        assert!(o.ancho > 0.0);
    }
}
