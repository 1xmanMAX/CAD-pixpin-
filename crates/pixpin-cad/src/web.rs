//! **El plano como pagina web** (13-oct-2026, el usuario: «al visor de
//! AutoCAD implementale un exportador en formato HTML en el que se considere
//! todo a la hora de exportar»).
//!
//! Un solo `.html` que se abre sin internet y ensena el plano como el visor:
//! las rayas, los rellenos, los sombreados con su patron, los circulos, las
//! letras (las SHX como trazos y las TrueType rellenas) y las imagenes, cada
//! cosa con su color y su grosor, **por capas** que se encienden y apagan en
//! la pagina; encima, las cotas y los marcos del visor y lo anotado con el
//! motor del lienzo (un SVG del mismo motor que exporta el lienzo). Con la
//! rueda se acerca alrededor del raton, arrastrando se mueve, «Ver todo»
//! encuadra y el fondo va claro u oscuro con los colores del visor.
//!
//! **Ligero aunque el plano sea grande**: va lo que ya tiene el modelo del
//! visor ([`Modelo`]), no el DWG: cada cosa en sus tramos ordenados por
//! tamano y sitio, y la pagina hace lo mismo que la tarjeta grafica: lo que
//! en pantalla mediria menos de un pixel no se dibuja y de lo demas solo lo
//! que cae en la ventana. Cada tramo se convierte en caminos del lienzo
//! (`Path2D`) la primera vez que se ve y se guardan: mover o acercar es
//! volver a pintarlos con otra matriz. Los numeros van en binario,
//! comprimidos con DEFLATE (el navegador los descomprime solo,
//! `DecompressionStream`) y en base64 dentro de la pagina.
//!
//! El plano por capas lo saca [`crate::convertir::Convertidor::por_capas`]
//! (en el proceso aparte, como al abrir); aqui se guarda ([`a_bytes`]) y se
//! hace la pagina ([`pagina`]).

use std::collections::HashMap;
use std::io::Write;

use crate::convertir::{GROSOR_NORMAL, Grupo, ImagenDelPlano, PlanoPorCapas};
use crate::modelo::{CORTE, GLIFO_DE_RAYAS, Modelo, Tramo};

const MAGIA: &[u8; 8] = b"PXCAPAS1";

/// **El plano por capas en bytes** (lo que deja el proceso aparte).
pub fn a_bytes(p: &PlanoPorCapas) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(MAGIA);
    let texto = |b: &mut Vec<u8>, s: &str| {
        b.extend_from_slice(&(s.len() as u32).to_le_bytes());
        b.extend_from_slice(s.as_bytes());
    };
    b.extend_from_slice(&(p.grupos.len() as u32).to_le_bytes());
    for g in &p.grupos {
        texto(&mut b, &g.capa);
        b.extend_from_slice(&g.grosor.to_le_bytes());
        b.push(g.encendida as u8);
        let m = g.modelo.a_bytes();
        b.extend_from_slice(&(m.len() as u32).to_le_bytes());
        b.extend_from_slice(&m);
    }
    b.extend_from_slice(&(p.imagenes.len() as u32).to_le_bytes());
    for i in &p.imagenes {
        texto(&mut b, &i.capa);
        texto(&mut b, &i.ruta);
        for v in [i.esquina, i.u, i.v].iter().flatten() {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    b
}

/// Lo contrario de [`a_bytes`]; `None` si no se entiende (roto, cortado).
pub fn de_bytes(b: &[u8]) -> Option<PlanoPorCapas> {
    struct L<'a> {
        b: &'a [u8],
        i: usize,
    }
    impl L<'_> {
        fn tomar(&mut self, n: usize) -> Option<&[u8]> {
            let t = self.b.get(self.i..self.i.checked_add(n)?)?;
            self.i += n;
            Some(t)
        }
        fn u32(&mut self) -> Option<u32> {
            Some(u32::from_le_bytes(self.tomar(4)?.try_into().ok()?))
        }
        fn f64(&mut self) -> Option<f64> {
            Some(f64::from_le_bytes(self.tomar(8)?.try_into().ok()?))
        }
        fn texto(&mut self) -> Option<String> {
            let n = self.u32()? as usize;
            Some(String::from_utf8_lossy(self.tomar(n)?).into_owned())
        }
    }
    if b.len() < 8 || &b[..8] != MAGIA {
        return None;
    }
    let mut l = L { b, i: 8 };
    let n = l.u32()? as usize;
    let mut grupos = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        let capa = l.texto()?;
        let grosor = i16::from_le_bytes(l.tomar(2)?.try_into().ok()?);
        let encendida = l.tomar(1)?[0] != 0;
        let largo = l.u32()? as usize;
        let modelo = Modelo::de_bytes(l.tomar(largo)?)?;
        grupos.push(Grupo {
            capa,
            grosor,
            encendida,
            modelo,
        });
    }
    let n = l.u32()? as usize;
    let mut imagenes = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        let capa = l.texto()?;
        let ruta = l.texto()?;
        let mut v = [0f64; 6];
        for x in &mut v {
            *x = l.f64()?;
        }
        imagenes.push(ImagenDelPlano {
            capa,
            ruta,
            esquina: [v[0], v[1]],
            u: [v[2], v[3]],
            v: [v[4], v[5]],
        });
    }
    (l.i == b.len()).then_some(PlanoPorCapas { grupos, imagenes })
}

