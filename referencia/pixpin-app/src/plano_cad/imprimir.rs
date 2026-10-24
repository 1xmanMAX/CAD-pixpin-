//! **Imprimir partes del plano** (como PixPin Android v0.111 y v0.112, que lo
//! pidio el usuario el 9-oct-2026: «ponerle un marco a una parte y mandarlo
//! a imprimir, como los marcos del lienzo», y «un frame predeterminado, como
//! un membrete»). Cada marco es una hoja; el papel y la impresora (o «Guardar
//! como PDF») los elige el dialogo moderno de Windows, con vista previa,
//! como el lienzo (`ventana_editor/exportar/imprimir.rs`).
//!
//! Se pinta **en vectores** con Direct2D: rayas, rellenos, circulos, letras
//! (cada una con su forma) y las cotas puestas, en los colores del papel
//! blanco (el 7 en negro y lo muy claro oscurecido). Los sombreados con
//! patron salen como un tono de su color, igual que en el movil.
//!
//! La lamina (`hoja`) es la de `planos/ImprimirPlano.kt`: el recuadro con los
//! margenes de la norma ISO 5457 (20 mm a la izquierda, 10 en lo demas) y el
//! membrete abajo a la derecha (proyecto, lamina n/N, titulo, dibujo, escala,
//! fecha y unidades); con las unidades del plano, cada hoja va a la 1:n de
//! siempre que quepa.

use std::collections::HashMap;
use std::path::Path;

use super::anotar::ParaImprimir;
use anyhow::{Context, Result};
use pixpin_cad::modelo::{CORTE, GLIFO_DE_RAYAS, Modelo, Tramo};
use pixpin_render::imprimir_moderno::{self as moderno, Documento, OpcionPropia, Pagina, Pedido};
use pixpin_render::{Color, MotorRender, Pintor, RectF};
use windows::Win32::Foundation::HWND;

/// Milimetros en DIP (1/96 de pulgada).
const MM: f32 = 96.0 / 25.4;
/// El margen del papel sin lamina: los 28 puntos del movil.
const MARGEN: f32 = 28.0 * 96.0 / 72.0;
/// Las rayas del plano: 0,35 puntos, como el movil.
const RAYA: f32 = 0.35 * 96.0 / 72.0;

/// Un marco `[x0, y0, x1, y1]` del plano (respecto a su origen), ordenado.
pub type Marco = [f64; 4];

/// Lo que dice el membrete.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lamina {
    pub con_membrete: bool,
    pub proyecto: String,
    pub titulo: String,
    pub autor: String,
    pub fecha: String,
}

/// Los milimetros que mide una unidad del plano (`$INSUNITS`).
pub fn mm_por_unidad(unidades: u32) -> Option<f64> {
    Some(match unidades {
        1 => 25.4,
        2 => 304.8,
        4 => 1.0,
        5 => 10.0,
        6 => 1000.0,
        7 => 1_000_000.0,
        14 => 100.0,
        _ => return None,
    })
}

/// Las escalas de siempre (1:n).
const ESCALAS: [u32; 32] = [
    1, 2, 5, 10, 20, 25, 50, 75, 100, 125, 150, 200, 250, 300, 400, 500, 750, 1000, 1250, 1500,
    2000, 2500, 5000, 7500, 10000, 12500, 15000, 20000, 25000, 50000, 75000, 100000,
];

/// La escala normal mas grande con que `ancho_mm` x `alto_mm` de la realidad
/// caben en el papel (mm): la primera 1:n con n >= lo justo.
pub fn escala_que_cabe(
    ancho_mm: f64,
    alto_mm: f64,
    papel_ancho: f64,
    papel_alto: f64,
) -> Option<u32> {
    if papel_ancho <= 0.0 || papel_alto <= 0.0 {
        return None;
    }
    let justa = (ancho_mm / papel_ancho).max(alto_mm / papel_alto);
    ESCALAS
        .iter()
        .copied()
        .find(|&n| n as f64 >= justa * 0.9999)
}

