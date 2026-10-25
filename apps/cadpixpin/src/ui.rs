//! **Las piezas de la interfaz**: botones que se pintan cada fotograma y
//! dejan su zona para el clic (sin controles de Windows), como las barras de
//! PixPin.

use std::path::PathBuf;

use cad_nucleo::enganche::Tipo;
use cad_nucleo::ordenes::{Grupo, Id};
use pixpin_render::{Color, Pintor, RectF};

use crate::tema;

/// Lo que hace un clic en una zona.
#[derive(Debug, Clone, PartialEq)]
pub enum Accion {
    // Inicio
    PlanoNuevo,
    AbrirPlano,
    AbrirModelo,
    AbrirReciente(PathBuf),
    /// El metrado de un modelo BIM (sin abrirlo), a CSV.
    MetradoBim,
    /// Abrir la copia del autoguardado (su ruta) en vez del original.
    Recuperar(PathBuf),
    /// Borrar una copia del autoguardado que no se quiere.
    DescartarCopia(PathBuf),
    // Mesa
    /// Ver el plano abierto `i` (su pestana).
    Documento(usize),
    /// Cerrar el plano abierto `i` (preguntando si guardar).
    CerrarDocumento(usize),
    Inicio,
    Guardar,
    GuardarComo,
    Pestana(Grupo),
    Orden(Id),
    Encuadre,
    CapaActual(String),
    CapaVisible(String),
    CapaBloqueo(String),
    PasarACapa(String),
    MetradoPlano,
    Alternar(Interruptor),
    Enganche(Tipo),
    /// Abrir la lista de una propiedad de lo elegido (a la altura `y` del panel).
    Desplegar(Campo, f32),
    /// Cambiar una propiedad de lo elegido.
    Propiedad(cad_nucleo::propiedades::Cambio),
    /// Repetir la ultima orden (menu del boton derecho).
    Repetir,
    /// Cancelar la orden o, sin orden, soltar lo elegido.
    Cancelar,
    /// Llamar la atencion sobre el panel de propiedades.
    VerPropiedades,
}

/// Las propiedades que se cambian desde el panel con una lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo {
    Capa,
    Color,
    TipoLinea,
    Grosor,
}

/// Los interruptores de la barra de estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interruptor {
    Enganche,
    Orto,
    Polar,
    Rejilla,
}

#[derive(Debug, Clone)]
pub struct Zona {
    pub r: RectF,
    pub accion: Accion,
    /// El globo al pasar por encima.
    pub ayuda: String,
}

pub fn dentro(r: &RectF, x: f32, y: f32) -> bool {
    x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto
}

pub fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> RectF {
    RectF { x, y, ancho, alto }
}

/// Como se pinta un boton.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Estilo {
    /// Sin fondo hasta pasar por encima.
    Plano,
    /// Elegido (la pestana actual, un interruptor encendido).
    Elegido,
    /// El principal (azul).
    Principal,
}

/// Pinta un boton con su texto centrado y deja su zona.
#[allow(clippy::too_many_arguments)]
pub fn boton(
    p: &Pintor,
    zonas: &mut Vec<Zona>,
    r: RectF,
    texto: &str,
    tam: f32,
    estilo: Estilo,
    encima: Option<(f32, f32)>,
    accion: Accion,
    ayuda: &str,
) {
    let sobre = encima.is_some_and(|(x, y)| dentro(&r, x, y));
    let (fondo, color): (Option<Color>, Color) = match estilo {
        Estilo::Principal => (
            Some(if sobre {
                tema::rgb(0x40, 0x9C, 0xFF)
            } else {
                tema::ACENTO
            }),
            tema::TEXTO,
        ),
        Estilo::Elegido => (Some(tema::ACENTO_SUAVE), tema::ACENTO),
        Estilo::Plano => (sobre.then_some(tema::PANEL_ALTO), tema::TEXTO),
    };
    if let Some(f) = fondo {
        p.rellenar_redondeado(r, 6.0_f32.min(r.alto / 2.0), f);
    }
    let (w, h) = p.medir_texto(texto, tam);
    p.texto(
        texto,
        r.x + (r.ancho - w) / 2.0,
        r.y + (r.alto - h) / 2.0,
        tam,
        color,
    );
    zonas.push(Zona {
        r,
        accion,
        ayuda: ayuda.to_string(),
    });
}

/// Un texto recortado a `ancho` (con «…» si no cabe).
pub fn recortar(p: &Pintor, texto: &str, tam: f32, ancho: f32) -> String {
    if p.medir_texto(texto, tam).0 <= ancho {
        return texto.to_string();
    }
    let mut s: Vec<char> = texto.chars().collect();
    while !s.is_empty() {
        s.pop();
        let t: String = s.iter().collect::<String>() + "…";
        if p.medir_texto(&t, tam).0 <= ancho {
            return t;
        }
    }
    String::new()
}

/// El globo de ayuda junto al cursor.
pub fn globo(p: &Pintor, texto: &str, x: f32, y: f32, e: f32, w: f32) {
    if texto.is_empty() {
        return;
    }
    let tam = tema::LETRA_PEQUENA * e;
    let (tw, th) = p.medir_texto(texto, tam);
    let (bx, by) = ((x + 14.0 * e).min(w - tw - 16.0 * e), y + 20.0 * e);
    p.rellenar_redondeado(
        rect(bx, by, tw + 14.0 * e, th + 8.0 * e),
        5.0 * e,
        tema::rgba(0x0E, 0x0E, 0x10, 0.95),
    );
    p.texto(texto, bx + 7.0 * e, by + 4.0 * e, tam, tema::TEXTO);
}
