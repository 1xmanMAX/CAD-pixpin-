//! **La ventana de un modelo 3D (BIM), como un pin**: la misma que la de los
//! planos (sin bordes, nace sin fijar y su pin la deja encima, la barra fuera que aparece y se
//! va), con lo que pide un modelo: girarlo, acercarse y cortarlo. Lo pidio el
//! usuario el 9-oct-2026: «un visor sencillo como un pin, que se pueda ver
//! facilmente cualquier modelo».
//!
//! - Arrastrar (izquierdo): girar alrededor del punto que se toco.
//! - Arrastrar (derecho o central), o Mayus + izquierdo: mover.
//! - Rueda: acercar y alejar hacia el cursor.
//! - Clic: el elemento tocado se resalta y se nombra (Muro · nombre).
//! - Otro clic (o doble clic) en el elegido: aislarlo, solo el, encuadrado;
//!   Esc o doble clic en el vacio vuelven a todo el modelo. Tambien la I.
//! - Doble clic, F o Inicio: el modelo entero.
//! - Barra (como la de los demas visores): asa, el tipo y el nombre del
//!   archivo, caja de seccion (la de Revit: el modelo dentro de una
//!   caja con un tirador en cada cara; arrastrandolos se corta por donde se
//!   quiera, y lo cortado se ve macizo), niveles, tema, pantalla completa,
//!   el pin y cerrar.
//! - Niveles y categorias (boton de la barra o L): un panel para mostrar u
//!   ocultar plantas y categorias; lo elegido dice su nivel, su volumen y su
//!   area (sacados de su malla).
//! - Teclas: Esc (deja los marcos o anotar, suelta lo elegido; sin nada,
//!   cierra), B tema, C caja, E aristas, T encima, P planta, flechas para
//!   girar; F11 pantalla completa (y Esc, la primera vez, sale de ella). Las
//!   de todos los visores: I los marcos, Supr el ultimo, Ctrl+P imprimirlos,
//!   M o Ctrl+M un marcador, A anotar.

mod arbol;

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, VK_CONTROL, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use crate::convertir::rgba;
use crate::gpu::{Gpu, PlanoGpu};
use crate::gpu3d::{ModeloGpu, Vista3d};
use crate::modelo::{Constructor, Modelo};
use crate::modelo3d::Modelo3d;
use crate::texto::Textos;
use crate::ventana::{
    ALTO_BARRA, Aspecto, CAPA_ENCIMA, Cabecera, De, ESTADO, Ev, TipoDeArchivo,
    alternar_pantalla_completa, ancho_barra, aviso, barra, botones_barra, colocar_barra,
    crear_barra, crear_ventana, poner_encima, raya, rect, riel, riel_cajas, riel_en, seguir_barra,
    tecla_de_pantalla_completa, vista_pantalla,
};
use crate::ventana::{
    Globo, NombresDeBotones, Unificada, ficha_en, globo_de_la_barra, globo_izquierda,
    pintar_fichas_y_pista, tecla_unificada,
};
use arbol::Arbol;

const INFINITE: u32 = u32::MAX;
/// La barra, en el orden de la del plano (y la de los demas visores).
pub(crate) const BOTONES: [&str; 8] = [
    "asa", "nombre", "corte", "capas", "tema", "pantalla", "fijar", "cerrar",
];
pub(crate) const FOV: f64 = std::f64::consts::FRAC_PI_4; // 45 grados

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn suma(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn por(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn punto(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cruz(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = punto(a, a).sqrt().max(1e-30);
    por(a, 1.0 / l)
}

/// La camara: mira a `objetivo` desde `dist`, girada `rumbo` (alrededor de
/// Z) y `altura` (sobre el horizonte), en radianes. Z es arriba.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbita {
    pub objetivo: [f64; 3],
    pub rumbo: f64,
    pub altura: f64,
    pub dist: f64,
}

pub(crate) const ALTURA_MAX: f64 = 1.5697; // casi 90 grados: la planta

impl Orbita {
    /// Desde el ojo hacia el objetivo, unitario.
    fn adelante(&self) -> [f64; 3] {
        let (c, s) = (self.altura.cos(), self.altura.sin());
        [-c * self.rumbo.cos(), -c * self.rumbo.sin(), -s]
    }

    pub fn ojo(&self) -> [f64; 3] {
        sub(self.objetivo, por(self.adelante(), self.dist))
    }

    /// Derecha, arriba y adelante de la pantalla.
    fn ejes(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let f = self.adelante();
        let mut r = cruz(f, [0.0, 0.0, 1.0]);
        if punto(r, r) < 1e-12 {
            r = [-self.rumbo.sin(), self.rumbo.cos(), 0.0];
        }
        let r = unit(r);
        let u = cruz(r, f);
        (r, u, f)
    }

    /// El modelo entero (su caja) en la ventana, visto desde el sureste y
    /// algo de arriba.
    pub fn encuadrar(caja: [f32; 6], ancho: u32, alto: u32) -> Orbita {
        let c = [
            (caja[0] + caja[3]) as f64 / 2.0,
            (caja[1] + caja[4]) as f64 / 2.0,
            (caja[2] + caja[5]) as f64 / 2.0,
        ];
        let d = [
            (caja[3] - caja[0]) as f64,
            (caja[4] - caja[1]) as f64,
            (caja[5] - caja[2]) as f64,
        ];
        let mut o = Orbita {
            objetivo: c,
            rumbo: -0.8,
            altura: 0.5,
            dist: 1.0,
        };
        o.dist = o
            .distancia_para(caja, ancho, alto)
            .max(punto(d, d).sqrt() * 1e-3)
            .max(1e-3);
        o
    }

    /// Lo que hay que alejarse para que las 8 esquinas de la caja quepan.
    fn distancia_para(&self, caja: [f32; 6], ancho: u32, alto: u32) -> f64 {
        let (r, u, f) = self.ejes();
        let ty = (FOV / 2.0).tan() * 0.94;
        let tx = ty * ancho.max(1) as f64 / alto.max(1) as f64;
        let mut d: f64 = 0.0;
        for i in 0..8 {
            let p = [
                caja[if i & 1 == 0 { 0 } else { 3 }] as f64,
                caja[if i & 2 == 0 { 1 } else { 4 }] as f64,
                caja[if i & 4 == 0 { 2 } else { 5 }] as f64,
            ];
            let q = sub(p, self.objetivo);
            let z = punto(q, f);
            d = d
                .max(punto(q, r).abs() / tx - z)
                .max(punto(q, u).abs() / ty - z);
        }
        d
    }

    /// De mundo (relativo al origen del modelo) a recorte, en filas (p * M).
    pub fn matriz(&self, ancho: u32, alto: u32, radio: f64) -> [[f32; 4]; 4] {
        let (r, u, f) = self.ejes();
        let e = self.ojo();
        let cerca = (self.dist * 0.01).max(radio * 1e-5).max(1e-4);
        let lejos = self.dist + radio * 4.0 + 1.0;
        let ys = 1.0 / (FOV / 2.0).tan();
        let xs = ys * alto.max(1) as f64 / ancho.max(1) as f64;
        let q = lejos / (lejos - cerca);
        // Vista (filas): x = r·(p-e), y = u·(p-e), z = f·(p-e).
        let v = [
            [r[0], u[0], f[0], 0.0],
            [r[1], u[1], f[1], 0.0],
            [r[2], u[2], f[2], 0.0],
            [-punto(r, e), -punto(u, e), -punto(f, e), 1.0],
        ];
        let p = [
            [xs, 0.0, 0.0, 0.0],
            [0.0, ys, 0.0, 0.0],
            [0.0, 0.0, q, 1.0],
            [0.0, 0.0, -cerca * q, 0.0],
        ];
        let mut m = [[0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                m[i][j] = (0..4).map(|k| v[i][k] * p[k][j]).sum::<f64>() as f32;
            }
        }
        m
    }

    /// La direccion (unitaria) del rayo que sale del ojo por el pixel (x, y).
    pub fn rayo(&self, x: f64, y: f64, ancho: u32, alto: u32) -> [f64; 3] {
        let (r, u, f) = self.ejes();
        let t = (FOV / 2.0).tan();
        let nx =
            (x / ancho.max(1) as f64 * 2.0 - 1.0) * t * ancho.max(1) as f64 / alto.max(1) as f64;
        let ny = (1.0 - y / alto.max(1) as f64 * 2.0) * t;
        unit(suma(f, suma(por(r, nx), por(u, ny))))
    }

    /// Lo que mide un pixel a la distancia del objetivo.
    fn por_pixel(&self, alto: u32) -> f64 {
        2.0 * self.dist * (FOV / 2.0).tan() / alto.max(1) as f64
    }

    /// Acercar `factor` (<1 acerca) dejando quieto `fijo` en la pantalla.
    pub fn zoom(&self, factor: f64, fijo: [f64; 3]) -> Orbita {
        let factor = factor.clamp(1e-3, 1e3);
        let mut o = *self;
        o.objetivo = suma(fijo, por(sub(self.objetivo, fijo), factor));
        o.dist = (self.dist * factor).max(1e-4);
        o
    }

    /// Girar alrededor de `pivote` (rumbo y altura en radianes).
    pub fn girar(&self, d_rumbo: f64, d_altura: f64, pivote: [f64; 3]) -> Orbita {
        let altura = (self.altura + d_altura).clamp(-ALTURA_MAX, ALTURA_MAX);
        let d_altura = altura - self.altura;
        let ojo = self.ojo();
        // Rumbo: alrededor de la vertical del pivote.
        let rz = |p: [f64; 3]| {
            let q = sub(p, pivote);
            let (c, s) = (d_rumbo.cos(), d_rumbo.sin());
            suma(pivote, [q[0] * c - q[1] * s, q[0] * s + q[1] * c, q[2]])
        };
        let (ojo, obj) = (rz(ojo), rz(self.objetivo));
        let mut o = Orbita {
            objetivo: obj,
            rumbo: self.rumbo + d_rumbo,
            altura: self.altura,
            dist: self.dist,
        };
        // Altura: alrededor del eje derecho de la camara, por el pivote.
        let (r, _, _) = o.ejes();
        let rot = |p: [f64; 3]| {
            let q = sub(p, pivote);
            let (c, s) = (d_altura.cos(), d_altura.sin());
            // Rodrigues, con el signo que sube el ojo al subir la altura.
            let k = por(r, -1.0);
            let v = suma(
                suma(por(q, c), por(cruz(k, q), s)),
                por(k, punto(k, q) * (1.0 - c)),
            );
            suma(pivote, v)
        };
        let (ojo, obj) = (rot(ojo), rot(o.objetivo));
        o.objetivo = obj;
        o.altura = altura;
        // Que el objetivo quede a `dist` delante del ojo nuevo.
        o.objetivo = suma(ojo, por(o.adelante(), self.dist));
        o
    }

    /// Mover la camara `dx`, `dy` pixeles (el modelo sigue al raton).
    pub fn mover(&self, dx: f64, dy: f64, alto: u32) -> Orbita {
        let (r, u, _) = self.ejes();
        let k = self.por_pixel(alto);
        let mut o = *self;
        o.objetivo = suma(self.objetivo, suma(por(r, -dx * k), por(u, dy * k)));
        o
    }

    fn hacia(&self, destino: &Orbita, k: f64) -> Orbita {
        let mut dr = destino.rumbo - self.rumbo;
        while dr > std::f64::consts::PI {
            dr -= std::f64::consts::TAU;
        }
        while dr < -std::f64::consts::PI {
            dr += std::f64::consts::TAU;
        }
        Orbita {
            objetivo: suma(self.objetivo, por(sub(destino.objetivo, self.objetivo), k)),
            rumbo: self.rumbo + dr * k,
            altura: self.altura + (destino.altura - self.altura) * k,
            dist: (self.dist.ln() + (destino.dist.ln() - self.dist.ln()) * k).exp(),
        }
    }

    fn cerca_de(&self, o: &Orbita) -> bool {
        let d = sub(self.objetivo, o.objetivo);
        punto(d, d).sqrt() < o.dist * 1e-3
            && (self.rumbo - o.rumbo).abs() < 1e-3
            && (self.altura - o.altura).abs() < 1e-3
            && (self.dist / o.dist - 1.0).abs() < 1e-3
    }
}

/// Los textos de la ventana, en el idioma de la app.
#[derive(Debug, Clone)]
pub struct TextosUi3d {
    pub abriendo: String,
    pub vacio: String,
    pub error: String,
    pub todo: String,
    pub tema: String,
    pub corte: String,
    pub aislar: String,
    pub panel: String,
    pub niveles: String,
    pub categorias: String,
    pub sin_nivel: String,
    pub mostrar_todo: String,
    pub aristas: String,
    pub planta: String,
    pub encima: String,
    pub cerrar: String,
    pub altura_corte: String,
    pub pista: String,
}

impl Default for TextosUi3d {
    fn default() -> Self {
        Self {
            abriendo: "Abriendo el modelo…".into(),
            vacio: "El modelo no tiene nada que dibujar".into(),
            error: "No se pudo abrir el modelo".into(),
            todo: "Ver todo el modelo\tF".into(),
            tema: "Fondo claro u oscuro\tB".into(),
            corte: "Caja de sección\tC".into(),
            aislar: "Aislar lo elegido\tI".into(),
            panel: "Niveles y categorías\tL".into(),
            niveles: "NIVELES".into(),
            categorias: "CATEGORÍAS".into(),
            sin_nivel: "Sin nivel".into(),
            mostrar_todo: "Mostrar todo".into(),
            aristas: "Aristas\tE".into(),
            planta: "Vista en planta\tP".into(),
            encima: "Siempre encima\tT".into(),
            cerrar: "Cerrar\tEsc".into(),
            altura_corte: "Corte".into(),
            pista: "Arrastrar: girar · Derecho: mover · Rueda: acercar · Clic: ver que es · Otro clic: aislarlo".into(),
        }
    }
}

const M_TODO: u32 = 1;
const M_TEMA: u32 = 2;
const M_CORTE: u32 = 3;
const M_ARISTAS: u32 = 4;
const M_PLANTA: u32 = 5;
const M_ENCIMA: u32 = 6;
const M_CERRAR: u32 = 7;
const M_AISLAR: u32 = 8;
const M_PANEL: u32 = 9;
const M_EXPORTAR: u32 = 10;
const M_MARCADOR: u32 = 11;
const M_MARCOS: u32 = 12;
const M_IMPRIMIR: u32 = 13;
const M_BORRAR_MARCOS: u32 = 14;
const M_ANOTAR: u32 = 15;
/// La primera vista guardada; las demas, seguidas.
const M_VISTA: u32 = 100;

#[allow(clippy::too_many_arguments)]
fn menu(
    hwnd: HWND,
    t: &TextosUi3d,
    corte: bool,
    aislado: Option<bool>,
    panel: bool,
    aristas: bool,
    fijada: bool,
    vistas: &[String],
    exportar: Option<&str>,
    marcos: &[(u32, &str, bool)],
) {
    // SAFETY: menu propio que se destruye antes de salir; cadenas vivas.
    unsafe {
        let Ok(m) = CreatePopupMenu() else { return };
        let poner = |id: u32, texto: &str, marca: bool| {
            let v: Vec<u16> = texto.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = AppendMenuW(
                m,
                if marca {
                    MF_STRING | MF_CHECKED
                } else {
                    MF_STRING
                },
                id as usize,
                PCWSTR(v.as_ptr()),
            );
        };
        poner(M_TODO, &t.todo, false);
        poner(M_PLANTA, &t.planta, false);
        poner(M_TEMA, &t.tema, false);
        poner(M_CORTE, &t.corte, corte);
        // Solo si hay algo elegido o aislado.
        if let Some(a) = aislado {
            poner(M_AISLAR, &t.aislar, a);
        }
        poner(M_PANEL, &t.panel, panel);
        poner(M_ARISTAS, &t.aristas, aristas);
        // Las vistas guardadas del croquis, con su tecla (1 a 9).
        if !vistas.is_empty() {
            let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
            for (i, n) in vistas.iter().enumerate() {
                let texto = if i < 9 {
                    format!("{n}	{}", i + 1)
                } else {
                    n.clone()
                };
                poner(M_VISTA + i as u32, &texto, false);
            }
        }
        // Los marcadores y los marcos para imprimir.
        if !marcos.is_empty() {
            let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
            for (id, texto, marca) in marcos {
                poner(*id, texto, *marca);
            }
        }
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        if let Some(e) = exportar {
            poner(M_EXPORTAR, e, false);
        }
        poner(M_ENCIMA, &t.encima, fijada);
        poner(M_CERRAR, &t.cerrar, false);
        let mut p = windows::Win32::Foundation::POINT::default();
        let _ = GetCursorPos(&mut p);
        let _ = TrackPopupMenu(m, TPM_RIGHTBUTTON, p.x, p.y, Some(0), hwnd, None);
        let _ = DestroyMenu(m);
    }
}

/// Lo que se escribe tras una cota: metros, pies o nada (un DWG no lo dice).
fn unidad_de(metros: f32) -> &'static str {
    if (metros - 1.0).abs() < 1e-6 {
        " m"
    } else if (metros - 0.3048).abs() < 0.001 {
        " ft"
    } else {
        ""
    }
}

/// Un punto del modelo en la pantalla (pixeles); `None` si queda detras del ojo.
fn en_pantalla(m: &[[f32; 4]; 4], p: [f64; 3], w: u32, h: u32) -> Option<[f64; 2]> {
    let v = [p[0], p[1], p[2], 1.0];
    let c: Vec<f64> = (0..4)
        .map(|j| (0..4).map(|i| v[i] * m[i][j] as f64).sum())
        .collect();
    (c[3] > 1e-9).then(|| {
        [
            (c[0] / c[3] + 1.0) / 2.0 * w as f64,
            (1.0 - c[1] / c[3]) / 2.0 * h as f64,
        ]
    })
}

/// El centro de la cara `k` de la caja (`k` como en la caja: 0..3 las
/// caras del minimo x, y, z; 3..6 las del maximo).
fn centro_de_cara(c: [f32; 6], k: usize) -> [f64; 3] {
    let mut p = [
        (c[0] + c[3]) as f64 / 2.0,
        (c[1] + c[4]) as f64 / 2.0,
        (c[2] + c[5]) as f64 / 2.0,
    ];
    p[k % 3] = c[k] as f64;
    p
}

/// Hacia fuera de la cara `k`.
fn fuera_de_cara(k: usize) -> [f64; 3] {
    let mut d = [0.0; 3];
    d[k % 3] = if k < 3 { -1.0 } else { 1.0 };
    d
}

/// Un decimo del lado mayor de la caja: la medida para ver como se mueve
/// una cara en la pantalla.
fn decimo(c: [f32; 6]) -> f64 {
    ((c[3] - c[0]).max(c[4] - c[1]).max(c[5] - c[2]) as f64 * 0.1).max(1e-6)
}

/// Las 12 aristas de la caja.
fn aristas_de_caja(c: [f32; 6]) -> Vec<([f64; 3], [f64; 3])> {
    let esq = |i: usize| {
        [
            c[if i & 1 == 0 { 0 } else { 3 }] as f64,
            c[if i & 2 == 0 { 1 } else { 4 }] as f64,
            c[if i & 4 == 0 { 2 } else { 5 }] as f64,
        ]
    };
    let mut v = Vec::new();
    for i in 0..8usize {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                v.push((esq(i), esq(i | bit)));
            }
        }
    }
    v
}

