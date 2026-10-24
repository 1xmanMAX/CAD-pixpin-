//! **Las formulas de las notas, entendidas** (13-oct-2026): el `object
//! Formulas` de Android (`motormd/Formulas.kt`), portado tal cual, con los
//! casos de su `FormulasTest.kt`.
//!
//! Una formula entre dolares (`$x^2$`) se lee a un arbol ([`Pieza`]) para
//! **componerla**: una fraccion con su raya, un exponente pequeño y subido,
//! una raiz con su tejado. Entiende el LaTeX de unas notas (`\frac`,
//! `\sqrt[3]{}`, griegas, operadores) y tambien **la escritura corriente**:
//! `1/2` es una fraccion, `sqrt(x)` o `raiz(x)` una raiz, `pi` es π, `<=` es
//! ≤. Lo que no entiende **se enseña tal cual**: antes texto de mas que
//! texto perdido.
//!
//! No es el interprete de la grafica del lienzo (`pixpin-motor2d::formula`,
//! que evalua): este solo sabe escribir.
//!
//! El editor de antes (el `RichEdit`) no puede apilar una fraccion: de aqui
//! saca [`toques`] — que subir, que bajar y que marcas esconder — para que
//! fuera del renglon del cursor `x^2` se vea x² y `a_1` se vea a₁.

/// Un trozo de formula ya entendido, listo para componer.
#[derive(Debug, Clone, PartialEq)]
pub enum Pieza {
    /// Letras, numeros y simbolos sueltos.
    Texto { texto: String, cursiva: bool },
    /// Varios seguidos, en la misma linea.
    Fila(Vec<Pieza>),
    /// Uno encima de otro, con su raya.
    Fraccion(Box<Pieza>, Box<Pieza>),
    /// Lo de arriba y lo de abajo pegados a una base: `x^2_i`.
    ConIndices {
        base: Box<Pieza>,
        arriba: Option<Box<Pieza>>,
        abajo: Option<Box<Pieza>>,
    },
    /// Una raiz, con su indice si lo lleva.
    Raiz {
        dentro: Box<Pieza>,
        indice: Option<Box<Pieza>>,
    },
    /// Entre parentesis que crecen con lo de dentro.
    Agrupado {
        dentro: Box<Pieza>,
        abre: String,
        cierra: String,
    },
    /// **Una matriz o un sistema** (`\begin{bmatrix}`, `pmatrix`,
    /// `vmatrix`, `cases`, `aligned`...): filas de celdas y lo que la abre y
    /// la cierra (`[` `]`, `(` `)`, `|` `|`, `{` sin cierre en un sistema, o
    /// nada). `izquierda`: las celdas pegadas a la izquierda (sistemas y
    /// alineados), si no centradas (matrices).
    Tabla {
        filas: Vec<Vec<Pieza>>,
        abre: String,
        cierra: String,
        izquierda: bool,
    },
    /// **Una marca que crece con lo que cubre** (15-oct-2026, para las
    /// laminas de geometria): la raya de un segmento (`\overline{AB}`), el
    /// arco (`\overset{\frown}{AC}`, `\wideparen{AC}`), la flecha de un
    /// vector (`\vec{v}`), el gorro (`\widehat{ABC}`), la tilde y el
    /// subrayado. Antes salia una letra suelta encima (`‾`) del ancho de un
    /// caracter.
    Adorno { dentro: Box<Pieza>, adorno: Adorno },
    /// **De otro color** (`\color{red}{X}`, `\textcolor{red}{X}`, o
    /// `{\color{red} X}` hasta el final del grupo). El color va como se
    /// escribio (`red`, `rojo`, `#e53935`): lo traduce quien compone.
    Color { color: String, dentro: Box<Pieza> },
    /// En negrita (`\mathbf`, `\textbf`, `\boldsymbol`, `\bm`).
    Negrita(Box<Pieza>),
    /// Dentro de un recuadro (`\boxed{}`, `\fbox{}`): el resultado final.
    Recuadro(Box<Pieza>),
    /// Algo pequeno encima de otra cosa (`\overset{a}{=}`, `\stackrel`).
    Encima {
        base: Box<Pieza>,
        arriba: Box<Pieza>,
    },
    /// Lo pequeno centrado debajo: `\underset{n}{x}`, el rotulo de
    /// `\underbrace{SS...S}_{n veces}` (18-oct).
    Debajo { base: Box<Pieza>, abajo: Box<Pieza> },
    /// **Con su tamano de display** (`\dfrac`, `\displaystyle`): una
    /// fraccion asi no encoge dentro de un renglon de texto.
    Grande(Box<Pieza>),
}

/// Las marcas de [`Pieza::Adorno`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adorno {
    /// `\overline`, `\bar`: el segmento AB.
    RayaEncima,
    /// `\overset{\frown}{}`, `\wideparen`, `\overarc`, `\arc`: el arco AC.
    ArcoEncima,
    /// `\vec`, `\overrightarrow`: el vector.
    Flecha,
    /// `\hat`, `\widehat`.
    Gorro,
    /// `\tilde`, `\widetilde`.
    Tilde,
    /// `\underline`.
    RayaDebajo,
    /// `\underbrace`: la llave de «"n" veces» (18-oct).
    LlaveDebajo,
}

fn texto(t: &str) -> Pieza {
    Pieza::Texto {
        texto: t.to_string(),
        cursiva: false,
    }
}

