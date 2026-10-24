//! **Las letras propias**: las de Excalidraw y las del lienzo de citas,
//! metidas en el ejecutable y cargadas en una coleccion de DirectWrite que
//! solo ve esta aplicacion.
//!
//! # Por que dentro del binario y no instaladas
//!
//! Instalar fuentes pide permisos y deja rastro en el sistema del usuario, y
//! pedirle «instala Excalifont» a quien solo quiere dibujar no es opcion. El
//! movil hace lo mismo con sus TTF en `res/font`. Aqui van en woff2 y **solo
//! el subconjunto latino** (el mismo trozo que Excalidraw y Fontsource sirven
//! para `U+0000-00FF`): 209 KB las ocho caras, frente a los 540 KB de las
//! tres TTF completas del movil. Lo que no este en el subconjunto (un
//! ideograma, un emoji) lo busca DirectWrite en las del sistema, que es su
//! reserva de siempre.
//!
//! # Como se cargan
//!
//! `IDWriteFactory5` (Windows 10 1703 en adelante) sabe desempaquetar woff2
//! (`UnpackFontFile`) y guardar fuentes en memoria
//! (`CreateInMemoryFontFileLoader`); con eso se arma un `IDWriteFontSet` y de
//! ahi la coleccion. En un Windows mas viejo `cast` falla y todo cae a Segoe
//! UI: se sigue viendo el texto, con otra letra, que es lo que decide el
//! movil cuando su fuente no carga.
//!
//! La coleccion se hace **una vez por hilo** y a la primera que se pide (unos
//! pocos milisegundos): los objetos COM de DirectWrite no son `Send` en
//! `windows`, y cada ventana pinta en su hilo.
//!
//! # El interlineado
//!
//! Con `Letra::interlineado` puesto, los renglones van a una distancia fija
//! de `tam * interlineado` (`DWRITE_LINE_SPACING_METHOD_UNIFORM`) con la
//! linea base centrada como la centra CSS, que es como pinta Excalidraw
//! (`getVerticalOffset`). Asi el alto de un texto es exactamente
//! `renglones * tam * interlineado`, que es lo que el motor escribe en su
//! caja, y no depende de las metricas de cada letra.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;

use windows::Win32::Graphics::DirectWrite::{
    DWRITE_CONTAINER_TYPE_WOFF2, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_METRICS,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_LINE_SPACING_METHOD_UNIFORM,
    DWRITE_TEXT_METRICS, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, IDWriteFactory,
    IDWriteFactory5, IDWriteFontCollection, IDWriteTextFormat, IDWriteTextLayout,
};
use windows::core::{BOOL, HSTRING, Interface, w};

/// Las familias que trae la aplicacion, con el nombre con que se piden.
///
/// Es el nombre que el motor guarda en `Figura::Texto::familia` y el que
/// Excalidraw escribe en su CSS: pedirlas por otro nombre seria tener que
/// traducirlo en cada sitio que pinta.
pub const FAMILIAS_PROPIAS: [&str; 9] = [
    "Excalifont",
    "Nunito",
    "Lilita One",
    "Comic Shanns",
    "Work Sans",
    "Fraunces",
    "Caveat",
    // Las del lector de documentos: «serif» y «sans-serif» del WebView de
    // Android (ver `lectura`).
    "Noto Serif",
    "Roboto",
];

/// Los ficheros, cada uno con la familia a la que pertenece. Caveat lleva dos
/// caras (500 y 700) porque la app de citas escribe su «Manuscrita» en 500 y
/// la negrita de una letra a mano simulada se ve emborronada.
const FICHEROS: [(&str, &[u8]); 16] = [
    ("Excalifont", include_bytes!("../letras/excalifont.woff2")),
    ("Nunito", include_bytes!("../letras/nunito.woff2")),
    ("Lilita One", include_bytes!("../letras/lilita-one.woff2")),
    (
        "Comic Shanns",
        include_bytes!("../letras/comic-shanns.woff2"),
    ),
    ("Work Sans", include_bytes!("../letras/work-sans-400.woff2")),
    ("Fraunces", include_bytes!("../letras/fraunces-600.woff2")),
    ("Caveat", include_bytes!("../letras/caveat-500.woff2")),
    ("Caveat", include_bytes!("../letras/caveat-700.woff2")),
    // Las cuatro caras que trae Android de su serif (y las mismas de Roboto):
    // pedir 600 u 800 cae en la 700 aqui como alli.
    (
        "Noto Serif",
        include_bytes!("../letras/noto-serif-400-normal.woff2"),
    ),
    (
        "Noto Serif",
        include_bytes!("../letras/noto-serif-700-normal.woff2"),
    ),
    (
        "Noto Serif",
        include_bytes!("../letras/noto-serif-400-italic.woff2"),
    ),
    (
        "Noto Serif",
        include_bytes!("../letras/noto-serif-700-italic.woff2"),
    ),
    (
        "Roboto",
        include_bytes!("../letras/roboto-400-normal.woff2"),
    ),
    (
        "Roboto",
        include_bytes!("../letras/roboto-700-normal.woff2"),
    ),
    (
        "Roboto",
        include_bytes!("../letras/roboto-400-italic.woff2"),
    ),
    (
        "Roboto",
        include_bytes!("../letras/roboto-700-italic.woff2"),
    ),
];

