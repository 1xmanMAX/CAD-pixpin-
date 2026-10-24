//! **El interprete de formulas** de la grafica y de la ecuacion (`object
//! Formula` de `motor/Graficas.kt` del movil, portado tal cual).
//!
//! De un texto como `2x^2 - 3sin(x)` a un arbol que se evalua y que se
//! escribe bonito: en el LaTeX de bolsillo de las notas (`latex`) y, sobre
//! todo, tipografiado dentro del dibujo (`ecuacion.rs`).
//!
//! Descenso recursivo de manual, con la precedencia de siempre (potencia,
//! luego producto, luego suma, luego comparacion) y la potencia asociando a
//! la derecha. La multiplicacion implicita se decide al leer: un numero
//! seguido de una letra o de un parentesis, un parentesis cerrado seguido de
//! otro abierto, una `x` seguida de una funcion... multiplican.
//!
//! Variables: `x`, `y` y `t`. Una grafica plana solo admite `x`; quien
//! compila mira [`Compilada::variables`] y decide.
//!
//! Condiciones: `<`, `<=`, `>`, `>=`, `=`, `!=` dan uno o cero, y
//! `si(cond, a, b)` elige. **Por partes**: `x^2 si x < 0; 2x si x >= 0` —las
//! partes van separadas por punto y coma y cada una con su condicion detras
//! de un `si`—; donde ninguna se cumple, no hay valor (NaN).
//!
//! La coma decimal se admite (`0,5`) cuando va entre digitos; el separador
//! de argumentos de `max(a; b)` es el punto y coma dentro de parentesis o la
//! coma.
//!
//! No hay KaTeX ni MathJax detras, ni en el movil ni aqui: el movil compone
//! la ecuacion con cajas propias y la dibuja con textos y rayas del lienzo.
//! Por eso esto es Rust puro, sin WebView2 ni dependencias.

use std::collections::BTreeSet;

/// El arbol de la formula.
#[derive(Debug, Clone, PartialEq)]
pub enum Nodo {
    /// Un numero, con el texto tal como se escribio (para el rotulo).
    Numero {
        valor: f64,
        texto: String,
    },
    /// `x`, `y` o `t`.
    Variable(char),
    /// `pi` o `e`.
    Constante {
        nombre: &'static str,
        valor: f64,
    },
    /// `op` es el del movil: `+ - * / % ^ < >`, `l` (<=), `g` (>=), `=` y
    /// `n` (!=). `implicita`: un producto sin signo (`2x`).
    Binaria {
        op: char,
        a: Box<Nodo>,
        b: Box<Nodo>,
        implicita: bool,
    },
    Negado(Box<Nodo>),
    /// Unos parentesis escritos: se conservan para el rotulo.
    Grupo(Box<Nodo>),
    /// Una funcion con su nombre canonico (`sin`, `sqrt`...).
    Funcion {
        nombre: &'static str,
        args: Vec<Nodo>,
    },
}

/// Las variables de una evaluacion.
#[derive(Debug, Clone, Copy, Default)]
pub struct Vars {
    pub x: f64,
    pub y: f64,
    pub t: f64,
}

fn casi_igual(x: f64, y: f64) -> bool {
    (x - y).abs() <= 1e-9 * 1f64.max(x.abs()).max(y.abs())
}

/// `Math.signum`: el cero es cero (el `signum` de Rust da uno).
fn signo(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        v
    }
}

