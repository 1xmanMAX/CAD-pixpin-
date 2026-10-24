//! **Exportar e imprimir el lienzo**: PNG, SVG, PDF, pagina web, copiar como
//! imagen e imprimir por marcos, como en el movil.
//!
//! Aqui no hay geometria ni formatos: las hojas las decide
//! `pixpin_motor2d::exportar`, el SVG y la pagina web los escribe el motor,
//! el PDF `pixpin_pdf::escribir`, la caja y el dialogo de imprimir son de
//! Windows (`pixpin_shell::exportar` y `pixpin_shell::imprimir`) y la
//! impresion es de Direct2D (`pixpin_render::imprimir`). Esto solo enchufa
//! unas piezas con otras y pinta las ordenes con el `Pintor`, que es lo unico
//! que necesita la GPU: el PNG y el papel.
//!
//! Lo usan dos sitios: el editor (atajos locales y el menu del clic derecho)
//! y el menu de la burbuja de un lienzo en el chat, que carga la hoja del
//! proyecto sin abrir el editor.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_motor2d::exportar::{self as ex, Alcance, Hoja, ID_PAPEL};
use pixpin_motor2d::exportar_html::{self, HojaWeb};
use pixpin_motor2d::exportar_svg::{self, Incrustada, OpcionesSvg};
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::{ColorRgba, Escena, EstiloTrazo};
use pixpin_render::{Color, Interpolacion, MotorRender, Pintor, RectF};
use pixpin_shell::exportar::{Eleccion, EntradaMenu, Formato, Que, Rotulos};
use pixpin_store::Catalogo;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// F11: imprimir con la vista previa del dialogo moderno de Windows.
mod imprimir;
/// La columna de una lamina rehecha con texto rico, a PNG (muestra a ojo).
#[cfg(test)]
mod muestra_texto_rico;
/// F18: los mosaicos salen pixelados en todo lo exportado, como en el movil.
pub(crate) mod tapado;

/// Lo que se pide desde un atajo o un menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Peticion {
    Exportar,
    CopiarPng,
    Imprimir,
    /// La hoja de compartir de toda la aplicacion (`crate::compartir`).
    Compartir,
}

const VK_C: u32 = b'C' as u32;
const VK_E: u32 = b'E' as u32;
const VK_P: u32 = b'P' as u32;
const VK_S: u32 = b'S' as u32;

/// **Los atajos del editor**, solo mientras el editor tiene el foco (no son
/// globales): `Ctrl+Mayus+E` exportar, `Ctrl+Mayus+C` copiar como PNG (el de
/// Excalidraw) y `Ctrl+P` imprimir, el de cualquier programa.
pub(crate) fn atajo(vk: u32, ctrl: bool, shift: bool, alt: bool) -> Option<Peticion> {
    match (vk, ctrl, shift, alt) {
        (VK_E, true, true, false) => Some(Peticion::Exportar),
        (VK_C, true, true, false) => Some(Peticion::CopiarPng),
        (VK_P, true, false, false) => Some(Peticion::Imprimir),
        // Compartir: el mismo atajo en el editor y en los lectores. Es el
        // «Guardar como» de muchos programas, y la hoja lo lleva dentro.
        (VK_S, true, true, false) => Some(Peticion::Compartir),
        _ => None,
    }
}

/// Los textos del editor, que no recibe el catalogo de la aplicacion: se
/// lee el idioma de los ajustes una vez, como hace `panel_dibujo`.
pub(crate) fn textos() -> &'static Catalogo {
    static TEXTOS: std::sync::OnceLock<Catalogo> = std::sync::OnceLock::new();
    TEXTOS.get_or_init(|| {
        let dir_exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        let appdata = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_default();
        let ajustes =
            pixpin_store::cargar(&pixpin_store::resolver(&dir_exe, &appdata)).unwrap_or_default();
        Catalogo::nuevo(pixpin_store::resolver_idioma(
            &pixpin_shell::entorno::locale_del_sistema(),
            ajustes.idioma,
        ))
    })
}

/// Lo ultimo que se eligio en la caja, para la siguiente vez en la sesion:
/// quien exporta cinco laminas a PDF no quiere volver a elegir PDF cinco
/// veces.
static ULTIMA: std::sync::Mutex<Option<Eleccion>> = std::sync::Mutex::new(None);

fn eleccion_inicial() -> Eleccion {
    ULTIMA.lock().ok().and_then(|g| *g).unwrap_or(Eleccion {
        formato: Formato::Png,
        que: Que::Todo,
        escala: 2,
        transparente: false,
    })
}

/// Un lienzo listo para exportar: la escena, lo elegido y de donde salen
/// los pixeles de sus imagenes y de su papel.
pub(crate) struct Lienzo<'a> {
    pub escena: &'a Escena,
    pub seleccion: &'a [u64],
    /// El papel de fondo (la foto o la pagina), con su tamano en la escena.
    pub papel: Option<(&'a ImagenRgba, f32, f32)>,
    pub fotos: &'a dyn Fn(u64) -> Option<&'a ImagenRgba>,
    /// Como se sugiere llamar al fichero, sin extension.
    pub nombre: String,
}

impl Lienzo<'_> {
    fn imagen(&self, id: u64) -> Option<&ImagenRgba> {
        if id == ID_PAPEL {
            self.papel.map(|(i, _, _)| i)
        } else {
            (self.fotos)(id)
        }
    }

    fn hojas(&self, alcance: Alcance) -> Vec<Hoja> {
        ex::hojas(
            self.escena,
            alcance,
            self.seleccion,
            self.papel.map(|(_, w, h)| (w, h)),
        )
    }

    fn hay_marcos(&self) -> bool {
        !pixpin_motor2d::marco::hojas_en_orden(&self.escena.elementos).is_empty()
    }
}

// El papel con que se exporta es el del lienzo (`Escena::fondo`, su
// `viewBackgroundColor`), no un blanco fijo: lo que sale es lo que se ve. Solo
// «fondo transparente» lo quita. Imprimir sigue sobre el papel de la
// impresora: tinta de color en toda la hoja es gastar sin que nadie lo pida.

/// El margen del papel al imprimir, en DIP: los 28 puntos del movil.
const MARGEN_IMPRESION: f32 = 28.0 / 72.0 * 96.0;

