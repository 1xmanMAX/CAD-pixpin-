//! **Las tablas de una nota Markdown** (H12): la tabla de barras de GFM que
//! escribe y lee el movil (`motormd/Tablas.kt`), y su paso a las tablas de
//! verdad del `RichEdit` de Windows y de vuelta.
//!
//! # Lo que se guarda
//!
//! Lo mismo que escribe el movil para una tabla sin celdas unidas
//! (`Tablas.aMarkdown`): una fila por renglon entre barras, con la fila de
//! guiones debajo de la primera y la alineacion en los dos puntos:
//!
//! ```text
//! | Objetivo | Tecnica |
//! |:---|:---|
//! | OE1 | Pareto |
//! ```
//!
//! Una barra dentro de una celda va como `\|` (si no, partiria la fila en
//! dos) y una celda no tiene saltos de renglon: el movil no los sabe leer
//! dentro de una fila, asi que uno pegado en una celda se guarda como
//! espacio.
//!
//! Una tabla que las barras no saben decir —celdas combinadas, colores,
//! una celda alineada distinto que su columna— se guarda como la guarda el
//! movil (`Tablas.aHtml`): un `<table>` minimo con `colspan`/`rowspan`,
//! `<th>`, `align` y, para los colores, `style="background:#…;color:#…"`
//! (ver [`crate::md_tabla_html`]). Las del movil se abren asi editables. Una
//! con titulo (`<caption>`) se queda como texto, que es la promesa de
//! siempre («antes texto de mas que texto perdido»).
//!
//! # Lo que ve el control
//!
//! En el `RichEdit` una tabla es texto con tres marcas (`EM_INSERTTABLE`,
//! texto crudo de `GT_RAWTEXT`): cada fila empieza con [`FILA_ABRE`] y un
//! salto, cada celda acaba con [`CELDA`] y la fila acaba con [`FILA_CIERRA`]
//! y un salto. [`para_control`] parte el Markdown en texto y tablas para
//! meterlo, y [`de_control`] lee lo que hay en el control y lo devuelve a
//! Markdown. Asi el resto del texto sigue siendo Markdown letra por letra,
//! como hasta ahora (ver `md_vivo`).
//!
//! Todo en unidades UTF-16, que son las posiciones del control; `\r` y `\n`
//! cuentan igual.

/// Lo que abre una fila de tabla en el texto del `RichEdit` (seguido de un
/// salto de renglon).
pub const FILA_ABRE: char = '\u{FFF9}';
/// Lo que cierra una fila (seguido de un salto de renglon).
pub const FILA_CIERRA: char = '\u{FFFB}';
/// Lo que cierra cada celda.
pub const CELDA: char = '\u{0007}';
/// Lo que el control pone en una celda tapada por una combinada (la que
/// queda debajo o a la derecha de la que manda): no es texto de nadie.
pub const TAPADA: char = '\u{FFFF}';
/// Un salto de renglon dentro de una celda (`\line` de RTF): el texto de
/// una combinada lo lleva entre los textos que junta, como en el movil.
pub const SALTO_EN_CELDA: char = '\u{000B}';

/// Un color `0xRRGGBB`.
pub type Rgb = u32;

/// Como se alinea una columna: los dos puntos de la fila de guiones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alineacion {
    #[default]
    Izquierda,
    Centro,
    Derecha,
}

/// A que altura va el texto de una celda (`valign` del movil,
/// `AlturaEnCelda`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vertical {
    #[default]
    Arriba,
    Medio,
    Abajo,
}

/// Lo que una celda lleva ademas de su texto. Es la `Celda` de
/// `motormd/Markdown.kt` (cabecera, alineacion, altura, `colspan`,
/// `rowspan`) mas los dos colores, que el movil lee y deja estar.
///
/// La tabla guarda la **rejilla entera**: una combinada es su celda de
/// arriba a la izquierda (la que manda, con `columnas`/`filas` > 1) y las
/// que tapa siguen en su sitio, vacias y `tapada`. El movil guarda solo las
/// que mandan; la rejilla es la de su `Tablas.rejilla`, y es lo que tiene el
/// control de Windows (cada celda tapada sigue ahi, con [`TAPADA`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Formato {
    /// Cuantas columnas ocupa (`colspan`).
    pub columnas: usize,
    /// Cuantas filas ocupa (`rowspan`).
    pub filas: usize,
    /// La tapa otra combinada.
    pub tapada: bool,
    pub fondo: Option<Rgb>,
    pub letra: Option<Rgb>,
    /// La suya, si no es la de su columna.
    pub alineacion: Option<Alineacion>,
    /// Si es de cabecera (`<th>`) y no lo dice su fila (la primera lo es).
    pub cabecera: Option<bool>,
    pub vertical: Vertical,
}

impl Default for Formato {
    fn default() -> Formato {
        Formato {
            columnas: 1,
            filas: 1,
            tapada: false,
            fondo: None,
            letra: None,
            alineacion: None,
            cabecera: None,
            vertical: Vertical::Arriba,
        }
    }
}

impl Formato {
    fn tapada() -> Formato {
        Formato {
            tapada: true,
            ..Formato::default()
        }
    }

    /// La pinta de una celda (colores, alineacion, cabecera, altura) sin
    /// lo de combinar: lo que heredan las que salen al separar.
    fn pinta(&self) -> Formato {
        Formato {
            columnas: 1,
            filas: 1,
            tapada: false,
            ..*self
        }
    }
}

/// Hasta donde crece una tabla al pegar en ella (una hoja entera pegada
/// por error no debe colgar el editor).
pub const MAX_FILAS: usize = 1000;
pub const MAX_COLUMNAS: usize = 50;

/// Una tabla: filas de celdas (la primera es la cabecera) con el texto de
/// cada una tal cual, en Markdown (`**x**` sigue siendo negrita dentro).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tabla {
    pub filas: Vec<Vec<String>>,
    /// Una por columna; las que falten son a la izquierda.
    pub alineaciones: Vec<Alineacion>,
    /// Lo de cada celda que no es texto, con la forma de `filas`. Vacio si
    /// ninguna lleva nada (la tabla de barras de siempre).
    pub formato: Vec<Vec<Formato>>,
}

impl Tabla {
    /// Una tabla vacia de `filas` x `columnas`, con los topes del movil
    /// (`Tablas.nueva`: de 2 a 20 filas, de 1 a 12 columnas).
    pub fn nueva(filas: usize, columnas: usize) -> Tabla {
        let f = filas.clamp(2, 20);
        let c = columnas.clamp(1, 12);
        Tabla {
            filas: vec![vec![String::new(); c]; f],
            alineaciones: vec![Alineacion::Izquierda; c],
            formato: Vec::new(),
        }
    }

    /// Cuantas columnas tiene: las de su fila mas larga, y al menos una.
    pub fn columnas(&self) -> usize {
        self.filas.iter().map(Vec::len).max().unwrap_or(0).max(1)
    }

    pub fn alineacion(&self, c: usize) -> Alineacion {
        self.alineaciones.get(c).copied().unwrap_or_default()
    }

    /// Lo de la celda `(f, c)`; lo de siempre si no lleva nada.
    pub fn formato(&self, f: usize, c: usize) -> Formato {
        self.formato
            .get(f)
            .and_then(|x| x.get(c))
            .copied()
            .unwrap_or_default()
    }

    /// Todas las filas con todas sus celdas, y el formato con la misma
    /// forma: lo que necesitan las operaciones que tocan formato.
    fn cuadrar(&mut self) {
        let n = self.columnas();
        for f in &mut self.filas {
            f.resize(n, String::new());
        }
        self.formato.resize(self.filas.len(), Vec::new());
        for f in &mut self.formato {
            f.resize(n, Formato::default());
        }
        self.alineaciones.resize(n, Alineacion::Izquierda);
    }

