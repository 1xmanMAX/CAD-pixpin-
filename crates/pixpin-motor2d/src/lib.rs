//! El motor de edicion avanzada en 2D de PixPin Max.
//!
//! Dibujo vectorial a mano alzada, sin interfaz: lo usan el pin (anotar
//! dentro), la pantalla (anotar encima) y, mas adelante, el PDF. Por eso es un
//! crate propio y no codigo dentro del pin — tres consumidores muy distintos,
//! una sola verdad sobre que es un trazo.
//!
//! Diseno: `docs/superpowers/specs/2026-09-02-s3a-motor2d-design.md`.
//! Los algoritmos se estudiaron de Excalidraw (MIT) y estan documentados en
//! `docs/investigacion/2026-09-02-excalidraw-analisis.md`; la implementacion
//! es nueva.
//!
//! Casi todo es puro y se prueba sin escritorio: solo el modulo de dibujo
//! toca Direct2D.

#![forbid(unsafe_code)]

pub mod aligerar;
pub mod angulos;
pub mod arco;
pub mod azar;
pub mod biblioteca;
/// Reorganizar una hoja por bloques: mover, intercambiar, duplicar, borrar,
/// estirar y anadir filas con las piezas que ya hay (25-oct-2026).
pub mod bloques;
pub mod bolita;
pub mod borrador;
pub mod cache;
pub mod camara;
pub mod cara_exacta;
pub mod codo;
pub mod contraste;
pub mod cuadricula;
pub mod curva;
pub mod ecuacion;
pub mod elemento;
pub mod enganche;
pub mod enlace;
pub mod escalabarra;
pub mod escena;
pub mod estilo;
pub mod estirar_bloque;
pub mod excalidraw;
pub mod exportar;
pub mod exportar_html;
pub mod exportar_svg;
pub mod figuras_de_serie;
pub mod forma_rapida;
pub mod formas;
pub mod formato;
pub mod formula;
pub mod formulas;
pub mod geometria_exacta;
pub mod gesto;
pub mod grafica;
pub mod hojita;
pub mod impacto;
pub mod indice;
pub mod instrumento;
pub mod lazo;
pub mod lupa_elemento;
pub mod marcas;
pub mod marco;
pub mod medida;
pub mod mosaico;
pub mod nudos;
pub mod organizar;
pub mod perimetros;
pub mod pintado;
pub mod portapapeles;
pub mod puntero_laser;
pub mod puntos_etiquetados;
pub mod recorte;
pub mod recorte_imagen;
pub mod regiones;
pub mod relleno;
pub mod rough;
pub mod seleccion;
pub mod serie;
pub mod tabla_dibujada;
pub mod texto;
pub mod texto_en_figuras;
pub mod texto_pegado;
pub mod texto_rico;
pub mod tinta;
pub mod tiradores;
pub mod transformar;
pub mod vector;
pub mod zona;

pub use aligerar::aligerar;
pub use azar::Azar;
pub use camara::{Camara, cuantos_se_ven, recortar};
pub use elemento::{ColorRgba, Elemento, EstiloTrazo, Figura, RecorteImagen};
pub use escalabarra::Barra;
pub use escena::Escena;
pub use formas::{PUNTAS, TipoPunta, elipse, linea, rectangulo};
pub use formato::{EXTENSION, ErrorFormato, cargar, guardar};
pub use impacto::{TOLERANCIA, dentro_de, elemento_en, elementos_en, esquinas_giradas, toca};
pub use medida::{
    Escala, longitud_de, medida_de, rotulo_del_reves, texto_de_cota, texto_de_medida,
};
pub use organizar::{
    Alineacion, Reparto, agrupar, al_fondo, al_frente, alinear, desagrupar, hermanos_de, repartir,
};
pub use perimetros::{
    Contorno, PASO_PERIMETRO, contornos_de, intersecciones_cerca, punto_en_el_perimetro,
    segmentos_de,
};
pub use pintado::{
    Orden, marco_de_seleccion, ordenes, ordenes_a_distancia, ordenes_de_escena,
    ordenes_de_escena_vista,
};
pub use relleno::{EstiloRelleno, lineas_de_rayado};
pub use seleccion::Seleccion;
pub use vector::{Punto2, distancia_a_segmento};
