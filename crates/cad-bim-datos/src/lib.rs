//! **cad-bim-datos** — los datos de un modelo BIM, de Revit o de IFC, en un
//! mismo modelo ([`ModeloBim`]): el fichero y el proyecto, la estructura
//! espacial y los niveles, cada elemento con su categoria, familia, tipo,
//! nivel, materiales, caja y todas sus propiedades por conjunto; los
//! materiales, espacios y tipos. Y sus exportaciones (CSV, JSON, Excel; de un
//! Revit, IFC4, GLB y su planta para el dibujo).
//!
//! Rust puro: Revit con `rvt-rs` (Apache-2.0), IFC con `ifc-lite` (MPL-2.0).
//! Se prueba en cualquier sistema; el explorador esta en `apps/cadpixpin`.

pub mod exportar;
pub mod ifc;
pub mod metrado;
pub mod modelo;
pub mod planta;
pub mod revit;

use std::path::Path;

pub use modelo::*;

/// **Como se lee.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opciones {
    /// Medir la caja de cada elemento de un IFC (pide su geometria: tarda mas).
    pub cajas: bool,
}

impl Default for Opciones {
    fn default() -> Self {
        Opciones { cajas: true }
    }
}

/// Si es un fichero que se lee aqui.
pub fn se_lee(ruta: &Path) -> bool {
    let ext = extension(ruta);
    ext == "ifc" || revit::es_revit(&ext)
}

fn extension(ruta: &Path) -> String {
    ruta.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// **Lee un Revit o un IFC.** `avance` recibe en que va (para mostrarlo).
/// Un fichero roto da un error, nunca un panico.
pub fn leer(
    ruta: &Path,
    opciones: Opciones,
    avance: &mut dyn FnMut(&str),
) -> Result<ModeloBim, String> {
    let ext = extension(ruta);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if revit::es_revit(&ext) {
            revit::leer(ruta, avance)
        } else {
            let bytes = std::fs::read(ruta).map_err(|e| format!("no se pudo leer: {e}"))?;
            // Un Revit con otra extension.
            if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
                return revit::leer(ruta, avance);
            }
            ifc::leer(&bytes, opciones.cajas, avance)
        }
    }))
    .unwrap_or_else(|_| Err("el fichero esta dañado (el lector se detuvo)".into()));
    let mut m = r?;
    m.archivo.ruta = ruta.display().to_string();
    m.archivo.nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if m.proyecto.nombre.is_none() {
        m.proyecto.nombre = ruta.file_stem().map(|n| n.to_string_lossy().into_owned());
    }
    Ok(m)
}

/// **Lee desde unos bytes de IFC** (sin fichero).
pub fn leer_ifc(bytes: &[u8], opciones: Opciones) -> Result<ModeloBim, String> {
    std::panic::catch_unwind(|| ifc::leer(bytes, opciones.cajas, &mut |_| {}))
        .unwrap_or_else(|_| Err("el IFC esta dañado (el lector se detuvo)".into()))
}

/// **Una imagen en pixeles RGBA** (8 bits por canal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixeles {
    pub ancho: u32,
    pub alto: u32,
    pub rgba: Vec<u8>,
}

/// **Decodifica un PNG** (la miniatura de Revit) a RGBA. `None` si no es un
/// PNG que se pueda leer o es demasiado grande (mas de 4096 de lado).
pub fn png_a_rgba(png: &[u8]) -> Option<Pixeles> {
    let mut d = png::Decoder::new(std::io::Cursor::new(png));
    d.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = d.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return None;
    }
    let n = (w * h) as usize;
    let datos = &buf[..info.line_size * h as usize];
    let mut rgba = Vec::with_capacity(n * 4);
    for fila in datos.chunks_exact(info.line_size) {
        match info.color_type {
            png::ColorType::Rgba => rgba.extend_from_slice(&fila[..w as usize * 4]),
            png::ColorType::Rgb => {
                for p in fila[..w as usize * 3].chunks_exact(3) {
                    rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for p in fila[..w as usize * 2].chunks_exact(2) {
                    rgba.extend_from_slice(&[p[0], p[0], p[0], p[1]]);
                }
            }
            png::ColorType::Grayscale => {
                for &g in &fila[..w as usize] {
                    rgba.extend_from_slice(&[g, g, g, 255]);
                }
            }
            png::ColorType::Indexed => return None,
        }
    }
    (rgba.len() == n * 4).then_some(Pixeles {
        ancho: w,
        alto: h,
        rgba,
    })
}
