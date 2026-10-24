//! **Del croquis a triangulos**, para la ventana del visor 3D.
//!
//! El movil pinta el croquis en 2D cada fotograma (`Croquis3DLienzo.kt`,
//! `Croquis3DPluma.kt`); aqui la tarjeta ya sabe de profundidad y luz, asi
//! que cada cosa se convierte una vez en lo que es de verdad en el espacio:
//!
//! - **Un trazo redondo es un tubo**: su seccion barrida por el recorrido,
//!   del ancho del mundo (`calibre`), con el pulso de la mano
//!   ([`super::anchos_del_mundo`]: aprieta y engorda, corre y adelgaza, las
//!   puntas afiladas) y acabado en domo, como el motor Pluma. La luz de la
//!   ventana le da su lomo y su flanco, y al girar cambia de tono.
//! - El liston es una barra de seccion cuadrada; el rodillo, una banda
//!   tumbada en la hoja donde se dio (su `normal`); la cuchilla, una banda de
//!   pie; el lapiz (`plana`), una raya fina sin pulso.
//! - Lo que alumbra va sin sombrear, avivado y con un halo translucido.
//! - Las hojas son un velo con su pauta y su canto; los cuerpos, un velo con
//!   sus meridianos; las imagenes, una rejilla con el color de cada punto.
//! - Con sol (o luna), cada trazo tira su sombra en el suelo (`z = 0`).
//!
//! Lo que tiene que leerse sobre el fondo (velos, pautas, suelo) lleva la
//! marca [`CONTRASTE`]: el sombreador lo pinta negro en fondo claro y blanco
//! en oscuro, asi que cambiar el tema no obliga a rehacer nada.

use super::{
    Croquis, FormaDeBola, Imagen3D, Lamina3D, Pincel, Pt3, Trazo3D, color_de, escalar, largo,
    luminancia, mas, menos, normalizado, p3, por, producto,
};
use crate::modelo3d::{Elemento, Modelo3d, V3, color_a_u32, normal_a_u32};

/// El vertice no se sombrea: su color tal cual (imagenes, luces, sombras).
pub const SIN_LUZ: u32 = 1 << 29;
/// El vertice va del color contrario al fondo (negro o blanco), con su alfa.
pub const CONTRASTE: u32 = 1 << 30;
/// El vertice se sombrea con el tono de pluma: su color en el medio, hacia el
/// blanco en el lomo y hacia el negro en el flanco (`Croquis3DPluma.kt`).
pub const PLUMA: u32 = 1 << 31;

/// Una foto ya leida, en RGBA de 8 bits.
#[derive(Debug, Clone)]
pub struct Foto {
    pub ancho: u32,
    pub alto: u32,
    pub rgba: Vec<u8>,
}

// Las medidas de Android (Croquis3DLienzo.kt), en tanto por uno.
const VELO_DE_LA_HOJA: f32 = (0x0E as f32 + 0x1C as f32 * 0.5) / 255.0;
const TOPE_DEL_VELO: f32 = 0x5A as f32 / 255.0;
const PAUTA_DE_LA_HOJA: f32 = 0.16;
const ALTURAS_DE_LA_PAUTA: [f64; 3] = [0.25, 0.5, 0.75];
const MAXIMOS_DEL_PERFIL: usize = 48;
const ALFA_DEL_CANTO: f32 = 0.75;
const VELO_DE_LA_BOLA: f32 = 0x22 as f32 / 255.0;
const ALFA_DE_LA_REJILLA: f32 = 0x33 as f32 / 255.0;
const MERIDIANOS_QUE_SE_PINTAN: usize = 6;
const PARALELOS_QUE_SE_PINTAN: usize = 6;
const LO_QUE_YA_ES_CLARO: f32 = 0.62;
const ALFA_DEL_SUELO: f32 = 0.16;
const ALFA_DE_LOS_EJES: f32 = 0.45;
const SOMBRA_DEL_SOL: [f32; 4] = [0.0, 0.0, 0.0, 0.33];
const SOMBRA_DE_LA_LUNA: [f32; 4] = [
    0x2B as f32 / 255.0,
    0x3A as f32 / 255.0,
    0x67 as f32 / 255.0,
    0.20,
];
const RESPLANDOR: f64 = 2.6;
const ALFA_DEL_RESPLANDOR: f32 = 0x44 as f32 / 255.0;
const SENOS_DEL_DOMO: [f64; 3] = [0.5, 0.87, 1.0];
const COSENOS_DEL_DOMO: [f64; 3] = [0.87, 0.5, 1e-4];
/// Lados del tubo: 16 se ve redondo y no cuesta.
const LADOS_DEL_TUBO: usize = 16;
/// Lo mas fina que sale una foto en la rejilla (celdas por lado).
const CELDAS_DE_LA_FOTO: u32 = 160;
/// La tinta de fabrica de dia (`tintaDeFabrica`), por si un color no se entiende.
const TINTA: [f32; 4] = [
    0x1e as f32 / 255.0,
    0x1e as f32 / 255.0,
    0x1e as f32 / 255.0,
    1.0,
];

