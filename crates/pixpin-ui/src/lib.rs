//! pixpin-ui — la interaccion como logica pura.
//!
//! Nada de este crate llama a Win32. Dibuja a traves de `pixpin-render` y
//! recibe eventos ya traducidos, de modo que el comportamiento del overlay
//! completo se prueba en milisegundos y sin escritorio. Si algun dia este
//! crate necesitase Win32, la frontera se ha roto: arreglar el diseno, no
//! relajar la regla.
#![forbid(unsafe_code)]

pub mod ajustes;
pub mod barra;
pub mod caja_herramientas;
pub mod chat;
/// Un color por tipo de archivo (`ColorDeExtension` del movil, v0.98.4).
pub mod color_de_extension;
pub mod confirmar;
pub mod evento_anotador;
pub mod historial;
pub mod hoja_compartir;
pub mod hojita;
pub mod info;
pub mod lupa;
pub mod menu;
pub mod mini;
pub mod overlay;
pub mod panel;
pub mod panel_lateral;
pub mod propiedades;
/// La pantalla de Proyectos del chat (`ui/Proyectos.kt` del movil).
pub mod proyectos;
pub mod reproductor;
pub mod resaltado;
pub mod riel_marcas;
pub mod tabla;

pub use barra::{AccionBarra, Barra};
pub use caja_herramientas::{
    BARRA_AGRUPADA, BOTONES, BOTONES_EDITOR, BotonCaja, CajaHerramientas, DestinoClic, GrupoBarra,
    MenuGrupo, agrupar, cara_del_grupo, grupo as grupo_boton, grupo_de_boton,
};
pub use evento_anotador::{EventoAnotador, Herramienta, TeclaAnotador};
pub use lupa::{FormatoColorLupa, Lupa, texto_color};
pub use overlay::{Efecto, EstadoOverlay, EventoEntrada, Fase, FormaCursor, TeclaOverlay};
pub use panel::PanelTodo;
