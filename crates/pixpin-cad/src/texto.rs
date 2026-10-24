//! **Los textos del plano**, rellenos como triangulos para que vayan con
//! todo lo demas en la tarjeta grafica: cada letra se saca de una fuente
//! de Windows (su contorno, con `ttf-parser`), se rellena una vez
//! (`lyon`) y se copia donde haga falta. Van en el nivel de su altura:
//! de lejos, un texto de menos de un pixel no se dibuja.
//!
//! Un estilo con fuente SHX de AutoCAD (romans, simplex, txt…) usa la suya
//! si esta en el equipo (`shx`: las de AutoCAD y las de Windows), con sus
//! trazos como rayas, como se ven en AutoCAD; lo que esa fuente no tenga,
//! y los estilos sin SHX a mano, van con Arial Narrow (se le parece en
//! ancho) o con Arial. Un estilo con TrueType usa la suya si esta.

use std::collections::HashMap;
use std::sync::Arc;

use lyon_tessellation::geom::point;
use lyon_tessellation::path::Path;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
};

use crate::convertir::Afin;
use crate::modelo::Constructor;

/// Una letra ya hecha: triangulos (TrueType) o pares de puntos (SHX), en
/// las unidades de su fuente, y lo que avanza.
struct Letra {
    triangulos: Vec<[f32; 2]>,
    rayas: bool,
    avance: f32,
}

struct Fuente {
    id: u32,
    /// La SHX del estilo, si la hay: sus letras van primero. Con ella las
    /// unidades son «una mayuscula = 1» y las TrueType se escalan a eso.
    shx: Option<crate::shx::Shx>,
    datos: Arc<Vec<u8>>,
    letras: HashMap<char, Option<Letra>>,
    /// Lo que mide una mayuscula en em: la altura de un texto de AutoCAD
    /// es la de sus mayusculas.
    alto_mayuscula: f32,
    /// Con SHX: lo que se agranda una letra TrueType para casar con ella.
    escala_ttf: f32,
}

/// Las carpetas de fuentes SHX: las del usuario (que pueden cambiar entre
/// un plano y otro) y las del sistema, buscadas una vez.
fn carpetas_shx() -> Vec<std::path::PathBuf> {
    static C: std::sync::OnceLock<Vec<std::path::PathBuf>> = std::sync::OnceLock::new();
    let mut v = crate::letras::carpetas_del_usuario();
    v.extend(
        C.get_or_init(crate::shx::carpetas_del_sistema)
            .iter()
            .cloned(),
    );
    v
}

pub struct Textos {
    fuentes: HashMap<String, Option<Fuente>>,
}

const CARPETA_FUENTES: &str = "C:\\Windows\\Fonts";

/// Las fuentes instaladas en Windows por su nombre de familia (el del
/// registro: «agency fb», «agency fb bold»…) y su fichero. Se lee una vez.
pub fn fuentes_instaladas() -> &'static HashMap<String, std::path::PathBuf> {
    static F: std::sync::OnceLock<HashMap<String, std::path::PathBuf>> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        use windows::Win32::System::Registry::{
            HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegEnumValueW,
            RegOpenKeyExW,
        };
        let mut mapa = HashMap::new();
        for raiz in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            let mut k = HKEY::default();
            // SAFETY: clave de solo lectura que se cierra al acabar; los
            // bufferes son locales y van con su largo.
            unsafe {
                if RegOpenKeyExW(
                    raiz,
                    windows::core::w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts"),
                    Some(0),
                    KEY_READ,
                    &mut k,
                )
                .is_err()
                {
                    continue;
                }
                for i in 0..4096u32 {
                    let mut nombre = [0u16; 512];
                    let mut largo_n = nombre.len() as u32;
                    let mut datos = [0u8; 1024];
                    let mut largo_d = datos.len() as u32;
                    let r = RegEnumValueW(
                        k,
                        i,
                        Some(windows::core::PWSTR(nombre.as_mut_ptr())),
                        &mut largo_n,
                        None,
                        None,
                        Some(datos.as_mut_ptr()),
                        Some(&mut largo_d),
                    );
                    if r.is_err() {
                        break;
                    }
                    let nombre =
                        String::from_utf16_lossy(&nombre[..largo_n as usize]).to_lowercase();
                    let u16s: Vec<u16> = datos[..largo_d as usize]
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .take_while(|&c| c != 0)
                        .collect();
                    let fichero = String::from_utf16_lossy(&u16s);
                    if fichero.is_empty() {
                        continue;
                    }
                    let ruta = if fichero.contains(':') {
                        std::path::PathBuf::from(&fichero)
                    } else {
                        std::path::Path::new(CARPETA_FUENTES).join(&fichero)
                    };
                    let limpio = nombre.replace(" (truetype)", "").replace(" (opentype)", "");
                    for n in limpio.split(" & ") {
                        mapa.entry(n.trim().to_string())
                            .or_insert_with(|| ruta.clone());
                    }
                }
                let _ = RegCloseKey(k);
            }
        }
        mapa
    })
}

