//! **EXPORTPDF**: una [`Escena`] a un PDF vectorial, una pagina por hoja
//! (todo el dibujo, una ventana o cada lamina), con el papel y la escala
//! pedidos, el grosor de cada linea en el papel, en color o monocromo, los
//! textos como texto de PDF (Helvetica) o como rayas, y cada capa como un
//! grupo de contenido opcional (las capas del visor de PDF).

use std::fmt::Write as _;
use std::io::Write as _;

use crate::formatos::escena::{Escena, Pieza, TextoExp};
use crate::formatos::{Rgb, shx};
use crate::geo::{self, Caja, Forma, P};

/// Puntos por mm.
const PT_POR_MM: f64 = 72.0 / 25.4;
/// El margen del papel al ajustar (mm).
pub const MARGEN: f64 = 10.0;
/// El papel mas grande que se escribe (mm; el tope de PDF es 5080).
pub const PAPEL_MAXIMO: f64 = 5000.0;

/// **Que papel.**
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Papel {
    /// Del tamano de lo que se exporta, con su margen.
    Ajustar,
    /// Un formato en mm (se gira para que quepa mejor).
    Formato(f64, f64),
}

/// **Una hoja del PDF**: la zona del dibujo que va, el papel (mm), cuantos
/// mm de papel es una unidad y donde cae la esquina de la zona (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hoja {
    pub zona: Caja,
    pub ancho: f64,
    pub alto: f64,
    pub k: f64,
    pub desplazamiento: P,
}

/// **Como se exporta.**
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OpcionesPdf {
    /// Todo en negro (como `monochrome.ctb`).
    pub monocromo: bool,
    /// Los textos como rayas (la romans) en vez de texto de PDF.
    pub trazos: bool,
    /// Sin grupos de contenido opcional (un PDF sin capas).
    pub sin_capas: bool,
}

/// **La hoja para una zona**: en `papel`, a `k` mm por unidad (o lo que
/// quepa si `None`), centrada.
pub fn hoja_para(zona: Caja, papel: Papel, k: Option<f64>) -> Hoja {
    let mut zona = zona;
    if zona[2] - zona[0] < 1e-9 {
        zona[0] -= 0.5;
        zona[2] += 0.5;
    }
    if zona[3] - zona[1] < 1e-9 {
        zona[1] -= 0.5;
        zona[3] += 0.5;
    }
    let (w, h) = (zona[2] - zona[0], zona[3] - zona[1]);
    match papel {
        Papel::Formato(a, b) => {
            let (ancho, alto) = if w >= h {
                (a.max(b), a.min(b))
            } else {
                (a.min(b), a.max(b))
            };
            let k = k
                .filter(|k| *k > 0.0 && k.is_finite())
                .unwrap_or_else(|| ((ancho - 2.0 * MARGEN) / w).min((alto - 2.0 * MARGEN) / h));
            Hoja {
                zona,
                ancho,
                alto,
                k,
                desplazamiento: [(ancho - w * k) / 2.0, (alto - h * k) / 2.0],
            }
        }
        Papel::Ajustar => {
            let mut k = k.filter(|k| *k > 0.0 && k.is_finite()).unwrap_or(1.0);
            let maximo = (PAPEL_MAXIMO - 2.0 * MARGEN) / w.max(h);
            if k > maximo {
                k = maximo;
            }
            Hoja {
                zona,
                ancho: w * k + 2.0 * MARGEN,
                alto: h * k + 2.0 * MARGEN,
                k,
                desplazamiento: [MARGEN, MARGEN],
            }
        }
    }
}

