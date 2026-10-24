//! **El texto del lector de documentos, partido como en el movil** (K16).
//!
//! El movil ensena un Word o un libro en un `WebView` con su hoja de estilo
//! (`DocxAHtml.ESTILO`, `Lectura.estilo`): letra «serif» o «sans-serif» de
//! Android (Noto Serif y Roboto), un peso de CSS (300, 400, 600, 800) y
//! renglones a `line-height:1.55` exactos. Lo anotado encima va en pixeles
//! de esa pagina, asi que **para que la tinta caiga en la misma palabra en
//! los dos aparatos el PC tiene que partir los renglones igual**: la misma
//! letra (incrustada en `letras`), el mismo peso, el mismo alto de renglon y
//! **los mismos sitios donde se puede partir**.
//!
//! # Por que se parte aqui y no en DirectWrite
//!
//! Medido contra el Chromium del movil con el Word del usuario: DirectWrite
//! parte donde el navegador no (tras la barra de un enlace, entre «N» y
//! «°»), asi que una columna de tabla le salia mas estrecha, los enlaces en
//! dos renglones y cada fila mas alta; al final de la pagina, miles de
//! pixeles de diferencia. Por eso de DirectWrite solo se toma **lo que
//! avanza cada letra** (`GetClusterMetrics`, con la letra, el peso, la
//! cursiva y el kerning de verdad) y los renglones se cortan aqui con las
//! reglas del navegador ([`se_parte_tras`]): por los espacios, tras un guion
//! o una raya, y dentro de una palabra solo si no cabe ni sola en un renglon
//! (`overflow-wrap:break-word`). Cada renglon se pinta luego con su propia
//! disposicion, sin partir.
//!
//! - Renglones de alto fijo (`DWRITE_LINE_SPACING_METHOD_UNIFORM`): el alto de
//!   un parrafo es `renglones * tam * interlineado`, como en CSS.
//! - El ancho minimo es el «min-content» de CSS: el trozo sin corte mas
//!   ancho, lo que necesita una columna de tabla.

use windows::Win32::Graphics::DirectWrite::{
    DWRITE_CLUSTER_METRICS, DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_WEIGHT,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
    DWRITE_TEXT_RANGE, DWRITE_WORD_WRAPPING_NO_WRAP, IDWriteFactory, IDWriteTextFormat,
    IDWriteTextLayout,
};
use windows::core::w;
use windows_numerics::Vector2;

use crate::lienzo::{Pintor, RectF, Tramo};
use crate::motor::{Color, MotorRender};

/// Con que letra se escribe un bloque del documento.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LetraDeLectura<'a> {
    /// «Noto Serif», «Roboto», «Courier New»…
    pub familia: &'a str,
    /// El peso de CSS del bloque (el de un tramo en negrita es el mayor de
    /// este y 700, como `b,strong{font-weight:max(peso,700)}` del movil).
    pub peso: u16,
    /// El `line-height` de CSS: el renglon mide `tam * interlineado`.
    pub interlineado: f32,
    pub alineacion: Alineacion,
}

/// `text-align` del bloque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alineacion {
    #[default]
    Izquierda,
    Centro,
    Derecha,
}

/// Lo que ocupa un parrafo a un ancho: el renglon mas ancho, el alto de
/// todos, el ancho minimo (el trozo sin corte mas ancho) y donde empieza y
/// acaba cada renglon (en unidades UTF-16, sin los espacios del final).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Medida {
    pub ancho: f32,
    pub alto: f32,
    pub minimo: f32,
    pub renglones: Vec<(u32, u32)>,
}

/// La letra de ancho fijo de un tramo de codigo: Courier New avanza 0,6 em
/// por letra, lo mismo que la «monospace» de Android (Droid Sans Mono).
pub const LETRA_FIJA: &str = "Courier New";

// ---------------------------------------------------------------------------
// Donde se puede partir

fn es_espacio(c: u16) -> bool {
    // El espacio duro (U+00A0) no parte: es el `&nbsp;` de un parrafo vacio.
    matches!(c, 0x20 | 0x09 | 0x2000..=0x200A | 0x3000) && c != 0x2007
}

fn es_letra_o_cifra(c: u16) -> bool {
    char::from_u32(u32::from(c)).is_some_and(|c| c.is_alphanumeric())
}