/// La clave de fuente (para [`Textos`]) de una familia de Windows, con
/// negrita si la hay: «ttf:<fichero>». `None` si no esta instalada.
pub fn clave_de_familia(familia: &str, negrita: bool) -> Option<String> {
    let f = familia.trim().trim_matches('"').to_lowercase();
    if f.is_empty() {
        return None;
    }
    if f.ends_with(".shx") || f.ends_with(".ttf") || f.ends_with(".otf") {
        return Some(f);
    }
    // Las del usuario (carpeta de letras de PixPin) mandan sobre las de Windows.
    let mias = crate::letras::familias_del_usuario();
    let m = fuentes_instaladas();
    // El registro nombra la negrita en el idioma de Windows.
    let negritas = ["bold", "negrita", "gras", "fett", "grassetto", "negrito"];
    let buscar = |m: &HashMap<String, std::path::PathBuf>| -> Option<std::path::PathBuf> {
        let r = if negrita {
            negritas
                .iter()
                .find_map(|n| m.get(&format!("{f} {n}")))
                .or_else(|| m.get(&f))
        } else {
            m.get(&f).or_else(|| m.get(&format!("{f} regular")))
        };
        r.cloned()
    };
    let r = buscar(&mias)
        .or_else(|| buscar(m))
        .map(|p| format!("ttf:{}", p.display()));
    if r.is_none() {
        // Se dice en el visor: «este plano pide letras que no estan».
        crate::letras::falta(familia.trim().trim_matches('"'));
    }
    r
}

/// El fichero de fuente de Windows para el `font_file` de un estilo.
pub fn fichero_para(estilo: &str) -> Vec<String> {
    let e = estilo.trim().to_ascii_lowercase();
    let mut v = Vec::new();
    if let Some(ruta) = e.strip_prefix("ttf:") {
        v.push(ruta.to_string());
    }
    if !e.starts_with("ttf:") && (e.ends_with(".ttf") || e.ends_with(".otf") || e.ends_with(".ttc"))
    {
        v.push(e.rsplit(['\\', '/']).next().unwrap_or(&e).to_string());
    }
    if e.contains("arial") || e.is_empty() {
        v.push("arial.ttf".into());
    }
    v.extend([
        "arialn.ttf".to_string(),
        "arial.ttf".to_string(),
        "segoeui.ttf".to_string(),
    ]);
    v
}

impl Textos {
    pub fn nuevo() -> Self {
        Self {
            fuentes: HashMap::new(),
        }
    }

    /// Lo que mide `texto` en pixeles con letra de `tam` (la barra).
    pub fn medir_pantalla(&mut self, texto: &str, tam: f64) -> f64 {
        self.ancho("segoeui.ttf", texto) as f64 * tam
    }

    /// Escribe en pixeles de la ventana (y hacia abajo), con Segoe UI, desde
    /// `x` sobre la base `y`; si no cabe antes de `x_max`, lo corta con «…».
    /// Devuelve lo que ocupa.
    #[allow(clippy::too_many_arguments)]
    pub fn en_pantalla(
        &mut self,
        c: &mut Constructor,
        texto: &str,
        x: f64,
        y: f64,
        tam: f64,
        color: u32,
        x_max: f64,
    ) -> f64 {
        let mut t: String = texto.to_string();
        if x + self.medir_pantalla(&t, tam) > x_max {
            while !t.is_empty() && x + self.medir_pantalla(&format!("{t}…"), tam) > x_max {
                t.pop();
            }
            t = format!("{}…", t.trim_end());
        }
        let m = Afin::traslacion(x, y, 0.0).por(&Afin::escala(tam, -tam, 1.0));
        self.escribir(c, "segoeui.ttf", &t, &m, 1e6, color);
        self.medir_pantalla(&t, tam)
    }