impl Nodo {
    pub fn en(&self, v: &Vars) -> f64 {
        match self {
            Nodo::Numero { valor, .. } => *valor,
            Nodo::Variable(c) => match c {
                'x' => v.x,
                'y' => v.y,
                _ => v.t,
            },
            Nodo::Constante { valor, .. } => *valor,
            Nodo::Binaria { op, a, b, .. } => {
                let x = a.en(v);
                let y = b.en(v);
                let si = |c: bool| if c { 1.0 } else { 0.0 };
                match op {
                    '+' => x + y,
                    '-' => x - y,
                    '*' => x * y,
                    '/' => x / y,
                    '%' => x % y,
                    '^' => x.powf(y),
                    '<' => si(x < y),
                    '>' => si(x > y),
                    'l' => si(x <= y),
                    'g' => si(x >= y),
                    '=' => si(casi_igual(x, y)),
                    'n' => si(!casi_igual(x, y)),
                    _ => f64::NAN,
                }
            }
            Nodo::Negado(a) => -a.en(v),
            Nodo::Grupo(a) => a.en(v),
            Nodo::Funcion { nombre, args } => {
                let a = |i: usize| args[i].en(v);
                match *nombre {
                    "sin" => a(0).sin(),
                    "cos" => a(0).cos(),
                    "tan" => a(0).tan(),
                    "asin" => a(0).asin(),
                    "acos" => a(0).acos(),
                    "atan" => a(0).atan(),
                    "sinh" => a(0).sinh(),
                    "cosh" => a(0).cosh(),
                    "tanh" => a(0).tanh(),
                    "sqrt" => a(0).sqrt(),
                    "cbrt" => a(0).cbrt(),
                    "abs" => a(0).abs(),
                    "exp" => a(0).exp(),
                    "ln" => a(0).ln(),
                    "log" => a(0).log10(),
                    "log2" => a(0).ln() / 2f64.ln(),
                    "floor" => a(0).floor(),
                    "ceil" => a(0).ceil(),
                    // `Math.rint`: a la par en el medio.
                    "round" => a(0).round_ties_even(),
                    "sign" => signo(a(0)),
                    "max" => a(0).max(a(1)),
                    "min" => a(0).min(a(1)),
                    "pow" => a(0).powf(a(1)),
                    "atan2" => a(0).atan2(a(1)),
                    "mod" => a(0) % a(1),
                    "si" => {
                        if a(0) != 0.0 {
                            a(1)
                        } else {
                            a(2)
                        }
                    }
                    _ => f64::NAN,
                }
            }
        }
    }

    fn variables_en(&self, s: &mut BTreeSet<char>) {
        match self {
            Nodo::Variable(c) => {
                s.insert(*c);
            }
            Nodo::Binaria { a, b, .. } => {
                a.variables_en(s);
                b.variables_en(s);
            }
            Nodo::Negado(a) | Nodo::Grupo(a) => a.variables_en(s),
            Nodo::Funcion { args, .. } => args.iter().for_each(|n| n.variables_en(s)),
            _ => {}
        }
    }

    fn es_suma(&self) -> bool {
        matches!(self, Nodo::Binaria { op: '+' | '-', .. })
    }

    fn es_compuesto(&self) -> bool {
        matches!(self, Nodo::Binaria { .. } | Nodo::Negado(_))
    }

