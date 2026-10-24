//! **Pegar las paginas de un PDF detras de las de otro, sin rasterizar nada.**
//! Puerto de `motor/PdfUnion.kt` del movil.
//!
//! Un proyecto tiene *un* documento, y sus hojas de PDF son estaticas y
//! vectoriales porque son paginas de ese documento. Cuando se une al proyecto
//! otro PDF y el proyecto ya tenia uno, las paginas nuevas tienen que ser
//! **paginas de verdad del mismo documento** (el movil lo aprendio el
//! 5-sep-2026: meterlas como fotos en lienzos las dejaba movibles y en
//! pixeles). Aqui se hace lo mismo que alli: las paginas del segundo se
//! **copian dentro del primero** con todo lo que alcanzan (contenido, fuentes,
//! imagenes), con numeros nuevos, colgadas de un nodo de paginas nuevo, y el
//! arbol del primero se reescribe con ese nodo de mas.
//!
//! Se escribe **de forma incremental**: ni un byte del original se mueve, se
//! anade detras. Si el original lleva su indice en un flujo (`/Type /XRef`,
//! PDF 1.5+), el anadido tambien, que mezclar las dos clases de indice es lo
//! que algunos lectores no perdonan.
//!
//! Por que un analizador propio y no una libreria: `Windows.Data.Pdf` solo
//! dibuja, no deja tocar objetos (ver la cabecera de `aligerar`), y meter
//! pdfium o lopdf son megas de binario para un pegado. Este lee lo que hace
//! falta para copiar objetos —los dos indices, los flujos de objetos y los
//! filtros Flate con predictor PNG— y nada mas. **Con un PDF cifrado no se
//! intenta** (habria que descifrar cada cadena y cada flujo): devuelve `None`
//! y quien llama cae a pegar las paginas pintadas, como el movil.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};

/// Lo que una pagina hereda de sus padres y hay que dejar escrito en ella al
/// copiarla, porque sus padres no viajan.
const HEREDABLES: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

/// Tope de objetos copiados, por si un PDF roto se referencia sin fin.
const MAX_OBJETOS: usize = 200_000;

/// Tope de elementos de una lista o un diccionario: mas es un archivo roto.
const MAX_ELEMENTOS: usize = 200_000;

/// Un valor de la sintaxis del PDF.
///
/// Los numeros, las cadenas y los nombres se guardan **tal como venian
/// escritos**: copiar un objeto es volver a escribir lo mismo, y reescribir
/// una cadena con otro escapado o un real con otros decimales es arriesgarse
/// a cambiar lo que dice sin ganar nada.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Valor {
    Nulo,
    Booleano(bool),
    Numero(Vec<u8>),
    /// Sin la barra.
    Nombre(Vec<u8>),
    /// Con sus delimitadores, `(...)` o `<...>`.
    Cadena(Vec<u8>),
    Lista(Vec<Valor>),
    Dicc(Dicc),
    Ref(u32, u16),
    /// El diccionario y los datos tal cual (sin descomprimir).
    Flujo(Dicc, Vec<u8>),
}

pub(crate) type Dicc = Vec<(Vec<u8>, Valor)>;

pub(crate) fn en<'a>(d: &'a Dicc, clave: &[u8]) -> Option<&'a Valor> {
    d.iter().find(|(k, _)| k == clave).map(|(_, v)| v)
}

pub(crate) fn poner(d: &mut Dicc, clave: &[u8], v: Valor) {
    match d.iter_mut().find(|(k, _)| k == clave) {
        Some((_, viejo)) => *viejo = v,
        None => d.push((clave.to_vec(), v)),
    }
}

pub(crate) fn entero(v: Option<&Valor>) -> Option<i64> {
    match v {
        Some(Valor::Numero(t)) => std::str::from_utf8(t).ok()?.split('.').next()?.parse().ok(),
        _ => None,
    }
}

pub(crate) fn numero(n: i64) -> Valor {
    Valor::Numero(n.to_string().into_bytes())
}

fn es_blanco(c: u8) -> bool {
    matches!(c, 0 | 9 | 10 | 12 | 13 | 32)
}