    fn fuente(&mut self, estilo: &str) -> Option<&mut Fuente> {
        let clave = estilo.to_ascii_lowercase();
        if !self.fuentes.contains_key(&clave) {
            let mut f = None;
            let shx = if clave.ends_with(".shx") || (!clave.is_empty() && !clave.contains('.')) {
                // La de AutoCAD si esta; si no, la de PixPin que mas se le parece.
                // Antes, si el plano la pide y no esta, se apunta para decirlo.
                crate::shx::buscar(&clave, &carpetas_shx())
                    .and_then(|r| crate::shx::abrir(&r))
                    .or_else(|| {
                        crate::letras::falta_shx(&clave);
                        None
                    })
                    .or_else(|| crate::shx::de_reserva(&clave))
            } else {
                None
            };
            crate::letras::revisar_ttf(&clave, std::path::Path::new(CARPETA_FUENTES));
            for nombre in fichero_para(&clave) {
                let ruta =
                    crate::letras::ruta_de_fuente(std::path::Path::new(CARPETA_FUENTES), &nombre);
                if let Ok(datos) = std::fs::read(&ruta)
                    && let Ok(cara) = ttf_parser::Face::parse(&datos, 0)
                {
                    let em = cara.units_per_em() as f32;
                    // Muchas fuentes viejas (Agency FB, Stencil...) no dicen cuanto mide
                    // una mayuscula: se mide la «H». Con 0,716 fijo salian un 7 % mas
                    // grandes o mas pequeñas que en AutoCAD.
                    let mayus = cara
                        .capital_height()
                        .map(|h| h as f32 / em)
                        .filter(|h| *h > 0.2)
                        .or_else(|| {
                            cara.glyph_index('H')
                                .and_then(|g| cara.glyph_bounding_box(g))
                                .map(|b| b.y_max as f32 / em)
                                .filter(|h| *h > 0.2 && *h < 1.2)
                        })
                        .unwrap_or(0.716);
                    let con_shx = shx.is_some();
                    f = Some(Fuente {
                        id: self.fuentes.len() as u32,
                        shx: None,
                        datos: Arc::new(datos),
                        letras: HashMap::new(),
                        alto_mayuscula: if con_shx { 1.0 } else { mayus },
                        escala_ttf: if con_shx { 1.0 / mayus } else { 1.0 },
                    });
                    break;
                }
            }
            if let Some(f) = f.as_mut() {
                f.shx = shx;
            }
            self.fuentes.insert(clave.clone(), f);
        }
        self.fuentes.get_mut(&clave)?.as_mut()
    }

    /// Lo que mide `texto` en em.
    fn ancho(&mut self, estilo: &str, texto: &str) -> f32 {
        let Some(f) = self.fuente(estilo) else {
            return 0.0;
        };
        texto
            .chars()
            .map(|c| f.letra(c).map_or(0.5, |l| l.avance))
            .sum()
    }

    /// Escribe una linea en coordenadas locales (x hacia la derecha, base
    /// en y = 0, unidades de em) llevadas al plano con `m`.
    fn escribir(
        &mut self,
        c: &mut Constructor,
        estilo: &str,
        texto: &str,
        m: &Afin,
        alto_mundo: f64,
        color: u32,
    ) {
        let Some(f) = self.fuente(estilo) else { return };
        let id = f.id;
        // De em al plano: las columnas de la matriz.
        let mat = [m.m[0][0], m.m[0][1], m.m[1][0], m.m[1][1]];
        let mut x = 0.0f64;
        for ch in texto.chars() {
            let Some(l) = f.letra(ch) else {
                x += 0.5;
                continue;
            };
            let avance = l.avance as f64;
            if !l.triangulos.is_empty() {
                let tri = &l.triangulos;
                let g = c.glifo(id, ch, l.rayas, || tri.clone());
                c.letra(g, m.punto(x, 0.0, 0.0), mat, color, alto_mundo);
            }
            x += avance;
        }
    }