/// Lo que se va llenando: el modelo, con los vertices relativos a `origen`.
struct Obra {
    m: Modelo3d,
    origen: Option<[f64; 3]>,
    min: [f64; 3],
    max: [f64; 3],
}

impl Obra {
    fn v(&mut self, p: Pt3, n: Pt3, color: [f32; 4], marcas: u32, e: u32) -> u32 {
        let o = *self.origen.get_or_insert([p.x, p.y, p.z]);
        for (k, x) in [p.x, p.y, p.z].into_iter().enumerate() {
            self.min[k] = self.min[k].min(x);
            self.max[k] = self.max[k].max(x);
        }
        let normal = normal_a_u32([n.x as f32, n.y as f32, n.z as f32]) | marcas;
        self.m.vertices.push(V3 {
            pos: [
                (p.x - o[0]) as f32,
                (p.y - o[1]) as f32,
                (p.z - o[2]) as f32,
            ],
            normal,
            color: color_a_u32(color),
            elemento: e,
        });
        (self.m.vertices.len() - 1) as u32
    }

    fn tri(&mut self, a: u32, b: u32, c: u32, transparente: bool) {
        let lista = if transparente {
            &mut self.m.transparentes
        } else {
            &mut self.m.opacos
        };
        lista.extend([a, b, c]);
    }

    /// Un cuadrilatero plano (dos triangulos), con su normal.
    fn cuadro(&mut self, q: [Pt3; 4], color: [f32; 4], marcas: u32, e: u32, transparente: bool) {
        let n = normalizado(producto(menos(q[1], q[0]), menos(q[3], q[0])));
        let n = if largo(n) < 0.5 {
            normalizado(producto(menos(q[2], q[1]), menos(q[0], q[1])))
        } else {
            n
        };
        let i: Vec<u32> = q.iter().map(|p| self.v(*p, n, color, marcas, e)).collect();
        self.tri(i[0], i[1], i[2], transparente);
        self.tri(i[0], i[2], i[3], transparente);
    }

    fn raya(&mut self, puntos: &[Pt3], color: [f32; 4], marcas: u32, e: u32, cerrada: bool) {
        if puntos.len() < 2 {
            return;
        }
        let base = self.m.vertices.len() as u32;
        for p in puntos {
            self.v(*p, p3(0.0, 0.0, 1.0), color, marcas, e);
        }
        let n = puntos.len() as u32;
        for i in 0..n - 1 {
            self.m.lineas.extend([base + i, base + i + 1]);
        }
        if cerrada && n > 2 {
            self.m.lineas.extend([base + n - 1, base]);
        }
    }

    fn elemento(&mut self, tipo: &str, nombre: &str) -> u32 {
        self.m.elementos.push(Elemento {
            tipo: tipo.to_string(),
            nombre: nombre.to_string(),
            ..Default::default()
        });
        (self.m.elementos.len() - 1) as u32
    }
}

fn con_alfa(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], (c[3] * a).clamp(0.0, 1.0)]
}

fn hacia_el_blanco(c: [f32; 4], k: f32) -> [f32; 4] {
    [
        c[0] + (1.0 - c[0]) * k,
        c[1] + (1.0 - c[1]) * k,
        c[2] + (1.0 - c[2]) * k,
        c[3],
    ]
}

/// Quita los puntos repetidos seguidos (no dan tangente).
fn sin_repetidos(puntos: &[Pt3], minimo: f64) -> (Vec<Pt3>, Vec<usize>) {
    let mut v: Vec<Pt3> = Vec::with_capacity(puntos.len());
    let mut de: Vec<usize> = Vec::with_capacity(puntos.len());
    for (i, p) in puntos.iter().enumerate() {
        if !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()) {
            continue;
        }
        if v.last().is_none_or(|q| largo(menos(*p, *q)) > minimo) {
            v.push(*p);
            de.push(i);
        }
    }
    (v, de)
}

fn tangentes(p: &[Pt3]) -> Vec<Pt3> {
    let n = p.len();
    (0..n)
        .map(|i| {
            let a = p[i.saturating_sub(1)];
            let b = p[(i + 1).min(n - 1)];
            normalizado(menos(b, a))
        })
        .collect()
}

/// Una perpendicular cualquiera a `t`.
fn perpendicular(t: Pt3) -> Pt3 {
    let otra = if t.z.abs() < 0.9 {
        p3(0.0, 0.0, 1.0)
    } else {
        p3(1.0, 0.0, 0.0)
    };
    normalizado(producto(t, otra))
}

/// Los marcos del recorrido por transporte paralelo (el giro minimo de
/// `MarcosMinimos.kt`, en su forma corta): la normal de cada muestra es la
/// anterior quitandole lo que tenga de la tangente nueva.
fn marcos(t: &[Pt3], arranque: Option<Pt3>) -> Vec<Pt3> {
    let mut v = Vec::with_capacity(t.len());
    let mut n = arranque
        .map(|a| menos(a, por(t[0], escalar(a, t[0]))))
        .filter(|a| largo(*a) > 1e-6)
        .map(normalizado)
        .unwrap_or_else(|| perpendicular(t[0]));
    for ti in t {
        let m = menos(n, por(*ti, escalar(n, *ti)));
        n = if largo(m) > 1e-6 {
            normalizado(m)
        } else {
            perpendicular(*ti)
        };
        v.push(n);
    }
    v
}

