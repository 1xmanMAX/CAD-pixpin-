//! **cad-nucleo** — editar planos DWG y DXF: el dibujo con deshacer, los
//! enganches, la seleccion, las ordenes al estilo de AutoCAD y las
//! herramientas del oficio (muros, ejes, puertas, cotas, laminas…).
//!
//! Rust puro sobre `opencadcodec` (OpenCADStudio): se prueba en cualquier
//! sistema; la ventana de Windows esta en `apps/cadpixpin`.

pub mod biblioteca;
pub mod bloque;
pub mod contorno;
pub mod cotas_bloque;
pub mod dibujo;
pub mod enganche;
pub mod entrada;
pub mod especialidad;
pub mod formas;
pub mod formatos;
pub mod geo;
pub mod herramienta;
pub mod herramientas;
pub mod metrados;
pub mod operaciones;
pub mod ordenes;
pub mod patrones;
pub mod portapapeles;
pub mod seleccion;
pub mod seleccion_rapida;
pub mod sesion;
pub mod tresd;
pub mod xref;

pub use dibujo::{Dibujo, Transaccion};
pub use opencadcodec;
pub use sesion::Sesion;