/// La letra de reserva: la de siempre de Windows, y la que se usa si la
/// coleccion propia no se puede montar.
pub const LETRA_DEL_SISTEMA: &str = "Segoe UI";

/// **Con que se escribe un texto**: la familia, las dos marcas que cambian
/// la cara y, si se quiere, el interlineado fijo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Letra<'a> {
    pub familia: &'a str,
    pub negrita: bool,
    pub cursiva: bool,
    /// `None`: el de la propia fuente (lo de siempre para la interfaz).
    /// `Some(k)`: renglones a `tam * k` exactos, como Excalidraw.
    pub interlineado: Option<f32>,
}

impl<'a> Letra<'a> {
    /// Una familia sin marcas ni interlineado fijo.
    pub fn de(familia: &'a str) -> Self {
        Letra {
            familia,
            negrita: false,
            cursiva: false,
            interlineado: None,
        }
    }
}

/// Si `familia` es de las que trae la aplicacion (sin distinguir mayusculas).
pub fn es_propia(familia: &str) -> bool {
    FAMILIAS_PROPIAS
        .iter()
        .any(|f| f.eq_ignore_ascii_case(familia.trim()))
}

/// **El nombre con que DirectWrite conoce cada fichero**, cuando no es el
/// de la familia. Medido cargandolos (ver `nombres_cargados`): los woff2 de
/// Google son cortes de fuentes variables y su tabla de nombres dice el de
/// la instancia por omision -«Nunito ExtraLight», aunque la cara que trae es
/// la Medium 500 que usa Excalidraw-, y el de Comic Shanns lleva el
/// «Regular» pegado. Pedirlas por el nombre de la familia a secas no las
/// encontraria y saldrian en Segoe UI sin avisar.
const NOMBRES_REALES: [(&str, &str); 3] = [
    ("Nunito", "Nunito ExtraLight"),
    ("Comic Shanns", "Comic Shanns Regular"),
    ("Fraunces", "Fraunces NonWonky"),
];

/// El nombre de familia que hay que pedir a la coleccion propia.
fn nombre_real(familia: &str) -> &str {
    let f = familia.trim();
    NOMBRES_REALES
        .iter()
        .find(|(l, _)| l.eq_ignore_ascii_case(f))
        .map_or(f, |(_, r)| r)
}

/// La coleccion propia de este hilo, montada la primera vez.
// La clave (familia, negrita, cursiva) se lee mejor en linea que tras un alias.
#[allow(clippy::type_complexity)]
struct Propias {
    coleccion: IDWriteFontCollection,
    /// `(ascenso, descenso)` en fracciones de em, por familia y marcas: la
    /// linea base del interlineado fijo sale de aqui.
    alturas: RefCell<HashMap<(String, bool, bool), Option<(f32, f32)>>>,
}

thread_local! {
    static PROPIAS: OnceCell<Option<Propias>> = const { OnceCell::new() };
    /// La fabrica compartida, para medir fuera de un fotograma.
    static FABRICA: OnceCell<Option<IDWriteFactory>> = const { OnceCell::new() };
}

fn fabrica_compartida() -> Option<IDWriteFactory> {
    FABRICA.with(|f| {
        f.get_or_init(|| {
            // SAFETY: crear la fabrica compartida no toma punteros del
            // llamante; es la misma que devuelve a `MotorRender`.
            unsafe { DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED).ok() }
        })
        .clone()
    })
}

