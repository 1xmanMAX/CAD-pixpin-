//! **Un lector de PDF pequeno y tolerante**: los objetos, los flujos con sus
//! filtros y el arbol de paginas, sin dependencias mas que `flate2`.
//!
//! No se fia de la tabla `xref` (muchos PDF la traen mal): recorre el
//! fichero buscando cada `n g obj` (saltando los datos de los flujos) y abre
//! los flujos de objetos (`/ObjStm`); lo que se define despues manda, como
//! en una actualizacion incremental. Los PDF cifrados se rechazan.

use std::collections::{HashMap, HashSet};
use std::io::Read;

/// Lo que se descomprime de un flujo como mucho (bombas de zlib).
pub const TOPE_FLUJO: usize = 256 << 20;
/// Cuantas paginas se miran como mucho.
const TOPE_PAGINAS: usize = 100_000;

/// **Un objeto de PDF.**
#[derive(Debug, Clone, PartialEq)]
pub enum Obj {
    Nulo,
    Bool(bool),
    Num(f64),
    Nombre(String),
    Cadena(Vec<u8>),
    Arr(Vec<Obj>),
    Dic(Dic),
    Ref(u32, u16),
    Flujo(Box<Flujo>),
}

/// Un diccionario (las llaves sin la barra).
pub type Dic = HashMap<String, Obj>;

/// **Un flujo**: su diccionario y sus datos tal cual (sin descomprimir).
#[derive(Debug, Clone, PartialEq)]
pub struct Flujo {
    pub dic: Dic,
    pub datos: Vec<u8>,
}

impl Obj {
    pub fn num(&self) -> Option<f64> {
        match self {
            Obj::Num(v) => Some(*v),
            _ => None,
        }
    }

    pub fn nombre(&self) -> Option<&str> {
        match self {
            Obj::Nombre(n) => Some(n),
            _ => None,
        }
    }
}

// ------------------------------------------------------------------ lexico

/// Un pedazo del PDF.
#[derive(Debug, Clone, PartialEq)]
pub enum Tok<'a> {
    Num(f64, bool),
    Nombre(String),
    Cadena(Vec<u8>),
    AbreArr,
    CierraArr,
    AbreDic,
    CierraDic,
    /// Un operador o `true`, `false`, `null`, `obj`, `R`…
    Palabra(&'a [u8]),
}

pub fn es_blanco(c: u8) -> bool {
    matches!(c, 0 | 9 | 10 | 12 | 13 | 32)
}

