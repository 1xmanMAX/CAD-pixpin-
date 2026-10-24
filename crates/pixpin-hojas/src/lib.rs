//! **Hojas de calculo de Excel (`.xlsx`, `.xlsm`) leidas dentro de PixPin.**
//!
//! Crate puro: no sabe de Windows ni pinta. Lee el libro (`.xlsx` y, sin
//! Office, el `.xls` antiguo BIFF8: [`xls`], [`cfb`]) y deja todo listo
//! para que `pixpin-render` (modulo `hoja_office`) lo pinte como lo pinta
//! Excel: anchos de columna y altos de fila en pixeles, estilos ya
//! resueltos (tema, matiz, paleta), formatos de numero y fecha aplicados,
//! combinaciones, hipervinculos, paneles inmovilizados e imagenes y
//! graficos anclados.
//!
//! **Lo primero es la velocidad.** Al abrir solo se leen la lista de hojas,
//! los estilos y el indice de los textos compartidos (sin crear un solo
//! `String`). La hoja se lee de tiron mientras se descomprime
//! ([`hoja::LectorHoja`]): la primera pantalla sale con las primeras filas
//! y el resto llega despues, en el mismo hilo por tandas o en otro.
//!
//! ```no_run
//! use pixpin_hojas::{Libro, Regional, Rejilla, Vista};
//! let libro = Libro::abrir("informe.xlsx".as_ref(), Regional::es_pe()).unwrap();
//! let mut hoja = libro.abrir_hoja(libro.hoja_activa);
//! hoja.cargar_hasta_fila(100); // lo de la primera pantalla
//! let mut rejilla = Rejilla::nueva(&hoja, &libro.estilos.fuente_normal);
//! let vista = Vista { ancho: 1200.0, alto: 800.0, escala: 1.0, x: 0.0, y: 0.0 };
//! for panel in rejilla.paneles(&vista) { /* pintar panel */ }
//! hoja.cargar_todo(); // o por tandas: hoja.cargar_tanda(50_000)
//! rejilla.actualizar(&hoja);
//! ```
//!
//! Escrito desde ECMA-376 (SpreadsheetML), sin copiar codigo de nadie. El
//! XML se lee con un lector propio ([`xml`]) que no expande entidades.

#![forbid(unsafe_code)]

pub mod cfb;
pub mod comentarios;
pub mod condicional;
pub mod dibujos;
pub mod estilos;
pub mod filtros;
pub mod formato;
pub mod hoja;
pub mod libro;
pub mod minigraficos;
pub mod paquete;
pub mod rejilla;
pub mod tablas;
pub mod textos;
pub mod validacion;
pub mod xls;
pub mod xml;

pub use dibujos::{Ancla, Contenido as ContenidoDibujo, Dibujo};
pub use estilos::{
    Alineacion, Borde, EstiloLinea, Fuente, Horizontal, Linea, Relleno, Rgb, Subrayado, Vertical,
};
pub use formato::{Mostrado, Regional};
pub use hoja::{
    Celda, Fila, FilaNueva, Hoja, LectorHoja, Rango, Tanda, Valor, VistaHoja, nombre_columna,
};
pub use libro::{Clase, Contenido, InfoHoja, Libro, TipoHoja, Visibilidad};
pub use rejilla::{Panel, Rejilla, Vista};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorHojas {
    #[error("no es un ZIP (¿es un .xls antiguo?)")]
    NoEsZip,
    #[error("el ZIP no tiene libro de Excel")]
    SinLibro,
    #[error("libro binario (.xlsb): no se puede leer")]
    Binario,
    #[error("no se pudo leer: {0}")]
    Io(String),
    #[error("el libro supera los limites de seguridad: {0}")]
    Limite(String),
}

#[cfg(test)]
mod pruebas;