    /// En el LaTeX de bolsillo de las notas.
    pub fn latex(&self) -> String {
        let si_suma = |n: &Nodo| {
            if n.es_suma() {
                format!("({})", n.latex())
            } else {
                n.latex()
            }
        };
        let si_compuesto = |n: &Nodo| {
            if n.es_compuesto() {
                format!("({})", n.latex())
            } else {
                n.latex()
            }
        };
        match self {
            Nodo::Numero { texto, .. } => texto.clone(),
            Nodo::Variable(c) => c.to_string(),
            Nodo::Constante { nombre, .. } => {
                if *nombre == "pi" {
                    "\\pi".into()
                } else {
                    (*nombre).into()
                }
            }
            Nodo::Binaria {
                op,
                a,
                b,
                implicita,
            } => match op {
                '+' => format!("{} + {}", a.latex(), b.latex()),
                '-' => format!("{} - {}", a.latex(), si_suma(b)),
                '*' if *implicita => {
                    let izq = a.latex();
                    let der = b.latex();
                    // «\pi» seguido de «x» se pegaria en «\pix».
                    let letra_letra = izq.chars().last().is_some_and(char::is_alphabetic)
                        && der.chars().next().is_some_and(char::is_alphabetic);
                    if letra_letra {
                        format!("{izq} {der}")
                    } else {
                        format!("{izq}{der}")
                    }
                }
                '*' => format!("{} \\cdot {}", a.latex(), b.latex()),
                '/' => format!("\\frac{{{}}}{{{}}}", a.latex(), b.latex()),
                '%' => format!("{} \\bmod {}", a.latex(), b.latex()),
                '^' => format!("{}^{{{}}}", si_compuesto(a), b.latex()),
                '<' => format!("{} < {}", a.latex(), b.latex()),
                '>' => format!("{} > {}", a.latex(), b.latex()),
                'l' => format!("{} \\le {}", a.latex(), b.latex()),
                'g' => format!("{} \\ge {}", a.latex(), b.latex()),
                '=' => format!("{} = {}", a.latex(), b.latex()),
                _ => format!("{} \\ne {}", a.latex(), b.latex()),
            },
            Nodo::Negado(a) => format!("-{}", si_suma(a)),
            Nodo::Grupo(a) => format!("({})", a.latex()),
            Nodo::Funcion { nombre, args } => {
                let todos = || args.iter().map(Nodo::latex).collect::<Vec<_>>().join(", ");
                match *nombre {
                    "sqrt" => format!("\\sqrt{{{}}}", args[0].latex()),
                    "cbrt" => format!("\\sqrt[3]{{{}}}", args[0].latex()),
                    "abs" => format!("|{}|", args[0].latex()),
                    "exp" => format!("e^{{{}}}", args[0].latex()),
                    "sin" | "cos" | "tan" | "ln" | "log" | "sinh" | "cosh" | "tanh" | "max"
                    | "min" => format!("\\{nombre}({})", todos()),
                    "asin" | "acos" | "atan" => {
                        format!("\\arc{}({})", &nombre[1..], args[0].latex())
                    }
                    "log2" => format!("\\log_{{2}}({})", args[0].latex()),
                    "pow" => format!("{}^{{{}}}", si_compuesto(&args[0]), args[1].latex()),
                    "si" => format!(
                        "\\{{{}\\ si\\ {};\\ {}\\}}",
                        args[1].latex(),
                        args[0].latex(),
                        args[2].latex()
                    ),
                    _ => format!("{nombre}({})", todos()),
                }
            }
        }
    }
}

/// Una parte de una funcion por partes: la expresion y, si la tiene, su
/// condicion.
#[derive(Debug, Clone, PartialEq)]
pub struct Parte {
    pub expresion: Nodo,
    pub condicion: Option<Nodo>,
}

/// La formula compilada. `texto` es como se escribio, limpio.
#[derive(Debug, Clone, PartialEq)]
pub struct Compilada {
    pub texto: String,
    pub partes: Vec<Parte>,
}

impl Compilada {
    /// Que variables usa: `x`, `y`, `t`.
    pub fn variables(&self) -> BTreeSet<char> {
        let mut s = BTreeSet::new();
        for p in &self.partes {
            p.expresion.variables_en(&mut s);
            if let Some(c) = &p.condicion {
                c.variables_en(&mut s);
            }
        }
        s
    }

    /// Cada parte en el LaTeX de bolsillo: la expresion y su condicion.
    pub fn latex(&self) -> Vec<(String, Option<String>)> {
        self.partes
            .iter()
            .map(|p| (p.expresion.latex(), p.condicion.as_ref().map(Nodo::latex)))
            .collect()
    }

    /// El valor con estas variables, o NaN si ninguna parte se cumple.
    pub fn evaluar(&self, x: f64, y: f64, t: f64) -> f64 {
        let v = Vars { x, y, t };
        for p in &self.partes {
            if p.condicion.as_ref().is_none_or(|c| c.en(&v) != 0.0) {
                return p.expresion.en(&v);
            }
        }
        f64::NAN
    }

