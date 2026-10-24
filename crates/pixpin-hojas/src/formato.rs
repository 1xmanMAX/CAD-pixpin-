//! Formatos de numero de Excel: de `#,##0.00;[Red]-#,##0.00` y un valor al
//! texto que se ve en la celda.
//!
//! Escrito desde la descripcion de ECMA-376 (parte 1, 18.8.31) y de como se
//! comporta Excel a la vista: secciones (`positivo;negativo;cero;texto`),
//! condiciones (`[>100]`), colores (`[Red]`, `[Color10]`), monedas y locales
//! (`[$S/-280A]`, `[$-409]`), relleno (`*`) y espacio de ancho (`_`),
//! porcentaje, escala por miles, cientifico, fracciones, fechas y horas
//! (con el dia 60 falso de 1900 y el sistema 1904) y tiempo transcurrido
//! (`[h]:mm`). Nunca evalua formulas: formatea el valor ya guardado.

use std::sync::Arc;

/// Los separadores y nombres del sistema. El `.` y la `,` de un formato son
/// «el decimal» y «los miles»: se pintan con los de aqui.
#[derive(Debug, Clone, PartialEq)]
pub struct Regional {
    pub decimal: char,
    pub miles: char,
    /// El formato con el que se pinta el formato integrado 14 (la fecha
    /// corta del sistema), ya en codigo de Excel (`dd/mm/yyyy`).
    pub fecha_corta: String,
    pub meses: [String; 12],
    pub meses_cortos: [String; 12],
    pub dias: [String; 7],
    pub dias_cortos: [String; 7],
    pub am: String,
    pub pm: String,
    /// Los nombres de VERDADERO/FALSO y de los errores en espanol (como los
    /// pinta un Excel en espanol) en vez de en ingles.
    pub espanol: bool,
    /// Lo que pinta la `/` sin comillas de un formato de fecha (Excel usa
    /// el separador de fechas de Windows, que el usuario puede cambiar).
    pub separador_fecha: char,
}

fn arr<const N: usize>(s: &str) -> [String; N] {
    let v: Vec<String> = s.split(',').map(str::to_string).collect();
    std::array::from_fn(|i| v.get(i).cloned().unwrap_or_default())
}

impl Regional {
    pub fn en_us() -> Self {
        Self {
            decimal: '.',
            miles: ',',
            fecha_corta: "m/d/yyyy".into(),
            meses: arr(
                "January,February,March,April,May,June,July,August,September,October,November,December",
            ),
            meses_cortos: arr("Jan,Feb,Mar,Apr,May,Jun,Jul,Aug,Sep,Oct,Nov,Dec"),
            dias: arr("Sunday,Monday,Tuesday,Wednesday,Thursday,Friday,Saturday"),
            dias_cortos: arr("Sun,Mon,Tue,Wed,Thu,Fri,Sat"),
            am: "AM".into(),
            pm: "PM".into(),
            espanol: false,
            separador_fecha: '/',
        }
    }

    /// Espanol del Peru, que es el del equipo del autor: punto decimal y coma
    /// de miles (como Mexico), fecha corta dia/mes/ano.
    pub fn es_pe() -> Self {
        Self {
            decimal: '.',
            miles: ',',
            fecha_corta: "dd/mm/yyyy".into(),
            // Asi los pinta Excel en espanol (medido): meses con mayuscula y
            // dias en minuscula.
            meses: arr(
                "Enero,Febrero,Marzo,Abril,Mayo,Junio,Julio,Agosto,Setiembre,Octubre,Noviembre,Diciembre",
            ),
            meses_cortos: arr("Ene,Feb,Mar,Abr,May,Jun,Jul,Ago,Set,Oct,Nov,Dic"),
            dias: arr("domingo,lunes,martes,miércoles,jueves,viernes,sábado"),
            dias_cortos: arr("dom,lun,mar,mié,jue,vie,sáb"),
            am: "a. m.".into(),
            pm: "p. m.".into(),
            espanol: true,
            separador_fecha: '/',
        }
    }
}

impl Default for Regional {
    fn default() -> Self {
        Self::es_pe()
    }
}

/// Lo que se ve en la celda.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mostrado {
    pub texto: String,
    /// Color forzado por el formato (`[Red]`), RGB.
    pub color: Option<[u8; 3]>,
    /// Donde va el relleno `*x` (byte del texto): lo de antes se alinea a la
    /// izquierda y lo de despues a la derecha, como el formato contable.
    pub relleno: Option<usize>,
}