/// El color en papel blanco: el 7 (alfa 0) en negro y lo muy claro oscurecido.
fn color_en_papel(c: u32, alfa: f32) -> Color {
    let a = (c >> 24) & 255;
    if a == 0 {
        return Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: alfa,
        };
    }
    let (mut r, mut g, mut b) = (
        (c & 255) as f32 / 255.0,
        ((c >> 8) & 255) as f32 / 255.0,
        ((c >> 16) & 255) as f32 / 255.0,
    );
    let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if l > 0.62 {
        let k = 0.5 / l;
        r *= k;
        g *= k;
        b *= k;
    }
    Color {
        r,
        g,
        b,
        a: a as f32 / 255.0 * alfa,
    }
}

fn corta(t: &Tramo, m: &Marco) -> bool {
    (t.caja[0] as f64) <= m[2]
        && (t.caja[2] as f64) >= m[0]
        && (t.caja[1] as f64) <= m[3]
        && (t.caja[3] as f64) >= m[1]
}

/// Pinta una hoja entera de `papel` (DIP): con lamina, el recuadro, el
/// membrete y el plano en lo que queda; si no, el plano con un margen.
#[allow(clippy::too_many_arguments)]
pub fn hoja(
    p: &Pintor,
    m: &Modelo,
    marco: &Marco,
    cotas: &[Vec<[f64; 2]>],
    anotado: Option<(&ParaImprimir, f32)>,
    papel: (f32, f32),
    lamina: &Lamina,
    numero: usize,
    total: usize,
) {
    let (w, h) = papel;
    if !lamina.con_membrete {
        dibujar(
            p,
            m,
            marco,
            cotas,
            anotado,
            (MARGEN, MARGEN, w - 2.0 * MARGEN, h - 2.0 * MARGEN),
            None,
        );
        return;
    }
    let (izq, otro) = ((20.0 * MM).min(w * 0.07), (10.0 * MM).min(w * 0.035));
    let (bx0, by0, bx1, by1) = (izq, otro, w - otro, h - otro);
    let ancho_m = (180.0 * MM).min(bx1 - bx0);
    let fila = (9.0 * MM).min((by1 - by0) * 0.06);
    let (mx0, my0) = (bx1 - ancho_m, by1 - fila * 4.0);
    let aire = 4.0 * MM;
    let caja = (
        bx0 + aire,
        by0 + aire,
        (bx1 - bx0) - 2.0 * aire,
        (my0 - by0) - 2.0 * aire,
    );
    let escala = escala_de(m, marco, caja.2, caja.3);
    dibujar(p, m, marco, cotas, anotado, caja, escala.map(|e| e.1));
    let negro = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| RectF {
        x: x0,
        y: y0,
        ancho: x1 - x0,
        alto: y1 - y0,
    };
    p.trazar(rect(bx0, by0, bx1, by1), 1.2 * 96.0 / 72.0, negro);
    p.trazar(rect(mx0, my0, bx1, by1), 1.2 * 96.0 / 72.0, negro);
    let fino = 0.5 * 96.0 / 72.0;
    for k in 1..4 {
        let y = my0 + k as f32 * fila;
        p.linea((mx0, y), (bx1, y), fino, negro);
    }
    let col = ancho_m * 0.72;
    p.linea((mx0 + col, my0), (mx0 + col, my0 + fila), fino, negro);
    let cuarto = ancho_m / 4.0;
    for k in 1..4 {
        let x = mx0 + k as f32 * cuarto;
        p.linea((x, my0 + 2.0 * fila), (x, by1), fino, negro);
    }
    let gris = Color {
        r: 0.33,
        g: 0.33,
        b: 0.33,
        a: 1.0,
    };
    let (t_rot, t_dato) = (fila * 0.24, fila * 0.40);
    let celda = |x0: f32, y0: f32, ancho: f32, etiqueta: &str, valor: &str, alto: f32| {
        p.texto(etiqueta, x0 + 1.5 * MM, y0 + 0.6 * MM, t_rot, gris);
        let mut t: String = valor.to_string();
        while t.chars().count() > 1 && p.medir_texto(&t, t_dato).0 > ancho - 3.0 * MM {
            t.pop();
        }
        if t != valor && !t.is_empty() {
            t.pop();
            t.push('…');
        }
        let (_, th) = p.medir_texto(&t, t_dato);
        p.texto(&t, x0 + 1.5 * MM, y0 + alto - th - 1.0 * MM, t_dato, negro);
    };
    celda(mx0, my0, col, "PROYECTO", &lamina.proyecto, fila);
    celda(
        mx0 + col,
        my0,
        ancho_m - col,
        "LÁMINA",
        &format!("{numero} / {total}"),
        fila,
    );
    celda(
        mx0,
        my0 + fila,
        ancho_m,
        "TÍTULO",
        &lamina.titulo,
        fila * 2.0,
    );
    let unidad = pixpin_cad::ventana::sufijo(m.unidades).trim();
    let unidad = if unidad.is_empty() { "—" } else { unidad };
    let esc = escala.map_or("Sin escala".to_string(), |(n, _)| format!("1:{n}"));
    celda(mx0, my0 + 3.0 * fila, cuarto, "DIBUJÓ", &lamina.autor, fila);
    celda(mx0 + cuarto, my0 + 3.0 * fila, cuarto, "ESCALA", &esc, fila);
    celda(
        mx0 + 2.0 * cuarto,
        my0 + 3.0 * fila,
        cuarto,
        "FECHA",
        &lamina.fecha,
        fila,
    );
    celda(
        mx0 + 3.0 * cuarto,
        my0 + 3.0 * fila,
        cuarto,
        "UNIDADES",
        unidad,
        fila,
    );
}