    /// Lo de la celda `(f, c)`, para cambiarlo.
    pub fn formato_mut(&mut self, f: usize, c: usize) -> &mut Formato {
        self.cuadrar();
        &mut self.formato[f][c]
    }

    /// Sin formato si ninguna celda lleva nada: asi una tabla de barras
    /// leida y vuelta a escribir es igual a la de antes.
    /// **La forma unica** de decir las alineaciones: la de cada columna es
    /// la de su celda de la primera fila (la combinada que la tapa, si la
    /// hay), y una celda solo lleva la suya si es otra. Dos tablas que se
    /// ven igual quedan iguales; es como la lee el control y el HTML.
    pub fn canonica(&mut self) {
        if self.filas.is_empty() {
            return;
        }
        let n = self.columnas();
        let de_celda: Vec<Vec<Alineacion>> = (0..self.filas.len())
            .map(|f| (0..n).map(|c| self.alineacion_de(f, c)).collect())
            .collect();
        let de_columna: Vec<Alineacion> = (0..n)
            .map(|c| {
                let (af, ac) = self.ancla(0, c);
                de_celda[af][ac]
            })
            .collect();
        self.cuadrar();
        self.alineaciones = de_columna;
        for (f, fila) in de_celda.iter().enumerate() {
            for (c, a) in fila.iter().enumerate() {
                let x = &mut self.formato[f][c];
                x.alineacion = (!x.tapada && *a != self.alineaciones[c]).then_some(*a);
            }
        }
        self.normalizar();
    }

    pub fn normalizar(&mut self) {
        if self
            .formato
            .iter()
            .flatten()
            .all(|x| *x == Formato::default())
        {
            self.formato.clear();
        }
    }

    /// Como se alinea la celda `(f, c)`: la suya o la de su columna.
    pub fn alineacion_de(&self, f: usize, c: usize) -> Alineacion {
        self.formato(f, c)
            .alineacion
            .unwrap_or_else(|| self.alineacion(c))
    }

    /// Si `(f, c)` es de cabecera: la primera fila, salvo que diga otra cosa.
    pub fn es_cabecera(&self, f: usize, c: usize) -> bool {
        self.formato(f, c).cabecera.unwrap_or(f == 0)
    }

    /// La celda que manda sobre `(f, c)`: ella misma si no esta tapada.
    pub fn ancla(&self, f: usize, c: usize) -> (usize, usize) {
        if !self.formato(f, c).tapada {
            return (f, c);
        }
        for af in (0..=f).rev() {
            for ac in (0..=c).rev() {
                let x = self.formato(af, ac);
                if !x.tapada && af + x.filas > f && ac + x.columnas > c {
                    return (af, ac);
                }
            }
        }
        (f, c)
    }

    /// **Si hace falta HTML para escribirla** (`Tabla.esAvanzada` del
    /// movil): celdas combinadas, colores, cabeceras fuera de la primera
    /// fila, una altura que no sea la de siempre, o una celda alineada
    /// distinto que la de arriba de su columna (las barras solo saben una
    /// alineacion por columna).
    pub fn es_avanzada(&self) -> bool {
        let n = self.columnas();
        (0..self.filas.len()).any(|f| {
            (0..n).any(|c| {
                let x = self.formato(f, c);
                x.tapada
                    || x.columnas > 1
                    || x.filas > 1
                    || x.fondo.is_some()
                    || x.letra.is_some()
                    || x.vertical != Vertical::Arriba
                    || self.es_cabecera(f, c) != (f == 0)
                    || self.alineacion_de(f, c) != self.alineacion_de(0, c)
            })
        })
    }

    /// El rectangulo de `a` a `b` (esquinas en cualquier orden), crecido
    /// hasta cubrir enteras las combinadas que toca: si no, quedaria media
    /// dentro y media fuera (`Tablas.fusionar` del movil hace lo mismo).
    /// Filas y columnas de principio y fin, incluidas: `(f1, f2, c1, c2)`.
    pub fn rectangulo(&self, a: (usize, usize), b: (usize, usize)) -> (usize, usize, usize, usize) {
        let alto = self.filas.len().max(1);
        let ancho = self.columnas();
        let mut f1 = a.0.min(b.0).min(alto - 1);
        let mut f2 = a.0.max(b.0).min(alto - 1);
        let mut c1 = a.1.min(b.1).min(ancho - 1);
        let mut c2 = a.1.max(b.1).min(ancho - 1);
        loop {
            let antes = (f1, f2, c1, c2);
            let (de_f, a_f, de_c, a_c) = antes;
            for f in de_f..=a_f {
                for c in de_c..=a_c {
                    let (af, ac) = self.ancla(f, c);
                    let x = self.formato(af, ac);
                    f1 = f1.min(af);
                    c1 = c1.min(ac);
                    f2 = f2.max(af + x.filas - 1).min(alto - 1);
                    c2 = c2.max(ac + x.columnas - 1).min(ancho - 1);
                }
            }
            if antes == (f1, f2, c1, c2) {
                return (f1, f2, c1, c2);
            }
        }
    }

    /// **Combina** el rectangulo de `a` a `b` (`Tablas.fusionar`): el texto
    /// de todas se junta en la de arriba a la izquierda, separado por
    /// saltos, y las demas quedan tapadas. `false` si no hay nada que
    /// combinar (una sola celda).
    pub fn combinar(&mut self, a: (usize, usize), b: (usize, usize)) -> bool {
        if self.filas.is_empty() {
            return false;
        }
        let (f1, f2, c1, c2) = self.rectangulo(a, b);
        // Una sola celda, o una sola combinada entera: nada que combinar.
        let ya = self.formato(f1, c1);
        if (f1 == f2 && c1 == c2)
            || (!ya.tapada && f2 - f1 + 1 == ya.filas && c2 - c1 + 1 == ya.columnas)
        {
            return false;
        }
        self.cuadrar();
        let mut textos = Vec::new();
        for f in f1..=f2 {
            for c in c1..=c2 {
                if !self.formato[f][c].tapada && !self.filas[f][c].trim().is_empty() {
                    textos.push(self.filas[f][c].trim().to_string());
                }
            }
        }
        let ancla = Formato {
            columnas: c2 - c1 + 1,
            filas: f2 - f1 + 1,
            ..self.formato[f1][c1].pinta()
        };
        for f in f1..=f2 {
            for c in c1..=c2 {
                self.filas[f][c].clear();
                self.formato[f][c] = Formato::tapada();
            }
        }
        self.filas[f1][c1] = textos.join("\n");
        self.formato[f1][c1] = ancla;
        true
    }

    /// **Separa** la combinada que cubre `(f, c)` (`Tablas.separar`): el
    /// texto se queda en la de arriba a la izquierda y las que vuelven
    /// heredan su pinta (si era cabecera o de color, ellas tambien).
    /// `false` si no estaba combinada.
    pub fn separar(&mut self, f: usize, c: usize) -> bool {
        if f >= self.filas.len() {
            return false;
        }
        let (af, ac) = self.ancla(f, c);
        let x = self.formato(af, ac);
        if x.filas <= 1 && x.columnas <= 1 {
            return false;
        }
        self.cuadrar();
        for ff in af..(af + x.filas).min(self.filas.len()) {
            for cc in ac..(ac + x.columnas).min(self.formato[ff].len()) {
                self.formato[ff][cc] = x.pinta();
            }
        }
        self.normalizar();
        true
    }