fn a_color(c: ColorRgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

fn a_tuplas(v: &[pixpin_motor2d::vector::Punto2]) -> Vec<(f32, f32)> {
    v.iter().map(|p| (p.x, p.y)).collect()
}

/// Las imagenes subidas, con su tamano en pixeles: el recorte de una imagen
/// se mide contra el, y preguntarselo al bitmap seria una llamada `unsafe`
/// que aqui no hace falta.
type Subidas = HashMap<u64, (ID2D1Bitmap1, u32, u32)>;

/// Sube a la GPU las imagenes que usan estas hojas. Antes de `dibujar`:
/// crear recursos con el `BeginDraw` abierto no se hace.
fn subir(motor: &MotorRender, hojas: &[Hoja], lienzo: &Lienzo<'_>) -> Subidas {
    let mut v = HashMap::new();
    for h in hojas {
        for o in &h.ordenes {
            if let Orden::Imagen { id_objeto, .. } = o
                && !v.contains_key(id_objeto)
                && let Some(img) = lienzo.imagen(*id_objeto)
            {
                match motor.bitmap_desde_pixeles_premultiplicado(img.ancho, img.alto, &img.pixeles)
                {
                    Ok(b) => {
                        v.insert(*id_objeto, (b, img.ancho, img.alto));
                    }
                    Err(e) => tracing::warn!(?e, id = id_objeto, "imagen que no se pudo exportar"),
                }
            }
        }
    }
    v
}

/// **Pinta una hoja** con el pintor de la pantalla, con la vista ya puesta.
/// Es la misma traduccion de ordenes que hace la capa (`capa.rs`), mas las
/// imagenes y las rayas discontinuas, y el grano de las tintas porosas con
/// la misma llamada que el editor (`Pintor::grano`), en una cache propia.
fn pintar_hoja(p: &Pintor<'_>, hoja: &Hoja, bitmaps: &Subidas) {
    let (x0, y0, x1, y1) = hoja.caja;
    let caja = RectF {
        x: x0,
        y: y0,
        ancho: x1 - x0,
        alto: y1 - y0,
    };
    // Propia y de usar y tirar: la del editor es de su dispositivo, y aqui se
    // pinta con otro (`motor_propio`).
    let mut telas = pixpin_render::CacheGrano::nueva();
    // El grafito, cada mapa justo antes de la orden que le toca, con el
    // mismo `Pintor::grafito` de la pantalla. Sin esto el PNG lo sacaba liso.
    let mut grafitos = hoja.grafitos.iter().peekable();
    let mut pintar_grafitos_hasta = |k: usize, telas: &mut pixpin_render::CacheGrano| {
        while let Some(g) = grafitos.next_if(|g| g.antes_de <= k) {
            let m = &g.mapa;
            let mapa = pixpin_render::MapaGrafito {
                rgba: &m.rgba,
                ancho: m.ancho,
                alto: m.alto,
                caja: m.caja(),
                huella: m.huella,
                angulo: m.angulo,
                centro: (m.centro.x, m.centro.y),
                id: m.id,
                generacion: m.generacion,
                sucio: None,
            };
            p.grafito(Some(&mut telas.grafito), &mapa, g.opacidad, 1.0);
        }
    };
    for (i, o) in hoja.ordenes.iter().enumerate() {
        pintar_grafitos_hasta(i, &mut telas);
        match o {
            Orden::Poligono { puntos, color } | Orden::Relleno { puntos, color } => {
                p.poligono(&a_tuplas(puntos), a_color(*color))
            }
            Orden::Tinta { contorno, color } => {
                let v = a_tuplas(contorno);
                p.tinta(&v, a_color(*color));
                if let Some(g) = ex::grano_de_la_orden(hoja, i) {
                    let tela = pixpin_motor2d::tinta::tejido(g.material);
                    // La llave de la silueta es la orden: en una hoja no hay
                    // dos con el mismo numero.
                    p.grano(
                        &mut telas,
                        (i as u64, 0),
                        &v,
                        &tela,
                        pixpin_motor2d::tinta::material::LADO_DEL_MOSAICO,
                        g.material as u32,
                        a_color(g.color),
                        g.paso,
                        g.inclinada,
                    );
                }
            }
            Orden::Polilinea {
                puntos,
                color,
                grosor,
                estilo,
            } => {
                let v = a_tuplas(puntos);
                if matches!(estilo, EstiloTrazo::Solido) {
                    p.polilinea(&v, *grosor, a_color(*color));
                } else {
                    p.polilinea_discontinua(&v, *grosor, a_color(*color));
                }
            }
            Orden::Velo { hueco, color } => p.velo(caja, &a_tuplas(hueco), a_color(*color)),
            // Con su letra, la misma que en pantalla (`dibujo::pintar::letra_de`).
            Orden::Texto {
                texto,
                x,
                y,
                tam,
                familia,
                color,
                ancho_max,
                negrita,
                cursiva,
            } => p.texto_con_letra(
                texto,
                *x,
                *y,
                *tam,
                *ancho_max,
                &crate::dibujo::pintar::letra_de(familia, *negrita, *cursiva),
                a_color(*color),
            ),
            // El numero de una cota, girado con su raya y con halo, igual que
            // en pantalla (`dibujo::pintar`).
            Orden::Rotulo {
                texto,
                x,
                y,
                tam,
                familia,
                color,
                halo,
                grosor_halo,
                centro,
                angulo,
            } => p.girado((centro.x, centro.y), *angulo, |p| {
                p.texto_con_halo(
                    texto,
                    *x,
                    *y,
                    *tam,
                    &crate::dibujo::pintar::letra_de(familia, false, false),
                    a_color(*color),
                    a_color(*halo),
                    *grosor_halo,
                )
            }),
            Orden::Imagen {
                id_objeto,
                x,
                y,
                ancho,
                alto,
                opacidad,
                recorte,
                angulo,
            } => {
                if let Some((b, bw, bh)) = bitmaps.get(id_objeto) {
                    // El trozo en pixeles del bitmap subido, que aqui es la
                    // imagen tal cual (`subir` no la reduce).
                    let fuente = recorte.and_then(|r| r.trozo_en(*bw as f32, *bh as f32));
                    // Girada como en pantalla, alrededor del centro de su caja.
                    p.girado((*x + *ancho / 2.0, *y + *alto / 2.0), *angulo, |p| {
                        p.bitmap_translucido(
                            b,
                            RectF {
                                x: *x,
                                y: *y,
                                ancho: *ancho,
                                alto: *alto,
                            },
                            fuente.map(|(a, b, c, d)| RectF {
                                x: a,
                                y: b,
                                ancho: c - a,
                                alto: d - b,
                            }),
                            Interpolacion::Cubica,
                            *opacidad,
                        );
                    });
                }
            }
        }
    }
    // Lo que va despues de la ultima orden.
    pintar_grafitos_hasta(usize::MAX, &mut telas);
}

/// Un dispositivo propio para exportar. Propio y no el de la ventana: asi
/// exportar no toca el destino, las caches ni el estado del contexto con el
/// que el editor pinta, y el chat, que no tiene ninguno, usa el mismo camino.
fn motor_propio() -> Result<(pixpin_capture::Dispositivo, MotorRender)> {
    let d = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para exportar")?;
    let m = MotorRender::nuevo(d.d3d()).context("sin motor para exportar")?;
    Ok((d, m))
}

/// **La hoja como imagen**, a `escala`, sobre `fondo` o transparente.
pub(crate) fn a_imagen(
    hoja: &Hoja,
    escala: f32,
    fondo: Option<ColorRgba>,
    lienzo: &Lienzo<'_>,
) -> Result<ImagenRgba> {
    let (w, h, k) = ex::tamano_png(hoja, escala);
    let (d, motor) = motor_propio()?;
    let bitmaps = subir(&motor, std::slice::from_ref(hoja), lienzo);
    let destino = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h)
        .context("sin textura para exportar")?;
    motor
        .dibujar(&destino.destino, |p| {
            match fondo {
                Some(c) => p.limpiar(a_color(c)),
                None => p.limpiar_transparente(),
            }
            p.poner_vista((0.0, 0.0), k, (-hoja.caja.0 * k, -hoja.caja.1 * k));
            pintar_hoja(p, hoja, &bitmaps);
            // Lo de dentro de las lupas, agrandado (`lupas::en_la_hoja`).
            super::lupas::en_la_hoja(
                p,
                lienzo.escena,
                hoja,
                k,
                lienzo.papel.map(|(_, w, h)| (w, h)),
                a_color(fondo.unwrap_or(lienzo.escena.fondo)),
                |p, h| pintar_hoja(p, h, &bitmaps),
            );
        })
        .context("no se pudo pintar la hoja")?;
    let (ancho, alto, pixeles) = destino.leer_rgba().context("no se pudo leer la hoja")?;
    Ok(ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
}

/// **Las miniaturas de unas figuras** (las del menu de figuras), de `lado`
/// pixeles, cada una sobre su cuadrito blanco: todas en una sola textura y
/// una sola lectura, que pintarlas de una en una tardaria lo que tarda la
/// GPU en ir y volver tantas veces. Las que no tienen nada que pintar salen
/// `None`.
pub(crate) fn miniaturas(
    figuras: &[pixpin_motor2d::biblioteca::FiguraGuardada],
    lado: u32,
) -> Result<Vec<Option<ImagenRgba>>> {
    const COLUMNAS: usize = 10;
    if figuras.is_empty() {
        return Ok(Vec::new());
    }
    let hojas: Vec<Option<Hoja>> = figuras
        .iter()
        .map(|f| {
            // Encogida, una raya de 1 px de una hoja entera no llega a un
            // pixel y la miniatura de un bloc saldria en blanco: cada trazo
            // conserva lo que se ve.
            let (x0, y0, x1, y1) = pixpin_motor2d::biblioteca::caja_de(&f.elementos)?;
            let k = (lado as f32 / (x1 - x0).max(1.0)).min(lado as f32 / (y1 - y0).max(1.0));
            let mut escena = Escena::nueva();
            for e in &f.elementos {
                let mut e = e.clone();
                e.grosor = e.grosor.max(0.9 / k);
                escena.anadir(e);
            }
            ex::hojas(&escena, Alcance::Todo, &[], None)
                .into_iter()
                .next()
        })
        .collect();
    let filas = figuras.len().div_ceil(COLUMNAS);
    let (w, h) = (lado * COLUMNAS as u32, lado * filas as u32);
    let (d, motor) = motor_propio()?;
    let destino = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h)
        .context("sin textura para las miniaturas")?;
    let sin_imagenes = Subidas::new();
    let aire = 4.0;
    motor
        .dibujar(&destino.destino, |p| {
            p.limpiar(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            });
            for (i, hoja) in hojas.iter().enumerate() {
                let Some(hoja) = hoja else { continue };
                let (ox, oy) = (
                    ((i % COLUMNAS) as u32 * lado) as f32,
                    ((i / COLUMNAS) as u32 * lado) as f32,
                );
                let util = lado as f32 - 2.0 * aire;
                let k = (util / hoja.ancho().max(1.0)).min(util / hoja.alto().max(1.0));
                let (dx, dy) = (
                    (util - hoja.ancho() * k) / 2.0 + aire,
                    (util - hoja.alto() * k) / 2.0 + aire,
                );
                p.poner_vista((ox + dx, oy + dy), k, (-hoja.caja.0 * k, -hoja.caja.1 * k));
                pintar_hoja(p, hoja, &sin_imagenes);
            }
        })
        .context("no se pudieron pintar las miniaturas")?;
    let (ancho, _, pixeles) = destino
        .leer_rgba()
        .context("no se pudieron leer las miniaturas")?;
    Ok((0..figuras.len())
        .map(|i| {
            hojas[i].as_ref()?;
            let (ox, oy) = ((i % COLUMNAS) as u32 * lado, (i / COLUMNAS) as u32 * lado);
            let mut v = Vec::with_capacity((lado * lado * 4) as usize);
            for fila in 0..lado {
                let desde = (((oy + fila) * ancho + ox) * 4) as usize;
                v.extend_from_slice(pixeles.get(desde..desde + (lado * 4) as usize)?);
            }
            Some(ImagenRgba {
                ancho: lado,
                alto: lado,
                pixeles: v,
            })
        })
        .collect())
}