/// Lo anotado con el motor del lienzo, ya como SVG: el SVG mide `caja` en
/// unidades de la capa (x a la derecha, y hacia abajo) y un punto `(x, y)`
/// de la capa cae en el plano en `(cero.x + x·u, cero.y − y·u)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Anotado {
    pub svg: String,
    pub caja: [f64; 4],
    pub u: f64,
    pub cero: [f64; 2],
}

/// Una imagen del plano ya leida (sus bytes tal cual: PNG o JPEG).
#[derive(Debug, Clone, PartialEq)]
pub struct ImagenWeb {
    pub capa: String,
    pub mime: &'static str,
    pub bytes: Vec<u8>,
    pub esquina: [f64; 2],
    pub u: [f64; 2],
    pub v: [f64; 2],
}

/// Los rotulos de la pagina, en el idioma de la app.
#[derive(Debug, Clone, PartialEq)]
pub struct TextosWeb {
    pub todo: String,
    pub tema: String,
    pub capas: String,
    pub grosores: String,
    pub anotaciones: String,
    pub todas: String,
    pub ninguna: String,
    pub sin_soporte: String,
    pub pista: String,
}

impl Default for TextosWeb {
    fn default() -> Self {
        TextosWeb {
            todo: "Ver todo".into(),
            tema: "Claro / oscuro".into(),
            capas: "Capas".into(),
            grosores: "Grosores".into(),
            anotaciones: "Anotaciones".into(),
            todas: "Todas".into(),
            ninguna: "Ninguna".into(),
            sin_soporte: "Este navegador no sabe descomprimir el plano: ábrelo con Edge, Chrome, Firefox o Safari recientes.".into(),
            pista: "Rueda: acercar · Arrastrar: mover · Doble clic: ver todo · F, B, L".into(),
        }
    }
}

/// Lo que lleva la pagina ademas de las capas. Todo en coordenadas del
/// plano **respecto a `origen`** (el del modelo del visor).
#[derive(Debug, Clone, Default)]
pub struct OpcionesWeb {
    pub titulo: String,
    pub origen: [f64; 2],
    /// Lo que se ve al abrir (`[x0, y0, x1, y1]`).
    pub vista: [f64; 4],
    pub unidades: u32,
    pub cotas: Vec<Vec<[f64; 2]>>,
    pub marcos: Vec<[f64; 4]>,
    pub anotado: Option<Anotado>,
    pub imagenes: Vec<ImagenWeb>,
    pub claro: bool,
    pub textos: Option<TextosWeb>,
}

/// Las palabras de 4 bytes que van a la pagina (lo que lee el `ArrayBuffer`).
#[derive(Default)]
struct Datos {
    w: Vec<u32>,
    colores: Vec<u32>,
    color_de: HashMap<u32, u32>,
}

impl Datos {
    fn f(&mut self, x: f32) {
        self.w.push(x.to_bits());
    }
    fn u(&mut self, x: u32) {
        self.w.push(x);
    }
    fn color(&mut self, c: u32) -> u32 {
        if let Some(i) = self.color_de.get(&c) {
            return *i;
        }
        let i = self.colores.len() as u32;
        self.colores.push(c);
        self.color_de.insert(c, i);
        i
    }
    fn sitio(&self) -> u32 {
        self.w.len() as u32
    }
    /// La ficha de un tramo: su caja, su tamano y donde empieza lo suyo.
    fn tramo(&mut self, t: &Tramo, desde: u32, cuantos: u32) {
        for v in t.caja {
            self.f(v);
        }
        self.f(t.tamano);
        self.u(desde);
        self.u(cuantos);
    }
}