/// La caja de seccion, ya en la pantalla: sus aristas y sus tiradores
/// (cara, centro y hacia fuera, unitario en pantalla).
struct CajaVista {
    aristas: Vec<([f64; 2], [f64; 2])>,
    tiradores: Vec<(usize, [f64; 2], [f64; 2])>,
    encima: Option<usize>,
}

fn caja_en_pantalla(
    c: [f32; 6],
    m: &[[f32; 4]; 4],
    w: u32,
    h: u32,
    encima: Option<usize>,
) -> CajaVista {
    let aristas = aristas_de_caja(c)
        .into_iter()
        .filter_map(|(a, b)| Some((en_pantalla(m, a, w, h)?, en_pantalla(m, b, w, h)?)))
        .collect();
    let lado = decimo(c);
    let tiradores = (0..6)
        .filter_map(|k| {
            let p = centro_de_cara(c, k);
            let a = en_pantalla(m, p, w, h)?;
            let b = en_pantalla(m, suma(p, por(fuera_de_cara(k), lado)), w, h)?;
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = dx.hypot(dy);
            Some((
                k,
                a,
                if l > 1e-6 {
                    [dx / l, dy / l]
                } else {
                    [0.0, 0.0]
                },
            ))
        })
        .collect();
    CajaVista {
        aristas,
        tiradores,
        encima,
    }
}

/// El tirador bajo (x, y), si hay uno a mano (el disco o su flecha).
fn tirador_en(cv: &CajaVista, x: i32, y: i32, e: f64) -> Option<usize> {
    cv.tiradores
        .iter()
        .map(|(k, p, d)| {
            let q = [p[0] + d[0] * 14.0 * e, p[1] + d[1] * 14.0 * e];
            let dist = |c: [f64; 2]| (c[0] - x as f64).hypot(c[1] - y as f64);
            (*k, dist(*p).min(dist(q)))
        })
        .filter(|(_, d)| *d <= 13.0 * e)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| k)
}

/// La caja de un elemento (relativa al origen), para encuadrarlo al aislarlo.
fn caja_de_elemento(m: &Modelo3d, e: u32) -> Option<[f32; 6]> {
    let mut c: Option<[f32; 6]> = None;
    for v in m.vertices.iter().filter(|v| v.elemento == e) {
        let p = v.pos;
        let k = c.get_or_insert([p[0], p[1], p[2], p[0], p[1], p[2]]);
        for i in 0..3 {
            k[i] = k[i].min(p[i]);
            k[i + 3] = k[i + 3].max(p[i]);
        }
    }
    c
}

/// La caja recien puesta: la del modelo, algo holgada (que no corte nada).
fn caja_inicial(c: [f32; 6]) -> [f32; 6] {
    let m = (c[3] - c[0]).max(c[4] - c[1]).max(c[5] - c[2]) * 0.01;
    [c[0] - m, c[1] - m, c[2] - m, c[3] + m, c[4] + m, c[5] + m]
}

/// Lleva la cara `k` de la caja a `valor`, sin cruzar la de enfrente ni
/// irse lejos del modelo (`limite`).
fn mover_cara(c: [f32; 6], k: usize, valor: f64, limite: [f32; 6]) -> [f32; 6] {
    let mut c = c;
    let a = k % 3;
    let hueco = ((limite[a + 3] - limite[a]) * 0.002).max(1e-6);
    let v = valor as f32;
    c[k] = if k < 3 {
        v.clamp(limite[a], (c[a + 3] - hueco).max(limite[a]))
    } else {
        v.clamp((c[a] + hueco).min(limite[a + 3]), limite[a + 3])
    };
    c
}

fn disco(c: &mut Constructor, centro: [f64; 2], r: f64, color: u32) {
    let n = 20;
    let mut t = Vec::with_capacity(n * 3);
    for i in 0..n {
        let a0 = i as f64 / n as f64 * std::f64::consts::TAU;
        let a1 = (i + 1) as f64 / n as f64 * std::f64::consts::TAU;
        t.push(centro);
        t.push([centro[0] + r * a0.cos(), centro[1] + r * a0.sin()]);
        t.push([centro[0] + r * a1.cos(), centro[1] + r * a1.sin()]);
    }
    c.triangulos(&t, color, Some(CAPA_ENCIMA));
}

struct Encima<'a> {
    /// Lo elegido, ya dicho: «Muro · nombre · 1ER PISO · 3.82 m³ · 21.80 m²».
    elegido: Option<String>,
    /// El panel de niveles y categorias, si esta abierto.
    arbol: Option<&'a arbol::Arbol>,
    caja: Option<CajaVista>,
    /// Mientras se arrastra un tirador: que cara y donde queda (en el mundo).
    cara: Option<(usize, f64)>,
    /// El punto alrededor del que se gira, mientras se gira.
    pivote: Option<[f64; 2]>,
    unidad: &'a str,
    pista: Option<&'a str>,
}

/// Lo que va plano encima del modelo: el nombre de lo elegido, la caja de
/// seccion con sus tiradores, el pivote del giro y la pista.
fn encima_del_modelo(
    textos: &mut Textos,
    a: &Encima,
    ui: &TextosUi3d,
    w: u32,
    h: u32,
    e: f64,
    claro: bool,
) -> Modelo {
    let mut c = Constructor::nuevo();
    let (pildora, letra) = if claro {
        (0xEE1C1C1Eu32, rgba(255, 255, 255))
    } else {
        (0xEEF5F5F7u32, rgba(0x1C, 0x1C, 0x1E))
    };
    let tam = 13.0 * e;
    let pastilla = |textos: &mut Textos, c: &mut Constructor, t: &str, x: f64, y: f64| {
        let ancho = textos.medir_pantalla(t, tam);
        rect(c, x, y, x + ancho + 20.0 * e, y + 26.0 * e, pildora);
        textos.en_pantalla(
            c,
            t,
            x + 10.0 * e,
            y + 17.5 * e,
            tam,
            letra,
            x + ancho + 40.0 * e,
        );
    };
    if let Some(t) = &a.elegido {
        pastilla(textos, &mut c, t, 12.0 * e, h as f64 - 38.0 * e);
    } else if let Some(p) = a.pista {
        pastilla(textos, &mut c, p, 12.0 * e, h as f64 - 38.0 * e);
    }
    let azul = rgba(0x00, 0x60, 0xDF);
    let blanco = rgba(255, 255, 255);
    if let Some(cv) = &a.caja {
        let raya_caja = (azul & 0x00ff_ffff) | 0xC000_0000;
        for (p, q) in &cv.aristas {
            raya(&mut c, *p, *q, 1.5 * e, raya_caja);
        }
        for (k, p, d) in &cv.tiradores {
            let grande = cv.encima == Some(*k) || a.cara.is_some_and(|(cara, _)| cara == *k);
            let r = if grande { 7.5 } else { 6.0 } * e;
            // La flecha hacia fuera de la cara y el disco en su centro.
            if d[0] != 0.0 || d[1] != 0.0 {
                let punta = [p[0] + d[0] * (r + 11.0 * e), p[1] + d[1] * (r + 11.0 * e)];
                let base = [p[0] + d[0] * (r + 2.0 * e), p[1] + d[1] * (r + 2.0 * e)];
                let n = [-d[1] * 5.5 * e, d[0] * 5.5 * e];
                c.triangulos(
                    &[
                        punta,
                        [base[0] + n[0], base[1] + n[1]],
                        [base[0] - n[0], base[1] - n[1]],
                    ],
                    azul,
                    Some(CAPA_ENCIMA),
                );
            }
            disco(&mut c, *p, r + 1.5 * e, blanco);
            disco(&mut c, *p, r, azul);
        }
    }
    if let Some(p) = a.pivote {
        disco(&mut c, p, 5.5 * e, blanco);
        disco(&mut c, p, 4.0 * e, rgba(0xFF, 0x8A, 0x00));
    }
    if let (Some((k, v)), Some(cv)) = (a.cara, &a.caja)
        && let Some((_, p, d)) = cv.tiradores.iter().find(|t| t.0 == k)
    {
        let eje = ["X", "Y", "Z"][k % 3];
        let t = format!("{} {eje} {v:.2}{}", ui.altura_corte, a.unidad);
        let ancho = textos.medir_pantalla(&t, tam);
        let (x, y) = (p[0] + d[0] * 30.0 * e, p[1] + d[1] * 30.0 * e);
        let x = if d[0] < 0.0 { x - ancho - 20.0 * e } else { x };
        let x = x.clamp(4.0 * e, (w as f64 - ancho - 24.0 * e).max(4.0 * e));
        let y = (y - 13.0 * e).clamp(4.0 * e, (h as f64 - 30.0 * e).max(4.0 * e));
        pastilla(textos, &mut c, &t, x, y);
    }
    // El panel, lo ultimo: encima de la caja y de todo.
    if let Some(ar) = a.arbol {
        ar.dibujar(&mut c, textos, ui, a.unidad, h, e, claro);
    }
    c.terminar()
}