fn es_delimitador(c: u8) -> bool {
    matches!(
        c,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn buscar(b: &[u8], desde: usize, aguja: &[u8]) -> Option<usize> {
    if aguja.is_empty() || desde >= b.len() {
        return None;
    }
    b[desde..]
        .windows(aguja.len())
        .position(|w| w == aguja)
        .map(|i| desde + i)
}

/// El lector de la sintaxis, sobre bytes y nunca sobre texto: entre los
/// objetos hay flujos binarios que un decodificador de caracteres romperia.
struct Lector<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Lector<'a> {
    fn saltar_blancos(&mut self) {
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            if es_blanco(c) {
                self.pos += 1;
            } else if c == b'%' {
                // Un comentario llega hasta el final de la linea.
                while self.pos < self.b.len() && !matches!(self.b[self.pos], b'\n' | b'\r') {
                    self.pos += 1;
                }
            } else {
                return;
            }
        }
    }

    fn palabra(&mut self, p: &[u8]) -> bool {
        self.saltar_blancos();
        if self.b[self.pos.min(self.b.len())..].starts_with(p) {
            let tras = self.pos + p.len();
            // «R» no puede ser el principio de «Rotate».
            if tras < self.b.len() && !es_blanco(self.b[tras]) && !es_delimitador(self.b[tras]) {
                return false;
            }
            self.pos = tras;
            true
        } else {
            false
        }
    }

    fn entero_crudo(&mut self) -> Option<(Vec<u8>, bool)> {
        self.saltar_blancos();
        let inicio = self.pos;
        if self.pos < self.b.len() && matches!(self.b[self.pos], b'+' | b'-') {
            self.pos += 1;
        }
        let mut digitos = 0;
        let mut punto = false;
        while self.pos < self.b.len() {
            match self.b[self.pos] {
                b'0'..=b'9' => digitos += 1,
                b'.' if !punto => punto = true,
                _ => break,
            }
            self.pos += 1;
        }
        if digitos == 0 {
            self.pos = inicio;
            return None;
        }
        Some((self.b[inicio..self.pos].to_vec(), punto))
    }

    /// Un valor entero. `archivo` solo hace falta para la longitud de un
    /// flujo que venga como referencia.
    fn valor(&mut self, archivo: Option<&Archivo>, hondo: u32) -> Option<Valor> {
        if hondo > 256 {
            return None;
        }
        self.saltar_blancos();
        let c = *self.b.get(self.pos)?;
        match c {
            b'<' if self.b.get(self.pos + 1) == Some(&b'<') => self.dicc_o_flujo(archivo, hondo),
            b'<' => {
                let fin = buscar(self.b, self.pos, b">")? + 1;
                let v = Valor::Cadena(self.b[self.pos..fin].to_vec());
                self.pos = fin;
                Some(v)
            }
            b'(' => {
                let inicio = self.pos;
                self.pos += 1;
                let mut nivel = 1;
                while self.pos < self.b.len() && nivel > 0 {
                    match self.b[self.pos] {
                        b'\\' => self.pos += 1,
                        b'(' => nivel += 1,
                        b')' => nivel -= 1,
                        _ => {}
                    }
                    self.pos += 1;
                }
                Some(Valor::Cadena(
                    self.b[inicio..self.pos.min(self.b.len())].to_vec(),
                ))
            }
            b'[' => {
                self.pos += 1;
                let mut v = Vec::new();
                loop {
                    self.saltar_blancos();
                    match self.b.get(self.pos) {
                        None => break,
                        Some(b']') => {
                            self.pos += 1;
                            break;
                        }
                        _ => {}
                    }
                    v.push(self.valor(archivo, hondo + 1)?);
                    if v.len() > MAX_ELEMENTOS {
                        return None;
                    }
                }
                Some(Valor::Lista(v))
            }
            b'/' => {
                self.pos += 1;
                let inicio = self.pos;
                while self.pos < self.b.len()
                    && !es_blanco(self.b[self.pos])
                    && !es_delimitador(self.b[self.pos])
                {
                    self.pos += 1;
                }
                Some(Valor::Nombre(self.b[inicio..self.pos].to_vec()))
            }
            b']' | b'>' | b')' | b'}' => None,
            _ => {
                if self.palabra(b"true") {
                    return Some(Valor::Booleano(true));
                }
                if self.palabra(b"false") {
                    return Some(Valor::Booleano(false));
                }
                if self.palabra(b"null") {
                    return Some(Valor::Nulo);
                }
                let (texto, punto) = self.entero_crudo()?;
                if !punto {
                    // `12 0 R` es una referencia; `12 0` a secas, dos numeros.
                    let guardado = self.pos;
                    if let Some((generacion, false)) = self.entero_crudo()
                        && self.palabra(b"R")
                    {
                        let n = std::str::from_utf8(&texto).ok()?.parse().ok()?;
                        let g = std::str::from_utf8(&generacion).ok()?.parse().ok()?;
                        return Some(Valor::Ref(n, g));
                    }
                    self.pos = guardado;
                }
                Some(Valor::Numero(texto))
            }
        }
    }

    fn dicc_o_flujo(&mut self, archivo: Option<&Archivo>, hondo: u32) -> Option<Valor> {
        self.pos += 2;
        let mut d: Dicc = Vec::new();
        loop {
            self.saltar_blancos();
            if self.b[self.pos.min(self.b.len())..].starts_with(b">>") {
                self.pos += 2;
                break;
            }
            let Valor::Nombre(k) = self.valor(archivo, hondo + 1)? else {
                return None;
            };
            let v = self.valor(archivo, hondo + 1)?;
            d.push((k, v));
            if d.len() > MAX_ELEMENTOS {
                return None;
            }
        }
        let guardado = self.pos;
        if !self.palabra(b"stream") {
            self.pos = guardado;
            return Some(Valor::Dicc(d));
        }
        // Tras `stream` va un salto (o retorno y salto) y luego los datos.
        if self.b.get(self.pos) == Some(&b'\r') {
            self.pos += 1;
        }
        if self.b.get(self.pos) == Some(&b'\n') {
            self.pos += 1;
        }
        let inicio = self.pos;
        let declarado = match en(&d, b"Length") {
            Some(Valor::Ref(n, _)) => archivo.and_then(|a| entero(a.objeto(*n).as_ref())),
            v => entero(v),
        };
        // **La longitud declarada no se cree a ciegas**: hay archivos
        // remendados donde miente. Se comprueba que detras venga `endstream`.
        let fin = match declarado {
            Some(l)
                if l >= 0 && inicio + l as usize <= self.b.len() && {
                    let mut i = inicio + l as usize;
                    while i < self.b.len() && es_blanco(self.b[i]) && i < inicio + l as usize + 4 {
                        i += 1;
                    }
                    self.b[i..].starts_with(b"endstream")
                } =>
            {
                inicio + l as usize
            }
            _ => {
                let f = buscar(self.b, inicio, b"endstream")?;
                // Sin el salto de linea de antes de `endstream`.
                let mut f2 = f;
                if f2 > inicio && self.b[f2 - 1] == b'\n' {
                    f2 -= 1;
                }
                if f2 > inicio && self.b[f2 - 1] == b'\r' {
                    f2 -= 1;
                }
                f2
            }
        };
        let datos = self.b[inicio..fin].to_vec();
        self.pos = fin;
        self.palabra(b"endstream");
        Some(Valor::Flujo(d, datos))
    }
}

/// Donde esta un objeto.
#[derive(Clone, Copy, Debug)]
enum Ubic {
    Directo(usize),
    /// Dentro del flujo de objetos `n`, en la posicion `i`.
    EnFlujo(u32, u32),
}

/// Un flujo de objetos ya descomprimido: sus datos, donde empieza el primer
/// objeto y la tabla de (numero, desplazamiento).
type FlujoDeObjetos = std::rc::Rc<(Vec<u8>, usize, Vec<(u32, usize)>)>;

/// Un PDF leido lo justo para ir a buscar sus objetos.
pub(crate) struct Archivo<'a> {
    b: &'a [u8],
    ubic: HashMap<u32, Ubic>,
    /// El trailer mas nuevo (o el diccionario del flujo de indice mas nuevo).
    trailer: Dicc,
    /// Donde empieza el indice mas nuevo: el `/Prev` del anadido.
    ultimo_indice: usize,
    /// Si el indice mas nuevo es un flujo (PDF 1.5+).
    indice_en_flujo: bool,
    flujos: std::cell::RefCell<HashMap<u32, FlujoDeObjetos>>,
}

