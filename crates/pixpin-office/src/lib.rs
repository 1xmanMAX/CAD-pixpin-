//! **Word y PowerPoint maquetados como en Office**, pagina a pagina.
//!
//! `pixpin-docs` lee un `.docx` como una lista de bloques que se reflujan
//! (como una pagina web). Aqui se hace lo otro: **la pagina tal como la
//! imprime Word** (tamano, margenes, columnas, encabezados, saltos) y **la
//! diapositiva tal como la proyecta PowerPoint** (su tamano, sus formas en
//! su sitio, heredadas del diseno y del patron).
//!
//! El crate es puro: no sabe de Windows. Mide el texto a traves de un
//! [`medidor::Medidor`] (DirectWrite en la aplicacion, uno de ancho fijo en
//! las pruebas) y devuelve [`pagina::Pagina`]s con todo ya colocado en
//! puntos, que pinta `pixpin-render::pagina_office`.
//!
//! **Perezoso**: abrir solo lee el ZIP y las partes de estilo; cada pagina
//! se maqueta cuando se pide (Word tiene que ir en orden, PowerPoint no), y
//! asi la primera se ensena antes de haber leido el resto.

#![forbid(unsafe_code)]

pub mod dml;
pub mod docx;
pub mod formas;
pub mod geometria;
pub mod grafico;
pub mod medidor;
pub mod pagina;
pub mod paquete;
pub mod pptx;
pub mod renglones;
pub mod tema;
pub mod xml;

use std::path::Path;
use std::sync::Arc;

pub use medidor::{Medidor, MedidorFijo, Metricas};
pub use pagina::{Color, Elemento, Letra, Pagina, Recursos};
pub use paquete::ErrorOffice;

/// Que tipo de documento es por la extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Word,
    PowerPoint,
}

/// El tipo por el nombre del fichero, o `None` si no es de los que se
/// maquetan aqui.
pub fn tipo_de(nombre: &str) -> Option<Tipo> {
    let ext = nombre.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "docx" | "docm" | "dotx" | "dotm" => Some(Tipo::Word),
        "pptx" | "pptm" | "potx" | "potm" | "ppsx" | "ppsm" => Some(Tipo::PowerPoint),
        _ => None,
    }
}

/// Un documento de Office abierto, del tipo que sea.
#[allow(clippy::large_enum_variant)] // hay uno por visor abierto: no compensa una caja
pub enum Documento {
    Word(docx::DocumentoWord),
    PowerPoint(pptx::Presentacion),
}

impl Documento {
    /// Abre un `.docx` o un `.pptx` (lee el ZIP y las partes de estilo, sin
    /// maquetar nada).
    pub fn abrir(ruta: &Path) -> Result<Documento, ErrorOffice> {
        let bytes: Arc<[u8]> = Arc::from(std::fs::read(ruta)?);
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        Documento::de_bytes(bytes, tipo_de(&nombre))
    }

    /// Lo mismo desde memoria. Sin tipo, se mira dentro del paquete.
    pub fn de_bytes(bytes: Arc<[u8]>, tipo: Option<Tipo>) -> Result<Documento, ErrorOffice> {
        let mut paquete = paquete::Paquete::de_bytes(bytes)?;
        let tipo = match tipo {
            Some(t) => t,
            None => {
                if paquete.existe("ppt/presentation.xml") {
                    Tipo::PowerPoint
                } else {
                    Tipo::Word
                }
            }
        };
        let _ = &mut paquete;
        Ok(match tipo {
            Tipo::Word => Documento::Word(docx::DocumentoWord::de_paquete(paquete)?),
            Tipo::PowerPoint => Documento::PowerPoint(pptx::Presentacion::de_paquete(paquete)?),
        })
    }

    pub fn tipo(&self) -> Tipo {
        match self {
            Documento::Word(_) => Tipo::Word,
            Documento::PowerPoint(_) => Tipo::PowerPoint,
        }
    }

    /// La pagina `n` (desde 0), maquetandola si aun no lo estaba (en Word,
    /// con todas las anteriores). `None` si no existe.
    pub fn pagina(&mut self, n: usize, medidor: &dyn Medidor) -> Option<Arc<Pagina>> {
        match self {
            Documento::Word(d) => d.pagina(n, medidor),
            Documento::PowerPoint(p) => p.diapositiva(n, medidor),
        }
    }

    /// Cuantas paginas hay ya maquetadas (Word) o cuantas diapositivas hay.
    pub fn paginas_listas(&self) -> usize {
        match self {
            Documento::Word(d) => d.paginas_listas(),
            Documento::PowerPoint(p) => p.cuantas(),
        }
    }

    /// Si ya se sabe el total de paginas (Word: maquetado hasta el final).
    pub fn terminado(&self) -> bool {
        match self {
            Documento::Word(d) => d.terminado(),
            Documento::PowerPoint(_) => true,
        }
    }

    /// El total si ya se sabe; si no, una estimacion (Word: la que guardo
    /// el propio Word en `docProps/app.xml`).
    pub fn total(&self) -> usize {
        match self {
            Documento::Word(d) => d.total(),
            Documento::PowerPoint(p) => p.cuantas(),
        }
    }

    /// Maqueta todo lo que falte (Word). Para quien quiera el total exacto.
    pub fn maquetar_todo(&mut self, medidor: &dyn Medidor) {
        if let Documento::Word(d) = self {
            d.maquetar_todo(medidor);
        }
    }

    /// Las imagenes, para el pintor.
    pub fn recursos(&self) -> Arc<Recursos> {
        match self {
            Documento::Word(d) => d.recursos(),
            Documento::PowerPoint(p) => p.recursos(),
        }
    }
}
