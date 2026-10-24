//! Donde cae cada cosa: posicion de columnas y filas (sumas acumuladas, con
//! las ocultas a cero), paneles inmovilizados, que se ve en una ventana y
//! que celda hay bajo un punto.
//!
//! Todo va en pixeles de la pantalla al 100 % de zoom ([`Rejilla::ppp`]:
//! 96 por defecto); quien pinta multiplica solo por el zoom.

use crate::dibujos::{Ancla, Dibujo};
use crate::estilos::Fuente;
use crate::hoja::{Hoja, Rango, Valor};

pub const MAX_COLUMNAS: u32 = 16_384;
pub const MAX_FILAS: u32 = 1_048_576;

fn es(nombre: &str, de: &str) -> bool {
    nombre.to_ascii_lowercase().starts_with(de)
}

/// Ancho del digito mas ancho de la fuente, en pixeles (el «MDW» de
/// ECMA-376 18.3.1.13). Calibri 11 da 7, Arial 10 da 7, Arial 11 da 8.
pub fn ancho_digito(f: &Fuente) -> f64 {
    ancho_digito_en(f, 96.0)
}

/// Como [`ancho_digito`], a `ppp` puntos por pulgada: Excel mide con la
/// letra a pixeles enteros de esa pantalla (11 pt a 144 ppp: 22 px).
pub fn ancho_digito_en(f: &Fuente, ppp: f64) -> f64 {
    let n = &*f.nombre;
    // Avance del digito en em, ajustado para dar lo que mide Excel con la
    // letra a pixeles enteros (Calibri y Aptos Narrow 11: 7 a 96 ppp y 11
    // a 144 ppp).
    let razon = if es(n, "calibri") || es(n, "aptos narrow") {
        0.48
    } else if es(n, "arial narrow") {
        0.45
    } else if es(n, "arial") || es(n, "helvetica") || es(n, "liberation sans") {
        0.556
    } else if es(n, "aptos") {
        0.53
    } else if es(n, "times") || es(n, "cambria") || es(n, "georgia") {
        0.5
    } else if es(n, "verdana") || es(n, "tahoma") {
        0.6
    } else if es(n, "consolas") || es(n, "courier") {
        0.55
    } else if es(n, "segoe") {
        0.53
    } else {
        0.5
    };
    ((f.tam as f64 * ppp / 72.0).round() * razon)
        .round()
        .max(1.0)
}

/// El alto por defecto de una fila para esa fuente, en puntos (Calibri 11:
/// 15; Arial 10: 12,75).
pub fn alto_por_fuente(f: &Fuente) -> f32 {
    let n = &*f.nombre;
    let razon = if es(n, "calibri") {
        1.2207
    } else if es(n, "arial") || es(n, "times") || es(n, "liberation") {
        1.149
    } else if es(n, "segoe") {
        1.33
    } else if es(n, "verdana") || es(n, "tahoma") {
        1.215
    } else {
        1.2
    };
    let px = (f.tam as f64 * 96.0 / 72.0 * razon + 2.0).round();
    (px * 0.75) as f32
}

/// Ancho de columna en caracteres -> pixeles (ECMA-376 18.3.1.13).
pub fn px_de_ancho(ancho: f64, mdw: f64) -> f64 {
    (((256.0 * ancho + (128.0 / mdw).trunc()) / 256.0) * mdw)
        .trunc()
        .max(0.0)
}

/// Puntos -> pixeles enteros, como redondea Excel las filas.
pub fn px_de_pt(pt: f32) -> f64 {
    px_de_pt_en(pt, 96.0)
}

/// Puntos -> pixeles enteros a `ppp` puntos por pulgada.
pub fn px_de_pt_en(pt: f32, ppp: f64) -> f64 {
    (pt as f64 * ppp / 72.0).round()
}

/// Lo que se ve de la hoja en un trozo de pantalla.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub fila0: u32,
    pub fila1: u32,
    pub col0: u32,
    pub col1: u32,
    /// El rectangulo de pantalla (x, y, ancho, alto), relativo al origen de
    /// la zona de celdas (debajo y a la derecha de los encabezados).
    pub pantalla: [f64; 4],
    /// El punto de la hoja (px al 100 %) que cae en la esquina del panel.
    pub origen: (f64, f64),
}