impl<'a> Archivo<'a> {
    pub(crate) fn leer(b: &'a [u8]) -> Option<Archivo<'a>> {
        if buscar(b, 0, b"%PDF-").is_none_or(|i| i > 1024) {
            return None;
        }
        let mut a = Archivo {
            b,
            ubic: HashMap::new(),
            trailer: Vec::new(),
            ultimo_indice: 0,
            indice_en_flujo: false,
            flujos: Default::default(),
        };
        let bien = a.leer_indices().is_some() && a.raiz().is_some();
        if !bien {
            // Un indice roto no es el final: los objetos siguen ahi escritos,
            // y se pueden encontrar recorriendo el fichero. Es lo que hacen
            // todos los lectores con un PDF remendado.
            a.reconstruir()?;
        }
        Some(a)
    }

    fn leer_indices(&mut self) -> Option<()> {
        let sx = self.b.windows(9).rposition(|w| w == b"startxref")?;
        let mut l = Lector {
            b: self.b,
            pos: sx + 9,
        };
        let (t, _) = l.entero_crudo()?;
        let inicio: usize = std::str::from_utf8(&t).ok()?.parse().ok()?;
        self.ultimo_indice = inicio;
        let mut cola = vec![inicio];
        let mut vistos = std::collections::HashSet::new();
        let mut primero = true;
        while let Some(off) = cola.pop() {
            if off >= self.b.len() || !vistos.insert(off) || vistos.len() > 512 {
                continue;
            }
            let mut l = Lector {
                b: self.b,
                pos: off,
            };
            l.saltar_blancos();
            let trailer = if self.b[l.pos..].starts_with(b"xref") {
                l.pos += 4;
                self.tabla_clasica(&mut l)?;
                if !l.palabra(b"trailer") {
                    return None;
                }
                let Valor::Dicc(d) = l.valor(None, 0)? else {
                    return None;
                };
                if primero {
                    self.indice_en_flujo = false;
                }
                d
            } else {
                let (_, _, v) = self.objeto_en(off, None)?;
                let Valor::Flujo(d, datos) = v else {
                    return None;
                };
                self.flujo_de_indice(&d, &datos)?;
                if primero {
                    self.indice_en_flujo = true;
                }
                d
            };
            if primero {
                self.trailer = trailer.clone();
                primero = false;
            }
            // Los mas viejos despues: lo ya puesto (mas nuevo) manda.
            if let Some(p) = entero(en(&trailer, b"Prev")) {
                cola.push(p as usize);
            }
            if let Some(p) = entero(en(&trailer, b"XRefStm")) {
                cola.push(p as usize);
            }
        }
        Some(())
    }

    fn tabla_clasica(&mut self, l: &mut Lector) -> Option<()> {
        loop {
            let guardado = l.pos;
            let Some((desde, false)) = l.entero_crudo() else {
                l.pos = guardado;
                return Some(());
            };
            let Some((cuantos, false)) = l.entero_crudo() else {
                l.pos = guardado;
                return Some(());
            };
            let desde: u32 = std::str::from_utf8(&desde).ok()?.parse().ok()?;
            let cuantos: u32 = std::str::from_utf8(&cuantos).ok()?.parse().ok()?;
            for i in 0..cuantos {
                let (off, _) = l.entero_crudo()?;
                let (_gen, _) = l.entero_crudo()?;
                l.saltar_blancos();
                let marca = *self.b.get(l.pos)?;
                l.pos += 1;
                let n = desde + i;
                if marca == b'n' && !self.ubic.contains_key(&n) {
                    let off: usize = std::str::from_utf8(&off).ok()?.parse().ok()?;
                    if off > 0 {
                        self.ubic.insert(n, Ubic::Directo(off));
                    }
                } else if marca == b'f' {
                    // Libre en esta revision: una mas vieja no lo resucita.
                    self.ubic.entry(n).or_insert(Ubic::Directo(0));
                }
            }
        }
    }

    fn flujo_de_indice(&mut self, d: &Dicc, datos: &[u8]) -> Option<()> {
        let datos = descodificar(d, datos)?;
        let w: Vec<usize> = match en(d, b"W") {
            Some(Valor::Lista(l)) => l
                .iter()
                .map(|v| entero(Some(v)).unwrap_or(0) as usize)
                .collect(),
            _ => return None,
        };
        if w.len() < 3 || w.iter().any(|&x| x > 8) {
            return None;
        }
        let tamano = entero(en(d, b"Size")).unwrap_or(0);
        let tramos: Vec<(i64, i64)> = match en(d, b"Index") {
            Some(Valor::Lista(l)) => l
                .chunks(2)
                .filter_map(|p| Some((entero(p.first())?, entero(p.get(1))?)))
                .collect(),
            _ => vec![(0, tamano)],
        };
        let paso = w[0] + w[1] + w[2];
        if paso == 0 {
            return None;
        }
        let campo = |fila: &[u8], desde: usize, ancho: usize| -> u64 {
            fila[desde..desde + ancho]
                .iter()
                .fold(0u64, |acc, &b| (acc << 8) | b as u64)
        };
        let mut fila = 0usize;
        for (desde, cuantos) in tramos {
            for i in 0..cuantos.max(0) {
                let Some(f) = datos.get(fila * paso..(fila + 1) * paso) else {
                    return Some(());
                };
                fila += 1;
                let tipo = if w[0] == 0 { 1 } else { campo(f, 0, w[0]) };
                let b = campo(f, w[0], w[1]);
                let c = campo(f, w[0] + w[1], w[2]);
                let n = (desde + i) as u32;
                if self.ubic.contains_key(&n) {
                    continue;
                }
                match tipo {
                    0 => {
                        self.ubic.insert(n, Ubic::Directo(0));
                    }
                    1 => {
                        self.ubic.insert(n, Ubic::Directo(b as usize));
                    }
                    2 => {
                        self.ubic.insert(n, Ubic::EnFlujo(b as u32, c as u32));
                    }
                    _ => {}
                }
            }
        }
        Some(())
    }

    /// Recorre el fichero buscando `N G obj` cuando el indice no sirve.
    fn reconstruir(&mut self) -> Option<()> {
        self.ubic.clear();
        let mut i = 0;
        while let Some(j) = buscar(self.b, i, b"obj") {
            i = j + 3;
            // Hacia atras: «numero blanco generacion blanco obj».
            let mut k = j;
            while k > 0 && es_blanco(self.b[k - 1]) {
                k -= 1;
            }
            let fin_gen = k;
            while k > 0 && self.b[k - 1].is_ascii_digit() {
                k -= 1;
            }
            if k == fin_gen {
                continue;
            }
            while k > 0 && es_blanco(self.b[k - 1]) {
                k -= 1;
            }
            let fin_num = k;
            while k > 0 && self.b[k - 1].is_ascii_digit() {
                k -= 1;
            }
            if k == fin_num {
                continue;
            }
            if let Some(n) = std::str::from_utf8(&self.b[k..fin_num])
                .ok()
                .and_then(|t| t.parse::<u32>().ok())
            {
                self.ubic.insert(n, Ubic::Directo(k));
            }
        }
        // El trailer: el ultimo que haya, o el catalogo si no hay ninguno.
        if let Some(t) = self.b.windows(7).rposition(|w| w == b"trailer") {
            let mut l = Lector {
                b: self.b,
                pos: t + 7,
            };
            if let Some(Valor::Dicc(d)) = l.valor(None, 0) {
                self.trailer = d;
            }
        }
        if self.raiz().is_none() {
            let catalogo = self.ubic.keys().copied().find(|n| {
                matches!(self.objeto(*n), Some(Valor::Dicc(d)) if en(&d, b"Type") == Some(&Valor::Nombre(b"Catalog".to_vec())))
            })?;
            poner(&mut self.trailer, b"Root", Valor::Ref(catalogo, 0));
        }
        // El anadido no puede apuntar a un indice que no sirve.
        self.trailer
            .retain(|(k, _)| k != b"Prev" && k != b"XRefStm");
        self.ultimo_indice = 0;
        self.indice_en_flujo = false;
        Some(())
    }

    /// Lee `N G obj <valor>` en `off`.
    fn objeto_en(&self, off: usize, archivo: Option<&Archivo>) -> Option<(u32, u16, Valor)> {
        let mut l = Lector {
            b: self.b,
            pos: off,
        };
        let (n, _) = l.entero_crudo()?;
        let (g, _) = l.entero_crudo()?;
        if !l.palabra(b"obj") {
            return None;
        }
        let v = l.valor(archivo, 0)?;
        Some((
            std::str::from_utf8(&n).ok()?.parse().ok()?,
            std::str::from_utf8(&g).ok()?.parse().ok()?,
            v,
        ))
    }

    pub(crate) fn objeto(&self, n: u32) -> Option<Valor> {
        match *self.ubic.get(&n)? {
            Ubic::Directo(0) => None,
            Ubic::Directo(off) => self.objeto_en(off, Some(self)).map(|(_, _, v)| v),
            Ubic::EnFlujo(s, i) => {
                let flujo = self.flujo_de_objetos(s)?;
                let (datos, primero, tabla) = &*flujo;
                let (_, rel) = *tabla.get(i as usize)?;
                let mut l = Lector {
                    b: datos,
                    pos: primero + rel,
                };
                l.valor(None, 0)
            }
        }
    }

    fn flujo_de_objetos(&self, s: u32) -> Option<FlujoDeObjetos> {
        if let Some(f) = self.flujos.borrow().get(&s) {
            return Some(f.clone());
        }
        let Ubic::Directo(off) = *self.ubic.get(&s)? else {
            return None;
        };
        let (_, _, Valor::Flujo(d, crudo)) = self.objeto_en(off, Some(self))? else {
            return None;
        };
        let datos = descodificar(&d, &crudo)?;
        let cuantos = entero(en(&d, b"N"))? as usize;
        let primero = entero(en(&d, b"First"))? as usize;
        let mut l = Lector { b: &datos, pos: 0 };
        let mut tabla = Vec::with_capacity(cuantos.min(MAX_ELEMENTOS));
        for _ in 0..cuantos.min(MAX_ELEMENTOS) {
            let (n, _) = l.entero_crudo()?;
            let (o, _) = l.entero_crudo()?;
            tabla.push((
                std::str::from_utf8(&n).ok()?.parse().ok()?,
                std::str::from_utf8(&o).ok()?.parse().ok()?,
            ));
        }
        let f = std::rc::Rc::new((datos, primero, tabla));
        self.flujos.borrow_mut().insert(s, f.clone());
        Some(f)
    }

    pub(crate) fn resolver(&self, v: Option<&Valor>) -> Option<Valor> {
        match v? {
            Valor::Ref(n, _) => self.objeto(*n),
            otro => Some(otro.clone()),
        }
    }

    pub(crate) fn dicc_de(&self, v: Option<&Valor>) -> Option<Dicc> {
        match self.resolver(v)? {
            Valor::Dicc(d) | Valor::Flujo(d, _) => Some(d),
            _ => None,
        }
    }

    /// El trailer mas nuevo: donde esta el catalogo (`/Root`).
    pub(crate) fn trailer(&self) -> &Dicc {
        &self.trailer
    }

    pub(crate) fn raiz(&self) -> Option<Dicc> {
        self.dicc_de(en(&self.trailer, b"Root"))
    }

    pub(crate) fn cifrado(&self) -> bool {
        en(&self.trailer, b"Encrypt").is_some()
    }

    /// Los numeros de objeto de las paginas, en orden.
    pub(crate) fn paginas(&self) -> Vec<u32> {
        let mut salida = Vec::new();
        let Some(raiz) = self.raiz() else {
            return salida;
        };
        let Some(Valor::Ref(n, _)) = en(&raiz, b"Pages") else {
            return salida;
        };
        let mut pila = vec![*n];
        let mut vistos = std::collections::HashSet::new();
        while let Some(n) = pila.pop() {
            if !vistos.insert(n) || vistos.len() > MAX_OBJETOS {
                continue;
            }
            let Some(d) = self.dicc_de(Some(&Valor::Ref(n, 0))) else {
                continue;
            };
            match self.resolver(en(&d, b"Kids")) {
                Some(Valor::Lista(hijos)) => {
                    // Al reves en la pila, para sacarlos en orden.
                    for h in hijos.iter().rev() {
                        if let Valor::Ref(k, _) = h {
                            pila.push(*k);
                        }
                    }
                }
                _ => salida.push(n),
            }
        }
        salida
    }

    pub(crate) fn siguiente_libre(&self) -> u32 {
        let por_tamano = entero(en(&self.trailer, b"Size")).unwrap_or(0).max(0) as u32;
        let por_indice = self.ubic.keys().copied().max().map_or(1, |m| m + 1);
        por_tamano.max(por_indice).max(1)
    }
}

/// Los datos de un flujo sin su filtro. Solo Flate (con o sin predictor PNG)
/// o sin filtro: es lo que llevan los indices y los flujos de objetos.
pub(crate) fn descodificar(d: &Dicc, datos: &[u8]) -> Option<Vec<u8>> {
    let filtros: Vec<Vec<u8>> = match en(d, b"Filter") {
        None => Vec::new(),
        Some(Valor::Nombre(n)) => vec![n.clone()],
        Some(Valor::Lista(l)) => l
            .iter()
            .filter_map(|v| match v {
                Valor::Nombre(n) => Some(n.clone()),
                _ => None,
            })
            .collect(),
        _ => return None,
    };
    let mut salida = datos.to_vec();
    for f in &filtros {
        if f != b"FlateDecode" && f != b"Fl" {
            return None;
        }
        let mut z = flate2::read::ZlibDecoder::new(&salida[..]);
        let mut fuera = Vec::new();
        // Un flujo cortado al final aun da lo que llevaba: se acepta si salio algo.
        if z.read_to_end(&mut fuera).is_err() && fuera.is_empty() {
            return None;
        }
        salida = fuera;
    }
    let parametros = match en(d, b"DecodeParms") {
        Some(Valor::Dicc(p)) => Some(p.clone()),
        Some(Valor::Lista(l)) => l.iter().find_map(|v| match v {
            Valor::Dicc(p) => Some(p.clone()),
            _ => None,
        }),
        _ => None,
    };
    if let Some(p) = parametros {
        let predictor = entero(en(&p, b"Predictor")).unwrap_or(1);
        if predictor >= 10 {
            let columnas = entero(en(&p, b"Columns")).unwrap_or(1).max(1) as usize;
            salida = deshacer_png(&salida, columnas)?;
        } else if predictor != 1 {
            return None;
        }
    }
    Some(salida)
}

/// El predictor PNG de los indices: cada fila lleva delante su tipo.
pub(crate) fn deshacer_png(datos: &[u8], columnas: usize) -> Option<Vec<u8>> {
    let fila = columnas + 1;
    let mut salida = Vec::with_capacity(datos.len());
    let mut arriba = vec![0u8; columnas];
    for f in datos.chunks(fila) {
        if f.len() < fila {
            break;
        }
        let tipo = f[0];
        let mut actual = f[1..].to_vec();
        for i in 0..columnas {
            let izq = if i > 0 { actual[i - 1] } else { 0 };
            let sup = arriba[i];
            let sup_izq = if i > 0 { arriba[i - 1] } else { 0 };
            actual[i] = match tipo {
                0 => actual[i],
                1 => actual[i].wrapping_add(izq),
                2 => actual[i].wrapping_add(sup),
                3 => actual[i].wrapping_add(((izq as u16 + sup as u16) / 2) as u8),
                4 => {
                    let p = izq as i16 + sup as i16 - sup_izq as i16;
                    let (pa, pb, pc) = (
                        (p - izq as i16).abs(),
                        (p - sup as i16).abs(),
                        (p - sup_izq as i16).abs(),
                    );
                    let pred = if pa <= pb && pa <= pc {
                        izq
                    } else if pb <= pc {
                        sup
                    } else {
                        sup_izq
                    };
                    actual[i].wrapping_add(pred)
                }
                _ => return None,
            };
        }
        salida.extend_from_slice(&actual);
        arriba = actual;
    }
    Some(salida)
}

fn escribir(v: &Valor, s: &mut Vec<u8>) {
    match v {
        Valor::Nulo => s.extend_from_slice(b"null"),
        Valor::Booleano(true) => s.extend_from_slice(b"true"),
        Valor::Booleano(false) => s.extend_from_slice(b"false"),
        Valor::Numero(t) | Valor::Cadena(t) => s.extend_from_slice(t),
        Valor::Nombre(n) => {
            s.push(b'/');
            s.extend_from_slice(n);
        }
        Valor::Lista(l) => {
            s.push(b'[');
            for (i, x) in l.iter().enumerate() {
                if i > 0 {
                    s.push(b' ');
                }
                escribir(x, s);
            }
            s.push(b']');
        }
        Valor::Dicc(d) => escribir_dicc(d, s),
        Valor::Ref(n, g) => {
            let _ = write!(s, "{n} {g} R");
        }
        Valor::Flujo(d, datos) => {
            let mut d = d.clone();
            poner(&mut d, b"Length", numero(datos.len() as i64));
            escribir_dicc(&d, s);
            s.extend_from_slice(b"\nstream\n");
            s.extend_from_slice(datos);
            s.extend_from_slice(b"\nendstream");
        }
    }
}

fn escribir_dicc(d: &Dicc, s: &mut Vec<u8>) {
    s.extend_from_slice(b"<<");
    for (k, v) in d {
        s.push(b'/');
        s.extend_from_slice(k);
        s.push(b' ');
        escribir(v, s);
        s.push(b' ');
    }
    s.extend_from_slice(b">>");
}

/// **Cuantas paginas tiene**, sin pedirselo a Windows: leyendo el arbol de
/// paginas. Es barato (no dibuja nada) y no necesita COM, asi que vale para
/// contar desde el hilo de la ventana. `None` si no se entiende.
pub fn contar_paginas(bytes: &[u8]) -> Option<u32> {
    let a = Archivo::leer(bytes)?;
    let n = a.paginas().len() as u32;
    (n > 0).then_some(n)
}

/// Si el PDF esta cifrado (lleva `/Encrypt`). Uno cifrado no se deja pegar
/// objeto a objeto; hay que pintar sus paginas.
pub fn esta_cifrado(bytes: &[u8]) -> bool {
    Archivo::leer(bytes).is_some_and(|a| a.cifrado())
}

/// **El primer PDF con las paginas del segundo detras**, o `None` si no se
/// pudo (cualquiera de los dos cifrado o ilegible). El resultado se vuelve a
/// leer antes de devolverlo: si no tiene las paginas que tiene que tener, no
/// se entrega.
pub fn anadir_paginas(primero: &[u8], segundo: &[u8]) -> Option<Vec<u8>> {
    anadir(primero, segundo, None)
}

/// **Solo esas paginas de un PDF** (desde 0, en ese orden), como un PDF
/// nuevo: `PdfUnion.soloPaginas` del movil. Es lo que deja entregar la
/// pagina uno de un proyecto y las siete a diez de otro **como paginas de
/// verdad** —vectoriales, con su texto—, y no como fotos. `None` si no se
/// puede (cifrado, ilegible) o si no queda ninguna.
pub fn solo_paginas(origen: &[u8], indices: &[usize]) -> Option<Vec<u8>> {
    if indices.is_empty() {
        return None;
    }
    anadir(&en_blanco(), origen, Some(indices))
}

/// Un PDF valido sin ninguna pagina: la base sobre la que se pegan otras
/// (`PdfUnion.enBlanco` del movil).
pub fn en_blanco() -> Vec<u8> {
    let objetos = [
        "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
        "2 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n",
    ];
    let mut s = String::from("%PDF-1.4\n");
    let mut donde = Vec::new();
    for o in objetos {
        donde.push(s.len());
        s.push_str(o);
    }
    let xref = s.len();
    s.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objetos.len() + 1
    ));
    for d in donde {
        s.push_str(&format!("{d:010} 00000 n \n"));
    }
    s.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objetos.len() + 1
    ));
    s.into_bytes()
}