/// Los simbolos que se escriben con barra invertida.
const SIMBOLOS: &[(&str, &str)] = &[
    ("alpha", "α"),
    ("beta", "β"),
    ("gamma", "γ"),
    ("delta", "δ"),
    ("epsilon", "ε"),
    ("zeta", "ζ"),
    ("eta", "η"),
    ("theta", "θ"),
    ("iota", "ι"),
    ("kappa", "κ"),
    ("lambda", "λ"),
    ("mu", "μ"),
    ("nu", "ν"),
    ("xi", "ξ"),
    ("pi", "π"),
    ("rho", "ρ"),
    ("sigma", "σ"),
    ("tau", "τ"),
    ("upsilon", "υ"),
    ("phi", "φ"),
    ("chi", "χ"),
    ("psi", "ψ"),
    ("omega", "ω"),
    ("Gamma", "Γ"),
    ("Delta", "Δ"),
    ("Theta", "Θ"),
    ("Lambda", "Λ"),
    ("Xi", "Ξ"),
    ("Pi", "Π"),
    ("Sigma", "Σ"),
    ("Phi", "Φ"),
    ("Psi", "Ψ"),
    ("Omega", "Ω"),
    ("times", "×"),
    ("div", "÷"),
    ("pm", "±"),
    ("mp", "∓"),
    ("cdot", "·"),
    ("ast", "∗"),
    ("star", "⋆"),
    ("leq", "≤"),
    ("le", "≤"),
    ("geq", "≥"),
    ("ge", "≥"),
    ("neq", "≠"),
    ("ne", "≠"),
    ("approx", "≈"),
    ("equiv", "≡"),
    ("sim", "∼"),
    ("propto", "∝"),
    ("infty", "∞"),
    ("partial", "∂"),
    ("nabla", "∇"),
    ("sum", "∑"),
    ("prod", "∏"),
    ("int", "∫"),
    ("oint", "∮"),
    ("sqrt", "√"),
    ("angle", "∠"),
    ("degree", "°"),
    ("in", "∈"),
    ("notin", "∉"),
    ("subset", "⊂"),
    ("supset", "⊃"),
    ("cup", "∪"),
    ("cap", "∩"),
    ("emptyset", "∅"),
    ("forall", "∀"),
    ("exists", "∃"),
    ("neg", "¬"),
    ("land", "∧"),
    ("lor", "∨"),
    ("rightarrow", "→"),
    ("to", "→"),
    ("leftarrow", "←"),
    ("leftrightarrow", "↔"),
    ("Rightarrow", "⇒"),
    ("Leftarrow", "⇐"),
    ("ldots", "…"),
    ("cdots", "⋯"),
    ("perp", "⊥"),
    ("parallel", "∥"),
    // Lo que tambien escriben los modelos al pasar una hoja (banco del
    // 15-oct): sin esto salia con su barra.
    ("dots", "…"),
    ("dotsc", "…"),
    ("dotsb", "⋯"),
    ("vdots", "⋮"),
    ("ddots", "⋱"),
    ("circ", "∘"),
    ("prime", "′"),
    ("iff", "⇔"),
    ("implies", "⇒"),
    ("impliedby", "⇐"),
    ("mid", "|"),
    ("vert", "|"),
    ("lvert", "|"),
    ("rvert", "|"),
    ("Vert", "‖"),
    ("langle", "⟨"),
    ("rangle", "⟩"),
    ("setminus", "∖"),
    ("ell", "ℓ"),
    ("hbar", "ℏ"),
    ("therefore", "∴"),
    ("because", "∵"),
    ("triangle", "△"),
    ("square", "□"),
    ("simeq", "≃"),
    ("cong", "≅"),
    ("ll", "≪"),
    ("gg", "≫"),
    ("subseteq", "⊆"),
    ("supseteq", "⊇"),
    ("varepsilon", "ε"),
    ("vartheta", "ϑ"),
    ("varphi", "φ"),
    ("varrho", "ϱ"),
    ("varsigma", "ς"),
    ("mapsto", "↦"),
    ("longrightarrow", "⟶"),
    ("uparrow", "↑"),
    ("downarrow", "↓"),
    ("bullet", "•"),
    ("checkmark", "✓"),
    ("leqslant", "≤"),
    ("geqslant", "≥"),
    ("lceil", "⌈"),
    ("rceil", "⌉"),
    ("lfloor", "⌊"),
    ("rfloor", "⌋"),
    ("aleph", "ℵ"),
    ("Re", "ℜ"),
    ("Im", "ℑ"),
    // Geometria (laminas del 15-oct): «m∢AOB», arcos, perpendiculares. Sin
    // estos salia «m\getsCOD» o el mandato con su barra.
    ("measuredangle", "∡"),
    ("sphericalangle", "∢"),
    ("angulo", "∢"),
    ("frown", "⌢"),
    ("smile", "⌣"),
    ("gets", "←"),
    ("nparallel", "∦"),
    ("ncong", "≇"),
    ("nsim", "≁"),
    ("doteq", "≐"),
    ("odot", "⊙"),
    ("oplus", "⊕"),
    ("otimes", "⊗"),
    ("blacksquare", "■"),
    ("lozenge", "◊"),
    ("diamond", "⋄"),
    ("textdegree", "°"),
    ("cdotp", "·"),
    ("backsim", "∽"),
];

/// Los mandatos que no se ven (tamaños, estilo): se quitan.
const MUDOS: &[&str] = &[
    "left",
    "right",
    "displaystyle",
    "textstyle",
    "scriptstyle",
    "limits",
    "nolimits",
    "big",
    "Big",
    "bigg",
    "Bigg",
    "bigl",
    "bigr",
    "Bigl",
    "Bigr",
    "biggl",
    "biggr",
    "middle",
    "hline",
    "noindent",
    "centering",
    "nonumber",
    "notag",
];

/// Las letras de pizarra (`\mathbb{R}`).
const PIZARRA: &[(char, &str)] = &[
    ('R', "ℝ"),
    ('N', "ℕ"),
    ('Z', "ℤ"),
    ('Q', "ℚ"),
    ('C', "ℂ"),
    ('P', "ℙ"),
    ('E', "𝔼"),
];

/// Las palabras que van derechas y no en cursiva.
const FUNCIONES: &[&str] = &[
    "sin", "cos", "tan", "cot", "sec", "csc", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh",
    "log", "ln", "exp", "lim", "max", "min", "det", "gcd", "mod", "sen", "tg", "ctg", "cosec",
    "arcsen", "arctg", "lg", "arg", "sup", "Pr", "dim", "ker", "coth", "arcsinh", "arccosh",
    "arctanh",
];

/// Lo que se escribe de forma corriente y significa un simbolo.
const PALABRAS: &[(&str, &str)] = &[
    ("inf", "∞"),
    ("infinito", "∞"),
    ("infinity", "∞"),
    ("grados", "°"),
    ("deg", "°"),
    ("sum", "∑"),
    ("suma", "∑"),
    ("prod", "∏"),
    ("int", "∫"),
    ("raiz", "√"),
];

/// Parejas de signos que se escriben con dos teclas y significan una.
const PAREJAS: &[(&str, &str)] = &[
    ("<=", "≤"),
    (">=", "≥"),
    ("!=", "≠"),
    ("/=", "≠"),
    ("+-", "±"),
    ("-+", "∓"),
    ("==", "≡"),
    ("~=", "≈"),
    ("->", "→"),
    ("=>", "⇒"),
    ("<-", "←"),
    ("<->", "↔"),
];

fn simbolo(p: &str) -> Option<&'static str> {
    SIMBOLOS.iter().find(|(k, _)| *k == p).map(|(_, v)| *v)
}

/// **Lee una formula** a su arbol. Nunca falla: lo raro sale tal cual.
pub fn leer(formula: &str) -> Pieza {
    let mut l = Lector {
        s: formula.chars().collect(),
        i: 0,
    };
    let mut partes = vec![l.expresion(None)];
    // Lo que corta la expresion (una coma, un `)` suelto) se escribe y se
    // sigue: antes se perdia todo lo de detras («x ∈ ℝ, rang(A) = 3» se
    // quedaba en «x ∈ ℝ»).
    while l.i < l.s.len() {
        let c = l.s[l.i];
        l.i += 1;
        partes.push(texto(&if c == ',' || c == ';' {
            format!("{c} ")
        } else {
            c.to_string()
        }));
        partes.push(l.expresion(None));
    }
    if partes.len() == 1 {
        partes.pop().unwrap()
    } else {
        Pieza::Fila(partes)
    }
}

struct Lector {
    s: Vec<char>,
    i: usize,
}

impl Lector {
    fn en(&self, k: usize) -> Option<char> {
        self.s.get(k).copied()
    }

    fn es_fin(&self, fin: Option<char>) -> bool {
        self.i >= self.s.len() || (fin.is_some() && self.en(self.i) == fin)
    }

    /// Sumas, restas y comparaciones: el nivel mas suelto.
    fn expresion(&mut self, fin: Option<char>) -> Pieza {
        let mut partes = vec![self.termino(fin)];
        loop {
            // Los espacios se saltan antes de mirar el signo (`1/2 + x`).
            self.saltar_espacios();
            if self.es_fin(fin) {
                break;
            }
            let Some(signo) = self.signo_suelto() else {
                // **Dentro de un grupo** (`{(x; y)}`, 18-oct): una coma o un
                // punto y coma no lo cierra. Antes se cortaba ahi, el grupo
                // se tragaba el `;` como si fuera la `}` y la llave salia
                // suelta («F (x y)}»). Se escribe y se sigue hasta la `}`.
                match (fin, self.en(self.i)) {
                    // (Cualquier cosa que no sea el cierre: el «°» de
                    // `_{50°}` tambien dejaba la llave suelta.)
                    (Some(f), Some(c)) if c != f => {
                        self.i += 1;
                        partes.push(texto(&if c == ',' || c == ';' {
                            format!("{c} ")
                        } else {
                            c.to_string()
                        }));
                        partes.push(self.termino(fin));
                        continue;
                    }
                    _ => break,
                }
            };
            partes.push(texto(&format!(" {signo} ")));
            partes.push(self.termino(fin));
        }
        if partes.len() == 1 {
            partes.pop().unwrap()
        } else {
            Pieza::Fila(partes)
        }
    }