/// Un apartado de un grupo: donde estan sus fichas de tramo y cuantas son.
type Apartado = (u32, u32);

/// Escribe los tramos de un tipo: primero el contenido de cada tramo y al
/// final sus fichas (7 palabras cada una).
fn apartado(
    d: &mut Datos,
    tramos: &[Tramo],
    mut contenido: impl FnMut(&mut Datos, &Tramo) -> u32,
) -> Apartado {
    let mut fichas = Vec::with_capacity(tramos.len());
    for t in tramos {
        let desde = d.sitio();
        let n = contenido(d, t);
        if n > 0 {
            fichas.push((*t, desde, n));
        }
    }
    let sitio = d.sitio();
    for (t, desde, n) in &fichas {
        d.tramo(t, *desde, *n);
    }
    (sitio, fichas.len() as u32)
}

fn rango(desde: u32, cuantos: u32, largo: usize) -> std::ops::Range<usize> {
    let a = (desde as usize).min(largo);
    a..(desde as usize).saturating_add(cuantos as usize).min(largo)
}

/// Un grupo (capa y grosor) en palabras; devuelve su ficha JSON.
fn grupo(d: &mut Datos, g: &Grupo, origen: [f64; 2], indice_capa: usize) -> String {
    let m = &g.modelo;
    let o = [
        (m.origen[0] - origen[0]) as f32,
        (m.origen[1] - origen[1]) as f32,
    ];
    // Rayas: cada tira partida donde cambia el color. [color, n, x, y...]
    let rayas = apartado(d, &m.tramos_lineas, |d, t| {
        let mut tiras = 0;
        let mut tira: Vec<(f32, f32)> = Vec::new();
        let mut color = 0u32;
        let mut cerrar = |d: &mut Datos, tira: &mut Vec<(f32, f32)>, color: u32| {
            if tira.len() >= 2 {
                let c = d.color(color);
                d.u(c);
                d.u(tira.len() as u32);
                for (x, y) in tira.iter() {
                    d.f(*x);
                    d.f(*y);
                }
                tiras += 1;
            }
            tira.clear();
        };
        for &i in &m.lineas[rango(t.desde, t.cuantos, m.lineas.len())] {
            if i == CORTE {
                cerrar(d, &mut tira, color);
                continue;
            }
            let Some(q) = m.vertices.get(i as usize) else {
                continue;
            };
            if q.color != color && !tira.is_empty() {
                let ultimo = *tira.last().unwrap_or(&(q.x, q.y));
                cerrar(d, &mut tira, color);
                tira.push(ultimo);
            }
            color = q.color;
            tira.push((q.x, q.y));
        }
        cerrar(d, &mut tira, color);
        tiras
    });
    // Rellenos: los triangulos de cada color juntos. [color, n, 6n]
    let rellenos = apartado(d, &m.tramos_triangulos, |d, t| {
        let mut por_color: Vec<(u32, Vec<f32>)> = Vec::new();
        for tri in m.triangulos[rango(t.desde, t.cuantos, m.triangulos.len())].chunks_exact(3) {
            let v: Vec<_> = tri
                .iter()
                .filter_map(|&i| m.vertices.get(i as usize))
                .collect();
            if v.len() != 3 {
                continue;
            }
            let c = v[0].color;
            let sitio = match por_color.iter().position(|(k, _)| *k == c) {
                Some(s) => s,
                None => {
                    por_color.push((c, Vec::new()));
                    por_color.len() - 1
                }
            };
            por_color[sitio].1.extend(v.iter().flat_map(|q| [q.x, q.y]));
        }
        for (c, v) in &por_color {
            let c = d.color(*c);
            d.u(c);
            d.u((v.len() / 6) as u32);
            for x in v {
                d.f(*x);
            }
        }
        por_color.len() as u32
    });
    // Los patrones de los sombreados: [centro2, inv4, escala, desde, n] y
    // sus familias de 16 palabras.
    let familias_en = d.sitio();
    for f in &m.familias {
        for v in [
            f.base[0],
            f.base[1],
            f.dir[0],
            f.dir[1],
            f.paso,
            f.corrimiento,
            f.largo,
        ] {
            d.f(v);
        }
        d.u(f.n);
        for v in f.trazos {
            d.f(v);
        }
    }
    let tramas_en = d.sitio();
    for t in &m.tramas {
        for v in [
            t.centro[0],
            t.centro[1],
            t.inversa[0],
            t.inversa[1],
            t.inversa[2],
            t.inversa[3],
            t.escala,
        ] {
            d.f(v);
        }
        d.u(familias_en + t.desde * 16);
        d.u(t.cuantas);
    }
    // Sombreados: [color, trama, n, 6n]
    let sombreados = apartado(d, &m.tramos_trama, |d, t| {
        let mut por: Vec<((u32, u32), Vec<f32>)> = Vec::new();
        for tri in
            m.triangulos_trama[rango(t.desde, t.cuantos, m.triangulos_trama.len())].chunks_exact(3)
        {
            let v: Vec<_> = tri
                .iter()
                .filter_map(|&i| m.vertices_trama.get(i as usize))
                .collect();
            if v.len() != 3 || v[0].trama as usize >= m.tramas.len() {
                continue;
            }
            let k = (v[0].color, v[0].trama);
            let sitio = match por.iter().position(|(x, _)| *x == k) {
                Some(s) => s,
                None => {
                    por.push((k, Vec::new()));
                    por.len() - 1
                }
            };
            por[sitio].1.extend(v.iter().flat_map(|q| [q.x, q.y]));
        }
        for ((c, tr), v) in &por {
            let c = d.color(*c);
            d.u(c);
            d.u(tramas_en + tr * 9);
            d.u((v.len() / 6) as u32);
            for x in v {
                d.f(*x);
            }
        }
        por.len() as u32
    });
    // Circulos y arcos: [x, y, r, inicio, barrido, color]
    let arcos = apartado(d, &m.tramos_arcos, |d, t| {
        let r = rango(t.desde, t.cuantos, m.arcos.len());
        let n = r.len() as u32;
        for a in &m.arcos[r] {
            for v in [a.centro[0], a.centro[1], a.radio, a.inicio, a.barrido] {
                d.f(v);
            }
            let c = d.color(a.color);
            d.u(c);
        }
        n
    });
    // Las letras distintas: [desde, n, rayas] con su malla en em.
    let malla_en = d.sitio();
    for p in &m.malla_letras {
        d.f(p[0]);
        d.f(p[1]);
    }
    let glifos_en = d.sitio();
    for g in &m.glifos {
        d.u(malla_en + g[0] * 2);
        d.u(g[1] & !GLIFO_DE_RAYAS);
        d.u((g[1] & GLIFO_DE_RAYAS != 0) as u32);
    }
    // Letras: [x, y, m0, m1, m2, m3, color, glifo]
    let letras = apartado(d, &m.tramos_letras, |d, t| {
        let r = rango(t.desde, t.cuantos, m.letras.len());
        let mut n = 0;
        for l in &m.letras[r] {
            if l.glifo as usize >= m.glifos.len() {
                continue;
            }
            for v in [l.pos[0], l.pos[1], l.m[0], l.m[1], l.m[2], l.m[3]] {
                d.f(v);
            }
            let c = d.color(l.color);
            d.u(c);
            d.u(glifos_en + l.glifo * 3);
            n += 1;
        }
        n
    });
    let par = |a: Apartado| format!("[{},{}]", a.0, a.1);
    format!(
        r#"{{"k":{},"g":{},"on":{},"o":[{},{}],"L":{},"T":{},"H":{},"A":{},"G":{}}}"#,
        indice_capa,
        g.grosor,
        g.encendida,
        num(o[0] as f64),
        num(o[1] as f64),
        par(rayas),
        par(rellenos),
        par(sombreados),
        par(arcos),
        par(letras)
    )
}