/// **Un tubo**: la seccion de `lados` barrida por `p`, de radio `r[i]`.
/// Redondo, con normales suaves; de cuatro lados, con caras planas y tapas.
#[allow(clippy::too_many_arguments)]
fn tubo(
    o: &mut Obra,
    p: &[Pt3],
    r: &[f64],
    lados: usize,
    arranque: Option<Pt3>,
    color: [f32; 4],
    marcas: u32,
    e: u32,
    transparente: bool,
) {
    let n = p.len();
    if n < 2 {
        return;
    }
    let t = tangentes(p);
    let nn = marcos(&t, arranque);
    let giro = if lados == 4 {
        std::f64::consts::FRAC_PI_4
    } else {
        0.0
    };
    let dir = |i: usize, k: usize| {
        let a = giro + std::f64::consts::TAU * k as f64 / lados as f64;
        let b = producto(t[i], nn[i]);
        mas(por(nn[i], a.cos()), por(b, a.sin()))
    };
    if lados == 4 {
        // Caras planas: la mitad del lado es el radio (las esquinas, a √2).
        let esquina = std::f64::consts::SQRT_2;
        for k in 0..4 {
            let mut previos: Option<(u32, u32)> = None;
            for i in 0..n {
                let normal = normalizado(mas(dir(i, k), dir(i, k + 1)));
                let a = o.v(
                    mas(p[i], por(dir(i, k), r[i] * esquina)),
                    normal,
                    color,
                    marcas,
                    e,
                );
                let b = o.v(
                    mas(p[i], por(dir(i, k + 1), r[i] * esquina)),
                    normal,
                    color,
                    marcas,
                    e,
                );
                if let Some((a0, b0)) = previos {
                    o.tri(a0, a, b, transparente);
                    o.tri(a0, b, b0, transparente);
                }
                previos = Some((a, b));
            }
        }
        // Las tapas de los dos extremos.
        for (i, signo) in [(0usize, -1.0), (n - 1, 1.0)] {
            let normal = por(t[i], signo);
            let c: Vec<u32> = (0..4)
                .map(|k| {
                    o.v(
                        mas(p[i], por(dir(i, k), r[i] * esquina)),
                        normal,
                        color,
                        marcas,
                        e,
                    )
                })
                .collect();
            o.tri(c[0], c[1], c[2], transparente);
            o.tri(c[0], c[2], c[3], transparente);
        }
        return;
    }
    let mut anillo_previo: Option<u32> = None;
    for i in 0..n {
        let base = o.m.vertices.len() as u32;
        for k in 0..lados {
            let d = dir(i, k);
            o.v(mas(p[i], por(d, r[i])), d, color, marcas, e);
        }
        if let Some(b0) = anillo_previo {
            for k in 0..lados as u32 {
                let k1 = (k + 1) % lados as u32;
                o.tri(b0 + k, base + k, base + k1, transparente);
                o.tri(b0 + k, base + k1, b0 + k1, transparente);
            }
        }
        anillo_previo = Some(base);
    }
}

/// `conDomos`: tres anillos mas en cada punta, con el ancho cayendo en
/// coseno hasta cero, para que la redonda acabe en domo y no en disco.
fn con_domos(p: &[Pt3], r: &[f64]) -> (Vec<Pt3>, Vec<f64>) {
    let n = p.len();
    if n < 2 {
        return (p.to_vec(), r.to_vec());
    }
    let t0 = normalizado(menos(p[1], p[0]));
    let tn = normalizado(menos(p[n - 1], p[n - 2]));
    let mut pp = Vec::with_capacity(n + 6);
    let mut rr = Vec::with_capacity(n + 6);
    for d in (0..3).rev() {
        pp.push(menos(p[0], por(t0, r[0] * SENOS_DEL_DOMO[d])));
        rr.push(r[0] * COSENOS_DEL_DOMO[d]);
    }
    pp.extend_from_slice(p);
    rr.extend_from_slice(r);
    for d in 0..3 {
        pp.push(mas(p[n - 1], por(tn, r[n - 1] * SENOS_DEL_DOMO[d])));
        rr.push(r[n - 1] * COSENOS_DEL_DOMO[d]);
    }
    (pp, rr)
}

