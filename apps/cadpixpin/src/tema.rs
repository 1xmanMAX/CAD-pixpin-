//! **Los colores y medidas** de la interfaz: los de PixPin Max (el gris
//! grafito de sus visores, el azul de acento, el verde y el naranja de sus
//! avisos), para que CAD-pixpin se sienta de la misma familia.

use pixpin_render::Color;

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

/// El fondo del plano (el del visor de PixPin en oscuro).
pub const FONDO_PLANO: [f32; 4] = [0.071, 0.071, 0.078, 1.0];
/// El fondo de la pantalla de inicio.
pub const FONDO: Color = rgb(0x1C, 0x1C, 0x1E);
/// Las barras y los paneles (el «cristal» de las barras de PixPin).
pub const PANEL: Color = rgb(0x26, 0x26, 0x2C);
pub const PANEL_ALTO: Color = rgb(0x30, 0x30, 0x37);
pub const BORDE: Color = rgb(0x3A, 0x3A, 0x42);
pub const TEXTO: Color = rgb(0xF5, 0xF5, 0xF7);
pub const TEXTO_SUAVE: Color = rgb(0xA1, 0xA1, 0xA8);
pub const TEXTO_APAGADO: Color = rgb(0x6E, 0x6E, 0x73);
/// El azul de PixPin (botones elegidos, la ventana de eleccion).
pub const ACENTO: Color = rgb(0x0A, 0x84, 0xFF);
pub const ACENTO_SUAVE: Color = rgba(0x0A, 0x84, 0xFF, 0.18);
pub const VERDE: Color = rgb(0x30, 0xD1, 0x58);
pub const VERDE_SUAVE: Color = rgba(0x30, 0xD1, 0x58, 0.15);
pub const NARANJA: Color = rgb(0xFF, 0x8A, 0x00);
pub const AMARILLO: Color = rgb(0xFF, 0xD6, 0x0A);
/// Lo que aun no se ha dibujado (la vista previa de la orden).
pub const PREVIA: Color = rgb(0x64, 0xD2, 0xFF);

/// Las medidas, en pixeles a 100 % (se multiplican por la escala del monitor).
pub const ALTO_PESTANAS: f32 = 34.0;
pub const ALTO_BOTONES: f32 = 58.0;
pub const ANCHO_PANEL: f32 = 270.0;
pub const ALTO_LINEA: f32 = 19.0;
pub const LINEAS_HISTORIAL: usize = 3;
pub const ALTO_ORDEN: f32 = 28.0;
pub const ALTO_ESTADO: f32 = 26.0;
pub const LETRA: f32 = 13.0;
pub const LETRA_PEQUENA: f32 = 11.0;
/// El lado del cuadrado de eleccion del cursor y el radio del enganche.
pub const CAJA_CURSOR: f32 = 10.0;
pub const RADIO_ENGANCHE: f32 = 12.0;

/// El color de un indice ACI (los nueve primeros y el gris para el resto),
/// para los cuadraditos de las capas.
pub fn de_aci(i: i16) -> Color {
    match i {
        1 => rgb(255, 0, 0),
        2 => rgb(255, 255, 0),
        3 => rgb(0, 255, 0),
        4 => rgb(0, 255, 255),
        5 => rgb(0, 0, 255),
        6 => rgb(255, 0, 255),
        7 => rgb(255, 255, 255),
        8 => rgb(128, 128, 128),
        9 => rgb(192, 192, 192),
        30 => rgb(255, 127, 0),
        140 => rgb(0, 170, 255),
        _ => rgb(160, 160, 160),
    }
}