/// El codigo de un formato integrado (los ids que no traen `<numFmt>`).
pub fn integrado(id: u32) -> Option<&'static str> {
    Some(match id {
        0 => "General",
        1 => "0",
        2 => "0.00",
        3 => "#,##0",
        4 => "#,##0.00",
        5 => "\"$\"#,##0_);(\"$\"#,##0)",
        6 => "\"$\"#,##0_);[Red](\"$\"#,##0)",
        7 => "\"$\"#,##0.00_);(\"$\"#,##0.00)",
        8 => "\"$\"#,##0.00_);[Red](\"$\"#,##0.00)",
        9 => "0%",
        10 => "0.00%",
        11 => "0.00E+00",
        12 => "# ?/?",
        13 => "# ??/??",
        // 14 es la fecha corta del sistema: la pone `Regional`.
        14 => "\u{1}",
        // 15 a 17: el guion es el separador de fechas del sistema si no es
        // la barra (lo que hace Excel con un separador propio).
        15 => "d\u{3}mmm\u{3}yy",
        16 => "d\u{3}mmm",
        17 => "mmm\u{3}yy",
        18 => "h:mm AM/PM",
        19 => "h:mm:ss AM/PM",
        20 => "h:mm",
        21 => "h:mm:ss",
        22 => "\u{2}",
        27..=36 | 50..=58 => "\u{1}",
        37 => "#,##0_);(#,##0)",
        38 => "#,##0_);[Red](#,##0)",
        39 => "#,##0.00_);(#,##0.00)",
        40 => "#,##0.00_);[Red](#,##0.00)",
        41 => "_(* #,##0_);_(* (#,##0);_(* \"-\"_);_(@_)",
        42 => "_(\"$\"* #,##0_);_(\"$\"* (#,##0);_(\"$\"* \"-\"_);_(@_)",
        43 => "_(* #,##0.00_);_(* (#,##0.00);_(* \"-\"??_);_(@_)",
        44 => "_(\"$\"* #,##0.00_);_(\"$\"* (#,##0.00);_(\"$\"* \"-\"??_);_(@_)",
        45 => "mm:ss",
        46 => "[h]:mm:ss",
        47 => "mm:ss.0",
        48 => "##0.0E+0",
        49 => "@",
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Mayor,
    MayorIgual,
    Menor,
    MenorIgual,
    Igual,
    Distinto,
}

#[derive(Debug, Clone, PartialEq)]
enum Ficha {
    Literal(String),
    /// `0`, `#` o `?`.
    Digito(u8),
    Punto,
    Coma,
    Porcentaje,
    /// `E+` (true) o `E-` (false).
    Exponente(bool),
    Barra,
    Arroba,
    General,
    Relleno(char),
    Hueco(char),
    Ano(u8),
    Mes(u8),
    Dia(u8),
    Hora(u8),
    Minuto(u8),
    Segundo(u8),
    /// Decimas de segundo: cuantos digitos.
    FraccionSegundo(u8),
    /// `AM/PM` (0) o `A/P` (1).
    AmPm(u8),
    /// `[h]`, `[m]`, `[s]`: 'h', 'm' o 's' y cuantas letras.
    Transcurrido(char, u8),
    /// La `/` de una fecha: el separador de fechas del sistema.
    SeparadorFecha,
    /// El guion de los formatos integrados 15 a 17.
    SeparadorGuion,
}

#[derive(Debug, Clone, PartialEq)]
struct Seccion {
    fichas: Vec<Ficha>,
    color: Option<[u8; 3]>,
    condicion: Option<(Op, f64)>,
    es_fecha: bool,
    ingles: bool,
}

/// Un formato ya troceado, listo para aplicar muchas veces.
#[derive(Debug, Clone, PartialEq)]
pub struct Formato {
    secciones: Vec<Seccion>,
    pub es_fecha: bool,
    pub es_texto: bool,
    pub es_general: bool,
}

const COLORES: [(&str, [u8; 3]); 8] = [
    ("black", [0, 0, 0]),
    ("blue", [0, 0, 255]),
    ("cyan", [0, 255, 255]),
    ("green", [0, 255, 0]),
    ("magenta", [255, 0, 255]),
    ("red", [255, 0, 0]),
    ("white", [255, 255, 255]),
    ("yellow", [255, 255, 0]),
];

impl Formato {
    /// Trocea un codigo de formato. `regional` decide el formato 14.
    pub fn nuevo(codigo: &str, regional: &Regional) -> Formato {
        let codigo = match codigo {
            "\u{1}" => regional.fecha_corta.clone(),
            "\u{2}" => format!("{} h:mm", regional.fecha_corta),
            c => c.to_string(),
        };
        let mut secciones = Vec::new();
        for trozo in partir_secciones(&codigo).into_iter().take(4) {
            secciones.push(seccion(&trozo));
        }
        if secciones.is_empty() {
            secciones.push(seccion("General"));
        }
        let es_general = secciones.len() == 1 && secciones[0].fichas == [Ficha::General];
        let es_fecha = secciones[0].es_fecha;
        let es_texto = secciones.len() == 1 && secciones[0].fichas.contains(&Ficha::Arroba);
        Formato {
            secciones,
            es_fecha,
            es_texto,
            es_general,
        }
    }

    pub fn general() -> Formato {
        Formato {
            secciones: vec![seccion("General")],
            es_fecha: false,
            es_texto: false,
            es_general: true,
        }
    }

    /// Formatea un numero.
    pub fn numero(&self, v: f64, fecha_1904: bool, r: &Regional) -> Mostrado {
        if !v.is_finite() {
            return Mostrado {
                texto: "#NUM!".into(),
                ..Default::default()
            };
        }
        let (sec, abs) = self.elegir(v);
        let Some(s) = sec else {
            return Mostrado {
                texto: general(v, r),
                ..Default::default()
            };
        };
        let valor = if abs { v.abs() } else { v };
        let mut m = if s.es_fecha {
            fecha(s, valor, fecha_1904, r)
        } else {
            numerica(s, valor, r)
        };
        m.color = s.color;
        m
    }

    /// Formatea un texto con la seccion de texto, si la hay.
    pub fn texto(&self, t: &str) -> Mostrado {
        let s = if self.secciones.len() >= 4 {
            Some(&self.secciones[3])
        } else {
            self.secciones
                .iter()
                .find(|s| s.fichas.contains(&Ficha::Arroba))
        };
        let Some(s) = s else {
            return Mostrado {
                texto: t.to_string(),
                ..Default::default()
            };
        };
        let mut m = Mostrado {
            color: s.color,
            ..Default::default()
        };
        for f in &s.fichas {
            match f {
                Ficha::Arroba => m.texto.push_str(t),
                Ficha::Literal(l) => m.texto.push_str(l),
                Ficha::Hueco(_) => m.texto.push(' '),
                Ficha::Relleno(_) => m.relleno = Some(m.texto.len()),
                _ => {}
            }
        }
        m
    }

    /// La seccion que toca a `v` y si se formatea su valor absoluto (la
    /// seccion de negativos ya trae su propio signo).
    fn elegir(&self, v: f64) -> (Option<&Seccion>, bool) {
        let num: Vec<&Seccion> = self
            .secciones
            .iter()
            .take(3)
            .filter(|s| !es_solo_texto(s))
            .collect();
        if num.is_empty() {
            return (None, false);
        }
        if num.iter().any(|s| s.condicion.is_some()) {
            // Condiciones: la primera que se cumpla; si ninguna, la ultima
            // sin condicion (o la ultima a secas).
            for (i, s) in num.iter().enumerate() {
                if let Some((op, n)) = s.condicion {
                    let ok = match op {
                        Op::Mayor => v > n,
                        Op::MayorIgual => v >= n,
                        Op::Menor => v < n,
                        Op::MenorIgual => v <= n,
                        Op::Igual => v == n,
                        Op::Distinto => v != n,
                    };
                    if ok {
                        // La seccion de un «menor que» negativo lleva su signo.
                        return (Some(s), i > 0 && v < 0.0);
                    }
                }
            }
            let resto = num.iter().find(|s| s.condicion.is_none()).or(num.last());
            return (resto.copied(), v < 0.0 && num.len() > 1);
        }
        match num.len() {
            1 => (Some(num[0]), false),
            2 => {
                if v < 0.0 {
                    (Some(num[1]), true)
                } else {
                    (Some(num[0]), false)
                }
            }
            _ => {
                if v > 0.0 {
                    (Some(num[0]), false)
                } else if v < 0.0 {
                    (Some(num[1]), true)
                } else {
                    (Some(num[2]), false)
                }
            }
        }
    }
}

fn es_solo_texto(s: &Seccion) -> bool {
    s.fichas.contains(&Ficha::Arroba)
}

/// Parte el codigo por `;` que no esten entre comillas, corchetes o tras `\`.
fn partir_secciones(c: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut act = String::new();
    let mut comillas = false;
    let mut corchete = false;
    let mut chars = c.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if !corchete => {
                comillas = !comillas;
                act.push(ch);
            }
            '\\' | '_' | '*' if !comillas && !corchete => {
                act.push(ch);
                if let Some(n) = chars.next() {
                    act.push(n);
                }
            }
            '[' if !comillas => {
                corchete = true;
                act.push(ch);
            }
            ']' if !comillas => {
                corchete = false;
                act.push(ch);
            }
            ';' if !comillas && !corchete => v.push(std::mem::take(&mut act)),
            _ => act.push(ch),
        }
    }
    v.push(act);
    v
}