pub fn es_delimitador(c: u8) -> bool {
    matches!(
        c,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// **El lector de pedazos** sobre unos bytes.
pub struct Lexico<'a> {
    pub d: &'a [u8],
    pub i: usize,
}

impl<'a> Lexico<'a> {
    pub fn new(d: &'a [u8]) -> Lexico<'a> {
        Lexico { d, i: 0 }
    }

    /// Salta blancos y comentarios.
    pub fn saltar(&mut self) {
        while self.i < self.d.len() {
            let c = self.d[self.i];
            if es_blanco(c) {
                self.i += 1;
            } else if c == b'%' {
                while self.i < self.d.len() && !matches!(self.d[self.i], b'\n' | b'\r') {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
    }

    pub fn siguiente(&mut self) -> Option<Tok<'a>> {
        self.saltar();
        let d = self.d;
        let c = *d.get(self.i)?;
        match c {
            b'[' => {
                self.i += 1;
                Some(Tok::AbreArr)
            }
            b']' => {
                self.i += 1;
                Some(Tok::CierraArr)
            }
            b'{' | b'}' => {
                // Solo en las funciones de tipo 4: como palabras.
                self.i += 1;
                Some(Tok::Palabra(&d[self.i - 1..self.i]))
            }
            b'<' if d.get(self.i + 1) == Some(&b'<') => {
                self.i += 2;
                Some(Tok::AbreDic)
            }
            b'>' if d.get(self.i + 1) == Some(&b'>') => {
                self.i += 2;
                Some(Tok::CierraDic)
            }
            b'<' => {
                self.i += 1;
                let mut v = Vec::new();
                let mut alto: Option<u8> = None;
                while self.i < d.len() && d[self.i] != b'>' {
                    if let Some(h) = hex(d[self.i]) {
                        match alto.take() {
                            Some(a) => v.push(a << 4 | h),
                            None => alto = Some(h),
                        }
                    }
                    self.i += 1;
                }
                if let Some(a) = alto {
                    v.push(a << 4);
                }
                self.i += 1;
                Some(Tok::Cadena(v))
            }
            b'(' => Some(Tok::Cadena(self.literal())),
            b'/' => {
                self.i += 1;
                let mut v = Vec::new();
                while self.i < d.len() && !es_blanco(d[self.i]) && !es_delimitador(d[self.i]) {
                    if d[self.i] == b'#'
                        && let (Some(a), Some(b)) = (
                            d.get(self.i + 1).and_then(|c| hex(*c)),
                            d.get(self.i + 2).and_then(|c| hex(*c)),
                        )
                    {
                        v.push(a << 4 | b);
                        self.i += 3;
                        continue;
                    }
                    v.push(d[self.i]);
                    self.i += 1;
                }
                Some(Tok::Nombre(String::from_utf8_lossy(&v).into_owned()))
            }
            b')' | b'>' => {
                // Sueltos: se saltan.
                self.i += 1;
                self.siguiente()
            }
            _ => {
                let ini = self.i;
                while self.i < d.len() && !es_blanco(d[self.i]) && !es_delimitador(d[self.i]) {
                    self.i += 1;
                }
                let p = &d[ini..self.i];
                if p.is_empty() {
                    self.i += 1;
                    return self.siguiente();
                }
                if let Some(n) = numero(p) {
                    return Some(Tok::Num(n, !p.contains(&b'.')));
                }
                Some(Tok::Palabra(p))
            }
        }
    }

    /// Una cadena `( … )` con sus escapes y parentesis anidados.
    fn literal(&mut self) -> Vec<u8> {
        let d = self.d;
        self.i += 1;
        let mut v = Vec::new();
        let mut nivel = 1;
        while self.i < d.len() {
            let c = d[self.i];
            self.i += 1;
            match c {
                b'\\' => {
                    let Some(&e) = d.get(self.i) else { break };
                    self.i += 1;
                    match e {
                        b'n' => v.push(b'\n'),
                        b'r' => v.push(b'\r'),
                        b't' => v.push(b'\t'),
                        b'b' => v.push(8),
                        b'f' => v.push(12),
                        b'\r' => {
                            if d.get(self.i) == Some(&b'\n') {
                                self.i += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut n = (e - b'0') as u32;
                            for _ in 0..2 {
                                match d.get(self.i) {
                                    Some(&o @ b'0'..=b'7') => {
                                        n = n * 8 + (o - b'0') as u32;
                                        self.i += 1;
                                    }
                                    _ => break,
                                }
                            }
                            v.push(n as u8);
                        }
                        otro => v.push(otro),
                    }
                }
                b'(' => {
                    nivel += 1;
                    v.push(c);
                }
                b')' => {
                    nivel -= 1;
                    if nivel == 0 {
                        break;
                    }
                    v.push(c);
                }
                _ => v.push(c),
            }
        }
        v
    }
}

/// Un numero de PDF (`12`, `-3.5`, `.5`, `+4.`).
fn numero(p: &[u8]) -> Option<f64> {
    let c0 = *p.first()?;
    if !(c0.is_ascii_digit() || matches!(c0, b'+' | b'-' | b'.')) {
        return None;
    }
    if !p
        .iter()
        .all(|c| c.is_ascii_digit() || matches!(c, b'+' | b'-' | b'.'))
    {
        return None;
    }
    let s = std::str::from_utf8(p).ok()?;
    // Algunos generadores escriben `--5` o `4.-2`: lo que se entienda.
    let s = s.trim_start_matches('+');
    let s = if let Some(r) = s.strip_prefix("--") {
        r
    } else {
        s
    };
    s.parse::<f64>()
        .ok()
        .or_else(|| {
            let corto: String = s
                .chars()
                .enumerate()
                .take_while(|(i, c)| c.is_ascii_digit() || *c == '.' || (*i == 0 && *c == '-'))
                .map(|(_, c)| c)
                .collect();
            corto.parse().ok()
        })
        .filter(|v: &f64| v.is_finite())
}

/// **Lee un objeto** desde el lexico (`None` si no hay o no se entiende).
pub fn objeto(lx: &mut Lexico, prof: u32) -> Option<Obj> {
    if prof > 64 {
        return None;
    }
    let t = lx.siguiente()?;
    objeto_desde(lx, t, prof)
}

/// Un objeto que empieza por el pedazo `t` (ya leido).
pub fn objeto_desde<'a>(lx: &mut Lexico<'a>, t: Tok<'a>, prof: u32) -> Option<Obj> {
    // Arreglos dentro de arreglos sin fin: un PDF malicioso no tumba la pila.
    if prof > 64 {
        return None;
    }
    Some(match t {
        Tok::Num(n, entero) => {
            if entero && n >= 0.0 {
                // `n g R`: una referencia.
                let guardado = lx.i;
                if let Some(Tok::Num(g, true)) = lx.siguiente()
                    && let Some(Tok::Palabra(b"R")) = lx.siguiente()
                {
                    return Some(Obj::Ref(n as u32, g as u16));
                }
                lx.i = guardado;
            }
            Obj::Num(n)
        }
        Tok::Nombre(n) => Obj::Nombre(n),
        Tok::Cadena(c) => Obj::Cadena(c),
        Tok::AbreArr => {
            let mut v = Vec::new();
            loop {
                let t = lx.siguiente()?;
                if t == Tok::CierraArr {
                    break;
                }
                if let Some(o) = objeto_desde(lx, t, prof + 1) {
                    v.push(o);
                }
            }
            Obj::Arr(v)
        }
        Tok::AbreDic => {
            let mut d = Dic::new();
            loop {
                let t = lx.siguiente()?;
                match t {
                    Tok::CierraDic => break,
                    Tok::Nombre(k) => {
                        let t2 = lx.siguiente()?;
                        if t2 == Tok::CierraDic {
                            break;
                        }
                        if let Some(v) = objeto_desde(lx, t2, prof + 1) {
                            d.insert(k, v);
                        }
                    }
                    _ => {}
                }
            }
            Obj::Dic(d)
        }
        Tok::Palabra(b"true") => Obj::Bool(true),
        Tok::Palabra(b"false") => Obj::Bool(false),
        Tok::Palabra(b"null") => Obj::Nulo,
        Tok::CierraArr | Tok::CierraDic | Tok::Palabra(_) => return None,
    })
}

// --------------------------------------------------------------- documento

/// **Un PDF abierto**: sus objetos por numero y su catalogo.
#[derive(Debug, Default)]
pub struct Documento {
    objetos: HashMap<u32, Obj>,
    raiz: Option<Obj>,
}

/// Busca `aguja` en `d` desde `desde`.
pub fn buscar(d: &[u8], aguja: &[u8], desde: usize) -> Option<usize> {
    if aguja.is_empty() || desde >= d.len() {
        return None;
    }
    let primero = aguja[0];
    let mut i = desde;
    while i + aguja.len() <= d.len() {
        let k = d[i..d.len() - aguja.len() + 1]
            .iter()
            .position(|c| *c == primero)?;
        i += k;
        if &d[i..i + aguja.len()] == aguja {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Los dos enteros `n g` justo antes de `obj` en `k`.
fn cabeza_de_objeto(d: &[u8], k: usize) -> Option<u32> {
    let mut i = k;
    let blancos = |i: &mut usize| {
        let ini = *i;
        while *i > 0 && es_blanco(d[*i - 1]) {
            *i -= 1;
        }
        *i < ini
    };
    let digitos = |i: &mut usize| {
        let fin = *i;
        while *i > 0 && d[*i - 1].is_ascii_digit() {
            *i -= 1;
        }
        (*i < fin).then(|| std::str::from_utf8(&d[*i..fin]).ok()?.parse::<u64>().ok())?
    };
    if !blancos(&mut i) {
        return None;
    }
    digitos(&mut i)?;
    if !blancos(&mut i) {
        return None;
    }
    let n = digitos(&mut i)?;
    if i > 0 && !es_blanco(d[i - 1]) && !es_delimitador(d[i - 1]) {
        return None;
    }
    u32::try_from(n).ok()
}

impl Documento {
    /// **Lee un PDF** de unos bytes.
    pub fn leer(d: &[u8]) -> Result<Documento, String> {
        let cabeza = &d[..d.len().min(1024)];
        if buscar(cabeza, b"%PDF-", 0).is_none() {
            return Err("no es un PDF (falta la cabecera %PDF-)".into());
        }
        let mut doc = Documento::default();
        let mut cifrado = false;
        let mut i = 0;
        while let Some(k) = buscar(d, b"obj", i) {
            i = k + 3;
            let tras = d.get(k + 3).copied();
            if tras.is_some_and(|c| !es_blanco(c) && !es_delimitador(c)) {
                continue;
            }
            let Some(num) = cabeza_de_objeto(d, k) else {
                continue;
            };
            let mut lx = Lexico { d, i: k + 3 };
            let Some(o) = objeto(&mut lx, 0) else {
                continue;
            };
            let guardado = lx.i;
            let o = match (o, lx.siguiente()) {
                (Obj::Dic(dic), Some(Tok::Palabra(b"stream"))) => {
                    // Los datos empiezan tras el fin de linea.
                    let mut ini = lx.i;
                    if d.get(ini) == Some(&b'\r') {
                        ini += 1;
                    }
                    if d.get(ini) == Some(&b'\n') {
                        ini += 1;
                    }
                    let largo = dic.get("Length").and_then(Obj::num).map(|v| v as usize);
                    let fin = largo
                        .filter(|l| {
                            let f = ini.saturating_add(*l);
                            f <= d.len() && {
                                let mut j = f;
                                while j < d.len() && es_blanco(d[j]) {
                                    j += 1;
                                }
                                d[j..].starts_with(b"endstream")
                            }
                        })
                        .map(|l| ini + l)
                        .or_else(|| {
                            let mut f = buscar(d, b"endstream", ini)?;
                            // Sin el fin de linea de antes.
                            if f > ini && d[f - 1] == b'\n' {
                                f -= 1;
                            }
                            if f > ini && d[f - 1] == b'\r' {
                                f -= 1;
                            }
                            Some(f)
                        })
                        .unwrap_or(d.len());
                    i = fin;
                    Obj::Flujo(Box::new(Flujo {
                        dic,
                        datos: d[ini..fin].to_vec(),
                    }))
                }
                (o, _) => {
                    i = guardado;
                    o
                }
            };
            if let Obj::Flujo(f) = &o {
                match f.dic.get("Type").and_then(Obj::nombre) {
                    Some("XRef") => {
                        cifrado |= f.dic.contains_key("Encrypt");
                        if let Some(r) = f.dic.get("Root") {
                            doc.raiz = Some(r.clone());
                        }
                    }
                    Some("ObjStm") => {
                        let f = (**f).clone();
                        doc.objetos.insert(num, o);
                        doc.abrir_flujo_de_objetos(&f);
                        continue;
                    }
                    _ => {}
                }
            }
            doc.objetos.insert(num, o);
        }
        // Los «trailer» de las tablas xref de siempre.
        let mut j = 0;
        while let Some(k) = buscar(d, b"trailer", j) {
            j = k + 7;
            let mut lx = Lexico { d, i: j };
            if let Some(Obj::Dic(t)) = objeto(&mut lx, 0) {
                cifrado |= t.contains_key("Encrypt");
                if let Some(r) = t.get("Root") {
                    doc.raiz = Some(r.clone());
                }
            }
        }
        if cifrado {
            return Err("el PDF esta cifrado (protegido con contrasena o permisos)".into());
        }
        if doc.raiz.as_ref().and_then(|r| doc.dic(r)).is_none() {
            // Sin trailer valido: el primer catalogo que haya.
            doc.raiz = doc
                .objetos
                .iter()
                .find(|(_, o)| {
                    matches!(o, Obj::Dic(d) if d.get("Type").and_then(Obj::nombre) == Some("Catalog"))
                })
                .map(|(n, _)| Obj::Ref(*n, 0));
        }
        if doc.raiz.is_none() {
            return Err("el PDF esta danado: no tiene catalogo".into());
        }
        Ok(doc)
    }

    /// Mete los objetos de un `/ObjStm`.
    fn abrir_flujo_de_objetos(&mut self, f: &Flujo) {
        let Ok(datos) = self.decodificar(f) else {
            return;
        };
        let n = f.dic.get("N").and_then(Obj::num).unwrap_or(0.0) as usize;
        let primero = f.dic.get("First").and_then(Obj::num).unwrap_or(0.0) as usize;
        let mut lx = Lexico::new(&datos);
        let mut pares = Vec::new();
        for _ in 0..n.min(1_000_000) {
            match (lx.siguiente(), lx.siguiente()) {
                (Some(Tok::Num(a, _)), Some(Tok::Num(b, _))) => pares.push((a as u32, b as usize)),
                _ => break,
            }
        }
        for (num, off) in pares {
            let Some(ini) = primero.checked_add(off).filter(|i| *i < datos.len()) else {
                continue;
            };
            let mut lx = Lexico { d: &datos, i: ini };
            if let Some(o) = objeto(&mut lx, 0) {
                self.objetos.insert(num, o);
            }
        }
    }

    /// **Sigue las referencias** hasta un objeto de verdad.
    pub fn resolver<'a>(&'a self, mut o: &'a Obj) -> &'a Obj {
        for _ in 0..32 {
            match o {
                Obj::Ref(n, _) => match self.objetos.get(n) {
                    Some(x) => o = x,
                    None => return &Obj::Nulo,
                },
                _ => return o,
            }
        }
        &Obj::Nulo
    }

    /// El diccionario de un objeto (o el de su flujo).
    pub fn dic<'a>(&'a self, o: &'a Obj) -> Option<&'a Dic> {
        match self.resolver(o) {
            Obj::Dic(d) => Some(d),
            Obj::Flujo(f) => Some(&f.dic),
            _ => None,
        }
    }

    /// Lo que vale `llave` en un diccionario, ya resuelto.
    pub fn en<'a>(&'a self, d: &'a Dic, llave: &str) -> &'a Obj {
        d.get(llave).map_or(&Obj::Nulo, |o| self.resolver(o))
    }

    pub fn num(&self, d: &Dic, llave: &str) -> Option<f64> {
        self.en(d, llave).num()
    }

    /// Un arreglo de numeros (`/MediaBox`, `/Matrix`).
    pub fn numeros(&self, o: &Obj) -> Vec<f64> {
        match self.resolver(o) {
            Obj::Arr(v) => v.iter().filter_map(|x| self.resolver(x).num()).collect(),
            _ => Vec::new(),
        }
    }

    pub fn catalogo(&self) -> Option<&Dic> {
        self.dic(self.raiz.as_ref()?)
    }

    /// **Los datos de un flujo**, sin sus filtros.
    pub fn decodificar(&self, f: &Flujo) -> Result<Vec<u8>, String> {
        let filtros: Vec<String> = match self.en(&f.dic, "Filter") {
            Obj::Nombre(n) => vec![n.clone()],
            Obj::Arr(v) => v
                .iter()
                .filter_map(|x| self.resolver(x).nombre().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        let parms: Vec<Option<&Dic>> = match self.en(&f.dic, "DecodeParms") {
            Obj::Arr(v) => v.iter().map(|x| self.dic(x)).collect(),
            o => vec![self.dic(o)],
        };
        let mut datos = f.datos.clone();
        for (k, filtro) in filtros.iter().enumerate() {
            let p = parms.get(k).copied().flatten();
            datos = match filtro.as_str() {
                "FlateDecode" | "Fl" => {
                    let v = inflar(&datos)?;
                    self.predictor(v, p)
                }
                "LZWDecode" | "LZW" => {
                    let temprano = p.and_then(|p| self.num(p, "EarlyChange")).unwrap_or(1.0);
                    let v = lzw(&datos, temprano != 0.0);
                    self.predictor(v, p)
                }
                "ASCIIHexDecode" | "AHx" => {
                    let mut con = Vec::with_capacity(datos.len() + 2);
                    con.push(b'<');
                    con.extend(datos.iter().copied().take_while(|c| *c != b'>'));
                    con.push(b'>');
                    match Lexico::new(&con).siguiente() {
                        Some(Tok::Cadena(v)) => v,
                        _ => Vec::new(),
                    }
                }
                "ASCII85Decode" | "A85" => ascii85(&datos),
                "RunLengthDecode" | "RL" => run_length(&datos),
                otro => return Err(format!("filtro {otro} no soportado")),
            };
        }
        Ok(datos)
    }

    /// El predictor PNG o TIFF de `DecodeParms` (flujos de xref, imagenes).
    fn predictor(&self, v: Vec<u8>, p: Option<&Dic>) -> Vec<u8> {
        let Some(p) = p else { return v };
        let pred = self.num(p, "Predictor").unwrap_or(1.0) as i32;
        if pred < 10 {
            return v;
        }
        let colores = self.num(p, "Colors").unwrap_or(1.0).max(1.0) as usize;
        let bits = self.num(p, "BitsPerComponent").unwrap_or(8.0).max(1.0) as usize;
        let columnas = self.num(p, "Columns").unwrap_or(1.0).max(1.0) as usize;
        let bpp = (colores * bits).div_ceil(8).max(1);
        let fila = (colores * bits * columnas).div_ceil(8);
        despredecir_png(&v, fila, bpp)
    }

    /// **Las paginas**, en orden, con lo que heredan.
    pub fn paginas(&self) -> Vec<Pagina<'_>> {
        let mut v = Vec::new();
        let Some(cat) = self.catalogo() else {
            return v;
        };
        let mut vistos = HashSet::new();
        self.recorrer_paginas(
            cat.get("Pages").unwrap_or(&Obj::Nulo),
            Herencia::default(),
            &mut vistos,
            &mut v,
            0,
        );
        v
    }

    fn recorrer_paginas<'a>(
        &'a self,
        nodo: &'a Obj,
        mut her: Herencia<'a>,
        vistos: &mut HashSet<u32>,
        out: &mut Vec<Pagina<'a>>,
        prof: u32,
    ) {
        if prof > 64 || out.len() >= TOPE_PAGINAS {
            return;
        }
        if let Obj::Ref(n, _) = nodo
            && !vistos.insert(*n)
        {
            return;
        }
        let Some(d) = self.dic(nodo) else { return };
        if let Some(r) = d.get("Resources") {
            her.recursos = Some(r);
        }
        if let Some(m) = d.get("MediaBox") {
            her.medios = Some(m);
        }
        if let Some(m) = d.get("CropBox") {
            her.recorte = Some(m);
        }
        if let Some(r) = self.num(d, "Rotate") {
            her.giro = r as i32;
        }
        match self.en(d, "Kids") {
            Obj::Arr(hijos) if d.get("Type").and_then(Obj::nombre) != Some("Page") => {
                for h in hijos {
                    self.recorrer_paginas(h, her, vistos, out, prof + 1);
                }
            }
            _ => {
                let caja = |o: Option<&Obj>| {
                    let v = o.map(|o| self.numeros(o)).unwrap_or_default();
                    (v.len() == 4).then(|| {
                        [
                            v[0].min(v[2]),
                            v[1].min(v[3]),
                            v[0].max(v[2]),
                            v[1].max(v[3]),
                        ]
                    })
                };
                let medios = caja(her.medios).unwrap_or([0.0, 0.0, 612.0, 792.0]);
                let caja_final = caja(her.recorte)
                    .map(|c| {
                        [
                            c[0].max(medios[0]),
                            c[1].max(medios[1]),
                            c[2].min(medios[2]),
                            c[3].min(medios[3]),
                        ]
                    })
                    .filter(|c| c[2] > c[0] && c[3] > c[1])
                    .unwrap_or(medios);
                out.push(Pagina {
                    dic: d,
                    recursos: her.recursos,
                    caja: caja_final,
                    giro: her.giro.rem_euclid(360),
                });
            }
        }
    }

    /// **El contenido de una pagina** (sus flujos uno tras otro).
    pub fn contenido(&self, p: &Pagina) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        let mut poner = |o: &Obj| -> Result<(), String> {
            if let Obj::Flujo(f) = self.resolver(o) {
                let v = self.decodificar(f)?;
                if out.len() + v.len() > TOPE_FLUJO {
                    return Err("el contenido de la pagina es demasiado grande".into());
                }
                out.extend_from_slice(&v);
                out.push(b'\n');
            }
            Ok(())
        };
        match self.en(p.dic, "Contents") {
            Obj::Arr(v) => {
                for o in v {
                    poner(o)?;
                }
            }
            o @ Obj::Flujo(_) => poner(o)?,
            _ => {}
        }
        Ok(out)
    }
}

/// Lo que una pagina hereda de sus padres.
#[derive(Clone, Copy, Default)]
struct Herencia<'a> {
    recursos: Option<&'a Obj>,
    medios: Option<&'a Obj>,
    recorte: Option<&'a Obj>,
    giro: i32,
}

/// **Una pagina**: su diccionario, sus recursos, su caja visible (puntos) y
/// cuanto se gira al verla (0, 90, 180, 270).
pub struct Pagina<'a> {
    pub dic: &'a Dic,
    pub recursos: Option<&'a Obj>,
    pub caja: [f64; 4],
    pub giro: i32,
}

// ----------------------------------------------------------------- filtros

/// Descomprime zlib (o deflate sin cabecera); si se corta, lo que salio.
pub fn inflar(d: &[u8]) -> Result<Vec<u8>, String> {
    let leer = |r: &mut dyn Read| -> (Vec<u8>, bool) {
        let mut v = Vec::new();
        let ok = r.take(TOPE_FLUJO as u64 + 1).read_to_end(&mut v).is_ok();
        (v, ok)
    };
    let (v, ok) = leer(&mut flate2::read::ZlibDecoder::new(d));
    let v = if ok && !v.is_empty() {
        v
    } else {
        let (v2, ok2) = leer(&mut flate2::read::DeflateDecoder::new(d));
        if (ok2 && !v2.is_empty()) || v2.len() > v.len() {
            v2
        } else if !v.is_empty() {
            v
        } else {
            return Err("flujo comprimido danado".into());
        }
    };
    if v.len() > TOPE_FLUJO {
        return Err("un flujo descomprimido pasa de 256 MB".into());
    }
    Ok(v)
}

fn despredecir_png(v: &[u8], fila: usize, bpp: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len());
    let mut previa = vec![0u8; fila];
    for trozo in v.chunks(fila + 1) {
        if trozo.len() < 2 {
            break;
        }
        let tipo = trozo[0];
        let mut actual = trozo[1..].to_vec();
        actual.resize(fila, 0);
        for i in 0..fila {
            let a = if i >= bpp { actual[i - bpp] } else { 0 } as i16;
            let b = previa[i] as i16;
            let c = if i >= bpp { previa[i - bpp] } else { 0 } as i16;
            let x = actual[i];
            actual[i] = match tipo {
                1 => x.wrapping_add(a as u8),
                2 => x.wrapping_add(b as u8),
                3 => x.wrapping_add(((a + b) / 2) as u8),
                4 => {
                    let p = a + b - c;
                    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                    let pr = if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    };
                    x.wrapping_add(pr as u8)
                }
                _ => x,
            };
        }
        out.extend_from_slice(&actual);
        previa = actual;
    }
    out
}

fn ascii85(d: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut grupo = [0u32; 5];
    let mut n = 0;
    let datos = d.strip_prefix(b"<~").unwrap_or(d);
    for &c in datos {
        match c {
            b'~' => break,
            b'z' if n == 0 => out.extend_from_slice(&[0, 0, 0, 0]),
            b'!'..=b'u' => {
                grupo[n] = (c - b'!') as u32;
                n += 1;
                if n == 5 {
                    let v = grupo
                        .iter()
                        .fold(0u32, |a, g| a.wrapping_mul(85).wrapping_add(*g));
                    out.extend_from_slice(&v.to_be_bytes());
                    n = 0;
                }
            }
            _ => {}
        }
    }
    if n > 1 {
        for g in grupo.iter_mut().skip(n) {
            *g = 84;
        }
        let v = grupo
            .iter()
            .fold(0u32, |a, g| a.wrapping_mul(85).wrapping_add(*g));
        out.extend_from_slice(&v.to_be_bytes()[..n - 1]);
    }
    out
}

fn run_length(d: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < d.len() {
        let n = d[i];
        i += 1;
        match n {
            128 => break,
            0..=127 => {
                let fin = (i + n as usize + 1).min(d.len());
                out.extend_from_slice(&d[i..fin]);
                i = fin;
            }
            _ => {
                if let Some(&b) = d.get(i) {
                    out.extend(std::iter::repeat_n(b, 257 - n as usize));
                }
                i += 1;
            }
        }
        if out.len() > TOPE_FLUJO {
            break;
        }
    }
    out
}

fn lzw(d: &[u8], temprano: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut tabla: Vec<Vec<u8>> = (0..=255u8).map(|b| vec![b]).collect();
    tabla.push(Vec::new());
    tabla.push(Vec::new());
    let mut ancho = 9;
    let (mut bits, mut nbits) = (0u32, 0u32);
    let mut previo: Option<Vec<u8>> = None;
    for &b in d {
        bits = bits << 8 | b as u32;
        nbits += 8;
        while nbits >= ancho {
            let codigo = ((bits >> (nbits - ancho)) & ((1 << ancho) - 1)) as usize;
            nbits -= ancho;
            match codigo {
                256 => {
                    tabla.truncate(258);
                    ancho = 9;
                    previo = None;
                    continue;
                }
                257 => return out,
                _ => {}
            }
            let entrada = if codigo < tabla.len() {
                tabla[codigo].clone()
            } else if let Some(p) = &previo {
                let mut e = p.clone();
                e.push(p[0]);
                e
            } else {
                return out;
            };
            out.extend_from_slice(&entrada);
            if let Some(mut p) = previo.take()
                && tabla.len() < 4096
            {
                p.push(entrada[0]);
                tabla.push(p);
            }
            previo = Some(entrada);
            let lim = tabla.len() + usize::from(temprano);
            ancho = if lim >= 2048 {
                12
            } else if lim >= 1024 {
                11
            } else if lim >= 512 {
                10
            } else {
                9
            };
            if out.len() > TOPE_FLUJO {
                return out;
            }
        }
    }
    out
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lexico_de_lo_tipico() {
        let mut lx = Lexico::new(b"<< /A 1 /B [2 0 R (a\\(b\\)) <4142>] /C#20D -.5 >> % x\n true");
        let o = objeto(&mut lx, 0).unwrap();
        let Obj::Dic(d) = o else { panic!() };
        assert_eq!(d["A"], Obj::Num(1.0));
        assert_eq!(
            d["B"],
            Obj::Arr(vec![
                Obj::Ref(2, 0),
                Obj::Cadena(b"a(b)".to_vec()),
                Obj::Cadena(b"AB".to_vec())
            ])
        );
        assert_eq!(d["C D"], Obj::Num(-0.5));
        assert_eq!(objeto(&mut lx, 0), Some(Obj::Bool(true)));
        assert_eq!(objeto(&mut lx, 0), None);
    }

    #[test]
    fn filtros_sencillos() {
        assert_eq!(ascii85(b"87cURD]i,\"Ebo80~>"), b"Hello World!");
        assert_eq!(
            run_length(&[2, b'a', b'b', b'c', 254, b'x', 128]),
            b"abcxxx"
        );
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut z, b"hola hola hola").unwrap();
        assert_eq!(inflar(&z.finish().unwrap()).unwrap(), b"hola hola hola");
        assert!(inflar(b"\xff\xff").is_err());
    }

    #[test]
    fn lo_que_no_es_pdf_o_esta_roto() {
        assert!(Documento::leer(b"hola").is_err());
        assert!(Documento::leer(b"%PDF-1.4\n1 0 obj << /A 1 >> endobj").is_err());
        let cifrado = b"%PDF-1.4\n1 0 obj << /Type /Catalog >> endobj\ntrailer << /Root 1 0 R /Encrypt 2 0 R >>";
        let e = Documento::leer(cifrado).unwrap_err();
        assert!(e.contains("cifrado"), "{e}");
    }
}
