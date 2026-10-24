//! **El plano leido, empaquetado para que quepa en una pagina web.** Puerto
//! de `motor/PlanoWeb.kt` del movil.
//!
//! Lo que sale de [`crate::plano`] son a veces dos millones de puntos:
//! escritos a lo bruto («M 12.34 56.78 L …») son treinta megas de texto. Aqui
//! se aprietan en tres pasos que se apoyan uno en otro:
//!
//! 1. **Se empalman los tramos.** AutoCAD escribe cada segmento de una
//!    polilinea por separado (`m … l … S` seiscientas mil veces) aunque el
//!    final de uno sea el principio del siguiente. Con las coordenadas en
//!    punto fijo dos extremos que coinciden coinciden **exactamente**, y
//!    volver a coserlos quita ordenes, puntos repetidos y trabajo al
//!    navegador.
//! 2. **Se escriben diferencias**, en zigzag y de longitud variable: un
//!    tramo corto ocupa dos bytes. Ordenar los caminos por donde caen en el
//!    papel hace que los saltos entre uno y otro tambien sean pequenos.
//! 3. **Se comprime** con DEFLATE (el de un ZIP) y va en base64. La pagina lo
//!    descomprime con su propio `inflar` de cien lineas: ni
//!    `DecompressionStream` ni nada por la red.
//!
//! El formato del JSON es **el del movil, campo a campo**, para que una
//! pagina de un aparato y la del otro sean la misma cosa.

use std::fmt::Write as _;
use std::io::Write as _;

use crate::plano::{self, Brocha, CERRAR, CURVA, FINEZA, LINEA, MOVER, Plano};

/// El ancho en unidades del dibujo que PixPin da al papel de un PDF
/// (`PdfDoc.PAGE_WIDTH` del movil, `ANCHO_PAPEL_PDF` del lienzo).
pub const ANCHO_EN_UNIDADES: f64 = 1400.0;

/// **La pagina de un PDF lista para la web, o `None` si no compensa**: si no
/// se entiende, si pinta algo que aqui no esta (una lamina a la que le falta
/// algo es peor que una borrosa, porque lo borroso se ve y lo que falta no)
/// o si apenas tiene geometria (un escaneo). Ver
/// [`Plano::se_manda_como_lineas`].
pub fn de_bytes(bytes: &[u8], pagina: usize, ancho_en_unidades: f64) -> Option<String> {
    let p = plano::de_bytes(bytes, pagina)?;
    p.se_manda_como_lineas()
        .then(|| a_json(&p, ancho_en_unidades))
}

/// **Varias paginas de una vez** (desde 0), leyendo el indice del PDF una
/// sola: con un documento de treinta hojas, abrirlo treinta veces seria lo
/// caro. `None` en las que no compensan.
pub fn de_paginas(bytes: &[u8], cuales: &[usize], ancho_en_unidades: f64) -> Vec<Option<String>> {
    let mut salida = Vec::with_capacity(cuales.len());
    plano::con_cada(bytes, cuales, |_, p| {
        salida.push(
            p.filter(Plano::se_manda_como_lineas)
                .map(|p| a_json(&p, ancho_en_unidades)),
        );
    });
    salida
}