/// Desempaqueta los woff2 y arma la coleccion. `None` en un Windows sin
/// `IDWriteFactory5` o si algo falla: quien pinta cae entonces a Segoe UI.
fn montar(dwrite: &IDWriteFactory) -> Option<Propias> {
    let f5: IDWriteFactory5 = dwrite.cast().ok()?;
    // SAFETY: todos los punteros son de rebanadas `'static` o de fragmentos
    // que DirectWrite presta y se devuelven con `ReleaseFileFragment` tras
    // copiarlos; el cargador en memoria COPIA los datos (propietario nulo),
    // asi que nada queda apuntando a memoria de esta funcion.
    unsafe {
        let cargador = f5.CreateInMemoryFontFileLoader().ok()?;
        f5.RegisterFontFileLoader(&cargador).ok()?;
        let constructor = f5.CreateFontSetBuilder().ok()?;
        let mut alguna = false;
        for (_, datos) in FICHEROS {
            let Ok(flujo) = f5.UnpackFontFile(
                DWRITE_CONTAINER_TYPE_WOFF2,
                datos.as_ptr().cast(),
                datos.len() as u32,
            ) else {
                continue;
            };
            let Ok(largo) = flujo.GetFileSize() else {
                continue;
            };
            let (mut trozo, mut contexto) = (std::ptr::null_mut(), std::ptr::null_mut());
            if flujo
                .ReadFileFragment(&mut trozo, 0, largo, &mut contexto)
                .is_err()
                || trozo.is_null()
            {
                continue;
            }
            let copia = std::slice::from_raw_parts(trozo as *const u8, largo as usize).to_vec();
            flujo.ReleaseFileFragment(contexto);
            let Ok(fichero) = cargador.CreateInMemoryFontFileReference(
                dwrite,
                copia.as_ptr().cast(),
                copia.len() as u32,
                None,
            ) else {
                continue;
            };
            if constructor.AddFontFile(&fichero).is_ok() {
                alguna = true;
            }
        }
        if !alguna {
            return None;
        }
        let juego = constructor.CreateFontSet().ok()?;
        let coleccion: IDWriteFontCollection = f5
            .CreateFontCollectionFromFontSet(&juego)
            .ok()?
            .cast()
            .ok()?;
        Some(Propias {
            coleccion,
            alturas: RefCell::new(HashMap::new()),
        })
    }
}

fn con_propias<R>(dwrite: &IDWriteFactory, f: impl FnOnce(Option<&Propias>) -> R) -> R {
    PROPIAS.with(|p| f(p.get_or_init(|| montar(dwrite)).as_ref()))
}

/// Si la coleccion propia esta disponible en este Windows.
pub fn disponibles() -> bool {
    fabrica_compartida().is_some_and(|d| con_propias(&d, |p| p.is_some()))
}

/// Los nombres de familia que DirectWrite ve en la coleccion propia: para
/// comprobar que cada fichero dice llamarse como se le pide.
pub fn nombres_cargados() -> Vec<String> {
    let Some(d) = fabrica_compartida() else {
        return Vec::new();
    };
    con_propias(&d, |p| {
        let Some(p) = p else {
            return Vec::new();
        };
        let mut v = Vec::new();
        // SAFETY: lecturas de la coleccion viva; el bufer de cada nombre se
        // dimensiona con `GetStringLength` mas el cero final.
        unsafe {
            for i in 0..p.coleccion.GetFontFamilyCount() {
                let Ok(fam) = p.coleccion.GetFontFamily(i) else {
                    continue;
                };
                let Ok(nombres) = fam.GetFamilyNames() else {
                    continue;
                };
                let largo = nombres.GetStringLength(0).unwrap_or(0) as usize;
                let mut b = vec![0u16; largo + 1];
                if nombres.GetString(0, &mut b).is_ok() {
                    v.push(String::from_utf16_lossy(&b[..largo]));
                }
            }
        }
        v
    })
}

/// El peso que se pide. **Caveat normal se pide «fina»** (100): su cara 500
/// dice pesar 1 en la tabla OS/2 del woff2 de Fontsource, y pidiendo 400
/// DirectWrite la descarta por la 700 (lo mide la prueba de abajo). Pidiendo
/// 100 la mas cercana es esa 500, que es la «Manuscrita» del lienzo de citas.
fn peso_de(
    familia: &str,
    negrita: bool,
) -> windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT {
    if negrita {
        DWRITE_FONT_WEIGHT_BOLD
    } else if familia.trim().eq_ignore_ascii_case("Caveat") {
        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT_THIN
    } else {
        DWRITE_FONT_WEIGHT_NORMAL
    }
}