fn seccion(codigo: &str) -> Seccion {
    let mut s = Seccion {
        fichas: Vec::new(),
        color: None,
        condicion: None,
        es_fecha: false,
        ingles: false,
    };
    let ch: Vec<char> = codigo.chars().collect();
    // ¿Es de fecha? Letras de fecha fuera de comillas y corchetes de color.
    let mut i = 0;
    let mut fecha = false;
    while i < ch.len() {
        match ch[i] {
            '"' => {
                i += 1;
                while i < ch.len() && ch[i] != '"' {
                    i += 1;
                }
            }
            '\\' | '_' | '*' => i += 1,
            '[' => {
                let ini = i;
                while i < ch.len() && ch[i] != ']' {
                    i += 1;
                }
                let dentro: String = ch[ini + 1..i.min(ch.len())]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase();
                if !dentro.is_empty() && dentro.chars().all(|c| matches!(c, 'h' | 'm' | 's')) {
                    fecha = true;
                }
            }
            'y' | 'Y' | 'd' | 'D' | 'h' | 'H' | 's' | 'S' | 'm' | 'M' => fecha = true,
            'G' | 'g'
                if ch[i..]
                    .iter()
                    .take(7)
                    .collect::<String>()
                    .eq_ignore_ascii_case("general") =>
            {
                i += 6
            }
            'e' | 'E' if i + 1 < ch.len() && (ch[i + 1] == '+' || ch[i + 1] == '-') => {}
            _ => {}
        }
        i += 1;
    }
    // «0.00 "m"» es numero: la m va entre comillas.
    s.es_fecha = fecha;
    let mut i = 0;
    let mut lit = String::new();
    let soltar = |lit: &mut String, f: &mut Vec<Ficha>| {
        if !lit.is_empty() {
            f.push(Ficha::Literal(std::mem::take(lit)));
        }
    };
    while i < ch.len() {
        let c = ch[i];
        match c {
            '"' => {
                i += 1;
                while i < ch.len() && ch[i] != '"' {
                    lit.push(ch[i]);
                    i += 1;
                }
            }
            '\\' => {
                if i + 1 < ch.len() {
                    lit.push(ch[i + 1]);
                    i += 1;
                }
            }
            '_' => {
                soltar(&mut lit, &mut s.fichas);
                if i + 1 < ch.len() {
                    s.fichas.push(Ficha::Hueco(ch[i + 1]));
                    i += 1;
                }
            }
            '*' => {
                soltar(&mut lit, &mut s.fichas);
                if i + 1 < ch.len() {
                    s.fichas.push(Ficha::Relleno(ch[i + 1]));
                    i += 1;
                }
            }
            '[' => {
                let ini = i + 1;
                while i < ch.len() && ch[i] != ']' {
                    i += 1;
                }
                let dentro: String = ch[ini.min(ch.len())..i.min(ch.len())].iter().collect();
                let bajo = dentro.to_ascii_lowercase();
                if let Some((_, rgb)) = COLORES.iter().find(|(n, _)| *n == bajo) {
                    s.color = Some(*rgb);
                } else if let Some(n) = bajo.strip_prefix("color") {
                    if let Ok(n) = n.trim().parse::<usize>() {
                        if (1..=56).contains(&n) {
                            let c = crate::estilos::PALETA[n + 7];
                            s.color = Some([(c >> 16) as u8, (c >> 8) as u8, c as u8]);
                        }
                    }
                } else if bajo.starts_with('$') {
                    // [$S/-280A]: el simbolo va antes del guion; el resto es
                    // el locale. [$-409] es solo locale (ingles).
                    let cuerpo = &dentro[1..];
                    let (simbolo, locale) = match cuerpo.find('-') {
                        Some(p) => (&cuerpo[..p], &cuerpo[p + 1..]),
                        None => (cuerpo, ""),
                    };
                    lit.push_str(simbolo);
                    let l = locale.to_ascii_lowercase();
                    if l.ends_with("409") || l.starts_with("en") {
                        s.ingles = true;
                    }
                } else if !bajo.is_empty() && bajo.chars().all(|c| matches!(c, 'h' | 'm' | 's')) {
                    soltar(&mut lit, &mut s.fichas);
                    let letra = bajo.chars().next().unwrap_or('h');
                    s.fichas
                        .push(Ficha::Transcurrido(letra, bajo.len().min(255) as u8));
                } else if let Some(cond) = condicion(&dentro) {
                    s.condicion = Some(cond);
                }
            }
            '0' | '#' | '?' if !s.es_fecha || (c == '0' && ultima_es_segundo(&s.fichas, &lit)) => {
                if s.es_fecha {
                    // `ss.00`: decimas de segundo.
                    let mut n = 0u8;
                    while i < ch.len() && ch[i] == '0' {
                        n = n.saturating_add(1);
                        i += 1;
                    }
                    if lit.ends_with('.') {
                        lit.pop();
                    }
                    soltar(&mut lit, &mut s.fichas);
                    s.fichas.push(Ficha::FraccionSegundo(n));
                    continue;
                }
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::Digito(c as u8));
            }
            '.' if !s.es_fecha => {
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::Punto);
            }
            ',' if !s.es_fecha => {
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::Coma);
            }
            '%' => {
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::Porcentaje);
            }
            '/' if !s.es_fecha => {
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::Barra);
            }
            '@' => {
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::Arroba);
            }
            'E' | 'e'
                if !s.es_fecha && i + 1 < ch.len() && (ch[i + 1] == '+' || ch[i + 1] == '-') =>
            {
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::Exponente(ch[i + 1] == '+'));
                i += 1;
            }
            'G' | 'g'
                if ch[i..]
                    .iter()
                    .take(7)
                    .collect::<String>()
                    .eq_ignore_ascii_case("general") =>
            {
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::General);
                i += 6;
            }
            'A' | 'a'
                if s.es_fecha
                    && ch[i..]
                        .iter()
                        .take(5)
                        .collect::<String>()
                        .eq_ignore_ascii_case("am/pm") =>
            {
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::AmPm(0));
                i += 4;
            }
            'A' | 'a'
                if s.es_fecha
                    && ch[i..]
                        .iter()
                        .take(3)
                        .collect::<String>()
                        .eq_ignore_ascii_case("a/p") =>
            {
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::AmPm(1));
                i += 2;
            }
            'y' | 'Y' | 'e' | 'm' | 'M' | 'd' | 'D' | 'h' | 'H' | 's' | 'S' if s.es_fecha => {
                soltar(&mut lit, &mut s.fichas);
                let base = c.to_ascii_lowercase();
                let mut n = 0u8;
                while i < ch.len() && ch[i].to_ascii_lowercase() == base {
                    n = n.saturating_add(1);
                    i += 1;
                }
                s.fichas.push(match base {
                    'y' | 'e' => Ficha::Ano(if base == 'e' { 4 } else { n }),
                    'm' => Ficha::Mes(n),
                    'd' => Ficha::Dia(n),
                    'h' => Ficha::Hora(n),
                    _ => Ficha::Segundo(n),
                });
                continue;
            }
            'B' | 'b'
                if s.es_fecha && i + 1 < ch.len() && (ch[i + 1] == '1' || ch[i + 1] == '2') =>
            {
                // Calendario (b1/b2): se ignora.
                i += 1;
            }
            '\u{3}' => {
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::SeparadorGuion);
            }
            '/' if s.es_fecha => {
                soltar(&mut lit, &mut s.fichas);
                s.fichas.push(Ficha::SeparadorFecha);
            }
            _ => lit.push(c),
        }
        i += 1;
    }
    soltar(&mut lit, &mut s.fichas);
    if s.es_fecha {
        // `m` es minuto si va justo despues de una hora o justo antes de
        // unos segundos (mirando solo las fichas de fecha).
        let idx: Vec<usize> = (0..s.fichas.len())
            .filter(|&k| {
                !matches!(
                    s.fichas[k],
                    Ficha::Literal(_)
                        | Ficha::Hueco(_)
                        | Ficha::SeparadorFecha
                        | Ficha::SeparadorGuion
                )
            })
            .collect();
        for (p, &k) in idx.iter().enumerate() {
            if let Ficha::Mes(n) = s.fichas[k] {
                if n > 2 {
                    continue;
                }
                let antes = p.checked_sub(1).map(|q| &s.fichas[idx[q]]);
                let despues = idx.get(p + 1).map(|&q| &s.fichas[q]);
                if matches!(
                    antes,
                    Some(Ficha::Hora(_)) | Some(Ficha::Transcurrido('h', _))
                ) || matches!(
                    despues,
                    Some(Ficha::Segundo(_)) | Some(Ficha::Transcurrido('s', _))
                ) {
                    s.fichas[k] = Ficha::Minuto(n);
                }
            }
        }
    }
    s
}

