//! pixpin-nivel — la politica de rendimiento como funcion pura.
//!
//! Este crate decide en que nivel corre la aplicacion (`Completo` o `Ligero`)
//! y con que presupuesto de recursos, a partir de los `Hechos` del equipo.
//! No sabe recogerlos: eso es hablar con Windows y vive en `pixpin_shell`.
//! Separar la recogida (L1) de la decision (L0) permite probar la politica
//! con equipos inventados que seria carisimo tener delante: la maquina suelo,
//! una VM con WARP, un sobremesa con VRAM de sobra.
//!
//! Diseno: docs/superpowers/specs/2026-08-31-rendimiento-equipos-modestos-design.md
#![forbid(unsafe_code)]

pub mod hechos;
pub mod nivel;
pub mod presupuesto;

pub use hechos::{FL_11_0, GIB, Hechos};
pub use nivel::{Decision, Nivel, Preferencia, Razon, decidir};
pub use presupuesto::Presupuesto;