/// Lo de [`anadir_paginas`] con `cuales` paginas del segundo (todas con
/// `None`). Un primero sin paginas solo vale si es el [`en_blanco`] de
/// [`solo_paginas`]: pegar a un documento vacio que llego de fuera es mas
/// probable que sea un PDF roto que uno de verdad.
fn anadir(primero: &[u8], segundo: &[u8], cuales: Option<&[usize]>) -> Option<Vec<u8>> {
    let a = Archivo::leer(primero)?;
    let b = Archivo::leer(segundo)?;
    if a.cifrado() || b.cifrado() {
        return None;
    }
    let raiz_a = a.raiz()?;
    let Some(&Valor::Ref(n_paginas_a, g_paginas_a)) = en(&raiz_a, b"Pages") else {
        return None;
    };
    let paginas_a = a.dicc_de(Some(&Valor::Ref(n_paginas_a, g_paginas_a)))?;
    let antes = a.paginas().len();
    let todas_b = b.paginas();
    let paginas_b: Vec<u32> = match cuales {
        Some(c) => c.iter().filter_map(|&i| todas_b.get(i).copied()).collect(),
        None => todas_b,
    };
    if paginas_b.is_empty() || (antes == 0 && cuales.is_none()) {
        return None;
    }

    let mut siguiente = a.siguiente_libre();
    let mut nuevos: Vec<(u32, u16, Valor)> = Vec::new();
    let mut numero_de: HashMap<u32, u32> = HashMap::new();
    let mut pendientes: VecDeque<u32> = VecDeque::new();

    // Los objetos que se copian llevan sus referencias renumeradas.
    fn trasladar(
        v: &Valor,
        numero_de: &mut HashMap<u32, u32>,
        pendientes: &mut VecDeque<u32>,
        siguiente: &mut u32,
    ) -> Valor {
        match v {
            Valor::Ref(n, _) => {
                let nuevo = *numero_de.entry(*n).or_insert_with(|| {
                    pendientes.push_back(*n);
                    let s = *siguiente;
                    *siguiente += 1;
                    s
                });
                Valor::Ref(nuevo, 0)
            }
            Valor::Lista(l) => Valor::Lista(
                l.iter()
                    .map(|x| trasladar(x, numero_de, pendientes, siguiente))
                    .collect(),
            ),
            Valor::Dicc(d) => Valor::Dicc(
                d.iter()
                    .map(|(k, x)| (k.clone(), trasladar(x, numero_de, pendientes, siguiente)))
                    .collect(),
            ),
            Valor::Flujo(d, datos) => Valor::Flujo(
                d.iter()
                    .map(|(k, x)| (k.clone(), trasladar(x, numero_de, pendientes, siguiente)))
                    .collect(),
                datos.clone(),
            ),
            otro => otro.clone(),
        }
    }

    let nodo = siguiente;
    siguiente += 1;
    let mut refs = Vec::new();
    for n in paginas_b {
        let Some(pagina) = b.dicc_de(Some(&Valor::Ref(n, 0))) else {
            continue;
        };
        let mut entradas: Dicc = Vec::new();
        for (k, v) in &pagina {
            // El padre es el nodo nuevo; las anotaciones no viajan, que
            // apuntan a formularios y destinos de un archivo que se queda.
            if k == b"Parent" || k == b"Annots" {
                continue;
            }
            let t = trasladar(v, &mut numero_de, &mut pendientes, &mut siguiente);
            entradas.push((k.clone(), t));
        }
        for clave in HEREDABLES {
            if en(&entradas, clave).is_some() {
                continue;
            }
            let mut d = Some(pagina.clone());
            let mut saltos = 0;
            while let Some(actual) = d {
                if saltos > 64 {
                    break;
                }
                saltos += 1;
                if let Some(v) = en(&actual, clave) {
                    let t = trasladar(v, &mut numero_de, &mut pendientes, &mut siguiente);
                    entradas.push((clave.to_vec(), t));
                    break;
                }
                d = b.dicc_de(en(&actual, b"Parent"));
            }
        }
        poner(&mut entradas, b"Type", Valor::Nombre(b"Page".to_vec()));
        poner(&mut entradas, b"Parent", Valor::Ref(nodo, 0));
        let mia = siguiente;
        siguiente += 1;
        nuevos.push((mia, 0, Valor::Dicc(entradas)));
        refs.push(Valor::Ref(mia, 0));
    }
    if refs.is_empty() {
        return None;
    }
    // Todo lo que las paginas alcanzan, hasta que no quede nada por copiar.
    while let Some(de_b) = pendientes.pop_front() {
        if nuevos.len() > MAX_OBJETOS {
            return None;
        }
        let v = b.objeto(de_b).unwrap_or(Valor::Nulo);
        let t = trasladar(&v, &mut numero_de, &mut pendientes, &mut siguiente);
        nuevos.push((numero_de[&de_b], 0, t));
    }
    let cuantas = refs.len();
    nuevos.push((
        nodo,
        0,
        Valor::Dicc(vec![
            (b"Type".to_vec(), Valor::Nombre(b"Pages".to_vec())),
            (b"Parent".to_vec(), Valor::Ref(n_paginas_a, g_paginas_a)),
            (b"Kids".to_vec(), Valor::Lista(refs)),
            (b"Count".to_vec(), numero(cuantas as i64)),
        ]),
    ));
    // El arbol del primero, con un hijo mas y la cuenta al dia.
    let mut raiz_paginas = paginas_a.clone();
    let mut hijos = match a.resolver(en(&paginas_a, b"Kids")) {
        Some(Valor::Lista(l)) => l,
        _ => Vec::new(),
    };
    hijos.push(Valor::Ref(nodo, 0));
    poner(&mut raiz_paginas, b"Kids", Valor::Lista(hijos));
    let cuenta = entero(en(&paginas_a, b"Count")).unwrap_or(antes as i64);
    poner(&mut raiz_paginas, b"Count", numero(cuenta + cuantas as i64));
    nuevos.push((n_paginas_a, g_paginas_a, Valor::Dicc(raiz_paginas)));

    let salida = incremental(&a, primero, &nuevos, siguiente);
    // Se vuelve a leer: si no salen las que tienen que salir, no se entrega.
    let comprobado = Archivo::leer(&salida)?.paginas().len();
    (comprobado == antes + cuantas).then_some(salida)
}