    pub fn en(&self, x: f64) -> f64 {
        self.evaluar(x, 0.0, 0.0)
    }
}

/// Compila, o `None` si no se entiende.
pub fn compilar(fuente: &str) -> Option<Compilada> {
    let texto = limpiar(fuente);
    if texto.is_empty() {
        return None;
    }
    let mut partes = Vec::new();
    for trozo in partes_de(&texto) {
        let (expr, cond) = separar_condicion(&trozo);
        let expresion = leer_entera(&expr)?;
        let condicion = match cond {
            Some(c) => Some(leer_entera(&c)?),
            None => None,
        };
        partes.push(Parte {
            expresion,
            condicion,
        });
    }
    if partes.is_empty() {
        return None;
    }
    Some(Compilada { texto, partes })
}

fn leer_entera(texto: &str) -> Option<Nodo> {
    let fichas = fichas(texto)?;
    let mut l = Lector { fichas, pos: 0 };
    let n = l.expresion()?;
    l.se_acabo().then_some(n)
}

/// Quita la decoracion de delante (`y =`, `f(x) =`) y pasa la coma decimal
/// entre digitos a punto.
fn limpiar(fuente: &str) -> String {
    let t = fuente.trim();
    let c: Vec<char> = t.chars().collect();
    // `^\s*[a-zA-Z]\s*(\([^)]*\))?\s*=\s*`, a mano: una letra, quiza unos
    // parentesis, y un igual (que no sea `==`, que ya es una comparacion).
    let mut i = 0;
    let mut quitar = 0;
    if i < c.len() && c[i].is_ascii_alphabetic() {
        i += 1;
        while i < c.len() && c[i].is_whitespace() {
            i += 1;
        }
        if i < c.len() && c[i] == '(' {
            match c[i..].iter().position(|&x| x == ')') {
                Some(k) => i += k + 1,
                None => i = usize::MAX,
            }
        }
        if i != usize::MAX {
            while i < c.len() && c[i].is_whitespace() {
                i += 1;
            }
            if i < c.len() && c[i] == '=' {
                i += 1;
                while i < c.len() && c[i].is_whitespace() {
                    i += 1;
                }
                quitar = i;
            }
        }
    }
    let c = &c[quitar..];
    let mut s = String::with_capacity(c.len());
    for (k, &ch) in c.iter().enumerate() {
        let entre_digitos = ch == ','
            && k > 0
            && c[k - 1].is_ascii_digit()
            && c.get(k + 1).is_some_and(char::is_ascii_digit);
        s.push(if entre_digitos { '.' } else { ch });
    }
    s.trim().to_string()
}