fn es_ideograma(c: u16) -> bool {
    matches!(c, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF)
}

/// **Si el navegador puede partir entre `antes` y `despues`** (el ultimo
/// caracter de un renglon y el primero del siguiente): tras un espacio (el
/// espacio se queda colgando al final), tras un guion entre letras o cifras,
/// tras una raya corta, antes y despues de una raya larga, tras un espacio
/// de ancho cero y entre ideogramas. No tras la barra ni el punto de un
/// enlace, ni entre una letra y el signo de grado: asi lo hace el `WebView`.
pub fn se_parte_tras(antes: u16, despues: u16) -> bool {
    if es_espacio(despues) {
        return false;
    }
    if es_espacio(antes) || antes == 0x200B {
        return true;
    }
    match antes {
        // Guion y guion de Unicode: entre dos letras o cifras.
        0x2D | 0x2010 => es_letra_o_cifra(despues),
        // Raya corta: detras.
        0x2013 => true,
        // Raya larga: detras, salvo pegada a otra.
        0x2014 => despues != 0x2014,
        _ => despues == 0x2014 && antes != 0x2014 || es_ideograma(antes) || es_ideograma(despues),
    }
}

/// **Parte en renglones** un texto de `anchos` (lo que avanza cada unidad
/// UTF-16: el ancho de su racimo en la primera y cero en las demas) a un
/// ancho maximo. Devuelve los renglones (sin sus espacios del final), el mas
/// ancho y el trozo sin corte mas ancho (el «min-content»).
///
/// Un salto de linea (`\n`, U+2028) parte siempre. Una palabra que no cabe
/// ni sola se parte por donde llegue (`overflow-wrap:break-word`).
pub fn partir(texto: &[u16], anchos: &[f32], ancho_max: f32) -> (Vec<(u32, u32)>, f32, f32) {
    let n = texto.len();
    let mut renglones = Vec::new();
    let mut mas_ancho = 0.0f32;
    let mut minimo = 0.0f32;
    let ancho_de = |a: usize, b: usize| -> f32 {
        // Sin los espacios del final: cuelgan fuera del renglon.
        let mut b = b;
        while b > a && es_espacio(texto[b - 1]) {
            b -= 1;
        }
        anchos[a..b].iter().sum()
    };
    let fin_sin_espacios = |a: usize, b: usize| -> usize {
        let mut b = b;
        while b > a && es_espacio(texto[b - 1]) {
            b -= 1;
        }
        b
    };

    // El min-content: cada trozo entre dos sitios donde se puede partir.
    let mut desde = 0usize;
    for i in 1..=n {
        let corte = i == n
            || texto[i - 1] == 0x0A
            || texto[i - 1] == 0x2028
            || se_parte_tras(texto[i - 1], texto[i]);
        if corte {
            let hasta = if i > desde && (texto[i - 1] == 0x0A || texto[i - 1] == 0x2028) {
                i - 1
            } else {
                i
            };
            minimo = minimo.max(ancho_de(desde, hasta));
            desde = i;
        }
    }

    let mut inicio = 0usize;
    while inicio < n || renglones.is_empty() {
        if inicio >= n {
            renglones.push((n as u32, n as u32));
            break;
        }
        let mut suma = 0.0f32;
        // El ultimo sitio donde se podia partir y lo que medía el renglon hasta ahi.
        let mut ultimo: Option<usize> = None;
        let mut i = inicio;
        let mut fin = n;
        let mut siguiente = n;
        while i < n {
            let c = texto[i];
            if c == 0x0A || c == 0x2028 {
                fin = i;
                siguiente = i + 1;
                break;
            }
            if i > inicio && se_parte_tras(texto[i - 1], c) {
                ultimo = Some(i);
            }
            let ancho = anchos[i];
            if !es_espacio(c) && suma + ancho > ancho_max + 0.01 && suma > 0.0 {
                match ultimo {
                    Some(u) => {
                        fin = u;
                        siguiente = u;
                    }
                    None => {
                        // Ni la palabra sola cabe: se parte por donde llega.
                        fin = i;
                        siguiente = i;
                    }
                }
                break;
            }
            suma += ancho;
            i += 1;
        }
        if i >= n && fin == n {
            siguiente = n;
        }
        let b = fin_sin_espacios(inicio, fin);
        mas_ancho = mas_ancho.max(ancho_de(inicio, fin));
        renglones.push((inicio as u32, b as u32));
        if siguiente <= inicio {
            // Nunca un renglon vacio que no avanza.
            siguiente = inicio + 1;
        }
        // Un salto al final del parrafo no abre otro renglon (un `<br>` al
        // final de un `<p>` tampoco).
        inicio = siguiente;
    }
    (renglones, mas_ancho, minimo)
}