    fn saltar_espacios(&mut self) {
        while self.en(self.i) == Some(' ') {
            self.i += 1;
        }
    }

    fn empieza(&self, t: &str) -> bool {
        let c: Vec<char> = t.chars().collect();
        self.s.len() >= self.i + c.len() && self.s[self.i..self.i + c.len()] == c[..]
    }

    fn signo_suelto(&mut self) -> Option<String> {
        for (escrito, simbolo) in PAREJAS {
            if self.empieza(escrito) {
                self.i += escrito.chars().count();
                return Some(simbolo.to_string());
            }
        }
        let c = self.en(self.i)?;
        if "+-=<>≤≥≠≈".contains(c) {
            self.i += 1;
            return Some(c.to_string());
        }
        None
    }

    /// Lo que se multiplica y se divide. **La barra hace una fraccion**.
    fn termino(&mut self, fin: Option<char>) -> Pieza {
        let mut izquierda = self.factor(fin);
        loop {
            let antes = self.i;
            self.saltar_espacios();
            if self.es_fin(fin) {
                self.i = antes;
                break;
            }
            let c = self.s[self.i];
            if c == '/' {
                self.i += 1;
                izquierda = Pieza::Fraccion(Box::new(izquierda), Box::new(self.factor(fin)));
            } else if c == '*' {
                self.i += 1;
                let d = self.factor(fin);
                izquierda = Pieza::Fila(vec![izquierda, texto("·"), d]);
            } else if es_de_factor(c) {
                // Pegado es multiplicado: `2x`, sin signo, como en un libro.
                let d = self.factor(fin);
                izquierda = Pieza::Fila(vec![izquierda, d]);
            } else {
                self.i = antes;
                break;
            }
        }
        izquierda
    }

    /// Un factor con sus exponentes y subindices pegados.
    fn factor(&mut self, fin: Option<char>) -> Pieza {
        let mut base = self.atomo(fin);
        while let Some(c @ ('^' | '_')) = self.en(self.i) {
            self.i += 1;
            let indice = self.atomo(fin);
            // `90^\circ` y `90^{\circ}` son grados: el signo de grado, no un
            // circulito subido.
            if c == '^' && matches!(plano(&indice).trim(), "∘" | "°") {
                base = Pieza::Fila(vec![base, texto("°")]);
                continue;
            }
            base = con_indice(base, indice, c == '^');
        }
        base
    }

    /// Lo mas pequeño: un numero, una letra, un parentesis, un mandato.
    fn atomo(&mut self, fin: Option<char>) -> Pieza {
        self.saltar_espacios();
        if self.es_fin(fin) {
            return texto("");
        }
        let c = self.s[self.i];
        match c {
            '\\' => self.mandato(),
            '(' | '[' => self.agrupado(c),
            '{' => {
                self.i += 1;
                let d = self.expresion(Some('}'));
                if self.i < self.s.len() {
                    self.i += 1;
                }
                d
            }
            _ if c.is_ascii_digit() || c == '.' => self.numero(),
            _ if c.is_alphabetic() => self.palabra(),
            _ => {
                self.i += 1;
                texto(&c.to_string())
            }
        }
    }

    fn numero(&mut self) -> Pieza {
        let mut n = String::new();
        while let Some(c) = self
            .en(self.i)
            .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        {
            n.push(c);
            self.i += 1;
        }
        texto(&n)
    }

    /// `pi` sale π sin barra, `sqrt(x)` y `raiz(x)` hacen raiz, `sin(x)` sale
    /// derecho, y `xy` son dos letras que se multiplican.
    fn palabra(&mut self) -> Pieza {
        let desde = self.i;
        let mut w = String::new();
        while let Some(c) = self.en(self.i).filter(|c| c.is_alphabetic()) {
            w.push(c);
            self.i += 1;
        }
        let minus = w.to_lowercase();
        if w == "sqrt" || minus == "raiz" || minus == "raíz" {
            return Pieza::Raiz {
                dentro: Box::new(self.argumento()),
                indice: None,
            };
        }
        if FUNCIONES.contains(&minus.as_str()) {
            // `sin(3t)` conserva sus parentesis (sin ellos salia «sin3·t»);
            // `sin x` sigue igual.
            self.saltar_espacios();
            if self.en(self.i) == Some('(') {
                let a = self.agrupado('(');
                return Pieza::Fila(vec![texto(&minus), a]);
            }
            let a = self.argumento();
            return Pieza::Fila(vec![texto(&minus), texto(" "), a]);
        }
        if let Some(s) = simbolo(&w) {
            return texto(s);
        }
        if let Some((_, s)) = PALABRAS.iter().find(|(k, _)| *k == minus) {
            return texto(s);
        }
        // Letras sueltas: cada una es una variable.
        self.i = desde + 1;
        Pieza::Texto {
            texto: w.chars().next().unwrap_or(' ').to_string(),
            cursiva: true,
        }
    }

    /// Lo que va detras de una funcion o de una raiz.
    fn argumento(&mut self) -> Pieza {
        self.saltar_espacios();
        if let Some(c @ ('(' | '{')) = self.en(self.i) {
            let cierra = if c == '(' { ')' } else { '}' };
            self.i += 1;
            let dentro = self.expresion(Some(cierra));
            if self.i < self.s.len() {
                self.i += 1;
            }
            return dentro;
        }
        self.atomo(None)
    }

    /// Un grupo entre llaves, o el siguiente caracter suelto.
    fn grupo(&mut self) -> Pieza {
        let Some(c) = self.en(self.i) else {
            return texto("");
        };
        if c == '{' {
            self.i += 1;
            let dentro = self.expresion(Some('}'));
            if self.i < self.s.len() {
                self.i += 1;
            }
            return dentro;
        }
        if c == '\\' {
            return self.mandato();
        }
        self.i += 1;
        Pieza::Texto {
            texto: c.to_string(),
            cursiva: c.is_alphabetic(),
        }
    }

