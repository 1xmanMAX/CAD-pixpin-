//! pixpin-render — el backend de dibujo Direct2D + DirectWrite.
//!
//! Frontera unica de dibujo del proyecto: todo lo que se pinta pasa por
//! aqui. Este crate habla con el sistema; `unsafe` permitido con `// SAFETY:`
//! en cada bloque. El bucle de render es dirigido por eventos, nunca por
//! fotogramas: sin trabajo no se dibuja nada, y de ahi sale el 0% de CPU.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod motor;

pub use motor::{Color, ErrorRender, MotorRender, premultiplicar, validar_tamano_rgba};

pub mod cache_presupuesto;
pub mod capa_estatica;
pub mod foto_capa;
pub mod fuera_de_pantalla;
pub mod grafito;
pub mod grano;
mod halo;
pub mod hoja_iconos;
pub mod hoja_office;
pub mod icono;
pub mod iconos_excalidraw;
pub mod imprimir;
pub mod imprimir_moderno;
pub mod lectura;
pub mod letras;
pub mod lienzo;
pub mod oscuro;
pub mod pagina_office;
pub mod perdida;
pub mod puntos;
pub mod superficie;
pub mod tinta;
pub mod trayecto_svg;

pub use superficie::{RitmoComposicion, Superficie};

pub use capa_estatica::{CapaEstatica, Estampa, sigue_valiendo};
pub use foto_capa::FotoDeCapa;
pub use grafito::{CacheGrafito, MapaGrafito};
pub use grano::{CacheGrano, GRADOS_DEL_GRANO};
pub use lienzo::{EstiloTexto, Interpolacion, Pintor, RectF, Tramo};
pub use tinta::{CacheTinta, PasoTrayecto, pasos_de_tinta, retardo_nitido};