impl Panel {
    /// De hoja (px al 100 %) a pantalla.
    pub fn a_pantalla(&self, x: f64, y: f64, escala: f64) -> (f64, f64) {
        (
            self.pantalla[0] + (x - self.origen.0) * escala,
            self.pantalla[1] + (y - self.origen.1) * escala,
        )
    }
}

/// La ventana con la que se mira la hoja.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vista {
    /// Tamano de la zona de celdas en pantalla (sin encabezados).
    pub ancho: f64,
    pub alto: f64,
    /// Pixeles de pantalla por pixel de la rejilla: el zoom (1,0 = 100 %).
    pub escala: f64,
    /// Lo desplazado: el punto de la hoja (px al 100 %) que va en la esquina
    /// de la zona que se mueve (la de despues de los paneles fijos).
    pub x: f64,
    pub y: f64,
}

pub struct Rejilla {
    /// Ancho del digito de la fuente Normal.
    pub mdw: f64,
    pub ancho_defecto: f64,
    pub alto_defecto: f64,
    xs: Vec<f64>,
    ys: Vec<f64>,
    /// Hasta que indice de `hoja.filas` se ha mirado.
    cursor: usize,
    /// El indice de la ultima fila mirada (si cambia, la hoja se reordeno).
    ultima: u32,
    version: u64,
    pub filas_fijas: u32,
    pub columnas_fijas: u32,
    ocultas_por_defecto: bool,
    /// Puntos por pulgada de la pantalla para la que se calculo.
    pub ppp: f64,
    /// Altos medidos (autoajuste de filas sin `ht`), en px.
    medidas: std::collections::HashMap<u32, f64>,
    /// Filas que ya se miraron para el autoajuste.
    ajustadas: std::collections::HashSet<u32>,
}

impl Rejilla {
    pub fn nueva(hoja: &Hoja, fuente_normal: &Fuente) -> Rejilla {
        Self::nueva_con_digito(hoja, fuente_normal, None)
    }

    /// Como [`Rejilla::nueva`], con el ancho del digito ya medido con la
    /// fuente de verdad (`pixpin_render::hoja_office::CacheHoja::ancho_digito`);
    /// `None` usa la estimacion de [`ancho_digito`].
    pub fn nueva_con_digito(hoja: &Hoja, fuente_normal: &Fuente, digito: Option<f64>) -> Rejilla {
        Self::nueva_en(hoja, fuente_normal, digito, 96.0)
    }

    /// La rejilla en pixeles de una pantalla de `ppp` puntos por pulgada
    /// (96 al 100 %, 144 al 150 %...). Excel calcula los anchos con la
    /// letra medida a esa resolucion, y por redondeos no sale lo mismo que
    /// calcular a 96 y estirar: con esto la hoja mide lo que mide en Excel
    /// en la misma pantalla. `digito` es el ancho del digito medido a esa
    /// resolucion (o `None` para estimarlo).
    pub fn nueva_en(hoja: &Hoja, fuente_normal: &Fuente, digito: Option<f64>, ppp: f64) -> Rejilla {
        let ppp = if ppp.is_finite() && ppp >= 24.0 {
            ppp.min(960.0)
        } else {
            96.0
        };
        let mdw = digito
            .filter(|d| d.is_finite() && *d >= 1.0)
            .unwrap_or_else(|| ancho_digito_en(fuente_normal, ppp));
        let cab = &hoja.cabecera;
        let ancho_defecto = match cab.ancho_defecto {
            Some(w) => px_de_ancho(w, mdw),
            None => {
                // baseColWidth (8) + el margen (2 x techo(mdw/4) + 1: 5 px
                // con Calibri a 96 ppp), redondeado a multiplo de 8 como hace
                // Excel (8,43 -> 64 px).
                let base = cab.ancho_base.unwrap_or(8.0);
                let margen = 2.0 * (mdw / 4.0).ceil() + 1.0;
                let px = (base * mdw + margen).trunc();
                (px / 8.0).ceil() * 8.0
            }
        };
        let alto_pt = cab
            .alto_defecto
            .unwrap_or_else(|| alto_por_fuente(fuente_normal));
        let ocultas_por_defecto = cab.filas_ocultas_por_defecto;
        let alto_defecto = if ocultas_por_defecto {
            0.0
        } else {
            px_de_pt_en(alto_pt, ppp)
        };
        let mut anchos = vec![ancho_defecto; MAX_COLUMNAS as usize];
        for c in &cab.columnas {
            let w = if c.oculta {
                0.0
            } else {
                c.ancho.map_or(ancho_defecto, |a| px_de_ancho(a, mdw))
            };
            for k in c.min..=c.max.min(MAX_COLUMNAS - 1) {
                anchos[k as usize] = w;
            }
        }
        let mut xs = Vec::with_capacity(anchos.len() + 1);
        let mut x = 0.0;
        xs.push(0.0);
        for w in anchos {
            x += w;
            xs.push(x);
        }
        let mut r = Rejilla {
            mdw,
            ancho_defecto,
            alto_defecto,
            xs,
            ys: vec![0.0],
            cursor: 0,
            ultima: 0,
            version: u64::MAX,
            filas_fijas: cab.vista.filas_fijas,
            columnas_fijas: cab.vista.columnas_fijas,
            ocultas_por_defecto,
            ppp,
            medidas: std::collections::HashMap::new(),
            ajustadas: std::collections::HashSet::new(),
        };
        r.actualizar(hoja);
        r
    }