    /// Un TEXT o un ATTRIB: una linea con su alineacion.
    #[allow(clippy::too_many_arguments)]
    pub fn simple(
        &mut self,
        c: &mut Constructor,
        texto: &str,
        ins: [f64; 3],
        alineado: Option<[f64; 3]>,
        alto: f64,
        giro: f64,
        ancho: f64,
        oblicuo: f64,
        h: u8,
        v: u8,
        estilo: &str,
        color: u32,
        t: &Afin,
    ) {
        let texto = texto.trim_end();
        if texto.is_empty() || !(alto > 0.0) || !alto.is_finite() {
            return;
        }
        let mayus = self.fuente(estilo).map_or(0.716, |f| f.alto_mayuscula) as f64;
        let ancho = if ancho > 0.0 && ancho.is_finite() {
            ancho
        } else {
            1.0
        };
        let mut escala_y = alto / mayus;
        let mut escala_x = escala_y * ancho;
        let largo_em = self.ancho(estilo, texto) as f64;
        let mut giro = giro;
        // Donde se engancha: el punto de insercion si es «izquierda, base»,
        // el de alineacion en lo demas.
        let ancla = match alineado {
            Some(a) if h != 0 || v != 0 => a,
            _ => ins,
        };
        let (mut dx, mut dy) = (0.0, 0.0);
        if (h == 3 || h == 5)
            && let Some(a) = alineado
        {
            // Alineado / Ajustado: entre los dos puntos.
            let (vx, vy) = (a[0] - ins[0], a[1] - ins[1]);
            let largo = (vx * vx + vy * vy).sqrt();
            if largo > 1e-12 && largo_em > 0.0 {
                giro = vy.atan2(vx);
                escala_x = largo / largo_em;
                if h == 3 {
                    escala_y = escala_x / ancho;
                }
            }
            let m = t
                .por(&Afin::traslacion(ins[0], ins[1], ins[2]))
                .por(&Afin::giro_z(giro))
                .por(&Afin::escala(escala_x, escala_y, 1.0))
                .por(&oblicua(oblicuo));
            self.escribir(c, estilo, texto, &m, alto, color);
            return;
        }
        let largo = largo_em * escala_x;
        match h {
            1 | 4 => dx = -largo / 2.0,
            2 => dx = -largo,
            _ => {}
        }
        match (h, v) {
            (4, _) | (_, 2) => dy = -alto / 2.0,
            (_, 1) => dy = alto * 0.3,
            (_, 3) => dy = -alto,
            _ => {}
        }
        let m = t
            .por(&Afin::traslacion(ancla[0], ancla[1], ancla[2]))
            .por(&Afin::giro_z(giro))
            .por(&Afin::traslacion(dx, dy, 0.0))
            .por(&Afin::escala(escala_x, escala_y, 1.0))
            .por(&oblicua(oblicuo));
        self.escribir(c, estilo, texto, &m, alto, color);
    }