// ---------------------------------------------------------------------------
// Con DirectWrite

fn formato(dwrite: &IDWriteFactory, letra: &LetraDeLectura, tam: f32) -> Option<IDWriteTextFormat> {
    crate::letras::formato_con_peso(
        dwrite,
        letra.familia,
        letra.peso,
        false,
        tam,
        letra.interlineado,
    )
}

/// Una disposicion en un solo renglon, con su letra y sus tramos.
fn en_un_renglon(
    dwrite: &IDWriteFactory,
    formato: &IDWriteTextFormat,
    texto: &[u16],
    ancho: f32,
    tramos: &[Tramo],
    letra: &LetraDeLectura,
) -> Option<IDWriteTextLayout> {
    let negrita = DWRITE_FONT_WEIGHT(i32::from(letra.peso.max(700)));
    // SAFETY: la disposicion copia el texto; el formato esta vivo; los
    // rangos se limitan al texto (DirectWrite recorta los que se pasen).
    unsafe {
        let d = dwrite
            .CreateTextLayout(texto, formato, ancho.max(1.0), f32::MAX)
            .ok()?;
        d.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP).ok()?;
        let alinear = match letra.alineacion {
            Alineacion::Izquierda => DWRITE_TEXT_ALIGNMENT_LEADING,
            Alineacion::Centro => DWRITE_TEXT_ALIGNMENT_CENTER,
            Alineacion::Derecha => DWRITE_TEXT_ALIGNMENT_TRAILING,
        };
        d.SetTextAlignment(alinear).ok()?;
        for t in tramos {
            let rango = DWRITE_TEXT_RANGE {
                startPosition: t.inicio,
                length: t.longitud,
            };
            if t.estilo.negrita {
                d.SetFontWeight(negrita, rango).ok()?;
            }
            if t.estilo.cursiva {
                d.SetFontStyle(DWRITE_FONT_STYLE_ITALIC, rango).ok()?;
            }
            if t.estilo.mono {
                d.SetFontFamilyName(w!("Courier New"), rango).ok()?;
                // La coleccion propia no la tiene: la del sistema.
                d.SetFontCollection(None, rango).ok()?;
            }
        }
        Some(d)
    }
}

/// Lo que avanza cada unidad UTF-16 del texto, con su letra y sus tramos.
fn avances(d: &IDWriteTextLayout, largo: usize) -> Option<Vec<f32>> {
    let mut cuantos = 0u32;
    // SAFETY: la primera llamada solo pide cuantos racimos hay (falla con
    // «buffer insuficiente», que es lo esperado); la segunda escribe en un
    // vector de ese tamano.
    unsafe {
        let _ = d.GetClusterMetrics(None, &mut cuantos);
        let mut m = vec![DWRITE_CLUSTER_METRICS::default(); cuantos as usize];
        if cuantos > 0 {
            d.GetClusterMetrics(Some(&mut m), &mut cuantos).ok()?;
        }
        let mut v = vec![0.0f32; largo];
        let mut i = 0usize;
        for c in m.iter().take(cuantos as usize) {
            if i < largo {
                v[i] = c.width;
            }
            i += c.length as usize;
        }
        Some(v)
    }
}

/// Los tramos que caen en `a..b`, contados desde `a`.
fn tramos_de(tramos: &[Tramo], a: u32, b: u32) -> Vec<Tramo> {
    tramos
        .iter()
        .filter_map(|t| {
            let i = t.inicio.max(a);
            let f = (t.inicio + t.longitud).min(b);
            (f > i).then(|| Tramo {
                inicio: i - a,
                longitud: f - i,
                estilo: t.estilo,
            })
        })
        .collect()
}