/// Las partes: separadas por punto y coma FUERA de parentesis.
fn partes_de(texto: &str) -> Vec<String> {
    let mut salida = Vec::new();
    let mut nivel = 0i32;
    let mut actual = String::new();
    for c in texto.chars() {
        match c {
            '(' | '[' | '{' => nivel += 1,
            ')' | ']' | '}' => nivel -= 1,
            _ => {}
        }
        if c == ';' && nivel == 0 {
            salida.push(std::mem::take(&mut actual));
        } else {
            actual.push(c);
        }
    }
    salida.push(actual);
    salida
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// `expresion si condicion`, o la expresion a secas. Tambien «para»,
/// «cuando» e «if». La primera palabra clave que tenga algo delante y algo
/// detras, entre espacios: `sin x` no es un «si».
fn separar_condicion(trozo: &str) -> (String, Option<String>) {
    let c: Vec<char> = trozo.chars().collect();
    for i in 1..c.len() {
        if !c[i].is_whitespace() || c[i - 1].is_whitespace() {
            continue;
        }
        // `i` empieza una racha de espacios.
        let mut j = i;
        while j < c.len() && c[j].is_whitespace() {
            j += 1;
        }
        for clave in ["si", "para", "cuando", "if"] {
            let k: Vec<char> = clave.chars().collect();
            let fin = j + k.len();
            if fin < c.len() && c[j..fin] == k[..] && c[fin].is_whitespace() {
                let resto: String = c[fin..].iter().collect();
                let delante: String = c[..i].iter().collect();
                if !resto.trim().is_empty() && !delante.trim().is_empty() {
                    return (delante.trim().to_string(), Some(resto.trim().to_string()));
                }
            }
        }
    }
    (trozo.to_string(), None)
}

#[derive(Debug, Clone, PartialEq)]
enum Ficha {
    Numero(f64, String),
    Nombre(String),
    Signo(char),
}

fn fichas(texto: &str) -> Option<Vec<Ficha>> {
    let c: Vec<char> = texto.chars().collect();
    let mut salida = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let sig = c.get(i + 1).copied();
        if ch.is_whitespace() {
            i += 1;
        } else if ch.is_ascii_digit() || (ch == '.' && sig.is_some_and(|s| s.is_ascii_digit())) {
            let inicio = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                i += 1;
            }
            let t: String = c[inicio..i].iter().collect();
            salida.push(Ficha::Numero(t.parse().ok()?, t));
        } else if ch.is_alphabetic() {
            let inicio = i;
            while i < c.len() && (c[i].is_alphabetic() || c[i].is_ascii_digit() || c[i] == '_') {
                i += 1;
            }
            let t: String = c[inicio..i].iter().collect();
            salida.push(Ficha::Nombre(t.to_lowercase()));
        } else {
            let (f, paso): (Vec<Ficha>, usize) = match ch {
                '²' => (vec![Ficha::Signo('^'), Ficha::Numero(2.0, "2".into())], 1),
                '³' => (vec![Ficha::Signo('^'), Ficha::Numero(3.0, "3".into())], 1),
                '√' => (vec![Ficha::Nombre("sqrt".into())], 1),
                '×' | '·' => (vec![Ficha::Signo('*')], 1),
                '÷' => (vec![Ficha::Signo('/')], 1),
                '−' | '–' => (vec![Ficha::Signo('-')], 1),
                ';' => (vec![Ficha::Signo(',')], 1),
                '≤' => (vec![Ficha::Signo('l')], 1),
                '≥' => (vec![Ficha::Signo('g')], 1),
                '≠' => (vec![Ficha::Signo('n')], 1),
                '<' if sig == Some('=') => (vec![Ficha::Signo('l')], 2),
                '<' => (vec![Ficha::Signo('<')], 1),
                '>' if sig == Some('=') => (vec![Ficha::Signo('g')], 2),
                '>' => (vec![Ficha::Signo('>')], 1),
                '=' if sig == Some('=') => (vec![Ficha::Signo('=')], 2),
                '=' => (vec![Ficha::Signo('=')], 1),
                '!' if sig == Some('=') => (vec![Ficha::Signo('n')], 2),
                '+' | '-' | '*' | '/' | '^' | '(' | ')' | ',' | '%' | '|' => {
                    (vec![Ficha::Signo(ch)], 1)
                }
                // Un caracter raro: no se entiende.
                _ => return None,
            };
            salida.extend(f);
            i += paso;
        }
    }
    Some(salida)
}

struct Lector {
    fichas: Vec<Ficha>,
    pos: usize,
}

impl Lector {
    fn mira(&self) -> Option<&Ficha> {
        self.fichas.get(self.pos)
    }

    fn toma(&mut self) -> Option<Ficha> {
        let f = self.fichas.get(self.pos).cloned();
        self.pos += 1;
        f
    }

    fn es_signo(&self, c: char) -> bool {
        matches!(self.mira(), Some(Ficha::Signo(s)) if *s == c)
    }

    fn se_acabo(&self) -> bool {
        self.pos >= self.fichas.len()
    }