    fn mandato(&mut self) -> Pieza {
        self.i += 1; // la barra
        let mut palabra = String::new();
        while let Some(c) = self.en(self.i).filter(|c| c.is_alphabetic()) {
            palabra.push(c);
            self.i += 1;
        }
        match palabra.as_str() {
            "frac" | "tfrac" => {
                let a = self.grupo();
                let b = self.grupo();
                Pieza::Fraccion(Box::new(a), Box::new(b))
            }
            // La alta: del tamano de la letra aunque vaya dentro de un
            // renglon de texto.
            "dfrac" | "cfrac" => {
                let a = self.grupo();
                let b = self.grupo();
                Pieza::Grande(Box::new(Pieza::Fraccion(Box::new(a), Box::new(b))))
            }
            // **Colores** (`\color{red}{X}`; sin llaves detras, hasta el
            // final del grupo, como en LaTeX).
            "color" | "textcolor" | "colorbox" => {
                let color = self.grupo_crudo().trim().to_string();
                self.saltar_espacios();
                let dentro = if self.en(self.i) == Some('{') || palabra != "color" {
                    self.grupo()
                } else {
                    // Lo que queda del grupo; la llave que lo cierra la
                    // consume quien lo abrio.
                    self.expresion(Some('}'))
                };
                Pieza::Color {
                    color,
                    dentro: Box::new(dentro),
                }
            }
            "boxed" | "fbox" | "framebox" => {
                let crudo = self.grupo_crudo();
                let dentro = if palabra == "boxed" {
                    leer(&crudo)
                } else {
                    texto(&crudo)
                };
                Pieza::Recuadro(Box::new(dentro))
            }
            // **Arcos, rayas y flechas encima**, del ancho de lo que cubren.
            "overline" | "bar" | "widebar" | "overarc" | "wideparen" | "arc" | "overparen"
            | "vec" | "overrightarrow" | "hat" | "widehat" | "tilde" | "widetilde"
            | "underline" => {
                let base = self.grupo();
                let adorno = match palabra.as_str() {
                    "overline" | "bar" | "widebar" => Adorno::RayaEncima,
                    "vec" | "overrightarrow" => Adorno::Flecha,
                    "hat" | "widehat" => Adorno::Gorro,
                    "tilde" | "widetilde" => Adorno::Tilde,
                    "underline" => Adorno::RayaDebajo,
                    _ => Adorno::ArcoEncima,
                };
                Pieza::Adorno {
                    dentro: Box::new(base),
                    adorno,
                }
            }
            // `\underbrace{SS...S}_{"n" veces}`: la llave debajo y su rotulo
            // debajo de ella; `\underset{n}{x}`, lo pequeno debajo.
            "underbrace" => {
                let base = Pieza::Adorno {
                    dentro: Box::new(self.grupo()),
                    adorno: Adorno::LlaveDebajo,
                };
                self.saltar_espacios();
                if self.en(self.i) == Some('_') {
                    self.i += 1;
                    let abajo = self.grupo();
                    Pieza::Debajo {
                        base: Box::new(base),
                        abajo: Box::new(abajo),
                    }
                } else {
                    base
                }
            }
            "underset" => {
                let abajo = self.grupo();
                let base = self.grupo();
                Pieza::Debajo {
                    base: Box::new(base),
                    abajo: Box::new(abajo),
                }
            }
            // `\overset{\frown}{AC}` es el arco; `\overset{?}{=}`, lo
            // pequeno encima.
            "overset" | "stackrel" => {
                let arriba = self.grupo();
                let base = self.grupo();
                match plano(&arriba).trim() {
                    "⌢" => Pieza::Adorno {
                        dentro: Box::new(base),
                        adorno: Adorno::ArcoEncima,
                    },
                    "‾" | "-" | "—" => Pieza::Adorno {
                        dentro: Box::new(base),
                        adorno: Adorno::RayaEncima,
                    },
                    "→" | "⟶" => Pieza::Adorno {
                        dentro: Box::new(base),
                        adorno: Adorno::Flecha,
                    },
                    _ => Pieza::Encima {
                        base: Box::new(base),
                        arriba: Box::new(arriba),
                    },
                }
            }
            "displaystyle" => Pieza::Grande(Box::new(self.expresion(Some('}')))),
            "sqrt" => {
                let mut indice = None;
                if self.en(self.i) == Some('[') {
                    self.i += 1;
                    indice = Some(Box::new(self.expresion(Some(']'))));
                    if self.i < self.s.len() {
                        self.i += 1;
                    }
                }
                Pieza::Raiz {
                    dentro: Box::new(self.grupo()),
                    indice,
                }
            }
            p if MUDOS.contains(&p) => texto(""),
            // `\ln x` con su hueco (salia «lnx»); `\sin(x)`, `\sin^2` pegados.
            p if FUNCIONES.contains(&p) => {
                let mut k = self.i;
                while self.en(k) == Some(' ') {
                    k += 1;
                }
                match self.en(k) {
                    Some('(' | '^' | '_') | None => texto(p),
                    _ => Pieza::Fila(vec![texto(p), texto(" ")]),
                }
            }
            p if simbolo(p).is_some() => texto(simbolo(p).unwrap_or_default()),
            // Texto derecho, con sus espacios: `\text{si } x > 0`.
            "text" | "textrm" | "textup" | "mbox" | "textnormal" | "mathrm" | "operatorname"
            | "mathsf" | "texttt" | "mathtt" => texto(&self.grupo_crudo()),
            // Negrita: lo de dentro de un `\mathbf{x^2}` es formula y se
            // lee; lo de un `\textbf{}` es texto tal cual.
            "textbf" | "mathbf" | "boldsymbol" | "bm" => {
                let crudo = self.grupo_crudo();
                let dentro = if palabra != "textbf" && crudo.contains(['^', '_', '\\', '{']) {
                    leer(&crudo)
                } else if palabra == "boldsymbol" || palabra == "bm" {
                    Pieza::Texto {
                        texto: crudo,
                        cursiva: true,
                    }
                } else {
                    texto(&crudo)
                };
                Pieza::Negrita(Box::new(dentro))
            }
            "textit" | "mathit" | "emph" => Pieza::Texto {
                texto: self.grupo_crudo(),
                cursiva: true,
            },
            "mathbb" | "mathcal" | "mathscr" | "mathfrak" => {
                let crudo = self.grupo_crudo();
                let t: String = crudo
                    .chars()
                    .map(|c| {
                        PIZARRA
                            .iter()
                            .find(|(k, _)| *k == c)
                            .map_or(c.to_string(), |(_, v)| v.to_string())
                    })
                    .collect();
                texto(&t)
            }
            // Acentos de vector, media...: la marca encima.
            "dot" | "ddot" => {
                let base = self.grupo();
                let marca = if palabra == "dot" { "˙" } else { "¨" };
                Pieza::ConIndices {
                    base: Box::new(base),
                    arriba: Some(Box::new(texto(marca))),
                    abajo: None,
                }
            }
            "quad" => texto("  "),
            // `\phantom{xx}`: su hueco, sin tinta (18-oct, salia como codigo).
            "phantom" | "hphantom" | "vphantom" => {
                let n = plano(&self.grupo()).chars().count().max(1);
                texto(&" ".repeat(n))
            }
            // `\overbrace{...}^{...}`: el arco encima y lo de arriba, encima.
            "overbrace" => {
                let base = Pieza::Adorno {
                    dentro: Box::new(self.grupo()),
                    adorno: Adorno::ArcoEncima,
                };
                self.saltar_espacios();
                if self.en(self.i) == Some('^') {
                    self.i += 1;
                    let arriba = self.grupo();
                    Pieza::Encima {
                        base: Box::new(base),
                        arriba: Box::new(arriba),
                    }
                } else {
                    base
                }
            }
            "qquad" => texto("    "),
            "begin" => self.entorno(),
            "end" => {
                let _ = self.grupo_crudo();
                texto("")
            }
            "" => {
                // `\{`, `\%`...: el caracter de detras, tal cual; los
                // espacios de LaTeX (`\,` `\;` `\:` `\ `), un espacio, y `\!`
                // nada.
                let c = self.en(self.i).unwrap_or(' ');
                self.i += 1;
                match c {
                    ',' | ';' | ':' | ' ' => texto(" "),
                    '!' => texto(""),
                    // Un `\\` suelto (salto de renglon fuera de un entorno).
                    '\\' => texto(" "),
                    c => texto(&c.to_string()),
                }
            }
            // Ni idea: se enseña con su barra.
            p => texto(&format!("\\{p}")),
        }
    }

    fn agrupado(&mut self, abre: char) -> Pieza {
        let cierra = if abre == '(' { ')' } else { ']' };
        let desde = self.i;
        self.i += 1;
        let dentro = self.expresion(Some(cierra));
        // Sin cierre no hay grupo: se devuelve tal cual para no perder nada.
        if self.en(self.i) == Some(cierra) {
            self.i += 1;
            Pieza::Agrupado {
                dentro: Box::new(dentro),
                abre: abre.to_string(),
                cierra: cierra.to_string(),
            }
        } else {
            self.i = desde + 1;
            texto(&abre.to_string())
        }
    }