/// El primer color de lo dibujado en un modelo.
fn color_de(m: &Modelo) -> Option<u32> {
    m.vertices
        .first()
        .map(|v| v.color)
        .or_else(|| m.letras.first().map(|l| l.color))
        .or_else(|| m.arcos.first().map(|a| a.color))
        .or_else(|| m.vertices_trama.first().map(|v| v.color))
}

/// Un numero para el JSON (nunca `NaN` ni infinito).
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// Un texto como cadena JSON (sirve tambien dentro de `<script>`: se
/// escapa `<` para que un nombre de capa no cierre el guion).
pub fn texto_json(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '<' => o.push_str("\\u003c"),
            '>' => o.push_str("\\u003e"),
            '&' => o.push_str("\\u0026"),
            '\u{2028}' => o.push_str("\\u2028"),
            '\u{2029}' => o.push_str("\\u2029"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Base64 de siempre (RFC 4648).
pub fn base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        o.push(A[(n >> 18) as usize & 63] as char);
        o.push(A[(n >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 {
            A[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        o.push(if c.len() > 2 {
            A[n as usize & 63] as char
        } else {
            '='
        });
    }
    o
}

/// El JavaScript de la pagina: el visor en un lienzo 2D.
const GUION: &str = include_str!("web.js");

/// **La pagina web del plano.** `grupos` es el plano por capas (o el del
/// visor entero como un solo grupo, si no se pudo leer por capas).
pub fn pagina(grupos: &[Grupo], o: &OpcionesWeb) -> String {
    let t = o.textos.clone().unwrap_or_default();
    let mut d = Datos::default();
    // Las capas, por su nombre, en el orden de los grupos.
    let mut capas: Vec<(String, bool)> = Vec::new();
    // El color de cada capa para su muestra en el panel: el primero que
    // sale en lo suyo.
    let mut muestras: Vec<Option<u32>> = Vec::new();
    let mut fichas = Vec::new();
    for g in grupos {
        let k = match capas.iter().position(|(c, _)| *c == g.capa) {
            Some(k) => k,
            None => {
                capas.push((g.capa.clone(), g.encendida));
                muestras.push(None);
                capas.len() - 1
            }
        };
        if muestras[k].is_none() {
            muestras[k] = color_de(&g.modelo);
        }
        fichas.push(grupo(&mut d, g, o.origen, k));
    }
    for i in &o.imagenes {
        if !capas.iter().any(|(c, _)| *c == i.capa) {
            capas.push((i.capa.clone(), true));
            muestras.push(None);
        }
    }
    let mut bytes = Vec::with_capacity(d.w.len() * 4);
    for w in &d.w {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    let comprimido = {
        let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(7));
        let _ = z.write_all(&bytes);
        z.finish().unwrap_or_default()
    };
    let capas_json: Vec<String> = capas
        .iter()
        .zip(&muestras)
        .map(|((c, on), m)| match m {
            Some(color) => format!(r#"{{"n":{},"on":{},"c":{}}}"#, texto_json(c), on, color),
            None => format!(r#"{{"n":{},"on":{}}}"#, texto_json(c), on),
        })
        .collect();
    let colores: Vec<String> = d.colores.iter().map(|c| c.to_string()).collect();
    let cotas: Vec<String> = o
        .cotas
        .iter()
        .filter(|c| c.len() >= 2)
        .map(|c| {
            format!(
                "[{}]",
                c.iter()
                    .map(|p| format!("{},{}", num(p[0]), num(p[1])))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect();
    let marcos: Vec<String> = o
        .marcos
        .iter()
        .map(|m| format!("[{},{},{},{}]", num(m[0]), num(m[1]), num(m[2]), num(m[3])))
        .collect();
    let imagenes: Vec<String> = o
        .imagenes
        .iter()
        .map(|i| {
            let k = capas.iter().position(|(c, _)| *c == i.capa).unwrap_or(0);
            format!(
                r#"{{"k":{},"src":"data:{};base64,{}","e":[{},{}],"u":[{},{}],"v":[{},{}]}}"#,
                k,
                i.mime,
                base64(&i.bytes),
                num(i.esquina[0]),
                num(i.esquina[1]),
                num(i.u[0]),
                num(i.u[1]),
                num(i.v[0]),
                num(i.v[1])
            )
        })
        .collect();
    let anotado = match &o.anotado {
        Some(a) => format!(
            r#"{{"caja":[{},{},{},{}],"u":{},"cero":[{},{}]}}"#,
            num(a.caja[0]),
            num(a.caja[1]),
            num(a.caja[2]),
            num(a.caja[3]),
            num(a.u),
            num(a.cero[0]),
            num(a.cero[1])
        ),
        None => "null".into(),
    };
    let sufijo = crate::ventana::sufijo(o.unidades).trim();
    let datos = format!(
        r#"{{"titulo":{},"vista":[{},{},{},{}],"claro":{},"unidad":{},"capas":[{}],"grupos":[{}],"colores":[{}],"cotas":[{}],"marcos":[{}],"imagenes":[{}],"anotado":{},"normal":{},"t":{{"todas":{},"ninguna":{},"sin":{}}}}}"#,
        texto_json(&o.titulo),
        num(o.vista[0]),
        num(o.vista[1]),
        num(o.vista[2]),
        num(o.vista[3]),
        o.claro,
        texto_json(sufijo),
        capas_json.join(","),
        fichas.join(","),
        colores.join(","),
        cotas.join(","),
        marcos.join(","),
        imagenes.join(","),
        anotado,
        GROSOR_NORMAL,
        texto_json(&t.todas),
        texto_json(&t.ninguna),
        texto_json(&t.sin_soporte),
    );
    let svg = o.anotado.as_ref().map(|a| a.svg.as_str()).unwrap_or("");
    let titulo = html(&o.titulo);
    let mut s = String::with_capacity(
        comprimido.len() * 4 / 3 + datos.len() + svg.len() + GUION.len() + 4096,
    );
    s.push_str("<!DOCTYPE html>\n<html lang=\"es\"><head><meta charset=\"utf-8\">\n");
    s.push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n");
    s.push_str("<meta name=\"generator\" content=\"PixPin Max · plano\">\n");
    s.push_str(&format!("<title>{titulo}</title>\n"));
    s.push_str(ESTILO);
    s.push_str("</head><body class=\"");
    s.push_str(if o.claro { "claro" } else { "oscuro" });
    s.push_str("\">\n<div id=\"barra\">");
    s.push_str(&format!(
        "<span id=\"nombre\" title=\"{titulo}\">{titulo}</span>"
    ));
    for (id, rotulo, tecla) in [
        ("b-todo", &t.todo, "F"),
        ("b-tema", &t.tema, "B"),
        ("b-capas", &t.capas, "L"),
        ("b-grosor", &t.grosores, "W"),
        ("b-anot", &t.anotaciones, "A"),
    ] {
        if id == "b-anot" && o.anotado.is_none() && o.cotas.is_empty() && o.marcos.is_empty() {
            continue;
        }
        s.push_str(&format!(
            "<button id=\"{id}\" title=\"{} ({tecla})\">{}</button>",
            html(rotulo),
            html(rotulo)
        ));
    }
    s.push_str(&format!(
        "<span id=\"pista\">{}</span></div>\n",
        html(&t.pista)
    ));
    s.push_str("<div id=\"lienzo\"><canvas id=\"plano\"></canvas><div id=\"anot\">");
    s.push_str(svg);
    s.push_str("</div><canvas id=\"encima\"></canvas></div>\n<div id=\"panel\" hidden></div>\n<div id=\"aviso\" hidden></div>\n");
    s.push_str("<script id=\"datos\" type=\"application/json\">");
    s.push_str(&datos);
    s.push_str("</script>\n<script id=\"plano-gz\" type=\"application/octet-stream\">");
    s.push_str(&base64(&comprimido));
    s.push_str("</script>\n<script>\n");
    s.push_str(GUION);
    s.push_str("\n</script>\n</body></html>\n");
    s
}

const ESTILO: &str = r#"<style>
:root{--fondo:#21262f;--barra:#1b1d22;--texto:#e8e8ec;--suave:#9a9aa2;--boton:#2a2b31;--sobre:#34353c;--azul:#1e88e5}
body.claro{--fondo:#fcfcf9;--barra:#f1f1f3;--texto:#1b1b1f;--suave:#5f6068;--boton:#e3e3e8;--sobre:#d6d6dc}
*{box-sizing:border-box}
html,body{margin:0;height:100%;overflow:hidden;background:var(--fondo);color:var(--texto);font:14px "Segoe UI",system-ui,sans-serif}
#barra{position:fixed;left:0;right:0;top:0;height:44px;display:flex;align-items:center;gap:6px;padding:0 10px;background:var(--barra);z-index:3;border-bottom:1px solid rgba(127,127,127,.25)}
#nombre{font-weight:600;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:40vw;margin-right:8px}
#barra button{background:var(--boton);color:var(--texto);border:0;border-radius:8px;padding:6px 11px;font:inherit;cursor:pointer}
#barra button:hover{background:var(--sobre)}
#barra button.on{background:var(--azul);color:#fff}
#pista{margin-left:auto;color:var(--suave);font-size:12px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
#lienzo{position:fixed;left:0;right:0;top:44px;bottom:0;overflow:hidden;touch-action:none;cursor:grab}
#lienzo.moviendo{cursor:grabbing}
#lienzo canvas{position:absolute;left:0;top:0;width:100%;height:100%}
#anot{position:absolute;left:0;top:0;transform-origin:0 0;pointer-events:none}
#anot svg{display:block;overflow:visible}
#panel{position:fixed;right:10px;top:52px;bottom:10px;width:min(320px,calc(100vw - 20px));overflow:auto;background:var(--barra);border-radius:12px;padding:10px;z-index:4;box-shadow:0 6px 24px rgba(0,0,0,.35)}
#panel label{display:flex;align-items:center;gap:8px;padding:4px 2px;cursor:pointer;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
#panel .muestra{width:12px;height:12px;border-radius:3px;flex:none;border:1px solid rgba(127,127,127,.5)}
#panel .fila{display:flex;gap:6px;margin-bottom:8px}
#panel .fila button{flex:1;background:var(--boton);color:var(--texto);border:0;border-radius:8px;padding:6px;font:inherit;cursor:pointer}
#aviso{position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);background:var(--barra);padding:14px 18px;border-radius:12px;max-width:80vw;z-index:5}
@media print{#barra,#panel{display:none}#lienzo{top:0}}
</style>
"#;

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::modelo::Constructor;

    fn grupo_de_prueba(capa: &str, grosor: i16, encendida: bool) -> Grupo {
        let mut c = Constructor::nuevo();
        c.polilinea(
            &[[0.0, 0.0], [10.0, 0.0], [10.0, 5.0]],
            crate::convertir::rgba(255, 0, 0),
            None,
        );
        c.triangulos(
            &[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            crate::convertir::COLOR_7,
            None,
        );
        c.arco(
            [5.0, 5.0],
            2.0,
            0.0,
            std::f64::consts::TAU,
            crate::convertir::rgba(0, 0, 255),
        );
        Grupo {
            capa: capa.into(),
            grosor,
            encendida,
            modelo: c.terminar(),
        }
    }

    #[test]
    fn el_plano_por_capas_vuelve_igual_y_lo_roto_no_se_entiende() {
        let p = PlanoPorCapas {
            grupos: vec![
                grupo_de_prueba("Muros", 50, true),
                grupo_de_prueba("Ejes <x>", GROSOR_NORMAL, false),
            ],
            imagenes: vec![ImagenDelPlano {
                capa: "Fotos".into(),
                ruta: "foto.png".into(),
                esquina: [1.0, 2.0],
                u: [3.0, 0.0],
                v: [0.0, 4.0],
            }],
        };
        let b = a_bytes(&p);
        assert_eq!(de_bytes(&b), Some(p));
        // Casos negativos: cortado, sobrante o de otra cosa.
        assert_eq!(de_bytes(&b[..b.len() - 3]), None);
        let mut largo = b.clone();
        largo.push(0);
        assert_eq!(de_bytes(&largo), None);
        assert_eq!(de_bytes(b"PXCAD\0\0\x07"), None);
        assert_eq!(de_bytes(&[]), None);
    }

    #[test]
    fn la_pagina_lleva_las_capas_los_datos_y_lo_anotado_sin_internet() {
        let grupos = vec![
            grupo_de_prueba("Muros", 50, true),
            grupo_de_prueba("Ejes </script>", GROSOR_NORMAL, false),
        ];
        let o = OpcionesWeb {
            titulo: "Planta <baja>".into(),
            vista: [0.0, 0.0, 10.0, 10.0],
            unidades: 6,
            cotas: vec![vec![[0.0, 0.0], [10.0, 0.0]]],
            marcos: vec![[0.0, 0.0, 10.0, 5.0]],
            anotado: Some(Anotado {
                svg: "<svg id=\"tinta\"></svg>".into(),
                caja: [0.0, 0.0, 5.0, 5.0],
                u: 0.5,
                cero: [0.0, 10.0],
            }),
            imagenes: vec![ImagenWeb {
                capa: "Fotos".into(),
                mime: "image/png",
                bytes: vec![1, 2, 3],
                esquina: [0.0; 2],
                u: [1.0, 0.0],
                v: [0.0, 1.0],
            }],
            claro: true,
            ..Default::default()
        };
        let h = pagina(&grupos, &o);
        assert!(h.starts_with("<!DOCTYPE html>"));
        assert!(h.contains("<title>Planta &lt;baja&gt;</title>"));
        // Las dos capas y la de la imagen, con su estado.
        assert!(h.contains(r#"{"n":"Muros","on":true,"c":"#));
        assert!(h.contains(r#"{"n":"Ejes \u003c/script\u003e","on":false,"c":"#));
        assert!(h.contains(r#"{"n":"Fotos","on":true}"#));
        assert!(h.contains("data:image/png;base64,AQID"));
        assert!(h.contains("<svg id=\"tinta\"></svg>"));
        assert!(h.contains(r#""unidad":"m""#));
        // Nada de fuera: ni guiones ni hojas de estilo ni letras de internet.
        assert!(
            !h.contains("src=\"http")
                && !h.contains("href=\"http")
                && !h.contains("url(http")
                && !h.contains("src=\"/")
        );
        // Un solo cierre de guion por guion: el nombre de la capa no lo cierra.
        assert_eq!(h.matches("</script>").count(), 3);
        // Los datos son base64 de un gzip.
        let ini = h.find("type=\"application/octet-stream\">").unwrap() + 32;
        let fin = ini + h[ini..].find("</script>").unwrap();
        assert!(h[ini..fin].starts_with("H4sI"), "{}", &h[ini..ini + 8]);
    }

    #[test]
    fn un_plano_vacio_da_una_pagina_sin_grupos_y_sin_anotar() {
        let h = pagina(
            &[],
            &OpcionesWeb {
                titulo: "Vacio".into(),
                ..Default::default()
            },
        );
        assert!(h.contains(r#""grupos":[]"#) && h.contains(r#""anotado":null"#));
        // Sin nada anotado no sale el boton de las anotaciones.
        assert!(!h.contains("id=\"b-anot\""));
    }

    /// Mide el HTML de los planos de muestra: `PIXPIN_MUESTRAS` (carpeta con
    /// DWG/DXF, copias) y `PIXPIN_SALIDA` (donde dejar las paginas).
    #[test]
    #[ignore = "convierte los planos de PIXPIN_MUESTRAS a HTML en PIXPIN_SALIDA"]
    fn paginas_de_las_muestras() {
        let (Ok(entrada), Ok(salida)) = (
            std::env::var("PIXPIN_MUESTRAS"),
            std::env::var("PIXPIN_SALIDA"),
        ) else {
            return;
        };
        std::fs::create_dir_all(&salida).unwrap();
        for e in std::fs::read_dir(&entrada).unwrap().flatten() {
            let ruta = e.path();
            if !matches!(
                ruta.extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.to_ascii_lowercase())
                    .as_deref(),
                Some("dwg" | "dxf")
            ) {
                continue;
            }
            let t = std::time::Instant::now();
            let (todo, _) = match crate::convertir::convertir_fichero(&ruta) {
                Ok(v) => v,
                Err(err) => {
                    println!("{:?}: ERROR {err}", e.file_name());
                    continue;
                }
            };
            let p = crate::convertir::convertir_fichero_por_capas(&ruta).unwrap();
            let c = todo.caja_util();
            let o = OpcionesWeb {
                titulo: ruta.file_stem().unwrap().to_string_lossy().into_owned(),
                origen: todo.origen,
                vista: [c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64],
                unidades: todo.unidades,
                claro: true,
                ..Default::default()
            };
            let h = pagina(&p.grupos, &o);
            let capas: std::collections::BTreeSet<_> =
                p.grupos.iter().map(|g| g.capa.clone()).collect();
            println!(
                "{:?}: {:?} dwg={} KB html={} KB grupos={} capas={} imagenes={} pxcad={} KB",
                e.file_name(),
                t.elapsed(),
                e.metadata().unwrap().len() / 1024,
                h.len() / 1024,
                p.grupos.len(),
                capas.len(),
                p.imagenes.len(),
                todo.a_bytes().len() / 1024
            );
            let nombre = format!("{}.html", ruta.file_stem().unwrap().to_string_lossy());
            std::fs::write(std::path::Path::new(&salida).join(nombre), h).unwrap();
        }
    }

    #[test]
    fn los_numeros_y_los_textos_del_json_son_validos() {
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(-0.0000001), "0");
        assert_eq!(num(f64::NAN), "0");
        assert_eq!(num(3.0), "3");
        assert_eq!(texto_json("a\"b\\c\nd"), r#""a\"b\\c\nd""#);
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}