    /// Un MTEXT: sus parrafos con el formato de cada trozo (su letra, su
    /// color, su altura: los `\\f`, `\\C`, `\\H` del texto), con su punto
    /// de enganche (1 arriba-izquierda … 9 abajo-derecha) y, si tiene ancho
    /// de caja, cortados por palabras.
    #[allow(clippy::too_many_arguments)]
    pub fn multilinea(
        &mut self,
        c: &mut Constructor,
        parrafos: &[Vec<Trozo>],
        ins: [f64; 3],
        alto: f64,
        giro: f64,
        enganche: u8,
        caja: f64,
        estilo: &str,
        color: u32,
        t: &Afin,
    ) {
        if !(alto > 0.0) || !alto.is_finite() {
            return;
        }
        // Cada palabra con su letra, su color, su alto y lo que mide.
        struct Pieza {
            texto: String,
            fuente: String,
            color: u32,
            alto: f64,
            escala_x: f64,
            ancho: f64,
            ancho_sin_espacio: f64,
        }
        let mut renglones: Vec<Vec<Pieza>> = Vec::new();
        for p in parrafos {
            // Si el parrafo entero casi cabe en la caja, se estrecha un poco
            // en vez de partirlo: las letras de aqui no miden exactamente
            // como las de quien hizo el plano, y partir un «CT: 3825.8» en
            // dos renglones lo monta encima de lo de abajo.
            let mut estrechar = 1.0;
            if caja > 0.0 && caja.is_finite() {
                let mut total = 0.0;
                for tr in p {
                    let fuente = tr.fuente.clone().unwrap_or_else(|| estilo.to_string());
                    let alto_t = match tr.alto {
                        Some((true, f)) if f > 0.0 => alto * f,
                        Some((false, a)) if a > 0.0 => a,
                        _ => alto,
                    };
                    let mayus = self.fuente(&fuente).map_or(0.716, |f| f.alto_mayuscula) as f64;
                    total += self.ancho(&fuente, tr.texto.trim_end()) as f64 * alto_t / mayus
                        * tr.ancho.filter(|w| *w > 0.0).unwrap_or(1.0);
                }
                if total > caja && total <= caja * 1.08 {
                    estrechar = caja / total;
                }
            }
            let mut renglon: Vec<Pieza> = Vec::new();
            let mut ancho_renglon = 0.0;
            for tr in p {
                let fuente = tr.fuente.clone().unwrap_or_else(|| estilo.to_string());
                let alto_t = match tr.alto {
                    Some((true, f)) if f > 0.0 => alto * f,
                    Some((false, a)) if a > 0.0 => a,
                    _ => alto,
                };
                let mayus = self.fuente(&fuente).map_or(0.716, |f| f.alto_mayuscula) as f64;
                let wf = tr.ancho.filter(|w| *w > 0.0).unwrap_or(1.0) * estrechar;
                let escala = alto_t / mayus;
                for palabra in tr.texto.split_inclusive(' ') {
                    let ancho = self.ancho(&fuente, palabra) as f64 * escala * wf;
                    let ancho_sin = self.ancho(&fuente, palabra.trim_end()) as f64 * escala * wf;
                    // Se corta solo si se pasa de verdad: las letras de aqui
                    // y las del autor no miden exactamente igual.
                    if caja > 0.0
                        && caja.is_finite()
                        && !renglon.is_empty()
                        && ancho_renglon + ancho_sin > caja * 1.01
                    {
                        renglones.push(std::mem::take(&mut renglon));
                        ancho_renglon = 0.0;
                    }
                    ancho_renglon += ancho;
                    renglon.push(Pieza {
                        texto: palabra.to_string(),
                        fuente: fuente.clone(),
                        color: tr.color.unwrap_or(color),
                        alto: alto_t,
                        escala_x: escala * wf,
                        ancho,
                        ancho_sin_espacio: ancho_sin,
                    });
                }
            }
            renglones.push(renglon);
        }
        let altos: Vec<f64> = renglones
            .iter()
            .map(|r| r.iter().map(|p| p.alto).fold(alto, f64::max))
            .collect();
        let total: f64 = altos.first().copied().unwrap_or(alto)
            + altos.iter().skip(1).map(|a| a * 5.0 / 3.0).sum::<f64>();
        let col = (enganche.clamp(1, 9) - 1) % 3;
        let fila = (enganche.clamp(1, 9) - 1) / 3;
        let primero = altos.first().copied().unwrap_or(alto);
        let mut y = match fila {
            0 => -primero,
            1 => total / 2.0 - primero,
            _ => total - primero,
        };
        for (i, r) in renglones.iter().enumerate() {
            if i > 0 {
                y -= altos[i] * 5.0 / 3.0;
            }
            let ancho: f64 = r.iter().map(|p| p.ancho).sum::<f64>()
                - r.last().map_or(0.0, |p| p.ancho - p.ancho_sin_espacio);
            let mut x = match col {
                0 => 0.0,
                1 => -ancho / 2.0,
                _ => -ancho,
            };
            for p in r {
                if !p.texto.trim().is_empty() {
                    let escala_y =
                        p.alto / self.fuente(&p.fuente).map_or(0.716, |f| f.alto_mayuscula) as f64;
                    let m = t
                        .por(&Afin::traslacion(ins[0], ins[1], ins[2]))
                        .por(&Afin::giro_z(giro))
                        .por(&Afin::traslacion(x, y, 0.0))
                        .por(&Afin::escala(p.escala_x, escala_y, 1.0));
                    self.escribir(c, &p.fuente, &p.texto, &m, p.alto, p.color);
                }
                x += p.ancho;
            }
        }
    }
}

