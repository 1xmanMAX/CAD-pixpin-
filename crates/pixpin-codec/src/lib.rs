//! pixpin-codec — codificacion y decodificacion de imagen.
//!
//! Este crate habla con librerias C. El `unsafe` esta permitido, pero cada
//! bloque lleva su comentario `// SAFETY:`.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod cosido;
pub mod filtros;
pub mod gif;
pub mod imagen;
pub mod portapapeles;
pub mod vista;
pub mod wic;

pub use cosido::{
    Cosedor, Orden, Plan, Resultado, SIN_ENCAJE, encontrar_desplazamiento, es_lisa, firmas,
    franjas_fijas,
};
pub use gif::{ErrorGif, OpcionesGif, codificar as codificar_gif};
pub use imagen::{
    ErrorCodec, FormatoImagen, ImagenRgba, cargar, codificar_png, guardar, redimensionar,
};
pub use portapapeles::{
    ContenidoPortapapeles, construir_hdrop, copiar_ficheros, copiar_imagen,
    copiar_imagen_y_ficheros, copiar_texto, leer,
};
