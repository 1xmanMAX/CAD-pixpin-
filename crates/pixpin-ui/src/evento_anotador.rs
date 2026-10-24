//! El sobre en el que llegan los pedidos de anotar en un pin.
//!
//! Aqui vivia el `Anotador` viejo: una maquina de anotar propia, con once
//! herramientas, para la capa de pantalla y la paleta del pin. Desde el
//! 2026-09-24 las dos usan el nucleo de dibujo unico (`apps/pixpin/src/
//! dibujo/`, el del lienzo), y la maquina quedo sin nadie que la llamara;
//! se borro el 2026-09-26 con sus pruebas. Quedan solo los eventos, porque
//! el pin los sigue usando para traducir lo que le llega de su ventana
//! (`pines.rs`) antes de pasarselo al nucleo.

use pixpin_motor2d::elemento::ColorRgba;
use pixpin_motor2d::vector::Punto2;

// La herramienta vive en el motor desde que la maquina del gesto se mudo
// alli: quien decide que hace un clic tiene que saber que herramienta hay
// puesta, y esa decision es logica pura, no interfaz.
pub use pixpin_motor2d::gesto::Herramienta;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeclaAnotador {
    Escape,
    Deshacer,
    Rehacer,
    Suprimir,
    /// Confirma el texto en curso (D57).
    Enter,
    /// Borra el ultimo caracter del texto en curso.
    Retroceso,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventoAnotador {
    Pulsar(Punto2),
    Mover(Punto2),
    /// Como `Mover`, pero con un punto de la entrada fina: uno que Windows
    /// fusiono o uno del lapiz con presion (E1).
    Muestra {
        p: Punto2,
        presion: Option<f32>,
    },
    Soltar(Punto2),
    /// Positivo hacia arriba, como manda Windows.
    Rueda(i32),
    Tecla(TeclaAnotador),
    /// Un caracter escrito, ya compuesto (el IME entrega el resultado).
    Caracter(char),
    CambiarHerramienta(Herramienta),
    CambiarColor(ColorRgba),
}