/// Una bolita (un trazo de un solo punto).
fn bolita(o: &mut Obra, c: Pt3, r: f64, color: [f32; 4], marcas: u32, e: u32, transparente: bool) {
    let (pa, me) = (6usize, 10usize);
    let base = o.m.vertices.len() as u32;
    for j in 0..=pa {
        let s = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * j as f64 / pa as f64;
        for i in 0..me {
            let v = std::f64::consts::TAU * i as f64 / me as f64;
            let d = p3(s.cos() * v.cos(), s.cos() * v.sin(), s.sin());
            o.v(mas(c, por(d, r)), d, color, marcas, e);
        }
    }
    for j in 0..pa as u32 {
        for i in 0..me as u32 {
            let i1 = (i + 1) % me as u32;
            let (a, b, cc, d) = (
                base + j * me as u32 + i,
                base + j * me as u32 + i1,
                base + (j + 1) * me as u32 + i1,
                base + (j + 1) * me as u32 + i,
            );
            o.tri(a, b, cc, transparente);
            o.tri(a, cc, d, transparente);
        }
    }
}

/// Una banda entre dos rieles (rodillo, cuchilla, sombra): de dos caras.
fn banda(
    o: &mut Obra,
    a: &[Pt3],
    b: &[Pt3],
    color: [f32; 4],
    marcas: u32,
    e: u32,
    transparente: bool,
) {
    for i in 1..a.len().min(b.len()) {
        o.cuadro(
            [a[i - 1], a[i], b[i], b[i - 1]],
            color,
            marcas,
            e,
            transparente,
        );
    }
}

/// La normal de la hoja en cada punto del trazo, si se sabe.
fn normales_del_trazo(t: &Trazo3D, de: &[usize]) -> Option<Vec<Pt3>> {
    if let Some(v) = t.normales.as_ref().filter(|v| v.len() == t.puntos.len()) {
        return Some(de.iter().map(|&i| normalizado(v[i])).collect());
    }
    t.normal.map(|n| vec![normalizado(n); de.len()])
}

/// Lo que mide la escena (para el suelo, las hojas sin fin y la sombra).
fn tamano(c: &Croquis) -> (Pt3, f64) {
    let pts = c.puntos();
    if pts.is_empty() {
        return (Pt3::default(), 100.0);
    }
    let mut mn = [f64::MAX; 3];
    let mut mx = [f64::MIN; 3];
    for p in &pts {
        for (k, x) in [p.x, p.y, p.z].into_iter().enumerate() {
            mn[k] = mn[k].min(x);
            mx[k] = mx[k].max(x);
        }
    }
    let centro = p3(
        (mn[0] + mx[0]) / 2.0,
        (mn[1] + mx[1]) / 2.0,
        (mn[2] + mx[2]) / 2.0,
    );
    let lado = (mx[0] - mn[0])
        .max(mx[1] - mn[1])
        .max(mx[2] - mn[2])
        .max(1.0);
    (centro, lado)
}