/// **Mide un parrafo** a un ancho: sus renglones, partidos como en el movil.
pub(crate) fn medir(
    dwrite: &IDWriteFactory,
    texto: &str,
    tam: f32,
    ancho_max: f32,
    tramos: &[Tramo],
    letra: &LetraDeLectura,
) -> Option<Medida> {
    let formato = formato(dwrite, letra, tam)?;
    let unidades: Vec<u16> = texto.encode_utf16().collect();
    let d = en_un_renglon(dwrite, &formato, &unidades, 1.0e6, tramos, letra)?;
    let anchos = avances(&d, unidades.len())?;
    let (renglones, ancho, minimo) = partir(&unidades, &anchos, ancho_max);
    Some(Medida {
        ancho,
        alto: renglones.len() as f32 * tam * letra.interlineado,
        minimo,
        renglones,
    })
}

/// Cada renglon con su disposicion y su y: lo que se pinta y se toca.
fn disposiciones(
    dwrite: &IDWriteFactory,
    texto: &str,
    tam: f32,
    ancho_max: f32,
    tramos: &[Tramo],
    letra: &LetraDeLectura,
    renglones: &[(u32, u32)],
) -> Option<Vec<(IDWriteTextLayout, u32, f32)>> {
    let formato = formato(dwrite, letra, tam)?;
    let unidades: Vec<u16> = texto.encode_utf16().collect();
    let medidos;
    let renglones = if renglones.is_empty() {
        let d = en_un_renglon(dwrite, &formato, &unidades, 1.0e6, tramos, letra)?;
        let anchos = avances(&d, unidades.len())?;
        medidos = partir(&unidades, &anchos, ancho_max).0;
        &medidos[..]
    } else {
        renglones
    };
    let renglon = tam * letra.interlineado;
    let mut v = Vec::with_capacity(renglones.len());
    for (k, &(a, b)) in renglones.iter().enumerate() {
        let (a, b) = (a.min(unidades.len() as u32), b.min(unidades.len() as u32));
        let trozo = &unidades[a as usize..b.max(a) as usize];
        let d = en_un_renglon(
            dwrite,
            &formato,
            trozo,
            ancho_max,
            &tramos_de(tramos, a, b),
            letra,
        )?;
        v.push((d, a, k as f32 * renglon));
    }
    Some(v)
}

impl Pintor<'_> {
    /// Pinta un parrafo del documento desde su esquina, **renglon a
    /// renglon** por donde se partio al medir (`renglones`; vacio, se parte
    /// aqui).
    #[allow(clippy::too_many_arguments)] // texto, posicion, tamano, ancho, tramos, letra, renglones y color
    pub fn parrafo_de_lectura(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        ancho_max: f32,
        tramos: &[Tramo],
        letra: &LetraDeLectura,
        renglones: &[(u32, u32)],
        color: Color,
    ) {
        let Some(lineas) = disposiciones(
            self.motor.dwrite(),
            texto,
            tam,
            ancho_max,
            tramos,
            letra,
            renglones,
        ) else {
            return;
        };
        let Some(p) = self.motor.pincel(color) else {
            return;
        };
        for (d, _, dy) in &lineas {
            self.motor.conto(|c| c.disposiciones += 1);
            // SAFETY: dentro del fotograma; objetos vivos.
            unsafe {
                self.motor.contexto().DrawTextLayout(
                    Vector2 { X: x, Y: y + dy },
                    d,
                    &p,
                    Default::default(),
                )
            };
        }
    }

    /// Mide un parrafo del documento dentro de un fotograma.
    pub fn medir_de_lectura(
        &self,
        texto: &str,
        tam: f32,
        ancho_max: f32,
        tramos: &[Tramo],
        letra: &LetraDeLectura,
    ) -> Medida {
        self.motor
            .medir_de_lectura(texto, tam, ancho_max, tramos, letra)
    }

    /// **Donde cae un trozo de un parrafo del documento** (buscar dentro):
    /// las cajas de las letras `inicio..inicio+largo` con el origen en la
    /// esquina del parrafo, preguntadas a los MISMOS renglones que se pintan.
    #[allow(clippy::too_many_arguments)] // texto, tamano, ancho, tramos, letra, renglones y trozo
    pub fn cajas_de_lectura(
        &self,
        texto: &str,
        tam: f32,
        ancho_max: f32,
        tramos: &[Tramo],
        letra: &LetraDeLectura,
        renglones: &[(u32, u32)],
        inicio: u32,
        largo: u32,
    ) -> Vec<RectF> {
        let Some(lineas) = disposiciones(
            self.motor.dwrite(),
            texto,
            tam,
            ancho_max,
            tramos,
            letra,
            renglones,
        ) else {
            return Vec::new();
        };
        let fin = inicio + largo;
        let mut v = Vec::new();
        for (k, (d, a, dy)) in lineas.iter().enumerate() {
            let b = renglones.get(k).map_or(u32::MAX, |r| r.1).max(*a);
            let (i, f) = (inicio.max(*a), fin.min(b));
            if f <= i {
                continue;
            }
            v.extend(
                crate::lienzo::cajas_de_disposicion(d, i - a, f - i)
                    .into_iter()
                    .map(|r| RectF { y: r.y + dy, ..r }),
            );
        }
        v
    }
}