    /// Lo de dentro de unas llaves, sin leer (`\text{...}`), o el siguiente
    /// caracter si no hay llaves.
    fn grupo_crudo(&mut self) -> String {
        self.saltar_espacios();
        match self.en(self.i) {
            Some('{') => {
                let mut prof = 0;
                let desde = self.i + 1;
                while let Some(c) = self.en(self.i) {
                    self.i += 1;
                    match c {
                        '\\' => self.i += 1,
                        '{' => prof += 1,
                        '}' => {
                            prof -= 1;
                            if prof == 0 {
                                return self.s[desde..self.i - 1].iter().collect();
                            }
                        }
                        _ => {}
                    }
                }
                self.s[desde.min(self.s.len())..].iter().collect()
            }
            Some(c) => {
                self.i += 1;
                c.to_string()
            }
            None => String::new(),
        }
    }

    /// **Un entorno** (`\begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}`):
    /// matrices, sistemas (`cases`) y alineados, a una [`Pieza::Tabla`].
    /// Uno que no se conoce se lee como matriz sin parentesis: sus celdas
    /// salen en su sitio y no con las barras de LaTeX.
    fn entorno(&mut self) -> Pieza {
        let nombre = self.grupo_crudo();
        let base = nombre.trim_end_matches('*').to_string();
        if base == "array" || base == "tabular" {
            // La descripcion de columnas (`{cc|c}`), que no se ve.
            let _ = self.grupo_crudo();
        }
        // El cuerpo, hasta su `\end{nombre}` (con otros iguales dentro).
        let abre: Vec<char> = format!("\\begin{{{nombre}}}").chars().collect();
        let cierra: Vec<char> = format!("\\end{{{nombre}}}").chars().collect();
        let desde = self.i;
        let mut prof = 1;
        let mut fin = self.s.len();
        let mut k = self.i;
        while k < self.s.len() {
            if self.s[k..].starts_with(&abre) {
                prof += 1;
                k += abre.len();
            } else if self.s[k..].starts_with(&cierra) {
                prof -= 1;
                if prof == 0 {
                    fin = k;
                    break;
                }
                k += cierra.len();
            } else {
                k += 1;
            }
        }
        let cuerpo: String = self.s[desde..fin].iter().collect();
        self.i = (fin + cierra.len()).min(self.s.len());
        let (abre, cierra, izquierda) = match base.as_str() {
            "pmatrix" | "smallmatrix" => ("(", ")", false),
            "bmatrix" => ("[", "]", false),
            "Bmatrix" => ("{", "}", false),
            "vmatrix" => ("|", "|", false),
            "Vmatrix" => ("‖", "‖", false),
            "cases" | "dcases" | "rcases" => ("{", "", true),
            "matrix" | "array" | "tabular" => ("", "", false),
            _ => ("", "", true),
        };
        let alineado = izquierda && abre.is_empty();
        let filas: Vec<Vec<Pieza>> = partir_tabla(&cuerpo)
            .into_iter()
            .map(|celdas| {
                if alineado {
                    // `x &= 2`: el renglon entero, sin las marcas.
                    vec![leer(&celdas.join(" "))]
                } else {
                    celdas.iter().map(|c| leer(c)).collect()
                }
            })
            .collect();
        Pieza::Tabla {
            filas,
            abre: abre.into(),
            cierra: cierra.into(),
            izquierda,
        }
    }
}

/// El cuerpo de un entorno en filas (`\\`) y celdas (`&`), sin mirar dentro
/// de las llaves. Sin renglones vacios ni `\hline`.
fn partir_tabla(cuerpo: &str) -> Vec<Vec<String>> {
    let c: Vec<char> = cuerpo.chars().collect();
    let mut filas: Vec<Vec<String>> = Vec::new();
    let mut fila: Vec<String> = Vec::new();
    let mut celda = String::new();
    let mut prof = 0i32;
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '\\' if prof == 0 && c.get(i + 1) == Some(&'\\') => {
                fila.push(std::mem::take(&mut celda));
                filas.push(std::mem::take(&mut fila));
                i += 2;
                // `\\[2pt]`: el hueco extra, que no se ve.
                if c.get(i) == Some(&'[') {
                    while i < c.len() && c[i] != ']' {
                        i += 1;
                    }
                    i += 1;
                }
                continue;
            }
            '\\' => {
                celda.push('\\');
                if let Some(&d) = c.get(i + 1) {
                    celda.push(d);
                }
                i += 2;
                continue;
            }
            '{' => prof += 1,
            '}' => prof -= 1,
            '&' if prof == 0 => {
                fila.push(std::mem::take(&mut celda));
                i += 1;
                continue;
            }
            _ => {}
        }
        celda.push(c[i]);
        i += 1;
    }
    fila.push(celda);
    filas.push(fila);
    filas
        .into_iter()
        .map(|f| {
            f.into_iter()
                .map(|c| c.replace("\\hline", "").trim().to_string())
                .collect::<Vec<_>>()
        })
        .filter(|f| f.iter().any(|c| !c.is_empty()))
        .collect()
}

fn es_de_factor(c: char) -> bool {
    c.is_alphanumeric() || c == '(' || c == '{' || c == '\\' || c == '.'
}

/// Junta indices sobre la misma base: `x^2_i` es una sola cosa.
fn con_indice(base: Pieza, indice: Pieza, arriba: bool) -> Pieza {
    match base {
        Pieza::ConIndices {
            base,
            arriba: a,
            abajo: b,
        } => {
            if arriba {
                Pieza::ConIndices {
                    base,
                    arriba: Some(Box::new(indice)),
                    abajo: b,
                }
            } else {
                Pieza::ConIndices {
                    base,
                    arriba: a,
                    abajo: Some(Box::new(indice)),
                }
            }
        }
        otra => {
            if arriba {
                Pieza::ConIndices {
                    base: Box::new(otra),
                    arriba: Some(Box::new(indice)),
                    abajo: None,
                }
            } else {
                Pieza::ConIndices {
                    base: Box::new(otra),
                    arriba: None,
                    abajo: Some(Box::new(indice)),
                }
            }
        }
    }
}

/// El arbol en texto plano (el de `FormulasTest`): `(a/b)`, `x^2_i`,
/// `sqrt(x)`. Para pruebas y para leer en voz alta o copiar.
pub fn plano(p: &Pieza) -> String {
    match p {
        Pieza::Texto { texto, .. } => texto.clone(),
        Pieza::Fila(v) => v.iter().map(plano).collect(),
        Pieza::Fraccion(a, b) => format!("({}/{})", plano(a), plano(b)),
        Pieza::ConIndices {
            base,
            arriba,
            abajo,
        } => {
            let mut s = plano(base);
            if let Some(a) = arriba {
                s.push('^');
                s.push_str(&plano(a));
            }
            if let Some(b) = abajo {
                s.push('_');
                s.push_str(&plano(b));
            }
            s
        }
        Pieza::Raiz { dentro, .. } => format!("sqrt({})", plano(dentro)),
        Pieza::Agrupado {
            dentro,
            abre,
            cierra,
        } => format!("{abre}{}{cierra}", plano(dentro)),
        Pieza::Tabla {
            filas,
            abre,
            cierra,
            ..
        } => {
            let f: Vec<String> = filas
                .iter()
                .map(|f| f.iter().map(plano).collect::<Vec<_>>().join(", "))
                .collect();
            format!("{abre}{}{cierra}", f.join("; "))
        }
        // Como antes (y como Android): la marca de indice de arriba.
        Pieza::Adorno { dentro, adorno } => format!(
            "{}^{}",
            plano(dentro),
            match adorno {
                Adorno::RayaEncima => "‾",
                Adorno::ArcoEncima => "⌒",
                Adorno::Flecha => "→",
                Adorno::Gorro => "^",
                Adorno::Tilde => "~",
                Adorno::RayaDebajo => "_",
                Adorno::LlaveDebajo => "",
            }
        ),
        Pieza::Color { dentro, .. } | Pieza::Negrita(dentro) | Pieza::Grande(dentro) => {
            plano(dentro)
        }
        Pieza::Recuadro(dentro) => format!("[{}]", plano(dentro)),
        Pieza::Encima { base, arriba } => format!("{}^{}", plano(base), plano(arriba)),
        Pieza::Debajo { base, abajo } => format!("{} ({})", plano(base), plano(abajo)),
    }
}