fn trazo(
    o: &mut Obra,
    c: &Croquis,
    t: &Trazo3D,
    sombras: &[([f32; 4], super::Sol3D)],
    escena: f64,
) {
    let ancho = t.ancho_del_mundo();
    let (p, de) = sin_repetidos(&t.puntos, ancho * 1e-4);
    if p.is_empty() {
        return;
    }
    let base = color_de(&t.color).unwrap_or(TINTA);
    let vigente = t.pincel.vigente();
    let nombre = c.nombre_de_grupo(t.grupo.as_deref()).unwrap_or("");
    let tipo = if t.punta.plana {
        "Lápiz".to_string()
    } else {
        super::mayuscula(t.pincel.como_se_llama())
    };
    let e = o.elemento(&tipo, nombre);
    // Lo que alumbra: encendido va sin sombrear y avivado; apagado, del
    // color del cristal (o el suyo mas apagado).
    let cuanta_luz = if t.alumbra() {
        (c.luces.cuanto() * t.luz.clamp(0.0, 1.0)) as f32
    } else {
        0.0
    };
    let (color, marcas) = if cuanta_luz > 0.01 {
        (hacia_el_blanco(base, 0.35 * cuanta_luz.min(1.0)), SIN_LUZ)
    } else if t.alumbra() {
        let apagada = t.color_apagada.as_deref().and_then(color_de).unwrap_or([
            base[0] * 0.6,
            base[1] * 0.6,
            base[2] * 0.6,
            base[3],
        ]);
        (apagada, PLUMA)
    } else {
        (base, PLUMA)
    };
    let color = con_alfa(color, t.opacidad.clamp(0.0, 1.0) as f32);
    let transparente = color[3] < 0.98;
    let radio = ancho / 2.0;
    if p.len() == 1 {
        bolita(
            o,
            p[0],
            radio * t.punta.ancho.clamp(0.2, 4.0),
            color,
            marcas,
            e,
            transparente,
        );
    } else {
        let normales = normales_del_trazo(t, &de);
        let tg = tangentes(&p);
        match vigente {
            _ if t.punta.plana => {
                let (pp, rr) = con_domos(&p, &vec![radio.min(ancho * 0.5); p.len()]);
                tubo(o, &pp, &rr, 6, None, color, marcas, e, transparente);
            }
            Pincel::Resaltador | Pincel::Cuchilla => {
                // La banda: tumbada en la hoja (rodillo) o de pie (cuchilla).
                let lado = lados_de_la_banda(&tg, normales.as_deref(), vigente == Pincel::Cuchilla);
                let (a, b): (Vec<Pt3>, Vec<Pt3>) = if vigente == Pincel::Cuchilla {
                    (
                        p.clone(),
                        p.iter()
                            .zip(&lado)
                            .map(|(q, s)| mas(*q, por(*s, ancho)))
                            .collect(),
                    )
                } else {
                    (
                        p.iter()
                            .zip(&lado)
                            .map(|(q, s)| menos(*q, por(*s, radio)))
                            .collect(),
                        p.iter()
                            .zip(&lado)
                            .map(|(q, s)| mas(*q, por(*s, radio)))
                            .collect(),
                    )
                };
                banda(o, &a, &b, color, marcas, e, transparente);
            }
            Pincel::Cuadrado => {
                let arranque = t.normal.or(t.eje_de_la_punta);
                tubo(
                    o,
                    &p,
                    &vec![radio; p.len()],
                    4,
                    arranque,
                    color,
                    marcas,
                    e,
                    transparente,
                );
            }
            _ => {
                // La redonda (y la luz): el pulso de la mano, si lo trae.
                let mut recorrido = vec![0.0; p.len()];
                for i in 1..p.len() {
                    recorrido[i] = recorrido[i - 1] + largo(menos(p[i], p[i - 1]));
                }
                let presiones: Option<Vec<f64>> = t
                    .presiones
                    .as_ref()
                    .filter(|v| v.len() == t.puntos.len())
                    .map(|v| de.iter().map(|&i| v[i]).collect());
                let tiempos: Option<Vec<f64>> = t
                    .tiempos
                    .as_ref()
                    .filter(|v| v.len() == t.puntos.len())
                    .map(|v| de.iter().map(|&i| v[i]).collect());
                let anchos = if vigente == Pincel::Redondo
                    && t.punta.pulso
                    && (presiones.is_some() || tiempos.is_some())
                {
                    super::anchos_del_mundo(
                        ancho,
                        presiones.as_deref(),
                        &recorrido,
                        tiempos.as_deref(),
                        true,
                        true,
                    )
                } else {
                    vec![ancho; p.len()]
                };
                let del_ancho = t.punta.ancho.clamp(0.2, 4.0);
                let radios: Vec<f64> = anchos.iter().map(|a| a * del_ancho / 2.0).collect();
                let (pp, rr) = con_domos(&p, &radios);
                tubo(
                    o,
                    &pp,
                    &rr,
                    LADOS_DEL_TUBO,
                    t.eje_de_la_punta,
                    color,
                    marcas,
                    e,
                    transparente,
                );
                if cuanta_luz > 0.01 {
                    // El halo: un tubo mas gordo y translucido alrededor.
                    let halo: Vec<f64> = rr.iter().map(|r| r * RESPLANDOR).collect();
                    let tinta = con_alfa(base, ALFA_DEL_RESPLANDOR * cuanta_luz.min(1.0));
                    tubo(
                        o,
                        &pp,
                        &halo,
                        LADOS_DEL_TUBO,
                        t.eje_de_la_punta,
                        tinta,
                        SIN_LUZ,
                        e,
                        true,
                    );
                }
            }
        }
    }
    // La sombra en el suelo: una banda tumbada, del ancho del trazo.
    for (tinta, sol) in sombras {
        let s: Vec<Pt3> = p
            .iter()
            .filter_map(|q| sol.sombra_de(*q))
            .map(|q| p3(q.x, q.y, escena * 1e-4))
            .collect();
        if s.len() < 2 {
            continue;
        }
        let tg = tangentes(&s);
        let lado: Vec<Pt3> = tg
            .iter()
            .map(|t| normalizado(producto(*t, p3(0.0, 0.0, 1.0))))
            .collect();
        let a: Vec<Pt3> = s
            .iter()
            .zip(&lado)
            .map(|(q, l)| menos(*q, por(*l, radio)))
            .collect();
        let b: Vec<Pt3> = s
            .iter()
            .zip(&lado)
            .map(|(q, l)| mas(*q, por(*l, radio)))
            .collect();
        banda(o, &a, &b, *tinta, SIN_LUZ, e, true);
    }
}

/// `laManchaDeLaHoja`: el color de la hoja, salvo que se pierda contra el
/// fondo; entonces el de contraste. Aqui no se sabe el fondo al cocer, asi
/// que los muy claros y los muy oscuros van de contraste (el sombreador los
/// pone del lado bueno) y los de color, con el suyo.
fn mancha(color: &str) -> ([f32; 4], u32) {
    match color_de(color) {
        Some(c) if (0.2..LO_QUE_YA_ES_CLARO).contains(&luminancia(c)) => (c, SIN_LUZ),
        _ => ([0.0, 0.0, 0.0, 1.0], SIN_LUZ | CONTRASTE),
    }
}