    /// Comparaciones: el nivel mas suelto.
    fn expresion(&mut self) -> Option<Nodo> {
        let mut izq = self.suma()?;
        while let Some(Ficha::Signo(c)) = self.mira() {
            let c = *c;
            if !"<>lg=n".contains(c) {
                break;
            }
            self.toma();
            let der = self.suma()?;
            izq = binaria(c, izq, der, false);
        }
        Some(izq)
    }

    fn suma(&mut self) -> Option<Nodo> {
        let mut izq = self.termino()?;
        while self.es_signo('+') || self.es_signo('-') {
            let Some(Ficha::Signo(c)) = self.toma() else {
                return None;
            };
            let der = self.termino()?;
            izq = binaria(c, izq, der, false);
        }
        Some(izq)
    }

    fn termino(&mut self) -> Option<Nodo> {
        let mut izq = self.unario()?;
        while let Some(f) = self.mira() {
            let explicito = matches!(f, Ficha::Signo('*' | '/' | '%'));
            // Implicita: lo que puede empezar un factor, sin signo en medio.
            // El «|» no: cerraria un valor absoluto abierto.
            let implicito = matches!(f, Ficha::Numero(..) | Ficha::Nombre(_) | Ficha::Signo('('));
            if !explicito && !implicito {
                break;
            }
            let c = if explicito {
                match self.toma() {
                    Some(Ficha::Signo(c)) => c,
                    _ => return None,
                }
            } else {
                '*'
            };
            let der = if explicito {
                self.unario()?
            } else {
                self.potencia()?
            };
            izq = binaria(c, izq, der, !explicito);
        }
        Some(izq)
    }

    fn unario(&mut self) -> Option<Nodo> {
        if self.es_signo('-') {
            self.toma();
            return Some(Nodo::Negado(Box::new(self.unario()?)));
        }
        if self.es_signo('+') {
            self.toma();
            return self.unario();
        }
        self.potencia()
    }

    fn potencia(&mut self) -> Option<Nodo> {
        let base = self.atomo()?;
        if self.es_signo('^') {
            self.toma();
            let exp = self.unario()?;
            return Some(binaria('^', base, exp, false));
        }
        Some(base)
    }

    fn atomo(&mut self) -> Option<Nodo> {
        match self.toma()? {
            Ficha::Numero(valor, texto) => Some(Nodo::Numero { valor, texto }),
            Ficha::Signo('(') => {
                let e = self.expresion()?;
                self.espera(')')?;
                Some(Nodo::Grupo(Box::new(e)))
            }
            Ficha::Signo('|') => {
                let e = self.expresion()?;
                self.espera('|')?;
                Some(Nodo::Funcion {
                    nombre: "abs",
                    args: vec![e],
                })
            }
            Ficha::Signo(_) => None,
            Ficha::Nombre(n) => self.nombre(&n),
        }
    }

    fn espera(&mut self, c: char) -> Option<()> {
        if !self.es_signo(c) {
            return None;
        }
        self.toma();
        Some(())
    }

    fn argumentos(&mut self) -> Option<Vec<Nodo>> {
        self.espera('(')?;
        let mut lista = Vec::new();
        if !self.es_signo(')') {
            lista.push(self.expresion()?);
            while self.es_signo(',') {
                self.toma();
                lista.push(self.expresion()?);
            }
        }
        self.espera(')')?;
        Some(lista)
    }

    /// `sin(x)` de siempre, o `sin x` sin parentesis, como en la pizarra.
    fn argumentos_o_implicito(&mut self) -> Option<Vec<Nodo>> {
        if self.es_signo('(') {
            return self.argumentos();
        }
        Some(vec![self.unario()?])
    }