/// Anade `nuevos` detras de `original` con su indice y su trailer.
pub(crate) fn incremental(
    a: &Archivo,
    original: &[u8],
    nuevos: &[(u32, u16, Valor)],
    libre: u32,
) -> Vec<u8> {
    let mut s = original.to_vec();
    if !s.ends_with(b"\n") {
        s.push(b'\n');
    }
    let mut sitios: Vec<(u32, u16, usize)> = Vec::with_capacity(nuevos.len() + 1);
    for (n, g, v) in nuevos {
        sitios.push((*n, *g, s.len()));
        let _ = writeln!(s, "{n} {g} obj");
        escribir(v, &mut s);
        s.extend_from_slice(b"\nendobj\n");
    }
    let mut trailer: Dicc = Vec::new();
    for clave in [&b"Root"[..], b"Info", b"ID"] {
        if let Some(v) = en(&a.trailer, clave) {
            trailer.push((clave.to_vec(), v.clone()));
        }
    }
    if a.ultimo_indice > 0 {
        trailer.push((b"Prev".to_vec(), numero(a.ultimo_indice as i64)));
    }
    if a.indice_en_flujo {
        // El indice como flujo, igual que el del original: sin filtro, que
        // son unas decenas de bytes por objeto y asi no hay nada que fallar.
        let propio = libre;
        let inicio = s.len();
        sitios.push((propio, 0, inicio));
        sitios.sort_by_key(|x| x.0);
        let mut datos = Vec::with_capacity(sitios.len() * 7);
        let mut index = Vec::new();
        for (n, g, off) in &sitios {
            datos.push(1u8);
            datos.extend_from_slice(&(*off as u32).to_be_bytes());
            datos.extend_from_slice(&g.to_be_bytes());
            index.push(numero(*n as i64));
            index.push(numero(1));
        }
        let mut d = trailer;
        d.insert(0, (b"Type".to_vec(), Valor::Nombre(b"XRef".to_vec())));
        d.push((b"Size".to_vec(), numero(propio as i64 + 1)));
        d.push((
            b"W".to_vec(),
            Valor::Lista(vec![numero(1), numero(4), numero(2)]),
        ));
        d.push((b"Index".to_vec(), Valor::Lista(index)));
        let _ = writeln!(s, "{propio} 0 obj");
        escribir(&Valor::Flujo(d, datos), &mut s);
        s.extend_from_slice(b"\nendobj\n");
        let _ = write!(s, "startxref\n{inicio}\n%%EOF\n");
    } else {
        let inicio = s.len();
        sitios.sort_by_key(|x| x.0);
        s.extend_from_slice(b"xref\n");
        for (n, g, off) in &sitios {
            let _ = write!(s, "{n} 1\n{off:010} {g:05} n\r\n");
        }
        let mut d = trailer;
        d.push((b"Size".to_vec(), numero(libre as i64)));
        s.extend_from_slice(b"trailer\n");
        escribir_dicc(&d, &mut s);
        let _ = write!(s, "\nstartxref\n{inicio}\n%%EOF\n");
    }
    s
}