    /// Las celdas que mandan sobre las del rectangulo de `a` a `b`, cada una
    /// una vez. Sin crecer: pintar una columna que cruza un titulo
    /// combinado pinta el titulo, no todas sus columnas.
    fn anclas_en(&self, a: (usize, usize), b: (usize, usize)) -> Vec<(usize, usize)> {
        let mut sal = Vec::new();
        if self.filas.is_empty() {
            return sal;
        }
        let (alto, ancho) = (self.filas.len(), self.columnas());
        let (f1, f2) = (a.0.min(b.0).min(alto - 1), a.0.max(b.0).min(alto - 1));
        let (c1, c2) = (a.1.min(b.1).min(ancho - 1), a.1.max(b.1).min(ancho - 1));
        for f in f1..=f2 {
            for c in c1..=c2 {
                let x = self.ancla(f, c);
                if !sal.contains(&x) {
                    sal.push(x);
                }
            }
        }
        sal
    }

    /// Pone (o quita, con `None`) el color de fondo a las celdas del
    /// rectangulo de `a` a `b`.
    pub fn poner_fondo(&mut self, a: (usize, usize), b: (usize, usize), color: Option<Rgb>) {
        for (f, c) in self.anclas_en(a, b) {
            self.formato_mut(f, c).fondo = color;
        }
        self.normalizar();
    }

    /// Lo mismo con el color de la letra.
    pub fn poner_letra(&mut self, a: (usize, usize), b: (usize, usize), color: Option<Rgb>) {
        for (f, c) in self.anclas_en(a, b) {
            self.formato_mut(f, c).letra = color;
        }
        self.normalizar();
    }

    /// Mete una fila vacia en `donde` (al final si se pasa). Si cae dentro
    /// de una combinada de varias filas, la combinada crece con ella.
    pub fn insertar_fila(&mut self, donde: usize) {
        let n = self.columnas();
        let i = donde.min(self.filas.len());
        if self.formato.is_empty() {
            self.filas.insert(i, vec![String::new(); n]);
            return;
        }
        self.cuadrar();
        let mut nueva = vec![Formato::default(); n];
        let mut crecen = Vec::new();
        if i > 0 && i < self.filas.len() {
            for (c, celda) in nueva.iter_mut().enumerate() {
                let (af, ac) = self.ancla(i, c);
                if af < i {
                    *celda = Formato::tapada();
                    if !crecen.contains(&(af, ac)) {
                        crecen.push((af, ac));
                    }
                }
            }
        }
        for (af, ac) in crecen {
            self.formato[af][ac].filas += 1;
        }
        self.filas.insert(i, vec![String::new(); n]);
        self.formato.insert(i, nueva);
    }

    /// Quita la fila `cual`. Una tabla sin filas no es una tabla, asi que
    /// la ultima no se quita (`Tablas.quitarFilas`). Una combinada que la
    /// cruza encoge, y si mandaba desde ella, pasa a mandar la de debajo.
    /// `false` si no se quito.
    pub fn quitar_fila(&mut self, cual: usize) -> bool {
        if self.filas.len() <= 1 || cual >= self.filas.len() {
            return false;
        }
        if !self.formato.is_empty() {
            self.cuadrar();
            for c in 0..self.columnas() {
                let (af, ac) = self.ancla(cual, c);
                if ac != c {
                    continue;
                }
                let x = self.formato[af][ac];
                if af < cual {
                    self.formato[af][ac].filas -= 1;
                } else if x.filas > 1 {
                    self.filas[cual + 1][ac] = std::mem::take(&mut self.filas[cual][ac]);
                    self.formato[cual + 1][ac] = Formato {
                        filas: x.filas - 1,
                        ..x
                    };
                }
            }
            self.formato.remove(cual);
        }
        self.filas.remove(cual);
        self.normalizar();
        true
    }

    /// Mete una columna vacia en `donde` (al final si se pasa); dentro de
    /// una combinada de varias columnas, la combinada crece con ella.
    pub fn insertar_columna(&mut self, donde: usize) {
        let n = self.columnas();
        let i = donde.min(n);
        if !self.formato.is_empty() {
            self.cuadrar();
            let mut tapadas = vec![false; self.filas.len()];
            if i > 0 && i < n {
                let mut crecen = Vec::new();
                for (f, tapada) in tapadas.iter_mut().enumerate() {
                    let (af, ac) = self.ancla(f, i);
                    if ac < i {
                        *tapada = true;
                        if !crecen.contains(&(af, ac)) {
                            crecen.push((af, ac));
                        }
                    }
                }
                for (af, ac) in crecen {
                    self.formato[af][ac].columnas += 1;
                }
            }
            for (fila, tapada) in self.formato.iter_mut().zip(tapadas) {
                fila.insert(
                    i,
                    if tapada {
                        Formato::tapada()
                    } else {
                        Formato::default()
                    },
                );
            }
        }
        for f in &mut self.filas {
            f.resize(n, String::new());
            f.insert(i, String::new());
        }
        self.alineaciones.resize(n, Alineacion::Izquierda);
        self.alineaciones.insert(i, Alineacion::Izquierda);
    }

    /// Quita la columna `cual`; la ultima que queda no (`quitarColumnas`).
    /// Las combinadas que la cruzan encogen como con las filas.
    pub fn quitar_columna(&mut self, cual: usize) -> bool {
        let n = self.columnas();
        if n <= 1 || cual >= n {
            return false;
        }
        if !self.formato.is_empty() {
            self.cuadrar();
            for f in 0..self.filas.len() {
                let (af, ac) = self.ancla(f, cual);
                if af != f {
                    continue;
                }
                let x = self.formato[af][ac];
                if ac < cual {
                    self.formato[af][ac].columnas -= 1;
                } else if x.columnas > 1 {
                    self.filas[af][cual + 1] = std::mem::take(&mut self.filas[af][cual]);
                    self.formato[af][cual + 1] = Formato {
                        columnas: x.columnas - 1,
                        ..x
                    };
                }
            }
            for fila in &mut self.formato {
                fila.remove(cual);
            }
        }
        for f in &mut self.filas {
            f.resize(n, String::new());
            f.remove(cual);
        }
        self.alineaciones.resize(n, Alineacion::Izquierda);
        self.alineaciones.remove(cual);
        self.normalizar();
        true
    }

    /// **Las filas que van juntas con la `f`**: las que une una combinada de
    /// varias filas (no se puede sacar media combinada). `[desde, hasta)`.
    pub fn banda_de_filas(&self, f: usize) -> (usize, usize) {
        let n = self.filas.len();
        let (mut a, mut b) = (f.min(n.saturating_sub(1)), (f + 1).min(n));
        loop {
            let antes = (a, b);
            for g in antes.0..antes.1 {
                for c in 0..self.columnas() {
                    let (af, ac) = self.ancla(g, c);
                    a = a.min(af);
                    b = b.max((af + self.formato(af, ac).filas).min(n));
                }
            }
            if (a, b) == antes {
                return (a, b);
            }
        }
    }

    /// Las columnas que van juntas con la `c` (las une una combinada).
    pub fn banda_de_columnas(&self, c: usize) -> (usize, usize) {
        let n = self.columnas();
        let (mut a, mut b) = (c.min(n - 1), (c + 1).min(n));
        loop {
            let antes = (a, b);
            for d in antes.0..antes.1 {
                for f in 0..self.filas.len() {
                    let (af, ac) = self.ancla(f, d);
                    a = a.min(ac);
                    b = b.max((ac + self.formato(af, ac).columnas).min(n));
                }
            }
            if (a, b) == antes {
                return (a, b);
            }
        }
    }

    /// Si entre la fila `i - 1` y la `i` no pasa ninguna combinada (se
    /// puede dejar algo ahi).
    pub fn corte_de_filas(&self, i: usize) -> bool {
        i == 0 || i >= self.filas.len() || (0..self.columnas()).all(|c| self.ancla(i, c).0 == i)
    }