/// Un trozo de un MTEXT con su formato: su letra (clave de [`Textos`]), su
/// color, su alto (factor o absoluto) y su ancho.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Trozo {
    pub texto: String,
    pub fuente: Option<String>,
    pub color: Option<u32>,
    pub alto: Option<(bool, f64)>,
    pub ancho: Option<f64>,
}

/// Los parrafos de un MTEXT con el formato de cada trozo.
pub fn parrafos_de_mtext(valor: &str) -> Vec<Vec<Trozo>> {
    use opencadcodec::entities::mtext_format::{MTextColor, MTextScalar};
    let doc = opencadcodec::entities::mtext_format::parse_mtext(valor, true);
    let mut v: Vec<Vec<Trozo>> = doc
        .paragraphs
        .iter()
        .map(|p| {
            p.spans
                .iter()
                .map(|s| {
                    let pr = &s.properties;
                    let texto = match &s.stacking {
                        Some(st) if !st.numerator.is_empty() || !st.denominator.is_empty() => {
                            format!("{}{}/{}", s.text, st.numerator, st.denominator)
                        }
                        _ => s.text.clone(),
                    };
                    let color = match (pr.color_rgb, pr.color) {
                        (Some((r, g, b)), _) => Some(crate::convertir::rgba(r, g, b)),
                        (None, Some(MTextColor::TrueColor(c))) => Some(crate::convertir::rgba(
                            (c >> 16) as u8,
                            (c >> 8) as u8,
                            c as u8,
                        )),
                        (None, Some(MTextColor::Index(i))) if i > 0 && i < 256 => {
                            crate::convertir::de_color(&opencadcodec::types::Color::Index(i as u8))
                        }
                        _ => None,
                    };
                    Trozo {
                        texto,
                        fuente: pr
                            .font
                            .as_ref()
                            .and_then(|f| clave_de_familia(&f.name, f.bold)),
                        color,
                        alto: pr.height.map(|h| match h {
                            MTextScalar::Factor(f) => (true, f),
                            MTextScalar::Absolute(a) => (false, a),
                        }),
                        ancho: pr.width_factor,
                    }
                })
                .collect()
        })
        .collect();
    if v.is_empty() {
        v.push(Vec::new());
    }
    v
}

fn oblicua(angulo: f64) -> Afin {
    let mut a = Afin::IDENTIDAD;
    if angulo.is_finite() && angulo.abs() > 1e-9 && angulo.abs() < 1.4 {
        a.m[0][1] = angulo.tan();
    }
    a
}

impl Fuente {
    fn letra(&mut self, ch: char) -> Option<&Letra> {
        if !self.letras.contains_key(&ch) {
            let de_shx = self
                .shx
                .as_ref()
                .and_then(|f| f.letra(ch))
                .filter(|l| !l.trazos.is_empty() || ch == ' ');
            let l = match de_shx {
                Some(l) => {
                    // Cada trazo, en pares de puntos (LINELIST).
                    let mut pares = Vec::new();
                    for t in &l.trazos {
                        for w in t.windows(2) {
                            pares.push(w[0]);
                            pares.push(w[1]);
                        }
                    }
                    Some(Letra {
                        triangulos: pares,
                        rayas: true,
                        avance: l.avance,
                    })
                }
                None => rellenar_letra(&self.datos, ch).map(|mut l| {
                    let k = self.escala_ttf;
                    if k != 1.0 {
                        for p in &mut l.triangulos {
                            p[0] *= k;
                            p[1] *= k;
                        }
                        l.avance *= k;
                    }
                    l
                }),
            };
            self.letras.insert(ch, l);
        }
        self.letras.get(&ch)?.as_ref()
    }
}