fn hoja(o: &mut Obra, c: &Croquis, l: &Lamina3D, escena: f64) {
    let opacidad = l.opacidad.clamp(0.0, 3.0) as f32;
    if let Some(bola) = &l.esfera {
        let tipo = match bola.forma {
            FormaDeBola::Cilindro => "Cilindro",
            FormaDeBola::Cono => "Cono",
            FormaDeBola::Anillo => "Anillo",
            _ => "Bola",
        };
        let e = o.elemento(tipo, "");
        let velo = [
            0.0,
            0.0,
            0.0,
            (VELO_DE_LA_BOLA * opacidad).min(0xC0 as f32 / 255.0),
        ];
        for q in bola.malla() {
            o.cuadro(q, velo, SIN_LUZ | CONTRASTE, e, true);
        }
        let rejilla = [
            0.0,
            0.0,
            0.0,
            (ALFA_DE_LA_REJILLA * opacidad).clamp(0.0, 0.6),
        ];
        let pasos = super::MERIDIANOS;
        let pi = std::f64::consts::PI;
        let meridianos = if bola.forma == FormaDeBola::Bola {
            MERIDIANOS_QUE_SE_PINTAN
        } else {
            2 * MERIDIANOS_QUE_SE_PINTAN
        };
        for m in 0..meridianos {
            let vuelta = pi * m as f64 / MERIDIANOS_QUE_SE_PINTAN as f64;
            let mut aro: Vec<Pt3> = (0..=pasos)
                .map(|i| bola.punto(vuelta, -pi / 2.0 + pi * i as f64 / pasos as f64))
                .collect();
            if bola.forma == FormaDeBola::Bola {
                aro.extend(
                    (0..=pasos)
                        .map(|i| bola.punto(vuelta + pi, pi / 2.0 - pi * i as f64 / pasos as f64)),
                );
            }
            o.raya(&aro, rejilla, SIN_LUZ | CONTRASTE, e, false);
        }
        let (desde, hasta) = if bola.forma == FormaDeBola::Bola {
            (1, PARALELOS_QUE_SE_PINTAN - 1)
        } else {
            (0, PARALELOS_QUE_SE_PINTAN)
        };
        for q in desde..=hasta {
            let subida = -pi / 2.0 + pi * q as f64 / PARALELOS_QUE_SE_PINTAN as f64;
            let aro: Vec<Pt3> = (0..=pasos)
                .map(|i| bola.punto(2.0 * pi * i as f64 / pasos as f64, subida))
                .collect();
            o.raya(&aro, rejilla, SIN_LUZ | CONTRASTE, e, false);
        }
        return;
    }
    if l.perfil.len() < 2 {
        return;
    }
    let e = o.elemento("Hoja", "");
    // Una hoja sin fin a lo hondo se pinta hasta donde llega lo dibujado.
    let fondo = if l.infinita && l.otro_perfil.is_none() {
        l.fondo.max(escena * 0.75)
    } else {
        l.fondo
    };
    let (color, marcas) = mancha(&l.color);
    let velo = [
        color[0],
        color[1],
        color[2],
        (VELO_DE_LA_HOJA * opacidad).min(TOPE_DEL_VELO),
    ];
    for q in l.tiras_con(fondo) {
        o.cuadro(q, velo, marcas, e, true);
    }
    let n = l.perfil.len();
    if c.pauta_de_las_hojas {
        let pauta = [0.0, 0.0, 0.0, (PAUTA_DE_LA_HOJA * opacidad).clamp(0.0, 1.0)];
        let paso = (n / MAXIMOS_DEL_PERFIL).max(1);
        let mut i = 0;
        while i < n {
            let (a, b) = l.lados(i, fondo);
            o.raya(&[a, b], pauta, SIN_LUZ | CONTRASTE, e, false);
            i += paso;
        }
        for t in ALTURAS_DE_LA_PAUTA {
            let raya: Vec<Pt3> = (0..n)
                .step_by(paso)
                .map(|k| {
                    let (a, b) = l.lados(k, fondo);
                    mas(a, por(menos(b, a), t))
                })
                .collect();
            o.raya(&raya, pauta, SIN_LUZ | CONTRASTE, e, false);
        }
    }
    if l.infinita {
        let canto = [0.0, 0.0, 0.0, (ALFA_DEL_CANTO * opacidad).clamp(0.0, 1.0)];
        for i in [0, n - 1] {
            let (a, b) = l.lados(i, fondo);
            o.raya(&[a, b], canto, SIN_LUZ | CONTRASTE, e, false);
        }
    } else {
        o.raya(
            &l.contorno(fondo),
            con_alfa(color, opacidad.min(1.0)),
            marcas,
            e,
            true,
        );
    }
}