    /// Pone al dia las filas con lo que haya llegado desde la ultima vez.
    pub fn actualizar(&mut self, hoja: &Hoja) {
        if self.version == hoja.version {
            return;
        }
        self.version = hoja.version;
        if hoja.filas.len() < self.cursor
            || (self.cursor > 0
                && hoja.filas.get(self.cursor - 1).map(|f| f.indice) != Some(self.ultima))
        {
            // Se reordenaron: de cero.
            self.ys.truncate(1);
            self.cursor = 0;
        }
        for f in &hoja.filas[self.cursor..] {
            let ultima = self.ys.len() as u32 - 1; // filas ya cubiertas
            if f.indice < ultima {
                continue;
            }
            let y0 = *self.ys.last().unwrap_or(&0.0);
            let huecos = (f.indice - ultima) as usize;
            let paso = self.alto_defecto;
            self.ys.reserve(huecos + 1);
            for k in 1..=huecos {
                self.ys.push(y0 + paso * k as f64);
            }
            let alto = if let Some(&px) = self.medidas.get(&f.indice).filter(|_| !f.oculta) {
                px
            } else if f.oculta {
                0.0
            } else if !f.alto.is_nan() {
                px_de_pt_en(f.alto, self.ppp)
            } else if self.ocultas_por_defecto {
                // zeroHeight: solo se ven las filas con alto propio.
                px_de_pt_en(15.0, self.ppp)
            } else {
                self.alto_defecto
            };
            let y = *self.ys.last().unwrap_or(&0.0) + alto;
            self.ys.push(y);
        }
        self.cursor = hoja.filas.len();
        self.ultima = hoja.filas.last().map_or(0, |f| f.indice);
    }

    /// ¿Ya se miro esta fila para el autoajuste?
    pub fn ajustada(&self, fila: u32) -> bool {
        self.ajustadas.contains(&fila)
    }

    /// Apunta los altos medidos de filas sin `ht` (px) y marca como miradas
    /// las de `miradas`. Rehace las posiciones solo si algun alto cambio.
    pub fn fijar_altos(&mut self, hoja: &Hoja, miradas: &[u32], altos: &[(u32, f64)]) -> bool {
        self.ajustadas.extend(miradas.iter().copied());
        let mut cambio = false;
        for &(f, px) in altos {
            if px.is_finite() && px >= 0.0 && (self.alto(f) - px).abs() >= 0.5 {
                self.medidas.insert(f, px);
                cambio = true;
            }
        }
        if cambio {
            self.ys.truncate(1);
            self.cursor = 0;
            self.version = u64::MAX;
            self.actualizar(hoja);
        }
        cambio
    }

    /// Filas con posicion calculada (de ahi en adelante, todas por defecto).
    fn cubiertas(&self) -> u32 {
        self.ys.len() as u32 - 1
    }