/// La 1:n que cabe en `aw` x `ah` DIP y los DIP por unidad del plano.
fn escala_de(m: &Modelo, marco: &Marco, aw: f32, ah: f32) -> Option<(u32, f32)> {
    let mm = mm_por_unidad(m.unidades)?;
    let n = escala_que_cabe(
        (marco[2] - marco[0]) * mm,
        (marco[3] - marco[1]) * mm,
        (aw / MM) as f64,
        (ah / MM) as f64,
    )?;
    Some((n, (mm / n as f64) as f32 * MM))
}

/// Dibuja `marco` del plano en la caja `(x, y, ancho, alto)` (DIP), centrado;
/// con `fija` (DIP por unidad) a esa escala, si no lo mas grande que quepa.
pub(super) fn dibujar(
    p: &Pintor,
    m: &Modelo,
    marco: &Marco,
    cotas: &[Vec<[f64; 2]>],
    anotado: Option<(&ParaImprimir, f32)>,
    caja: (f32, f32, f32, f32),
    fija: Option<f32>,
) {
    let (ax, ay, aw, ah) = caja;
    let (mw, mh) = ((marco[2] - marco[0]) as f32, (marco[3] - marco[1]) as f32);
    let s = fija.unwrap_or((aw / mw).min(ah / mh));
    if !(s > 0.0) || !s.is_finite() {
        return;
    }
    let (ox, oy) = (ax + (aw - mw * s) / 2.0, ay + (ah - mh * s) / 2.0);
    let (x0, y1) = (marco[0] as f32, marco[3] as f32);
    let a_papel = |x: f32, y: f32| (ox + (x - x0) * s, oy + (y1 - y) * s);
    let recorte = RectF {
        x: ox,
        y: oy,
        ancho: mw * s,
        alto: mh * s,
    };
    // Lo que en papel mediria menos de una decima de punto no se dibuja.
    let minimo = 0.1 / s;
    p.con_recorte(recorte, |p| {
        // 1. Rellenos y sombreados (estos, como un tono).
        let triangulos = |indices: &[u32],
                          verts: &dyn Fn(u32) -> (f32, f32, u32),
                          tramos: &[Tramo],
                          alfa: f32| {
            let mut por_color: HashMap<u32, Vec<Vec<(f32, f32)>>> = HashMap::new();
            for t in tramos
                .iter()
                .filter(|t| t.tamano >= minimo && corta(t, marco))
            {
                let fin = (t.desde + t.cuantos) as usize;
                for tri in indices[t.desde as usize..fin.min(indices.len())].chunks_exact(3) {
                    let (a, b, c) = (verts(tri[0]), verts(tri[1]), verts(tri[2]));
                    por_color.entry(a.2).or_default().push(vec![
                        a_papel(a.0, a.1),
                        a_papel(b.0, b.1),
                        a_papel(c.0, c.1),
                    ]);
                }
            }
            for (c, figuras) in por_color {
                p.figuras(&figuras, true, 0.0, color_en_papel(c, alfa));
            }
        };
        let v = |i: u32| {
            m.vertices
                .get(i as usize)
                .map_or((0.0, 0.0, 0), |v| (v.x, v.y, v.color))
        };
        triangulos(&m.triangulos, &v, &m.tramos_triangulos, 1.0);
        let vt = |i: u32| {
            m.vertices_trama
                .get(i as usize)
                .map_or((0.0, 0.0, 0), |v| (v.x, v.y, v.color))
        };
        triangulos(&m.triangulos_trama, &vt, &m.tramos_trama, 0.27);
        // 2. Rayas: cada tira partida donde cambia de color.
        let mut por_color: HashMap<u32, Vec<Vec<(f32, f32)>>> = HashMap::new();
        for t in m
            .tramos_lineas
            .iter()
            .filter(|t| t.tamano >= minimo * 0.75 && corta(t, marco))
        {
            let mut tira: Vec<(f32, f32)> = Vec::new();
            let mut color = 0u32;
            let fin = ((t.desde + t.cuantos) as usize).min(m.lineas.len());
            for &i in &m.lineas[t.desde as usize..fin] {
                if i == CORTE {
                    if tira.len() >= 2 {
                        por_color
                            .entry(color)
                            .or_default()
                            .push(std::mem::take(&mut tira));
                    }
                    tira.clear();
                    continue;
                }
                let Some(q) = m.vertices.get(i as usize) else {
                    continue;
                };
                if q.color != color && !tira.is_empty() {
                    let ultimo = *tira.last().unwrap_or(&(0.0, 0.0));
                    if tira.len() >= 2 {
                        por_color
                            .entry(color)
                            .or_default()
                            .push(std::mem::take(&mut tira));
                    }
                    tira = vec![ultimo];
                }
                color = q.color;
                tira.push(a_papel(q.x, q.y));
            }
            if tira.len() >= 2 {
                por_color.entry(color).or_default().push(tira);
            }
        }
        // 3. Circulos y arcos, troceados.
        for t in m
            .tramos_arcos
            .iter()
            .filter(|t| t.tamano >= minimo * 0.75 && corta(t, marco))
        {
            let fin = ((t.desde + t.cuantos) as usize).min(m.arcos.len());
            for a in &m.arcos[t.desde as usize..fin] {
                let n = ((a.barrido.abs() * a.radio * s / 2.0).ceil() as usize).clamp(8, 720);
                let pts: Vec<(f32, f32)> = (0..=n)
                    .map(|k| {
                        let ang = a.inicio + a.barrido * k as f32 / n as f32;
                        a_papel(
                            a.centro[0] + a.radio * ang.cos(),
                            a.centro[1] + a.radio * ang.sin(),
                        )
                    })
                    .collect();
                por_color.entry(a.color).or_default().push(pts);
            }
        }
        for (c, figuras) in &por_color {
            p.figuras(figuras, false, RAYA, color_en_papel(*c, 1.0));
        }
        // 4. Letras: un texto de menos de un punto no se lee.
        let mut rellenas: HashMap<u32, Vec<Vec<(f32, f32)>>> = HashMap::new();
        let mut de_rayas: HashMap<u32, Vec<Vec<(f32, f32)>>> = HashMap::new();
        for t in m
            .tramos_letras
            .iter()
            .filter(|t| t.tamano >= 1.0 / s && corta(t, marco))
        {
            let fin = ((t.desde + t.cuantos) as usize).min(m.letras.len());
            for l in &m.letras[t.desde as usize..fin] {
                let Some(g) = m.glifos.get(l.glifo as usize) else {
                    continue;
                };
                let rayas = g[1] & GLIFO_DE_RAYAS != 0;
                let (desde, n) = (g[0] as usize, (g[1] & !GLIFO_DE_RAYAS) as usize);
                let Some(malla) = m.malla_letras.get(desde..desde + n) else {
                    continue;
                };
                let pt = |e: &[f32; 2]| {
                    a_papel(
                        l.pos[0] + l.m[0] * e[0] + l.m[1] * e[1],
                        l.pos[1] + l.m[2] * e[0] + l.m[3] * e[1],
                    )
                };
                if rayas {
                    let d = de_rayas.entry(l.color).or_default();
                    for par in malla.chunks_exact(2) {
                        d.push(vec![pt(&par[0]), pt(&par[1])]);
                    }
                } else {
                    let d = rellenas.entry(l.color).or_default();
                    for tri in malla.chunks_exact(3) {
                        d.push(vec![pt(&tri[0]), pt(&tri[1]), pt(&tri[2])]);
                    }
                }
            }
        }
        for (c, f) in &rellenas {
            p.figuras(f, true, 0.0, color_en_papel(*c, 1.0));
        }
        for (c, f) in &de_rayas {
            p.figuras(f, false, RAYA, color_en_papel(*c, 1.0));
        }
        // 5. Las cotas, en naranja con su medida.
        let naranja = Color {
            r: 0.88,
            g: 0.42,
            b: 0.0,
            a: 1.0,
        };
        let tam = 7.0 * 96.0 / 72.0;
        for cadena in cotas {
            let mut total = 0.0;
            for par in cadena.windows(2) {
                let (a, b) = (
                    a_papel(par[0][0] as f32, par[0][1] as f32),
                    a_papel(par[1][0] as f32, par[1][1] as f32),
                );
                p.linea(a, b, 0.8 * 96.0 / 72.0, naranja);
                let d = (par[1][0] - par[0][0]).hypot(par[1][1] - par[0][1]);
                total += d;
                let t = pixpin_cad::ventana::medida(d, m.unidades);
                p.texto(
                    &t,
                    (a.0 + b.0) / 2.0 + 2.0,
                    (a.1 + b.1) / 2.0 - tam - 2.0,
                    tam,
                    naranja,
                );
            }
            if cadena.len() > 2
                && let Some(u) = cadena.last()
            {
                let q = a_papel(u[0] as f32, u[1] as f32);
                p.texto(
                    &format!("Σ {}", pixpin_cad::ventana::medida(total, m.unidades)),
                    q.0 + 4.0,
                    q.1 + 2.0,
                    tam,
                    naranja,
                );
            }
        }
        // 6. Lo anotado, con el motor del lienzo: la capa (x a la derecha, y
        // hacia abajo, en la unidad de la letra del plano) puesta en la hoja. `esc` es la
        // vista del papel que ya habia (la de la miniatura, o 1).
        if let Some((a, esc)) = anotado {
            let u = a.unidad as f32;
            let cero = ((ox - x0 * s) * esc, (oy + y1 * s) * esc);
            let visible = (
                marco[0] as f32 / u,
                -(marco[3] as f32) / u,
                marco[2] as f32 / u,
                -(marco[1] as f32) / u,
            );
            a.pintar(p, u * s * esc, cero, visible);
            p.poner_vista((0.0, 0.0), esc, (0.0, 0.0));
        }
    });
    // El borde del marco, fino.
    p.trazar(
        recorte,
        0.5 * 96.0 / 72.0,
        Color {
            r: 0.53,
            g: 0.53,
            b: 0.53,
            a: 1.0,
        },
    );
}