/// **La hoja de una lamina**: el marco ocupa todo el papel. Si el marco
/// tiene la forma de un formato (A4…A0), ese es el papel (de los que
/// encajan, el de la escala mas parecida a `k_defecto`, porque todos los A
/// tienen la misma forma); si no, el marco a `k_defecto` mm por unidad.
pub fn hoja_de_lamina(marco: Caja, k_defecto: f64) -> Hoja {
    let (w, h) = (marco[2] - marco[0], marco[3] - marco[1]);
    let mejor = crate::especialidad::FORMATOS
        .iter()
        .filter_map(|(_, a, b)| {
            let (ancho, alto) = if w >= h { (*a, *b) } else { (*b, *a) };
            let k = ancho / w;
            ((alto / h - k).abs() < k * 0.005).then_some((ancho, alto, k))
        })
        .min_by(|x, y| {
            let d = |k: f64| (k / k_defecto).ln().abs();
            d(x.2)
                .partial_cmp(&d(y.2))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    if let Some((ancho, alto, k)) = mejor {
        return Hoja {
            zona: marco,
            ancho,
            alto,
            k,
            desplazamiento: [0.0, 0.0],
        };
    }
    let k = k_defecto.min(PAPEL_MAXIMO / w.max(h));
    Hoja {
        zona: marco,
        ancho: w * k,
        alto: h * k,
        k,
        desplazamiento: [0.0, 0.0],
    }
}

/// Los anchos de Helvetica (1/1000 de em) de ` ` a `~`.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];
/// La altura de las mayusculas de Helvetica (ems).
pub const MAYUS_HELVETICA: f64 = 0.718;

/// **Lo que mide un texto en Helvetica**, en alturas de mayuscula.
pub fn largo_helvetica(s: &str) -> f64 {
    let ems: f64 = s
        .chars()
        .map(|c| {
            let i = c as u32;
            if (32..127).contains(&i) {
                HELVETICA[(i - 32) as usize] as f64
            } else {
                556.0
            }
        })
        .sum::<f64>()
        / 1000.0;
    ems / MAYUS_HELVETICA
}

/// Un numero corto para el PDF.
pub fn num(v: f64) -> String {
    let v = if v.abs() < 5e-5 { 0.0 } else { v };
    let mut s = format!("{v:.4}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" { "0".into() } else { s }
}

/// Un texto en WinAnsi, como cadena literal de PDF.
fn cadena_pdf(s: &str) -> String {
    let mut out = String::from("(");
    for c in s.chars() {
        let b: u8 = match c as u32 {
            0x20..=0x7E | 0xA0..=0xFF => c as u32 as u8,
            _ => match super::importar::WIN_80.chars().position(|w| w == c) {
                Some(i) => 0x80 + i as u8,
                None => b'?',
            },
        };
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            0x20..=0x7E => out.push(b as char),
            otro => {
                let _ = write!(out, "\\{otro:03o}");
            }
        }
    }
    out.push(')');
    out
}

/// Un texto de PDF (UTF-16 con BOM si hace falta), en hexadecimal.
fn texto_hex(s: &str) -> String {
    let mut out = String::from("<FEFF");
    for u in s.encode_utf16() {
        let _ = write!(out, "{u:04X}");
    }
    out.push('>');
    out
}

/// Los Bezier de un arco (de a lo sumo un cuarto de vuelta cada uno).
pub fn arco_a_bezier(c: P, r: f64, inicio: f64, barrido: f64) -> Vec<[P; 4]> {
    let n = ((barrido.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).max(1);
    let paso = barrido / n as f64;
    let k = 4.0 / 3.0 * (paso / 4.0).tan();
    (0..n)
        .map(|i| {
            let a0 = inicio + paso * i as f64;
            let a1 = a0 + paso;
            let p0 = geo::polar(c, r, a0);
            let p3 = geo::polar(c, r, a1);
            let p1 = [p0[0] - k * r * a0.sin(), p0[1] + k * r * a0.cos()];
            let p2 = [p3[0] + k * r * a1.sin(), p3[1] - k * r * a1.cos()];
            [p0, p1, p2, p3]
        })
        .collect()
}

/// El contenido de una hoja.
struct Pagina<'a> {
    s: String,
    hoja: &'a Hoja,
    f: f64,
    o: P,
    trazo: Option<Rgb>,
    relleno: Option<Rgb>,
    grosor: Option<f64>,
    /// Donde quedo la pluma (para seguir sin `m`).
    pluma: Option<P>,
}

impl Pagina<'_> {
    fn p(&self, q: P) -> P {
        [
            (q[0] - self.hoja.zona[0]) * self.f + self.o[0],
            (q[1] - self.hoja.zona[1]) * self.f + self.o[1],
        ]
    }

    fn mover(&mut self, q: P) {
        let t = self.p(q);
        if self.pluma.is_some_and(|a| geo::dist(a, t) < 1e-4) {
            return;
        }
        let _ = writeln!(self.s, "{} {} m", num(t[0]), num(t[1]));
        self.pluma = Some(t);
    }

    fn linea(&mut self, q: P) {
        let t = self.p(q);
        let _ = writeln!(self.s, "{} {} l", num(t[0]), num(t[1]));
        self.pluma = Some(t);
    }

    fn curva(&mut self, b: [P; 4]) {
        let (p1, p2, p3) = (self.p(b[1]), self.p(b[2]), self.p(b[3]));
        let _ = writeln!(
            self.s,
            "{} {} {} {} {} {} c",
            num(p1[0]),
            num(p1[1]),
            num(p2[0]),
            num(p2[1]),
            num(p3[0]),
            num(p3[1])
        );
        self.pluma = Some(p3);
    }

    fn forma(&mut self, f: &Forma) {
        match *f {
            Forma::Segmento { a, b } => {
                self.mover(a);
                self.linea(b);
            }
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } => {
                let bs = arco_a_bezier(c, r, inicio, barrido);
                if let Some(b) = bs.first() {
                    self.mover(b[0]);
                }
                for b in bs {
                    self.curva(b);
                }
            }
            Forma::Circulo { c, r } => {
                let bs = arco_a_bezier(c, r, 0.0, std::f64::consts::TAU);
                self.pluma = None;
                self.mover(bs[0][0]);
                for b in bs {
                    self.curva(b);
                }
                self.s.push_str("h\n");
            }
        }
    }

    fn color_trazo(&mut self, c: Rgb) {
        if self.trazo != Some(c) {
            let _ = writeln!(self.s, "{} RG", rgb(c));
            self.trazo = Some(c);
        }
    }

    fn color_relleno(&mut self, c: Rgb) {
        if self.relleno != Some(c) {
            let _ = writeln!(self.s, "{} rg", rgb(c));
            self.relleno = Some(c);
        }
    }

    fn grosor(&mut self, mm: f64) {
        let w = mm.max(0.0) * PT_POR_MM;
        if self.grosor.is_none_or(|g| (g - w).abs() > 1e-6) {
            let _ = writeln!(self.s, "{} w", num(w));
            self.grosor = Some(w);
        }
    }

    fn texto(&mut self, t: &TextoExp, color: Rgb, grosor: f64, trazos: bool) {
        if trazos {
            let o = t.origen(shx::largo(&t.texto));
            let rayas = shx::rayas(&t.texto, o, t.alto, t.giro, t.ancho);
            if rayas.is_empty() {
                return;
            }
            self.color_trazo(color);
            self.grosor(grosor);
            for r in rayas {
                self.pluma = None;
                if let Some(a) = r.first() {
                    self.mover(*a);
                }
                for q in &r[1..] {
                    self.linea(*q);
                }
            }
            self.s.push_str("S\n");
            self.pluma = None;
            return;
        }
        let o = t.origen(largo_helvetica(&t.texto));
        let p = self.p(o);
        let tam = t.alto * self.f / MAYUS_HELVETICA;
        if tam < 0.01 {
            return;
        }
        let (s, c) = t.giro.sin_cos();
        self.color_relleno(color);
        let _ = writeln!(
            self.s,
            "BT /F1 {} Tf {} {} {} {} {} {} Tm {} Tj ET",
            num(tam),
            num(c * t.ancho),
            num(s * t.ancho),
            num(-s),
            num(c),
            num(p[0]),
            num(p[1]),
            cadena_pdf(&t.texto)
        );
    }
}