    pub fn x(&self, col: u32) -> f64 {
        self.xs[(col.min(MAX_COLUMNAS)) as usize]
    }

    pub fn y(&self, fila: u32) -> f64 {
        let n = self.cubiertas();
        let fila = fila.min(MAX_FILAS);
        if fila <= n {
            self.ys[fila as usize]
        } else {
            self.ys[n as usize] + (fila - n) as f64 * self.alto_defecto
        }
    }

    pub fn ancho(&self, col: u32) -> f64 {
        self.x(col + 1) - self.x(col)
    }

    pub fn alto(&self, fila: u32) -> f64 {
        self.y(fila + 1) - self.y(fila)
    }

    /// La columna que contiene la x (px al 100 %).
    pub fn col_en(&self, x: f64) -> u32 {
        let k = self.xs.partition_point(|&v| v <= x);
        (k.max(1) as u32 - 1).min(MAX_COLUMNAS - 1)
    }

    pub fn fila_en(&self, y: f64) -> u32 {
        let n = self.cubiertas();
        let fin = self.ys[n as usize];
        if y >= fin {
            if self.alto_defecto <= 0.0 {
                return n.min(MAX_FILAS - 1);
            }
            return n
                .saturating_add(((y - fin) / self.alto_defecto) as u32)
                .min(MAX_FILAS - 1);
        }
        let k = self.ys.partition_point(|&v| v <= y);
        (k.max(1) as u32 - 1).min(MAX_FILAS - 1)
    }

    /// Rectangulo de una celda (o de su combinacion) en px al 100 %.
    pub fn caja(&self, r: Rango) -> [f64; 4] {
        let (x0, y0) = (self.x(r.col0), self.y(r.fila0));
        [x0, y0, self.x(r.col1 + 1) - x0, self.y(r.fila1 + 1) - y0]
    }

    /// Lo que mide todo lo usado (para las barras de desplazamiento).
    pub fn tamano_usado(&self, hoja: &Hoja) -> (f64, f64) {
        let col = hoja
            .max_col
            .max(hoja.cabecera.dimension.map_or(0, |d| d.col1));
        let fila = hoja
            .max_fila()
            .max(hoja.cabecera.dimension.map_or(0, |d| d.fila1));
        let mut w = self.x(col + 1);
        let mut h = self.y(fila + 1);
        for d in &hoja.cola.dibujos {
            let c = self.caja_dibujo(d);
            w = w.max(c[0] + c[2]);
            h = h.max(c[1] + c[3]);
        }
        (w, h)
    }

    /// Los paneles visibles (1, 2 o 4 con paneles inmovilizados), con lo
    /// que cae dentro de cada uno.
    pub fn paneles(&self, v: &Vista) -> Vec<Panel> {
        let e = v.escala.max(0.01);
        let ff = self.filas_fijas;
        let cf = self.columnas_fijas;
        let ancho_fijo = (self.x(cf) * e).min(v.ancho);
        let alto_fijo = (self.y(ff) * e).min(v.alto);
        let ox = v.x.max(self.x(cf));
        let oy = v.y.max(self.y(ff));
        let mut ps = Vec::with_capacity(4);
        let mut panel = |x0: f64,
                         y0: f64,
                         ancho: f64,
                         alto: f64,
                         origen: (f64, f64),
                         tope_c: Option<u32>,
                         tope_f: Option<u32>| {
            if ancho <= 0.0 || alto <= 0.0 {
                return;
            }
            let col0 = self.col_en(origen.0);
            let mut col1 = self.col_en(origen.0 + ancho / e);
            let fila0 = self.fila_en(origen.1);
            let mut fila1 = self.fila_en(origen.1 + alto / e);
            if let Some(t) = tope_c {
                col1 = col1.min(t.saturating_sub(1));
            }
            if let Some(t) = tope_f {
                fila1 = fila1.min(t.saturating_sub(1));
            }
            ps.push(Panel {
                fila0,
                fila1,
                col0,
                col1,
                pantalla: [x0, y0, ancho, alto],
                origen,
            });
        };
        if cf > 0 && ff > 0 {
            panel(
                0.0,
                0.0,
                ancho_fijo,
                alto_fijo,
                (0.0, 0.0),
                Some(cf),
                Some(ff),
            );
        }
        if ff > 0 {
            panel(
                ancho_fijo,
                0.0,
                v.ancho - ancho_fijo,
                alto_fijo,
                (ox, 0.0),
                None,
                Some(ff),
            );
        }
        if cf > 0 {
            panel(
                0.0,
                alto_fijo,
                ancho_fijo,
                v.alto - alto_fijo,
                (0.0, oy),
                Some(cf),
                None,
            );
        }
        panel(
            ancho_fijo,
            alto_fijo,
            v.ancho - ancho_fijo,
            v.alto - alto_fijo,
            (ox, oy),
            None,
            None,
        );
        ps
    }