/// **Los mandatos que el lector no entendio** (salen con su barra): para
/// avisar al asistente y para las pruebas del caso negativo.
pub fn desconocidos(p: &Pieza) -> Vec<String> {
    match p {
        Pieza::Texto { texto, .. } if texto.starts_with('\\') && texto.len() > 1 => {
            vec![texto.clone()]
        }
        Pieza::Texto { .. } => Vec::new(),
        Pieza::Fila(l) => l.iter().flat_map(desconocidos).collect(),
        Pieza::Fraccion(a, b)
        | Pieza::Encima { base: a, arriba: b }
        | Pieza::Debajo { base: a, abajo: b } => [desconocidos(a), desconocidos(b)].concat(),
        Pieza::ConIndices {
            base,
            arriba,
            abajo,
        } => {
            let mut v = desconocidos(base);
            v.extend(arriba.iter().flat_map(|a| desconocidos(a)));
            v.extend(abajo.iter().flat_map(|a| desconocidos(a)));
            v
        }
        Pieza::Raiz { dentro, indice } => {
            let mut v = desconocidos(dentro);
            v.extend(indice.iter().flat_map(|a| desconocidos(a)));
            v
        }
        Pieza::Agrupado { dentro, .. }
        | Pieza::Adorno { dentro, .. }
        | Pieza::Color { dentro, .. }
        | Pieza::Negrita(dentro)
        | Pieza::Recuadro(dentro)
        | Pieza::Grande(dentro) => desconocidos(dentro),
        Pieza::Tabla { filas, .. } => filas.iter().flatten().flat_map(desconocidos).collect(),
    }
}

/// La marca de un adorno escrita detras, para el texto plano.
fn marca_de(a: Adorno) -> &'static str {
    match a {
        Adorno::RayaEncima => "\u{305}",
        Adorno::ArcoEncima => "⌒",
        Adorno::Flecha => "\u{20d7}",
        Adorno::Gorro => "\u{302}",
        Adorno::Tilde => "\u{303}",
        Adorno::RayaDebajo => "\u{332}",
        Adorno::LlaveDebajo => "",
    }
}

/// **La formula escrita para quien no sabe componerla**: el texto de
/// reserva de un texto rico del lienzo (`texto_rico::plano`), lo que ve el
/// movil viejo o Excalidraw. Se lee como se leeria en un cuaderno: `x²`,
/// `a₁`, `(2a+2b)/2`, `√(x)`, `A̅B̅`, `AC⌒`, `45°`. Distinto de [`plano`],
/// que es el de las pruebas de Android y no se toca.
pub fn legible(p: &Pieza) -> String {
    fn compuesto(p: &Pieza) -> bool {
        let s = legible(p);
        s.chars().count() > 1 && s.chars().any(|c| "+-−=·×÷ /".contains(c))
    }
    fn entre(p: &Pieza) -> String {
        if compuesto(p) {
            format!("({})", legible(p))
        } else {
            legible(p)
        }
    }
    fn alzado(s: &str, arriba: bool) -> Option<String> {
        const ARRIBA: [(char, char); 16] = [
            ('0', '⁰'),
            ('1', '¹'),
            ('2', '²'),
            ('3', '³'),
            ('4', '⁴'),
            ('5', '⁵'),
            ('6', '⁶'),
            ('7', '⁷'),
            ('8', '⁸'),
            ('9', '⁹'),
            ('+', '⁺'),
            ('-', '⁻'),
            ('n', 'ⁿ'),
            ('i', 'ⁱ'),
            ('(', '⁽'),
            (')', '⁾'),
        ];
        const ABAJO: [(char, char); 18] = [
            ('0', '₀'),
            ('1', '₁'),
            ('2', '₂'),
            ('3', '₃'),
            ('4', '₄'),
            ('5', '₅'),
            ('6', '₆'),
            ('7', '₇'),
            ('8', '₈'),
            ('9', '₉'),
            ('+', '₊'),
            ('-', '₋'),
            ('a', 'ₐ'),
            ('e', 'ₑ'),
            ('o', 'ₒ'),
            ('x', 'ₓ'),
            ('i', 'ᵢ'),
            ('n', 'ₙ'),
        ];
        let tabla: &[(char, char)] = if arriba { &ARRIBA } else { &ABAJO };
        s.chars()
            .map(|c| tabla.iter().find(|(k, _)| *k == c).map(|(_, v)| *v))
            .collect()
    }
    match p {
        Pieza::Texto { texto, .. } => texto.clone(),
        Pieza::Fila(v) => v.iter().map(legible).collect(),
        Pieza::Fraccion(a, b) => format!("{}/{}", entre(a), entre(b)),
        Pieza::ConIndices {
            base,
            arriba,
            abajo,
        } => {
            let mut s = legible(base);
            for (k, sube) in [(abajo, false), (arriba, true)] {
                if let Some(k) = k {
                    let t = legible(k);
                    match alzado(&t, sube) {
                        Some(u) => s.push_str(&u),
                        None => s.push_str(&format!("{}({t})", if sube { '^' } else { '_' })),
                    }
                }
            }
            s
        }
        Pieza::Raiz { dentro, indice } => {
            let i = indice
                .as_ref()
                .map(|i| alzado(&legible(i), true).unwrap_or_default())
                .unwrap_or_default();
            format!("{i}√{}", entre(dentro))
        }
        Pieza::Agrupado {
            dentro,
            abre,
            cierra,
        } => format!("{abre}{}{cierra}", legible(dentro)),
        Pieza::Tabla {
            filas,
            abre,
            cierra,
            ..
        } => {
            let f: Vec<String> = filas
                .iter()
                .map(|f| f.iter().map(legible).collect::<Vec<_>>().join(", "))
                .collect();
            format!("{abre}{}{cierra}", f.join("; "))
        }
        Pieza::Adorno {
            dentro,
            adorno: Adorno::ArcoEncima,
        } => format!("{}⌒", legible(dentro)),
        Pieza::Adorno { dentro, adorno } => {
            // Una marca combinante detras de cada letra: A̅B̅.
            let m = marca_de(*adorno);
            legible(dentro).chars().map(|c| format!("{c}{m}")).collect()
        }
        Pieza::Color { dentro, .. }
        | Pieza::Negrita(dentro)
        | Pieza::Grande(dentro)
        | Pieza::Recuadro(dentro) => legible(dentro),
        Pieza::Encima { base, arriba } => format!(
            "{}{}",
            legible(base),
            alzado(&legible(arriba), true).unwrap_or_default()
        ),
        Pieza::Debajo { base, abajo } => format!("{} ({})", legible(base), legible(abajo)),
    }
}

/// Lo que el `RichEdit` hace con un trozo de la formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toque {
    /// Pequeño y subido (exponente, numerador de una `a/b`).
    Arriba,
    /// Pequeño y bajado (subindice, denominador).
    Abajo,
    /// Una marca que no se ve (`^`, `_`, las llaves de un grupo).
    Oculto,
}