/// El plano que se imprime: una copia, que el visor sigue andando.
struct PlanoImpreso {
    m: Modelo,
    marcos: Vec<Marco>,
    cotas: Vec<Vec<[f64; 2]>>,
    anotado: Option<ParaImprimir>,
    lamina: Lamina,
}

/// La opcion propia del dialogo: con lamina o sin ella.
const OPCION: &str = "pixpin-lamina";
const CON: &str = "con";
const SIN: &str = "sin";

impl Documento for PlanoImpreso {
    fn paginar(&mut self, _motor: &MotorRender, alcance: Option<&str>) -> usize {
        self.lamina.con_membrete = alcance != Some(SIN);
        self.marcos.len()
    }

    fn pintar(&self, i: usize, pagina: Pagina, p: &Pintor) {
        let Some(marco) = self.marcos.get(i) else {
            return;
        };
        // Todo en DIP del papel.
        p.poner_vista((0.0, 0.0), pagina.escala, (0.0, 0.0));
        hoja(
            p,
            &self.m,
            marco,
            &self.cotas,
            self.anotado.as_ref().map(|a| (a, pagina.escala)),
            pagina.papel,
            &self.lamina,
            i + 1,
            self.marcos.len(),
        );
    }
}

/// La fecha de hoy, «9/10/2026».
fn hoy() -> String {
    let dias = pixpin_shell::entorno::a_local(pixpin_shell::entorno::ahora_utc_ms())
        .div_euclid(86_400_000);
    // De dias desde 1970 a fecha civil (Howard Hinnant).
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let anio = yoe + era * 400 + if mes <= 2 { 1 } else { 0 };
    format!("{d}/{mes}/{anio}")
}