fn estilo(cursiva: bool) -> windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STYLE {
    if cursiva {
        DWRITE_FONT_STYLE_ITALIC
    } else {
        DWRITE_FONT_STYLE_NORMAL
    }
}

/// `(ascenso, descenso)` de la cara que DirectWrite elegiria, en em.
fn alturas_de(
    coleccion: &IDWriteFontCollection,
    familia: &str,
    negrita: bool,
    cursiva: bool,
) -> Option<(f32, f32)> {
    // SAFETY: consultas de solo lectura sobre objetos vivos; `metricas` es
    // una estructura local que DirectWrite rellena.
    unsafe {
        let (mut indice, mut existe) = (0u32, BOOL(0));
        coleccion
            .FindFamilyName(&HSTRING::from(familia), &mut indice, &mut existe)
            .ok()?;
        if !existe.as_bool() {
            return None;
        }
        let cara = coleccion
            .GetFontFamily(indice)
            .ok()?
            .GetFirstMatchingFont(
                peso_de(familia, negrita),
                DWRITE_FONT_STRETCH_NORMAL,
                estilo(cursiva),
            )
            .ok()?;
        let mut m = DWRITE_FONT_METRICS::default();
        cara.GetMetrics(&mut m);
        let em = m.designUnitsPerEm.max(1) as f32;
        Some((m.ascent as f32 / em, m.descent as f32 / em))
    }
}

/// **El formato de texto de una letra**: la familia buscada en la coleccion
/// propia si es de las nuestras, en la del sistema si no, y Segoe UI si la
/// propia no se pudo montar.
pub(crate) fn formato(
    dwrite: &IDWriteFactory,
    letra: &Letra,
    tam: f32,
) -> Option<IDWriteTextFormat> {
    let propia = es_propia(letra.familia);
    con_propias(dwrite, |p| {
        let (nombre, coleccion): (&str, Option<&IDWriteFontCollection>) = match (propia, p) {
            (true, Some(p)) => (nombre_real(letra.familia), Some(&p.coleccion)),
            (true, None) => (LETRA_DEL_SISTEMA, None),
            (false, _) => (letra.familia, None),
        };
        let nombre = if nombre.trim().is_empty() {
            LETRA_DEL_SISTEMA
        } else {
            nombre
        };
        // SAFETY: cadenas propias vivas durante la llamada; el formato copia
        // el nombre y guarda su propia referencia a la coleccion.
        let formato = unsafe {
            dwrite
                .CreateTextFormat(
                    &HSTRING::from(nombre),
                    coleccion,
                    peso_de(nombre, letra.negrita),
                    estilo(letra.cursiva),
                    DWRITE_FONT_STRETCH_NORMAL,
                    tam.max(0.01),
                    w!("es-ES"),
                )
                .ok()?
        };
        if let Some(k) = letra.interlineado {
            let clave = (nombre.to_string(), letra.negrita, letra.cursiva);
            let alturas = match (coleccion, p) {
                (Some(c), Some(p)) => *p
                    .alturas
                    .borrow_mut()
                    .entry(clave)
                    .or_insert_with(|| alturas_de(c, nombre, letra.negrita, letra.cursiva)),
                _ => sistema(dwrite, nombre, letra),
            };
            // Sin metricas, el reparto de una letra corriente (80/20).
            let (asc, desc) = alturas.unwrap_or((0.8, 0.2));
            let linea = tam * k;
            let base = tam * asc + (linea - tam * (asc + desc)) / 2.0;
            // SAFETY: formato recien creado y vivo.
            unsafe {
                formato
                    .SetLineSpacing(DWRITE_LINE_SPACING_METHOD_UNIFORM, linea, base)
                    .ok()?;
            }
        }
        Some(formato)
    })
}

