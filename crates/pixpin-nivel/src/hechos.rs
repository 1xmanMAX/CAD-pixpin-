//! Los hechos del equipo sobre los que se decide el nivel. Solo datos.

pub const GIB: u64 = 1024 * 1024 * 1024;

/// `D3D_FEATURE_LEVEL_11_0`, con la codificacion numerica de Windows.
pub const FL_11_0: u32 = 0xb000;

/// Lo que se sabe del equipo al arrancar. Lo rellena `pixpin_shell::hechos`
/// con cuatro consultas del orden de microsegundos; aqui es un valor inerte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hechos {
    pub ram_fisica_bytes: u64,
    pub nucleos_fisicos: u32,
    pub nucleos_logicos: u32,
    /// Cero en graficos integrados: la textura sale de la RAM del sistema.
    pub vram_dedicada_bytes: u64,
    /// El adaptador es WARP u otro render por software.
    pub gpu_es_software: bool,
    /// Nivel de caracteristica D3D. Cero significa "no se pudo crear".
    pub nivel_caracteristica: u32,
}