    fn nombre(&mut self, n: &str) -> Option<Nodo> {
        match n {
            "x" => return Some(Nodo::Variable('x')),
            "y" => return Some(Nodo::Variable('y')),
            "t" => return Some(Nodo::Variable('t')),
            "pi" | "π" => {
                return Some(Nodo::Constante {
                    nombre: "pi",
                    valor: std::f64::consts::PI,
                });
            }
            "e" => {
                return Some(Nodo::Constante {
                    nombre: "e",
                    valor: std::f64::consts::E,
                });
            }
            _ => {}
        }
        // Las de un argumento, con su nombre canonico.
        let una: Option<&'static str> = match n {
            "sin" | "sen" => Some("sin"),
            "cos" => Some("cos"),
            "tan" | "tg" => Some("tan"),
            "asin" | "arcsin" | "asen" => Some("asin"),
            "acos" | "arccos" => Some("acos"),
            "atan" | "arctan" | "atg" => Some("atan"),
            "sinh" | "senh" => Some("sinh"),
            "cosh" => Some("cosh"),
            "tanh" => Some("tanh"),
            "sqrt" | "raiz" | "raíz" => Some("sqrt"),
            "cbrt" => Some("cbrt"),
            "abs" => Some("abs"),
            "exp" => Some("exp"),
            "ln" => Some("ln"),
            "log" | "log10" => Some("log"),
            "log2" => Some("log2"),
            "floor" | "suelo" => Some("floor"),
            "ceil" | "techo" => Some("ceil"),
            "round" | "redondea" => Some("round"),
            "sign" | "signo" => Some("sign"),
            _ => None,
        };
        if let Some(nombre) = una {
            let args = self.argumentos_o_implicito()?;
            return (args.len() == 1).then_some(Nodo::Funcion { nombre, args });
        }
        let dos: Option<&'static str> = match n {
            "max" => Some("max"),
            "min" => Some("min"),
            "pow" => Some("pow"),
            "atan2" => Some("atan2"),
            "mod" => Some("mod"),
            _ => None,
        };
        if let Some(nombre) = dos {
            let args = self.argumentos()?;
            return (args.len() == 2).then_some(Nodo::Funcion { nombre, args });
        }
        if n == "si" || n == "if" {
            let args = self.argumentos()?;
            return (args.len() == 3).then_some(Nodo::Funcion { nombre: "si", args });
        }
        None
    }
}

