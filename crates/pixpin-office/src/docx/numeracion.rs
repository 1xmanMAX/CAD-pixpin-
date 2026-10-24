//! **Las listas de Word** (`numbering.xml`): vinetas y numeros con su
//! formato (1, a, i, A, I...), su texto (`%1.%2.`), su sangria y su letra,
//! y los contadores que van subiendo segun avanza el documento.

use std::collections::HashMap;

use pixpin_docs::xml::Nodo;

use crate::docx::estilos::{PropCorrida, PropParrafo};
use crate::tema::Tema;

#[derive(Debug, Clone, Default)]
pub struct Nivel {
    pub inicio: u32,
    /// `decimal`, `lowerLetter`, `bullet`...
    pub formato: String,
    pub texto: String,
    /// `left`, `center`, `right`.
    pub alineacion: String,
    /// Lo que va tras el numero: `tab`, `space` o `nothing`.
    pub sufijo: String,
    pub ppr: PropParrafo,
    pub rpr: PropCorrida,
    /// Reiniciar tras este nivel (`w:lvlRestart`); `None` = el de siempre.
    pub reinicio: Option<u32>,
    /// `w:isLgl`: los niveles superiores en decimal.
    pub legal: bool,
}

/// Por nivel: el inicio que se fuerza y el nivel que se redefine.
type Sustituciones = HashMap<u32, (Option<u32>, Option<Nivel>)>;

#[derive(Debug, Default)]
pub struct Numeracion {
    /// abstractNumId → niveles.
    abstractos: HashMap<u32, Vec<Nivel>>,
    /// numId → (abstractNumId, sustituciones por nivel).
    numeros: HashMap<u32, (u32, Sustituciones)>,
    /// Contadores por abstracto (Word cuenta por la lista abstracta cuando
    /// varios numId la comparten sin reinicio) y nivel.
    contadores: HashMap<u32, [i64; 9]>,
    /// Que numId uso por ultima vez cada abstracto: si cambia y el nuevo
    /// tiene `startOverride`, se reinicia.
    ultimo_num: HashMap<u32, u32>,
}

impl Numeracion {
    pub fn leer(raiz: Option<&Nodo>, tema: &Tema) -> Numeracion {
        let mut n = Numeracion::default();
        let Some(raiz) = raiz else {
            return n;
        };
        let leer_nivel = |l: &Nodo| {
            let mut v = Nivel {
                inicio: 1,
                sufijo: "tab".into(),
                alineacion: "left".into(),
                ..Default::default()
            };
            for e in l.elementos() {
                match e.nombre.as_str() {
                    "start" => v.inicio = e.valor().parse().unwrap_or(1),
                    "numFmt" => v.formato = e.valor().to_string(),
                    "lvlText" => v.texto = e.valor().to_string(),
                    "lvlJc" => v.alineacion = e.valor().to_string(),
                    "suff" => v.sufijo = e.valor().to_string(),
                    "pPr" => v.ppr = PropParrafo::leer(e, tema),
                    "rPr" => v.rpr = PropCorrida::leer(e, tema),
                    "lvlRestart" => v.reinicio = e.valor().parse().ok(),
                    "isLgl" => v.legal = crate::paquete::interruptor(e),
                    _ => {}
                }
            }
            v
        };
        for a in raiz.elementos().filter(|e| e.nombre == "abstractNum") {
            let id: u32 = a.atributo("abstractNumId").parse().unwrap_or(0);
            let mut niveles = vec![Nivel::default(); 9];
            for l in a.elementos().filter(|e| e.nombre == "lvl") {
                let i: usize = l.atributo("ilvl").parse().unwrap_or(0);
                if i < 9 {
                    niveles[i] = leer_nivel(l);
                }
            }
            n.abstractos.insert(id, niveles);
        }
        // Un abstracto que remite a un estilo de numeracion
        // (`numStyleLink`) se queda con los niveles del que lo define.
        for num in raiz.elementos().filter(|e| e.nombre == "num") {
            let id: u32 = num.atributo("numId").parse().unwrap_or(0);
            let abs: u32 = num
                .hijo("abstractNumId")
                .and_then(|a| a.valor().parse().ok())
                .unwrap_or(0);
            let mut sust = HashMap::new();
            for o in num.elementos().filter(|e| e.nombre == "lvlOverride") {
                let i: u32 = o.atributo("ilvl").parse().unwrap_or(0);
                let inicio = o.hijo("startOverride").and_then(|s| s.valor().parse().ok());
                let nivel = o.hijo("lvl").map(leer_nivel);
                sust.insert(i, (inicio, nivel));
            }
            n.numeros.insert(id, (abs, sust));
        }
        n
    }

    /// El nivel `ilvl` de la lista `num_id`, si existe.
    pub fn nivel(&self, num_id: u32, ilvl: u32) -> Option<Nivel> {
        let (abs, sust) = self.numeros.get(&num_id)?;
        if let Some((_, Some(l))) = sust.get(&ilvl) {
            return Some(l.clone());
        }
        self.abstractos.get(abs)?.get(ilvl as usize).cloned()
    }