/// **Los toques de una formula para el editor de antes**: posiciones en
/// UTF-16 dentro del texto de la formula (sin sus dolares). Exponentes y
/// subindices suben o bajan con sus marcas escondidas; una fraccion
/// sencilla `a/b` queda como ᵃ/ᵦ. Lo que no se sabe tocar se deja.
pub fn toques(formula: &str) -> Vec<(usize, usize, Toque)> {
    let c: Vec<char> = formula.chars().collect();
    // Posicion UTF-16 de cada caracter.
    let mut u = Vec::with_capacity(c.len() + 1);
    let mut n = 0;
    for ch in &c {
        u.push(n);
        n += ch.len_utf16();
    }
    u.push(n);
    let mut v = Vec::new();
    // Lo que va en un indice: un grupo entre llaves o un caracter.
    let indice = |i: usize| -> Option<(usize, usize, bool)> {
        match c.get(i)? {
            '{' => {
                let mut prof = 0;
                for (j, ch) in c.iter().enumerate().skip(i) {
                    match ch {
                        '{' => prof += 1,
                        '}' => {
                            prof -= 1;
                            if prof == 0 {
                                return Some((i, j + 1, true));
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
            ch if ch.is_alphanumeric() => {
                // Un numero entero de golpe (`x^10` no, ese pide llaves: uno solo, como LaTeX).
                Some((i, i + 1, false))
            }
            _ => None,
        }
    };
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            m @ ('^' | '_') => {
                let toque = if m == '^' {
                    Toque::Arriba
                } else {
                    Toque::Abajo
                };
                if let Some((a, b, llaves)) = indice(i + 1) {
                    v.push((u[i], u[i + 1], Toque::Oculto));
                    if llaves {
                        v.push((u[a], u[a + 1], Toque::Oculto));
                        v.push((u[a + 1], u[b - 1], toque));
                        v.push((u[b - 1], u[b], Toque::Oculto));
                    } else {
                        v.push((u[a], u[b], toque));
                    }
                    i = b;
                    continue;
                }
                i += 1;
            }
            '/' => {
                // `a/b` con un trozo de letras o cifras a cada lado, pegado.
                let es = |ch: char| ch.is_alphanumeric() || ch == '.';
                let mut a = i;
                while a > 0 && es(c[a - 1]) {
                    a -= 1;
                }
                let mut b = i + 1;
                while b < c.len() && es(c[b]) {
                    b += 1;
                }
                if a < i && b > i + 1 && !v.iter().any(|(x, y, _)| *x < u[i] && *y > u[a]) {
                    v.push((u[a], u[i], Toque::Arriba));
                    v.push((u[i + 1], u[b], Toque::Abajo));
                    i = b;
                    continue;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    v
}

#[cfg(test)]
mod pruebas {
    //! Los casos de `FormulasTest.kt` de Android, uno a uno.
    use super::*;

    /// El arbol en texto plano, sin los espacios que pone el compositor.
    fn t(p: &Pieza) -> String {
        plano(p).replace(' ', "")
    }

    fn l(s: &str) -> Pieza {
        leer(s)
    }

    #[test]
    fn un_exponente_se_cuelga_de_su_base() {
        let p = l("x^2");
        assert!(matches!(p, Pieza::ConIndices { .. }), "salio {p:?}");
        assert_eq!(t(&p), "x^2");
    }

    #[test]
    fn el_exponente_entre_llaves_puede_ser_largo() {
        assert_eq!(t(&l("x^{n+1}")), "x^n+1");
    }

    #[test]
    fn arriba_y_abajo_a_la_vez_sobre_la_misma_base() {
        let Pieza::ConIndices { arriba, abajo, .. } = l("x^2_i") else {
            panic!()
        };
        assert_eq!(t(&arriba.unwrap()), "2");
        assert_eq!(t(&abajo.unwrap()), "i");
    }

    #[test]
    fn un_subindice_solo() {
        let Pieza::ConIndices { arriba, abajo, .. } = l("a_1") else {
            panic!()
        };
        assert_eq!(t(&abajo.unwrap()), "1");
        assert!(arriba.is_none());
    }

    #[test]
    fn una_fraccion_tiene_arriba_y_abajo() {
        let Pieza::Fraccion(a, b) = l("\\frac{a}{b}") else {
            panic!()
        };
        assert_eq!(t(&a), "a");
        assert_eq!(t(&b), "b");
    }

    #[test]
    fn una_fraccion_dentro_de_otra() {
        let p = l("\\frac{\\frac{a}{b}}{c}");
        let Pieza::Fraccion(a, _) = &p else { panic!() };
        assert!(matches!(**a, Pieza::Fraccion(..)));
        assert_eq!(t(&p), "((a/b)/c)");
    }

    #[test]
    fn la_fraccion_sin_llaves_coge_un_caracter_de_cada_lado() {
        assert_eq!(t(&l("\\frac12")), "(1/2)");
    }

    #[test]
    fn una_raiz_normal_y_una_con_indice() {
        assert_eq!(t(&l("\\sqrt{x+1}")), "sqrt(x+1)");
        let Pieza::Raiz { dentro, indice } = l("\\sqrt[3]{x}") else {
            panic!()
        };
        assert_eq!(t(&indice.unwrap()), "3");
        assert_eq!(t(&dentro), "x");
    }

    #[test]
    fn las_letras_griegas_y_los_operadores_salen_como_simbolos() {
        assert_eq!(t(&l("\\pi")), "π");
        assert_eq!(t(&l("\\leq")), "≤");
        assert_eq!(t(&l("\\sum")), "∑");
        assert_eq!(t(&l("\\infty")), "∞");
        assert_eq!(t(&l("\\Delta")), "Δ");
    }

    #[test]
    fn las_funciones_se_quedan_como_palabra() {
        assert_eq!(t(&l("\\sin")), "sin");
        assert_eq!(t(&l("\\log")), "log");
    }

    #[test]
    fn un_mandato_desconocido_se_ensena_tal_cual() {
        assert!(t(&l("\\loquesea")).contains("loquesea"));
    }

    #[test]
    fn las_letras_van_en_cursiva_y_los_numeros_no() {
        let Pieza::Fila(v) = l("2x") else { panic!() };
        assert!(matches!(&v[0], Pieza::Texto { cursiva: false, .. }));
        assert!(matches!(&v[1], Pieza::Texto { cursiva: true, .. }));
    }

    #[test]
    fn una_formula_completa_se_entiende_entera() {
        let p = t(&l("x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}"));
        assert!(p.contains('±'), "{p}");
        assert!(p.contains("sqrt("), "{p}");
        assert!(p.contains("b^2"), "{p}");
        assert!(p.contains("/2"), "{p}");
    }

    #[test]
    fn los_parentesis_agrupan() {
        let Pieza::ConIndices { base, arriba, .. } = l("(a+b)^2") else {
            panic!()
        };
        assert!(matches!(*base, Pieza::Agrupado { .. }));
        assert_eq!(t(&arriba.unwrap()), "2");
    }

    #[test]
    fn la_barra_hace_una_fraccion() {
        let p = l("1/2");
        assert!(matches!(p, Pieza::Fraccion(..)), "salio {p:?}");
        assert_eq!(t(&p), "(1/2)");
    }

    #[test]
    fn la_fraccion_se_ata_mas_fuerte_que_la_suma() {
        let p = l("1/2 + x");
        let Pieza::Fila(v) = &p else { panic!() };
        assert!(matches!(v[0], Pieza::Fraccion(..)));
        assert!(t(&p).contains("(1/2)"));
    }

    #[test]
    fn los_exponentes_se_escriben_con_el_circunflejo_de_siempre() {
        assert_eq!(t(&l("x^2")), "x^2");
        assert_eq!(t(&l("x^{10}")), "x^10");
    }

    #[test]
    fn las_palabras_corrientes_hacen_su_simbolo() {
        assert_eq!(t(&l("pi")), "π");
        assert_eq!(t(&l("inf")), "∞");
        assert_eq!(t(&l("alpha")), "α");
    }

    #[test]
    fn raiz_y_sqrt_hacen_lo_mismo() {
        assert!(matches!(l("raiz(9)"), Pieza::Raiz { .. }));
        assert!(matches!(l("sqrt(9)"), Pieza::Raiz { .. }));
        assert_eq!(t(&l("raiz(x+1)")), "sqrt(x+1)");
    }

    #[test]
    fn las_parejas_de_signos_se_convierten() {
        assert!(t(&l("a <= b")).contains('≤'));
        assert!(t(&l("a != b")).contains('≠'));
        assert!(t(&l("a +- b")).contains('±'));
        assert!(t(&l("a -> b")).contains('→'));
    }

    #[test]
    fn pegado_significa_multiplicado() {
        let p = t(&l("2x"));
        assert_eq!(p, "2x");
        assert!(!p.contains('·'));
    }

    #[test]
    fn el_asterisco_si_pinta_su_punto() {
        assert!(t(&l("2*x")).contains('·'));
    }

    #[test]
    fn una_funcion_sale_derecha_y_con_su_argumento() {
        let p = t(&l("sin(x)"));
        assert!(p.starts_with("sin") && p.contains('x'), "{p}");
    }

    #[test]
    fn el_latex_de_antes_sigue_funcionando() {
        assert_eq!(t(&l("\\frac{a}{b}")), "(a/b)");
        assert_eq!(t(&l("\\pi")), "π");
        assert!(matches!(l("\\sqrt{x}"), Pieza::Raiz { .. }));
    }

    #[test]
    fn una_formula_de_verdad_escrita_a_lo_facil() {
        let p = t(&l("x = (-b +- sqrt(b^2 - 4ac)) / (2a)"));
        assert!(
            p.contains('±') && p.contains("sqrt(") && p.contains("b^2"),
            "{p}"
        );
    }

    #[test]
    fn nada_de_esto_revienta() {
        for s in [
            "", "^", "_", "{", "}", "\\", "\\frac", "\\sqrt[", "((((", "x^^2", "a/", "/b", "x_{",
        ] {
            leer(s);
            toques(s);
        }
    }

    #[test]
    fn una_llave_sin_cerrar_no_pierde_lo_de_dentro() {
        assert!(t(&l("x^{2")).contains('2'));
    }

    // ---- Los toques del editor de antes ----

    #[test]
    fn el_exponente_sube_y_sus_marcas_se_esconden() {
        assert_eq!(
            toques("x^2"),
            vec![(1, 2, Toque::Oculto), (2, 3, Toque::Arriba)]
        );
        assert_eq!(
            toques("x^{n+1}_i"),
            vec![
                (1, 2, Toque::Oculto),
                (2, 3, Toque::Oculto),
                (3, 6, Toque::Arriba),
                (6, 7, Toque::Oculto),
                (7, 8, Toque::Oculto),
                (8, 9, Toque::Abajo)
            ]
        );
        assert_eq!(
            toques("1/2"),
            vec![(0, 1, Toque::Arriba), (2, 3, Toque::Abajo)]
        );
    }

    #[test]
    fn sin_nada_que_tocar_no_se_toca() {
        assert!(toques("a + b").is_empty());
        // Un circunflejo sin nada detras, o una barra suelta, se dejan a la vista.
        assert!(toques("x^").is_empty());
        assert!(toques("a / b").is_empty());
        assert!(toques("x^{2").is_empty());
        // Las posiciones van en UTF-16 (una letra de dos unidades delante).
        assert_eq!(
            toques("𝑥^2"),
            vec![(2, 3, Toque::Oculto), (3, 4, Toque::Arriba)]
        );
    }

    // ---- Lo que faltaba (banco del 15-oct): salia con su barra ----

    /// Ningun mandato a la vista en lo leido.
    fn sin_barras(s: &str) -> String {
        let p = plano(&l(s));
        assert!(!p.contains('\\'), "{s} -> {p}");
        p
    }

    /// La ultima pieza de una fila (o ella misma).
    fn ultima(p: Pieza) -> Pieza {
        match p {
            Pieza::Fila(mut v) => v.pop().unwrap(),
            p => p,
        }
    }

    #[test]
    fn el_texto_y_los_nombres_salen_derechos_y_con_sus_espacios() {
        assert_eq!(sin_barras(r"\text{si } x > 0").replace(' ', ""), "six>0");
        assert!(matches!(
            l(r"\mathrm{d}"),
            Pieza::Texto { cursiva: false, .. }
        ));
        assert_eq!(sin_barras(r"\operatorname{rang}(A)"), "rang(A)");
        assert_eq!(sin_barras(r"\mathbf{v}"), "v");
        assert_eq!(sin_barras(r"\mathbb{R}"), "ℝ");
        assert!(sin_barras(r"\int_0^{\infty} e^{-x}\,dx").contains('∞'));
        assert_eq!(t(&l(r"\ln x")), "lnx");
        assert!(sin_barras(r"\vec{F} = m\vec{a}").contains('→'));
        assert!(sin_barras(r"\left( \frac{a}{b} \right)").contains("(a/b)"));
        assert!(!sin_barras(r"a\,b\;c\quad d").contains(','));
        // Lo de detras de una coma no se pierde.
        assert!(sin_barras(r"x \in \mathbb{R}, y = 2").contains("y = 2"));
    }

    #[test]
    fn las_matrices_salen_como_tabla_con_sus_corchetes() {
        let p = ultima(l(
            r"A = \begin{bmatrix} 2 & -1 & 0 \\ 1 & 3 & 2 \\ 0 & 1 & 4 \end{bmatrix}",
        ));
        let Pieza::Tabla {
            filas,
            abre,
            cierra,
            izquierda,
        } = &p
        else {
            panic!("{p:?}")
        };
        assert_eq!(
            (abre.as_str(), cierra.as_str(), *izquierda),
            ("[", "]", false)
        );
        assert_eq!(filas.len(), 3);
        assert!(filas.iter().all(|f| f.len() == 3));
        assert_eq!(t(&filas[0][1]), "-1");
        for (env, a, c) in [
            ("pmatrix", "(", ")"),
            ("vmatrix", "|", "|"),
            ("matrix", "", ""),
        ] {
            let Pieza::Tabla {
                abre,
                cierra,
                filas,
                ..
            } = l(&format!(r"\begin{{{env}}} a & b \\ c & d \end{{{env}}}"))
            else {
                panic!()
            };
            assert_eq!(
                (abre.as_str(), cierra.as_str(), filas.len()),
                (a, c, 2),
                "{env}"
            );
        }
    }

    #[test]
    fn un_sistema_con_cases_lleva_llave_y_va_a_la_izquierda() {
        let Pieza::Tabla {
            filas,
            abre,
            cierra,
            izquierda,
        } = l(r"\begin{cases} 2x + y = 5 \\ x - y = 1 \end{cases}")
        else {
            panic!()
        };
        assert_eq!((abre.as_str(), cierra.as_str(), izquierda), ("{", "", true));
        assert_eq!(filas.len(), 2);
        assert_eq!(t(&filas[1][0]), "x-y=1");
        // Con condiciones: dos celdas por fila.
        let Pieza::Tabla { filas, .. } = ultima(l(
            r"f(x) = \begin{cases} x^2 & \text{si } x < 0 \\ x & \text{si } x \ge 0 \end{cases}",
        )) else {
            panic!()
        };
        assert_eq!(filas[0].len(), 2);
        // Un alineado: cada renglon entero, sin los `&`.
        let Pieza::Tabla { filas, abre, .. } = l(r"\begin{aligned} x &= 2 \\ y &= 3 \end{aligned}")
        else {
            panic!()
        };
        assert!(abre.is_empty() && filas.len() == 2 && filas[0].len() == 1);
        assert_eq!(t(&filas[0][0]), "x=2");
    }

    #[test]
    fn las_funciones_con_parentesis_los_conservan() {
        assert_eq!(t(&l("2 sin(3t)")), "2sin(3t)");
    }
}