fn incrustada(lienzo: &Lienzo<'_>, id: u64) -> Option<Incrustada> {
    let img = lienzo.imagen(id)?;
    Some(Incrustada {
        mime: "image/png",
        bytes: pixpin_codec::codificar_png(img).ok()?,
    })
}

fn svg_de(hoja: &Hoja, fondo: Option<ColorRgba>, marcos: bool, lienzo: &Lienzo<'_>) -> String {
    exportar_svg::svg(hoja, OpcionesSvg { fondo, marcos }, &|id| {
        incrustada(lienzo, id)
    })
}

/// El fichero de la hoja `i` de `n`: el elegido si solo hay una, y con su
/// numero detras si hay varias (un PNG o un SVG no tienen paginas).
fn ruta_numerada(ruta: &Path, i: usize, n: usize) -> PathBuf {
    if n <= 1 {
        return ruta.to_path_buf();
    }
    let tronco = ruta
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = ruta
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    ruta.with_file_name(format!("{tronco}-{}.{ext}", i + 1))
}

/// Las hojas de la pagina web: con «todo», el lienzo entero **y** cada marco
/// detras, que es lo que saca el movil por omision (`HOJAS_AMBOS`).
fn hojas_web(que: Que, lienzo: &Lienzo<'_>) -> Vec<Hoja> {
    match que {
        Que::Todo => {
            let mut v = lienzo.hojas(Alcance::Todo);
            if lienzo.hay_marcos() {
                let marcos = ex::hojas(
                    lienzo.escena,
                    Alcance::Marcos,
                    &[],
                    lienzo.papel.map(|(_, w, h)| (w, h)),
                );
                // Sin marcos con algo dentro, «cada marco» devuelve el lienzo
                // entero otra vez: no se repite.
                if !(marcos.len() == 1 && v.first() == marcos.first()) {
                    v.extend(marcos);
                }
            }
            v
        }
        Que::Seleccion => lienzo.hojas(Alcance::Seleccion),
        Que::Marcos => lienzo.hojas(Alcance::Marcos),
    }
}

fn alcance_de(que: Que) -> Alcance {
    match que {
        Que::Todo => Alcance::Todo,
        Que::Seleccion => Alcance::Seleccion,
        Que::Marcos => Alcance::Marcos,
    }
}