fn ultima_es_segundo(f: &[Ficha], lit: &str) -> bool {
    lit == "."
        && matches!(
            f.last(),
            Some(Ficha::Segundo(_)) | Some(Ficha::Transcurrido('s', _))
        )
}

fn condicion(s: &str) -> Option<(Op, f64)> {
    let s = s.trim();
    let (op, resto) = if let Some(r) = s.strip_prefix(">=") {
        (Op::MayorIgual, r)
    } else if let Some(r) = s.strip_prefix("<=") {
        (Op::MenorIgual, r)
    } else if let Some(r) = s.strip_prefix("<>") {
        (Op::Distinto, r)
    } else if let Some(r) = s.strip_prefix('>') {
        (Op::Mayor, r)
    } else if let Some(r) = s.strip_prefix('<') {
        (Op::Menor, r)
    } else {
        (Op::Igual, s.strip_prefix('=')?)
    };
    Some((op, resto.trim().parse().ok()?))
}

/// Los digitos de `v` (>= 0) redondeado a `dec` decimales como lo hace Excel:
/// primero a 15 cifras significativas y despues mitad hacia arriba sobre el
/// decimal (2,675 con dos decimales da 2,68, no 2,67).
pub fn digitos(v: f64, dec: usize) -> (String, String) {
    let v = v.abs();
    if v == 0.0 {
        return ("0".into(), "0".repeat(dec));
    }
    let e = format!("{v:.14e}");
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let cifras: Vec<u8> = mant
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect();
    // valor = 0.cifras * 10^(exp+1)
    let punto = exp + 1; // cuantas cifras van antes del punto
    // Cifras hasta `dec` decimales.
    let corte = punto + dec as i32;
    let mut d: Vec<u8> = if corte <= 0 {
        Vec::new()
    } else {
        (0..corte as usize)
            .map(|k| cifras.get(k).copied().unwrap_or(0))
            .collect()
    };
    let siguiente = if corte < 0 {
        0
    } else {
        cifras.get(corte as usize).copied().unwrap_or(0)
    };
    // `d` es el entero round(v * 10^dec), cifra a cifra.
    if siguiente >= 5 {
        let mut k = d.len();
        loop {
            if k == 0 {
                d.insert(0, 1);
                break;
            }
            k -= 1;
            if d[k] == 9 {
                d[k] = 0;
            } else {
                d[k] += 1;
                break;
            }
        }
    }
    while d.len() < dec + 1 {
        d.insert(0, 0);
    }
    let (ent, frac) = d.split_at(d.len() - dec);
    let mut ent: String = ent.iter().map(|c| (b'0' + c) as char).collect();
    let frac: String = frac.iter().map(|c| (b'0' + c) as char).collect();
    let t = ent.trim_start_matches('0');
    ent = if t.is_empty() { "0".into() } else { t.into() };
    (ent, frac)
}

/// El formato General: hasta 11 caracteres (como en una columna normal) y
/// cientifico para lo muy grande o muy pequeno.
pub fn general(v: f64, r: &Regional) -> String {
    general_en(v, r, 11)
}