    /// Cuenta un parrafo numerado y devuelve el texto de su numero.
    #[allow(clippy::needless_range_loop)] // los contadores de cada nivel se tocan por indice
    pub fn siguiente(&mut self, num_id: u32, ilvl: u32) -> Option<(String, Nivel)> {
        let ilvl = ilvl.min(8);
        let (abs, sust) = self.numeros.get(&num_id)?.clone();
        let niveles: Vec<Nivel> = (0..9)
            .map(|i| self.nivel(num_id, i).unwrap_or_default())
            .collect();
        let nivel = niveles[ilvl as usize].clone();
        let cambio = self
            .ultimo_num
            .insert(abs, num_id)
            .is_some_and(|u| u != num_id);
        let contadores = self.contadores.entry(abs).or_insert([i64::MIN; 9]);
        if cambio {
            // Otro numId sobre la misma lista: si trae inicio propio, empieza.
            for (i, (inicio, _)) in &sust {
                if let Some(v) = inicio {
                    for k in *i as usize..9 {
                        contadores[k] = i64::MIN;
                    }
                    let _ = v;
                }
            }
        }
        let inicio_de = |i: usize| -> i64 {
            sust.get(&(i as u32))
                .and_then(|(s, _)| *s)
                .map(|v| v as i64)
                .unwrap_or(niveles[i].inicio as i64)
        };
        let i = ilvl as usize;
        contadores[i] = if contadores[i] == i64::MIN {
            inicio_de(i)
        } else {
            contadores[i] + 1
        };
        // Los niveles inferiores vuelven a empezar.
        for k in i + 1..9 {
            let reinicia = niveles[k]
                .reinicio
                .map(|r| r as usize > i || r == 0)
                .unwrap_or(true);
            if reinicia {
                contadores[k] = i64::MIN;
            }
        }
        // Los superiores que no hayan salido aun valen su inicio.
        let valores: Vec<i64> = (0..9)
            .map(|k| {
                if contadores[k] == i64::MIN {
                    inicio_de(k)
                } else {
                    contadores[k]
                }
            })
            .collect();
        if nivel.formato == "none" {
            return Some((String::new(), nivel));
        }
        let mut texto = String::new();
        let cs: Vec<char> = nivel.texto.chars().collect();
        let mut j = 0;
        while j < cs.len() {
            if cs[j] == '%' && j + 1 < cs.len() && cs[j + 1].is_ascii_digit() {
                let k = cs[j + 1].to_digit(10).unwrap_or(1) as usize;
                if (1..=9).contains(&k) {
                    let fmt = if nivel.legal && k - 1 < i {
                        "decimal"
                    } else {
                        niveles[k - 1].formato.as_str()
                    };
                    texto.push_str(&formatear(valores[k - 1], fmt));
                }
                j += 2;
                continue;
            }
            texto.push(cs[j]);
            j += 1;
        }
        Some((texto, nivel))
    }
}

/// Un numero en el formato de Word.
pub fn formatear(v: i64, formato: &str) -> String {
    let v = v.max(0);
    match formato {
        "lowerLetter" => letras(v, false),
        "upperLetter" => letras(v, true),
        "lowerRoman" => romano(v).to_lowercase(),
        "upperRoman" => romano(v),
        "decimalZero" => format!("{v:02}"),
        "ordinal" => format!("{v}."),
        "bullet" => String::new(),
        "none" => String::new(),
        _ => v.to_string(),
    }
}

fn letras(v: i64, mayus: bool) -> String {
    if v <= 0 {
        return String::new();
    }
    // Word repite la letra: a..z, aa..zz, aaa...
    let base = if mayus { b'A' } else { b'a' };
    let c = (base + ((v - 1) % 26) as u8) as char;
    std::iter::repeat_n(c, ((v - 1) / 26 + 1) as usize).collect()
}

fn romano(mut v: i64) -> String {
    if v <= 0 || v > 39999 {
        return v.to_string();
    }
    let tabla = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut s = String::new();
    for (n, t) in tabla {
        while v >= n {
            s.push_str(t);
            v -= n;
        }
    }
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_docs::xml::leer;

    #[test]
    fn los_formatos_de_numero() {
        assert_eq!(formatear(3, "lowerLetter"), "c");
        assert_eq!(formatear(28, "upperLetter"), "BB");
        assert_eq!(formatear(14, "lowerRoman"), "xiv");
        assert_eq!(formatear(7, "decimal"), "7");
    }

    #[test]
    fn la_lista_cuenta_y_reinicia_los_niveles_de_abajo() {
        let x = leer(
            r#"<w:numbering>
            <w:abstractNum w:abstractNumId="0">
              <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl>
              <w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%1.%2)"/></w:lvl>
            </w:abstractNum>
            <w:num w:numId="5"><w:abstractNumId w:val="0"/></w:num>
            </w:numbering>"#,
        )
        .unwrap();
        let mut n = Numeracion::leer(Some(&x), &Tema::default());
        let t: Vec<String> = [(0, 0), (1, 0), (1, 0), (0, 0), (1, 0)]
            .iter()
            .map(|&(l, _)| n.siguiente(5, l).unwrap().0)
            .collect();
        assert_eq!(t, vec!["1.", "1.a)", "1.b)", "2.", "2.a)"]);
        assert!(
            n.siguiente(99, 0).is_none(),
            "una lista que no existe no numera"
        );
    }
}