/// **El plano como JSON**: todo sale ya en unidades de un papel de
/// `ancho_en_unidades`, y el visor no tiene que saber de puntos.
pub fn a_json(plano: &Plano, ancho_en_unidades: f64) -> String {
    let escala = ancho_en_unidades / plano.ancho.max(1e-9);
    let por_paso = escala / FINEZA as f64;
    let mut flujo = Flujo::default();
    let mut cabeceras: Vec<String> = Vec::with_capacity(plano.brochas.len());
    for b in &plano.brochas {
        let ordenes = escribir_brocha(b, &mut flujo);
        if ordenes == 0 {
            continue;
        }
        let mut c = format!(
            "{{\"c\":{},\"t\":\"{}\",\"g\":{}",
            b.capa,
            hex(b.color),
            numero(b.grosor * escala, 3)
        );
        if b.relleno {
            c.push_str(",\"r\":1");
        }
        if b.par_impar {
            // Lo que el movil no manda todavia y el visor de aqui si sabe
            // pintar: el relleno par-impar de las letras con agujeros.
            c.push_str(",\"p\":1");
        }
        if b.alfa < 0.999 {
            let _ = write!(c, ",\"o\":{}", numero(b.alfa, 3));
        }
        if !b.raya.is_empty() {
            let r: Vec<String> = b.raya.iter().map(|v| numero(v * escala, 3)).collect();
            let _ = write!(c, ",\"d\":[{}]", r.join(","));
        }
        let _ = write!(c, ",\"n\":{ordenes}}}");
        cabeceras.push(c);
    }
    let comprimido = comprimido(&flujo.b);
    let mut s = String::with_capacity(1024 + comprimido.len());
    let _ = write!(
        s,
        "{{\"a\":{},\"b\":{},\"e\":{},\"capas\":[",
        numero(ancho_en_unidades, 3),
        numero(plano.alto * escala, 3),
        numero(por_paso, 9)
    );
    for (i, c) in plano.capas.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "{{\"n\":\"{}\"", texto(&c.nombre));
        if !c.encendida {
            s.push_str(",\"v\":0");
        }
        s.push('}');
    }
    s.push_str("],\"brochas\":[");
    s.push_str(&cabeceras.join(","));
    // **Cada imagen se escribe una vez** y cada colocacion dice con `i`
    // cual es la suya: 81 colocaciones de 21 imagenes eran 1,3 MB donde
    // bastan 0,3.
    s.push_str("],\"imagenes\":[");
    let mut orden: Vec<usize> = Vec::new();
    for f in &plano.fotos {
        if !orden.contains(&f.id) {
            orden.push(f.id);
        }
    }
    for (i, sena) in orden.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let f = plano
            .fotos
            .iter()
            .find(|f| f.id == *sena)
            .expect("la sena sale de una foto");
        let _ = write!(s, "{{\"u\":\"data:{};base64,{}\"", f.tipo, base64(&f.datos));
        if let (Some(m), Some(t)) = (&f.mascara, f.tipo_mascara) {
            let _ = write!(s, ",\"k\":\"data:{t};base64,{}\"", base64(m));
        }
        s.push('}');
    }
    s.push_str("],\"fotos\":[");
    for (i, f) in plano.fotos.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let k = orden.iter().position(|x| *x == f.id).unwrap_or(0);
        let _ = write!(
            s,
            "{{\"c\":{},\"i\":{k},\"m\":[{},{},{},{},{},{}]",
            f.capa,
            numero(f.a * escala, 3),
            numero(f.b * escala, 3),
            numero(f.c * escala, 3),
            numero(f.d * escala, 3),
            numero(f.x * escala, 3),
            numero(f.y * escala, 3)
        );
        if f.alfa < 0.999 {
            let _ = write!(s, ",\"o\":{}", numero(f.alfa, 3));
        }
        s.push('}');
    }
    s.push_str("],\"textos\":[");
    for (i, t) in plano.textos.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(
            s,
            "{{\"c\":{},\"t\":\"{}\",\"s\":\"{}\",\"m\":[{},{},{},{},{},{}],\"w\":{}",
            t.capa,
            hex(t.color),
            texto(&t.texto),
            numero(t.a * escala, 3),
            numero(t.b * escala, 3),
            numero(t.c * escala, 3),
            numero(t.d * escala, 3),
            numero(t.x * escala, 3),
            numero(t.y * escala, 3),
            numero(t.ancho, 4)
        );
        if t.familia != "sans-serif" {
            let _ = write!(s, ",\"f\":\"{}\"", t.familia);
        }
        if t.negrita {
            s.push_str(",\"n\":1");
        }
        if t.cursiva {
            s.push_str(",\"i\":1");
        }
        if t.alfa < 0.999 {
            let _ = write!(s, ",\"o\":{}", numero(t.alfa, 3));
        }
        s.push('}');
    }
    let _ = write!(s, "],\"datos\":\"{comprimido}\"}}");
    s
}

