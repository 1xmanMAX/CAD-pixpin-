//! **Las plantillas de pagina entera**: planificacion, academico, creativo y
//! diario. Una hoja A4 (794 × 1123, o apaisada) con su banda de color, el
//! titulo en Fraunces y las secciones con su rotulo y su raya de acento.
//!
//! Todas salen de las mismas piezas ([`Pag`]: seccion, renglones, tabla,
//! casillas, caja...) con el color de su categoria ([`Tema`]): cambiar el
//! aire de todas es tocar aqui una vez.

use super::{Al, CAVEAT, FRAUNCES, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};

/// A4 vertical a 96 ppp.
pub(super) const W: f32 = 794.0;
pub(super) const H: f32 = 1123.0;
/// El margen.
pub(super) const M: f32 = 48.0;
/// Donde empieza lo de debajo del titulo.
const Y0: f32 = 128.0;

pub(super) const TINTA: ColorRgba = hex(0x22304a);
pub(super) const ROTULO: ColorRgba = hex(0x5d6b82);
pub(super) const RAYA: ColorRgba = hex(0xcfd7e3);
pub(super) const BORDE: ColorRgba = hex(0xb4c0d3);
pub(super) const BLANCO: ColorRgba = hex(0xffffff);

/// El color de una categoria: el acento (rotulos, banda, rayas fuertes) y
/// su version suave (cabeceras, fondos).
#[derive(Clone, Copy)]
pub(super) struct Tema {
    pub categoria: &'static str,
    pub acento: ColorRgba,
    pub suave: ColorRgba,
    pub titulo: &'static str,
}

pub(super) const PLANIFICACION: Tema = Tema {
    categoria: "Planificación",
    acento: hex(0x3f6db3),
    suave: hex(0xe8eff9),
    titulo: FRAUNCES,
};
pub(super) const ACADEMICO: Tema = Tema {
    categoria: "Académico",
    acento: hex(0x2c8a7f),
    suave: hex(0xe3f3f0),
    titulo: FRAUNCES,
};
pub(super) const CREATIVO: Tema = Tema {
    categoria: "Creativo",
    acento: hex(0xc0692b),
    suave: hex(0xfbeee3),
    titulo: FRAUNCES,
};
pub(super) const DIARIO: Tema = Tema {
    categoria: "Diario",
    acento: hex(0xbf5878),
    suave: hex(0xfbe9ef),
    titulo: CAVEAT,
};

const DIAS: [&str; 7] = ["LUN", "MAR", "MIÉ", "JUE", "VIE", "SÁB", "DOM"];
const LETRAS: [&str; 7] = ["L", "M", "X", "J", "V", "S", "D"];

/// Una hoja a medio hacer.
pub(super) struct Pag {
    pub l: Lapiz,
    pub t: Tema,
    pub w: f32,
    pub h: f32,
    /// Donde empieza cada campo de la derecha, en el orden en que se dieron.
    campos: Vec<f32>,
}

impl Pag {
    /// La hoja: papel blanco, banda de color arriba, el titulo y los campos
    /// de la derecha («Semana», «Mes»...), y la raya de acento debajo.
    pub fn nueva(t: Tema, titulo: &str, campos: &[&str]) -> Pag {
        Pag::de_tamano(t, W, H, titulo, campos)
    }

    pub fn apaisada(t: Tema, titulo: &str, campos: &[&str]) -> Pag {
        Pag::de_tamano(t, H, W, titulo, campos)
    }

    /// Vertical y, si lo de dentro no cabe en un A4, mas larga.
    pub fn alta(t: Tema, titulo: &str, campos: &[&str], h: f32) -> Pag {
        Pag::de_tamano(t, W, h.max(H), titulo, campos)
    }

    pub fn de_tamano(t: Tema, w: f32, h: f32, titulo: &str, campos: &[&str]) -> Pag {
        let mut p = Pag {
            l: Lapiz::default(),
            t,
            w,
            h,
            campos: vec![0.0; campos.len()],
        };
        p.l.rect(
            0.0,
            0.0,
            w,
            h,
            Some(hex(0xdde3ec)),
            Some(BLANCO),
            1.0,
            false,
        );
        if t.titulo == CAVEAT {
            p.adornos();
        }
        p.l.rect(0.0, 0.0, w, 10.0, None, Some(t.acento), 1.0, false);
        if t.titulo == CAVEAT {
            p.l.texto(M, 34.0, titulo, 50.0, CAVEAT, t.acento, true, Al::Izq);
        } else {
            p.l.texto(M, 42.0, titulo, 34.0, t.titulo, TINTA, true, Al::Izq);
        }
        let mut x = w - M;
        for (i, c) in campos.iter().enumerate().rev() {
            p.l.texto(
                x - CAMPO,
                62.0,
                &c.to_uppercase(),
                10.0,
                WORK_SANS,
                ROTULO,
                true,
                Al::Izq,
            );
            p.l.linea(x - CAMPO, 82.0, x, 82.0, BORDE, 1.0);
            p.campos[i] = x - CAMPO;
            x -= CAMPO + 22.0;
        }
        p.l.linea(M, 104.0, w - M, 104.0, t.acento, 2.0);
        p
    }

    /// Las manchas suaves de las hojas del diario, detras de todo.
    fn adornos(&mut self) {
        let s = self.t.suave;
        let (w, h) = (self.w, self.h);
        // Dentro del papel: una mancha que se sale agranda la figura.
        self.l.circulo(w - 130.0, 200.0, 84.0, None, Some(s), 1.0);
        self.l.circulo(w - 62.0, 300.0, 34.0, None, Some(s), 1.0);
        self.l.circulo(132.0, h - 132.0, 100.0, None, Some(s), 1.0);
        self.l.circulo(262.0, h - 56.0, 40.0, None, Some(s), 1.0);
    }

    /// El rotulo de una seccion y su raya de acento. Devuelve donde empieza
    /// lo de debajo.
    pub fn seccion(&mut self, x: f32, y: f32, w: f32, s: &str) -> f32 {
        self.l.texto(
            x,
            y,
            &s.to_uppercase(),
            11.0,
            WORK_SANS,
            self.t.acento,
            true,
            Al::Izq,
        );
        self.l
            .linea(x, y + 20.0, x + w, y + 20.0, self.t.acento, 1.25);
        y + 20.0
    }