struct Trazador {
    b: lyon_tessellation::path::path::Builder,
    em: f32,
    abierto: bool,
}

impl ttf_parser::OutlineBuilder for Trazador {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.abierto {
            self.b.end(true);
        }
        self.b.begin(point(x / self.em, y / self.em));
        self.abierto = true;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.b.line_to(point(x / self.em, y / self.em));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.b.quadratic_bezier_to(
            point(x1 / self.em, y1 / self.em),
            point(x / self.em, y / self.em),
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.b.cubic_bezier_to(
            point(x1 / self.em, y1 / self.em),
            point(x2 / self.em, y2 / self.em),
            point(x / self.em, y / self.em),
        );
    }
    fn close(&mut self) {
        if self.abierto {
            self.b.end(true);
            self.abierto = false;
        }
    }
}

fn rellenar_letra(datos: &[u8], ch: char) -> Option<Letra> {
    let cara = ttf_parser::Face::parse(datos, 0).ok()?;
    let id = cara.glyph_index(ch).or_else(|| cara.glyph_index('?'))?;
    let em = cara.units_per_em() as f32;
    let avance = cara.glyph_hor_advance(id).map_or(0.5, |a| a as f32 / em);
    let mut tr = Trazador {
        b: Path::builder(),
        em,
        abierto: false,
    };
    let mut triangulos = Vec::new();
    if cara.outline_glyph(id, &mut tr).is_some() {
        if tr.abierto {
            tr.b.end(true);
        }
        let camino = tr.b.build();
        let mut bufs: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
        // Una tolerancia gruesa (1/150 de em): de cerca sigue siendo redonda y
        // cada letra son pocas decenas de triangulos.
        let ok = FillTessellator::new().tessellate_path(
            &camino,
            &FillOptions::tolerance(1.0 / 150.0).with_fill_rule(FillRule::NonZero),
            &mut BuffersBuilder::new(&mut bufs, |v: FillVertex| v.position().to_array()),
        );
        if ok.is_ok() {
            triangulos = bufs
                .indices
                .iter()
                .map(|&i| bufs.vertices[i as usize])
                .collect();
        }
    }
    Some(Letra {
        triangulos,
        rayas: false,
        avance,
    })
}

/// Los renglones de un MTEXT sin sus codigos de formato (`\P`, `\f…;`,
/// llaves, `\S` de las fracciones…), con la libreria.
pub fn lineas_de_mtext(valor: &str) -> Vec<String> {
    let doc = opencadcodec::entities::mtext_format::parse_mtext(valor, true);
    let mut v: Vec<String> = doc
        .paragraphs
        .iter()
        .map(|p| p.spans.iter().map(|s| s.text.as_str()).collect::<String>())
        .collect();
    if v.is_empty() {
        v.push(String::new());
    }
    v
}