/// **El formato de un texto del lector de documentos** (`lectura`): como
/// [`formato`], pero con el peso de CSS tal cual (300, 400, 600, 800…) y el
/// interlineado siempre fijo, que es lo que hace `line-height:1.55` en la
/// pagina del movil. Sin una cara de ese peso, DirectWrite toma la mas
/// cercana, igual que el navegador.
pub(crate) fn formato_con_peso(
    dwrite: &IDWriteFactory,
    familia: &str,
    peso: u16,
    cursiva: bool,
    tam: f32,
    interlineado: f32,
) -> Option<IDWriteTextFormat> {
    let propia = es_propia(familia);
    con_propias(dwrite, |p| {
        let (nombre, coleccion): (&str, Option<&IDWriteFontCollection>) = match (propia, p) {
            (true, Some(p)) => (nombre_real(familia), Some(&p.coleccion)),
            (true, None) => (LETRA_DEL_SISTEMA, None),
            (false, _) => (familia, None),
        };
        let nombre = if nombre.trim().is_empty() {
            LETRA_DEL_SISTEMA
        } else {
            nombre
        };
        // SAFETY: cadenas propias vivas durante la llamada; el formato copia
        // el nombre y guarda su propia referencia a la coleccion.
        let formato = unsafe {
            dwrite
                .CreateTextFormat(
                    &HSTRING::from(nombre),
                    coleccion,
                    windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT(i32::from(
                        peso.clamp(1, 999),
                    )),
                    estilo(cursiva),
                    DWRITE_FONT_STRETCH_NORMAL,
                    tam.max(0.01),
                    w!("es-ES"),
                )
                .ok()?
        };
        let negrita = peso >= 600;
        let letra = Letra {
            familia: nombre,
            negrita,
            cursiva,
            interlineado: None,
        };
        let alturas = match (coleccion, p) {
            (Some(c), Some(p)) => *p
                .alturas
                .borrow_mut()
                .entry((nombre.to_string(), negrita, cursiva))
                .or_insert_with(|| alturas_de(c, nombre, negrita, cursiva)),
            _ => sistema(dwrite, nombre, &letra),
        };
        let (asc, desc) = alturas.unwrap_or((0.8, 0.2));
        // La linea base centrada en el renglon, como el «medio interlineado»
        // de CSS: lo que sobra se reparte arriba y abajo.
        let linea = tam * interlineado.max(0.1);
        let base = tam * asc + (linea - tam * (asc + desc)) / 2.0;
        // SAFETY: formato recien creado y vivo.
        unsafe {
            formato
                .SetLineSpacing(DWRITE_LINE_SPACING_METHOD_UNIFORM, linea, base)
                .ok()?;
        }
        Some(formato)
    })
}

/// Las alturas de una familia del sistema (Segoe UI, Courier New).
fn sistema(dwrite: &IDWriteFactory, familia: &str, letra: &Letra) -> Option<(f32, f32)> {
    let mut coleccion = None;
    // SAFETY: pide la coleccion del sistema a la fabrica viva.
    unsafe { dwrite.GetSystemFontCollection(&mut coleccion, false).ok()? };
    alturas_de(&coleccion?, familia, letra.negrita, letra.cursiva)
}

/// **La disposicion de un texto con su letra.** Un texto suelto del lienzo
/// no se parte nunca (`sin_partir`): sus renglones son los que escribio
/// quien lo escribio, como en Excalidraw, y la caja se ajusta a ellos y no
/// al reves.
pub(crate) fn disposicion(
    dwrite: &IDWriteFactory,
    texto: &str,
    tam: f32,
    ancho_max: f32,
    letra: &Letra,
) -> Option<(IDWriteTextLayout, f32, f32)> {
    let formato = formato(dwrite, letra, tam)?;
    let contenido: Vec<u16> = texto.encode_utf16().collect();
    // SAFETY: la disposicion copia el texto; formato vivo.
    unsafe {
        let d = dwrite
            .CreateTextLayout(&contenido, &formato, ancho_max.max(1.0), f32::MAX)
            .ok()?;
        if ancho_max >= SIN_PARTIR {
            d.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP).ok()?;
        }
        let mut m = DWRITE_TEXT_METRICS::default();
        d.GetMetrics(&mut m).ok()?;
        // Con los espacios del final: el cursor detras de «hola » tiene que
        // caer detras del espacio, y la caja de Excalidraw tambien lo cuenta.
        Some((d, m.widthIncludingTrailingWhitespace, m.height))
    }
}

/// A partir de este ancho la disposicion no parte renglones.
pub const SIN_PARTIR: f32 = 1.0e6;

