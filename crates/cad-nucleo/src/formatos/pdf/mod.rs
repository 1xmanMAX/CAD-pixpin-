//! **PDF**: leer (para importar sus vectores) y escribir (para exportar el
//! plano), en Rust puro.

pub mod exportar;
pub mod importar;
pub mod objetos;

pub use exportar::{Hoja, OpcionesPdf, Papel};
pub use objetos::Documento;
