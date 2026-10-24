//! pixpin-cad — ver planos DWG y DXF en una ventana como un pin, fluido
//! aunque el plano sea grande y el equipo modesto.
//!
//! - [`leer`]: abrir el fichero con `opencadcodec` (MPL-2.0).
//! - [`convertir`]: del plano a rayas y triangulos ([`modelo::Modelo`]).
//! - [`modelo`]: lo que va a la tarjeta grafica, ordenado para dibujar solo
//!   lo que se ve, y su cache en disco.

// `!(x > 0.0)` es a proposito en todo el crate: un NaN (un plano roto) no
// es mayor que cero y se rechaza en la misma comparacion.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

pub mod anotado;
pub mod convertir;
pub mod convertir3d;
pub mod croquis;
pub mod gpu;
pub mod gpu3d;
pub mod iconos;
pub mod leer;
pub mod letras;
pub mod marcadores;
pub mod modelo;
pub mod modelo3d;
pub mod proxy;
/// Pintar un plano a PNG en la CPU: para las pruebas y, con el rasgo
/// `pintar-png`, para las de la app (el banco del asistente).
#[cfg(any(test, feature = "pintar-png"))]
pub mod pruebas_png;
pub mod regla;
pub mod relleno;
pub mod shx;
pub mod teselar;
pub mod texto;
mod trayecto_svg;
pub mod ventana;
pub mod ventana3d;
pub mod web;