    /// La celda bajo un punto de pantalla (relativo a la zona de celdas).
    pub fn celda_en(&self, v: &Vista, px: f64, py: f64) -> Option<(u32, u32)> {
        for p in self.paneles(v) {
            let [x, y, w, h] = p.pantalla;
            if px >= x && px < x + w && py >= y && py < y + h {
                let hx = p.origen.0 + (px - x) / v.escala;
                let hy = p.origen.1 + (py - y) / v.escala;
                return Some((self.fila_en(hy), self.col_en(hx)));
            }
        }
        None
    }

    /// La celda bajo el punto, subiendo a la esquina de su combinacion.
    pub fn celda_en_hoja(&self, hoja: &Hoja, v: &Vista, px: f64, py: f64) -> Option<(u32, u32)> {
        let (f, c) = self.celda_en(v, px, py)?;
        Some(
            hoja.combinada_en(f, c)
                .map_or((f, c), |r| (r.fila0, r.col0)),
        )
    }

    /// La caja de un dibujo en px al 100 %.
    pub fn caja_dibujo(&self, d: &Dibujo) -> [f64; 4] {
        // Los desplazamientos y tamanos del dibujo vienen en px a 96 ppp.
        let k = self.ppp / 96.0;
        let base = match d.ancla {
            Ancla::DosCeldas { desde, hasta } => {
                let x0 = self.x(desde.col) + (desde.dx * k).min(self.ancho(desde.col));
                let y0 = self.y(desde.fila) + (desde.dy * k).min(self.alto(desde.fila));
                let x1 = self.x(hasta.col) + (hasta.dx * k).min(self.ancho(hasta.col));
                let y1 = self.y(hasta.fila) + (hasta.dy * k).min(self.alto(hasta.fila));
                [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
            }
            Ancla::UnaCelda { desde, ancho, alto } => [
                self.x(desde.col) + desde.dx * k,
                self.y(desde.fila) + desde.dy * k,
                ancho * k,
                alto * k,
            ],
            Ancla::Absoluta { x, y, ancho, alto } => [x * k, y * k, ancho * k, alto * k],
        };
        match d.dentro {
            Some([fx, fy, fw, fh]) => [
                base[0] + fx * base[2],
                base[1] + fy * base[3],
                fw * base[2],
                fh * base[3],
            ],
            None => base,
        }
    }

    /// ¿Esta vacia (sin valor) la celda, y fuera de cualquier combinacion?
    /// Es lo que deja que un texto se desborde sobre ella.
    pub fn libre(hoja: &Hoja, fila: u32, col: u32) -> bool {
        let vacia = hoja
            .celda(fila, col)
            .is_none_or(|c| c.valor == Valor::Vacio);
        vacia && hoja.combinada_en(fila, col).is_none()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn anchos_como_excel() {
        let calibri = Fuente::default();
        assert_eq!(ancho_digito(&calibri), 7.0);
        assert_eq!(px_de_ancho(9.140625, 7.0), 64.0);
        assert_eq!(px_de_ancho(8.43, 7.0), 59.0);
        assert_eq!(alto_por_fuente(&calibri), 15.0);
        let arial = Fuente {
            nombre: Arc::from("Arial"),
            tam: 10.0,
            ..Fuente::default()
        };
        assert_eq!(ancho_digito(&arial), 7.0);
        assert_eq!(alto_por_fuente(&arial), 12.75);
        assert_eq!(px_de_pt(15.0), 20.0);
    }
}