    pub fn corte_de_columnas(&self, i: usize) -> bool {
        i == 0 || i >= self.columnas() || (0..self.filas.len()).all(|f| self.ancla(f, i).1 == i)
    }

    /// **Mueve la fila `desde`** (con las que van juntas con ella, ver
    /// [`Tabla::banda_de_filas`]) para que quede delante de la fila `a` de
    /// ahora (`a` = cuantas hay: al final). Se lleva su texto, sus colores y
    /// sus combinadas. `false` si no se movio (a su sitio, o `a` parte una
    /// combinada).
    pub fn mover_fila(&mut self, desde: usize, a: usize) -> bool {
        let n = self.filas.len();
        if desde >= n || a > n {
            return false;
        }
        let (i, j) = self.banda_de_filas(desde);
        if (i..=j).contains(&a) || !self.corte_de_filas(a) {
            return false;
        }
        if !self.formato.is_empty() {
            self.cuadrar();
        }
        let destino = if a > j { a - (j - i) } else { a };
        let filas: Vec<Vec<String>> = self.filas.drain(i..j).collect();
        self.filas.splice(destino..destino, filas);
        if !self.formato.is_empty() {
            let formato: Vec<Vec<Formato>> = self.formato.drain(i..j).collect();
            self.formato.splice(destino..destino, formato);
        }
        true
    }

    /// **Mueve la columna `desde`** delante de la `a` de ahora, como
    /// [`Tabla::mover_fila`]; tambien su alineacion.
    pub fn mover_columna(&mut self, desde: usize, a: usize) -> bool {
        let n = self.columnas();
        if desde >= n || a > n {
            return false;
        }
        let (i, j) = self.banda_de_columnas(desde);
        if (i..=j).contains(&a) || !self.corte_de_columnas(a) {
            return false;
        }
        if !self.formato.is_empty() {
            self.cuadrar();
        }
        let destino = if a > j { a - (j - i) } else { a };
        let mover = |v: &mut Vec<String>| {
            v.resize(n, String::new());
            let x: Vec<String> = v.drain(i..j).collect();
            v.splice(destino..destino, x);
        };
        self.filas.iter_mut().for_each(mover);
        for fila in &mut self.formato {
            let x: Vec<Formato> = fila.drain(i..j).collect();
            fila.splice(destino..destino, x);
        }
        self.alineaciones.resize(n, Alineacion::Izquierda);
        let x: Vec<Alineacion> = self.alineaciones.drain(i..j).collect();
        self.alineaciones.splice(destino..destino, x);
        true
    }

    /// **Pega `otra` desde la celda `(f, c)`**, como una hoja de calculo:
    /// sus textos pisan los de debajo, y sus colores y su alineacion
    /// tambien si los trae; la tabla crece en filas y columnas si no cabe
    /// (hasta [`MAX_FILAS`] x [`MAX_COLUMNAS`]). Sus combinadas no se copian
    /// (aqui mandan las de aqui) y una celda tapada de aqui no se escribe.
    pub fn pegar_en(&mut self, f: usize, c: usize, otra: &Tabla) {
        let alto = (f + otra.filas.len()).min(MAX_FILAS);
        let ancho = (c + otra.columnas()).min(MAX_COLUMNAS);
        while self.filas.len() < alto {
            self.insertar_fila(self.filas.len());
        }
        while self.columnas() < ancho {
            self.insertar_columna(self.columnas());
        }
        self.cuadrar();
        for (df, fila) in otra.filas.iter().enumerate() {
            for (dc, texto) in fila.iter().enumerate() {
                let (ff, cc) = (f + df, c + dc);
                let de = otra.formato(df, dc);
                if ff >= alto || cc >= ancho || de.tapada || self.formato[ff][cc].tapada {
                    continue;
                }
                self.filas[ff][cc] = texto.clone();
                let a = otra.alineacion_de(df, dc);
                let x = &mut self.formato[ff][cc];
                if de.fondo.is_some() {
                    x.fondo = de.fondo;
                }
                if de.letra.is_some() {
                    x.letra = de.letra;
                }
                if a != Alineacion::Izquierda {
                    x.alineacion = Some(a);
                }
            }
        }
        // Una alineacion igual a la de su columna no es de la celda.
        for ff in 0..self.filas.len() {
            for cc in 0..self.formato[ff].len() {
                if self.formato[ff][cc].alineacion == Some(self.alineacion(cc)) {
                    self.formato[ff][cc].alineacion = None;
                }
            }
        }
        self.normalizar();
    }
}

// ---------------------------------------------------------------------------
// Markdown

/// Las celdas de una fila de barras, sin las barras de los bordes y con
/// `\|` vuelto barra (`Tablas.celdasDeFila`).
fn celdas_de_fila(fila: &str) -> Vec<String> {
    let t = fila.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t
        .strip_suffix('|')
        .filter(|x| !x.ends_with('\\'))
        .unwrap_or(t);
    let mut salida = Vec::new();
    let mut actual = String::new();
    let mut letras = t.chars().peekable();
    while let Some(c) = letras.next() {
        match c {
            '\\' if letras.peek() == Some(&'|') => {
                actual.push('|');
                letras.next();
            }
            '|' => salida.push(std::mem::take(&mut actual).trim().to_string()),
            _ => actual.push(c),
        }
    }
    salida.push(actual.trim().to_string());
    salida
}

fn son_guiones(c: &str) -> bool {
    let c = c.trim();
    let c = c.strip_prefix(':').unwrap_or(c);
    let c = c.strip_suffix(':').unwrap_or(c);
    !c.is_empty() && c.chars().all(|x| x == '-')
}

fn alineacion_de(c: &str) -> Alineacion {
    let c = c.trim();
    match (c.starts_with(':'), c.ends_with(':') && c.len() > 1) {
        (true, true) => Alineacion::Centro,
        (_, true) => Alineacion::Derecha,
        _ => Alineacion::Izquierda,
    }
}

/// La fila de guiones de debajo de la cabecera (`|:---|---:|`).
pub fn es_separadora(renglon: &str) -> bool {
    let t = renglon.trim();
    t.starts_with('|') && {
        let c = celdas_de_fila(t);
        !c.is_empty() && c.iter().all(|x| son_guiones(x))
    }
}

/// Si en el renglon `i` empieza una tabla: una fila de barras con la de
/// guiones debajo. Sin exigirla, una linea suelta con barras (una ruta, una
/// cuenta) se partiria en columnas (`Markdown.esTabla`).
fn empieza_tabla(renglones: &[&str], i: usize) -> bool {
    renglones[i].trim().starts_with('|') && renglones.get(i + 1).is_some_and(|s| es_separadora(s))
}

/// Lee una tabla de barras. `None` si no lo es.
pub fn leer_gfm(texto: &str) -> Option<Tabla> {
    let renglones: Vec<&str> = texto
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('|'))
        .collect();
    if renglones.len() < 2 || !es_separadora(renglones[1]) {
        return None;
    }
    let alineaciones: Vec<Alineacion> = celdas_de_fila(renglones[1])
        .iter()
        .map(|c| alineacion_de(c))
        .collect();
    let filas: Vec<Vec<String>> = renglones
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .map(|(_, r)| celdas_de_fila(r))
        .collect();
    Some(Tabla {
        filas,
        alineaciones,
        formato: Vec::new(),
    })
}

/// El texto de una celda como va dentro de una fila: sin saltos y con la
/// barra escapada.
fn celda_a_gfm(c: &str) -> String {
    let llano: String = c
        .chars()
        .map(|x| {
            if x == '\n' || x == '\r' || x == CELDA {
                ' '
            } else {
                x
            }
        })
        .collect();
    llano.trim().replace('|', "\\|")
}