fn rgb(c: Rgb) -> String {
    format!(
        "{} {} {}",
        num(c[0] as f64 / 255.0),
        num(c[1] as f64 / 255.0),
        num(c[2] as f64 / 255.0)
    )
}

/// **Escribe el PDF** de la escena, una pagina por hoja.
pub fn escribir(escena: &Escena, hojas: &[Hoja], op: &OpcionesPdf, titulo: &str) -> Vec<u8> {
    let capas = if op.sin_capas {
        Vec::new()
    } else {
        escena.capas()
    };
    // Los numeros de objeto: 1 catalogo, 2 paginas, 3 fuente, 4 info, luego
    // las capas y luego cada pagina con su contenido.
    let primera_capa = 5;
    let primera_hoja = primera_capa + capas.len();
    let mut objetos: Vec<Vec<u8>> = Vec::new();
    let kids: Vec<String> = (0..hojas.len())
        .map(|i| format!("{} 0 R", primera_hoja + 2 * i))
        .collect();
    let refs_capas: Vec<String> = (0..capas.len())
        .map(|i| format!("{} 0 R", primera_capa + i))
        .collect();
    let oc = if capas.is_empty() {
        String::new()
    } else {
        format!(
            " /OCProperties << /OCGs [{0}] /D << /Order [{0}] /ON [{0}] >> >> /PageMode /UseOC",
            refs_capas.join(" ")
        )
    };
    objetos.push(format!("<< /Type /Catalog /Pages 2 0 R{oc} >>").into_bytes());
    objetos.push(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            hojas.len()
        )
        .into_bytes(),
    );
    objetos.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    objetos.push(
        format!(
            "<< /Producer (CAD-pixpin) /Creator (CAD-pixpin) /Title {} >>",
            texto_hex(titulo)
        )
        .into_bytes(),
    );
    for c in &capas {
        objetos.push(format!("<< /Type /OCG /Name {} >>", texto_hex(c)).into_bytes());
    }
    let propiedades: String = (0..capas.len())
        .map(|i| format!("/OC{i} {} 0 R ", primera_capa + i))
        .collect();
    for (i, hoja) in hojas.iter().enumerate() {
        let contenido = contenido_de_hoja(escena, hoja, op, &capas);
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        let _ = z.write_all(contenido.as_bytes());
        let comprimido = z.finish().unwrap_or_default();
        let (w, h) = (hoja.ancho * PT_POR_MM, hoja.alto * PT_POR_MM);
        let props = if capas.is_empty() {
            String::new()
        } else {
            format!(" /Properties << {propiedades}>>")
        };
        objetos.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R >>{props} >> /Contents {} 0 R >>",
                num(w),
                num(h),
                primera_hoja + 2 * i + 1
            )
            .into_bytes(),
        );
        let mut flujo = format!(
            "<< /Length {} /Filter /FlateDecode >>\nstream\n",
            comprimido.len()
        )
        .into_bytes();
        flujo.extend_from_slice(&comprimido);
        flujo.extend_from_slice(b"\nendstream");
        objetos.push(flujo);
    }
    let mut out: Vec<u8> = b"%PDF-1.5\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut posiciones = Vec::with_capacity(objetos.len());
    for (i, o) in objetos.iter().enumerate() {
        posiciones.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    let mut tabla = format!("xref\n0 {}\n0000000000 65535 f \n", objetos.len() + 1);
    for p in posiciones {
        let _ = writeln!(tabla, "{p:010} 00000 n ");
    }
    let _ = write!(
        tabla,
        "trailer\n<< /Size {} /Root 1 0 R /Info 4 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objetos.len() + 1
    );
    out.extend_from_slice(tabla.as_bytes());
    out
}

