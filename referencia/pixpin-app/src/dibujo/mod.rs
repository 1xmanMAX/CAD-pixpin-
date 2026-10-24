//! **Las herramientas de dibujo, una sola vez para toda la aplicacion.**
//!
//! El usuario lo pidio con estas palabras (2026-09-24): «estas herramientas
//! de dibujo tienen que ser usadas en todo lugar que necesite estas
//! herramientas, ya que se supone que optimizamos estas herramientas para
//! que sean usadas en todo lugar». Hasta entonces habia cuatro maneras de
//! dibujar (el lienzo, el lector, el pin y la capa de pantalla) y solo la
//! del lienzo tenia todas las herramientas.
//!
//! Aqui vive lo que no depende de ninguna ventana: que herramientas salen
//! (`permitidas`), que hace cada tecla y cada boton (`teclas`), la mano que
//! atiende el raton y el teclado (`mano`), como se pinta lo dibujado
//! (`pintar`), como se adapta la tinta a un papel oscuro (`tema`) y las
//! cuatro herramientas que no dibujan (`construir`). Cada anfitrion aporta
//! solo el fondo, la transformada documento→pantalla y donde se guarda.
//! Ver `docs/superpowers/specs/2026-09-24-herramientas-unicas-design.md`.

pub(crate) mod construir;
/// Los grupos de la barra: la cara de cada uno y el clic (como el movil).
pub(crate) mod grupos;
/// La varita de la lupa y apuntar lo que mira (lienzo-imagen, 26-sep).
pub(crate) mod lupa;
pub(crate) mod mano;
/// La cota del movil pintada por el camino de la pantalla, para mirarla.
#[cfg(test)]
mod muestra_cota;
/// La figura central de la lamina de geometria, con las herramientas del lienzo.
#[cfg(test)]
mod muestra_lamina;
pub(crate) mod permitidas;
pub(crate) mod pintar;
pub(crate) mod teclas;
pub(crate) mod tema;