    /// `n` renglones cada `paso`, empezando un paso por debajo de `y`.
    pub fn renglones(&mut self, x: f32, y: f32, w: f32, n: usize, paso: f32) -> f32 {
        for i in 1..=n {
            let yy = y + paso * i as f32;
            self.l.linea(x, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * n as f32
    }

    /// `01`, `02`... con su renglon.
    pub fn numerados(&mut self, x: f32, y: f32, w: f32, n: usize, paso: f32) -> f32 {
        self.numerados_desde(x, y, w, 1, n, paso)
    }

    /// Los mismos renglones numerados, empezando en `desde`.
    pub fn numerados_desde(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        desde: u32,
        n: usize,
        paso: f32,
    ) -> f32 {
        for i in 0..n {
            let yy = y + paso * (i + 1) as f32;
            self.l.texto(
                x + 2.0,
                yy - 17.0,
                &numero(desde + i as u32),
                10.0,
                WORK_SANS,
                self.t.acento,
                true,
                Al::Izq,
            );
            self.l.linea(x + 30.0, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * n as f32
    }

    /// Lo ya sabido de un campo de la cabecera («Mes»: noviembre), escrito
    /// a la derecha de su raya.
    pub fn valor(&mut self, campo: usize, s: &str) {
        if let Some(&x) = self.campos.get(campo) {
            self.l
                .texto(x + CAMPO, 60.0, s, 13.0, WORK_SANS, TINTA, false, Al::Der);
        }
    }

    /// Casillas para marcar, cada una con su renglon (y su texto si lo hay).
    pub fn casillas(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        n: usize,
        paso: f32,
        textos: &[&str],
    ) -> f32 {
        for i in 1..=n {
            let yy = y + paso * i as f32;
            self.l.rect(
                x + 2.0,
                yy - 15.0,
                12.0,
                12.0,
                Some(BORDE),
                Some(BLANCO),
                1.25,
                true,
            );
            if let Some(s) = textos.get(i - 1) {
                self.l.texto(
                    x + 22.0,
                    yy - 17.0,
                    s,
                    11.0,
                    WORK_SANS,
                    ROTULO,
                    false,
                    Al::Izq,
                );
            }
            self.l.linea(x + 22.0, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * n as f32
    }

    /// Un recuadro redondeado con su rotulo dentro, arriba a la izquierda.
    pub fn caja(&mut self, x: f32, y: f32, w: f32, h: f32, s: &str) {
        self.l.rect(x, y, w, h, Some(BORDE), None, 1.25, true);
        if !s.is_empty() {
            self.l.texto(
                x + 12.0,
                y + 10.0,
                &s.to_uppercase(),
                10.0,
                WORK_SANS,
                ROTULO,
                true,
                Al::Izq,
            );
        }
    }

    /// Una caja con el fondo suave.
    pub fn caja_suave(&mut self, x: f32, y: f32, w: f32, h: f32, s: &str) {
        self.l.rect(x, y, w, h, None, Some(self.t.suave), 1.0, true);
        if !s.is_empty() {
            self.l.texto(
                x + 12.0,
                y + 10.0,
                &s.to_uppercase(),
                10.0,
                WORK_SANS,
                self.t.acento,
                true,
                Al::Izq,
            );
        }
    }

    /// **Una tabla**: cabecera con fondo suave, columnas por fraccion del
    /// ancho, filas alternas apenas sombreadas y el borde de fuera. Devuelve
    /// donde acaba.
    pub fn tabla(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        cols: &[(&str, f32)],
        filas: usize,
        fh: f32,
    ) -> f32 {
        let total: f32 = cols.iter().map(|(_, f)| f).sum();
        let cab = if cols.iter().any(|(s, _)| !s.is_empty()) {
            fh.max(26.0)
        } else {
            0.0
        };
        let alto = cab + fh * filas as f32;
        if cab > 0.0 {
            self.l
                .rect(x, y, w, cab, None, Some(self.t.suave), 1.0, false);
        }
        for f in (1..filas).step_by(2) {
            self.l.rect(
                x,
                y + cab + fh * f as f32,
                w,
                fh,
                None,
                Some(hex(0xf7f9fc)),
                1.0,
                false,
            );
        }
        let mut cx = x;
        for (i, (s, f)) in cols.iter().enumerate() {
            let cw = w * f / total;
            if !s.is_empty() {
                self.l.texto(
                    cx + cw / 2.0,
                    y + (cab - 14.0) / 2.0,
                    &s.to_uppercase(),
                    9.5,
                    WORK_SANS,
                    self.t.acento,
                    true,
                    Al::Centro,
                );
            }
            if i > 0 {
                self.l.linea(cx, y, cx, y + alto, RAYA, 1.0);
            }
            cx += cw;
        }
        for f in 0..filas {
            let yy = y + cab + fh * f as f32;
            if f > 0 || cab > 0.0 {
                self.l.linea(x, yy, x + w, yy, RAYA, 1.0);
            }
        }
        self.l.rect(x, y, w, alto, Some(BORDE), None, 1.25, false);
        y + alto
    }

    /// Una lista de «Etiqueta: ____» (datos del vuelo, del curso...).
    pub fn datos(&mut self, x: f32, y: f32, w: f32, etiquetas: &[&str], paso: f32) -> f32 {
        for (i, s) in etiquetas.iter().enumerate() {
            let yy = y + paso * (i + 1) as f32;
            self.l.texto(
                x + 2.0,
                yy - 18.0,
                s,
                11.0,
                WORK_SANS,
                ROTULO,
                false,
                Al::Izq,
            );
            self.l.linea(x, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * etiquetas.len() as f32
    }

    /// Los circulitos de los dias (L M X J V S D), para marcar el de hoy.
    pub fn dias_en_circulos(&mut self, xd: f32, y: f32) {
        for (i, d) in LETRAS.iter().enumerate() {
            let cx = xd - (6 - i) as f32 * 22.0;
            self.l.circulo(cx, y, 9.0, Some(BORDE), None, 1.0);
            self.l
                .texto(cx, y - 6.5, d, 9.0, WORK_SANS, ROTULO, true, Al::Centro);
        }
    }

    pub fn fin(self, clave: &str, nombre: &str) -> FiguraGuardada {
        let cat = self.t.categoria;
        self.l.figura(clave, cat, nombre)
    }

    /// La hoja de un tramo: si es una de varias, con su pie («Hoja 2 de 4 ·
    /// 11–20») abajo a la derecha, en el margen.
    pub fn fin_de(mut self, clave: &str, nombre: &str, tr: &Tramo) -> FiguraGuardada {
        if let Some((i, n)) = tr.hoja {
            let s = format!("HOJA {i} DE {n}  ·  {}", tr.rotulo());
            let (w, h) = (self.w, self.h);
            self.l
                .texto(w - M, h - 32.0, &s, 9.0, WORK_SANS, ROTULO, true, Al::Der);
        }
        self.fin(clave, nombre)
    }
}

/// El ancho de un campo de la cabecera.
const CAMPO: f32 = 150.0;

/// Como se escribe un numero de una serie: con dos cifras como poco, para
/// que la columna no baile entre el 9 y el 10.
pub(super) fn numero(i: u32) -> String {
    format!("{i:02}")
}

/// **Que trozo de su serie lleva una hoja** (22-oct): el usuario elige una
/// plantilla y pide «del 1 al 40» (una hoja mas larga) o «de 10 en 10»
/// (cuatro hojas). La serie es lo que cuenta cada plantilla: filas del
/// cronograma, horas del diario, dias de los habitos...
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Tramo {
    pub desde: u32,
    pub hasta: u32,
    /// Las semanas del cronograma (su segunda serie).
    pub semanas: (u32, u32),
    /// `(esta, de cuantas)` si es una de varias hojas.
    pub hoja: Option<(u32, u32)>,
    /// `(ano, mes 1-12)`: el calendario de verdad (mensual y habitos).
    pub mes: Option<(i32, u32)>,
}

impl Tramo {
    pub const fn de(desde: u32, hasta: u32) -> Tramo {
        Tramo {
            desde,
            hasta,
            semanas: (1, 12),
            hoja: None,
            mes: None,
        }
    }

    pub fn n(&self) -> usize {
        (self.hasta.saturating_sub(self.desde) + 1) as usize
    }

    pub fn numeros(&self) -> std::ops::RangeInclusive<u32> {
        self.desde..=self.hasta
    }

    /// «11–20», o el mes.
    pub fn rotulo(&self) -> String {
        match self.mes {
            Some((a, m)) => format!("{} {a}", MESES[m as usize - 1]),
            None => format!("{}–{}", self.desde, self.hasta),
        }
    }
}

pub(super) const MESES: [&str; 12] = [
    "Enero",
    "Febrero",
    "Marzo",
    "Abril",
    "Mayo",
    "Junio",
    "Julio",
    "Agosto",
    "Septiembre",
    "Octubre",
    "Noviembre",
    "Diciembre",
];

/// Cuantos dias tiene un mes.
pub(super) fn dias_del_mes(a: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (a % 4 == 0 && a % 100 != 0) || a % 400 == 0 => 29,
        _ => 28,
    }
}

/// El dia de la semana, 0 el lunes (Sakamoto).
pub(super) fn dia_de_la_semana(a: i32, m: u32, d: u32) -> usize {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let a = if m < 3 { a - 1 } else { a };
    let domingo0 = (a + a / 4 - a / 100 + a / 400 + T[m as usize - 1] + d as i32).rem_euclid(7);
    ((domingo0 + 6) % 7) as usize
}

/// **Las plantillas que se alargan**: su clave, lo que cuenta su serie y su
/// tramo de serie (el del menu de figuras).
pub(super) struct Ampliable {
    pub clave: &'static str,
    pub cuenta: &'static str,
    pub defecto: (u32, u32),
    pub minimo: u32,
    pub maximo: u32,
}

pub(super) const AMPLIABLES: [Ampliable; 13] = [
    Ampliable {
        clave: "cronograma",
        cuenta: "filas",
        defecto: (1, 18),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "planificador-diario",
        cuenta: "horas",
        defecto: (6, 22),
        minimo: 0,
        maximo: 24,
    },
    Ampliable {
        clave: "planificador-mensual",
        cuenta: "meses",
        defecto: (1, 1),
        minimo: 1,
        maximo: 1,
    },
    Ampliable {
        clave: "lista-de-tareas",
        cuenta: "tareas",
        defecto: (1, 42),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "metas",
        cuenta: "pasos",
        defecto: (1, 12),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "habitos",
        cuenta: "días",
        defecto: (1, 31),
        minimo: 1,
        maximo: 366,
    },
    Ampliable {
        clave: "itinerario",
        cuenta: "días",
        defecto: (1, 3),
        minimo: 1,
        maximo: 99,
    },
    Ampliable {
        clave: "lista-numerada",
        cuenta: "números",
        defecto: (1, 30),
        minimo: 0,
        maximo: 9999,
    },
    Ampliable {
        clave: "semestre",
        cuenta: "horas",
        defecto: (7, 22),
        minimo: 0,
        maximo: 24,
    },
    Ampliable {
        clave: "asistencia",
        cuenta: "alumnos",
        defecto: (1, 30),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "storyboard-4",
        cuenta: "viñetas",
        defecto: (1, 4),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "storyboard-6",
        cuenta: "viñetas",
        defecto: (1, 6),
        minimo: 1,
        maximo: 999,
    },
    Ampliable {
        clave: "receta",
        cuenta: "pasos",
        defecto: (1, 16),
        minimo: 1,
        maximo: 999,
    },
];

/// **Una plantilla que se alarga, con ese tramo.** `None` si la clave no es
/// de ninguna.
pub(super) fn hacer(clave: &str, tr: &Tramo) -> Option<FiguraGuardada> {
    Some(match clave {
        "cronograma" => cronograma(tr),
        "planificador-diario" => diario(tr),
        "planificador-mensual" => mensual(tr),
        "lista-de-tareas" => tareas(tr),
        "metas" => metas(tr),
        "habitos" => habitos(tr),
        "itinerario" => itinerario(tr),
        "lista-numerada" => lista_numerada(tr),
        "semestre" => semestre(tr),
        "asistencia" => asistencia(tr),
        "storyboard-4" => storyboard(4, tr),
        "storyboard-6" => storyboard(6, tr),
        "receta" => receta(tr),
        _ => return None,
    })
}

/// La de serie, con su tramo de siempre.
fn de_serie(clave: &str) -> FiguraGuardada {
    let a = AMPLIABLES
        .iter()
        .find(|a| a.clave == clave)
        .expect("es de la tabla");
    hacer(clave, &Tramo::de(a.defecto.0, a.defecto.1)).expect("es de la tabla")
}

// ---------------------------------------------------------------------------

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    v.extend([
        de_serie("cronograma"),
        de_serie("planificador-diario"),
        semanal(),
        de_serie("planificador-mensual"),
        de_serie("lista-de-tareas"),
        de_serie("lista-numerada"),
        de_serie("metas"),
        de_serie("habitos"),
        comidas(),
        finanzas(),
        de_serie("itinerario"),
        viaje(),
        diario_de_viaje(),
        de_serie("semestre"),
        de_serie("asistencia"),
        curso(),
        estudio(),
        entrega(),
        mapa_mental(),
        de_serie("storyboard-4"),
        de_serie("storyboard-6"),
        de_serie("receta"),
        pentagramas(),
        diario_libre(),
        autocuidado(),
        gratitud(),
        metas_del_dia(),
        sueno(),
    ]);
}

const ANCHO: f32 = W - 2.0 * M;

// ------------------------------------------------------------- planificacion

/// **El cronograma** (el que le hizo falta al usuario para un boceto
/// preliminar): actividades, responsable y doce semanas en tres meses.
fn cronograma(tr: &Tramo) -> FiguraGuardada {
    let (s0, s1) = tr.semanas;
    let k = (s1.saturating_sub(s0) + 1) as usize;
    // Con pocas filas (una hoja de un tramo corto) se estiran hasta llenar el papel.
    let fh = ((W - 132.0 - 28.0 - 48.0 - M) / tr.n() as f32).clamp(28.0, 44.0);
    let y = 132.0;
    // Con mas de doce semanas la hoja se ensancha con columnas del mismo
    // ancho; con menos, las que hay se reparten el A4 apaisado.
    let fijo = 7.4;
    let unidad = (H - 2.0 * M) / (fijo + 0.9 * 12.0);
    let ancho = (unidad * (fijo + 0.9 * k as f32) + 2.0 * M).max(H);
    let alto = (y + fh + fh * tr.n() as f32 + 48.0 + M).max(W);
    let mut p = Pag::de_tamano(
        PLANIFICACION,
        ancho,
        alto,
        "Cronograma",
        &["Proyecto", "Inicio"],
    );
    let (x, w) = (M, ancho - 2.0 * M);
    let fijas = [("#", 0.6), ("Actividad", 5.2), ("Resp.", 1.6)];
    let semanas: Vec<String> = (s0..=s1).map(|s| format!("S{s}")).collect();
    let mut cols: Vec<(&str, f32)> = fijas.to_vec();
    cols.extend(semanas.iter().map(|s| (s.as_str(), 0.9)));
    let total: f32 = cols.iter().map(|(_, f)| f).sum();
    let ancho_fijo = w * fijo / total;
    // Los meses encima de las semanas: cuatro semanas cada uno, contadas
    // desde la primera del proyecto (la 13 es del mes 4).
    let ancho_semana = w * 0.9 / total;
    let mut j = 0;
    while j < k {
        let mes = (s0 + j as u32 - 1) / 4;
        let cuantas = (j..k)
            .take_while(|&i| (s0 + i as u32 - 1) / 4 == mes)
            .count();
        let mx = x + ancho_fijo + ancho_semana * j as f32;
        let mw = ancho_semana * cuantas as f32;
        p.l.rect(
            mx + 1.0,
            y - 26.0,
            mw - 2.0,
            24.0,
            None,
            Some(p.t.acento),
            1.0,
            false,
        );
        let rotulo = if cuantas >= 2 {
            format!("MES {}", mes + 1)
        } else {
            format!("M{}", mes + 1)
        };
        p.l.texto(
            mx + mw / 2.0,
            y - 22.0,
            &rotulo,
            10.0,
            WORK_SANS,
            BLANCO,
            true,
            Al::Centro,
        );
        j += cuantas;
    }
    let fin = p.tabla(x, y, w, &cols, tr.n(), fh);
    for (f, i) in tr.numeros().enumerate() {
        let yy = y + fh + fh * f as f32;
        p.l.texto(
            x + w * 0.3 / total,
            yy + (fh - 14.0) / 2.0,
            &numero(i),
            10.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Centro,
        );
    }
    // La leyenda.
    let ly = fin + 18.0;
    p.l.rect(x, ly, 34.0, 12.0, None, Some(p.t.acento), 1.0, true);
    p.l.texto(
        x + 42.0,
        ly - 2.0,
        "Avance",
        11.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Izq,
    );
    p.l.rect(x + 120.0, ly, 34.0, 12.0, None, Some(p.t.suave), 1.0, true);
    p.l.texto(
        x + 162.0,
        ly - 2.0,
        "Planeado",
        11.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Izq,
    );
    let r = x + 260.0;
    p.l.poli(
        &[
            (r + 7.0, ly - 1.0),
            (r + 14.0, ly + 6.0),
            (r + 7.0, ly + 13.0),
            (r, ly + 6.0),
            (r + 7.0, ly - 1.0),
        ],
        hex(0xd9822b),
        1.5,
        Some(hex(0xf0a04b)),
    );
    p.l.texto(
        r + 22.0,
        ly - 2.0,
        "Hito",
        11.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Izq,
    );
    p.fin_de("cronograma", "Cronograma", tr)
}

fn diario(tr: &Tramo) -> FiguraGuardada {
    let alto = Y0 + 30.0 + 54.0 * tr.n() as f32 + M;
    let mut p = Pag::alta(PLANIFICACION, "Planificador diario", &["Fecha"], alto);
    p.dias_en_circulos(W - M - 9.0 - 172.0, 72.0);
    let izq = 330.0;
    let y = p.seccion(M, Y0, izq, "Horario");
    for (i, h) in tr.numeros().enumerate() {
        let yy = y + 10.0 + i as f32 * 54.0;
        p.l.texto(
            M,
            yy + 4.0,
            &format!("{h:02}"),
            11.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Izq,
        );
        p.l.linea(M + 30.0, yy + 18.0, M + izq, yy + 18.0, RAYA, 1.0);
        p.l.linea_con(
            M + 30.0,
            yy + 45.0,
            M + izq,
            yy + 45.0,
            RAYA,
            1.0,
            EstiloTrazo::Punteado,
        );
    }
    let x2 = M + izq + 30.0;
    let w2 = W - M - x2;
    let y = p.seccion(x2, Y0, w2, "Meta del día");
    p.caja_suave(x2, y + 10.0, w2, 70.0, "");
    let y = p.seccion(x2, y + 100.0, w2, "Prioridades");
    let y = p.numerados(x2, y, w2, 3, 34.0);
    let y = p.seccion(x2, y + 26.0, w2, "Otras tareas");
    let y = p.casillas(x2, y, w2, 10, 32.0, &[]);
    let y = p.seccion(x2, y + 26.0, w2, "Notas");
    p.caja(x2, y + 10.0, w2, p.h - M - y - 10.0, "");
    p.fin_de("planificador-diario", "Planificador diario", tr)
}

fn semanal() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Planificador semanal", &["Semana", "Mes"]);
    let izq = 360.0;
    let y = p.seccion(M, Y0, izq, "Horario");
    let alto = (H - M - y - 12.0) / 7.0;
    for (i, d) in DIAS.iter().enumerate() {
        let yy = y + 12.0 + alto * i as f32;
        p.l.rect(M, yy, 38.0, alto - 8.0, None, Some(p.t.suave), 1.0, true);
        p.l.texto(
            M + 19.0,
            yy + 8.0,
            d,
            10.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
        p.l.rect(M, yy, izq, alto - 8.0, Some(BORDE), None, 1.25, true);
        p.renglones(M + 50.0, yy + 2.0, izq - 62.0, 3, (alto - 14.0) / 3.4);
    }
    let x2 = M + izq + 28.0;
    let w2 = W - M - x2;
    let y = p.seccion(x2, Y0, w2, "Objetivo de la semana");
    p.caja_suave(x2, y + 10.0, w2, 80.0, "");
    let y = p.seccion(x2, y + 110.0, w2, "Prioridades");
    let y = p.numerados(x2, y, w2, 4, 32.0);
    let y = p.seccion(x2, y + 24.0, w2, "Otras tareas");
    let y = p.casillas(x2, y, w2, 9, 30.0, &[]);
    let y = p.seccion(x2, y + 24.0, w2, "Hábitos");
    let celda = 22.0;
    let x0 = x2 + w2 - 7.0 * celda;
    for (i, d) in LETRAS.iter().enumerate() {
        p.l.texto(
            x0 + celda * i as f32 + celda / 2.0,
            y + 6.0,
            d,
            9.0,
            WORK_SANS,
            ROTULO,
            true,
            Al::Centro,
        );
    }
    for f in 0..6 {
        let yy = y + 26.0 + f as f32 * 28.0;
        p.l.linea(x2, yy + 20.0, x0 - 8.0, yy + 20.0, RAYA, 1.0);
        for i in 0..7 {
            p.l.circulo(
                x0 + celda * i as f32 + celda / 2.0,
                yy + 11.0,
                7.5,
                Some(BORDE),
                None,
                1.0,
            );
        }
    }
    p.fin("planificador-semanal", "Planificador semanal")
}

/// El mensual: en blanco, o con el mes de verdad (cada dia en su dia de la
/// semana, el mes y el ano escritos en la cabecera).
fn mensual(tr: &Tramo) -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Planificador mensual", &["Mes", "Año"]);
    let y = Y0;
    let cw = ANCHO / 7.0;
    p.l.rect(M, y, ANCHO, 28.0, None, Some(p.t.acento), 1.0, true);
    for (i, d) in DIAS.iter().enumerate() {
        p.l.texto(
            M + cw * i as f32 + cw / 2.0,
            y + 7.0,
            d,
            10.0,
            WORK_SANS,
            BLANCO,
            true,
            Al::Centro,
        );
    }
    // Con mes: las semanas que tenga (4 a 6) se reparten el alto de seis.
    let (primero, dias) = match tr.mes {
        Some((a, m)) => {
            p.valor(0, MESES[m as usize - 1]);
            p.valor(1, &a.to_string());
            (dia_de_la_semana(a, m, 1), dias_del_mes(a, m) as usize)
        }
        None => (0, 0),
    };
    let filas = if dias > 0 {
        (primero + dias).div_ceil(7)
    } else {
        6
    };
    let fh = 96.0 * 6.0 / filas as f32;
    let y1 = y + 36.0;
    for f in 0..filas {
        for c in 0..7 {
            let (cx, cy) = (M + cw * c as f32, y1 + fh * f as f32);
            let dia = (f * 7 + c + 1)
                .checked_sub(primero)
                .filter(|&d| d >= 1 && d <= dias);
            let fuera = dias > 0 && dia.is_none();
            let fondo = if fuera {
                Some(hex(0xf3f5f9))
            } else if c >= 5 {
                Some(hex(0xf6f8fc))
            } else {
                None
            };
            p.l.rect(
                cx + 2.0,
                cy + 2.0,
                cw - 4.0,
                fh - 4.0,
                Some(RAYA),
                fondo,
                1.0,
                true,
            );
            if fuera {
                continue;
            }
            match dia {
                Some(d) => {
                    p.l.circulo(cx + cw - 19.0, cy + 18.0, 10.0, None, Some(p.t.suave), 1.0);
                    p.l.texto(
                        cx + cw - 19.0,
                        cy + 11.0,
                        &d.to_string(),
                        10.0,
                        WORK_SANS,
                        p.t.acento,
                        true,
                        Al::Centro,
                    );
                }
                None => {
                    p.l.circulo(cx + cw - 19.0, cy + 18.0, 8.0, Some(RAYA), None, 1.0)
                }
            }
        }
    }
    let y = y1 + 96.0 * 6.0 + 22.0;
    let mitad = ANCHO / 2.0 - 14.0;
    let ya = p.seccion(M, y, mitad, "Objetivo del mes");
    p.renglones(M, ya, mitad, 2, 30.0);
    let yb = p.seccion(M, ya + 80.0, mitad, "Prioridades");
    p.numerados(M, yb, mitad, 4, 30.0);
    let x2 = M + mitad + 28.0;
    let yc = p.seccion(x2, y, mitad, "Tareas");
    p.casillas(x2, yc, mitad, 7, 30.0, &[]);
    p.fin_de("planificador-mensual", "Planificador mensual", tr)
}

/// Las tareas van numeradas en dos columnas (la primera mitad a la
/// izquierda), cada una con su casilla y su fecha.
fn tareas(tr: &Tramo) -> FiguraGuardada {
    let filas = tr.n().div_ceil(2);
    let paso = ((H - Y0 - 20.0 - 150.0 - 20.0 - M) / filas as f32).clamp(34.0, 52.0);
    let alto = Y0 + 20.0 + 150.0 + 20.0 + paso * filas as f32 + M;
    let mut p = Pag::alta(PLANIFICACION, "Lista de tareas", &["Fecha"], alto);
    let y = p.seccion(M, Y0, ANCHO, "Lo más importante");
    p.caja_suave(M, y + 10.0, ANCHO, 116.0, "");
    // Lo mas importante va con un punto y no con numeros: los numeros son
    // los de la lista de abajo.
    for i in 1..=3 {
        let yy = y + 12.0 + 34.0 * i as f32;
        p.l.circulo(M + 24.0, yy - 9.0, 5.0, None, Some(p.t.acento), 1.0);
        p.l.linea(M + 40.0, yy, M + ANCHO - 14.0, yy, RAYA, 1.0);
    }
    let y = y + 150.0;
    let mitad = ANCHO / 2.0 - 14.0;
    let mut numeros = tr.numeros();
    for (k, x) in [M, M + mitad + 28.0].into_iter().enumerate() {
        let ys = p.seccion(
            x,
            y,
            mitad,
            if k == 0 { "Pendientes" } else { "Más tareas" },
        );
        for (f, i) in numeros.by_ref().take(filas).enumerate() {
            let yy = ys + paso * (f + 1) as f32;
            p.l.texto(
                x + 2.0,
                yy - 17.0,
                &numero(i),
                10.0,
                WORK_SANS,
                p.t.acento,
                true,
                Al::Izq,
            );
            p.l.rect(
                x + 32.0,
                yy - 17.0,
                14.0,
                14.0,
                Some(BORDE),
                None,
                1.25,
                true,
            );
            p.l.linea(x + 56.0, yy, x + mitad - 60.0, yy, RAYA, 1.0);
            p.l.texto(
                x + mitad - 26.0,
                yy - 17.0,
                "/",
                11.0,
                WORK_SANS,
                RAYA,
                false,
                Al::Centro,
            );
            p.l.linea(x + mitad - 50.0, yy, x + mitad, yy, RAYA, 1.0);
        }
    }
    p.fin_de("lista-de-tareas", "Lista de tareas", tr)
}

/// **La lista numerada** (22-oct): la mas sencilla, para cualquier cosa que
/// se cuente («del 1 al 50»): el numero en su pastilla, el renglon y una
/// casilla al final.
fn lista_numerada(tr: &Tramo) -> FiguraGuardada {
    let paso = ((H - Y0 - 20.0 - M) / tr.n() as f32).clamp(32.0, 80.0);
    let alto = Y0 + 20.0 + paso * tr.n() as f32 + M;
    let mut p = Pag::alta(PLANIFICACION, "Lista numerada", &["Tema", "Fecha"], alto);
    let y = Y0 + 8.0;
    for (f, i) in tr.numeros().enumerate() {
        let yy = y + paso * f as f32;
        if f % 2 == 1 {
            p.l.rect(M, yy, ANCHO, paso, None, Some(hex(0xf7f9fc)), 1.0, false);
        }
        p.l.rect(
            M + 4.0,
            yy + paso - 26.0,
            40.0,
            20.0,
            None,
            Some(p.t.suave),
            1.0,
            true,
        );
        p.l.texto(
            M + 24.0,
            yy + paso - 23.0,
            &numero(i),
            10.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
        p.l.linea(
            M + 56.0,
            yy + paso - 8.0,
            M + ANCHO - 34.0,
            yy + paso - 8.0,
            RAYA,
            1.0,
        );
        p.l.rect(
            M + ANCHO - 20.0,
            yy + paso - 23.0,
            14.0,
            14.0,
            Some(BORDE),
            Some(BLANCO),
            1.25,
            true,
        );
    }
    p.fin_de("lista-numerada", "Lista numerada", tr)
}

fn metas(tr: &Tramo) -> FiguraGuardada {
    // Lo de la derecha: los pasos, los hitos (26 + 6 filas) y el progreso.
    let alto =
        Y0 + 20.0 + 30.0 * tr.n() as f32 + 26.0 + 20.0 + 10.0 + 26.0 + 28.0 * 6.0 + 30.0 + 60.0 + M;
    let mut p = Pag::alta(PLANIFICACION, "Metas", &["Fecha límite"], alto);
    let mitad = ANCHO / 2.0 - 14.0;
    let y = p.seccion(M, Y0, mitad, "Mi meta");
    p.caja_suave(M, y + 10.0, mitad, 170.0, "");
    let y = p.seccion(M, y + 206.0, mitad, "¿Por qué?");
    let y = p.renglones(M, y, mitad, 6, 30.0);
    let y = p.seccion(M, y + 26.0, mitad, "Motivación");
    p.renglones(M, y, mitad, 6, 30.0);
    let x2 = M + mitad + 28.0;
    let y = p.seccion(x2, Y0, mitad, "Pasos");
    let y = p.numerados_desde(x2, y, mitad, tr.desde, tr.n(), 30.0);
    let y = p.seccion(x2, y + 26.0, mitad, "Hitos");
    p.tabla(
        x2,
        y + 10.0,
        mitad,
        &[("Hito", 4.0), ("Fecha", 1.6), ("✓", 0.7)],
        6,
        28.0,
    );
    let y = p.h - M - 60.0;
    let y = p.seccion(M, y, ANCHO, "Progreso");
    let paso = ANCHO / 10.0;
    for i in 0..10 {
        p.l.rect(
            M + paso * i as f32 + 2.0,
            y + 12.0,
            paso - 4.0,
            22.0,
            Some(BORDE),
            None,
            1.0,
            true,
        );
        p.l.texto(
            M + paso * (i as f32 + 0.5),
            y + 16.0,
            &format!("{}%", (i + 1) * 10),
            9.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Centro,
        );
    }
    p.fin_de("metas", "Metas", tr)
}

/// Los habitos: una columna por dia del tramo. Con mes, cada numero lleva
/// debajo la letra de su dia de la semana y el mes va en la cabecera.
fn habitos(tr: &Tramo) -> FiguraGuardada {
    let nombre = 168.0;
    let n = tr.n();
    // Hasta 31 dias se reparten el A4; con mas, la hoja se ensancha.
    let minima = (ANCHO - nombre) / 31.0;
    let ancho = (2.0 * M + nombre + minima * n as f32).max(W);
    let mut p = Pag::de_tamano(PLANIFICACION, ancho, H, "Hábitos del mes", &["Mes"]);
    if let Some((a, m)) = tr.mes {
        p.valor(0, &format!("{} {a}", MESES[m as usize - 1]));
    }
    let y = Y0 + 8.0;
    let celda = (ancho - 2.0 * M - nombre) / n as f32;
    let lado = (celda * 0.62).clamp(13.0, 20.0);
    let cab = if tr.mes.is_some() { 40.0 } else { 28.0 };
    p.l.texto(
        M,
        y + 4.0,
        "HÁBITO",
        10.0,
        WORK_SANS,
        p.t.acento,
        true,
        Al::Izq,
    );
    for (d, i) in tr.numeros().enumerate() {
        let cx = M + nombre + celda * (d as f32 + 0.5);
        p.l.texto(
            cx,
            y + 5.0,
            &i.to_string(),
            8.5,
            WORK_SANS,
            ROTULO,
            true,
            Al::Centro,
        );
        if let Some((a, m)) = tr.mes {
            let s = dia_de_la_semana(a, m, i);
            let color = if s >= 5 { p.t.acento } else { BORDE };
            p.l.texto(
                cx,
                y + 18.0,
                LETRAS[s],
                7.5,
                WORK_SANS,
                color,
                true,
                Al::Centro,
            );
        }
    }
    let fh = 34.0;
    let ancho_util = ancho - 2.0 * M;
    for f in 0..22 {
        let yy = y + cab + fh * f as f32;
        if f % 2 == 0 {
            p.l.rect(M, yy, ancho_util, fh, None, Some(p.t.suave), 1.0, true);
        }
        for d in 0..n {
            let cx = M + nombre + celda * (d as f32 + 0.5);
            p.l.rect(
                cx - lado / 2.0,
                yy + (fh - lado) / 2.0,
                lado,
                lado,
                Some(BORDE),
                Some(BLANCO),
                1.0,
                true,
            );
        }
    }
    let y = y + cab + fh * 22.0 + 24.0;
    let y = p.seccion(M, y, ancho_util, "Notas del mes");
    p.renglones(M, y, ancho_util, if tr.mes.is_some() { 2 } else { 3 }, 30.0);
    p.fin_de("habitos", "Hábitos del mes", tr)
}

fn comidas() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Comidas de la semana", &["Semana"]);
    let cols = [
        ("", 0.9),
        ("Desayuno", 2.0),
        ("Almuerzo", 2.0),
        ("Cena", 2.0),
        ("Snack", 1.4),
    ];
    let fh = 80.0;
    let fin = p.tabla(M, Y0, ANCHO, &cols, 7, fh);
    let total: f32 = cols.iter().map(|(_, f)| f).sum();
    for (i, d) in DIAS.iter().enumerate() {
        let yy = Y0 + 26.0 + fh * i as f32;
        p.l.rect(
            M,
            yy,
            ANCHO * 0.9 / total,
            fh,
            None,
            Some(p.t.suave),
            1.0,
            false,
        );
        p.l.texto(
            M + ANCHO * 0.45 / total,
            yy + fh / 2.0 - 8.0,
            d,
            11.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
    }
    let y = fin + 26.0;
    let tercio = (ANCHO - 56.0) / 3.0;
    let ys = p.seccion(M, y, tercio * 2.0 + 28.0, "Lista de compras");
    p.casillas(M, ys, tercio, 7, 30.0, &[]);
    p.casillas(M + tercio + 28.0, ys, tercio, 7, 30.0, &[]);
    let x3 = M + 2.0 * (tercio + 28.0);
    let yn = p.seccion(x3, y, tercio, "Notas");
    p.caja(x3, yn + 10.0, tercio, H - M - yn - 10.0, "");
    p.fin("comidas", "Comidas de la semana")
}

fn finanzas() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Finanzas del mes", &["Mes"]);
    let mitad = ANCHO / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    let cols = [("Fecha", 1.0), ("Concepto", 2.6), ("Monto", 1.3)];
    let total = |p: &mut Pag, x: f32, y: f32| {
        p.l.rect(x, y, mitad, 28.0, None, Some(p.t.suave), 1.0, false);
        p.l.texto(
            x + 12.0,
            y + 7.0,
            "TOTAL",
            10.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Izq,
        );
        y + 28.0
    };
    let y = p.seccion(M, Y0, mitad, "Gastos fijos");
    let y = p.tabla(M, y + 10.0, mitad, &cols, 10, 27.0);
    let y = total(&mut p, M, y);
    let y = p.seccion(M, y + 24.0, mitad, "Gastos variables");
    let y = p.tabla(M, y + 10.0, mitad, &cols, 16, 27.0);
    total(&mut p, M, y);
    let y = p.seccion(x2, Y0, mitad, "Ingresos");
    let y = p.tabla(x2, y + 10.0, mitad, &cols, 6, 27.0);
    let y = total(&mut p, x2, y);
    let y = p.seccion(x2, y + 24.0, mitad, "Ahorro");
    let y = p.tabla(
        x2,
        y + 10.0,
        mitad,
        &[("Para", 2.6), ("Monto", 1.3)],
        4,
        27.0,
    );
    let y = total(&mut p, x2, y);
    let y = p.seccion(x2, y + 24.0, mitad, "Resumen");
    let filas = [
        "Ingresos",
        "Gastos fijos",
        "Gastos variables",
        "Ahorro",
        "Saldo",
    ];
    let fin = p.tabla(
        x2,
        y + 10.0,
        mitad,
        &[("", 2.6), ("Monto", 1.3)],
        filas.len(),
        30.0,
    );
    for (i, s) in filas.iter().enumerate() {
        let yy = fin - 30.0 * (filas.len() - i) as f32;
        p.l.texto(
            x2 + 12.0,
            yy + 8.0,
            s,
            11.0,
            WORK_SANS,
            if i == 4 { p.t.acento } else { ROTULO },
            i == 4,
            Al::Izq,
        );
    }
    p.fin("finanzas", "Finanzas del mes")
}

/// Un bloque por dia del tramo: «DIA 4», la raya para su fecha y su tabla.
fn itinerario(tr: &Tramo) -> FiguraGuardada {
    // Cada dia: rotulo 28, cabecera 30 y siete filas de 30, y 30 de aire.
    let bloque = 28.0 + 30.0 + 7.0 * 30.0 + 30.0;
    let alto = Y0 + bloque * tr.n() as f32 - 30.0 + M;
    let mut p = Pag::alta(PLANIFICACION, "Itinerario", &["Destino"], alto);
    let mut y = Y0;
    for d in tr.numeros() {
        let s = format!("DÍA {d}");
        p.l.texto(M, y, &s, 11.0, WORK_SANS, p.t.acento, true, Al::Izq);
        let tras =
            M + crate::texto::medida(
                &s,
                11.0,
                WORK_SANS,
                crate::texto::EstiloDeTexto {
                    negrita: true,
                    ..Default::default()
                },
            )
            .0 + 12.0;
        p.l.linea(tras, y + 16.0, tras + 170.0, y + 16.0, BORDE, 1.0);
        y = p.tabla(
            M,
            y + 28.0,
            ANCHO,
            &[("Hora", 1.0), ("Lugar", 2.4), ("Detalles", 3.2)],
            7,
            30.0,
        ) + 30.0;
    }
    p.fin_de("itinerario", "Itinerario", tr)
}

fn viaje() -> FiguraGuardada {
    let mut p = Pag::nueva(
        PLANIFICACION,
        "Planificador de viaje",
        &["Destino", "Fechas"],
    );
    let mitad = ANCHO / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    let ficha = |p: &mut Pag, x: f32, y: f32, filas: &[&str]| {
        let fin = p.tabla(x, y, mitad, &[("", 1.4), ("", 2.2)], filas.len(), 30.0);
        for (i, s) in filas.iter().enumerate() {
            let yy = y + 30.0 * i as f32;
            p.l.rect(
                x,
                yy,
                mitad * 1.4 / 3.6,
                30.0,
                None,
                Some(p.t.suave),
                1.0,
                false,
            );
            p.l.texto(
                x + 10.0,
                yy + 9.0,
                &s.to_uppercase(),
                9.5,
                WORK_SANS,
                p.t.acento,
                true,
                Al::Izq,
            );
        }
        p.l.rect(x, y, mitad, fin - y, Some(BORDE), None, 1.25, false);
        fin
    };
    let y = p.seccion(M, Y0, mitad, "Vuelo");
    let fin_izq = ficha(
        &mut p,
        M,
        y + 10.0,
        &[
            "Aerolínea",
            "Reserva",
            "N.º de vuelo",
            "Salida",
            "Llegada",
            "N.º de vuelo",
            "Salida",
            "Llegada",
        ],
    );
    let y = p.seccion(x2, Y0, mitad, "Alojamiento");
    let y = ficha(
        &mut p,
        x2,
        y + 10.0,
        &["Nombre", "Dirección", "Fechas", "Entrada / salida"],
    );
    let y = p.seccion(x2, y + 22.0, mitad, "Transporte");
    let fin_der = ficha(
        &mut p,
        x2,
        y + 10.0,
        &["Empresa", "Reserva", "Recoger / devolver"],
    );
    let y = fin_izq.max(fin_der) + 30.0;
    let ya = p.seccion(M, y, mitad, "Equipaje");
    p.casillas(M, ya, mitad, 16, 30.0, &[]);
    let yb = p.seccion(x2, y, mitad, "Antes de salir");
    p.casillas(x2, yb, mitad, 16, 30.0, &[]);
    p.fin("viaje", "Planificador de viaje")
}

fn diario_de_viaje() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Diario de viaje", &["Lugar", "Fechas"]);
    let alto_foto = 380.0;
    p.caja_suave(M, Y0, ANCHO, alto_foto, "Foto o dibujo");
    let y = Y0 + alto_foto + 20.0;
    let paso = 22.0;
    let (cols, filas) = ((ANCHO / paso) as usize, ((H - M - y) / paso) as usize);
    for c in 0..=cols {
        let x = M + paso * c as f32;
        p.l.linea(x, y, x, y + paso * filas as f32, hex(0xe1e7f0), 1.0);
    }
    for f in 0..=filas {
        let yy = y + paso * f as f32;
        p.l.linea(M, yy, M + paso * cols as f32, yy, hex(0xe1e7f0), 1.0);
    }
    p.fin("diario-de-viaje", "Diario de viaje")
}

// ----------------------------------------------------------------- academico

/// El horario: una fila por hora del tramo y, debajo, los cursos.
fn semestre(tr: &Tramo) -> FiguraGuardada {
    let fh = 38.0;
    let horas = tr.n();
    let alto = Y0 + 26.0 + fh * horas as f32 + 26.0 + 20.0 + 12.0 + 62.0 * 3.0 + M;
    let mut p = Pag::alta(ACADEMICO, "Horario del semestre", &["Inicio", "Fin"], alto);
    let mut cols = vec![("", 0.55)];
    cols.extend(DIAS.iter().map(|d| (*d, 1.0)));
    let fin = p.tabla(M, Y0, ANCHO, &cols, horas, fh);
    for (f, h) in tr.numeros().enumerate() {
        let yy = Y0 + 26.0 + fh * f as f32;
        p.l.texto(
            M + 18.0,
            yy + (fh - 14.0) / 2.0,
            &numero(h),
            10.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
    }
    let y = p.seccion(M, fin + 26.0, ANCHO, "Mis cursos");
    let mitad = ANCHO / 2.0 - 12.0;
    for i in 0..6 {
        let (x, yy) = (
            M + (mitad + 24.0) * (i % 2) as f32,
            y + 12.0 + 62.0 * (i / 2) as f32,
        );
        p.l.rect(x, yy, mitad, 52.0, Some(BORDE), None, 1.25, true);
        p.l.rect(x, yy, 8.0, 52.0, None, Some(p.t.acento), 1.0, false);
        p.l.texto(
            x + 18.0,
            yy + 6.0,
            &format!("Curso {}:", i + 1),
            11.0,
            WORK_SANS,
            ROTULO,
            true,
            Al::Izq,
        );
        p.l.linea(x + 18.0, yy + 26.0, x + mitad - 12.0, yy + 26.0, RAYA, 1.0);
        p.l.texto(
            x + 18.0,
            yy + 30.0,
            "Aula:",
            10.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Izq,
        );
    }
    p.fin_de("semestre", "Horario del semestre", tr)
}

/// **La lista de asistencia** (22-oct): numero, nombre y doce columnas para
/// las fechas, con la clave de lo que se marca debajo.
fn asistencia(tr: &Tramo) -> FiguraGuardada {
    let fh = ((H - Y0 - 26.0 - 50.0 - M) / tr.n() as f32).clamp(26.0, 44.0);
    let alto = Y0 + 26.0 + fh * tr.n() as f32 + 50.0 + M;
    let mut p = Pag::alta(ACADEMICO, "Lista de asistencia", &["Curso", "Mes"], alto);
    let mut cols = vec![("N.º", 0.7), ("Nombre", 4.4)];
    cols.extend(std::iter::repeat_n(("", 0.62), 12));
    let total: f32 = cols.iter().map(|(_, f)| f).sum();
    let fin = p.tabla(M, Y0, ANCHO, &cols, tr.n(), fh);
    for (f, i) in tr.numeros().enumerate() {
        let yy = Y0 + 26.0 + fh * f as f32;
        p.l.texto(
            M + ANCHO * 0.35 / total,
            yy + (fh - 14.0) / 2.0,
            &numero(i),
            10.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
    }
    let y = fin + 18.0;
    let mut x = M;
    for (marca, que) in [
        ("✓", "Presente"),
        ("F", "Falta"),
        ("T", "Tarde"),
        ("J", "Justificada"),
    ] {
        p.l.rect(x, y, 20.0, 20.0, Some(BORDE), Some(p.t.suave), 1.0, true);
        p.l.texto(
            x + 10.0,
            y + 3.0,
            marca,
            10.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Centro,
        );
        p.l.texto(
            x + 28.0,
            y + 3.0,
            que,
            10.5,
            WORK_SANS,
            ROTULO,
            false,
            Al::Izq,
        );
        x += 130.0;
    }
    p.fin_de("asistencia", "Lista de asistencia", tr)
}

fn curso() -> FiguraGuardada {
    let mut p = Pag::nueva(ACADEMICO, "Curso", &["Ciclo"]);
    let mitad = ANCHO / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    let y = p.seccion(M, Y0, mitad, "Datos");
    let y = p.datos(
        M,
        y,
        mitad,
        &[
            "Curso:",
            "Docente:",
            "Horario:",
            "Aula:",
            "Atención:",
            "Contacto:",
        ],
        32.0,
    );
    let y = p.seccion(M, y + 24.0, mitad, "Libros y materiales");
    let y = p.tabla(M, y + 6.0, mitad, &[("", 5.0), ("", 1.0)], 7, 30.0);
    let y = p.seccion(M, y + 24.0, mitad, "Fechas importantes");
    p.tabla(M, y + 6.0, mitad, &[("Fecha", 1.0), ("Qué", 3.0)], 8, 30.0);
    let y = p.seccion(x2, Y0, mitad, "Entregas");
    let y = p.tabla(
        x2,
        y + 6.0,
        mitad,
        &[("Fecha", 1.0), ("Trabajo", 3.2), ("✓", 0.6)],
        18,
        30.0,
    );
    let y = p.seccion(x2, y + 24.0, mitad, "Notas");
    p.caja(x2, y + 10.0, mitad, H - M - y - 10.0, "");
    p.fin("curso", "Curso")
}

fn estudio() -> FiguraGuardada {
    let mut p = Pag::nueva(ACADEMICO, "Sesión de estudio", &["Fecha"]);
    let mitad = ANCHO / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    let y = p.seccion(M, Y0, mitad, "Tiempo");
    let y = p.datos(M, y, mitad, &["Inicio:", "Fin:", "Total:"], 30.0);
    let y = p.seccion(M, y + 24.0, mitad, "Objetivos");
    let y = p.numerados(M, y, mitad, 4, 30.0);
    let y = p.seccion(M, y + 24.0, mitad, "Temas / capítulos");
    let y = p.renglones(M, y, mitad, 10, 30.0);
    let y = p.seccion(M, y + 24.0, mitad, "Pausas");
    p.renglones(M, y, mitad, 4, 30.0);
    let y = p.seccion(x2, Y0, mitad, "Antes de empezar");
    let y = p.casillas(
        x2,
        y,
        mitad,
        6,
        28.0,
        &[
            "Materiales listos",
            "Lugar tranquilo",
            "Sin distracciones",
            "Música de fondo",
            "Agua y algo de comer",
            "Meta clara",
        ],
    );
    let y = p.seccion(x2, y + 24.0, mitad, "Bloques de tiempo");
    let fin = p.tabla(
        x2,
        y + 8.0,
        mitad,
        &[
            ("", 1.3),
            ("15", 1.0),
            ("30", 1.0),
            ("45", 1.0),
            ("60", 1.0),
        ],
        8,
        28.0,
    );
    for i in 0..8 {
        p.l.texto(
            x2 + 10.0,
            y + 8.0 + 26.0 + 28.0 * i as f32 + 7.0,
            &format!("Hora {}", i + 1),
            10.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Izq,
        );
    }
    let y = p.seccion(x2, fin + 24.0, mitad, "Notas");
    let alto = H - M - y - 10.0;
    p.caja(x2, y + 10.0, mitad, alto, "");
    let (cx, cy) = (x2 + mitad - 56.0, y + 10.0 + alto - 56.0);
    p.l.circulo(cx, cy, 44.0, Some(p.t.acento), Some(p.t.suave), 1.5);
    p.l.texto(
        cx,
        cy - 8.0,
        "MI PREMIO",
        10.0,
        WORK_SANS,
        p.t.acento,
        true,
        Al::Centro,
    );
    p.fin("sesion-de-estudio", "Sesión de estudio")
}

fn entrega() -> FiguraGuardada {
    let mut p = Pag::nueva(ACADEMICO, "Planificador de tareas", &["Curso"]);
    let izq = 300.0;
    let y = p.seccion(M, Y0, izq, "Detalles");
    let y = p.datos(M, y, izq, &["Tarea:", "Inicio:", "Entrega:"], 32.0);
    let y = p.seccion(M, y + 24.0, izq, "Recursos");
    let y = p.renglones(M, y, izq, 5, 30.0);
    let y = p.seccion(M, y + 24.0, izq, "Pasos");
    p.casillas(M, y, izq, 15, 30.0, &[]);
    let x2 = M + izq + 28.0;
    let w2 = W - M - x2;
    let y = p.seccion(x2, Y0, w2, "Espacio libre (bocetos, notas, ideas)");
    let alto = H - M - y - 10.0;
    p.l.rect(x2, y + 10.0, w2, alto, Some(BORDE), None, 1.25, true);
    let paso = 24.0;
    let (nc, nf) = ((w2 / paso) as usize, (alto / paso) as usize);
    let (ox, oy) = (
        x2 + (w2 - paso * (nc - 1) as f32) / 2.0,
        y + 10.0 + (alto - paso * (nf - 1) as f32) / 2.0,
    );
    for c in 0..nc {
        for f in 0..nf {
            p.l.circulo(
                ox + paso * c as f32,
                oy + paso * f as f32,
                1.3,
                None,
                Some(hex(0xb7c2d2)),
                1.0,
            );
        }
    }
    p.fin("planificador-de-tareas", "Planificador de tareas")
}

fn mapa_mental() -> FiguraGuardada {
    let mut p = Pag::apaisada(ACADEMICO, "Mapa mental", &["Tema"]);
    let (cx, cy) = (H / 2.0, 120.0 + (W - 120.0) / 2.0);
    let acento = p.t.acento;
    // Las hojas de los bordes y las ramas.
    let tarjeta = |p: &mut Pag, x: f32, y: f32| {
        p.l.rect(x, y, 150.0, 86.0, Some(BORDE), Some(BLANCO), 1.25, true);
        p.l.rect(x, y, 150.0, 22.0, None, Some(p.t.suave), 1.0, true);
    };
    let sub = [
        (cx - 250.0, cy - 150.0),
        (cx + 100.0, cy - 150.0),
        (cx - 250.0, cy + 110.0),
        (cx + 100.0, cy + 110.0),
    ];
    let hojas = [
        [(M, 140.0), (M, 270.0)],
        [(H - M - 150.0, 140.0), (H - M - 150.0, 270.0)],
        [(M, W - M - 216.0), (M, W - M - 86.0)],
        [
            (H - M - 150.0, W - M - 216.0),
            (H - M - 150.0, W - M - 86.0),
        ],
    ];
    for (k, (sx, sy)) in sub.iter().enumerate() {
        let (scx, scy) = (sx + 75.0, sy + 20.0);
        p.l.poli(&[(cx, cy), (cx, scy), (scx, scy)], acento, 1.5, None);
        for (hx, hy) in hojas[k] {
            let (ex, ey) = if hx < cx {
                (hx + 150.0, hy + 43.0)
            } else {
                (hx, hy + 43.0)
            };
            let medio = if hx < cx { ex + 30.0 } else { ex - 30.0 };
            p.l.poli(
                &[
                    (if hx < cx { *sx } else { sx + 150.0 }, scy),
                    (medio, scy),
                    (medio, ey),
                    (ex, ey),
                ],
                BORDE,
                1.25,
                None,
            );
            tarjeta(&mut p, hx, hy);
        }
    }
    for (sx, sy) in sub {
        p.l.rect(
            sx,
            sy,
            150.0,
            40.0,
            Some(acento),
            Some(p.t.suave),
            1.5,
            true,
        );
        p.l.texto(
            sx + 75.0,
            sy + 12.0,
            "SUBTEMA",
            10.0,
            WORK_SANS,
            acento,
            true,
            Al::Centro,
        );
    }
    p.l.rect(
        cx - 110.0,
        cy - 34.0,
        220.0,
        68.0,
        None,
        Some(acento),
        1.0,
        true,
    );
    p.l.texto(
        cx,
        cy - 9.0,
        "TEMA CENTRAL",
        13.0,
        WORK_SANS,
        BLANCO,
        true,
        Al::Centro,
    );
    p.fin("mapa-mental", "Mapa mental")
}

// ------------------------------------------------------------------ creativo

/// El ancho de las viñetas del storyboard de 4 (por el se sabe cual de los
/// dos es el que esta elegido: los dos se titulan igual).
pub(super) const VINETA_DE_4: f32 = 340.0;

fn storyboard(n: usize, tr: &Tramo) -> FiguraGuardada {
    let acento = CREATIVO.acento;
    let fw6 = (ANCHO - 28.0) / 2.0;
    let fh6 = fw6 * 9.0 / 16.0;
    let alto = if n == 4 {
        Y0 + 8.0 + tr.n() as f32 * (191.0 + 34.0) - 34.0 + M
    } else {
        Y0 + 8.0 + tr.n().div_ceil(2) as f32 * (fh6 + 110.0) - 26.0 + M
    };
    let mut p = Pag::alta(CREATIVO, "Storyboard", &["Proyecto", "Página"], alto);
    if let Some((i, _)) = tr.hoja {
        p.valor(1, &i.to_string());
    }
    let numero = |p: &mut Pag, x: f32, y: f32, i: u32| {
        let r = if i >= 100 { 15.0 } else { 12.0 };
        p.l.circulo(x + 16.0, y + 16.0, r, None, Some(acento), 1.0);
        p.l.texto(
            x + 16.0,
            y + 8.5,
            &format!("{i}"),
            11.0,
            WORK_SANS,
            BLANCO,
            true,
            Al::Centro,
        );
    };
    if n == 4 {
        let (fw, fh) = (VINETA_DE_4, 191.0);
        for (i, v) in tr.numeros().enumerate() {
            let y = Y0 + 8.0 + i as f32 * (fh + 34.0);
            p.l.rect(M, y, fw, fh, Some(BORDE), Some(hex(0xfcfcfd)), 1.5, false);
            numero(&mut p, M + 8.0, y + 8.0, v);
            let x2 = M + fw + 26.0;
            let w2 = W - M - x2;
            p.l.texto(x2, y, "ESCENA", 10.0, WORK_SANS, acento, true, Al::Izq);
            p.l.linea(x2 + 52.0, y + 15.0, x2 + 120.0, y + 15.0, BORDE, 1.0);
            p.l.texto(
                x2 + 140.0,
                y,
                "TOMA",
                10.0,
                WORK_SANS,
                acento,
                true,
                Al::Izq,
            );
            p.l.linea(x2 + 180.0, y + 15.0, x2 + w2, y + 15.0, BORDE, 1.0);
            p.renglones(x2, y + 18.0, w2, 6, 28.0);
        }
    } else {
        let (fw, fh) = (fw6, fh6);
        for (i, v) in tr.numeros().enumerate() {
            let (x, y) = (
                M + (fw + 28.0) * (i % 2) as f32,
                Y0 + 8.0 + (fh + 110.0) * (i / 2) as f32,
            );
            p.l.rect(x, y, fw, fh, Some(BORDE), Some(hex(0xfcfcfd)), 1.5, false);
            numero(&mut p, x + 8.0, y + 8.0, v);
            p.renglones(x, y + fh, fw, 3, 28.0);
        }
    }
    p.fin_de(
        &format!("storyboard-{n}"),
        &format!("Storyboard de {n} viñetas"),
        tr,
    )
}

/// La receta: los pasos del tramo a la derecha y, a la izquierda, tantos
/// ingredientes como pasos (16 como poco).
fn receta(tr: &Tramo) -> FiguraGuardada {
    let filas = tr.n().max(16);
    let alto = Y0 + 260.0 + 20.0 + 30.0 * filas as f32 + 26.0 + 20.0 + 10.0 + 120.0 + M;
    let mut p = Pag::alta(CREATIVO, "Receta", &["Categoría"], alto);
    let mitad = ANCHO / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    p.caja_suave(M, Y0, mitad, 230.0, "Foto");
    let y = p.datos(x2, Y0 - 6.0, mitad, &["Nombre:"], 34.0);
    let y = p.seccion(x2, y + 22.0, mitad, "Detalles");
    let fin = p.tabla(x2, y + 8.0, mitad, &[("", 1.4), ("", 2.0)], 3, 32.0);
    for (i, s) in ["Preparación", "Cocción", "Porciones"].iter().enumerate() {
        let yy = y + 8.0 + 32.0 * i as f32;
        p.l.rect(
            x2,
            yy,
            mitad * 1.4 / 3.4,
            32.0,
            None,
            Some(p.t.suave),
            1.0,
            false,
        );
        p.l.texto(
            x2 + 10.0,
            yy + 10.0,
            &s.to_uppercase(),
            9.5,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Izq,
        );
    }
    p.l.rect(
        x2,
        y + 8.0,
        mitad,
        fin - y - 8.0,
        Some(BORDE),
        None,
        1.25,
        false,
    );
    let y = Y0 + 260.0;
    let ya = p.seccion(M, y, mitad, "Ingredientes");
    p.casillas(M, ya, mitad, filas, 30.0, &[]);
    let yb = p.seccion(x2, y, mitad, "Preparación");
    p.numerados_desde(x2, yb, mitad, tr.desde, tr.n(), 30.0);
    let y = ya + filas as f32 * 30.0 + 26.0;
    let y = p.seccion(M, y, ANCHO, "Notas");
    p.caja(M, y + 10.0, ANCHO, p.h - M - y - 10.0, "");
    p.fin_de("receta", "Receta", tr)
}

fn pentagramas() -> FiguraGuardada {
    let mut p = Pag::nueva(CREATIVO, "Partitura", &["Título", "Autor"]);
    let mut y = Y0 + 20.0;
    while y + 40.0 < H - M {
        for k in 0..5 {
            let yy = y + 9.0 * k as f32;
            p.l.linea(M, yy, W - M, yy, hex(0x8f9bb0), 1.0);
        }
        p.l.linea(M, y, M, y + 36.0, hex(0x8f9bb0), 1.5);
        p.l.linea(W - M, y, W - M, y + 36.0, hex(0x8f9bb0), 1.5);
        y += 82.0;
    }
    p.fin("pentagramas", "Partitura")
}

// -------------------------------------------------------------------- diario

/// La fila del animo: cinco caritas para marcar una.
fn animo(p: &mut Pag, x: f32, y: f32) {
    for i in 0..5 {
        let cx = x + i as f32 * 30.0;
        p.l.circulo(cx, y, 11.0, Some(p.t.acento), None, 1.25);
        p.l.circulo(cx - 4.0, y - 3.0, 1.2, None, Some(p.t.acento), 1.0);
        p.l.circulo(cx + 4.0, y - 3.0, 1.2, None, Some(p.t.acento), 1.0);
        let curva = [-4.0, -2.0, 0.0, 2.0, 4.0][i];
        p.l.poli(
            &[
                (cx - 5.0, y + 4.0),
                (cx, y + 4.0 + curva * 0.8),
                (cx + 5.0, y + 4.0),
            ],
            p.t.acento,
            1.25,
            None,
        );
    }
}

fn diario_libre() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Diario", &["Fecha"]);
    p.l.texto(
        M,
        Y0,
        "¿CÓMO ME SIENTO?",
        10.0,
        WORK_SANS,
        p.t.acento,
        true,
        Al::Izq,
    );
    animo(&mut p, M + 150.0, Y0 + 7.0);
    p.renglones(M, Y0 + 24.0, ANCHO, 30, 31.0);
    p.fin("diario", "Diario")
}

fn autocuidado() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Autocuidado", &["Fecha"]);
    let acento = p.t.acento;
    let cuarto = (ANCHO - 36.0) / 4.0;
    for (i, s) in ["Ánimo", "Clima", "Comidas", "Agua"].iter().enumerate() {
        let x = M + (cuarto + 12.0) * i as f32;
        p.caja_suave(x, Y0, cuarto, 74.0, s);
        let y = Y0 + 50.0;
        match i {
            0 => animo(&mut p, x + 24.0, y),
            1 => {
                p.l.circulo(x + 26.0, y, 9.0, Some(acento), None, 1.25);
                for k in 0..8 {
                    let a = (k as f32 * 45.0).to_radians();
                    p.l.linea(
                        x + 26.0 + 12.0 * a.cos(),
                        y + 12.0 * a.sin(),
                        x + 26.0 + 16.0 * a.cos(),
                        y + 16.0 * a.sin(),
                        acento,
                        1.25,
                    );
                }
                p.l.arco(x + 70.0, y + 2.0, 9.0, 180.0, 360.0, acento, 1.25);
                p.l.arco(x + 82.0, y - 2.0, 11.0, 200.0, 360.0, acento, 1.25);
                p.l.linea(x + 61.0, y + 2.0, x + 93.0, y + 2.0, acento, 1.25);
                for k in 0..3 {
                    let dx = x + 118.0 + k as f32 * 8.0;
                    p.l.linea(dx, y - 6.0, dx - 4.0, y + 6.0, acento, 1.25);
                }
            }
            2 => {
                for k in 0..3 {
                    p.l.circulo(
                        x + 26.0 + k as f32 * 34.0,
                        y,
                        11.0,
                        Some(acento),
                        None,
                        1.25,
                    );
                }
            }
            _ => {
                for k in 0..8 {
                    let dx = x + 18.0 + k as f32 * 18.0;
                    p.l.poli(
                        &[
                            (dx, y - 10.0),
                            (dx + 6.0, y),
                            (dx + 4.0, y + 6.0),
                            (dx, y + 8.0),
                            (dx - 4.0, y + 6.0),
                            (dx - 6.0, y),
                            (dx, y - 10.0),
                        ],
                        acento,
                        1.25,
                        None,
                    );
                }
            }
        }
    }
    let mut y = Y0 + 104.0;
    let preguntas: [(&str, usize); 9] = [
        ("¿Hablé con alguien hoy?", 0),
        ("¿Cuánta energía social tengo?", 0),
        ("¿Cuánto tiempo pasé con el móvil?", 0),
        ("¿Hice ejercicio? ¿Cuánto?", 0),
        ("¿Dormí una siesta?", 0),
        ("¿Comí bien hoy?", 0),
        ("¿Qué comí?", 2),
        ("¿Cómo se siente mi cuerpo?", 3),
        ("¿Tomé mis medicinas?", 0),
    ];
    for (s, extra) in preguntas {
        p.l.circulo(M + 5.0, y + 8.0, 4.0, None, Some(acento), 1.0);
        p.l.texto(
            M + 16.0,
            y,
            s,
            12.0,
            WORK_SANS,
            hex(0x4a3f48),
            true,
            Al::Izq,
        );
        p.l.texto(
            W - M - 70.0,
            y + 1.0,
            "SÍ    NO",
            10.0,
            WORK_SANS,
            acento,
            true,
            Al::Izq,
        );
        p.l.linea(M + 16.0, y + 26.0, W - M - 90.0, y + 26.0, RAYA, 1.0);
        y += 40.0;
        if extra > 0 {
            y = p.renglones(M + 16.0, y - 14.0, ANCHO - 16.0, extra, 30.0) + 18.0;
        }
    }
    let y = p.seccion(M, y + 6.0, ANCHO, "Medicinas");
    p.tabla(M, y + 10.0, ANCHO, &[("", 1.0); 8], 2, 34.0);
    p.fin("autocuidado", "Autocuidado")
}

fn gratitud() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Gratitud", &["Fecha"]);
    let izq = 400.0;
    let x2 = M + izq + 24.0;
    let w2 = W - M - x2;
    let alto = (H - M - Y0 - 2.0 * 20.0) / 3.0;
    for (i, (s, d)) in [
        ("Agradezco", "Dibuja lo que te hace feliz"),
        ("Mis logros", "Dibuja un logro"),
        ("Lo mejor del día", "Dibuja algo que te hizo sonreír"),
    ]
    .iter()
    .enumerate()
    {
        let y = Y0 + (alto + 20.0) * i as f32;
        let ys = p.seccion(M, y, izq, s);
        p.renglones(M, ys, izq, ((alto - 30.0) / 29.0) as usize, 29.0);
        p.caja(x2, y, w2, alto, "");
        p.l.texto(
            x2 + w2 / 2.0,
            y + 10.0,
            d,
            15.0,
            CAVEAT,
            p.t.acento,
            true,
            Al::Centro,
        );
    }
    p.fin("gratitud", "Gratitud")
}

fn metas_del_dia() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Metas del día", &["Fecha"]);
    let alto = (H - M - Y0 - 2.0 * 18.0) / 3.0;
    for (i, s) in [
        "Intención de la mañana",
        "Afirmaciones del mediodía",
        "Reflexión de la noche",
    ]
    .iter()
    .enumerate()
    {
        let y = Y0 + (alto + 18.0) * i as f32;
        p.l.rect(M, y, ANCHO, alto, Some(BORDE), Some(BLANCO), 1.25, true);
        p.l.rect(M, y, ANCHO, 34.0, None, Some(p.t.suave), 1.0, true);
        p.l.circulo(M + 20.0, y + 17.0, 7.0, None, Some(p.t.acento), 1.0);
        p.l.texto(
            M + 36.0,
            y + 9.0,
            &s.to_uppercase(),
            11.0,
            WORK_SANS,
            p.t.acento,
            true,
            Al::Izq,
        );
    }
    p.fin("metas-del-dia", "Metas del día")
}

fn sueno() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Sueño", &["Fecha"]);
    let acento = p.t.acento;
    let izq = ANCHO - 90.0;
    let y = p.datos(M, Y0 - 6.0, izq / 2.0 - 12.0, &["Me acosté:"], 32.0);
    p.datos(
        M + izq / 2.0 + 12.0,
        Y0 - 6.0,
        izq / 2.0 - 12.0,
        &["Me levanté:"],
        32.0,
    );
    let mut y = y + 20.0;
    let preguntas: [(&str, usize); 9] = [
        ("¿Seguí mi horario de siempre?", 0),
        ("¿Cómo dormí?", 0),
        ("¿Cómo fue el lugar? (ruido, luz, temperatura)", 2),
        ("¿Miré el móvil antes de dormir?", 0),
        ("¿Comí algo antes de acostarme?", 0),
        ("¿Me desperté en la noche? ¿Cuántas veces?", 0),
        ("¿Soñé? ¿Con qué?", 4),
        ("¿Me sentí descansado al despertar?", 0),
        ("Notas", 4),
    ];
    for (s, extra) in preguntas {
        p.l.poli(
            &[
                (M + 5.0, y + 3.0),
                (M + 9.0, y + 8.0),
                (M + 5.0, y + 13.0),
                (M + 1.0, y + 8.0),
                (M + 5.0, y + 3.0),
            ],
            acento,
            1.0,
            Some(acento),
        );
        p.l.texto(
            M + 16.0,
            y,
            s,
            12.0,
            WORK_SANS,
            hex(0x4a3f48),
            true,
            Al::Izq,
        );
        p.l.linea(M + 16.0, y + 26.0, M + izq, y + 26.0, RAYA, 1.0);
        y += 40.0;
        if extra > 0 {
            y = p.renglones(M + 16.0, y - 14.0, izq - 16.0, extra, 30.0) + 18.0;
        }
    }
    // Las horas dormidas, en una columna a la derecha.
    let x = W - M - 40.0;
    p.l.rect(
        x - 10.0,
        Y0 - 6.0,
        60.0,
        500.0,
        None,
        Some(p.t.suave),
        1.0,
        true,
    );
    p.l.texto(
        x + 20.0,
        Y0 + 2.0,
        "HORAS",
        9.5,
        WORK_SANS,
        acento,
        true,
        Al::Centro,
    );
    for h in 0..12 {
        let cy = Y0 + 38.0 + h as f32 * 37.0;
        p.l.circulo(x + 20.0, cy, 12.0, Some(acento), Some(BLANCO), 1.25);
        p.l.texto(
            x + 20.0,
            cy - 7.0,
            &format!("{}", h + 1),
            10.0,
            WORK_SANS,
            acento,
            true,
            Al::Centro,
        );
    }
    p.fin("sueno", "Sueño")
}