/// **Lo que un croquis 3D le pide de mas a la ventana** (tanda 5): su fondo,
/// con que tema abre, que se encuadra al abrir, desde donde se mira y sus
/// vistas guardadas. Para un modelo BIM, todo vacio ([`ver`]).
#[derive(Debug, Clone, Default)]
pub struct Extras {
    /// El fondo propio (`colorDelFondo`), mientras no se cambie el tema.
    pub fondo: Option<[f32; 4]>,
    /// Con que tema abre (por defecto, oscuro).
    pub claro: Option<bool>,
    /// Lo que se encuadra (en el mundo, absoluto), si no es todo el modelo:
    /// el suelo y las hojas sin fin no cuentan.
    pub encuadre: Option<[f64; 6]>,
    /// Desde donde se mira al abrir y al pulsar F: rumbo y altura.
    pub orientacion: Option<(f64, f64)>,
    /// Las vistas congeladas del croquis: en el menu y con las teclas 1 a 9.
    pub vistas: Vec<crate::croquis::Vista3D>,
    /// «Exportar…» en el menu (y Ctrl+E): quien abre la ventana sabe como.
    pub exportar: Option<AlExportar>,
}

/// La foto de lo que se ve: `(ancho, alto, RGBA)`.
pub type Foto = (u32, u32, Vec<u8>);

/// El «Exportar…» del menu: su rotulo y que hacer con la vista que se ve
/// (la foto de la ventana, si se pudo leer).
#[derive(Clone)]
pub struct AlExportar {
    pub texto: String,
    pub hacer: std::sync::Arc<dyn Fn(Option<Foto>) + Send + Sync>,
}

/// **Lo que se ve en la ventana** (su zona de cliente), leido de la
/// pantalla: la ventana pinta con la tarjeta y no tiene superficie GDI que
/// imprimir, asi que la vista se exporta como se ve.
pub fn foto_de_la_ventana(hwnd: HWND) -> Option<Foto> {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, ClientToScreen, CreateCompatibleDC,
        CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SRCCOPY,
        SelectObject,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
    let mut r = RECT::default();
    let mut p = POINT::default();
    // SAFETY: ventana viva de este hilo; estructuras locales.
    unsafe {
        GetClientRect(hwnd, &mut r).ok()?;
        let _ = ClientToScreen(hwnd, &mut p);
    }
    let (ancho, alto) = (r.right - r.left, r.bottom - r.top);
    if ancho <= 0 || alto <= 0 {
        return None;
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: ancho,
            biHeight: -alto,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: objetos GDI creados y soltados aqui; `bits` vive hasta el
    // DeleteObject y mide ancho x alto x 4.
    unsafe {
        let pantalla = GetDC(None);
        let dc = CreateCompatibleDC(Some(pantalla));
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let Ok(bmp) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DeleteDC(dc);
            ReleaseDC(None, pantalla);
            return None;
        };
        let viejo = SelectObject(dc, bmp.into());
        let _ = BitBlt(dc, 0, 0, ancho, alto, Some(pantalla), p.x, p.y, SRCCOPY);
        let n = (ancho * alto * 4) as usize;
        let mut px = std::slice::from_raw_parts(bits as *const u8, n).to_vec();
        SelectObject(dc, viejo);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(dc);
        ReleaseDC(None, pantalla);
        for c in px.chunks_exact_mut(4) {
            c.swap(0, 2);
            c[3] = 255;
        }
        Some((ancho as u32, alto as u32, px))
    }
}

impl std::fmt::Debug for AlExportar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlExportar")
            .field("texto", &self.texto)
            .finish()
    }
}

/// **Abre la ventana del modelo** y no vuelve hasta que se cierra.
pub fn ver(
    titulo: &str,
    cargando: Receiver<Result<Modelo3d, String>>,
    ui: TextosUi3d,
) -> Result<(), String> {
    ver_con(titulo, cargando, ui, Extras::default())
}

/// [`ver`], con lo que pide un croquis ([`Extras`]).
pub fn ver_con(
    titulo: &str,
    cargando: Receiver<Result<Modelo3d, String>>,
    ui: TextosUi3d,
    extras: Extras,
) -> Result<(), String> {
    ver_con_barra(titulo, Barra3d::default(), cargando, ui, extras)
}

/// **Lo que la app le pone a la barra y al riel del modelo**: el tipo de
/// archivo (el recuadro junto al nombre, como en los demas visores) y lo del
/// riel de la derecha (19-oct-2026): los marcadores con todo el estado de la
/// vista ([`crate::marcadores`]), lo anotado sobre cada uno (el motor del
/// lienzo, con una capa por marcador) y los marcos para imprimir numerados,
/// como los del plano.
#[derive(Default)]
pub struct Barra3d {
    pub tipo: Option<TipoDeArchivo>,
    /// Donde se guardan los marcadores y los marcos del modelo.
    pub marcadores: Option<std::path::PathBuf>,
    /// El motor del lienzo sobre lo anotado del marcador `id` (su capa).
    #[allow(clippy::type_complexity)]
    pub anotador: Option<Box<dyn Fn(&str) -> Box<dyn crate::anotado::Anotador>>>,
    /// Quitar lo anotado de un marcador que se quita.
    #[allow(clippy::type_complexity)]
    pub quitar_tinta: Option<Box<dyn Fn(&str)>>,
    /// La foto de un trozo del escritorio `[x, y, ancho, alto]`: la de cada
    /// marco al imprimir.
    #[allow(clippy::type_complexity)]
    pub foto: Option<Box<dyn Fn([i32; 4]) -> Option<Foto>>>,
    /// Imprimir las fotos de los marcos en grupo, una hoja cada una (la
    /// ventana y las fotos, en su orden).
    #[allow(clippy::type_complexity)]
    pub imprimir: Option<Box<dyn Fn(isize, Vec<Foto>)>>,
    pub textos: TextosMarcos3d,
    /// El globo de los botones de la barra (ver [`Globo`]); sin el, no sale.
    pub globo: Option<Globo>,
}

/// Los textos de los marcadores y los marcos del modelo.
#[derive(Debug, Clone)]
pub struct TextosMarcos3d {
    pub marcador: String,
    pub marcos: String,
    pub imprimir: String,
    pub borrar_marcos: String,
    pub pista_marcos: String,
    pub pista_anotar: String,
    /// Anotar sobre la vista de ahora (el menu, con su tecla).
    pub anotar: String,
    /// Los nombres de los botones de la barra y del riel, para su globo.
    pub botones: NombresDeBotones,
}

impl Default for TextosMarcos3d {
    fn default() -> Self {
        TextosMarcos3d {
            anotar: "Anotar sobre esta vista\tA".into(),
            botones: NombresDeBotones::default(),
            marcador: "Marcador aquí (con el corte y lo oculto)\tCtrl+M".into(),
            marcos: "Marcos para imprimir\tI".into(),
            imprimir: "Imprimir los marcos…\tCtrl+P".into(),
            borrar_marcos: "Borrar los marcos".into(),
            pista_marcos: "Arrastra para marcar lo que se imprime · Ctrl+P: imprimir · Supr: quitar el último · Los números: ir a su vista · Esc: salir".into(),
            pista_anotar: "Lo anotado solo se ve en la vista de este marcador".into(),
        }
    }
}

/// **El estado de la vista de ahora**, para un marcador o un marco: la
/// camara, el corte y lo aislado en el mundo (con el origen sumado) y lo
/// oculto del panel.
#[allow(clippy::too_many_arguments)]
fn estado_de(
    cam: &Orbita,
    m: &Modelo3d,
    seccion: Option<[f32; 6]>,
    aislado: Option<u32>,
    arbol: Option<&Arbol>,
    claro: bool,
    aristas: bool,
) -> crate::marcadores::Estado3d {
    let origen = m.origen;
    crate::marcadores::Estado3d {
        objetivo: [
            cam.objetivo[0] + origen[0],
            cam.objetivo[1] + origen[1],
            cam.objetivo[2] + origen[2],
        ],
        rumbo: cam.rumbo,
        altura: cam.altura,
        dist: cam.dist,
        seccion: seccion.map(|c| std::array::from_fn(|k| c[k] as f64 + origen[k % 3])),
        aislado,
        aislado_clave: aislado.and_then(|el| m.clave_de_elemento(el)),
        ocultos: arbol.map(|a| a.ocultos_en_texto(m)).unwrap_or_default(),
        claro,
        aristas,
    }
}

/// La camara de un estado, en el mundo del modelo (sin su origen).
fn orbita_de_estado(s: &crate::marcadores::Estado3d, origen: [f64; 3]) -> Orbita {
    Orbita {
        objetivo: [
            s.objetivo[0] - origen[0],
            s.objetivo[1] - origen[1],
            s.objetivo[2] - origen[2],
        ],
        rumbo: s.rumbo,
        altura: s.altura.clamp(-ALTURA_MAX, ALTURA_MAX),
        dist: s.dist.max(1e-4),
    }
}

/// **Lo que ve el motor del lienzo sobre el modelo**: la pantalla contada
/// desde el centro y en milesimas del alto de la ventana (ver
/// [`crate::marcadores::a_normal`]); asi lo anotado no se separa del
/// modelo aunque la ventana cambie de tamano. Una unidad de la capa es mas
/// o menos un pixel.
fn vista_de_tinta(hwnd: HWND, w: u32, h: u32, e: f64, claro: bool) -> crate::anotado::VistaPlano {
    let mut p = windows::Win32::Foundation::POINT::default();
    // SAFETY: ventana propia; punto local.
    let _ = unsafe { windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut p) };
    crate::anotado::VistaPlano {
        centro: [0.0, 0.0],
        px: 1000.0 / h.max(1) as f64,
        ancho: w,
        alto: h,
        origen_pantalla: (p.x, p.y),
        escala_por_cien: (e * 100.0).round() as u32,
        unidad: 1.0,
        claro,
    }
}

/// Lo que sale encima del modelo por los marcos: los que son de la vista de
/// ahora (su raya azul y su numero), el que se arrastra y, marcando, los
/// numeros de todos y la pista. Y la pista de anotar.
#[allow(clippy::too_many_arguments)]
fn encima_de_marcos(
    textos: &mut Textos,
    marcos: &[crate::marcadores::Marco3d],
    ahora: Option<&crate::marcadores::Estado3d>,
    nuevo: Option<([f64; 2], [f64; 2])>,
    marcando: bool,
    pista: Option<&str>,
    w: u32,
    h: u32,
    e: f64,
) -> Modelo {
    let mut c = Constructor::nuevo();
    let azul = rgba(0x00, 0x78, 0xD4);
    let blanco = rgba(255, 255, 255);
    let caja_px = |c: &mut Constructor, k: [f64; 4], g: f64| {
        for (a, b) in [
            ([k[0], k[1]], [k[2], k[1]]),
            ([k[2], k[1]], [k[2], k[3]]),
            ([k[2], k[3]], [k[0], k[3]]),
            ([k[0], k[3]], [k[0], k[1]]),
        ] {
            raya(c, a, b, g, azul);
        }
    };
    let tam = 12.0 * e;
    for (i, m) in marcos.iter().enumerate() {
        if !ahora.is_some_and(|s| crate::marcadores::coincide(s, &m.estado)) {
            continue;
        }
        let k = crate::marcadores::caja_en_pixeles(m.caja, w, h);
        caja_px(&mut c, k, 2.0 * e);
        let t = (i + 1).to_string();
        let ancho = textos.medir_pantalla(&t, tam);
        rect(
            &mut c,
            k[0],
            k[1],
            k[0] + ancho + 12.0 * e,
            k[1] + 20.0 * e,
            azul,
        );
        textos.en_pantalla(
            &mut c,
            &t,
            k[0] + 6.0 * e,
            k[1] + 15.0 * e,
            tam,
            blanco,
            k[0] + 80.0 * e,
        );
    }
    if let Some((a, b)) = nuevo {
        caja_px(
            &mut c,
            [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[0].max(b[0]),
                a[1].max(b[1]),
            ],
            1.5 * e,
        );
    }
    if marcando {
        // Las fichas de los marcos (cada una lleva a la vista del suyo) y, a
        // su derecha, la pista: las mismas que en el plano y los documentos.
        pintar_fichas_y_pista(&mut c, textos, marcos.len(), h, e, pista.unwrap_or(""));
    } else if let Some(p) = pista {
        let t13 = 13.0 * e;
        let ancho = textos.medir_pantalla(p, t13);
        let (x, y) = (12.0 * e, h as f64 - 38.0 * e);
        rect(
            &mut c,
            x,
            y,
            x + ancho + 20.0 * e,
            y + 26.0 * e,
            0xEE1C_1C1E,
        );
        textos.en_pantalla(
            &mut c,
            p,
            x + 10.0 * e,
            y + 17.5 * e,
            t13,
            blanco,
            x + ancho + 40.0 * e,
        );
    }
    c.terminar()
}