/// **Escribe lo elegido en `ruta`.** Devuelve los ficheros escritos (varios
/// si un PNG o un SVG salen de varios marcos), o una lista vacia si no habia
/// nada que exportar.
pub(crate) fn exportar_a(ruta: &Path, e: Eleccion, lienzo: &Lienzo<'_>) -> Result<Vec<PathBuf>> {
    let fondo = (!e.transparente).then_some(lienzo.escena.fondo);
    let hojas = if e.formato == Formato::Html {
        hojas_web(e.que, lienzo)
    } else {
        lienzo.hojas(alcance_de(e.que))
    };
    if hojas.is_empty() {
        return Ok(Vec::new());
    }
    let mut escritos = Vec::new();
    match e.formato {
        Formato::Png => {
            for (i, h) in hojas.iter().enumerate() {
                let img = a_imagen(h, e.escala as f32, fondo, lienzo)?;
                let destino = ruta_numerada(ruta, i, hojas.len());
                std::fs::write(&destino, pixpin_codec::codificar_png(&img)?)
                    .with_context(|| format!("no se pudo escribir {}", destino.display()))?;
                escritos.push(destino);
            }
        }
        Formato::Svg => {
            for (i, h) in hojas.iter().enumerate() {
                let destino = ruta_numerada(ruta, i, hojas.len());
                std::fs::write(&destino, svg_de(h, fondo, false, lienzo))
                    .with_context(|| format!("no se pudo escribir {}", destino.display()))?;
                escritos.push(destino);
            }
        }
        Formato::Pdf => {
            // La letra con la que la pantalla pinta el texto, para que el PDF
            // parta las lineas donde ella y con su misma cara. Si no se puede
            // leer, el PDF sale igual, en Helvetica.
            let segoe = pixpin_pdf::letra::del_sistema("Segoe UI");
            if segoe.is_none() {
                tracing::warn!("sin Segoe UI para el PDF: el texto sale en Helvetica");
            }
            let bytes = pixpin_pdf::escribir::de_hojas_con_letra(
                &hojas,
                fondo,
                &|id| {
                    lienzo.imagen(id).map(|i| pixpin_pdf::escribir::Pixeles {
                        ancho: i.ancho,
                        alto: i.alto,
                        rgba: i.pixeles.clone(),
                    })
                },
                segoe.as_ref(),
            )
            .context("sin paginas")?;
            std::fs::write(ruta, bytes)
                .with_context(|| format!("no se pudo escribir {}", ruta.display()))?;
            escritos.push(ruta.to_path_buf());
        }
        Formato::Html => {
            let web: Vec<HojaWeb> = hojas
                .iter()
                .enumerate()
                .map(|(i, h)| HojaWeb {
                    nombre: h.nombre.clone(),
                    // Los marcos a la vista solo en la hoja del lienzo entero,
                    // que es la unica que los trae.
                    svg: svg_de(h, fondo, i == 0 && !h.marcos.is_empty(), lienzo),
                    fondo: ex::hex(lienzo.escena.fondo),
                })
                .collect();
            // El lienzo entero viaja dentro, para volver a editarlo.
            let json =
                pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::con_escena(
                    &pixpin_motor2d::excalidraw::Lienzo::vacio(),
                    lienzo.escena,
                ));
            let nombre = exportar_html::nombre_de_fichero(&lienzo.nombre);
            let pagina = exportar_html::paginas(
                &web,
                &lienzo.nombre,
                &nombre,
                exportar_html::Opciones::default(),
                Some(&json),
            )
            .context("sin hojas")?;
            std::fs::write(ruta, pagina)
                .with_context(|| format!("no se pudo escribir {}", ruta.display()))?;
            escritos.push(ruta.to_path_buf());
        }
    }
    Ok(escritos)
}

/// **Copia lo elegido (o todo) como imagen**, como el `Ctrl+Mayus+C` de
/// Excalidraw: PNG con el papel, a escala 1, y su SVG al lado.
pub(crate) fn copiar_png(lienzo: &Lienzo<'_>) -> Result<bool> {
    let alcance = if lienzo.seleccion.is_empty() {
        Alcance::Todo
    } else {
        Alcance::Seleccion
    };
    let Some(hoja) = lienzo.hojas(alcance).into_iter().next() else {
        return Ok(false);
    };
    let img = a_imagen(&hoja, 1.0, Some(lienzo.escena.fondo), lienzo)?;
    let png = pixpin_codec::codificar_png(&img)?;
    let svg = svg_de(&hoja, Some(lienzo.escena.fondo), false, lienzo);
    Ok(pixpin_shell::exportar::copiar_imagen(
        img.ancho,
        img.alto,
        &img.pixeles,
        &png,
        Some(&svg),
    ))
}

/// Las hojas que se imprimen y el tamano de su papel: lo que decide el
/// dialogo. Pura, para probarla sin impresora.
/// Cada hoja con su numero `(esta, de cuantas)` si va por marcos: el numero
/// es el de la hoja en el lienzo, no el de la impresion (imprimir solo la 3
/// saca una hoja con «3 / 5»).
fn hojas_a_imprimir(
    rango: &pixpin_shell::imprimir::Rango,
    lienzo: &Lienzo<'_>,
) -> Vec<(Option<(usize, usize)>, Hoja)> {
    use pixpin_shell::imprimir::Rango;
    match rango {
        Rango::Seleccion => lienzo
            .hojas(Alcance::Seleccion)
            .into_iter()
            .map(|h| (None, h))
            .collect(),
        otro => {
            let todas = lienzo.hojas(Alcance::Marcos);
            let de = todas.len();
            let numerar = lienzo.hay_marcos();
            todas
                .into_iter()
                .enumerate()
                .filter(|(i, _)| otro.incluye(*i as u32 + 1))
                .map(|(i, h)| (numerar.then_some((i + 1, de)), h))
                .collect()
        }
    }
}

/// **Imprime el lienzo por marcos**, como el movil: cada marco una pagina,
/// en su orden; sin marcos, todo en una. El papel, la impresora y que
/// paginas los elige el dialogo de Windows. Devuelve `false` si no habia
/// nada que imprimir.
///
/// Primero el dialogo moderno, **con la vista previa dentro**
/// (`imprimir.rs`); si ese no se abre (un Windows sin el, o que fallo ya una
/// vez en esta sesion), el clasico de siempre, sin vista previa pero que
/// imprime lo mismo.
pub(crate) fn imprimir(propietaria: HWND, lienzo: &Lienzo<'_>, textos: &Catalogo) -> Result<bool> {
    let paginas = lienzo.hojas(Alcance::Marcos);
    let Some(primera) = paginas.first() else {
        return Ok(false);
    };
    if !pixpin_render::imprimir_moderno::roto() {
        match imprimir::con_vista_previa(propietaria, lienzo, primera, textos) {
            Ok(()) => return Ok(true),
            Err(e) => tracing::warn!(?e, "sin dialogo moderno de imprimir: el clasico"),
        }
    }
    imprimir_clasico(propietaria, lienzo, primera, paginas.len())
}

/// El dialogo clasico (`PrintDlgEx`): el respaldo del moderno.
fn imprimir_clasico(
    propietaria: HWND,
    lienzo: &Lienzo<'_>,
    primera: &Hoja,
    n_paginas: usize,
) -> Result<bool> {
    let paginas_len = n_paginas;
    let Some(eleccion) = pixpin_shell::imprimir::pedir_impresion(
        propietaria,
        paginas_len as u32,
        !lienzo.seleccion.is_empty(),
        ex::apaisada(primera),
    ) else {
        return Ok(true);
    };
    let hojas = hojas_a_imprimir(&eleccion.rango, lienzo);
    if hojas.is_empty() {
        return Ok(false);
    }
    let (_d, motor) = motor_propio()?;
    let (numeros, hojas): (Vec<_>, Vec<_>) = hojas.into_iter().unzip();
    let bitmaps = subir(&motor, &hojas, lienzo);
    let tamanos = vec![eleccion.papel; hojas.len()];
    pixpin_render::imprimir::imprimir(
        &motor,
        &pixpin_render::imprimir::Trabajo {
            impresora: &eleccion.impresora,
            nombre: &lienzo.nombre,
            devmode: Some(&eleccion.devmode),
            paginas: &tamanos,
            fichero: None,
        },
        &mut |i, p| {
            // El mismo pintado que la vista previa del dialogo moderno.
            imprimir::pintar_en_papel(
                p,
                &hojas[i],
                pixpin_render::imprimir_moderno::Pagina {
                    papel: eleccion.papel,
                    escala: 1.0,
                },
                &bitmaps,
                numeros[i],
            )
        },
    )
    .context("la impresora no acepto el trabajo")?;
    Ok(true)
}