/// **Un PDF con una imagen por pagina**, para las paginas que no se dejan
/// pegar tal cual (un PDF cifrado): se pintan, y lo pintado se pega como PDF
/// propio (`pegarComoPaginasPintadas` del movil). Cada pagina mide lo que un
/// A4 de ancho y el alto que pida la proporcion de su imagen.
pub fn de_imagenes(paginas: &[pixpin_codec::imagen::ImagenRgba]) -> Option<Vec<u8>> {
    let paginas: Vec<_> = paginas
        .iter()
        .filter(|p| p.ancho > 0 && p.alto > 0 && p.pixeles.len() >= (p.ancho * p.alto * 4) as usize)
        .collect();
    if paginas.is_empty() {
        return None;
    }
    let mut s = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut sitios: Vec<usize> = Vec::new();
    let objeto = |s: &mut Vec<u8>, sitios: &mut Vec<usize>, v: Valor| {
        sitios.push(s.len());
        let n = sitios.len();
        let _ = writeln!(s, "{n} 0 obj");
        escribir(&v, s);
        s.extend_from_slice(b"\nendobj\n");
    };
    // 1 catalogo, 2 arbol; luego por pagina: pagina, contenido, imagen.
    let cuantas = paginas.len();
    let kids: Vec<Valor> = (0..cuantas)
        .map(|i| Valor::Ref(3 + 3 * i as u32, 0))
        .collect();
    objeto(
        &mut s,
        &mut sitios,
        Valor::Dicc(vec![
            (b"Type".to_vec(), Valor::Nombre(b"Catalog".to_vec())),
            (b"Pages".to_vec(), Valor::Ref(2, 0)),
        ]),
    );
    objeto(
        &mut s,
        &mut sitios,
        Valor::Dicc(vec![
            (b"Type".to_vec(), Valor::Nombre(b"Pages".to_vec())),
            (b"Kids".to_vec(), Valor::Lista(kids)),
            (b"Count".to_vec(), numero(cuantas as i64)),
        ]),
    );
    for (i, p) in paginas.iter().enumerate() {
        let base = 3 + 3 * i as u32;
        let ancho = crate::escribir::A4.0;
        let alto = ancho * p.alto as f32 / p.ancho as f32;
        let medida = |v: f32| Valor::Numero(format!("{v:.2}").into_bytes());
        objeto(
            &mut s,
            &mut sitios,
            Valor::Dicc(vec![
                (b"Type".to_vec(), Valor::Nombre(b"Page".to_vec())),
                (b"Parent".to_vec(), Valor::Ref(2, 0)),
                (
                    b"MediaBox".to_vec(),
                    Valor::Lista(vec![numero(0), numero(0), medida(ancho), medida(alto)]),
                ),
                (b"Contents".to_vec(), Valor::Ref(base + 1, 0)),
                (
                    b"Resources".to_vec(),
                    Valor::Dicc(vec![(
                        b"XObject".to_vec(),
                        Valor::Dicc(vec![(b"Im0".to_vec(), Valor::Ref(base + 2, 0))]),
                    )]),
                ),
            ]),
        );
        let contenido = format!("q {ancho:.2} 0 0 {alto:.2} 0 0 cm /Im0 Do Q").into_bytes();
        objeto(&mut s, &mut sitios, Valor::Flujo(Vec::new(), contenido));
        // RGB sin alfa, comprimido: una pagina pintada es papel, no lleva
        // transparencia que conservar.
        let mut rgb = Vec::with_capacity((p.ancho * p.alto * 3) as usize);
        for px in p.pixeles.chunks_exact(4) {
            rgb.extend_from_slice(&px[..3]);
        }
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(&rgb).ok()?;
        let comprimido = z.finish().ok()?;
        objeto(
            &mut s,
            &mut sitios,
            Valor::Flujo(
                vec![
                    (b"Type".to_vec(), Valor::Nombre(b"XObject".to_vec())),
                    (b"Subtype".to_vec(), Valor::Nombre(b"Image".to_vec())),
                    (b"Width".to_vec(), numero(p.ancho as i64)),
                    (b"Height".to_vec(), numero(p.alto as i64)),
                    (b"ColorSpace".to_vec(), Valor::Nombre(b"DeviceRGB".to_vec())),
                    (b"BitsPerComponent".to_vec(), numero(8)),
                    (b"Filter".to_vec(), Valor::Nombre(b"FlateDecode".to_vec())),
                ],
                comprimido,
            ),
        );
    }
    let inicio = s.len();
    let _ = write!(s, "xref\n0 {}\n0000000000 65535 f\r\n", sitios.len() + 1);
    for off in &sitios {
        let _ = write!(s, "{off:010} 00000 n\r\n");
    }
    let _ = write!(
        s,
        "trailer\n<</Size {} /Root 1 0 R>>\nstartxref\n{inicio}\n%%EOF\n",
        sitios.len() + 1
    );
    Some(s)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_codec::imagen::ImagenRgba;

    fn imagen(ancho: u32, alto: u32, gris: u8) -> ImagenRgba {
        ImagenRgba {
            ancho,
            alto,
            pixeles: [gris, gris, gris, 255].repeat((ancho * alto) as usize),
        }
    }

    /// Un PDF de `n` paginas escrito con indice en flujo (PDF 1.5) y la
    /// pagina dentro de un flujo de objetos comprimido, que es como salen hoy
    /// los PDF de Word o de un escaner moderno.
    fn pdf_con_indice_en_flujo(n: usize) -> Vec<u8> {
        let mut s = b"%PDF-1.5\n".to_vec();
        let mut sitios: Vec<(u32, usize)> = Vec::new();
        // 1 catalogo, 2 arbol (con el MediaBox para que se herede), 3
        // contenido, 4 flujo de objetos con las paginas (5..5+n).
        let kids: String = (0..n).map(|i| format!("{} 0 R ", 5 + i)).collect();
        for (num, cuerpo) in [
            (1u32, "<</Type/Catalog/Pages 2 0 R>>".to_string()),
            (
                2,
                format!("<</Type/Pages/Kids[{kids}]/Count {n}/MediaBox[0 0 200 300]>>"),
            ),
        ] {
            sitios.push((num, s.len()));
            s.extend_from_slice(format!("{num} 0 obj\n{cuerpo}\nendobj\n").as_bytes());
        }
        let contenido = b"0 0 1 rg 10 10 50 50 re f";
        sitios.push((3, s.len()));
        s.extend_from_slice(format!("3 0 obj\n<</Length {}>>stream\n", contenido.len()).as_bytes());
        s.extend_from_slice(contenido);
        s.extend_from_slice(b"\nendstream\nendobj\n");
        let mut cabecera = String::new();
        let mut cuerpos = String::new();
        for i in 0..n {
            cabecera.push_str(&format!("{} {} ", 5 + i, cuerpos.len()));
            cuerpos.push_str("<</Type/Page/Parent 2 0 R/Contents 3 0 R>> ");
        }
        let crudo = format!("{cabecera}{cuerpos}");
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(crudo.as_bytes()).unwrap();
        let comprimido = z.finish().unwrap();
        sitios.push((4, s.len()));
        s.extend_from_slice(
            format!(
                "4 0 obj\n<</Type/ObjStm/N {n}/First {}/Filter/FlateDecode/Length {}>>stream\n",
                cabecera.len(),
                comprimido.len()
            )
            .as_bytes(),
        );
        s.extend_from_slice(&comprimido);
        s.extend_from_slice(b"\nendstream\nendobj\n");
        // El indice: 0 libre, 1-4 directos, 5.. dentro del 4, y el propio.
        let propio = 5 + n as u32;
        let inicio = s.len();
        let mut filas = vec![0u8, 0, 0, 0, 0xFF];
        for num in 1..=4u32 {
            let off = sitios.iter().find(|x| x.0 == num).unwrap().1 as u32;
            filas.push(1);
            filas.extend_from_slice(&off.to_be_bytes()[1..]);
            filas.push(0);
        }
        for i in 0..n {
            filas.extend_from_slice(&[2, 0, 0, 4, i as u8]);
        }
        filas.push(1);
        filas.extend_from_slice(&(inicio as u32).to_be_bytes()[1..]);
        filas.push(0);
        s.extend_from_slice(
            format!(
                "{propio} 0 obj\n<</Type/XRef/Size {}/W[1 3 1]/Root 1 0 R/Length {}>>stream\n",
                propio + 1,
                filas.len()
            )
            .as_bytes(),
        );
        s.extend_from_slice(&filas);
        s.extend_from_slice(
            format!("\nendstream\nendobj\nstartxref\n{inicio}\n%%EOF\n").as_bytes(),
        );
        s
    }

    #[test]
    fn dos_pdf_escritos_aqui_se_unen_con_las_paginas_del_segundo_detras() {
        let a = de_imagenes(&[imagen(40, 60, 200)]).unwrap();
        let b = de_imagenes(&[imagen(40, 60, 10), imagen(60, 40, 90)]).unwrap();
        assert_eq!(contar_paginas(&a), Some(1));
        let unido = anadir_paginas(&a, &b).expect("se tienen que poder unir");
        assert_eq!(contar_paginas(&unido), Some(3));
        // Incremental: el original sigue entero al principio.
        assert!(unido.starts_with(&a));
    }

    /// `PdfUnion.soloPaginas`: la tercera y la primera, en ese orden, como
    /// paginas de verdad (la tercera es la apaisada).
    #[test]
    fn solo_esas_paginas_salen_en_su_orden_y_como_paginas_de_verdad() {
        let b = de_imagenes(&[imagen(40, 60, 10), imagen(40, 60, 50), imagen(60, 40, 90)]).unwrap();
        let dos = solo_paginas(&b, &[2, 0]).expect("se sacan");
        assert_eq!(contar_paginas(&dos), Some(2));
        let a = Archivo::leer(&dos).unwrap();
        let primera = a.dicc_de(Some(&Valor::Ref(a.paginas()[0], 0))).unwrap();
        let Some(Valor::Lista(caja)) = a.resolver(en(&primera, b"MediaBox")) else {
            panic!("la pagina lleva su caja escrita");
        };
        let ancho = numero_de_prueba(&caja[2]);
        let alto = numero_de_prueba(&caja[3]);
        assert!(ancho > alto, "la primera es la apaisada: {ancho}x{alto}");
        // Y se puede seguir pegando detras, como hace el PDF de varios proyectos.
        let mas = anadir_paginas(&dos, &solo_paginas(&b, &[1]).unwrap()).unwrap();
        assert_eq!(contar_paginas(&mas), Some(3));
    }

    #[test]
    fn sin_paginas_pedidas_o_fuera_de_rango_no_sale_nada() {
        let b = de_imagenes(&[imagen(40, 60, 10)]).unwrap();
        assert!(solo_paginas(&b, &[]).is_none());
        assert!(solo_paginas(&b, &[5]).is_none());
        assert!(solo_paginas(b"no es un pdf", &[0]).is_none());
        // Y el en blanco no se acepta como primero de una union normal.
        assert!(anadir_paginas(&en_blanco(), &b).is_none());
        assert_eq!(
            contar_paginas(&en_blanco()),
            None,
            "sin paginas no cuenta como documento"
        );
    }

    fn numero_de_prueba(v: &Valor) -> f64 {
        match v {
            Valor::Numero(t) => std::str::from_utf8(t).unwrap().parse().unwrap(),
            _ => panic!("no es un numero"),
        }
    }

    #[test]
    fn un_pdf_con_indice_en_flujo_y_objetos_comprimidos_se_lee_y_se_une() {
        let moderno = pdf_con_indice_en_flujo(2);
        assert_eq!(contar_paginas(&moderno), Some(2));
        let viejo = de_imagenes(&[imagen(10, 10, 0)]).unwrap();
        // Como segundo: sus paginas estan dentro de un flujo de objetos.
        let unido = anadir_paginas(&viejo, &moderno).unwrap();
        assert_eq!(contar_paginas(&unido), Some(3));
        // Como primero: el anadido lleva su indice tambien en flujo.
        let unido = anadir_paginas(&moderno, &viejo).unwrap();
        assert_eq!(contar_paginas(&unido), Some(3));
        let a = Archivo::leer(&unido).unwrap();
        assert!(
            a.indice_en_flujo,
            "el anadido sigue la clase de indice del original"
        );
    }

    #[test]
    fn lo_que_una_pagina_heredaba_de_su_arbol_viaja_escrito_en_ella() {
        // Las paginas del PDF moderno no llevan MediaBox: lo heredan del
        // arbol, que no se copia. Sin escribirselo, saldrian sin tamano.
        let moderno = pdf_con_indice_en_flujo(1);
        let viejo = de_imagenes(&[imagen(10, 10, 0)]).unwrap();
        let unido = anadir_paginas(&viejo, &moderno).unwrap();
        let a = Archivo::leer(&unido).unwrap();
        let ultima = *a.paginas().last().unwrap();
        let d = a.dicc_de(Some(&Valor::Ref(ultima, 0))).unwrap();
        assert!(en(&d, b"MediaBox").is_some(), "le falta el tamano heredado");
    }

    #[test]
    fn un_pdf_cifrado_no_se_une_y_lo_dice() {
        // Caso negativo: copiar objetos cifrados dejaria cadenas y flujos
        // ilegibles dentro de otro documento. Se devuelve None y quien llama
        // pega las paginas pintadas.
        let a = de_imagenes(&[imagen(10, 10, 0)]).unwrap();
        let texto = String::from_utf8_lossy(&a).replace(
            "/Root 1 0 R>>",
            "/Root 1 0 R /Encrypt << /Filter /Standard >> >>",
        );
        let cifrado = texto.into_bytes();
        assert!(esta_cifrado(&cifrado));
        assert!(!esta_cifrado(&a));
        assert!(anadir_paginas(&a, &cifrado).is_none());
        assert!(anadir_paginas(&cifrado, &a).is_none());
    }

    #[test]
    fn lo_que_no_es_un_pdf_no_se_une() {
        assert!(anadir_paginas(b"hola", b"adios").is_none());
        assert_eq!(contar_paginas(b"no soy un pdf"), None);
    }

    /// Con PDF de verdad, que es donde se rompen los analizadores: se pasan
    /// en `PIXPIN_PDF_REALES` separados por `;` y se unen de dos en dos.
    #[test]
    #[ignore = "necesita PDF reales: PIXPIN_PDF_REALES=a.pdf;b.pdf"]
    fn pdf_reales_se_unen_y_windows_dibuja_la_ultima_pagina() {
        let Ok(lista) = std::env::var("PIXPIN_PDF_REALES") else {
            return;
        };
        let rutas: Vec<_> = lista.split(';').filter(|s| !s.is_empty()).collect();
        for par in rutas.windows(2) {
            let a = std::fs::read(par[0]).unwrap();
            let b = std::fs::read(par[1]).unwrap();
            let (na, nb) = (contar_paginas(&a), contar_paginas(&b));
            eprintln!("{} ({na:?}) + {} ({nb:?})", par[0], par[1]);
            let Some(unido) = anadir_paginas(&a, &b) else {
                eprintln!(
                    "  no se unen (cifrado: {} / {})",
                    esta_cifrado(&a),
                    esta_cifrado(&b)
                );
                continue;
            };
            let ruta =
                std::env::temp_dir().join(format!("pixpin-union-real-{}.pdf", std::process::id()));
            std::fs::write(&ruta, &unido).unwrap();
            let d = crate::Documento::abrir(&ruta).expect("Windows tiene que abrirlo");
            assert_eq!(Some(d.paginas()), na.zip(nb).map(|(x, y)| x + y));
            d.renderizar(d.paginas() - 1, 200)
                .expect("la ultima pagina pegada se dibuja");
            drop(d);
            let _ = std::fs::remove_file(&ruta);
        }
    }

    #[test]
    fn un_pdf_unido_se_deja_dibujar_por_windows() {
        let a = de_imagenes(&[imagen(40, 60, 200)]).unwrap();
        let b = pdf_con_indice_en_flujo(1);
        let unido = anadir_paginas(&a, &b).unwrap();
        let ruta = std::env::temp_dir().join(format!("pixpin-union-{}.pdf", std::process::id()));
        std::fs::write(&ruta, &unido).unwrap();
        let d = crate::Documento::abrir(&ruta).expect("Windows tiene que abrirlo");
        assert_eq!(d.paginas(), 2);
        let img = d.renderizar(1, 100).expect("la pagina pegada se dibuja");
        assert_eq!(img.ancho, 100);
        drop(d);
        let _ = std::fs::remove_file(&ruta);
    }
}