fn imagen(
    o: &mut Obra,
    c: &Croquis,
    im: &Imagen3D,
    foto: Option<&Foto>,
    sombras: &[([f32; 4], super::Sol3D)],
    escena: f64,
) {
    if im.esquinas.len() < 4 {
        return;
    }
    let e = o.elemento(
        "Imagen",
        c.nombre_de_grupo(im.grupo.as_deref()).unwrap_or(""),
    );
    let q = [
        im.esquinas[0],
        im.esquinas[1],
        im.esquinas[2],
        im.esquinas[3],
    ];
    let opacidad = im.opacidad.clamp(0.0, 1.0) as f32;
    for (tinta, sol) in sombras {
        let s: Vec<Pt3> = q
            .iter()
            .filter_map(|p| sol.sombra_de(*p))
            .map(|p| p3(p.x, p.y, escena * 1e-4))
            .collect();
        if s.len() == 4 {
            o.raya(&s, *tinta, SIN_LUZ, e, true);
        }
    }
    let Some(f) = foto
        .filter(|f| f.ancho > 0 && f.alto > 0 && f.rgba.len() >= (f.ancho * f.alto * 4) as usize)
    else {
        // Sin la foto en este equipo: su marco y sus diagonales, para que se
        // sepa que ahi hay una imagen.
        let gris = [0.0, 0.0, 0.0, 0.5 * opacidad.max(0.3)];
        o.raya(&q, gris, SIN_LUZ | CONTRASTE, e, true);
        o.raya(&[q[0], q[2]], gris, SIN_LUZ | CONTRASTE, e, false);
        o.raya(&[q[1], q[3]], gris, SIN_LUZ | CONTRASTE, e, false);
        return;
    };
    // La rejilla: cada vertice con el color de su punto de la foto; la
    // tarjeta interpola entre ellos.
    let (gw, gh) = (
        f.ancho.min(CELDAS_DE_LA_FOTO),
        f.alto.min(CELDAS_DE_LA_FOTO),
    );
    let normal = normalizado(producto(menos(q[1], q[0]), menos(q[3], q[0])));
    let base = o.m.vertices.len() as u32;
    let mut translucida = opacidad < 0.98;
    for j in 0..=gh {
        let v = j as f64 / gh as f64;
        for i in 0..=gw {
            let u = i as f64 / gw as f64;
            let arriba = mas(q[0], por(menos(q[1], q[0]), u));
            let abajo = mas(q[3], por(menos(q[2], q[3]), u));
            let p = mas(arriba, por(menos(abajo, arriba), v));
            let (x, y) = (
                (u * (f.ancho - 1) as f64).round() as u32,
                (v * (f.alto - 1) as f64).round() as u32,
            );
            let k = ((y * f.ancho + x) * 4) as usize;
            let px = &f.rgba[k..k + 4];
            let a = px[3] as f32 / 255.0 * opacidad;
            translucida |= a < 0.98;
            o.v(
                p,
                normal,
                [
                    px[0] as f32 / 255.0,
                    px[1] as f32 / 255.0,
                    px[2] as f32 / 255.0,
                    a,
                ],
                SIN_LUZ,
                e,
            );
        }
    }
    let fila = gw + 1;
    for j in 0..gh {
        for i in 0..gw {
            let a = base + j * fila + i;
            o.tri(a, a + 1, a + fila + 1, translucida);
            o.tri(a, a + fila + 1, a + fila, translucida);
        }
    }
}

/// El suelo: la reticula alrededor de lo dibujado y los dos ejes del mundo.
fn suelo(o: &mut Obra, c: &Croquis, centro: Pt3, escena: f64) {
    let e = o.elemento("Suelo", "");
    let alcance = escena * 0.5 * 1.25;
    let lado = 10f64.powf((alcance / 4.0).max(1e-9).log10().floor());
    let (x0, x1) = (centro.x - alcance, centro.x + alcance);
    let (y0, y1) = (centro.y - alcance, centro.y + alcance);
    if c.pauta_de_las_hojas {
        let tinta = [0.0, 0.0, 0.0, ALFA_DEL_SUELO];
        let mut i = (x0 / lado).ceil() as i64;
        while (i as f64) * lado <= x1 && i < (x0 / lado).ceil() as i64 + 400 {
            let x = i as f64 * lado;
            if i != 0 {
                o.raya(
                    &[p3(x, y0, 0.0), p3(x, y1, 0.0)],
                    tinta,
                    SIN_LUZ | CONTRASTE,
                    e,
                    false,
                );
            }
            i += 1;
        }
        let mut j = (y0 / lado).ceil() as i64;
        while (j as f64) * lado <= y1 && j < (y0 / lado).ceil() as i64 + 400 {
            let y = j as f64 * lado;
            if j != 0 {
                o.raya(
                    &[p3(x0, y, 0.0), p3(x1, y, 0.0)],
                    tinta,
                    SIN_LUZ | CONTRASTE,
                    e,
                    false,
                );
            }
            j += 1;
        }
    }
    // Los ejes se quedan siempre: son el cero del mundo.
    let ejes = [0.0, 0.0, 0.0, ALFA_DE_LOS_EJES];
    if (x0..=x1).contains(&0.0) {
        o.raya(
            &[p3(0.0, y0, 0.0), p3(0.0, y1, 0.0)],
            ejes,
            SIN_LUZ | CONTRASTE,
            e,
            false,
        );
    }
    if (y0..=y1).contains(&0.0) {
        o.raya(
            &[p3(x0, 0.0, 0.0), p3(x1, 0.0, 0.0)],
            ejes,
            SIN_LUZ | CONTRASTE,
            e,
            false,
        );
    }
}