impl MotorRender {
    /// Mide un parrafo del documento **fuera** de un fotograma: quien
    /// exporta la pagina web mide sin ventana, con la misma disposicion.
    pub fn medir_de_lectura(
        &self,
        texto: &str,
        tam: f32,
        ancho_max: f32,
        tramos: &[Tramo],
        letra: &LetraDeLectura,
    ) -> Medida {
        medir(self.dwrite(), texto, tam, ancho_max, tramos, letra).unwrap_or_default()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::lienzo::EstiloTexto;

    fn dwrite() -> IDWriteFactory {
        // SAFETY: crear la fabrica compartida no toma punteros del llamante.
        unsafe {
            windows::Win32::Graphics::DirectWrite::DWriteCreateFactory(
                windows::Win32::Graphics::DirectWrite::DWRITE_FACTORY_TYPE_SHARED,
            )
            .expect("fabrica")
        }
    }

    fn letra(familia: &str) -> LetraDeLectura<'_> {
        LetraDeLectura {
            familia,
            peso: 400,
            interlineado: 1.55,
            alineacion: Alineacion::Izquierda,
        }
    }

    fn u(t: &str) -> Vec<u16> {
        t.encode_utf16().collect()
    }

    /// Diez de ancho cada letra.
    fn diez(t: &str) -> (Vec<u16>, Vec<f32>) {
        let v = u(t);
        let n = v.len();
        (v, vec![10.0; n])
    }

    const TEXTO: &str = "Valentin Mendez, J. C. (2026). Herramientas de inteligencia artificial para su \
                         aplicacion en la fase de construccion de proyectos de ingenieria civil.";

    #[test]
    fn se_parte_por_los_espacios_y_el_espacio_del_final_cuelga() {
        let (t, a) = diez("uno dos tres");
        let (r, ancho, minimo) = partir(&t, &a, 75.0);
        assert_eq!(r, vec![(0, 7), (8, 12)], "«uno dos» (70) cabe; «tres» baja");
        assert_eq!(ancho, 70.0);
        assert_eq!(minimo, 40.0, "la palabra mas ancha");
        // A lo ancho de todo, un renglon.
        assert_eq!(partir(&t, &a, 1.0e6).0, vec![(0, 12)]);
    }

    #[test]
    fn como_el_movil_no_se_parte_un_enlace_por_su_barra_ni_una_letra_de_su_grado() {
        assert!(!se_parte_tras(u16::from(b'/'), u16::from(b'd')));
        assert!(!se_parte_tras(u16::from(b'.'), u16::from(b'o')));
        assert!(!se_parte_tras(u16::from(b'N'), 0xB0));
        assert!(
            !se_parte_tras(0xA0, u16::from(b'a')),
            "el espacio duro no parte"
        );
        // Si tras un guion entre letras o cifras, y alrededor de una raya larga.
        assert!(se_parte_tras(u16::from(b'-'), u16::from(b'4')));
        assert!(se_parte_tras(u16::from(b'a'), 0x2014));
        assert!(se_parte_tras(0x2014, u16::from(b'a')));
        assert!(!se_parte_tras(u16::from(b'-'), u16::from(b' ')));
        let (t, a) = diez("https://doi.org/10.1080/0144619005002489");
        let (_, _, minimo) = partir(&t, &a, 100.0);
        assert_eq!(minimo, 400.0, "el enlace entero es el minimo");
    }

