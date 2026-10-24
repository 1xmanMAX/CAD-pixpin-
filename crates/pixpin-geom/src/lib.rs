//! pixpin-geom — geometria pura, sin una sola llamada al sistema operativo.
//!
//! Aqui vive la aritmetica donde de verdad se cometen errores: el escritorio
//! virtual con DPI mixto, la normalizacion del arrastre, la resolucion del
//! ajuste automatico. Al no depender de Windows ni de la GPU, se prueba en
//! milisegundos y con disposiciones de monitores inventadas que seria
//! carisimo reproducir en hardware.
#![forbid(unsafe_code)]

pub mod ajuste;
pub mod monitores;
pub mod parrafos;
pub mod pin_geometria;
pub mod punto;
pub mod rect;
pub mod seleccion;
pub mod seleccion_texto;

pub use ajuste::{Candidato, resolver_ajuste};
pub use monitores::{DisposicionMonitores, Monitor};
pub use parrafos::{LineaTexto, a_texto, agrupar};
pub use pin_geometria::{
    Esquina, esquina_en, iman_de_bordes, recolocar_en_area, redimension_libre,
    redimension_proporcional,
};
pub use punto::Punto;
pub use rect::Rect;
pub use seleccion::{Seleccion, Tirador};
