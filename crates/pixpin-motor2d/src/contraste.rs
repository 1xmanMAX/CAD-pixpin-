//! **La tinta que se lee sobre el papel**, para lo que el motor pinta con un
//! color que no es exactamente el guardado: la cota.
//!
//! Puerto de `DrawTheme.adaptar` (`motor/Theme.kt`) y de
//! `contrastingTextColor` (`motor/Renderer.kt`) del movil. La cota del movil
//! pinta raya y numero con `tema(parseColor(strokeColor))`, o sea la tinta
//! adaptada al papel de la escena, y el halo del numero con el color
//! contrario a esa tinta. Sin esto, el cian claro de una cota sobre papel
//! blanco salia aqui cian claro con halo negro, y en el movil sale un cian
//! oscuro con halo blanco: el usuario lo vio en la misma foto en los dos
//! aparatos (27-sep-2026).
//!
//! Vive en el motor, y no en `apps/pixpin/src/dibujo/tema.rs` (que tiene la
//! misma cuenta para el papel de noche), porque la cota sale igual en
//! pantalla, en el PNG, en el SVG, en el PDF y en el papel: los cinco pasan
//! por `pintado::ordenes_medibles`, y el color tiene que llegar ya decidido.

use crate::elemento::ColorRgba;

/// Por debajo de esta saturacion, un color es un gris: tinta, no color.
const ES_GRIS: f64 = 0.14;
/// Por debajo de esto, la tinta se adapta. WCAG 1.4.11: trazos y graficos.
const CONTRASTE_QUE_VALE: f64 = 3.0;
/// Adonde se lleva la que se adapta: WCAG 1.4.3, texto normal.
const CONTRASTE_BUSCADO: f64 = 4.5;

fn a8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Luminancia relativa de WCAG, de canales de 0 a 255.
fn luminancia(r: u8, g: u8, b: u8) -> f64 {
    let lineal = |c: u8| {
        let v = c as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lineal(r) + 0.7152 * lineal(g) + 0.0722 * lineal(b)
}

/// El contraste de WCAG entre dos luminancias.
pub fn contraste(l1: f64, l2: f64) -> f64 {
    (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05)
}

/// El contraste de WCAG entre dos colores (el alfa no cuenta).
pub fn contraste_entre(a: ColorRgba, b: ColorRgba) -> f64 {
    contraste(
        luminancia(a8(a.r), a8(a.g), a8(a.b)),
        luminancia(a8(b.r), a8(b.g), a8(b.b)),
    )
}

fn a_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let alto = r.max(g).max(b);
    let bajo = r.min(g).min(b);
    let l = (alto + bajo) / 2.0;
    if alto == bajo {
        return (0.0, 0.0, l);
    }
    let d = alto - bajo;
    let s = if l > 0.5 {
        d / (2.0 - alto - bajo)
    } else {
        d / (alto + bajo)
    };
    let h = if alto == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if alto == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    (h, s, l)
}