/// **Abre la caja de exportar** y escribe lo elegido.
pub(crate) fn exportar(propietaria: HWND, lienzo: &Lienzo<'_>, textos: &Catalogo) -> Result<bool> {
    let t = |k: &str| textos.t(k);
    let (titulo, que, todo, seleccion, marcos, escala, transparente) = (
        t("exportar-titulo"),
        t("exportar-que"),
        t("exportar-todo"),
        t("exportar-seleccion"),
        t("exportar-marcos"),
        t("exportar-escala"),
        t("exportar-transparente"),
    );
    let (png, svg, pdf, html) = (
        t("exportar-tipo-png"),
        t("exportar-tipo-svg"),
        t("exportar-tipo-pdf"),
        t("exportar-tipo-html"),
    );
    let rotulos = Rotulos {
        titulo: &titulo,
        que: &que,
        todo: &todo,
        seleccion: &seleccion,
        marcos: &marcos,
        escala: &escala,
        transparente: &transparente,
        png: &png,
        svg: &svg,
        pdf: &pdf,
        html: &html,
    };
    let mut inicial = eleccion_inicial();
    // Con algo elegido, lo natural es exportar eso: es lo que hace Excalidraw.
    if !lienzo.seleccion.is_empty() {
        inicial.que = Que::Seleccion;
    }
    let Some((ruta, e)) = pixpin_shell::exportar::pedir_exportacion(
        propietaria,
        &exportar_html::nombre_de_fichero(&lienzo.nombre),
        &rotulos,
        inicial,
        !lienzo.seleccion.is_empty(),
        lienzo.hay_marcos(),
    ) else {
        return Ok(true);
    };
    if let Ok(mut g) = ULTIMA.lock() {
        *g = Some(e);
    }
    let escritos = exportar_a(&ruta, e, lienzo)?;
    tracing::info!(?escritos, formato = ?e.formato, que = ?e.que, "lienzo exportado");
    // Lo exportado, en pantalla para arrastrarlo o mandarlo: sin esto habia
    // que ir a buscarlo a la carpeta que se acababa de elegir.
    if !escritos.is_empty() {
        crate::salida::mostrar(escritos.clone(), textos.t("salida-titulo-exportado"));
    }
    Ok(!escritos.is_empty())
}

/// **Atiende una peticion** y avisa con un cuadro si no habia nada o si
/// fallo: el editor no tiene donde poner un aviso, y quedarse callado ante
/// un «no se pudo» es lo peor que puede pasar al exportar.
pub(crate) fn atender(p: Peticion, propietaria: HWND, lienzo: &Lienzo<'_>, textos: &Catalogo) {
    // Lo pixelado sale pixelado en todo lo que sale de aqui (`tapado`): los
    // mosaicos pasan a ser imagenes ya tapadas de lo que tienen debajo.
    if let Some(t) = tapado::tapar(lienzo) {
        let fotos = |id: u64| t.imagenes.get(&id).or_else(|| (lienzo.fotos)(id));
        let con_tapados = Lienzo {
            escena: &t.escena,
            seleccion: lienzo.seleccion,
            papel: lienzo.papel,
            fotos: &fotos,
            nombre: lienzo.nombre.clone(),
        };
        return atender_tal_cual(p, propietaria, &con_tapados, textos);
    }
    atender_tal_cual(p, propietaria, lienzo, textos);
}

/// [`atender`] con los mosaicos ya tapados.
fn atender_tal_cual(p: Peticion, propietaria: HWND, lienzo: &Lienzo<'_>, textos: &Catalogo) {
    let (vacio, fallo) = match p {
        Peticion::Imprimir => ("imprimir-vacio", "imprimir-fallo"),
        _ => ("exportar-vacio", "exportar-fallo"),
    };
    let r = match p {
        Peticion::Exportar => exportar(propietaria, lienzo, textos),
        Peticion::CopiarPng => copiar_png(lienzo),
        Peticion::Imprimir => imprimir(propietaria, lienzo, textos),
        Peticion::Compartir => compartir(lienzo),
    };
    match r {
        Ok(true) => {}
        Ok(false) => pixpin_shell::exportar::informar(
            propietaria,
            &textos.t("exportar-titulo"),
            &textos.t(vacio),
        ),
        Err(e) => {
            tracing::error!(?e, ?p, "exportar o imprimir el lienzo");
            pixpin_shell::exportar::informar(
                propietaria,
                &textos.t("exportar-titulo"),
                &format!("{}\n\n{e:#}", textos.t(fallo)),
            );
        }
    }
}

/// **Abre la hoja de compartir** con una copia del lienzo: la hoja vive en
/// su hilo y el editor sigue dibujando mientras tanto. Se copian la escena y
/// los pixeles de las fotos que usa (no todas las del editor) y del papel.
/// `false` si no hay nada que compartir.
fn compartir(lienzo: &Lienzo<'_>) -> Result<bool> {
    if lienzo.escena.visibles().next().is_none() && lienzo.papel.is_none() {
        return Ok(false);
    }
    let mut fotos = HashMap::new();
    for e in lienzo.escena.visibles() {
        if let pixpin_motor2d::Figura::Imagen { id_objeto } = e.figura
            && let Some(img) = (lienzo.fotos)(id_objeto)
        {
            fotos.entry(id_objeto).or_insert_with(|| img.clone());
        }
    }
    let (idioma, ubicacion) = idioma_y_ubicacion();
    crate::compartir::ventana::abrir(
        idioma,
        ubicacion,
        crate::compartir::Cosa::Lienzo(crate::compartir::LienzoSuelto {
            escena: lienzo.escena.clone(),
            papel: lienzo.papel.map(|(i, w, h)| (i.clone(), w, h)),
            fotos,
            nombre: lienzo.nombre.clone(),
        }),
    );
    Ok(true)
}

/// El idioma y donde viven los datos, leidos como `textos`: el editor no
/// recibe los de la aplicacion.
fn idioma_y_ubicacion() -> (pixpin_store::Idioma, pixpin_store::Ubicacion) {
    let dir_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default();
    let ubicacion = pixpin_store::resolver(&dir_exe, &appdata);
    (crate::compartir::idioma_de(&ubicacion), ubicacion)
}

/// Desde el editor: los textos se leen aqui.
pub(crate) fn atender_en_el_editor(p: Peticion, propietaria: HWND, lienzo: &Lienzo<'_>) {
    atender(p, propietaria, lienzo, textos());
}

/// **El menu del clic derecho del lienzo**, en `(x, y)` de pantalla.
pub(crate) fn menu(propietaria: HWND, x: i32, y: i32) -> Option<Peticion> {
    let t = textos();
    let entradas = [
        // Compartir, lo primero: es lo de todos los sitios de la aplicacion.
        EntradaMenu {
            id: 5,
            texto: t.t("compartir-menu"),
        },
        EntradaMenu {
            id: 1,
            texto: t.t("exportar-menu"),
        },
        EntradaMenu {
            id: 2,
            texto: t.t("exportar-copiar-png"),
        },
        EntradaMenu {
            id: 0,
            texto: String::new(),
        },
        EntradaMenu {
            id: 3,
            texto: t.t("imprimir-menu"),
        },
        EntradaMenu {
            id: 0,
            texto: String::new(),
        },
        // El «Fondo del lienzo» del menu de Excalidraw (`interfaz.md` §1.3).
        // No es una `Peticion`: no exporta nada, solo ensena el selector del
        // papel en el panel. Se atiende aqui mismo y el editor recibe `None`
        // y repinta: asi el bucle del editor, que reescribe otro trabajo,
        // no necesita una rama mas.
        EntradaMenu {
            id: 4,
            texto: t.t("fondo-lienzo-menu"),
        },
        // «Referencias → Imagen de referencia» de los ajustes del movil: la
        // foto que se copia, flotando encima (`referencia.rs`). La misma
        // entrada la quita si ya esta puesta.
        EntradaMenu {
            id: 6,
            texto: t.t("lienzo-referencia-menu"),
        },
        EntradaMenu {
            id: 0,
            texto: String::new(),
        },
        // «Pegar»: lo mismo que Ctrl+V (lo atiende el editor, ver
        // `tomar_pegar`), donde se abrio el menu.
        EntradaMenu {
            id: 7,
            texto: t.t("lienzo-pegar-menu"),
        },
    ];
    match pixpin_shell::exportar::menu_emergente(propietaria, x, y, &entradas)? {
        1 => Some(Peticion::Exportar),
        2 => Some(Peticion::CopiarPng),
        3 => Some(Peticion::Imprimir),
        5 => Some(Peticion::Compartir),
        4 => {
            crate::panel_dibujo::pedir_papel();
            None
        }
        6 => {
            super::referencia::pedir();
            None
        }
        7 => {
            PEGAR.store(true, std::sync::atomic::Ordering::SeqCst);
            None
        }
        _ => None,
    }
}