fn binaria(op: char, a: Nodo, b: Nodo, implicita: bool) -> Nodo {
    Nodo::Binaria {
        op,
        a: Box::new(a),
        b: Box::new(b),
        implicita,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn f(texto: &str, x: f64) -> f64 {
        compilar(texto)
            .unwrap_or_else(|| panic!("no compila: {texto}"))
            .en(x)
    }

    fn cerca(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-12, "{a} != {b}");
    }

    #[test]
    fn la_aritmetica_de_siempre_y_su_precedencia() {
        cerca(f("1 + 2 * 3", 0.0), 7.0);
        cerca(f("(1 + 2) * 3", 0.0), 9.0);
        cerca(f("2^3^0", 0.0), 2.0);
        cerca(f("-2^2", 0.0), -4.0);
        cerca(f("1/2", 0.0), 0.5);
        cerca(f("0,5", 0.0), 0.5);
        cerca(f("7 % 3", 0.0), 1.0);
    }

    #[test]
    fn la_x_y_las_funciones_de_calculadora() {
        cerca(f("x^2", 3.0), 9.0);
        cerca(f("sin(pi/2)", 0.0), 1.0);
        cerca(f("sqrt(x)", 4.0), 2.0);
        cerca(f("ln(e)", 0.0), 1.0);
        cerca(f("log(100)", 0.0), 2.0);
        cerca(f("|x|", -3.0), 3.0);
        cerca(f("max(x; 5)", 2.0), 5.0);
        cerca(f("max(x, 5)", 2.0), 5.0);
        cerca(f("pow(2, x)", 3.0), 8.0);
        cerca(f("sen(0)", 0.0), 0.0);
        cerca(f("signo(0)", 0.0), 0.0);
    }

    #[test]
    fn la_multiplicacion_implicita_y_el_prefijo_y_igual() {
        cerca(f("2x", 3.0), 6.0);
        cerca(f("2(x+1)", 3.0), 8.0);
        cerca(f("(x+1)(x+1)", 3.0), 16.0);
        cerca(f("x sin(x)", 0.0), 0.0);
        cerca(f("2 sin x", std::f64::consts::FRAC_PI_2), 2.0);
        cerca(f("y = x^2", 3.0), 9.0);
        cerca(f("f(x) = x²", 3.0), 9.0);
        cerca(f("2pi", 0.0), 2.0 * std::f64::consts::PI);
    }

    #[test]
    fn una_funcion_por_partes_elige_el_trozo_que_cumple() {
        let c = compilar("x^2 si x < 0; 2x si x >= 0").unwrap();
        cerca(c.en(-2.0), 4.0);
        cerca(c.en(3.0), 6.0);
        cerca(c.en(0.0), 0.0);
        let hueco = compilar("x si x > 1").unwrap();
        assert!(hueco.en(0.0).is_nan());
        cerca(hueco.en(2.0), 2.0);
        cerca(f("si(x<0, x^2, 2x)", -2.0), 4.0);
        cerca(f("x <= 3", 3.0), 1.0);
        cerca(f("x != 3", 3.0), 0.0);
    }

    #[test]
    fn la_formula_se_escribe_en_latex_de_bolsillo() {
        assert_eq!(compilar("1/x").unwrap().latex()[0].0, "\\frac{1}{x}");
        assert_eq!(compilar("x^2 + 2x").unwrap().latex()[0].0, "x^{2} + 2x");
        assert_eq!(compilar("sqrt(x)").unwrap().latex()[0].0, "\\sqrt{x}");
        assert_eq!(compilar("sin(pi x)").unwrap().latex()[0].0, "\\sin(\\pi x)");
        assert_eq!(compilar("(x+1)^3").unwrap().latex()[0].0, "(x + 1)^{3}");
        assert_eq!(compilar("exp(-x)").unwrap().latex()[0].0, "e^{-x}");
        let partes = compilar("x^2 si x < 0; 2x si x >= 0").unwrap().latex();
        assert_eq!(partes.len(), 2);
        assert_eq!(partes[0].1.as_deref(), Some("x < 0"));
        assert_eq!(partes[1].1.as_deref(), Some("x \\ge 0"));
    }

    #[test]
    fn las_variables_se_conocen() {
        let v = |t: &str| {
            compilar(t)
                .unwrap()
                .variables()
                .into_iter()
                .collect::<String>()
        };
        assert_eq!(v("x^2"), "x");
        assert_eq!(v("sin(x) cos(y)"), "xy");
        assert_eq!(v("cos(t)"), "t");
        assert_eq!(v("2"), "");
        cerca(compilar("x + y").unwrap().evaluar(1.0, 2.0, 0.0), 3.0);
        cerca(compilar("t/2").unwrap().evaluar(0.0, 0.0, 1.0), 0.5);
    }

    #[test]
    fn lo_que_no_se_entiende_no_compila_y_lo_que_no_existe_da_nan() {
        assert!(compilar("").is_none());
        assert!(compilar("x +").is_none());
        assert!(compilar("foo(x)").is_none());
        assert!(compilar("(x").is_none());
        assert!(compilar("x $ 2").is_none());
        assert!(compilar("1.2.3").is_none());
        assert!(f("sqrt(x)", -1.0).is_nan());
        assert!(f("1/x", 0.0).is_infinite());
    }

    #[test]
    fn sin_de_x_no_es_una_condicion_y_si_con_espacios_si() {
        // «sin x» lleva la palabra «si» dentro pero no es un «si» suelto.
        cerca(f("sin x", 0.0), 0.0);
        assert_eq!(separar_condicion("x si x > 0").1.as_deref(), Some("x > 0"));
        assert_eq!(separar_condicion("x para x > 0").0, "x");
        assert!(separar_condicion("sin x").1.is_none());
    }
}