/// Escribe la tabla como la escribe el movil (`Tablas.aMarkdown`), sin salto
/// al final.
pub fn a_gfm(t: &Tabla) -> String {
    let n = t.columnas();
    let mut s = String::new();
    for (f, fila) in t.filas.iter().enumerate() {
        s.push_str("| ");
        let celdas: Vec<String> = (0..n)
            .map(|c| fila.get(c).map(|x| celda_a_gfm(x)).unwrap_or_default())
            .collect();
        s.push_str(&celdas.join(" | "));
        s.push_str(" |\n");
        if f == 0 {
            s.push('|');
            for c in 0..n {
                s.push_str(match t.alineacion(c) {
                    Alineacion::Centro => ":---:",
                    Alineacion::Derecha => "---:",
                    Alineacion::Izquierda => ":---",
                });
                s.push('|');
            }
            s.push('\n');
        }
    }
    s.pop();
    s
}

// ---------------------------------------------------------------------------
// Ida al control

/// Lo que se mete en el control: el texto con un renglon vacio donde va
/// cada tabla, y las tablas con la posicion (UTF-16) de su renglon vacio.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParaControl {
    pub texto: String,
    pub tablas: Vec<(usize, Tabla)>,
}

fn es_valla(r: &str) -> bool {
    let t = r.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// Parte el Markdown en texto y tablas. Una tabla dentro de un bloque de
/// codigo no es tabla. El hueco de cada tabla es un salto de renglon que
/// ocupa su sitio (con el salto de su ultima fila dentro).
pub fn para_control(md: &str) -> ParaControl {
    // Los renglones con su salto pegado, para no perder ni anadir ninguno.
    let mut renglones: Vec<&str> = md.split_inclusive('\n').collect();
    if renglones.is_empty() {
        renglones.push("");
    }
    let sin_salto: Vec<&str> = renglones
        .iter()
        .map(|r| r.trim_end_matches(['\n', '\r']))
        .collect();
    let mut sal = ParaControl::default();
    let mut en_codigo = false;
    let mut i = 0;
    let mut pos = 0usize;
    while i < renglones.len() {
        if es_valla(sin_salto[i]) {
            en_codigo = !en_codigo;
        }
        if !en_codigo && empieza_tabla(&sin_salto, i) {
            let mut j = i;
            while j < renglones.len() && sin_salto[j].trim().starts_with('|') {
                j += 1;
            }
            if let Some(t) = leer_gfm(&sin_salto[i..j].join("\n")) {
                sal.tablas.push((pos, t));
                sal.texto.push('\n');
                pos += 1;
                i = j;
                continue;
            }
        }
        // Una tabla en HTML, como las escribe el movil (`Markdown.kt`: un
        // renglon que empieza por `<table`, hasta el que cierra).
        if !en_codigo
            && sin_salto[i]
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("<table")
        {
            let mut j = i;
            while j < renglones.len() {
                j += 1;
                if sin_salto[j - 1].to_ascii_lowercase().contains("</table>") {
                    break;
                }
            }
            if let Some(t) = crate::md_tabla_html::leer_de_nota(&sin_salto[i..j].join("\n")) {
                sal.tablas.push((pos, t));
                sal.texto.push('\n');
                pos += 1;
                i = j;
                continue;
            }
        }
        sal.texto.push_str(renglones[i]);
        pos += renglones[i].encode_utf16().count();
        i += 1;
    }
    sal
}

// ---------------------------------------------------------------------------
// Vuelta del control

/// Una tabla tal como esta en el texto del control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TablaEnControl {
    /// Donde empieza su primera fila (la marca [`FILA_ABRE`]).
    pub desde: usize,
    /// Donde acaba: detras del salto de su ultima fila.
    pub hasta: usize,
    /// Las celdas; la alineacion no esta en el texto (la sabe el control).
    pub tabla: Tabla,
    /// Donde empieza el texto de cada celda, fila por fila.
    pub celdas: Vec<Vec<usize>>,
}

fn es_salto(c: char) -> bool {
    c == '\n' || c == '\r'
}

/// Las tablas que hay en el texto del control, en orden.
pub fn tablas_en_control(texto: &str) -> Vec<TablaEnControl> {
    let letras: Vec<char> = texto.chars().collect();
    // La posicion UTF-16 de cada letra.
    let mut pos = Vec::with_capacity(letras.len() + 1);
    let mut p = 0;
    for c in &letras {
        pos.push(p);
        p += c.len_utf16();
    }
    pos.push(p);
    let mut sal: Vec<TablaEnControl> = Vec::new();
    let mut i = 0;
    while i < letras.len() {
        if letras[i] == FILA_ABRE && letras.get(i + 1).is_some_and(|c| es_salto(*c)) {
            let inicio = i;
            let mut filas = Vec::new();
            let mut celdas = Vec::new();
            while i < letras.len()
                && letras[i] == FILA_ABRE
                && letras.get(i + 1).is_some_and(|c| es_salto(*c))
            {
                let mut k = i + 2;
                let mut fila = Vec::new();
                let mut donde = Vec::new();
                let mut actual = String::new();
                let mut desde = pos[k];
                while k < letras.len() && letras[k] != FILA_CIERRA {
                    if letras[k] == CELDA {
                        fila.push(std::mem::take(&mut actual));
                        donde.push(desde);
                        desde = pos[k + 1];
                    } else {
                        match letras[k] {
                            // Lo de una tapada no es texto de nadie.
                            TAPADA => {}
                            // El salto de renglon dentro de la celda.
                            x if es_salto(x) || x == SALTO_EN_CELDA => actual.push('\n'),
                            x => actual.push(x),
                        }
                    }
                    k += 1;
                }
                filas.push(fila);
                celdas.push(donde);
                // La marca de cierre y su salto.
                k += 1;
                if k < letras.len() && es_salto(letras[k]) {
                    k += 1;
                }
                i = k;
            }
            sal.push(TablaEnControl {
                desde: pos[inicio],
                hasta: pos[i.min(letras.len())],
                tabla: Tabla {
                    filas,
                    alineaciones: Vec::new(),
                    formato: Vec::new(),
                },
                celdas,
            });
            continue;
        }
        i += 1;
    }
    sal
}

/// El texto del control, de vuelta a Markdown. `alineaciones(k)` da las de
/// la tabla `k` (en orden), que el texto no las lleva.
pub fn de_control(texto: &str, alineaciones: &mut dyn FnMut(usize) -> Vec<Alineacion>) -> String {
    de_control_con(texto, &mut |k, t| t.alineaciones = alineaciones(k))
}

/// **La tabla escrita como hay que escribirla**: con barras si las barras
/// la saben decir, y si no (combinadas, colores…) en el HTML que lee el
/// movil (`Tablas.aTexto`). Sin salto al final.
pub fn a_texto(t: &Tabla) -> String {
    if t.es_avanzada() {
        crate::md_tabla_html::a_html(t)
    } else {
        a_gfm(t)
    }
}