/// Imprimiendo los marcos: cual toca (`None` en la cola: la vista entera,
/// si no hay marcos), las fotos hechas, la vista a la que se vuelve y en
/// que paso va (poner la vista del marco, fotografiarla, volver).
struct Impresion {
    cola: Vec<Option<usize>>,
    siguiente: usize,
    fotos: Vec<Foto>,
    volver: crate::marcadores::Estado3d,
    /// Si el tema se habia tocado (el fondo propio de un croquis vuelve).
    tema_tocado: bool,
    paso: PasoDeImprimir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasoDeImprimir {
    Poner,
    Fotografiar,
    Volver,
    Entregar,
}

/// [`ver_con`], con lo de la barra y el riel ([`Barra3d`]).
pub fn ver_con_barra(
    titulo: &str,
    barra_app: Barra3d,
    cargando: Receiver<Result<Modelo3d, String>>,
    ui: TextosUi3d,
    extras: Extras,
) -> Result<(), String> {
    let nombres_de_vistas: Vec<String> = extras.vistas.iter().map(|v| v.nombre.clone()).collect();
    // Encuadrar con la orientacion del croquis, si la trae.
    let encuadre = |caja: [f32; 6], w: u32, h: u32| {
        let mut o = Orbita::encuadrar(caja, w, h);
        if let Some((rumbo, altura)) = extras.orientacion {
            o.rumbo = rumbo;
            o.altura = altura.clamp(-ALTURA_MAX, ALTURA_MAX);
            o.dist = o.distancia_para(caja, w, h).max(1e-3);
        }
        o
    };
    // El tema se ha tocado: el fondo propio deja de mandar.
    let mut tema_tocado = false;
    let (hwnd, mut w, mut h) = crear_ventana(w!("PixPinModelo3d"), &format!("{titulo} — PixPin"))
        .map_err(|e| e.to_string())?;
    let mut gpu = match Gpu::nueva(hwnd, w, h) {
        Ok(g) => g,
        Err(e) => {
            // SAFETY: ventana propia.
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(format!("sin Direct3D: {e}"));
        }
    };
    // SAFETY: ventana propia.
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let e = dpi as f64 / 96.0;
    let mut textos = Textos::nuevo();
    let Barra3d {
        tipo,
        marcadores: fichero_marcadores,
        anotador: hacer_anotador,
        quitar_tinta,
        foto,
        imprimir: al_imprimir,
        textos: tm,
        globo: globo_barra,
    } = barra_app;
    // El boton de la barra cuyo globo se ensena ahora.
    let mut globo_puesto: Option<usize> = None;
    let cabecera = Cabecera::nueva(&mut textos, titulo, tipo, e);
    // El riel de la derecha, como el de los demas visores: el marcador
    // (siempre), el lapiz (lo anotado sobre un marcador) y el marco para
    // imprimir, si la app sabe imprimir.
    let mut botones_riel: Vec<&str> = vec!["marcador"];
    if hacer_anotador.is_some() {
        botones_riel.push("anotar");
    }
    if al_imprimir.is_some() && foto.is_some() {
        botones_riel.push("marcos");
    }
    let mut riel_encima: Option<usize> = None;
    // **Los marcadores y los marcos** (19-oct-2026), con todo el estado.
    let mut guardados = crate::marcadores::Guardados::de(fichero_marcadores.as_deref());
    let mut punto_encima: Option<usize> = None;
    // El marcador que es la vista de ahora (su punto en azul; su tinta se ve).
    let mut actual: Option<usize> = None;
    // El motor del lienzo de cada marcador (nace al verlo) y el que se anota.
    let mut anotadores: std::collections::HashMap<String, Box<dyn crate::anotado::Anotador>> =
        std::collections::HashMap::new();
    let mut anotando: Option<String> = None;
    // Los marcos: si se estan poniendo y el que se arrastra (pixeles).
    let mut marcando = false;
    let mut nuevo_marco: Option<([f64; 2], [f64; 2])> = None;
    let mut imprimiendo: Option<Impresion> = None;
    let (bw, bh) = (
        ancho_barra(e, &BOTONES, &cabecera).ceil() as i32,
        (ALTO_BARRA * e).ceil() as i32,
    );
    let hbarra = crear_barra(hwnd, bw, bh).map_err(|e| e.to_string())?;
    let mut gpu_barra = Gpu::nueva(hbarra, bw as u32, bh as u32).map_err(|e| e.to_string())?;
    ESTADO.with(|s| s.borrow_mut().barra = Some((hbarra, (8.0 * e) as i32)));
    // A pantalla completa: donde estaba la ventana antes.
    let mut antes_de_pantalla_completa: Option<RECT> = None;
    let mut modelo: Option<(Modelo3d, ModeloGpu)> = None;
    let mut mensaje = ui.abriendo.clone();
    let mut cam = Orbita {
        objetivo: [0.0; 3],
        rumbo: -0.8,
        altura: 0.5,
        dist: 10.0,
    };
    let mut destino = cam;
    let mut radio = 10.0;
    // Lo que se encuadra y se corta: la caja sin los puntos sueltos lejanos.
    let mut caja_vista = [0f32; 6];
    let mut claro = extras.claro.unwrap_or(false);
    let mut fijada = crate::ventana::FIJADA_AL_NACER;
    let mut aristas = true;
    // La caja de seccion (relativa al origen del modelo), si esta puesta.
    let mut seccion: Option<[f32; 6]> = None;
    // Arrastrando un tirador: la cara, desde donde, su valor al empezar y
    // cuantos pixeles se mueve en pantalla por unidad del mundo.
    let mut tirador: Option<(usize, i32, i32, f64, [f64; 2])> = None;
    let mut tirador_encima: Option<usize> = None;
    let mut elegido: Option<u32> = None;
    // El panel de niveles y categorias: lo que tiene el modelo y si se ve.
    let mut arbol_m: Option<Arbol> = None;
    // El encuadre del principio, mientras nadie toque la vista: si la
    // ventana cambia de tamano (el pin se coloca despues de abrirse), se
    // vuelve a encuadrar a la medida nueva.
    let mut sin_tocar = true;
    let mut panel = false;
    // El clic empezo en el panel: al soltar no se elige nada.
    let mut bajo_en_panel = false;
    // Aislado: el unico elemento que se ve y su caja.
    let mut aislado: Option<(u32, [f32; 6])> = None;
    let mut pista_hasta = Some(Instant::now() + Duration::from_secs(6));
    let (mut dentro_plano, mut dentro_barra) = (false, false);
    let mut fuera_desde: Option<Instant> = None;
    let mut barra_visible = false;
    // Arrastre: el ultimo punto, la camara alli, que boton, el pivote y si
    // ya se movio.
    let mut arrastre: Option<(i32, i32, Orbita, u8, [f64; 3], bool)> = None;
    let mut boton_encima: Option<usize> = None;
    let mut sucio = true;
    let mut barra_sucia = true;
    // El ultimo punto tocado por la rueda: se reusa mientras se sigue girando.
    let mut rueda: Option<(Instant, i32, i32, [f64; 3])> = None;
    let mut aviso_ui: Option<(Modelo, PlanoGpu)> = None;
    // Para las pruebas: imprimir los marcos dos segundos despues de abrir.
    let mut imprimir_al_abrir: Option<Instant> = None;
    let vista = |cam: &Orbita,
                 w: u32,
                 h: u32,
                 radio: f64,
                 elegido: Option<u32>,
                 seccion: Option<[f32; 6]>,
                 aislado: Option<u32>,
                 claro: bool| {
        let (r, u, f) = cam.ejes();
        let luz = unit(suma(suma(por(f, -0.5), por(u, 0.75)), por(r, 0.35)));
        let ojo = cam.ojo();
        Vista3d {
            mvp: cam.matriz(w, h, radio),
            luz: [luz[0] as f32, luz[1] as f32, luz[2] as f32, 0.0],
            ojo: [ojo[0] as f32, ojo[1] as f32, ojo[2] as f32, 1.0],
            corte: [
                if seccion.is_some() { 1.0 } else { 0.0 },
                0.0,
                (4.5 * e) as f32,
                0.0,
            ],
            sec_min: seccion.map_or([0.0; 4], |c| [c[0], c[1], c[2], 0.0]),
            sec_max: seccion.map_or([0.0; 4], |c| [c[3], c[4], c[5], 0.0]),
            elegido: elegido.map_or(0, |e| e + 1),
            claro: if claro { 1.0 } else { 0.0 },
            pantalla: [w as f32, h as f32],
            aislado: aislado.map_or(0, |e| e + 1),
            relleno: [0.0; 3],
        }
    };
    'bucle: loop {
        let animando = cam != destino;
        // SAFETY: bucle de mensajes de este hilo.
        unsafe {
            if !animando && !sucio && !barra_sucia && imprimiendo.is_none() {
                let espera = if modelo.is_none()
                    || barra_visible
                    || fuera_desde.is_some()
                    || pista_hasta.is_some()
                    || imprimir_al_abrir.is_some()
                {
                    50
                } else {
                    INFINITE
                };
                let _ = MsgWaitForMultipleObjectsEx(None, espera, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
            let mut m = MSG::default();
            while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
        }
        // Los marcadores y los marcos que trajo la sincronizacion (o la
        // ventana del plano en 2D, que comparte fichero).
        if guardados.mirar_el_disco() {
            punto_encima = None;
            sucio = true;
            // El marcador que se anotaba lo quito el otro aparato.
            if anotando
                .as_ref()
                .is_some_and(|id| !guardados.c.modelo.iter().any(|m| &m.id == id))
            {
                anotando = None;
            }
        }
        if modelo.is_none() && mensaje == ui.abriendo {
            let llegado = match cargando.try_recv() {
                Ok(r) => Some(r),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(String::new())),
            };
            if let Some(r) = llegado {
                match r {
                    Ok(m) if m.vacio() => mensaje = ui.vacio.clone(),
                    Ok(m) => match gpu.subir_3d(&m) {
                        Ok(g) => {
                            let c = m.caja;
                            radio = ((c[3] - c[0]).hypot(c[4] - c[1]).hypot(c[5] - c[2]) as f64
                                / 2.0)
                                .max(1e-3);
                            caja_vista = m.caja_util();
                            if let Some(a) = extras.encuadre {
                                let o = m.origen;
                                let r = |k: usize| (a[k] - o[k % 3]) as f32;
                                caja_vista = [r(0), r(1), r(2), r(3), r(4), r(5)];
                            }
                            arbol_m = Some(Arbol::de(&m, &ui));
                            cam = encuadre(caja_vista, w, h);
                            destino = cam;
                            modelo = Some((m, g));
                            if std::env::var_os("PIXPIN_IMPRIMIR_MARCOS_AL_ABRIR").is_some() {
                                imprimir_al_abrir = Some(Instant::now() + Duration::from_secs(2));
                            }
                            mensaje.clear();
                        }
                        Err(err) => mensaje = format!("{}: {err}", ui.error),
                    },
                    Err(err) if err.is_empty() => mensaje = ui.error.clone(),
                    Err(err) => mensaje = format!("{}: {err}", ui.error),
                }
                aviso_ui = None;
                sucio = true;
            }
        }
        let eventos: Vec<(De, Ev)> = ESTADO.with(|s| s.borrow_mut().eventos.drain(..).collect());
        let b_barra = botones_barra(e, &BOTONES, &cabecera);
        let boton_en = |x: i32| -> Option<usize> {
            b_barra
                .iter()
                .position(|(a, b)| (x as f64) >= *a && (x as f64) < *b)
        };
        let caja = caja_vista;
        let (mut cambiar_tema, mut cambiar_corte, mut cambiar_encima, mut encuadrar, mut planta) =
            (false, false, false, false, false);
        let mut cambiar_pantalla = false;
        // Los marcadores y los marcos: poner uno aqui, ir a uno o quitarlo;
        // anotar; marcar; imprimir; y el estado que hay que poner.
        let mut poner_marcador = false;
        let mut ir_a_marcador: Option<usize> = None;
        let mut quitar_marcador: Option<usize> = None;
        let mut cambiar_anotar = false;
        let mut cambiar_marcos = false;
        let mut imprimir_marcos = false;
        let mut pedir_estado: Option<(crate::marcadores::Estado3d, bool)> = None;
        let isla_riel = riel_cajas(w, e, botones_riel.len()).map(|(i, _)| i);
        let (n_marcadores, n_marcos, alto_ahora) =
            (guardados.c.modelo.len(), guardados.c.marcos.len(), h);
        let punto_bajo = move |x: i32, y: i32| {
            isla_riel
                .and_then(|i| crate::marcadores::punto_en(i, n_marcadores, alto_ahora, e, x, y))
        };
        let chip_bajo = move |x: i32, y: i32| {
            if marcando {
                ficha_en(n_marcos, alto_ahora, e, x, y)
            } else {
                None
            }
        };
        // Cambio lo que se ve en el panel: hay que rehacer los indices.
        let mut refiltrar = false;
        let mut cambiar_panel = false;
        // Aislar lo elegido (Some(e)) o volver a todo (None).
        let mut aislar: Option<Option<u32>> = None;
        // Una vista guardada del croquis, pedida por el menu o su tecla.
        let mut vista_pedida: Option<usize> = None;
        // Lo que se ve: el aislado o el modelo.
        let caja = aislado.map_or(caja, |a| a.1);
        // Hasta donde puede ir la caja: el modelo entero y algo mas.
        let limite = modelo
            .as_ref()
            .map_or([0f32; 6], |(m, _)| caja_inicial(caja_inicial(m.caja)));
        for (de, ev) in eventos {
            if matches!((de, &ev), (De::Plano, Ev::Tecla(_))) {
                sin_tocar = false;
            }
            // Imprimiendo, la ventana no atiende (va de vista en vista sola).
            if imprimiendo.is_some()
                && de == De::Plano
                && !matches!(ev, Ev::Tamano(..) | Ev::Cerrar)
            {
                continue;
            }
            // **El riel de la derecha** (marcador, lapiz y marco), los puntos
            // de los marcadores y los numeros de los marcos: sus clics no son
            // del modelo ni del motor del lienzo.
            if de == De::Plano && modelo.is_some() {
                match ev {
                    Ev::Mover(x, y) => {
                        let ahora = riel_en(w, e, botones_riel.len(), x, y).flatten();
                        if ahora != riel_encima {
                            riel_encima = ahora;
                            sucio = true;
                        }
                        let punto = punto_bajo(x, y);
                        if punto != punto_encima {
                            punto_encima = punto;
                            sucio = true;
                        }
                    }
                    Ev::Salir if riel_encima.is_some() || punto_encima.is_some() => {
                        riel_encima = None;
                        punto_encima = None;
                        sucio = true;
                    }
                    Ev::Bajar(x, y, _) | Ev::Doble(x, y)
                        if arrastre.is_none()
                            && tirador.is_none()
                            && nuevo_marco.is_none()
                            && (punto_bajo(x, y).is_some() || chip_bajo(x, y).is_some()) =>
                    {
                        if let Some(i) = chip_bajo(x, y) {
                            pedir_estado =
                                guardados.c.marcos.get(i).map(|m| (m.estado.clone(), true));
                        } else if matches!(ev, Ev::Bajar(_, _, 2)) {
                            // Clic derecho en un punto: quitar el marcador.
                            quitar_marcador = punto_bajo(x, y);
                        } else {
                            ir_a_marcador = punto_bajo(x, y);
                        }
                        // SAFETY: suelta la captura que puso el boton.
                        let _ = unsafe { ReleaseCapture() };
                        continue;
                    }
                    Ev::Subir(x, y, _)
                        if arrastre.is_none()
                            && tirador.is_none()
                            && nuevo_marco.is_none()
                            && (punto_bajo(x, y).is_some() || chip_bajo(x, y).is_some()) =>
                    {
                        continue;
                    }
                    Ev::Bajar(x, y, _) | Ev::Doble(x, y) if arrastre.is_none() => {
                        if let Some(b) = riel_en(w, e, botones_riel.len(), x, y) {
                            // SAFETY: suelta la captura que puso el boton.
                            let _ = unsafe { ReleaseCapture() };
                            if matches!(ev, Ev::Bajar(_, _, 0)) {
                                match b.map(|i| botones_riel[i]) {
                                    Some("marcador") => poner_marcador = true,
                                    Some("anotar") => cambiar_anotar = true,
                                    Some("marcos") => cambiar_marcos = true,
                                    _ => {}
                                }
                            }
                            continue;
                        }
                    }
                    _ => {}
                }
            }
            // F11 y Esc a pantalla completa, como en los demas visores (Esc,
            // salvo si el motor del lienzo lo quiere para lo que escribe).
            if let (De::Plano, Ev::Tecla(vk)) = (de, ev)
                && tecla_de_pantalla_completa(vk, antes_de_pantalla_completa.is_some())
                && !(vk == 0x1B
                    && anotando
                        .as_ref()
                        .and_then(|id| anotadores.get(id))
                        .is_some_and(|a| a.quiere_escape()))
            {
                cambiar_pantalla = true;
                continue;
            }
            // **Anotando sobre un marcador, el raton y el teclado son del motor
            // del lienzo** (como en el plano). La rueda no: moveria la vista y
            // lo anotado es de esta.
            if de == De::Plano
                && let Some(id) = &anotando
                && let Some(a) = anotadores.get_mut(id)
            {
                let v = vista_de_tinta(hwnd, w, h, e, claro);
                let (ox, oy) = v.origen_pantalla;
                // SAFETY: lee el estado de las teclas.
                let tecla = |vk: i32| unsafe { GetKeyState(vk) } < 0;
                let ev_motor = match ev {
                    Ev::Mover(x, y) => Some(crate::anotado::EventoPlano::Mover(x + ox, y + oy)),
                    Ev::Bajar(x, y, 0) | Ev::Doble(x, y) => {
                        Some(crate::anotado::EventoPlano::Bajar(x + ox, y + oy))
                    }
                    Ev::Subir(x, y, 0) => Some(crate::anotado::EventoPlano::Subir(x + ox, y + oy)),
                    Ev::Tecla(0x1B) if !a.quiere_escape() => None,
                    Ev::Tecla(vk) => Some(crate::anotado::EventoPlano::Tecla {
                        vk,
                        shift: tecla(0x10),
                        ctrl: tecla(0x11),
                        alt: tecla(0x12),
                    }),
                    Ev::TeclaSoltada(vk) => Some(crate::anotado::EventoPlano::TeclaSoltada(vk)),
                    Ev::Caracter(c) => char::from_u32(c).map(crate::anotado::EventoPlano::Caracter),
                    _ => None,
                };
                if let Some(evm) = ev_motor {
                    let r = a.evento(evm, &v);
                    if r.arrastra {
                        // SAFETY: ventana propia.
                        unsafe { windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(hwnd) };
                    }
                    if r.salir {
                        cambiar_anotar = true;
                    }
                    sucio = true;
                    if matches!(ev, Ev::Mover(..)) {
                        dentro_plano = true;
                    }
                    if r.consumido
                        || matches!(ev, Ev::Tecla(_) | Ev::TeclaSoltada(_) | Ev::Caracter(_))
                    {
                        continue;
                    }
                }
                if matches!(ev, Ev::Tecla(0x1B)) {
                    cambiar_anotar = true;
                    continue;
                }
                if matches!(
                    ev,
                    Ev::Bajar(..)
                        | Ev::Subir(..)
                        | Ev::Doble(..)
                        | Ev::Rueda(..)
                        | Ev::Desplazar(..)
                ) {
                    continue;
                }
            }
            // **Marcando, arrastrar con el izquierdo hace un marco** (no gira).
            if marcando && de == De::Plano && modelo.is_some() {
                let en_panel = |x: i32, y: i32| {
                    panel && arbol_m.as_ref().is_some_and(|ar| ar.dentro(x, y, h, e))
                };
                match ev {
                    Ev::Bajar(x, y, 0) if !en_panel(x, y) => {
                        nuevo_marco = Some(([x as f64, y as f64], [x as f64, y as f64]));
                        pista_hasta = None;
                        sucio = true;
                        continue;
                    }
                    Ev::Doble(x, y) if !en_panel(x, y) => continue,
                    Ev::Mover(x, y) if nuevo_marco.is_some() => {
                        dentro_plano = true;
                        if let Some((_, b)) = &mut nuevo_marco {
                            *b = [x as f64, y as f64];
                        }
                        sucio = true;
                        continue;
                    }
                    Ev::Subir(x, y, 0) if nuevo_marco.is_some() => {
                        if let (Some((a, _)), Some((m, _))) = (nuevo_marco.take(), &modelo) {
                            let b = [x as f64, y as f64];
                            // Menos de 8 pixeles es un clic, no un marco.
                            if (b[0] - a[0]).abs() > 8.0 * e && (b[1] - a[1]).abs() > 8.0 * e {
                                let p = crate::marcadores::a_normal(
                                    a[0].min(b[0]),
                                    a[1].min(b[1]),
                                    w,
                                    h,
                                );
                                let q = crate::marcadores::a_normal(
                                    a[0].max(b[0]),
                                    a[1].max(b[1]),
                                    w,
                                    h,
                                );
                                let estado = estado_de(
                                    &cam,
                                    m,
                                    seccion,
                                    aislado.map(|a| a.0),
                                    arbol_m.as_ref(),
                                    claro,
                                    aristas,
                                );
                                guardados.c.marcos.push(crate::marcadores::Marco3d {
                                    id: crate::marcadores::nuevo_id(),
                                    caja: [p[0], p[1], q[0], q[1]],
                                    estado,
                                });
                                guardados.guardar();
                            }
                        }
                        sucio = true;
                        continue;
                    }
                    _ => {}
                }
            }
            match (de, ev) {
                (_, Ev::Cerrar) => break 'bucle,
                (De::Barra, Ev::Mover(x, _)) => {
                    dentro_barra = true;
                    let b = boton_en(x);
                    if b != boton_encima {
                        boton_encima = b;
                        barra_sucia = true;
                    }
                }
                (De::Barra, Ev::Salir) => {
                    dentro_barra = false;
                    boton_encima = None;
                    barra_sucia = true;
                }
                (De::Barra, Ev::Bajar(x, _, 0)) => match boton_en(x).map(|i| BOTONES[i]) {
                    // El asa (o el nombre) mueve la ventana; a pantalla completa, no.
                    Some("asa" | "nombre") if antes_de_pantalla_completa.is_none() => {
                        // SAFETY: ventanas propias.
                        unsafe {
                            let _ = ReleaseCapture();
                            SendMessageW(
                                hwnd,
                                WM_NCLBUTTONDOWN,
                                Some(WPARAM(HTCAPTION as usize)),
                                Some(LPARAM(0)),
                            );
                        }
                    }
                    Some("tema") => cambiar_tema = true,
                    Some("corte") => cambiar_corte = true,
                    Some("capas") => cambiar_panel = true,
                    Some("pantalla") => cambiar_pantalla = true,
                    Some("fijar") => cambiar_encima = true,
                    Some("cerrar") => break 'bucle,
                    _ => {}
                },
                (De::Barra, _) => {}
                (De::Plano, Ev::Tamano(nw, nh)) => {
                    if (nw, nh) != (w, h) {
                        w = nw;
                        h = nh;
                        let _ = gpu.redimensionar(w, h);
                        aviso_ui = None;
                        sucio = true;
                        if sin_tocar && modelo.is_some() {
                            cam = encuadre(caja_vista, w, h);
                            destino = cam;
                        }
                    }
                }
                (De::Plano, Ev::Mover(x, y)) => {
                    dentro_plano = true;
                    if let Some((k, x0, y0, v0, por_unidad)) = tirador {
                        // La cara sigue al raton a lo largo de su eje.
                        let l2 = por_unidad[0] * por_unidad[0] + por_unidad[1] * por_unidad[1];
                        if l2 > 1e-9
                            && let Some(c) = seccion
                        {
                            let d = ((x - x0) as f64 * por_unidad[0]
                                + (y - y0) as f64 * por_unidad[1])
                                / l2;
                            let signo = if k < 3 { -1.0 } else { 1.0 };
                            seccion = Some(mover_cara(c, k, v0 + signo * d, limite));
                            sucio = true;
                        }
                    } else if let Some((x0, y0, c0, b, pivote, movido)) = arrastre {
                        let (dx, dy) = ((x - x0) as f64, (y - y0) as f64);
                        if movido || dx.abs() + dy.abs() > 3.0 {
                            // SAFETY: lee el estado de Mayus.
                            let mayus = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                            // **Girar**: poco a poco desde el ultimo punto (al
                            // llegar a la planta y volver responde en seguida)
                            // y media vuelta por el lado corto de la ventana,
                            // sea del tamano que sea.
                            let k = std::f64::consts::PI / w.min(h).max(1) as f64;
                            cam = if b == 0 && !mayus {
                                c0.girar(-dx * k, dy * k, pivote)
                            } else {
                                c0.mover(dx, dy, h)
                            };
                            destino = cam;
                            arrastre = Some((x, y, cam, b, pivote, true));
                            sucio = true;
                        }
                    } else if let Some(c) = seccion {
                        let cv = caja_en_pantalla(c, &cam.matriz(w, h, radio), w, h, None);
                        let encima = tirador_en(&cv, x, y, e);
                        if encima != tirador_encima {
                            tirador_encima = encima;
                            sucio = true;
                        }
                    }
                }
                (De::Plano, Ev::Salir) => dentro_plano = false,
                (De::Plano, Ev::Bajar(x, y, b)) => {
                    pista_hasta = None;
                    sin_tocar = false;
                    // SAFETY: lee el estado de Ctrl.
                    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                    if panel
                        && let Some(ar) = &mut arbol_m
                        && ar.dentro(x, y, h, e)
                    {
                        // **El panel**: mostrar u ocultar (Ctrl: solo ese).
                        bajo_en_panel = true;
                        if b == 0 && ar.tocar(x, y, ctrl, &ui, h, e) {
                            refiltrar = true;
                        }
                        sucio = true;
                    } else if b == 0 && ctrl {
                        // SAFETY: ventana propia.
                        unsafe {
                            let _ = ReleaseCapture();
                            SendMessageW(
                                hwnd,
                                WM_NCLBUTTONDOWN,
                                Some(WPARAM(HTCAPTION as usize)),
                                Some(LPARAM(0)),
                            );
                        }
                    } else if let Some(c) = seccion
                        && b == 0
                        && let Some(k) = tirador_en(
                            &caja_en_pantalla(c, &cam.matriz(w, h, radio), w, h, None),
                            x,
                            y,
                            e,
                        )
                    {
                        // Un tirador de la caja: cuantos pixeles por unidad
                        // se mueve su cara, para que siga al raton.
                        let m = cam.matriz(w, h, radio);
                        let p = centro_de_cara(c, k);
                        let lado = decimo(c);
                        let por_unidad = match (
                            en_pantalla(&m, p, w, h),
                            en_pantalla(&m, suma(p, por(fuera_de_cara(k), lado)), w, h),
                        ) {
                            (Some(a), Some(b)) => [(b[0] - a[0]) / lado, (b[1] - a[1]) / lado],
                            _ => [0.0, 0.0],
                        };
                        tirador = Some((k, x, y, c[k] as f64, por_unidad));
                        sucio = true;
                    } else if let Some((_, g)) = &modelo {
                        // Girar alrededor de lo que se toca; si no se toca
                        // nada, del centro de lo que se ve (la caja, si esta
                        // puesta), para que el modelo no salga disparado.
                        cam = destino;
                        let v = vista(
                            &cam,
                            w,
                            h,
                            radio,
                            elegido,
                            seccion,
                            aislado.map(|a| a.0),
                            claro,
                        );
                        let pivote = if b == 0 {
                            gpu.elegir(g, &v, x, y)
                                .ok()
                                .flatten()
                                .map(|(_, p)| p.map(|c| c as f64))
                        } else {
                            None
                        };
                        let c = seccion.unwrap_or(caja);
                        let centro = [
                            (c[0] + c[3]) as f64 / 2.0,
                            (c[1] + c[4]) as f64 / 2.0,
                            (c[2] + c[5]) as f64 / 2.0,
                        ];
                        // El centro, si queda delante del ojo; si no, el objetivo.
                        let delante = punto(sub(centro, cam.ojo()), cam.adelante()) > 0.0;
                        let pivote = pivote.unwrap_or(if delante { centro } else { cam.objetivo });
                        arrastre = Some((x, y, cam, b, pivote, false));
                    }
                }
                (De::Plano, Ev::Subir(x, y, b)) => {
                    let era_tirador = tirador.take().is_some();
                    let movido = arrastre.is_some_and(|a| a.5);
                    arrastre = None;
                    if std::mem::take(&mut bajo_en_panel) {
                        // Del panel: ya se hizo al bajar.
                    } else if era_tirador || movido {
                        // Fuera el pivote y la medida de la cara.
                        sucio = true;
                    } else if b == 2 {
                        let a = if aislado.is_some() {
                            Some(true)
                        } else {
                            elegido.map(|_| false)
                        };
                        let mut de_marcos: Vec<(u32, &str, bool)> =
                            vec![(M_MARCADOR, tm.marcador.as_str(), false)];
                        if hacer_anotador.is_some() {
                            de_marcos.push((M_ANOTAR, tm.anotar.as_str(), anotando.is_some()));
                        }
                        if botones_riel.contains(&"marcos") {
                            de_marcos.push((M_MARCOS, tm.marcos.as_str(), marcando));
                            de_marcos.push((M_IMPRIMIR, tm.imprimir.as_str(), false));
                            if !guardados.c.marcos.is_empty() {
                                de_marcos.push((M_BORRAR_MARCOS, tm.borrar_marcos.as_str(), false));
                            }
                        }
                        menu(
                            hwnd,
                            &ui,
                            seccion.is_some(),
                            a,
                            panel,
                            aristas,
                            fijada,
                            &nombres_de_vistas,
                            extras.exportar.as_ref().map(|x| x.texto.as_str()),
                            &de_marcos,
                        );
                    } else if b == 0
                        && let Some((_, g)) = &modelo
                    {
                        let v = vista(
                            &cam,
                            w,
                            h,
                            radio,
                            elegido,
                            seccion,
                            aislado.map(|a| a.0),
                            claro,
                        );
                        let nuevo = gpu.elegir(g, &v, x, y).ok().flatten().map(|(e, _)| e);
                        if nuevo.is_some()
                            && nuevo == elegido
                            && aislado.is_none_or(|a| Some(a.0) != nuevo)
                        {
                            // **Otro clic en el elegido: aislarlo.**
                            aislar = Some(nuevo);
                        } else if nuevo != elegido {
                            elegido = nuevo;
                            sucio = true;
                        }
                    }
                }
                (De::Plano, Ev::Doble(x, y))
                    if panel && arbol_m.as_ref().is_some_and(|ar| ar.dentro(x, y, h, e)) =>
                {
                    // Un doble clic en el panel son dos clics.
                    // SAFETY: lee el estado de Ctrl.
                    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                    if let Some(ar) = &mut arbol_m
                        && ar.tocar(x, y, ctrl, &ui, h, e)
                    {
                        refiltrar = true;
                    }
                    bajo_en_panel = true;
                    sucio = true;
                }
                (De::Plano, Ev::Doble(x, y)) => {
                    // En un elemento lo aisla el clic que sigue (el de soltar);
                    // en el vacio, todo el modelo.
                    let hay = modelo.as_ref().is_some_and(|(_, g)| {
                        let v = vista(
                            &cam,
                            w,
                            h,
                            radio,
                            elegido,
                            seccion,
                            aislado.map(|a| a.0),
                            claro,
                        );
                        gpu.elegir(g, &v, x, y).ok().flatten().is_some()
                    });
                    if !hay {
                        if aislado.is_some() {
                            aislar = Some(None);
                        } else {
                            encuadrar = true;
                        }
                    }
                }
                // Dos dedos en el panel tactil: mover el modelo con los
                // dedos, sin girar ni acercar.
                (De::Plano, Ev::Desplazar(_, _, dx, dy)) => {
                    pista_hasta = None;
                    sin_tocar = false;
                    cam = cam.mover(dx, dy, h);
                    destino = destino.mover(dx, dy, h);
                    sucio = true;
                }
                (De::Plano, Ev::Rueda(x, y, d)) => {
                    pista_hasta = None;
                    sin_tocar = false;
                    if panel
                        && let Some(ar) = &mut arbol_m
                        && ar.dentro(x, y, h, e)
                    {
                        ar.rueda(d, h, e);
                        sucio = true;
                        continue;
                    }
                    let factor = 1.2f64.powf(-(d as f64) / 120.0);
                    // El punto bajo el cursor: el tocado (si se sigue girando
                    // la rueda en el mismo sitio, el de antes).
                    let fijo = match rueda {
                        Some((t, rx, ry, p))
                            if t.elapsed() < Duration::from_millis(400)
                                && (rx - x).abs() + (ry - y).abs() < 4 =>
                        {
                            Some(p)
                        }
                        _ => None,
                    };
                    let fijo = fijo.or_else(|| {
                        let (_, g) = modelo.as_ref()?;
                        let v = vista(
                            &destino,
                            w,
                            h,
                            radio,
                            elegido,
                            seccion,
                            aislado.map(|a| a.0),
                            claro,
                        );
                        gpu.elegir(g, &v, x, y)
                            .ok()
                            .flatten()
                            .map(|(_, p)| p.map(|c| c as f64))
                    });
                    // Sin nada debajo: el punto del rayo a la distancia del objetivo.
                    let fijo = fijo.unwrap_or_else(|| {
                        suma(
                            destino.ojo(),
                            por(destino.rayo(x as f64, y as f64, w, h), destino.dist),
                        )
                    });
                    rueda = Some((Instant::now(), x, y, fijo));
                    destino = destino.zoom(factor, fijo);
                }
                (De::Plano, Ev::Menu(id)) => match id {
                    M_TODO => encuadrar = true,
                    M_TEMA => cambiar_tema = true,
                    M_CORTE => cambiar_corte = true,
                    M_AISLAR => aislar = Some(if aislado.is_some() { None } else { elegido }),
                    M_PANEL => cambiar_panel = true,
                    M_ARISTAS => {
                        aristas = !aristas;
                        sucio = true;
                    }
                    M_PLANTA => planta = true,
                    M_EXPORTAR => {
                        if let Some(x) = &extras.exportar {
                            (x.hacer)(foto_de_la_ventana(hwnd));
                        }
                    }
                    M_ENCIMA => cambiar_encima = true,
                    M_CERRAR => break 'bucle,
                    M_MARCADOR => poner_marcador = true,
                    M_ANOTAR => cambiar_anotar = true,
                    M_MARCOS => cambiar_marcos = true,
                    M_IMPRIMIR => imprimir_marcos = true,
                    M_BORRAR_MARCOS => {
                        guardados.c.marcos.clear();
                        guardados.guardar();
                        sucio = true;
                    }
                    n if (M_VISTA..M_VISTA + extras.vistas.len() as u32).contains(&n) => {
                        vista_pedida = Some((n - M_VISTA) as usize)
                    }
                    _ => {}
                },
                (De::Plano, Ev::TeclaSoltada(_) | Ev::Caracter(_)) => {}
                // **Las teclas de todos los visores** ([`Unificada`]): I los
                // marcos (con algo elegido o aislado, la I sigue aislando: era
                // su tecla antes), Supr el ultimo marco, Ctrl+P imprimirlos,
                // M o Ctrl+M un marcador, A anotar.
                // SAFETY: lee el estado de Ctrl.
                (De::Plano, Ev::Tecla(vk))
                    if tecla_unificada(
                        vk,
                        unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0,
                        marcando,
                    )
                    .is_some_and(|u| {
                        u != Unificada::Marcos
                            || marcando
                            || (elegido.is_none() && aislado.is_none())
                    }) =>
                {
                    // SAFETY: lee el estado de Ctrl.
                    match tecla_unificada(
                        vk,
                        unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0,
                        marcando,
                    ) {
                        Some(Unificada::Marcos) if botones_riel.contains(&"marcos") => {
                            cambiar_marcos = true
                        }
                        Some(Unificada::QuitarUltimoMarco) => {
                            if guardados.c.marcos.pop().is_some() {
                                guardados.guardar();
                            }
                            sucio = true;
                        }
                        Some(Unificada::Imprimir) => imprimir_marcos = true,
                        Some(Unificada::Marcador) => poner_marcador = true,
                        Some(Unificada::Anotar) => cambiar_anotar = true,
                        _ => {}
                    }
                }
                (De::Plano, Ev::Tecla(vk)) => match vk {
                    // Ctrl+1 a Ctrl+9: ir al marcador con ese numero.
                    // SAFETY: lee el estado de Ctrl.
                    0x31..=0x39 if unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0 => {
                        ir_a_marcador = Some((vk - 0x31) as usize)
                    }
                    // Marcando, Esc sale del modo (anotando, ya lo dejo el motor).
                    0x1B if marcando => cambiar_marcos = true,
                    0x1B => {
                        if aislado.is_some() {
                            aislar = Some(None);
                        } else if elegido.is_some() {
                            elegido = None;
                            sucio = true;
                        } else {
                            break 'bucle;
                        }
                    }
                    0x46 | 0x24 => encuadrar = true,
                    0x42 => cambiar_tema = true,
                    0x43 => cambiar_corte = true,
                    0x49 => aislar = Some(if aislado.is_some() { None } else { elegido }),
                    0x4C => cambiar_panel = true,
                    // E: las aristas (la A es anotar, como en todos).
                    0x45 => {
                        aristas = !aristas;
                        sucio = true;
                    }
                    0x50 => planta = true,
                    0x54 => cambiar_encima = true,
                    // 1 a 9: las vistas guardadas del croquis.
                    0x31..=0x39 => vista_pedida = Some((vk - 0x31) as usize),
                    0xBB | 0x6B => destino = destino.zoom(1.0 / 1.4, destino.objetivo),
                    0xBD | 0x6D => destino = destino.zoom(1.4, destino.objetivo),
                    0x25 => destino = destino.girar(0.26, 0.0, destino.objetivo),
                    0x27 => destino = destino.girar(-0.26, 0.0, destino.objetivo),
                    0x26 => destino = destino.girar(0.0, 0.2, destino.objetivo),
                    0x28 => destino = destino.girar(0.0, -0.2, destino.objetivo),
                    _ => {}
                },
            }
        }
        if let Some(que) = aislar
            && let Some((m, _)) = &modelo
        {
            // Aislar y encuadrarlo; o volver a todo, encuadrado.
            aislado = que.and_then(|e| Some((e, caja_de_elemento(m, e)?)));
            let c = aislado.map_or(caja_vista, |a| a.1);
            destino = encuadre(c, w, h);
            if let Some(e) = que {
                elegido = Some(e);
            }
            sucio = true;
        }
        if let Some(i) = vista_pedida
            && let (Some(v), Some((m, _))) = (extras.vistas.get(i), &modelo)
        {
            destino = crate::croquis::orbita_de(v, m.origen, w, h, e);
            sin_tocar = false;
        }
        let caja = aislado.map_or(caja_vista, |a| a.1);
        if encuadrar && modelo.is_some() {
            destino = encuadre(caja, w, h);
        }
        if planta && modelo.is_some() {
            let mut o = Orbita::encuadrar(caja, w, h);
            o.rumbo = -std::f64::consts::FRAC_PI_2;
            o.altura = ALTURA_MAX;
            o.dist = o.distancia_para(caja, w, h).max(1e-3);
            destino = o;
        }
        // ---- Los marcadores, lo anotado y los marcos (19-oct-2026).
        if imprimir_al_abrir.is_some_and(|t| Instant::now() >= t) {
            // Para las pruebas (`PIXPIN_IMPRIMIR_MARCOS_AL_ABRIR`): una vez.
            imprimir_al_abrir = None;
            imprimir_marcos = true;
        }
        if let Some(i) = ir_a_marcador {
            pedir_estado = guardados.c.modelo.get(i).map(|m| (m.estado.clone(), true));
        }
        if let Some(i) = quitar_marcador.filter(|i| *i < guardados.c.modelo.len()) {
            let id = guardados.c.modelo.remove(i).id;
            if anotando.as_deref() == Some(id.as_str()) {
                anotando = None;
            }
            anotadores.remove(&id);
            if let Some(f) = &quitar_tinta {
                f(&id);
            }
            guardados.guardar();
            punto_encima = None;
            sucio = true;
        }
        // Poner un marcador aqui (la vista a la que va la camara), o anotar
        // sin estar en uno: el lapiz pone el marcador de esta vista y anota
        // sobre el.
        let quiere_anotar = cambiar_anotar && anotando.is_none() && hacer_anotador.is_some();
        if (poner_marcador || quiere_anotar)
            && pedir_estado.is_none()
            && imprimiendo.is_none()
            && let Some((m, _)) = &modelo
        {
            let ahora = estado_de(
                &destino,
                m,
                seccion,
                aislado.map(|a| a.0),
                arbol_m.as_ref(),
                claro,
                aristas,
            );
            let i = match guardados
                .c
                .modelo
                .iter()
                .position(|mk| crate::marcadores::coincide(&ahora, &mk.estado))
            {
                Some(i) => i,
                None => {
                    guardados.c.modelo.push(crate::marcadores::Marcador3d {
                        id: crate::marcadores::nuevo_id(),
                        nombre: String::new(),
                        estado: ahora,
                    });
                    guardados.guardar();
                    guardados.c.modelo.len() - 1
                }
            };
            // La vista es ya la del marcador (sin esperar a que llegue).
            cam = destino;
            if quiere_anotar {
                let id = guardados.c.modelo[i].id.clone();
                if let Some(f) = &hacer_anotador {
                    anotadores.entry(id.clone()).or_insert_with(|| f(&id));
                }
                marcando = false;
                nuevo_marco = None;
                anotando = Some(id);
                cambiar_anotar = false;
            }
            sucio = true;
        }
        if cambiar_anotar && let Some(id) = anotando.take() {
            // Lo que se escribia se queda escrito y la capa se guarda.
            if let Some(a) = anotadores.get_mut(&id) {
                a.terminar();
            }
            sucio = true;
        }
        if cambiar_marcos && imprimiendo.is_none() {
            marcando = !marcando;
            nuevo_marco = None;
            if marcando && let Some(id) = anotando.take() {
                if let Some(a) = anotadores.get_mut(&id) {
                    a.terminar();
                }
            }
            sucio = true;
        }
        // **Imprimir los marcos en grupo** (Ctrl+P): se va a la vista de
        // cada uno, se pinta sin la interfaz y se fotografia su recuadro;
        // al final se vuelve a donde se estaba y las fotos van juntas al
        // dialogo. Sin marcos, la vista entera.
        if imprimir_marcos
            && imprimiendo.is_none()
            && al_imprimir.is_some()
            && foto.is_some()
            && let Some((m, _)) = &modelo
        {
            if let Some(id) = anotando.take()
                && let Some(a) = anotadores.get_mut(&id)
            {
                a.terminar();
            }
            let volver = estado_de(
                &destino,
                m,
                seccion,
                aislado.map(|a| a.0),
                arbol_m.as_ref(),
                claro,
                aristas,
            );
            let cola = if guardados.c.marcos.is_empty() {
                vec![None]
            } else {
                (0..guardados.c.marcos.len()).map(Some).collect()
            };
            imprimiendo = Some(Impresion {
                cola,
                siguiente: 0,
                fotos: Vec::new(),
                volver,
                tema_tocado,
                paso: PasoDeImprimir::Poner,
            });
            nuevo_marco = None;
            barra_visible = false;
            // SAFETY: ventana propia.
            let _ = unsafe { ShowWindow(hbarra, SW_HIDE) };
        }
        if let Some(imp) = &mut imprimiendo {
            match imp.paso {
                PasoDeImprimir::Poner => {
                    let mut est = match imp.cola.get(imp.siguiente).copied().flatten() {
                        Some(i) => guardados
                            .c
                            .marcos
                            .get(i)
                            .map_or_else(|| imp.volver.clone(), |m| m.estado.clone()),
                        None => imp.volver.clone(),
                    };
                    // En el papel, los colores de verdad: el tema claro, no
                    // el oscuro (que al imprimir salia apagado).
                    est.claro = true;
                    pedir_estado = Some((est, false));
                    imp.paso = PasoDeImprimir::Fotografiar;
                }
                PasoDeImprimir::Volver => {
                    pedir_estado = Some((imp.volver.clone(), false));
                    imp.paso = PasoDeImprimir::Entregar;
                }
                _ => {}
            }
            sucio = true;
        }
        // **Poner un estado entero** (el de un marcador o un marco): la
        // camara (suave, o de golpe al imprimir), el corte, lo aislado, lo
        // oculto, el tema y las aristas.
        if let Some((est, animar)) = pedir_estado.take()
            && let Some((m, _)) = &modelo
        {
            let o = m.origen;
            destino = orbita_de_estado(&est, o);
            if !animar {
                cam = destino;
            }
            seccion = est
                .seccion
                .map(|s| std::array::from_fn(|k| (s[k] - o[k % 3]) as f32));
            tirador = None;
            tirador_encima = None;
            let el = est
                .aislado_clave
                .as_deref()
                .and_then(|k| m.elemento_de_clave(k))
                .or(est.aislado.filter(|_| est.aislado_clave.is_none()));
            aislado = el.and_then(|el| Some((el, caja_de_elemento(m, el)?)));
            if let Some(ar) = &mut arbol_m
                && ar.poner_ocultos(&est.ocultos, m)
            {
                refiltrar = true;
            }
            if claro != est.claro {
                claro = est.claro;
                tema_tocado = true;
                aviso_ui = None;
            }
            aristas = est.aristas;
            sin_tocar = false;
            pista_hasta = None;
            barra_sucia = true;
            sucio = true;
        }
        if cambiar_panel {
            panel = !panel;
            barra_sucia = true;
            sucio = true;
        }
        if refiltrar && let (Some((m, g)), Some(ar)) = (&mut modelo, &arbol_m) {
            let ver = |k: u32| m.elementos.get(k as usize).is_none_or(|el| ar.visible(el));
            if let Err(err) = gpu.filtrar_3d(g, m, &ver) {
                tracing::warn!(error = %err, "no se pudo ocultar");
            }
            // Lo elegido, si se oculto, se suelta.
            if elegido.is_some_and(|k| !ver(k)) {
                elegido = None;
            }
        }
        if cambiar_tema {
            claro = !claro;
            tema_tocado = true;
            barra_sucia = true;
            aviso_ui = None;
            sucio = true;
        }
        if cambiar_corte && modelo.is_some() {
            seccion = if seccion.is_some() {
                None
            } else {
                Some(caja_inicial(caja))
            };
            tirador = None;
            tirador_encima = None;
            barra_sucia = true;
            sucio = true;
        }
        if cambiar_encima {
            fijada = !fijada;
            poner_encima(hwnd, fijada);
            barra_sucia = true;
        }
        if cambiar_pantalla {
            alternar_pantalla_completa(hwnd, &mut antes_de_pantalla_completa);
            barra_sucia = true;
        }
        if pista_hasta.is_some_and(|t| Instant::now() > t) {
            pista_hasta = None;
            sucio = true;
        }
        // La barra, como en los planos.
        let quiere = dentro_plano || dentro_barra || arrastre.is_some();
        if quiere {
            fuera_desde = None;
        } else if barra_visible && fuera_desde.is_none() {
            fuera_desde = Some(Instant::now());
        }
        let mostrar = imprimiendo.is_none()
            && (quiere || fuera_desde.is_some_and(|t| t.elapsed() < Duration::from_millis(600)));
        if !mostrar {
            fuera_desde = None;
        }
        if mostrar && modelo.is_some() {
            if barra_visible {
                seguir_barra(hwnd);
            } else {
                let mut r = RECT::default();
                // SAFETY: ventanas propias; estructuras locales.
                unsafe {
                    let _ = GetWindowRect(hwnd, &mut r);
                    let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                    let mut info = MONITORINFO {
                        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                        ..Default::default()
                    };
                    let _ = GetMonitorInfoW(mon, &mut info);
                    let (x, y) = colocar_barra(r, info.rcWork, bw, bh, (8.0 * e) as i32);
                    let _ = SetWindowPos(
                        hbarra,
                        Some(if fijada { HWND_TOPMOST } else { HWND_NOTOPMOST }),
                        x,
                        y,
                        bw,
                        bh,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                }
                barra_visible = true;
                barra_sucia = true;
            }
        } else if barra_visible {
            barra_visible = false;
            // SAFETY: ventana propia.
            let _ = unsafe { ShowWindow(hbarra, SW_HIDE) };
        }
        if cam != destino {
            let c = cam.hacia(&destino, 0.35);
            cam = if c.cerca_de(&destino) { destino } else { c };
            sucio = true;
        }
        if barra_sucia && barra_visible {
            let activos = BOTONES.map(|b| match b {
                "corte" => seccion.is_some(),
                "capas" => panel,
                "fijar" => fijada,
                _ => false,
            });
            let aspecto = Aspecto {
                encima: boton_encima,
                claro,
                pantalla_completa: antes_de_pantalla_completa.is_some(),
                activos: &activos,
                ..Aspecto::default()
            };
            let m = barra(&mut textos, e, &BOTONES, &cabecera, aspecto);
            if let Ok(p) = gpu_barra.subir(&m) {
                let (gw, gh) = gpu_barra.tamano();
                let fondo = if claro {
                    [0.976, 0.976, 0.984, 1.0]
                } else {
                    [0.118, 0.118, 0.125, 1.0]
                };
                let _ = gpu_barra.dibujar(fondo, &[(&p, vista_pantalla(&m, gw, gh))]);
            }
            barra_sucia = false;
        }
        // El globo del boton de la barra bajo el raton, en su ventanita.
        let quiere_globo = boton_encima.filter(|_| barra_visible && dentro_barra);
        if quiere_globo != globo_puesto {
            globo_de_la_barra(
                globo_barra.as_ref(),
                hbarra,
                barra_visible,
                quiere_globo,
                &BOTONES,
                &b_barra,
                &tm.botones,
                e,
            );
            globo_puesto = quiere_globo;
        }
        if aviso_ui.is_none() && !mensaje.is_empty() {
            let m = aviso(&mut textos, &mensaje, w, h, e, claro);
            aviso_ui = gpu.subir(&m).ok().map(|p| (m, p));
            sucio = true;
        }
        // **El marcador que es la vista de ahora** (la que se ve): su punto
        // en azul y su tinta a la vista. Si se anotaba sobre otro (se giro,
        // se corto...), se deja de anotar y su capa se guarda.
        let estado_ahora = modelo.as_ref().map(|(m, _)| {
            estado_de(
                &cam,
                m,
                seccion,
                aislado.map(|a| a.0),
                arbol_m.as_ref(),
                claro,
                aristas,
            )
        });
        let antes = actual;
        actual = estado_ahora.as_ref().and_then(|s| {
            guardados
                .c
                .modelo
                .iter()
                .position(|mk| crate::marcadores::coincide(s, &mk.estado))
        });
        if actual != antes {
            sucio = true;
        }
        let id_actual = actual.map(|i| guardados.c.modelo[i].id.clone());
        if anotando.is_some() && anotando != id_actual {
            if let Some(a) = anotando.take().and_then(|id| anotadores.get_mut(&id)) {
                a.terminar();
            }
            sucio = true;
        }
        if let (Some(id), Some(f)) = (&id_actual, &hacer_anotador)
            && !anotadores.contains_key(id)
        {
            anotadores.insert(id.clone(), f(id));
        }
        let mut pintado = false;
        if sucio {
            let fondo = match extras.fondo {
                Some(f) if !tema_tocado => f,
                _ if claro => [0.955, 0.958, 0.965, 1.0],
                _ => [0.13, 0.15, 0.19, 1.0],
            };
            // Imprimiendo, solo el modelo (y lo anotado de su vista).
            let sin_interfaz = imprimiendo.is_some();
            let mut capas: Vec<(&PlanoGpu, crate::gpu::Vista)> = Vec::new();
            if mensaje.is_empty() {
                aviso_ui = None;
            } else if let Some((m, p)) = &aviso_ui {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            // Marcando, su pista va abajo en vez de la de siempre.
            let pista_propia = if marcando {
                Some(tm.pista_marcos.as_str())
            } else {
                anotando.as_ref().map(|_| tm.pista_anotar.as_str())
            };
            let datos = Encima {
                elegido: modelo
                    .as_ref()
                    .filter(|_| pista_propia.is_none())
                    .and_then(|(m, _)| {
                        Some(arbol::describir(
                            m.elementos.get(elegido? as usize)?,
                            m,
                            unidad_de(m.metros),
                        ))
                    }),
                arbol: arbol_m.as_ref().filter(|_| panel),
                caja: seccion
                    .map(|c| caja_en_pantalla(c, &cam.matriz(w, h, radio), w, h, tirador_encima)),
                cara: tirador.and_then(|t| {
                    Some((
                        t.0,
                        modelo.as_ref()?.0.origen[t.0 % 3] + seccion?[t.0] as f64,
                    ))
                }),
                pivote: arrastre
                    .filter(|a| a.5 && a.3 == 0)
                    .and_then(|a| en_pantalla(&cam.matriz(w, h, radio), a.4, w, h)),
                unidad: modelo.as_ref().map_or("", |(m, _)| unidad_de(m.metros)),
                pista: pista_hasta
                    .is_some()
                    .then_some(ui.pista.as_str())
                    .filter(|_| pista_propia.is_none()),
            };
            let encima = if modelo.is_some() && !sin_interfaz {
                let m = encima_del_modelo(&mut textos, &datos, &ui, w, h, e, claro);
                gpu.subir(&m).ok().map(|p| (m, p))
            } else {
                None
            };
            if let Some((m, p)) = &encima {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            // Los marcos de esta vista, el que se arrastra y, marcando, sus
            // numeros y la pista.
            let de_marcos = (modelo.is_some()
                && !sin_interfaz
                && (marcando || pista_propia.is_some() || !guardados.c.marcos.is_empty()))
            .then(|| {
                let m = encima_de_marcos(
                    &mut textos,
                    &guardados.c.marcos,
                    estado_ahora.as_ref(),
                    nuevo_marco,
                    marcando,
                    pista_propia,
                    w,
                    h,
                    e,
                );
                gpu.subir(&m).ok().map(|p| (m, p))
            })
            .flatten();
            if let Some((m, p)) = &de_marcos {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            // El riel de la derecha, encima del modelo, y debajo los puntos
            // de los marcadores (el de esta vista en azul).
            let con_riel = modelo.is_some() && !sin_interfaz;
            let riel_m = con_riel
                .then(|| {
                    let activos: Vec<bool> = botones_riel
                        .iter()
                        .map(|b| {
                            (*b == "anotar" && anotando.is_some()) || (*b == "marcos" && marcando)
                        })
                        .collect();
                    let m = riel(e, w, &botones_riel, riel_encima, &activos);
                    gpu.subir(&m).ok().map(|p| (m, p))
                })
                .flatten();
            if let Some((m, p)) = &riel_m {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            // El globo del boton del riel bajo el raton: su nombre y su
            // tecla, a su izquierda (como en el plano y en los lectores).
            let globo_m = match (con_riel, riel_encima, riel_cajas(w, e, botones_riel.len())) {
                (true, Some(i), Some((_, cajas))) => cajas
                    .get(i)
                    .zip(tm.botones.globo(botones_riel[i]))
                    .and_then(|(k, t)| {
                        let m = globo_izquierda(&mut textos, *k, &t, e);
                        gpu.subir(&m).ok().map(|p| (m, p))
                    }),
                _ => None,
            };
            if let Some((m, p)) = &globo_m {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            let puntos_m = match riel_cajas(w, e, botones_riel.len()) {
                Some((isla, _)) if con_riel && !guardados.c.modelo.is_empty() => {
                    let mut c = Constructor::nuevo();
                    crate::marcadores::pintar_puntos(
                        &mut c,
                        &mut textos,
                        isla,
                        guardados.c.modelo.len(),
                        h,
                        e,
                        punto_encima,
                        actual,
                    );
                    let m = c.terminar();
                    gpu.subir(&m).ok().map(|p| (m, p))
                }
                _ => None,
            };
            if let Some((m, p)) = &puntos_m {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            let v = vista(
                &cam,
                w,
                h,
                radio,
                elegido.filter(|_| !sin_interfaz),
                seccion,
                aislado.map(|a| a.0),
                claro,
            );
            // Lo anotado sobre el marcador de esta vista, con el motor del
            // lienzo en el mismo dispositivo (y su barra, anotando).
            let vt = vista_de_tinta(hwnd, w, h, e, claro);
            let con_motor = anotando.is_some() && !sin_interfaz;
            // Lo anotado es de la vista exacta del marcador (va en pixeles):
            // solo con la camara ya parada en ella. Llegando aun se mueve un
            // poco (lo que deja `coincide`) y lo anotado iria fuera de su
            // sitio, detras del modelo.
            let id_tinta = id_actual
                .as_ref()
                .filter(|_| cam == destino || anotando.is_some());
            let r = match id_tinta.and_then(|id| anotadores.get_mut(id)) {
                Some(a) => gpu.dibujar_3d_con(
                    fondo,
                    modelo.as_ref().map(|(_, g)| (g, v)),
                    aristas,
                    &capas,
                    Some(&mut |d, t| a.pintar(d, t, &vt, con_motor)),
                ),
                None => {
                    gpu.dibujar_3d(fondo, modelo.as_ref().map(|(_, g)| (g, v)), aristas, &capas)
                }
            };
            if let Err(err) = r {
                tracing::warn!(error = %err, "no se pudo dibujar el modelo");
            }
            // Lo anotado iba en su foto estirada: otro fotograma, ya nitido.
            sucio = imprimiendo.is_none()
                && id_tinta
                    .and_then(|id| anotadores.get(id))
                    .is_some_and(|a| a.falta_pintar());
            pintado = true;
        }
        // Imprimiendo: la foto del marco recien pintado, o entregar las fotos.
        if pintado && let Some(imp) = &mut imprimiendo {
            match imp.paso {
                PasoDeImprimir::Fotografiar => {
                    // Que lo pintado este ya en la pantalla.
                    // SAFETY: sin precondiciones; sin DWM no se espera.
                    unsafe {
                        let _ = windows::Win32::Graphics::Dwm::DwmFlush();
                        let _ = windows::Win32::Graphics::Dwm::DwmFlush();
                    }
                    let caja = match imp
                        .cola
                        .get(imp.siguiente)
                        .copied()
                        .flatten()
                        .and_then(|i| guardados.c.marcos.get(i))
                    {
                        Some(m) => crate::marcadores::caja_en_pixeles(m.caja, w, h),
                        None => [0.0, 0.0, w as f64, h as f64],
                    };
                    let mut o = windows::Win32::Foundation::POINT::default();
                    // SAFETY: ventana propia; punto local.
                    let _ = unsafe { windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut o) };
                    let (x0, y0) = (
                        caja[0].max(0.0).round() as i32,
                        caja[1].max(0.0).round() as i32,
                    );
                    let (x1, y1) = (
                        caja[2].min(w as f64).round() as i32,
                        caja[3].min(h as f64).round() as i32,
                    );
                    if x1 > x0
                        && y1 > y0
                        && let Some(f) = &foto
                        && let Some(ft) = f([o.x + x0, o.y + y0, x1 - x0, y1 - y0])
                    {
                        imp.fotos.push(ft);
                    }
                    imp.siguiente += 1;
                    imp.paso = if imp.siguiente < imp.cola.len() {
                        PasoDeImprimir::Poner
                    } else {
                        PasoDeImprimir::Volver
                    };
                    sucio = true;
                }
                PasoDeImprimir::Entregar => {
                    if let Some(imp) = imprimiendo.take() {
                        tema_tocado = imp.tema_tocado;
                        sucio = true;
                        tracing::info!(hojas = imp.fotos.len(), "modelo: marcos a imprimir");
                        if !imp.fotos.is_empty()
                            && let Some(f) = &al_imprimir
                        {
                            f(hwnd.0 as isize, imp.fotos);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // Lo que se anotaba se queda escrito y las capas se guardan.
    for a in anotadores.values_mut() {
        a.terminar();
    }
    if let Some(f) = &globo_barra {
        f("", [0; 4], e);
    }
    // SAFETY: ventanas propias.
    unsafe {
        let _ = DestroyWindow(hbarra);
        let _ = DestroyWindow(hwnd);
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn proyectar(m: &[[f32; 4]; 4], p: [f64; 3], w: u32, h: u32) -> [f64; 2] {
        let v = [p[0] as f32, p[1] as f32, p[2] as f32, 1.0];
        let c: Vec<f32> = (0..4)
            .map(|j| (0..4).map(|i| v[i] * m[i][j]).sum())
            .collect();
        [
            ((c[0] / c[3]) as f64 + 1.0) / 2.0 * w as f64,
            (1.0 - (c[1] / c[3]) as f64) / 2.0 * h as f64,
        ]
    }

    #[test]
    fn el_objetivo_cae_en_el_centro_y_la_derecha_a_la_derecha() {
        let o = Orbita {
            objetivo: [1.0, 2.0, 3.0],
            rumbo: -0.8,
            altura: 0.5,
            dist: 20.0,
        };
        let m = o.matriz(800, 600, 10.0);
        let c = proyectar(&m, o.objetivo, 800, 600);
        assert!((c[0] - 400.0).abs() < 0.01 && (c[1] - 300.0).abs() < 0.01);
        // Lo que esta mas alto sale mas arriba (y menor en pantalla).
        let arriba = proyectar(&m, [1.0, 2.0, 5.0], 800, 600);
        assert!(arriba[1] < 300.0);
        // Sin espejo: visto desde el sur (rumbo -90°), +X sale a la derecha.
        let s = Orbita {
            objetivo: [0.0; 3],
            rumbo: -std::f64::consts::FRAC_PI_2,
            altura: 0.2,
            dist: 10.0,
        };
        let p = proyectar(&s.matriz(800, 600, 10.0), [1.0, 0.0, 0.0], 800, 600);
        assert!(p[0] > 400.0, "{p:?}");
    }

    #[test]
    fn el_zoom_deja_quieto_el_punto_fijo() {
        let o = Orbita {
            objetivo: [0.0; 3],
            rumbo: 0.3,
            altura: 0.4,
            dist: 50.0,
        };
        let fijo = [5.0, -3.0, 2.0];
        let antes = proyectar(&o.matriz(800, 600, 30.0), fijo, 800, 600);
        let z = o.zoom(0.5, fijo);
        let despues = proyectar(&z.matriz(800, 600, 30.0), fijo, 800, 600);
        assert!((antes[0] - despues[0]).abs() < 0.05 && (antes[1] - despues[1]).abs() < 0.05);
        assert!((z.dist - 25.0).abs() < 1e-9);
    }

    #[test]
    fn girar_alrededor_de_un_punto_lo_deja_en_su_sitio_de_pantalla_si_es_el_objetivo() {
        let o = Orbita {
            objetivo: [3.0, 1.0, 0.0],
            rumbo: 0.3,
            altura: 0.4,
            dist: 50.0,
        };
        let g = o.girar(0.5, 0.2, o.objetivo);
        assert!((g.rumbo - 0.8).abs() < 1e-9 && (g.altura - 0.6).abs() < 1e-9);
        let d = sub(g.objetivo, o.objetivo);
        assert!(punto(d, d).sqrt() < 1e-6, "{:?}", g.objetivo);
        // La altura no pasa de la planta.
        assert!(o.girar(0.0, 5.0, o.objetivo).altura <= ALTURA_MAX);
    }

    #[test]
    fn las_caras_de_la_caja_no_se_cruzan_ni_se_van_lejos() {
        let limite = [0.0, 0.0, 0.0, 10.0, 10.0, 10.0];
        let c = [1.0, 1.0, 1.0, 9.0, 9.0, 9.0];
        // La de arriba baja hasta 4.
        assert_eq!(mover_cara(c, 5, 4.0, limite)[5], 4.0);
        // No pasa de la de abajo, ni del limite.
        assert!(mover_cara(c, 5, -3.0, limite)[5] > 1.0);
        assert_eq!(mover_cara(c, 0, -50.0, limite)[0], 0.0);
        assert!(mover_cara(c, 0, 50.0, limite)[0] < 9.0);
    }

    #[test]
    fn los_tiradores_miran_hacia_fuera() {
        let o = Orbita {
            objetivo: [5.0, 5.0, 5.0],
            rumbo: -0.8,
            altura: 0.5,
            dist: 40.0,
        };
        let c = [0.0, 0.0, 0.0, 10.0, 10.0, 10.0];
        let cv = caja_en_pantalla(c, &o.matriz(800, 600, 20.0), 800, 600, None);
        assert_eq!(cv.aristas.len(), 12);
        assert_eq!(cv.tiradores.len(), 6);
        // El de arriba apunta hacia arriba en la pantalla; el de abajo, hacia abajo.
        let arriba = cv.tiradores.iter().find(|t| t.0 == 5).unwrap();
        let abajo = cv.tiradores.iter().find(|t| t.0 == 2).unwrap();
        assert!(
            arriba.2[1] < -0.5 && abajo.2[1] > 0.5,
            "{:?} {:?}",
            arriba.2,
            abajo.2
        );
        // Y se encuentra al tocarlo.
        assert_eq!(
            tirador_en(&cv, arriba.1[0] as i32, arriba.1[1] as i32, 1.0),
            Some(5)
        );
    }

    #[test]
    fn encuadrar_mete_la_caja_entera() {
        let caja = [0.0, 0.0, 0.0, 40.0, 10.0, 12.0];
        let o = Orbita::encuadrar(caja, 800, 600);
        let m = o.matriz(800, 600, 25.0);
        for i in 0..8 {
            let p = [
                caja[if i & 1 == 0 { 0 } else { 3 }] as f64,
                caja[if i & 2 == 0 { 1 } else { 4 }] as f64,
                caja[if i & 4 == 0 { 2 } else { 5 }] as f64,
            ];
            let q = proyectar(&m, p, 800, 600);
            assert!(
                q[0] >= 0.0 && q[0] <= 800.0 && q[1] >= 0.0 && q[1] <= 600.0,
                "{q:?}"
            );
        }
    }
}
