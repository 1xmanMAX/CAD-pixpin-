//! **El catalogo de ordenes enchufables** ([`crate::herramienta`]). Cada
//! modulo da sus `REGISTROS`; aqui se juntan.

use std::sync::LazyLock;

use crate::herramienta::Registro;

pub mod bloques;
pub mod capas;
pub mod cotas;
pub mod dibujo;
pub mod modificar;
pub mod portapapeles;
pub mod puntos;
pub mod seleccionar;
pub mod sombreado;
pub mod textos;
pub mod tresd;
pub mod vista;

/// Los registros de cada modulo (una linea por modulo).
const MODULOS: &[&[Registro]] = &[
    puntos::REGISTROS,
    dibujo::REGISTROS,
    modificar::REGISTROS,
    cotas::REGISTROS,
    textos::REGISTROS,
    bloques::REGISTROS,
    sombreado::REGISTROS,
    vista::REGISTROS,
    capas::REGISTROS,
    portapapeles::REGISTROS,
    seleccionar::REGISTROS,
    tresd::REGISTROS,
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