/// Como [`de_control`], pero `completar(k, tabla)` pone en la tabla `k` lo
/// que el texto no lleva: alineaciones, combinadas y colores (lo sabe el
/// control).
pub fn de_control_con(texto: &str, completar: &mut dyn FnMut(usize, &mut Tabla)) -> String {
    let tablas = tablas_en_control(texto);
    if tablas.is_empty() {
        return texto.replace('\r', "\n");
    }
    let u: Vec<u16> = texto.encode_utf16().collect();
    let trozo = |a: usize, b: usize| String::from_utf16_lossy(&u[a..b]).replace('\r', "\n");
    let mut sal = String::with_capacity(texto.len());
    let mut desde = 0;
    for (k, t) in tablas.iter().enumerate() {
        sal.push_str(&trozo(desde, t.desde));
        let mut tabla = t.tabla.clone();
        completar(k, &mut tabla);
        sal.push_str(&a_texto(&tabla));
        // El salto de la ultima fila es el que separa la tabla de lo que
        // sigue; sin el (tabla al final del todo) no se inventa.
        if t.hasta > t.desde
            && u.get(t.hasta - 1)
                .is_some_and(|c| *c == b'\n' as u16 || *c == b'\r' as u16)
        {
            sal.push('\n');
        }
        desde = t.hasta;
    }
    sal.push_str(&trozo(desde, u.len()));
    sal
}

/// La tabla, fila y columna de la celda donde cae `pos`, si cae en una.
pub fn celda_en(tablas: &[TablaEnControl], pos: usize) -> Option<(usize, usize, usize)> {
    let (k, t) = tablas
        .iter()
        .enumerate()
        .find(|(_, t)| t.desde <= pos && pos < t.hasta)?;
    let mut ultima = None;
    for (f, fila) in t.celdas.iter().enumerate() {
        for (c, &inicio) in fila.iter().enumerate() {
            if inicio <= pos {
                ultima = Some((k, f, c));
            }
        }
    }
    ultima
}