/// «Pegar» del menu, a la espera de que el editor lo atienda como un Ctrl+V.
static PEGAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Si se eligio «Pegar» en el menu (y se olvida: una vez por eleccion).
pub(crate) fn tomar_pegar() -> bool {
    PEGAR.swap(false, std::sync::atomic::Ordering::SeqCst)
}

/// El ancho al que se dibuja la pagina de un PDF debajo de una hoja: el mismo
/// que usa el editor (`fondo_lienzo::ANCHO_PAPEL_PDF`, el del movil), o lo anotado no
/// caeria en su sitio.
const ANCHO_PAGINA: u32 = crate::fondo_lienzo::ANCHO_PAPEL_PDF as u32;

/// Una hoja del proyecto cargada para exportarla sin abrir el editor.
pub(crate) struct HojaCargada {
    pub escena: Escena,
    pub fotos: HashMap<u64, ImagenRgba>,
    pub papel: Option<ImagenRgba>,
}

/// **Carga la hoja de un mensaje del chat**: su `.excalidraw`, sus fotos y,
/// si se dibujo sobre una pagina del PDF, esa pagina de papel. Lo mismo que
/// hace el chat al abrirla en el editor.
pub(crate) fn cargar_hoja(
    raiz: &Path,
    proyecto: &str,
    m: &pixpin_proyecto::cuaderno::Mensaje,
) -> Result<HojaCargada> {
    let id = m
        .referencia
        .as_deref()
        .filter(|r| !r.is_empty())
        .context("el mensaje no tiene hoja")?;
    let ruta = pixpin_proyecto::almacen::lienzo(raiz, proyecto, id);
    let texto = std::fs::read_to_string(&ruta)
        .with_context(|| format!("no se pudo leer {}", ruta.display()))?;
    let lienzo = pixpin_motor2d::excalidraw::leer(&texto).context("dibujo que no se entiende")?;
    let escena = pixpin_motor2d::excalidraw::a_escena(&lienzo);
    let mut fotos = HashMap::new();
    for (idf, rel) in pixpin_motor2d::excalidraw::ficheros(&lienzo) {
        let Some(r) = pixpin_proyecto::vista::ruta_real(raiz, proyecto, &rel) else {
            continue;
        };
        match pixpin_codec::cargar(&r) {
            Ok(img) => {
                fotos.insert(idf, img);
            }
            Err(e) => tracing::warn!(?e, ruta = %r.display(), "foto de la hoja ilegible"),
        }
    }
    let papel = m.pagina.and_then(|pagina| {
        let pdf = crate::pdf_en_chat::documento_de(raiz, proyecto)?;
        pixpin_pdf::Documento::abrir(&pdf)
            .and_then(|d| d.renderizar(pagina, ANCHO_PAGINA))
            .map_err(|e| tracing::warn!(?e, pagina, "no se pudo dibujar la pagina"))
            .ok()
    });
    Ok(HojaCargada {
        escena,
        fotos,
        papel,
    })
}