/// **Lo que ocupa un texto escrito con `letra`**, medido con DirectWrite y
/// sin fotograma: lo que necesita el motor para ajustar la caja de un texto
/// a lo escrito. Los renglones vacios cuentan como un espacio, como en
/// Excalidraw (`measureText`): si midieran cero, el texto recien abierto no
/// tendria donde pintar el cursor.
///
/// `None` si DirectWrite no contesta; quien llama usa entonces su cuenta a
/// ojo.
pub fn medir(texto: &str, tam: f32, letra: &Letra) -> Option<(f32, f32)> {
    let dwrite = fabrica_compartida()?;
    let relleno: String = texto
        .split('\n')
        .map(|r| if r.is_empty() { " " } else { r })
        .collect::<Vec<_>>()
        .join("\n");
    let (_, ancho, alto) = disposicion(&dwrite, &relleno, tam, SIN_PARTIR, letra)?;
    // **El avance no es lo que ocupa la letra** (lo mismo que mide el movil
    // en `DrawFonts.medirTexto`): en una letra a mano el rabo de la ultima
    // se sale del avance. Lo que sobresale por la derecha es tinta que
    // tambien hay que reservar. `GetOverhangMetrics` lo da relativo a la
    // caja de maquetar, asi que se pregunta a una disposicion de ese ancho.
    let sobra = sobresale(&dwrite, &relleno, tam, letra, ancho).unwrap_or(0.0);
    Some((ancho + sobra, alto))
}

/// **Las caras de una familia propia como fichero de fuente llano** (TTF u
/// OTF, desempaquetado del woff2), para quien no pinta con DirectWrite: el
/// `RichEdit` del editor de notas las registra en GDI
/// (`AddFontMemResourceEx`, solo para el proceso) y las pide por su nombre.
/// Vacio si la familia no es propia o este Windows no sabe desempaquetar.
pub fn ficheros_llanos(familia: &str) -> Vec<Vec<u8>> {
    let Some(dwrite) = fabrica_compartida() else {
        return Vec::new();
    };
    let Ok(f5) = dwrite.cast::<IDWriteFactory5>() else {
        return Vec::new();
    };
    let mut sal = Vec::new();
    for (_, datos) in FICHEROS
        .iter()
        .filter(|(f, _)| f.eq_ignore_ascii_case(familia.trim()))
    {
        // SAFETY: el woff2 es una rebanada `'static`; el fragmento que presta
        // DirectWrite se copia y se devuelve antes de soltar el flujo.
        unsafe {
            let Ok(flujo) = f5.UnpackFontFile(
                DWRITE_CONTAINER_TYPE_WOFF2,
                datos.as_ptr().cast(),
                datos.len() as u32,
            ) else {
                continue;
            };
            let Ok(largo) = flujo.GetFileSize() else {
                continue;
            };
            let (mut trozo, mut contexto) = (std::ptr::null_mut(), std::ptr::null_mut());
            if flujo
                .ReadFileFragment(&mut trozo, 0, largo, &mut contexto)
                .is_err()
                || trozo.is_null()
            {
                continue;
            }
            sal.push(std::slice::from_raw_parts(trozo as *const u8, largo as usize).to_vec());
            flujo.ReleaseFileFragment(contexto);
        }
    }
    sal
}