/// General que quepa en `max` caracteres (para columnas estrechas).
pub fn general_en(v: f64, r: &Regional, max: usize) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let neg = v < 0.0;
    let a = v.abs();
    let signo = if neg { "-" } else { "" };
    let cientifico = |dec: usize| -> String {
        let mut exp = a.log10().floor() as i32;
        let mut m = a / 10f64.powi(exp);
        let (mut e, mut f) = digitos(m, dec);
        if e.len() > 1 {
            exp += 1;
            m = a / 10f64.powi(exp);
            (e, f) = digitos(m, dec);
        }
        let f = f.trim_end_matches('0');
        let mant = if f.is_empty() {
            e
        } else {
            format!("{e}{}{f}", r.decimal)
        };
        format!(
            "{signo}{mant}E{}{:02}",
            if exp < 0 { '-' } else { '+' },
            exp.abs()
        )
    };
    let max = max.max(5);
    if a >= 1e11_f64.min(10f64.powi(max as i32 - neg as i32)) || a < 1e-9 {
        // Mantisa: lo que quepa con «E+00».
        let dec = max.saturating_sub(6 + neg as usize).min(5);
        return cientifico(dec);
    }
    let ent_len = digitos(a, 0).0.len();
    let disp = max.saturating_sub(ent_len + neg as usize + 1);
    let (e, f) = digitos(a, disp);
    let f = f.trim_end_matches('0');
    if e == "0" && f.is_empty() {
        // Tan pequeno que no se ve a esta anchura: cientifico.
        return cientifico(max.saturating_sub(6 + neg as usize).min(5));
    }
    if f.is_empty() {
        format!("{signo}{e}")
    } else {
        format!("{signo}{e}{}{f}", r.decimal)
    }
}

fn numerica(s: &Seccion, v: f64, r: &Regional) -> Mostrado {
    let f = &s.fichas;
    let mut m = Mostrado::default();
    if f.contains(&Ficha::General) {
        for x in f {
            match x {
                Ficha::General => m.texto.push_str(&general(v, r)),
                Ficha::Literal(l) => m.texto.push_str(l),
                Ficha::Hueco(_) => m.texto.push(' '),
                Ficha::Relleno(_) => m.relleno = Some(m.texto.len()),
                _ => {}
            }
        }
        return m;
    }
    let negativo = v < 0.0;
    let mut v = v.abs();
    // Porcentaje.
    let porc = f.iter().filter(|x| **x == Ficha::Porcentaje).count();
    for _ in 0..porc {
        v *= 100.0;
    }
    // Posiciones clave.
    let pos_digitos: Vec<usize> = (0..f.len())
        .filter(|&k| matches!(f[k], Ficha::Digito(_)))
        .collect();
    if pos_digitos.is_empty() {
        // Solo literales: «"Total"». El signo se pierde salvo en la primera
        // seccion de un formato de una sola.
        if negativo {
            m.texto.push('-');
        }
        for x in f {
            match x {
                Ficha::Literal(l) => m.texto.push_str(l),
                Ficha::Hueco(_) => m.texto.push(' '),
                Ficha::Relleno(_) => m.relleno = Some(m.texto.len()),
                Ficha::Porcentaje => m.texto.push('%'),
                _ => {}
            }
        }
        return m;
    }
    let ultimo_digito = *pos_digitos.last().unwrap_or(&0);
    let punto = f.iter().position(|x| *x == Ficha::Punto);
    let exp_pos = f.iter().position(|x| matches!(x, Ficha::Exponente(_)));
    let barra = f.iter().position(|x| *x == Ficha::Barra);
    // Comas: de escala (justo detras del ultimo digito de la parte entera)
    // o de miles (entre digitos).
    let fin_entera = punto.or(exp_pos).unwrap_or(ultimo_digito + 1);
    let ultimo_entero = pos_digitos
        .iter()
        .copied()
        .filter(|&k| k < fin_entera)
        .max();
    let primer_digito = pos_digitos[0];
    let mut escala = 0;
    let mut miles = false;
    for (k, x) in f.iter().enumerate() {
        if *x == Ficha::Coma {
            let tras_ultimo =
                ultimo_entero.is_some_and(|u| k > u) && (k < fin_entera || punto.is_none());
            let siguientes_son_comas = f[k + 1..]
                .iter()
                .take_while(|y| !matches!(y, Ficha::Digito(_)))
                .all(|y| !matches!(y, Ficha::Digito(_)));
            if tras_ultimo
                && siguientes_son_comas
                && !pos_digitos.iter().any(|&d| d > k && d < fin_entera)
            {
                escala += 1;
            } else if k > primer_digito && k < fin_entera {
                miles = true;
            }
        }
    }
    for _ in 0..escala {
        v /= 1000.0;
    }
    if let Some(b) = barra {
        return fraccion(s, v, negativo, b, r);
    }
    // Cientifico.
    let mut exponente: Option<i32> = None;
    if let Some(ep) = exp_pos {
        let ent_ph = pos_digitos
            .iter()
            .filter(|&&k| k < punto.unwrap_or(ep).min(ep))
            .count()
            .max(1) as i32;
        let dec_ph = match punto {
            Some(p) if p < ep => pos_digitos.iter().filter(|&&k| k > p && k < ep).count(),
            _ => 0,
        };
        let mut e = if v == 0.0 {
            0
        } else {
            v.log10().floor() as i32
        };
        if ent_ph > 1 {
            // Ingenieria: exponente multiplo de los digitos enteros.
            e = e.div_euclid(ent_ph) * ent_ph;
        }
        let mut mant = if v == 0.0 { 0.0 } else { v / 10f64.powi(e) };
        let (ent, _) = digitos(mant, dec_ph);
        if ent.len() as i32 > ent_ph.max(1) && v != 0.0 {
            e += if ent_ph > 1 { ent_ph } else { 1 };
            mant = v / 10f64.powi(e);
        }
        v = mant;
        exponente = Some(e);
    }
    let fin_mant = exp_pos.unwrap_or(f.len());
    let dec_ph: Vec<usize> = match punto {
        Some(p) => pos_digitos
            .iter()
            .copied()
            .filter(|&k| k > p && k < fin_mant)
            .collect(),
        None => Vec::new(),
    };
    let ent_ph: Vec<usize> = pos_digitos
        .iter()
        .copied()
        .filter(|&k| k < punto.unwrap_or(fin_mant).min(fin_mant))
        .collect();
    let (ent, frac) = digitos(v, dec_ph.len());
    let ent = if ent == "0" && ent_ph.iter().all(|&k| f[k] != Ficha::Digito(b'0')) {
        String::new()
    } else {
        ent
    };
    // Parte entera: de derecha a izquierda.
    let mut salida_ent: Vec<(usize, String)> = Vec::new(); // (ficha, texto)
    {
        let digs: Vec<char> = ent.chars().collect();
        let mut quedan = digs.len();
        let mut puestos = 0usize;
        let poner = |c: char, puestos: &mut usize, txt: &mut String| {
            if miles && *puestos > 0 && *puestos % 3 == 0 {
                txt.insert(0, r.miles);
            }
            txt.insert(0, c);
            *puestos += 1;
        };
        for (n, &k) in ent_ph.iter().enumerate().rev() {
            let mut txt = String::new();
            if n == 0 {
                // El primero se lleva todos los digitos que sobren.
                while quedan > 0 {
                    quedan -= 1;
                    poner(digs[quedan], &mut puestos, &mut txt);
                }
                if txt.is_empty() {
                    match f[k] {
                        Ficha::Digito(b'0') => poner('0', &mut puestos, &mut txt),
                        Ficha::Digito(b'?') => txt.push(' '),
                        _ => {}
                    }
                }
            } else if quedan > 0 {
                quedan -= 1;
                poner(digs[quedan], &mut puestos, &mut txt);
            } else {
                match f[k] {
                    Ficha::Digito(b'0') => poner('0', &mut puestos, &mut txt),
                    Ficha::Digito(b'?') => txt.push(' '),
                    _ => {}
                }
            }
            salida_ent.push((k, txt));
        }
    }
    // Parte decimal: de izquierda a derecha; los `#` y `?` del final no
    // ponen ceros.
    let fr: Vec<char> = frac.chars().collect();
    let ultimo_no_cero = fr.iter().rposition(|&c| c != '0');
    let mut salida_dec: Vec<(usize, String)> = Vec::new();
    for (n, &k) in dec_ph.iter().enumerate() {
        let c = fr.get(n).copied().unwrap_or('0');
        let sobra = ultimo_no_cero.is_none_or(|u| n > u);
        let t = match f[k] {
            Ficha::Digito(b'0') => c.to_string(),
            Ficha::Digito(b'#') => {
                if sobra {
                    String::new()
                } else {
                    c.to_string()
                }
            }
            _ => {
                if sobra {
                    " ".into()
                } else {
                    c.to_string()
                }
            }
        };
        salida_dec.push((k, t));
    }
    // Montar en orden.
    // Un negativo que redondea a cero se pinta sin signo (salvo en cientifico).
    let es_cero = ent.trim_matches(['0', ' ']).is_empty() && frac.chars().all(|c| c == '0');
    if negativo && (exponente.is_some() || !es_cero) {
        m.texto.push('-');
    }
    for (k, x) in f.iter().enumerate() {
        match x {
            Ficha::Digito(_) => {
                if let Some((_, t)) = salida_ent.iter().find(|(p, _)| *p == k) {
                    m.texto.push_str(t);
                } else if let Some((_, t)) = salida_dec.iter().find(|(p, _)| *p == k) {
                    m.texto.push_str(t);
                } else if exponente.is_some() && k > fin_mant {
                    // digitos del exponente: se montan en Exponente.
                }
            }
            Ficha::Punto => {
                if punto == Some(k) {
                    m.texto.push(r.decimal);
                } else {
                    m.texto.push('.');
                }
            }
            Ficha::Coma => {}
            Ficha::Porcentaje => m.texto.push('%'),
            Ficha::Literal(l) => m.texto.push_str(l),
            Ficha::SeparadorFecha => m.texto.push(r.separador_fecha),
            Ficha::Hueco(_) => m.texto.push(' '),
            Ficha::Relleno(_) => m.relleno = Some(m.texto.len()),
            Ficha::Exponente(mas) => {
                let e = exponente.unwrap_or(0);
                m.texto.push('E');
                if e < 0 {
                    m.texto.push('-');
                } else if *mas {
                    m.texto.push('+');
                }
                let ancho = f[k + 1..]
                    .iter()
                    .filter(|y| matches!(y, Ficha::Digito(_)))
                    .count()
                    .max(1);
                m.texto.push_str(&format!("{:0ancho$}", e.abs()));
                // Lo que siga tras los digitos del exponente (literales).
                for y in &f[k + 1..] {
                    match y {
                        Ficha::Literal(l) => m.texto.push_str(l),
                        Ficha::Hueco(_) => m.texto.push(' '),
                        _ => {}
                    }
                }
                break;
            }
            _ => {}
        }
    }
    m
}