/// **Desde el menu de la burbuja del chat**: exportar o imprimir la hoja de
/// ese mensaje. Devuelve el aviso que ensenar si algo no fue bien.
pub(crate) fn desde_el_chat(
    p: Peticion,
    propietaria: HWND,
    textos: &Catalogo,
    raiz: &Path,
    proyecto: &str,
    m: &pixpin_proyecto::cuaderno::Mensaje,
    nombre: &str,
) -> Option<String> {
    let hoja = match cargar_hoja(raiz, proyecto, m) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(?e, "no se pudo cargar la hoja para exportar");
            return Some(textos.t(match p {
                Peticion::Imprimir => "imprimir-fallo",
                _ => "exportar-fallo",
            }));
        }
    };
    let fotos = |id: u64| hoja.fotos.get(&id);
    let lienzo = Lienzo {
        escena: &hoja.escena,
        seleccion: &[],
        papel: hoja
            .papel
            .as_ref()
            .map(|i| (i, i.ancho as f32, i.alto as f32)),
        fotos: &fotos,
        nombre: nombre.to_string(),
    };
    atender(p, propietaria, &lienzo, textos);
    None
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::{Elemento, Figura};

    /// Las miniaturas de las figuras de serie con la GPU de verdad: cada una
    /// sale de su tamano y con algo pintado (no el cuadro blanco). Con
    /// `PIXPIN_MINIATURAS_PNG=<fichero>` deja la hoja entera para mirarla.
    #[test]
    #[ignore = "necesita GPU y sesion de escritorio"]
    fn las_miniaturas_de_las_figuras_de_serie_salen_pintadas() {
        let figuras = pixpin_motor2d::figuras_de_serie::catalogo();
        let lado = 120;
        let v = miniaturas(&figuras, lado).expect("miniaturas");
        assert_eq!(v.len(), figuras.len());
        for (f, m) in figuras.iter().zip(&v) {
            let m = m
                .as_ref()
                .unwrap_or_else(|| panic!("sin miniatura: {}", f.nombre));
            assert_eq!((m.ancho, m.alto), (lado, lado));
            let pintados = m
                .pixeles
                .chunks_exact(4)
                .filter(|p| p[0] < 235 || p[1] < 235 || p[2] < 235)
                .count();
            assert!(pintados > 30, "{} casi en blanco ({pintados})", f.nombre);
        }
        if let Some(ruta) = std::env::var_os("PIXPIN_MINIATURAS_PNG") {
            let columnas = 10u32;
            let filas = (v.len() as u32).div_ceil(columnas);
            let (w, h) = (columnas * lado, filas * lado);
            let mut todo = vec![40u8; (w * h * 4) as usize];
            for (i, m) in v.iter().flatten().enumerate() {
                let (ox, oy) = ((i as u32 % columnas) * lado, (i as u32 / columnas) * lado);
                for y in 0..lado {
                    let d = (((oy + y) * w + ox) * 4) as usize;
                    let s = (y * lado * 4) as usize;
                    todo[d..d + (lado * 4) as usize]
                        .copy_from_slice(&m.pixeles[s..s + (lado * 4) as usize]);
                }
            }
            let png = pixpin_codec::codificar_png(&ImagenRgba {
                ancho: w,
                alto: h,
                pixeles: todo,
            })
            .unwrap();
            std::fs::write(ruta, png).unwrap();
        }
    }

    #[test]
    fn los_atajos_son_los_de_excalidraw_y_solo_con_sus_modificadores() {
        assert_eq!(atajo(VK_E, true, true, false), Some(Peticion::Exportar));
        assert_eq!(atajo(VK_C, true, true, false), Some(Peticion::CopiarPng));
        assert_eq!(atajo(VK_P, true, false, false), Some(Peticion::Imprimir));
        // Casos negativos: Ctrl+C es copiar elementos y Ctrl+Mayus+P no es
        // imprimir; con Alt tampoco (Ctrl+Alt+C es tomar el estilo).
        assert_eq!(atajo(VK_C, true, false, false), None);
        assert_eq!(atajo(VK_P, true, true, false), None);
        assert_eq!(atajo(VK_C, true, true, true), None);
        assert_eq!(atajo(VK_E, false, true, false), None);
        // Compartir es Ctrl+Mayus+S; Ctrl+S a secas no (es guardar).
        assert_eq!(atajo(VK_S, true, true, false), Some(Peticion::Compartir));
        assert_eq!(atajo(VK_S, true, false, false), None);
    }

    #[test]
    fn un_png_o_un_svg_de_varias_hojas_sale_numerado() {
        let r = Path::new("C:\\x\\casa.png");
        assert_eq!(ruta_numerada(r, 0, 1), PathBuf::from("C:\\x\\casa.png"));
        assert_eq!(ruta_numerada(r, 1, 3), PathBuf::from("C:\\x\\casa-2.png"));
    }

    fn escena_con_marcos() -> Escena {
        let mut escena = Escena::nueva();
        for (y, nombre) in [(0.0, "Planta"), (400.0, "Alzado")] {
            escena.anadir(Elemento {
                figura: Figura::Marco {
                    nombre: nombre.into(),
                },
                x: 0.0,
                y,
                ancho: 300.0,
                alto: 200.0,
                ..Elemento::default()
            });
            escena.anadir(Elemento {
                figura: Figura::Rectangulo,
                x: 20.0,
                y: y + 20.0,
                ancho: 100.0,
                alto: 80.0,
                ..Elemento::default()
            });
        }
        escena
    }

    #[test]
    fn la_web_de_todo_lleva_el_lienzo_entero_y_detras_cada_marco() {
        let escena = escena_con_marcos();
        let lienzo = Lienzo {
            escena: &escena,
            seleccion: &[],
            papel: None,
            fotos: &|_| None,
            nombre: "casa".into(),
        };
        let v = hojas_web(Que::Todo, &lienzo);
        let nombres: Vec<&str> = v.iter().map(|h| h.nombre.as_str()).collect();
        assert_eq!(nombres, ["", "Planta", "Alzado"]);
        // Caso negativo: sin marcos, una sola hoja y no el lienzo repetido.
        let mut solo = Escena::nueva();
        solo.anadir(Elemento {
            figura: Figura::Rectangulo,
            ancho: 10.0,
            alto: 10.0,
            ..Elemento::default()
        });
        let l2 = Lienzo {
            escena: &solo,
            ..lienzo
        };
        assert_eq!(hojas_web(Que::Todo, &l2).len(), 1);
    }

    #[test]
    fn imprimir_elige_las_paginas_del_dialogo_por_su_numero() {
        use pixpin_shell::imprimir::Rango;
        let escena = escena_con_marcos();
        let lienzo = Lienzo {
            escena: &escena,
            seleccion: &[],
            papel: None,
            fotos: &|_| None,
            nombre: "casa".into(),
        };
        assert_eq!(hojas_a_imprimir(&Rango::Todas, &lienzo).len(), 2);
        let segunda = hojas_a_imprimir(&Rango::Paginas(vec![(2, 2)]), &lienzo);
        assert_eq!(segunda.len(), 1);
        assert_eq!(segunda[0].1.nombre, "Alzado");
        // Lleva el numero que tiene en el lienzo, no «1 / 1».
        assert_eq!(segunda[0].0, Some((2, 2)));
        // Caso negativo: una pagina que no existe no imprime nada.
        assert!(hojas_a_imprimir(&Rango::Paginas(vec![(9, 9)]), &lienzo).is_empty());
    }

    /// **El papel elegido sale al exportar**, en los cuatro formatos, y solo
    /// «fondo transparente» lo quita. Deja `papel-crema.png` y
    /// `papel-transparente.png` en la carpeta de las muestras para mirarlos.
    #[test]
    fn el_papel_del_lienzo_sale_al_exportar_salvo_con_fondo_transparente() {
        let dir = std::env::var_os("PIXPIN_MUESTRAS_EXPORTAR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!("pixpin-papel-{}", std::process::id()))
            });
        std::fs::create_dir_all(&dir).expect("carpeta");
        let mut escena = escena_con_marcos();
        // «Crema» del movil, `#fdf6e3`.
        let crema = ColorRgba::opaco(253.0 / 255.0, 246.0 / 255.0, 227.0 / 255.0);
        assert!(escena.poner_fondo(crema));
        let lienzo = Lienzo {
            escena: &escena,
            seleccion: &[],
            papel: None,
            fotos: &|_| None,
            nombre: "papel".into(),
        };
        let eleccion = |formato, transparente| Eleccion {
            formato,
            que: Que::Todo,
            escala: 1,
            transparente,
        };
        let esquina = |ruta: &Path| {
            let img = pixpin_codec::cargar(ruta).expect("png legible");
            img.pixeles[..4].to_vec()
        };

        let png = exportar_a(
            &dir.join("papel-crema.png"),
            eleccion(Formato::Png, false),
            &lienzo,
        )
        .unwrap();
        assert_eq!(
            esquina(&png[0]),
            [253, 246, 227, 255],
            "el papel del lienzo, no blanco"
        );
        let svg = exportar_a(
            &dir.join("papel-crema.svg"),
            eleccion(Formato::Svg, false),
            &lienzo,
        )
        .unwrap();
        let svg = std::fs::read_to_string(&svg[0]).unwrap();
        assert!(svg.contains("#fdf6e3"), "el rectangulo del papel en el SVG");
        let web = exportar_a(
            &dir.join("papel-crema.html"),
            eleccion(Formato::Html, false),
            &lienzo,
        )
        .unwrap();
        let web = std::fs::read_to_string(&web[0]).unwrap();
        assert!(
            web.contains("data-fondo=\"#fdf6e3\""),
            "la hoja de la web con su papel"
        );
        let pdf = exportar_a(
            &dir.join("papel-crema.pdf"),
            eleccion(Formato::Pdf, false),
            &lienzo,
        )
        .unwrap();
        let pagina = pixpin_pdf::Documento::abrir(&pdf[0])
            .and_then(|d| d.renderizar(0, 300))
            .expect("el pdf se dibuja");
        // En el centro de la pagina y no en la esquina: el PDF es un A4 con
        // su margen blanco, y el papel del lienzo es la hoja encajada en el.
        // El centro cae entre los dos marcos, donde no hay tinta.
        let i = ((pagina.alto / 2 * pagina.ancho + pagina.ancho / 2) * 4) as usize;
        let p = &pagina.pixeles[i..i + 4];
        assert!(
            p[0].abs_diff(253) <= 2 && p[1].abs_diff(246) <= 2 && p[2].abs_diff(227) <= 2,
            "el papel en la pagina del PDF: {p:?}"
        );

        // Caso negativo: con «fondo transparente» no hay papel que valga.
        let png = exportar_a(
            &dir.join("papel-transparente.png"),
            eleccion(Formato::Png, true),
            &lienzo,
        )
        .unwrap();
        assert_eq!(esquina(&png[0])[3], 0, "la esquina es transparente");
        let svg = exportar_a(
            &dir.join("papel-transparente.svg"),
            eleccion(Formato::Svg, true),
            &lienzo,
        )
        .unwrap();
        assert!(
            !std::fs::read_to_string(&svg[0])
                .unwrap()
                .contains("#fdf6e3")
        );
    }

    /// **Las muestras**: el mismo lienzo en los cuatro formatos, para mirarlo.
    /// Solo con `PIXPIN_MUESTRAS_EXPORTAR` puesta (la carpeta de salida); sin
    /// ella no escribe nada fuera de la carpeta temporal.
    #[test]
    fn el_mismo_lienzo_sale_en_png_svg_pdf_y_web() {
        let dir = std::env::var_os("PIXPIN_MUESTRAS_EXPORTAR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!("pixpin-exportar-{}", std::process::id()))
            });
        std::fs::create_dir_all(&dir).expect("carpeta");
        let mut escena = escena_con_marcos();
        escena.anadir(Elemento {
            figura: Figura::Texto {
                texto: "Planta baja\nescala 1:50".into(),
                tam: 20.0,
                familia: "Segoe UI".into(),
            },
            x: 140.0,
            y: 30.0,
            ancho: 150.0,
            alto: 60.0,
            ..Elemento::default()
        });
        escena.anadir(Elemento {
            figura: Figura::Elipse,
            x: 140.0,
            y: 420.0,
            ancho: 120.0,
            alto: 90.0,
            trazo: ColorRgba::opaco(0.9, 0.2, 0.2),
            ..Elemento::default()
        });
        // Una tinta porosa (rayado) y una de tiza, gordas, para ver su grano.
        for (y, material) in [
            (125.0, pixpin_motor2d::tinta::MaterialTinta::Rayado),
            (160.0, pixpin_motor2d::tinta::MaterialTinta::Tiza),
        ] {
            escena.anadir(Elemento {
                figura: Figura::Lapiz {
                    puntos: (0..26)
                        .map(|i| {
                            pixpin_motor2d::vector::Punto2::nuevo(
                                50.0 + i as f32 * 8.0,
                                y + (i % 4) as f32,
                            )
                        })
                        .collect(),
                    presiones: Vec::new(),
                    opciones: Some(Default::default()),
                },
                grosor: 10.0,
                trazo: ColorRgba::opaco(0.15, 0.35, 0.85),
                material,
                ..Elemento::default()
            });
        }
        // Una foto de cuatro colores recortada a su cuarto verde (arriba a la
        // derecha): sale verde entera, sin nada del rojo.
        let mut cuadrantes = Vec::new();
        for y in 0..80u32 {
            for x in 0..80u32 {
                cuadrantes.extend_from_slice(match (x < 40, y < 40) {
                    (true, true) => &[230u8, 30, 30, 255],
                    (false, true) => &[30, 200, 60, 255],
                    (true, false) => &[40, 60, 220, 255],
                    (false, false) => &[240, 220, 40, 255],
                });
            }
        }
        let foto = ImagenRgba {
            ancho: 80,
            alto: 80,
            pixeles: cuadrantes,
        };
        let mut con_foto = Elemento {
            figura: Figura::Imagen { id_objeto: 42 },
            x: 20.0,
            y: 510.0,
            ancho: 80.0,
            alto: 80.0,
            ..Elemento::default()
        };
        con_foto.extras.recorte = Some(pixpin_motor2d::RecorteImagen {
            x: 40.0,
            y: 0.0,
            ancho: 40.0,
            alto: 40.0,
            ancho_natural: 80.0,
            alto_natural: 80.0,
        });
        escena.anadir(con_foto);
        let fotos = |id: u64| (id == 42).then_some(&foto);
        let lienzo = Lienzo {
            escena: &escena,
            seleccion: &[],
            papel: None,
            fotos: &fotos,
            nombre: "Casa de muestra".into(),
        };
        let casos = [
            (Formato::Png, Que::Todo, "muestra.png"),
            (Formato::Svg, Que::Todo, "muestra.svg"),
            (Formato::Pdf, Que::Marcos, "muestra.pdf"),
            (Formato::Html, Que::Todo, "muestra.html"),
        ];
        for (formato, que, nombre) in casos {
            let e = Eleccion {
                formato,
                que,
                escala: 2,
                transparente: false,
            };
            let escritos = exportar_a(&dir.join(nombre), e, &lienzo).expect("exporta");
            assert_eq!(escritos.len(), 1, "{nombre}");
            let bytes = std::fs::read(&escritos[0]).expect("leer");
            match formato {
                Formato::Png => {
                    assert!(bytes.starts_with(b"\x89PNG"));
                    let img = pixpin_codec::cargar(&escritos[0]).expect("png legible");
                    // A escala 2, el doble de la hoja entera.
                    let h = &lienzo.hojas(Alcance::Todo)[0];
                    assert_eq!(img.ancho, (h.ancho() * 2.0).round() as u32);
                    // El papel es blanco y opaco en la esquina.
                    assert_eq!(&img.pixeles[..4], &[255, 255, 255, 255]);
                    // Y hay tinta: algun pixel oscuro.
                    assert!(
                        img.pixeles
                            .chunks_exact(4)
                            .any(|p| p[0] < 80 && p[3] == 255)
                    );
                    // La foto sale recortada: en su centro, verde (el cuarto
                    // elegido), no el cruce de los cuatro colores.
                    let (x0, y0) = (h.caja.0, h.caja.1);
                    let (cx, cy) = (((60.0 - x0) * 2.0) as u32, ((550.0 - y0) * 2.0) as u32);
                    let i = ((cy * img.ancho + cx) * 4) as usize;
                    let c = &img.pixeles[i..i + 4];
                    assert!(
                        c[1] > 150 && c[0] < 90,
                        "verde en el centro de la foto: {c:?}"
                    );
                    // Y el grano cambia lo pintado: sin el, el rayado sale
                    // liso y la imagen es otra.
                    let mut lisa = h.clone();
                    lisa.granos.clear();
                    let sin = a_imagen(&lisa, 2.0, Some(escena.fondo), &lienzo).expect("sin grano");
                    let con = a_imagen(h, 2.0, Some(escena.fondo), &lienzo).expect("con grano");
                    let distintos = con
                        .pixeles
                        .chunks_exact(4)
                        .zip(sin.pixeles.chunks_exact(4))
                        .filter(|(a, b)| a != b)
                        .count();
                    assert!(distintos > 500, "el grano se pinta en el PNG: {distintos}");
                }
                Formato::Svg => {
                    let s = String::from_utf8(bytes).expect("utf8");
                    assert!(s.starts_with("<?xml") && s.trim_end().ends_with("</svg>"));
                    assert!(s.contains("Planta baja</tspan>"));
                    assert_eq!(s.matches("<pattern ").count(), 2, "una tela por material");
                    assert!(s.contains("viewBox=\"40 0 40 40\""), "la foto recortada");
                }
                Formato::Pdf => {
                    let doc = pixpin_pdf::Documento::abrir(&escritos[0]).expect("el pdf abre");
                    assert_eq!(doc.paginas(), 2, "una pagina por marco");
                    let s = String::from_utf8_lossy(&bytes);
                    assert!(s.contains("/PatternType 1"), "el grano va dentro");
                    assert!(s.contains("+SegoeUI"), "la letra de la pantalla va dentro");
                }
                Formato::Html => {
                    let s = String::from_utf8(bytes).expect("utf8");
                    assert_eq!(s.matches("class=\"hoja\"").count(), 3);
                    assert!(s.contains("class=\"marcos\""));
                    assert!(s.contains("class=\"excalidraw\""));
                    assert!(s.contains("url(#grano-rayado-"));
                }
            }
        }
        // Caso negativo: lo elegido sin elegir nada no escribe ningun fichero.
        let nada = exportar_a(
            &dir.join("nada.png"),
            Eleccion {
                formato: Formato::Png,
                que: Que::Seleccion,
                escala: 1,
                transparente: true,
            },
            &lienzo,
        )
        .expect("no falla");
        assert!(nada.is_empty());
        assert!(!dir.join("nada.png").exists());
    }
}
