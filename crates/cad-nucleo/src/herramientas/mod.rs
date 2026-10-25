//! **El catalogo de ordenes enchufables** ([`crate::herramienta`]). Cada
//! modulo da sus `REGISTROS`; aqui se juntan.

use std::sync::LazyLock;

use crate::herramienta::Registro;

pub mod capas;
pub mod portapapeles;
pub mod puntos;
pub mod seleccionar;
pub mod vista;

/// Los registros de cada modulo (una linea por modulo).
const MODULOS: &[&[Registro]] = &[
    puntos::REGISTROS,
    vista::REGISTROS,
    capas::REGISTROS,
    portapapeles::REGISTROS,
    seleccionar::REGISTROS,
];

static CATALOGO: LazyLock<Vec<&'static Registro>> =
    LazyLock::new(|| MODULOS.iter().flat_map(|m| m.iter()).collect());

/// Todas las ordenes enchufables.
pub fn catalogo() -> &'static [&'static Registro] {
    &CATALOGO
}

/// El registro de una orden por su nombre.
pub fn de(nombre: &str) -> Option<&'static Registro> {
    catalogo()
        .iter()
        .copied()
        .find(|r| r.orden.nombre == nombre)
}