/// El texto de un TEXT con sus `%%d`, `%%c`... ya cambiados.
pub fn texto_plano(valor: &str) -> String {
    let doc = opencadcodec::entities::mtext_format::parse_plain_text(valor);
    doc.paragraphs
        .iter()
        .map(|p| p.spans.iter().map(|s| s.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_mtext_pierde_el_formato_y_parte_los_parrafos() {
        let l = lineas_de_mtext("{\\fArial|b1;Hola}\\Pmundo");
        assert_eq!(l, vec!["Hola".to_string(), "mundo".to_string()]);
        assert_eq!(texto_plano("45%%d"), "45°");
    }

    #[test]
    fn una_letra_de_arial_tiene_triangulos_y_avance() {
        let mut t = Textos::nuevo();
        let Some(f) = t.fuente("arial.ttf") else {
            return; // sin fuentes de Windows (no deberia pasar)
        };
        let a = f.letra('A').unwrap();
        assert!(a.triangulos.len() >= 9 && a.avance > 0.3 && !a.rayas);
        // Con AutoCAD en el equipo, romans.shx da rayas; sin el, Arial.
        if let Some(r) = t.fuente("romans.shx") {
            let a = r.letra('A').unwrap();
            assert!(!a.triangulos.is_empty() && a.triangulos.len() % 2 == 0 || !a.rayas);
        }
        let Some(f) = t.fuente("arial.ttf") else {
            return;
        };
        // Un espacio no tiene contorno pero si avance.
        let e = f.letra(' ').unwrap();
        assert!(e.triangulos.is_empty() && e.avance > 0.1);
    }
}

#[cfg(test)]
mod pruebas_letras_del_usuario {
    use super::*;
    use crate::letras;

    fn carpeta(nombre: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-texto-letras-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Una SHX que el plano pide y no esta se apunta (y se escribe con la de
    /// Hershey); puesta en la carpeta del usuario, se usa y ya no falta.
    #[test]
    fn la_shx_que_falta_se_dice_y_la_del_usuario_manda() {
        let _g = letras::UNA_A_LA_VEZ
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        letras::poner_carpeta(None);
        letras::sacar_lo_que_falta();
        let mut t = Textos::nuevo();
        assert!(
            t.fuente("pixpin-prueba-rara.shx").is_some(),
            "la de reserva escribe igual"
        );
        assert_eq!(
            letras::sacar_lo_que_falta(),
            vec!["pixpin-prueba-rara.shx".to_string()]
        );
        // Ahora el usuario la pone (aqui, las rayas de romans con ese nombre).
        let d = carpeta("shx");
        std::fs::write(
            d.join("PIXPIN-PRUEBA-RARA.SHX"),
            include_bytes!("../fuentes-shx/romans.shx"),
        )
        .unwrap();
        letras::poner_carpeta(Some(d.clone()));
        let mut t = Textos::nuevo();
        let f = t.fuente("pixpin-prueba-rara.shx").expect("la del usuario");
        assert!(f.letra('A').is_some_and(|l| l.rayas));
        assert!(letras::sacar_lo_que_falta().is_empty(), "ya no falta");
        letras::poner_carpeta(None);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Una familia TrueType que no esta se apunta; la del usuario (por su
    /// nombre de verdad, no el del fichero) manda sobre la de Windows.
    #[test]
    fn la_familia_que_falta_se_dice_y_la_del_usuario_manda() {
        let _g = letras::UNA_A_LA_VEZ
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        letras::poner_carpeta(None);
        letras::sacar_lo_que_falta();
        assert_eq!(clave_de_familia("Familia Que No Existe", false), None);
        assert_eq!(
            letras::sacar_lo_que_falta(),
            vec!["Familia Que No Existe".to_string()]
        );
        let arial = std::path::Path::new(CARPETA_FUENTES).join("arial.ttf");
        if !arial.is_file() {
            return;
        }
        let d = carpeta("ttf");
        std::fs::copy(&arial, d.join("copia-de-arial.ttf")).unwrap();
        letras::poner_carpeta(Some(d.clone()));
        let k = clave_de_familia("Arial", false).unwrap();
        assert!(k.to_lowercase().ends_with("copia-de-arial.ttf"), "{k}");
        // Un fichero pedido por su nombre tambien sale de la carpeta del usuario.
        assert_eq!(
            letras::ruta_de_fuente(std::path::Path::new(CARPETA_FUENTES), "COPIA-DE-ARIAL.TTF"),
            d.join("copia-de-arial.ttf")
        );
        assert!(letras::sacar_lo_que_falta().is_empty());
        letras::poner_carpeta(None);
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod medir_mayusculas {
    #[test]
    #[ignore = "mide fuentes del sistema"]
    fn mayusculas() {
        for f in [
            "AGENCYR.TTF",
            "AGENCYB.TTF",
            "FREESCPT.TTF",
            "STENCIL.TTF",
            "arial.ttf",
            "ARIALN.TTF",
        ] {
            let d = std::fs::read(format!("C:/Windows/Fonts/{f}")).unwrap();
            let c = ttf_parser::Face::parse(&d, 0).unwrap();
            let em = c.units_per_em() as f32;
            let h = c
                .glyph_index('H')
                .and_then(|g| c.glyph_bounding_box(g))
                .map(|b| b.y_max as f32 / em);
            println!(
                "{f}: capHeight={:?} H={:?} asc={}",
                c.capital_height().map(|v| v as f32 / em),
                h,
                c.ascender() as f32 / em
            );
        }
    }
}