/// Lo que ocupa a lo ancho el texto de una celda, en letras visibles: sin
/// las marcas de Markdown mas comunes, que el editor esconde. Sirve para
/// repartir el ancho de las columnas.
pub fn letras_visibles(celda: &str) -> usize {
    celda
        .chars()
        .filter(|c| !matches!(c, '*' | '`' | '~' | '_' | '$'))
        .count()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const DEL_MOVIL: &str =
        "| Objetivo | Tecnica |\n|:---|:---|\n| OE1 | Pareto |\n| OE2 | Likert 1\\|5 |";

    #[test]
    fn una_tabla_del_movil_se_lee_y_se_escribe_igual() {
        let t = leer_gfm(DEL_MOVIL).unwrap();
        assert_eq!(t.filas.len(), 3);
        assert_eq!(t.filas[2][1], "Likert 1|5");
        assert_eq!(a_gfm(&t), DEL_MOVIL);
    }

    #[test]
    fn la_alineacion_va_en_los_dos_puntos() {
        let t = leer_gfm("| a | b | c |\n|---|:-:|--:|\n| 1 | 2 | 3 |").unwrap();
        assert_eq!(
            t.alineaciones,
            vec![
                Alineacion::Izquierda,
                Alineacion::Centro,
                Alineacion::Derecha
            ]
        );
        assert!(a_gfm(&t).contains("|:---|:---:|---:|"));
    }

    #[test]
    fn sin_fila_de_guiones_no_hay_tabla() {
        assert_eq!(leer_gfm("| a | b |\n| c | d |"), None);
        assert_eq!(leer_gfm("ruta | con | barras"), None);
        assert!(!es_separadora("| a | --- |"));
        assert!(es_separadora("|:---|---:|"));
    }

    #[test]
    fn una_celda_con_salto_se_guarda_en_un_renglon() {
        let t = Tabla {
            filas: vec![vec!["uno\ndos".into()], vec!["x".into()]],
            alineaciones: vec![],
            formato: vec![],
        };
        assert_eq!(a_gfm(&t), "| uno dos |\n|:---|\n| x |");
    }

    #[test]
    fn la_tabla_nueva_es_la_del_movil() {
        let t = Tabla::nueva(3, 2);
        assert_eq!(a_gfm(&t), "|  |  |\n|:---|:---|\n|  |  |\n|  |  |");
        assert_eq!(Tabla::nueva(0, 99).columnas(), 12);
    }

    #[test]
    fn filas_y_columnas_se_ponen_y_se_quitan_sin_dejar_la_tabla_vacia() {
        let mut t = leer_gfm("| a | b |\n|---|--:|\n| 1 | 2 |").unwrap();
        t.insertar_columna(1);
        assert_eq!(t.filas[0], vec!["a", "", "b"]);
        assert_eq!(t.alineacion(2), Alineacion::Derecha);
        t.insertar_fila(1);
        assert_eq!(t.filas[1], vec!["", "", ""]);
        assert!(t.quitar_columna(0));
        assert_eq!(t.filas[0], vec!["", "b"]);
        assert!(t.quitar_columna(0));
        assert!(!t.quitar_columna(0), "la ultima columna no se quita");
        assert!(t.quitar_fila(0));
        assert!(t.quitar_fila(0));
        assert!(!t.quitar_fila(0), "la ultima fila no se quita");
        assert!(!t.quitar_fila(9));
    }

    #[test]
    fn el_markdown_se_parte_en_texto_y_tablas() {
        let md = format!("# Plan\n{DEL_MOVIL}\nfin");
        let p = para_control(&md);
        assert_eq!(p.texto, "# Plan\n\nfin");
        assert_eq!(p.tablas.len(), 1);
        assert_eq!(p.tablas[0].0, 7);
        assert_eq!(p.tablas[0].1.filas[1][0], "OE1");
    }

    #[test]
    fn una_tabla_dentro_de_codigo_no_se_toca() {
        let md = "```\n| a |\n|---|\n```\n";
        let p = para_control(md);
        assert_eq!(p.texto, md);
        assert!(p.tablas.is_empty());
    }

    /// El texto que tendria el control con esa tabla metida en su hueco.
    fn en_control(p: &ParaControl) -> String {
        let mut t: Vec<u16> = p.texto.encode_utf16().collect();
        for (pos, tabla) in p.tablas.iter().rev() {
            let mut filas = String::new();
            for f in &tabla.filas {
                filas.push(FILA_ABRE);
                filas.push('\r');
                for c in f {
                    filas.push_str(c);
                    filas.push(CELDA);
                }
                filas.push(FILA_CIERRA);
                filas.push('\r');
            }
            t.splice(*pos..*pos + 1, filas.encode_utf16());
        }
        String::from_utf16_lossy(&t)
    }

    #[test]
    fn ida_y_vuelta_por_el_control_da_el_mismo_markdown() {
        let md = format!("# Plan\n{DEL_MOVIL}\nfin **ya**\n\n| x |\n|---:|\n| 1 |\n");
        let p = para_control(&md);
        let control = en_control(&p);
        let mut alineaciones = |k: usize| p.tablas[k].1.alineaciones.clone();
        let vuelta = de_control(&control, &mut alineaciones);
        assert_eq!(vuelta, md);
    }

    #[test]
    fn una_tabla_al_final_sin_salto_vuelve_con_su_salto() {
        let md = "a\n| x |\n|---|\n| 1 |";
        let p = para_control(md);
        let control = en_control(&p);
        let vuelta = de_control(&control, &mut |_| vec![]);
        assert_eq!(vuelta, "a\n| x |\n|:---|\n| 1 |\n");
    }

    #[test]
    fn un_texto_sin_tablas_vuelve_tal_cual() {
        assert_eq!(de_control("uno\rdos", &mut |_| vec![]), "uno\ndos");
        assert!(tablas_en_control("sin \u{7} marcas").is_empty());
    }

    #[test]
    fn la_celda_del_cursor_se_encuentra_por_su_posicion() {
        let p = para_control("x\n| a | bb |\n|---|---|\n| c | d |\n");
        let control = en_control(&p);
        let tablas = tablas_en_control(&control);
        assert_eq!(tablas.len(), 1);
        let t = &tablas[0];
        assert_eq!(t.desde, 2);
        assert_eq!(t.celdas[0], vec![4, 6]);
        assert_eq!(celda_en(&tablas, 4), Some((0, 0, 0)));
        assert_eq!(celda_en(&tablas, 7), Some((0, 0, 1)));
        assert_eq!(celda_en(&tablas, t.celdas[1][1]), Some((0, 1, 1)));
        assert_eq!(celda_en(&tablas, 0), None);
        assert_eq!(celda_en(&tablas, t.hasta), None);
    }

    #[test]
    fn las_letras_visibles_no_cuentan_las_marcas() {
        assert_eq!(letras_visibles("**OE1**"), 3);
    }

    // -----------------------------------------------------------------------
    // Combinar, separar y colores

    fn rejilla(filas: usize, columnas: usize) -> Tabla {
        let mut t = Tabla::nueva(filas, columnas);
        for (f, fila) in t.filas.iter_mut().enumerate() {
            for (c, x) in fila.iter_mut().enumerate() {
                *x = format!("{f}{c}");
            }
        }
        t
    }

    #[test]
    fn combinar_junta_los_textos_en_la_de_arriba_y_tapa_las_demas() {
        let mut t = rejilla(3, 3);
        assert!(
            t.combinar((1, 2), (0, 1)),
            "las esquinas en cualquier orden"
        );
        assert_eq!(t.filas[0][1], "01\n02\n11\n12");
        let a = t.formato(0, 1);
        assert_eq!((a.filas, a.columnas, a.tapada), (2, 2, false));
        for (f, c) in [(0, 2), (1, 1), (1, 2)] {
            assert!(t.formato(f, c).tapada);
            assert_eq!(t.filas[f][c], "");
            assert_eq!(t.ancla(f, c), (0, 1));
        }
        assert_eq!(t.ancla(2, 2), (2, 2));
        assert!(t.es_avanzada());
    }

    #[test]
    fn combinar_una_sola_celda_no_hace_nada() {
        let mut t = rejilla(2, 2);
        assert!(!t.combinar((1, 1), (1, 1)));
        assert_eq!(t, rejilla(2, 2));
        assert!(!t.es_avanzada());
        assert!(!Tabla::default().clone().combinar((0, 0), (1, 1)));
        // Ni una combinada consigo misma (desde cualquiera de sus celdas).
        let mut t = rejilla(3, 3);
        t.combinar((0, 0), (1, 1));
        let antes = t.clone();
        assert!(!t.combinar((1, 1), (1, 1)));
        assert_eq!(t, antes);
    }

    #[test]
    fn combinar_crece_hasta_cubrir_la_combinada_que_toca() {
        let mut t = rejilla(3, 3);
        t.combinar((0, 1), (1, 1));
        // De (1,0) a (1,1) toca media combinada: se agranda hasta (0,0).
        assert_eq!(t.rectangulo((1, 0), (1, 1)), (0, 1, 0, 1));
        assert!(t.combinar((1, 0), (1, 1)));
        let a = t.formato(0, 0);
        assert_eq!((a.filas, a.columnas), (2, 2));
        assert_eq!(t.filas[0][0], "00\n01\n11\n10");
    }

    #[test]
    fn separar_devuelve_las_celdas_con_la_pinta_de_la_combinada() {
        let mut t = rejilla(3, 3);
        t.poner_fondo((1, 1), (1, 1), Some(0xffc9c9));
        t.combinar((1, 1), (2, 2));
        assert!(t.separar(2, 2), "desde cualquiera de sus celdas");
        for (f, c) in [(1, 1), (1, 2), (2, 1), (2, 2)] {
            let x = t.formato(f, c);
            assert!(!x.tapada);
            assert_eq!((x.filas, x.columnas), (1, 1));
            assert_eq!(x.fondo, Some(0xffc9c9));
        }
        assert_eq!(t.filas[1][1], "11\n12\n21\n22");
        assert!(!t.separar(0, 0), "una celda suelta no se separa");
        assert!(!t.separar(9, 0));
    }

    #[test]
    fn los_colores_van_a_las_celdas_que_mandan_y_se_quitan_con_none() {
        let mut t = rejilla(3, 3);
        t.combinar((0, 0), (0, 1));
        t.poner_fondo((0, 1), (2, 1), Some(0xa5d8ff));
        // La combinada manda: se pinta ella, no su tapada.
        assert_eq!(t.formato(0, 0).fondo, Some(0xa5d8ff));
        assert_eq!(t.formato(0, 1).fondo, None);
        assert_eq!(t.formato(2, 1).fondo, Some(0xa5d8ff));
        t.poner_letra((2, 0), (2, 2), Some(0xe03131));
        assert_eq!(t.formato(2, 2).letra, Some(0xe03131));
        t.separar(0, 0);
        t.poner_fondo((0, 0), (2, 2), None);
        t.poner_letra((0, 0), (2, 2), None);
        assert!(t.formato.is_empty(), "sin nada vuelve a ser la de barras");
        assert!(!t.es_avanzada());
    }

    #[test]
    fn pintar_una_columna_que_cruza_un_titulo_combinado_no_pinta_las_demas() {
        let mut t = rejilla(3, 3);
        t.combinar((0, 0), (0, 2));
        t.poner_letra((0, 2), (2, 2), Some(0x1971c2));
        assert_eq!(
            t.formato(0, 0).letra,
            Some(0x1971c2),
            "el titulo tapa esa columna"
        );
        assert_eq!(t.formato(1, 2).letra, Some(0x1971c2));
        assert_eq!(t.formato(1, 1).letra, None);
        assert_eq!(t.formato(2, 0).letra, None);
    }

    #[test]
    fn una_fila_metida_dentro_de_una_combinada_la_alarga() {
        let mut t = rejilla(4, 2);
        t.combinar((1, 0), (2, 0));
        t.insertar_fila(2);
        assert_eq!(t.formato(1, 0).filas, 3);
        assert!(t.formato(2, 0).tapada);
        assert!(!t.formato(2, 1).tapada);
        // Encima o debajo de la combinada no la toca.
        t.insertar_fila(1);
        t.insertar_fila(9);
        assert_eq!(t.formato(2, 0).filas, 3);
        assert_eq!(t.filas.len(), 7);
        assert!(!t.formato(6, 0).tapada);
    }

    #[test]
    fn quitar_la_fila_de_arriba_de_una_combinada_pasa_el_mando_a_la_de_debajo() {
        let mut t = rejilla(4, 2);
        t.combinar((1, 0), (3, 0));
        assert!(t.quitar_fila(1));
        let x = t.formato(1, 0);
        assert_eq!((x.filas, x.tapada), (2, false));
        assert_eq!(t.filas[1][0], "10\n20\n30");
        assert!(t.formato(2, 0).tapada);
        assert!(t.quitar_fila(2));
        assert_eq!(t.formato(1, 0).filas, 1);
        assert!(
            t.formato.is_empty(),
            "una combinada de una celda ya no es combinada"
        );
    }

    #[test]
    fn columnas_dentro_y_fuera_de_una_combinada() {
        let mut t = rejilla(2, 3);
        t.combinar((0, 0), (0, 1));
        t.insertar_columna(1);
        assert_eq!(t.formato(0, 0).columnas, 3);
        assert!(t.formato(0, 1).tapada && t.formato(0, 2).tapada);
        assert!(!t.formato(1, 1).tapada);
        assert!(t.quitar_columna(0));
        assert_eq!(t.formato(0, 0).columnas, 2);
        assert_eq!(t.filas[0][0], "00\n01");
        assert!(t.quitar_columna(1));
        assert_eq!(t.formato(0, 0), Formato::default());
        assert_eq!(t.columnas(), 2);
    }

    #[test]
    fn pegar_en_una_tabla_rellena_desde_la_celda_y_la_agranda() {
        let mut t = rejilla(2, 2);
        let mut otra = rejilla(2, 2);
        otra.filas = vec![vec!["a".into(), "b".into()], vec!["c".into(), "d".into()]];
        otra.formato_mut(1, 1).fondo = Some(0xffec99);
        t.pegar_en(1, 1, &otra);
        assert_eq!(t.filas.len(), 3);
        assert_eq!(t.columnas(), 3);
        assert_eq!(t.filas[1][1], "a");
        assert_eq!(t.filas[2][2], "d");
        assert_eq!(t.filas[0][0], "00", "lo de fuera no se toca");
        assert_eq!(t.formato(2, 2).fondo, Some(0xffec99));
        assert_eq!(t.formato(1, 1).fondo, None);
    }

    #[test]
    fn pegar_no_escribe_en_una_tapada_ni_pasa_de_los_topes() {
        let mut t = rejilla(2, 2);
        t.combinar((0, 0), (0, 1));
        let otra = Tabla {
            filas: vec![vec!["x".into(), "y".into()]],
            ..Tabla::default()
        };
        t.pegar_en(0, 0, &otra);
        assert_eq!(t.filas[0][0], "x");
        assert_eq!(t.filas[0][1], "", "la tapada sigue vacia");
        let ancha = Tabla {
            filas: vec![vec![String::new(); MAX_COLUMNAS + 5]],
            ..Tabla::default()
        };
        let mut t = rejilla(2, 2);
        t.pegar_en(0, 1, &ancha);
        assert_eq!(t.columnas(), MAX_COLUMNAS);
    }

    #[test]
    fn una_celda_alineada_distinto_que_su_columna_pide_html() {
        let mut t = leer_gfm("| a | b |\n|:-:|---|\n| 1 | 2 |").unwrap();
        assert!(!t.es_avanzada());
        t.formato_mut(1, 0).alineacion = Some(Alineacion::Derecha);
        assert!(t.es_avanzada());
        t.formato_mut(1, 0).alineacion = Some(Alineacion::Centro);
        assert!(!t.es_avanzada(), "la misma que la columna no cuenta");
        t.formato_mut(1, 1).cabecera = Some(true);
        assert!(t.es_avanzada());
    }

    #[test]
    fn el_texto_de_una_tapada_del_control_no_es_texto() {
        let control =
            format!("{FILA_ABRE}\rA{SALTO_EN_CELDA}B{CELDA}{TAPADA}{CELDA}{FILA_CIERRA}\r");
        let t = tablas_en_control(&control).remove(0);
        assert_eq!(t.tabla.filas, vec![vec!["A\nB".to_string(), String::new()]]);
    }

    #[test]
    fn una_tabla_con_combinadas_vuelve_del_control_como_html() {
        let control = format!(
            "x\n{FILA_ABRE}\rA{CELDA}{TAPADA}{CELDA}{FILA_CIERRA}\r{FILA_ABRE}\r1{CELDA}2{CELDA}{FILA_CIERRA}\r"
        );
        let md = de_control_con(&control, &mut |_, t| {
            t.combinar((0, 0), (0, 1));
        });
        assert_eq!(
            md,
            "x\n<table>\n  <tr>\n    <th colspan=\"2\">A</th>\n  </tr>\n  <tr>\n    <td>1</td>\n    <td>2</td>\n  </tr>\n</table>\n"
        );
        // Y de vuelta al control es la misma tabla.
        let p = para_control(&md);
        assert_eq!(p.texto, "x\n\n");
        assert_eq!(p.tablas[0].1.formato(0, 0).columnas, 2);
        assert!(p.tablas[0].1.formato(0, 1).tapada);
    }

    #[test]
    fn una_tabla_html_con_titulo_o_dentro_de_codigo_se_queda_como_texto() {
        let con_titulo =
            "<table>\n  <caption>Plan</caption>\n  <tr>\n    <td>a</td>\n  </tr>\n</table>\n";
        let p = para_control(con_titulo);
        assert!(p.tablas.is_empty());
        assert_eq!(p.texto, con_titulo);
        let en_codigo = "```\n<table><tr><td>a</td></tr></table>\n```\n";
        assert!(para_control(en_codigo).tablas.is_empty());
        // Sin cerrar ni filas: tampoco.
        assert!(para_control("<table>\nsin filas\n").tablas.is_empty());
    }

    // -----------------------------------------------------------------------
    // Mover filas y columnas con su asa (H12, 1-oct)

    fn cuatro() -> Tabla {
        leer_gfm("| A | B | C |\n|:---|:-:|--:|\n| 1 | x | p |\n| 2 | y | q |\n| 3 | z | r |")
            .unwrap()
    }

    fn columna(t: &Tabla, c: usize) -> Vec<String> {
        t.filas.iter().map(|f| f[c].clone()).collect()
    }

    #[test]
    fn mover_una_fila_lleva_su_texto_y_sus_colores() {
        let mut t = cuatro();
        t.poner_fondo((1, 0), (1, 2), Some(0xff0000));
        assert!(t.mover_fila(1, 4), "la del 1, al final");
        assert_eq!(columna(&t, 0), ["A", "2", "3", "1"]);
        assert_eq!(t.formato(3, 1).fondo, Some(0xff0000));
        assert_eq!(t.formato(1, 1).fondo, None);
        assert!(t.mover_fila(3, 1), "y de vuelta arriba");
        assert_eq!(columna(&t, 0), ["A", "1", "2", "3"]);
        // Ida y vuelta por el Markdown (con color va como <table>).
        let mut sin_color = cuatro();
        assert!(sin_color.mover_fila(3, 1));
        assert_eq!(leer_gfm(&a_gfm(&sin_color)).unwrap(), sin_color);
        // Casos negativos: a su propio sitio, o una que no esta.
        assert!(!t.mover_fila(1, 1) && !t.mover_fila(1, 2));
        assert!(!t.mover_fila(9, 0) && !t.mover_fila(0, 9));
    }

    #[test]
    fn mover_una_columna_lleva_su_alineacion() {
        let mut t = cuatro();
        assert!(t.mover_columna(2, 0), "la C, delante de todas");
        assert_eq!(t.filas[0], ["C", "A", "B"]);
        assert_eq!(
            t.alineaciones,
            [
                Alineacion::Derecha,
                Alineacion::Izquierda,
                Alineacion::Centro
            ]
        );
        assert!(t.mover_columna(0, 3));
        assert_eq!(t.filas[1], ["1", "x", "p"]);
        assert!(!t.mover_columna(1, 2), "a su sitio no");
    }

    #[test]
    fn una_combinada_se_mueve_entera_y_no_se_parte() {
        let mut t = cuatro();
        assert!(t.combinar((1, 0), (2, 0)), "el 1 y el 2, de dos filas");
        // Coger la fila de abajo de la combinada mueve las dos.
        assert!(t.mover_fila(2, 4));
        assert_eq!(columna(&t, 1), ["B", "z", "x", "y"]);
        assert_eq!(t.formato(2, 0).filas, 2);
        assert!(t.formato(3, 0).tapada);
        // No se puede dejar nada en medio de una combinada.
        assert!(!t.mover_fila(1, 3), "entre sus dos filas no");
        assert_eq!(t.banda_de_filas(3), (2, 4));
        // Las columnas igual.
        let mut t = cuatro();
        assert!(t.combinar((0, 0), (0, 1)));
        assert!(!t.mover_columna(2, 1), "entre las dos de la combinada no");
        assert!(t.mover_columna(0, 3), "la combinada entera, al final");
        assert_eq!(t.filas[1], ["p", "1", "x"]);
        assert_eq!(t.formato(0, 1).columnas, 2);
    }
}