/// **El croquis en triangulos y rayas**, listo para [`crate::ventana3d`].
/// `fotos` da la foto de una ruta de [`Imagen3D`] (la resuelve quien abre:
/// aqui no se sabe donde vive cada `pixpin:files/…`).
pub fn a_modelo(c: &Croquis, fotos: &dyn Fn(&str) -> Option<Foto>) -> Modelo3d {
    let mut o = Obra {
        m: Modelo3d::default(),
        origen: None,
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    let (centro, escena) = tamano(c);
    // El origen, en el centro de lo dibujado: los milimetros no se pierden en f32.
    o.origen = Some([centro.x, centro.y, centro.z]);
    let mut sombras = Vec::new();
    if let Some(s) = c.sol {
        sombras.push((SOMBRA_DEL_SOL, s));
    }
    if let Some(s) = c.luna {
        sombras.push((SOMBRA_DE_LA_LUNA, s));
    }
    if !c.vacio() {
        suelo(&mut o, c, centro, escena);
    }
    for l in &c.laminas {
        hoja(&mut o, c, l, escena);
    }
    for im in c.imagenes.iter().filter(|i| !i.oculto) {
        let f = fotos(&im.ruta);
        imagen(&mut o, c, im, f.as_ref(), &sombras, escena);
    }
    for t in c.trazos.iter().filter(|t| !t.oculto) {
        trazo(&mut o, c, t, &sombras, escena);
    }
    let org = o.origen.unwrap_or([0.0; 3]);
    o.m.origen = org;
    if o.min[0].is_finite() {
        o.m.caja = [
            (o.min[0] - org[0]) as f32,
            (o.min[1] - org[1]) as f32,
            (o.min[2] - org[2]) as f32,
            (o.max[0] - org[0]) as f32,
            (o.max[1] - org[1]) as f32,
            (o.max[2] - org[2]) as f32,
        ];
    }
    // Sin unidades: el croquis mide en `dp` del movil, no en metros.
    o.m.metros = 0.0;
    o.m
}

/// **Para encuadrar**: la caja de lo dibujado (sin el suelo ni las hojas sin
/// fin, que son mas grandes que el dibujo), relativa al origen del modelo.
pub fn caja_de_lo_dibujado(c: &Croquis, origen: [f64; 3]) -> Option<[f32; 6]> {
    let pts = c.puntos();
    if pts.is_empty() {
        return None;
    }
    let mut caja = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
    for p in pts {
        for (k, x) in [p.x - origen[0], p.y - origen[1], p.z - origen[2]]
            .into_iter()
            .enumerate()
        {
            caja[k] = caja[k].min(x as f32);
            caja[k + 3] = caja[k + 3].max(x as f32);
        }
    }
    Some(caja)
}

/// El fondo del croquis, si eligio uno (`colorDelFondo`); nulo es el del tema.
pub fn fondo_propio(c: &Croquis) -> Option<[f32; 4]> {
    c.color_del_fondo.as_deref().and_then(color_de)
}

/// **Con que tema se abre**: el del fondo propio si lo hay; si no, el que
/// deja ver la tinta (un croquis hecho de dia trae tinta casi negra, y en
/// fondo oscuro no se veria nada).
pub fn abre_claro(c: &Croquis) -> bool {
    if let Some(f) = fondo_propio(c) {
        return luminancia(f) > 0.4;
    }
    let tintas: Vec<f32> = c
        .trazos
        .iter()
        .filter(|t| !t.oculto)
        .filter_map(|t| color_de(&t.color))
        .map(luminancia)
        .collect();
    if tintas.is_empty() {
        return true;
    }
    tintas.iter().sum::<f32>() / (tintas.len() as f32) < 0.5
}

/// **El lado de la banda en cada muestra** (rodillo: `normal × tangente`;
/// cuchilla: la normal). Donde el trazo va casi a lo largo de la normal —un
/// salto de una hoja a otra, una hoja vista de canto— ese producto se queda
/// en nada y su direccion baila: la banda se retorcia en un pico. Ahi se
/// sigue con el lado de la muestra anterior, y nunca se da la vuelta de
/// golpe (un lado al reves pliega la banda sobre si misma).
pub(crate) fn lados_de_la_banda(tg: &[Pt3], normales: Option<&[Pt3]>, de_pie: bool) -> Vec<Pt3> {
    let mut v: Vec<Pt3> = Vec::with_capacity(tg.len());
    #[allow(clippy::needless_range_loop)] // `i` indexa tambien las normales
    for i in 0..tg.len() {
        let n = normales
            .and_then(|v| v.get(i).copied())
            .unwrap_or_else(|| perpendicular(tg[i]));
        let crudo = if de_pie { n } else { producto(n, tg[i]) };
        let previo = v.last().copied();
        let mut lado = match previo {
            Some(p) if largo(crudo) < 0.3 => p,
            _ if largo(crudo) < 1e-9 => perpendicular(tg[i]),
            _ => normalizado(crudo),
        };
        if let Some(p) = previo
            && escalar(lado, p) < 0.0
        {
            lado = por(lado, -1.0);
        }
        v.push(lado);
    }
    v
}