/// Cuanto sale la tinta por la derecha de un texto de ancho `ancho`.
fn sobresale(
    dwrite: &IDWriteFactory,
    texto: &str,
    tam: f32,
    letra: &Letra,
    ancho: f32,
) -> Option<f32> {
    let formato = formato(dwrite, letra, tam)?;
    let contenido: Vec<u16> = texto.encode_utf16().collect();
    // SAFETY: la disposicion copia el texto; formato vivo.
    unsafe {
        let d = dwrite
            .CreateTextLayout(&contenido, &formato, ancho.max(1.0), f32::MAX)
            .ok()?;
        d.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP).ok()?;
        Some(d.GetOverhangMetrics().ok()?.right.max(0.0))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_siete_familias_propias_cargan_con_el_nombre_con_que_se_piden() {
        if !disponibles() {
            // Windows anterior a 1703: no hay nada que probar aqui, la
            // reserva la prueba la de abajo.
            return;
        }
        let cargadas = nombres_cargados();
        for f in FAMILIAS_PROPIAS {
            assert!(
                cargadas
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(nombre_real(f))),
                "{f} no esta; se cargaron {cargadas:?}"
            );
        }
    }

    #[test]
    fn cada_familia_mide_distinto_y_una_desconocida_cae_a_la_del_sistema() {
        let texto = "Hola mundo";
        let segoe = medir(texto, 20.0, &Letra::de("Segoe UI")).expect("mide");
        let excali = medir(texto, 20.0, &Letra::de("Excalifont")).expect("mide");
        let caveat = medir(texto, 20.0, &Letra::de("Caveat")).expect("mide");
        if disponibles() {
            assert!((segoe.0 - excali.0).abs() > 1.0, "{segoe:?} {excali:?}");
            assert!((caveat.0 - excali.0).abs() > 1.0, "{caveat:?} {excali:?}");
        }
        // Caso negativo: una familia que no existe no deja el texto sin
        // medir.
        let rara = medir(texto, 20.0, &Letra::de("No Existe Ninguna")).expect("mide");
        assert!(rara.0 > 0.0);
    }

    #[test]
    fn con_interlineado_fijo_el_alto_es_renglones_por_tam_por_interlineado() {
        let mut l = Letra::de("Excalifont");
        l.interlineado = Some(1.25);
        let (_, uno) = medir("a", 20.0, &l).expect("mide");
        let (_, tres) = medir("a\nb\nc", 20.0, &l).expect("mide");
        assert!((uno - 25.0).abs() < 0.01, "{uno}");
        assert!((tres - 75.0).abs() < 0.01, "{tres}");
    }

    #[test]
    fn los_espacios_del_final_cuentan_y_un_texto_vacio_no_mide_cero() {
        let l = Letra::de("Excalifont");
        let (sin, _) = medir("hola", 20.0, &l).expect("mide");
        let (con, _) = medir("hola ", 20.0, &l).expect("mide");
        assert!(con > sin, "el espacio del final no conto: {sin} {con}");
        // Caso negativo: vacio mide un espacio, no cero.
        let (vacio, alto) = medir("", 20.0, &l).expect("mide");
        assert!(vacio > 0.0 && alto > 0.0);
    }

    #[test]
    fn caveat_normal_es_la_cara_500_y_la_negrita_la_700() {
        // La cara 500 dice pesar 1 en su tabla OS/2: si DirectWrite eligiera
        // por cercania pura, la normal saldria con la cara gorda.
        if !disponibles() {
            return;
        }
        let peso_elegido = |negrita: bool| -> i32 {
            let d = fabrica_compartida().expect("fabrica");
            con_propias(&d, |p| {
                let p = p.expect("coleccion");
                // SAFETY: consultas de solo lectura sobre la coleccion viva.
                unsafe {
                    let (mut i, mut hay) = (0u32, BOOL(0));
                    p.coleccion
                        .FindFamilyName(&HSTRING::from("Caveat"), &mut i, &mut hay)
                        .expect("busca");
                    p.coleccion
                        .GetFontFamily(i)
                        .expect("familia")
                        .GetFirstMatchingFont(
                            peso_de("Caveat", negrita),
                            DWRITE_FONT_STRETCH_NORMAL,
                            DWRITE_FONT_STYLE_NORMAL,
                        )
                        .expect("cara")
                        .GetWeight()
                        .0
                }
            })
        };
        assert_eq!(peso_elegido(true), 700);
        assert_ne!(
            peso_elegido(false),
            700,
            "la normal salio con la cara gorda"
        );
    }

    #[test]
    fn nunito_pedida_por_su_nombre_de_familia_no_cae_a_segoe() {
        if !disponibles() {
            return;
        }
        let nunito = medir("Hola mundo", 20.0, &Letra::de("Nunito")).expect("mide");
        let segoe = medir("Hola mundo", 20.0, &Letra::de("Segoe UI")).expect("mide");
        let rara = medir("Hola mundo", 20.0, &Letra::de("Nunito Rara")).expect("mide");
        assert!((nunito.0 - segoe.0).abs() > 0.5, "{nunito:?} {segoe:?}");
        // Caso negativo: lo que no se llama asi no la encuentra.
        assert!((rara.0 - segoe.0).abs() < 0.01, "{rara:?} {segoe:?}");
    }

    #[test]
    fn la_negrita_ensancha() {
        let normal = medir("Hola mundo", 20.0, &Letra::de("Nunito")).expect("mide");
        let mut l = Letra::de("Nunito");
        l.negrita = true;
        let negrita = medir("Hola mundo", 20.0, &l).expect("mide");
        assert!(negrita.0 > normal.0, "{normal:?} {negrita:?}");
    }
}