/// El nombre del proyecto: el del proyecto de PixPin si el plano vive en uno
/// (`proyectos/<id>/archivos/`), si no el de su carpeta.
fn proyecto_de(ruta: &Path) -> String {
    let carpeta = ruta.parent();
    if let Some(c) = carpeta
        && c.file_name().is_some_and(|n| n == "archivos")
        && let Some(raiz) = c.parent()
        && let Ok(t) = std::fs::read_to_string(raiz.join("proyecto.json"))
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&t)
        && let Some(n) = v.get("nombre").and_then(|n| n.as_str())
    {
        return n.to_string();
    }
    carpeta
        .and_then(|c| c.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// **Abre el dialogo de imprimir** con los marcos del plano (sin marcos, el
/// plano entero en una hoja). Se llama desde el hilo del visor, que sigue
/// atendiendo sus mensajes mientras el dialogo esta abierto.
pub fn imprimir(
    ventana: isize,
    m: &Modelo,
    marcos: &[Marco],
    cotas: &[Vec<[f64; 2]>],
    anotado: Option<ParaImprimir>,
    ruta: &Path,
) -> Result<()> {
    let marcos: Vec<Marco> = if marcos.is_empty() {
        vec![[
            m.caja[0] as f64,
            m.caja[1] as f64,
            m.caja[2] as f64,
            m.caja[3] as f64,
        ]]
    } else {
        marcos.to_vec()
    };
    let titulo = ruta
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lamina = Lamina {
        con_membrete: true,
        proyecto: proyecto_de(ruta),
        titulo: titulo.clone(),
        autor: std::env::var("USERNAME").unwrap_or_default(),
        fecha: hoy(),
    };
    let apaisada = marcos[0][2] - marcos[0][0] > marcos[0][3] - marcos[0][1];
    let d = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para imprimir")?;
    let motor = MotorRender::nuevo(d.d3d()).context("sin motor para imprimir")?;
    let pedido = Pedido {
        titulo,
        apaisada,
        opcion: Some(OpcionPropia {
            id: OPCION.into(),
            titulo: "Lámina".into(),
            elementos: vec![
                (CON.into(), "Con marco y membrete".into()),
                (SIN.into(), "Sin marco".into()),
            ],
            inicial: CON.into(),
        }),
        ppp_previa: 144.0,
    };
    let doc = PlanoImpreso {
        m: m.clone(),
        marcos,
        cotas: cotas.to_vec(),
        anotado,
        lamina,
    };
    moderno::mostrar(
        HWND(ventana as *mut _),
        pedido,
        d.d3d().clone(),
        motor,
        Box::new(doc),
    )
    .context("el dialogo de imprimir no se abrio")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_escala_es_la_de_siempre_que_cabe() {
        // 20 m x 10 m en 250 x 150 mm: lo justo es 1:80, va a 1:100.
        assert_eq!(escala_que_cabe(20_000.0, 10_000.0, 250.0, 150.0), Some(100));
        // Lo justo exacto vale.
        assert_eq!(escala_que_cabe(5_000.0, 1.0, 100.0, 100.0), Some(50));
        // Caso negativo: mas grande que la mas pequeña, sin escala.
        assert_eq!(escala_que_cabe(1e12, 1.0, 100.0, 100.0), None);
        assert_eq!(mm_por_unidad(6), Some(1000.0));
        assert_eq!(mm_por_unidad(0), None);
    }

    #[test]
    fn en_papel_el_color_7_es_negro_y_lo_claro_se_oscurece() {
        let negro = color_en_papel(0x00ff_ffff, 1.0);
        assert_eq!((negro.r, negro.g, negro.b), (0.0, 0.0, 0.0));
        let amarillo = color_en_papel(0xff00_ffff, 1.0);
        assert!(amarillo.r < 0.9 && amarillo.g < 0.9);
        // Caso negativo: un color oscuro queda igual.
        let azul = color_en_papel(0xff80_0000, 1.0);
        assert!((azul.b - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn la_fecha_de_hoy_tiene_dia_mes_y_anio() {
        let f = hoy();
        let partes: Vec<u32> = f.split('/').filter_map(|x| x.parse().ok()).collect();
        assert_eq!(partes.len(), 3);
        assert!(
            (1..=31).contains(&partes[0]) && (1..=12).contains(&partes[1]) && partes[2] >= 2026
        );
    }
}