fn de_hsl(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let canal = |t0: f64| {
        let mut t = t0;
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (canal(h + 1.0 / 3.0), canal(h), canal(h - 1.0 / 3.0))
}

/// **La tinta adaptada al papel** (`DrawTheme.adaptar`): si ya se lee (3:1)
/// no se toca; si no, se le cambia la claridad y se le deja el tono hasta
/// 4,5:1; un gris es tinta y se le da la vuelta. El alfa se conserva.
pub fn adaptar(c: ColorRgba, papel: ColorRgba) -> ColorRgba {
    let lp = luminancia(a8(papel.r), a8(papel.g), a8(papel.b));
    let (r8, g8, b8) = (a8(c.r), a8(c.g), a8(c.b));
    if contraste(luminancia(r8, g8, b8), lp) >= CONTRASTE_QUE_VALE {
        return c;
    }
    let (h, s, l) = a_hsl(r8 as f64 / 255.0, g8 as f64 / 255.0, b8 as f64 / 255.0);
    let gris = s < ES_GRIS;
    // Hacia donde hay mas contraste: oscurecer sobre papel claro, aclarar
    // sobre oscuro.
    let oscurecer = contraste(0.0, lp) >= contraste(1.0, lp);
    let con = |claridad: f64| {
        let (r, g, b) = de_hsl(h, if gris { 0.0 } else { s }, claridad.clamp(0.0, 1.0));
        let canal = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
        (canal(r), canal(g), canal(b))
    };
    // Un gris empieza en su espejo (blanco -> negro); un color, en su propia
    // claridad.
    let mut claridad = if gris { 1.0 - l } else { l };
    let paso = if oscurecer { -0.02 } else { 0.02 };
    let mut mejor = con(claridad);
    while (0.0..=1.0).contains(&claridad) {
        let x = con(claridad);
        mejor = x;
        if contraste(luminancia(x.0, x.1, x.2), lp) >= CONTRASTE_BUSCADO {
            break;
        }
        claridad += paso;
    }
    ColorRgba {
        r: mejor.0 as f32 / 255.0,
        g: mejor.1 as f32 / 255.0,
        b: mejor.2 as f32 / 255.0,
        a: c.a,
    }
}

/// **El papel contra el que se adapta**: el fondo de la escena, o blanco si
/// es transparente. `DrawTheme.colorDe("transparent")` no entiende la
/// palabra y devuelve blanco, y un papel transparente se ve sobre blanco en
/// el movil.
pub fn papel_de(fondo: ColorRgba) -> ColorRgba {
    if fondo.a <= 0.0 {
        ColorRgba::opaco(1.0, 1.0, 1.0)
    } else {
        ColorRgba { a: 1.0, ..fondo }
    }
}

/// **El color del halo de un rotulo** (`contrastingTextColor` del movil):
/// negro si la tinta es clara y blanco si es oscura, con la media ponderada
/// de sus canales (sin linealizar) contra 150 de 255, y con el alfa de la
/// tinta.
pub fn color_de_halo(tinta: ColorRgba) -> ColorRgba {
    let lum = 0.299 * a8(tinta.r) as f32 + 0.587 * a8(tinta.g) as f32 + 0.114 * a8(tinta.b) as f32;
    let v = if lum > 150.0 { 0.0 } else { 1.0 };
    ColorRgba {
        r: v,
        g: v,
        b: v,
        a: tinta.a,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hex(h: u32) -> ColorRgba {
        ColorRgba::opaco(
            ((h >> 16) & 0xff) as f32 / 255.0,
            ((h >> 8) & 0xff) as f32 / 255.0,
            (h & 0xff) as f32 / 255.0,
        )
    }

    const BLANCO: u32 = 0xffffff;

    #[test]
    fn un_cian_claro_sobre_papel_blanco_se_oscurece_sin_perder_el_tono() {
        let cian = hex(0x0edeff);
        let c = adaptar(cian, hex(BLANCO));
        assert!(
            contraste_entre(c, hex(BLANCO)) >= 4.5,
            "tiene que leerse: {c:?}"
        );
        // El mismo tono: el azul sigue mandando sobre el verde y el rojo va
        // casi a cero.
        assert!(c.b > c.g && c.g > c.r, "sigue siendo cian: {c:?}");
        assert!(c.b < cian.b, "mas oscuro que el original: {c:?}");
    }

    #[test]
    fn una_tinta_que_ya_se_lee_no_se_toca() {
        let rojo = hex(0xe03131);
        assert_eq!(adaptar(rojo, hex(BLANCO)), rojo);
        let negro = hex(0x1e1e1e);
        assert_eq!(adaptar(negro, hex(BLANCO)), negro);
    }

    #[test]
    fn el_halo_de_una_tinta_oscura_es_blanco_y_el_de_una_clara_negro() {
        // El cian tal cual (claro) llevaria halo negro: por eso hay que
        // adaptarlo antes, y entonces el halo sale blanco como en el movil.
        assert_eq!(color_de_halo(hex(0x0edeff)), hex(0x000000));
        let adaptado = adaptar(hex(0x0edeff), hex(BLANCO));
        assert_eq!(color_de_halo(adaptado), hex(0xffffff));
    }

    #[test]
    fn el_halo_conserva_el_alfa_de_la_tinta() {
        let medio = ColorRgba {
            a: 0.5,
            ..hex(0x000000)
        };
        assert_eq!(color_de_halo(medio).a, 0.5);
    }

    #[test]
    fn un_papel_transparente_cuenta_como_blanco() {
        let transparente = ColorRgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        assert_eq!(papel_de(transparente), hex(BLANCO));
        assert_eq!(papel_de(hex(0x121212)), hex(0x121212));
    }
}