// ---------------------------------------------------------------------------
// El flujo de ordenes

/// Escribe una brocha y devuelve cuantas ordenes ocupo: los subcaminos, los
/// que se tocan empalmados (no en los rellenos: su orden es su dibujo) y en
/// orden de bandas.
fn escribir_brocha(b: &Brocha, flujo: &mut Flujo) -> usize {
    let mut trozos = trocear(b);
    if trozos.is_empty() {
        return 0;
    }
    let cabezas: Vec<usize> = if b.relleno {
        (0..trozos.len()).collect()
    } else {
        empalmar(&mut trozos)
    };
    cabezas
        .iter()
        .map(|&c| escribir_trozo(b, &trozos, c, flujo))
        .sum()
}

/// Un subcamino: donde empiezan y acaban sus ordenes y sus puntos.
#[derive(Clone, Debug)]
struct Trozo {
    op0: usize,
    op1: usize,
    pt0: usize,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    cerrado: bool,
    siguiente: Option<usize>,
    usado: bool,
}

fn trocear(b: &Brocha) -> Vec<Trozo> {
    let mut out = Vec::new();
    let (mut i, mut p) = (0usize, 0usize);
    while i < b.ops.len() {
        if b.ops[i] != MOVER {
            i += 1;
            continue;
        }
        let (op0, pt0) = (i, p);
        if p >= b.xs.len() {
            break;
        }
        let (x0, y0) = (b.xs[p], b.ys[p]);
        let mut cerrado = false;
        let mut ultimo = p;
        i += 1;
        p += 1;
        while i < b.ops.len() {
            match b.ops[i] {
                MOVER => break,
                LINEA => {
                    ultimo = p;
                    p += 1;
                }
                CURVA => {
                    ultimo = p + 2;
                    p += 3;
                }
                CERRAR => cerrado = true,
                _ => {}
            }
            i += 1;
        }
        if ultimo < b.xs.len() && p <= b.xs.len() {
            out.push(Trozo {
                op0,
                op1: i,
                pt0,
                x0,
                y0,
                x1: b.xs[ultimo],
                y1: b.ys[ultimo],
                cerrado,
                siguiente: None,
                usado: false,
            });
        }
    }
    out
}

/// **Cose los subcaminos que se tocan**: se indexan por donde empiezan y se
/// tira del hilo. Lo que queda suelto se ordena por bandas de 64 puntos, de
/// arriba abajo y de izquierda a derecha, para que el salto de un camino al
/// siguiente sea corto y el compresor lo note. Devuelve las cabezas.
fn empalmar(trozos: &mut [Trozo]) -> Vec<usize> {
    let mut por_inicio: std::collections::HashMap<(i32, i32), Vec<usize>> =
        std::collections::HashMap::with_capacity(trozos.len() * 2);
    for (k, t) in trozos.iter().enumerate() {
        if !t.cerrado {
            por_inicio.entry((t.x0, t.y0)).or_default().push(k);
        }
    }
    let mut cabezas = Vec::with_capacity(trozos.len());
    for k in 0..trozos.len() {
        if trozos[k].usado {
            continue;
        }
        trozos[k].usado = true;
        cabezas.push(k);
        let mut actual = k;
        let mut largo = 0;
        while largo < 100_000 {
            largo += 1;
            let fin = (trozos[actual].x1, trozos[actual].y1);
            let Some(sig) = por_inicio
                .get(&fin)
                .and_then(|c| c.iter().copied().find(|&j| !trozos[j].usado))
            else {
                break;
            };
            trozos[sig].usado = true;
            trozos[actual].siguiente = Some(sig);
            actual = sig;
        }
    }
    let banda = FINEZA * 64;
    cabezas.sort_by_key(|&k| (trozos[k].y0.div_euclid(banda), trozos[k].x0));
    cabezas
}