fn contenido_de_hoja(escena: &Escena, hoja: &Hoja, op: &OpcionesPdf, capas: &[String]) -> String {
    let f = hoja.k * PT_POR_MM;
    let mut pg = Pagina {
        s: String::new(),
        hoja,
        f,
        o: [
            hoja.desplazamiento[0] * PT_POR_MM,
            hoja.desplazamiento[1] * PT_POR_MM,
        ],
        trazo: None,
        relleno: None,
        grosor: None,
        pluma: None,
    };
    // Lo de fuera de la zona no se ve (con 1 mm de holgura: una linea en el
    // borde de la zona no pierde la mitad de su grosor).
    let holgura = PT_POR_MM;
    let a = pg.p([hoja.zona[0], hoja.zona[1]]);
    let b = pg.p([hoja.zona[2], hoja.zona[3]]);
    let (a, b) = (
        [a[0] - holgura, a[1] - holgura],
        [b[0] + holgura, b[1] + holgura],
    );
    let _ = writeln!(
        pg.s,
        "q 1 J 1 j {} {} {} {} re W n",
        num(a[0]),
        num(a[1]),
        num(b[0] - a[0]),
        num(b[1] - a[1])
    );
    let mut capa_abierta: Option<usize> = None;
    for e in &escena.elementos {
        if !geo::se_tocan(
            &crate::formatos::escena::caja_de_pieza(&e.pieza),
            &hoja.zona,
        ) {
            continue;
        }
        let k = capas.iter().position(|c| *c == e.capa);
        if k != capa_abierta {
            if capa_abierta.is_some() {
                pg.s.push_str("EMC\n");
            }
            if let Some(k) = k {
                let _ = writeln!(pg.s, "/OC /OC{k} BDC");
            }
            capa_abierta = k;
        }
        let color = if op.monocromo { [0, 0, 0] } else { e.color };
        match &e.pieza {
            Pieza::Formas(fs) => {
                pg.color_trazo(color);
                pg.grosor(e.grosor);
                pg.pluma = None;
                for fm in fs {
                    pg.forma(fm);
                }
                pg.s.push_str("S\n");
                pg.pluma = None;
            }
            Pieza::Relleno(anillos) => {
                pg.color_relleno(color);
                for an in anillos {
                    pg.pluma = None;
                    pg.mover(an[0]);
                    for q in &an[1..] {
                        pg.linea(*q);
                    }
                    pg.s.push_str("h\n");
                }
                pg.s.push_str("f*\n");
                pg.pluma = None;
            }
            Pieza::Texto(t) => pg.texto(t, color, e.grosor, op.trazos),
        }
    }
    if capa_abierta.is_some() {
        pg.s.push_str("EMC\n");
    }
    pg.s.push_str("Q\n");
    pg.s
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn numeros_cortos() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(-0.00001), "0");
        assert_eq!(num(2.50), "2.5");
        assert_eq!(num(-1.23456), "-1.2346");
    }

    #[test]
    fn cadenas_con_escapes_y_acentos() {
        assert_eq!(cadena_pdf("a(b)\\"), "(a\\(b\\)\\\\)");
        assert_eq!(cadena_pdf("ñ€"), "(\\361\\200)");
    }

    #[test]
    fn hojas_ajustadas_y_en_formato() {
        let z = [0.0, 0.0, 10_000.0, 5_000.0];
        let h = hoja_para(z, Papel::Formato(297.0, 420.0), None);
        assert_eq!((h.ancho, h.alto), (420.0, 297.0));
        assert!((h.k - 0.04).abs() < 1e-9);
        let h = hoja_para(z, Papel::Ajustar, Some(0.01));
        assert_eq!((h.ancho, h.alto), (120.0, 70.0));
        // Una lamina A3 a 1/100 (en mm): A3 apaisado, 0,01 mm por unidad.
        let h = hoja_de_lamina([0.0, 0.0, 42_000.0, 29_700.0], 0.01);
        assert_eq!((h.ancho, h.alto), (420.0, 297.0));
        assert!((h.k - 0.01).abs() < 1e-12);
    }

    #[test]
    fn el_arco_en_bezier_pasa_por_sus_puntos() {
        let b = arco_a_bezier([0.0, 0.0], 1.0, 0.0, std::f64::consts::PI);
        assert_eq!(b.len(), 2);
        assert!(geo::dist(b[1][3], [-1.0, 0.0]) < 1e-12);
        // El medio de un cuarto de vuelta, casi en el circulo.
        let [p0, p1, p2, p3] = b[0];
        let m = [
            (p0[0] + 3.0 * p1[0] + 3.0 * p2[0] + p3[0]) / 8.0,
            (p0[1] + 3.0 * p1[1] + 3.0 * p2[1] + p3[1]) / 8.0,
        ];
        assert!((geo::norma(m) - 1.0).abs() < 1e-3);
    }
}