fn fraccion(s: &Seccion, v: f64, negativo: bool, barra: usize, r: &Regional) -> Mostrado {
    let f = &s.fichas;
    let mut m = Mostrado::default();
    // Denominador: digitos o un numero fijo tras la barra.
    let tras: Vec<&Ficha> = f[barra + 1..].iter().collect();
    let mut den_fijo: Option<u64> = None;
    let mut den_digitos = 0usize;
    for x in &tras {
        match x {
            Ficha::Digito(b'0') if den_fijo.is_none() => den_digitos += 1,
            Ficha::Digito(_) => den_digitos += 1,
            Ficha::Literal(l) if den_digitos == 0 => {
                let n: String = l.chars().take_while(char::is_ascii_digit).collect();
                if !n.is_empty() {
                    den_fijo = n.parse().ok();
                }
                break;
            }
            _ => break,
        }
    }
    // ¿Hay parte entera? Digitos separados del numerador por un literal.
    let antes: Vec<usize> = (0..barra)
        .filter(|&k| matches!(f[k], Ficha::Digito(_)))
        .collect();
    // Grupo del numerador: los digitos contiguos justo antes de la barra.
    let mut k = barra;
    while k > 0 && matches!(f[k - 1], Ficha::Digito(_)) {
        k -= 1;
    }
    let hay_entero = antes.iter().any(|&a| a < k);
    let (entero, resto) = if hay_entero {
        (v.trunc(), v.fract())
    } else {
        (0.0, v)
    };
    let max_den = 10u64.pow(den_digitos.clamp(1, 7) as u32) - 1;
    let (mut num, mut den) = match den_fijo {
        Some(d) if d > 0 => (((resto * d as f64).round()) as u64, d),
        _ => mejor_fraccion(resto, max_den),
    };
    let mut entero = entero as u64;
    if hay_entero && den > 0 && num >= den {
        entero += num / den;
        num %= den;
    }
    if num == 0 && !hay_entero {
        den = den.max(1);
    }
    if negativo && (entero > 0 || num > 0) {
        m.texto.push('-');
    }
    if hay_entero {
        if entero > 0 || num == 0 {
            m.texto.push_str(&entero.to_string());
        }
        if num > 0 {
            if entero > 0 {
                m.texto.push(' ');
            }
            m.texto.push_str(&format!("{num}/{den}"));
        } else {
            // Excel deja el hueco de la fraccion en blanco.
            m.texto.push_str(&" ".repeat(den_digitos * 2 + 1 + 1));
        }
    } else {
        m.texto.push_str(&format!("{num}/{den}"));
    }
    let _ = r;
    m
}