/// Escribe un camino y los que le siguen empalmados: el segundo no vuelve a
/// levantar la pluma, sigue desde donde estaba el primero.
fn escribir_trozo(b: &Brocha, trozos: &[Trozo], cabeza: usize, flujo: &mut Flujo) -> usize {
    let mut ordenes = 0;
    let mut t = Some(cabeza);
    let mut primero = true;
    while let Some(k) = t {
        let tr = &trozos[k];
        let mut p = tr.pt0;
        for i in tr.op0..tr.op1 {
            match b.ops[i] {
                MOVER => {
                    if primero {
                        flujo.op(MOVER);
                        flujo.punto(b.xs[p], b.ys[p]);
                        ordenes += 1;
                    }
                    p += 1;
                }
                LINEA => {
                    flujo.op(LINEA);
                    flujo.punto(b.xs[p], b.ys[p]);
                    ordenes += 1;
                    p += 1;
                }
                CURVA => {
                    flujo.op(CURVA);
                    for q in 0..3 {
                        flujo.punto(b.xs[p + q], b.ys[p + q]);
                    }
                    ordenes += 1;
                    p += 3;
                }
                CERRAR => {
                    flujo.op(CERRAR);
                    ordenes += 1;
                }
                _ => {}
            }
        }
        primero = false;
        t = tr.siguiente;
    }
    ordenes
}

/// El flujo de bytes: una orden y sus diferencias, en zigzag y de longitud
/// variable (siete bits por byte, el octavo dice si sigue).
#[derive(Default)]
struct Flujo {
    b: Vec<u8>,
    ux: i32,
    uy: i32,
}

impl Flujo {
    fn op(&mut self, o: u8) {
        self.b.push(o);
    }

    fn punto(&mut self, x: i32, y: i32) {
        self.varint(x.wrapping_sub(self.ux));
        self.varint(y.wrapping_sub(self.uy));
        self.ux = x;
        self.uy = y;
    }

    fn varint(&mut self, v: i32) {
        let mut z = ((v << 1) ^ (v >> 31)) as u32;
        loop {
            if z & !0x7F == 0 {
                self.b.push(z as u8);
                return;
            }
            self.b.push(((z & 0x7F) | 0x80) as u8);
            z >>= 7;
        }
    }
}

/// DEFLATE crudo (sin cabecera zlib) y en base64: lo que sabe leer `inflar`.
pub(crate) fn comprimido(datos: &[u8]) -> String {
    let mut z = flate2::write::DeflateEncoder::new(
        Vec::with_capacity(datos.len() / 3 + 64),
        flate2::Compression::best(),
    );
    let _ = z.write_all(datos);
    base64(&z.finish().unwrap_or_default())
}

fn base64(datos: &[u8]) -> String {
    pixpin_motor2d::exportar::base64(datos)
}

// ---------------------------------------------------------------------------
// JSON a mano

fn hex(color: u32) -> String {
    format!("#{:06x}", color & 0xFFFFFF)
}

/// **Un numero con los decimales justos**: con miles de brochas, escribir
/// `0.9166666666666666` donde basta `0.917` son kilobytes de nada.
pub(crate) fn numero(v: f64, decimales: u32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let f = 10f64.powi(decimales as i32);
    let r = (v * f).round() / f;
    if r == r.trunc() && r.abs() < 1e15 {
        return format!("{}", r as i64);
    }
    let s = format!("{r:.*}", decimales as usize);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Texto dentro de una cadena JSON que ademas va dentro de un `<script>`:
/// `</script` dentro del JSON cerraria la etiqueta que lo envuelve.
pub(crate) fn texto(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '<' => o.push_str("\\u003c"),
            '>' => o.push_str("\\u003e"),
            '&' => o.push_str("\\u0026"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o
}

#[cfg(test)]
#[path = "plano_web/pruebas.rs"]
mod pruebas;