    #[test]
    fn una_palabra_que_no_cabe_sola_se_parte_por_donde_llega_y_un_salto_parte_siempre() {
        let (t, a) = diez("ab abcdefgh");
        let (r, _, _) = partir(&t, &a, 50.0);
        assert_eq!(r, vec![(0, 2), (3, 8), (8, 11)]);
        let (t, a) = diez("uno\ndos");
        assert_eq!(partir(&t, &a, 1.0e6).0, vec![(0, 3), (4, 7)]);
        // Caso negativo: un texto vacio es un renglon (el del `&nbsp;`).
        assert_eq!(partir(&[], &[], 100.0).0, vec![(0, 0)]);
    }

    #[test]
    fn el_alto_de_un_parrafo_son_sus_renglones_por_el_interlineado_de_css() {
        let d = dwrite();
        let m = medir(&d, TEXTO, 16.0, 352.0, &[], &letra("Noto Serif")).expect("mide");
        let renglones = m.renglones.len() as f32;
        assert!(renglones >= 2.0, "{m:?}");
        assert!(
            (m.alto - renglones * 24.8).abs() < 0.01,
            "cada renglon 16 x 1,55 = 24,8: {m:?}"
        );
        assert!(m.ancho <= 352.0 + 0.01);
        // Caso negativo: a otro interlineado, otro alto con los mismos renglones.
        let m2 = medir(
            &d,
            TEXTO,
            16.0,
            352.0,
            &[],
            &LetraDeLectura {
                interlineado: 1.25,
                ..letra("Noto Serif")
            },
        )
        .expect("mide");
        assert!((m2.alto - renglones * 20.0).abs() < 0.01, "{m2:?}");
    }

    #[test]
    fn la_serif_y_la_sans_del_movil_parten_distinto_y_el_minimo_es_la_palabra_mas_ancha() {
        if !crate::letras::disponibles() {
            return;
        }
        let d = dwrite();
        let serif = medir(&d, TEXTO, 16.0, 1.0e6, &[], &letra("Noto Serif")).expect("mide");
        let sans = medir(&d, TEXTO, 16.0, 1.0e6, &[], &letra("Roboto")).expect("mide");
        assert!(
            serif.ancho > sans.ancho + 20.0,
            "Noto Serif es mas ancha que Roboto: {serif:?} {sans:?}"
        );
        let palabra =
            medir(&d, "construccion", 16.0, 1.0e6, &[], &letra("Noto Serif")).expect("mide");
        let frase = medir(
            &d,
            "la construccion",
            16.0,
            1.0e6,
            &[],
            &letra("Noto Serif"),
        )
        .expect("mide");
        assert!(
            (frase.minimo - palabra.ancho).abs() < 0.5,
            "{frase:?} {palabra:?}"
        );
    }

    #[test]
    fn un_tramo_en_negrita_ensancha_y_uno_en_cursiva_de_la_serif_estrecha() {
        if !crate::letras::disponibles() {
            return;
        }
        let d = dwrite();
        let todo = |estilo| {
            [Tramo {
                inicio: 0,
                longitud: 200,
                estilo,
            }]
        };
        let medir = |tramos: &[Tramo]| {
            medir(&d, TEXTO, 16.0, 1.0e6, tramos, &letra("Noto Serif"))
                .unwrap()
                .ancho
        };
        let normal = medir(&[]);
        let negrita = medir(&todo(EstiloTexto {
            negrita: true,
            ..Default::default()
        }));
        let cursiva = medir(&todo(EstiloTexto {
            cursiva: true,
            ..Default::default()
        }));
        assert!(negrita > normal + 10.0, "{negrita} {normal}");
        assert!(
            cursiva < normal - 10.0,
            "la cursiva de verdad, no la inclinada: {cursiva} {normal}"
        );
    }

    #[test]
    fn los_tramos_de_un_renglon_se_cuentan_desde_su_principio() {
        let t = [
            Tramo {
                inicio: 2,
                longitud: 6,
                estilo: EstiloTexto {
                    negrita: true,
                    ..Default::default()
                },
            },
            Tramo {
                inicio: 20,
                longitud: 3,
                estilo: EstiloTexto::default(),
            },
        ];
        let r = tramos_de(&t, 5, 12);
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].inicio, r[0].longitud), (0, 3));
    }
}