/// La fraccion mas cercana con denominador hasta `max` (Stern-Brocot).
fn mejor_fraccion(x: f64, max: u64) -> (u64, u64) {
    let x = x.abs();
    let ent = x.floor();
    let fr = x - ent;
    let (mut a, mut b, mut c, mut d) = (0u64, 1u64, 1u64, 1u64);
    let mut mejor = (0u64, 1u64);
    let mut err = fr;
    if (1.0 - fr) < err {
        mejor = (1, 1);
        err = 1.0 - fr;
    }
    for _ in 0..64 {
        let mn = a + c;
        let md = b + d;
        if md > max {
            break;
        }
        let med = mn as f64 / md as f64;
        let e = (med - fr).abs();
        if e < err {
            err = e;
            mejor = (mn, md);
        }
        if fr < med {
            c = mn;
            d = md;
        } else {
            a = mn;
            b = md;
        }
    }
    (mejor.0 + ent as u64 * mejor.1, mejor.1)
}

/// Fecha civil de un numero de serie: (ano, mes, dia, dia de la semana 0=domingo).
pub fn fecha_de_serie(dias: i64, fecha_1904: bool) -> (i64, u32, u32, u32) {
    if fecha_1904 {
        let (y, m, d) = civil(dias + dias_desde_civil(1904, 1, 1));
        return (y, m, d, ((dias + 5).rem_euclid(7)) as u32);
    }
    let sem = ((dias + 6).rem_euclid(7)) as u32;
    if dias == 0 {
        return (1900, 1, 0, sem);
    }
    if dias == 60 {
        return (1900, 2, 29, sem);
    }
    let ajuste = if dias < 60 { 1 } else { 0 };
    let (y, m, d) = civil(dias + ajuste + dias_desde_civil(1899, 12, 30));
    (y, m, d, sem)
}

/// Dias desde 1970-01-01 (algoritmo civil de Howard Hinnant, dominio publico).
fn dias_desde_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn fecha(s: &Seccion, v: f64, fecha_1904: bool, r: &Regional) -> Mostrado {
    let mut m = Mostrado::default();
    if !(0.0..=2_958_466.0).contains(&v) {
        // Fuera del rango de fechas de Excel: almohadillas.
        m.texto = "#".repeat(11);
        return m;
    }
    let f = &s.fichas;
    let dec_seg = f
        .iter()
        .filter_map(|x| {
            if let Ficha::FraccionSegundo(n) = x {
                Some(*n as i32)
            } else {
                None
            }
        })
        .max()
        .unwrap_or(0)
        .min(3);
    let paso = 10f64.powi(dec_seg);
    // Redondeo al segundo (o a la decima que se muestre).
    let total = (v * 86400.0 * paso).round() / paso;
    let dias = (total / 86400.0).floor();
    let seg_dia = total - dias * 86400.0;
    let dias = dias as i64;
    let (y, mes, d, sem) = fecha_de_serie(dias, fecha_1904);
    let seg_ent = seg_dia.floor() as i64;
    let hh = seg_ent / 3600;
    let mi = (seg_ent / 60) % 60;
    let ss = seg_ent % 60;
    let fr = seg_dia - seg_ent as f64;
    let ampm = f.iter().find_map(|x| {
        if let Ficha::AmPm(k) = x {
            Some(*k)
        } else {
            None
        }
    });
    let ingles = s.ingles;
    let en = Regional::en_us();
    let nombres = if ingles { &en } else { r };
    for x in f {
        match x {
            Ficha::Literal(l) => m.texto.push_str(l),
            Ficha::SeparadorFecha => m.texto.push(r.separador_fecha),
            Ficha::SeparadorGuion => m.texto.push(if r.separador_fecha == '/' {
                '-'
            } else {
                r.separador_fecha
            }),
            Ficha::Hueco(_) => m.texto.push(' '),
            Ficha::Relleno(_) => m.relleno = Some(m.texto.len()),
            Ficha::Ano(n) => {
                if *n <= 2 {
                    m.texto.push_str(&format!("{:02}", y.rem_euclid(100)));
                } else {
                    m.texto.push_str(&format!("{y:04}"));
                }
            }
            Ficha::Mes(n) => {
                let i = (mes as usize).clamp(1, 12) - 1;
                match n {
                    1 => m.texto.push_str(&mes.to_string()),
                    2 => m.texto.push_str(&format!("{mes:02}")),
                    3 => m.texto.push_str(&nombres.meses_cortos[i]),
                    5 => m.texto.extend(nombres.meses[i].chars().next()),
                    _ => m.texto.push_str(&nombres.meses[i]),
                }
            }
            Ficha::Dia(n) => match n {
                1 => m.texto.push_str(&d.to_string()),
                2 => m.texto.push_str(&format!("{d:02}")),
                3 => m.texto.push_str(&nombres.dias_cortos[sem as usize % 7]),
                _ => m.texto.push_str(&nombres.dias[sem as usize % 7]),
            },
            Ficha::Hora(n) => {
                let h = if ampm.is_some() {
                    let h = hh % 12;
                    if h == 0 { 12 } else { h }
                } else {
                    hh
                };
                if *n >= 2 {
                    m.texto.push_str(&format!("{h:02}"));
                } else {
                    m.texto.push_str(&h.to_string());
                }
            }
            Ficha::Minuto(n) => {
                if *n >= 2 {
                    m.texto.push_str(&format!("{mi:02}"));
                } else {
                    m.texto.push_str(&mi.to_string());
                }
            }
            Ficha::Segundo(n) => {
                if *n >= 2 {
                    m.texto.push_str(&format!("{ss:02}"));
                } else {
                    m.texto.push_str(&ss.to_string());
                }
            }
            Ficha::FraccionSegundo(n) => {
                let n = (*n as usize).min(3);
                let t = format!("{:.*}", n, fr);
                m.texto.push(r.decimal);
                m.texto.push_str(t.split('.').nth(1).unwrap_or(""));
            }
            Ficha::AmPm(k) => {
                let pm = hh >= 12;
                if *k == 1 {
                    m.texto.push(if pm { 'P' } else { 'A' });
                } else if ingles {
                    m.texto.push_str(if pm { "PM" } else { "AM" });
                } else {
                    m.texto.push_str(if pm { &r.pm } else { &r.am });
                }
            }
            Ficha::Transcurrido(c, n) => {
                let total_seg = (dias as f64 * 86400.0 + seg_ent as f64) as i64;
                let val = match c {
                    'h' => total_seg / 3600,
                    'm' => total_seg / 60,
                    _ => total_seg,
                };
                m.texto.push_str(&format!("{:0w$}", val, w = *n as usize));
            }
            _ => {}
        }
    }
    m
}

/// Cache de formatos ya troceados, por su codigo.
#[derive(Default)]
pub struct Formatos {
    mapa: std::collections::HashMap<String, Arc<Formato>>,
}

impl Formatos {
    pub fn obtener(&mut self, codigo: &str, r: &Regional) -> Arc<Formato> {
        if let Some(f) = self.mapa.get(codigo) {
            return f.clone();
        }
        let f = Arc::new(Formato::nuevo(codigo, r));
        self.mapa.insert(codigo.to_string(), f.clone());
        f
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn n(codigo: &str, v: f64) -> String {
        Formato::nuevo(codigo, &Regional::en_us())
            .numero(v, false, &Regional::en_us())
            .texto
    }

    #[test]
    fn general_como_excel() {
        let r = Regional::en_us();
        assert_eq!(general(0.0, &r), "0");
        assert_eq!(general(1.0 / 3.0, &r), "0.333333333");
        assert_eq!(general(-1.0 / 3.0, &r), "-0.33333333");
        assert_eq!(general(123456789012.0, &r), "1.23457E+11");
        assert_eq!(general(12345678901.0, &r), "12345678901");
        assert_eq!(general(0.1 + 0.2, &r), "0.3");
        assert_eq!(general(1e-10, &r), "1E-10");
        assert_eq!(general(42.0, &r), "42");
    }

    #[test]
    fn numeros_con_miles_decimales_y_negativos() {
        assert_eq!(n("#,##0.00", 1234567.891), "1,234,567.89");
        assert_eq!(n("#,##0.00", -1234.5), "-1,234.50");
        assert_eq!(n("0.00", 2.675), "2.68");
        assert_eq!(n("#,##0;(#,##0)", -1234.0), "(1,234)");
        assert_eq!(n("0%", 0.256), "26%");
        assert_eq!(n("0.0%", 0.5), "50.0%");
        assert_eq!(n("#,##0,\"K\"", 1234567.0), "1,235K");
        assert_eq!(n("0.00E+00", 12345.0), "1.23E+04");
        assert_eq!(n("##0.0E+0", 12345.0), "12.3E+3");
        assert_eq!(n("000-0000", 5551234.0), "555-1234");
        assert_eq!(n("#.##", 3.1), "3.1");
        assert_eq!(n("0.0#", 3.0), "3.0");
        assert_eq!(n("\"$\"#,##0.00", 5.0), "$5.00");
        assert_eq!(n("[$S/-280A] #,##0.00", 5.0), "S/ 5.00");
        assert_eq!(n("0;-0;\"cero\"", 0.0), "cero");
        assert_eq!(n("[>=1000]#,##0,\"K\";0", 2500.0), "3K");
        assert_eq!(n("[>=1000]#,##0,\"K\";0", 25.0), "25");
        assert_eq!(n("# ?/?", 1.5), "1 1/2");
        assert_eq!(n("?/8", 0.25), "2/8");
    }

    #[test]
    fn contable_con_relleno_y_color() {
        let f = Formato::nuevo(
            "_(* #,##0.00_);[Red]_(* (#,##0.00);_(* \"-\"??_);_(@_)",
            &Regional::en_us(),
        );
        let m = f.numero(-1234.5, false, &Regional::en_us());
        assert_eq!(m.texto, " (1,234.50)");
        assert_eq!(m.color, Some([255, 0, 0]));
        assert_eq!(m.relleno, Some(1));
        assert_eq!(f.numero(0.0, false, &Regional::en_us()).texto, " -   ");
        assert_eq!(f.texto("hola").texto, " hola ");
    }

    #[test]
    fn fechas_y_horas() {
        assert_eq!(n("yyyy-mm-dd", 45000.0), "2023-03-15");
        assert_eq!(n("d/m/yy h:mm", 45000.75), "15/3/23 18:00");
        assert_eq!(n("dddd, mmmm d, yyyy", 1.0), "Sunday, January 1, 1900");
        assert_eq!(n("yyyy-mm-dd", 60.0), "1900-02-29");
        assert_eq!(n("yyyy-mm-dd", 61.0), "1900-03-01");
        assert_eq!(n("h:mm AM/PM", 0.5), "12:00 PM");
        assert_eq!(n("[h]:mm:ss", 1.5), "36:00:00");
        assert_eq!(n("mm:ss.0", 0.000_011_574_07 * 1.5), "00:01.5");
        assert_eq!(n("mmm-yy", 45000.0), "Mar-23");
        let r = Regional::en_us();
        assert_eq!(
            Formato::nuevo("yyyy-mm-dd", &r).numero(0.0, true, &r).texto,
            "1904-01-01"
        );
        let es = Regional::es_pe();
        assert_eq!(
            Formato::nuevo(integrado(14).unwrap(), &es)
                .numero(45000.0, false, &es)
                .texto,
            "15/03/2023"
        );
    }

    #[test]
    fn el_separador_de_fechas_del_sistema() {
        // Medido en un Excel en espanol con el separador de fechas cambiado
        // a espacio: la barra y el guion de los integrados 15-17 lo usan.
        let mut r = Regional::es_pe();
        r.separador_fecha = ' ';
        let f = |c: &str, v: f64| Formato::nuevo(c, &r).numero(v, false, &r).texto;
        assert_eq!(f("d/m/yyyy", 1234.5678), "18 5 1903");
        assert_eq!(f(integrado(15).unwrap(), 1234.5678), "18 May 03");
        assert_eq!(f("mmmm", 1234.5678), "Mayo");
        assert_eq!(f("dddd", 0.0), "sábado");
        assert_eq!(f("h:mm AM/PM", 0.0), "12:00 a. m.");
        assert_eq!(f("[$-409]mmmm d, yyyy", 0.0), "January 0, 1900");
        assert_eq!(f("[h]:mm:ss", 0.000123), "0:00:11");
        assert_eq!(f("mm:ss.0", 0.000123), "00:10.6");
        assert_eq!(f("# ??/??", 1234.5678), "1234 46/81");
        let en = Regional::en_us();
        assert_eq!(
            Formato::nuevo(integrado(15).unwrap(), &en)
                .numero(45000.0, false, &en)
                .texto,
            "15-Mar-23"
        );
    }

    #[test]
    fn codigos_raros_no_revientan() {
        for c in [
            "",
            ";;;;;;",
            "[",
            "\"",
            "0.0.0",
            "E+",
            "#/",
            "[h",
            "\\",
            "*",
            "_",
            "[Color99]0",
            "0,,,,,,",
            "?/0",
        ] {
            let f = Formato::nuevo(c, &Regional::en_us());
            let _ = f.numero(1234.5, false, &Regional::en_us());
            let _ = f.numero(-1e300, false, &Regional::en_us());
            let _ = f.texto("x");
        }
    }
}
