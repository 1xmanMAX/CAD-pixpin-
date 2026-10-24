//! **Los iconos de la barra: los mismos que los de los demas visores.** El
//! usuario (19-oct-2026): «los mismos iconos deben mantenerse para todos los
//! visores y no variar, para evitar confusiones». Son los trazados de
//! Material de `pixpin_render::icono::material` copiados tal cual (el `d`
//! del `24px.svg` oficial de `google/material-design-icons`, Apache-2.0);
//! aqui no se tira de `pixpin-render` (Direct2D, Office...) solo para eso.
//! Una prueba de la app (`plano_cad`) comprueba que no se separan.
//!
//! Se pintan con la misma geometria que el plano: el trazado se lee
//! ([`crate::trayecto_svg`]) y lyon lo rellena en triangulos.

use lyon_tessellation::geom::point;
use lyon_tessellation::path::Path;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
};

use crate::modelo::Constructor;
use crate::trayecto_svg::{Tramo, analizar};

/// Un icono de Material de 24 x 24: su trazado y si se rellena par-impar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icono {
    pub d: &'static str,
    pub par_impar: bool,
}

/// `drag_indicator` de Material, el de `pixpin_render::icono::material::DRAG_INDICATOR`.
pub const DRAG_INDICATOR: Icono = Icono {
    d: "M11 18c0 1.1-.9 2-2 2s-2-.9-2-2 .9-2 2-2 2 .9 2 2zm-2-8c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0-6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm6 4c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm0 2c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0 6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z",
    par_impar: false,
};

/// `straighten` de Material, el de `pixpin_render::icono::material::STRAIGHTEN`.
pub const STRAIGHTEN: Icono = Icono {
    d: "M21 6H3c-1.1 0-2 .9-2 2v8c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm0 10H3V8h2v4h2V8h2v4h2V8h2v4h2V8h2v4h2V8h2v8z",
    par_impar: false,
};

/// `crop_free` de Material, el de `pixpin_render::icono::material::CROP_FREE`.
pub const CROP_FREE: Icono = Icono {
    d: "M3 5v4h2V5h4V3H5c-1.1 0-2 .9-2 2zm2 10H3v4c0 1.1.9 2 2 2h4v-2H5v-4zm14 4h-4v2h4c1.1 0 2-.9 2-2v-4h-2v4zm0-16h-4v2h4v4h2V5c0-1.1-.9-2-2-2z",
    par_impar: false,
};

/// `edit` de Material, el de `pixpin_render::icono::material::EDIT`.
pub const EDIT: Icono = Icono {
    d: "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z",
    par_impar: false,
};

/// `view_in_ar` de Material, el de `pixpin_render::icono::material::VIEW_IN_AR`.
pub const VIEW_IN_AR: Icono = Icono {
    d: "M18.25 7.6l-5.5-3.18c-.46-.27-1.04-.27-1.5 0L5.75 7.6c-.46.27-.75.76-.75 1.3v6.35c0 .54.29 1.03.75 1.3l5.5 3.18c.46.27 1.04.27 1.5 0l5.5-3.18c.46-.27.75-.76.75-1.3V8.9c0-.54-.29-1.03-.75-1.3zM7 14.96v-4.62l4 2.32v4.61l-4-2.31zm5-4.03L8 8.61l4-2.31 4 2.31-4 2.32zm1 6.34v-4.61l4-2.32v4.62l-4 2.31zM7 2H3.5C2.67 2 2 2.67 2 3.5V7h2V4h3V2zm10 0h3.5c.83 0 1.5.67 1.5 1.5V7h-2V4h-3V2zM7 22H3.5c-.83 0-1.5-.67-1.5-1.5V17h2v3h3v2zm10 0h3.5c.83 0 1.5-.67 1.5-1.5V17h-2v3h-3v2z",
    par_impar: false,
};

/// `font_download` de Material, el de `pixpin_render::icono::material::FONT_DOWNLOAD`.
pub const FONT_DOWNLOAD: Icono = Icono {
    d: "M9.93 13.5h4.14L12 7.98zM20 2H4c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-4.05 16.5l-1.14-3H9.17l-1.12 3H5.96l5.11-13h1.86l5.11 13h-2.09z",
    par_impar: false,
};

/// `content_cut` de Material, el de `pixpin_render::icono::material::CONTENT_CUT`.
pub const CONTENT_CUT: Icono = Icono {
    d: "M9.64 7.64c.23-.5.36-1.05.36-1.64 0-2.21-1.79-4-4-4S2 3.79 2 6s1.79 4 4 4c.59 0 1.14-.13 1.64-.36L10 12l-2.36 2.36C7.14 14.13 6.59 14 6 14c-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4c0-.59-.13-1.14-.36-1.64L12 14l7 7h3v-1L9.64 7.64zM6 8c-1.1 0-2-.89-2-2s.9-2 2-2 2 .89 2 2-.9 2-2 2zm0 12c-1.1 0-2-.89-2-2s.9-2 2-2 2 .89 2 2-.9 2-2 2zm6-7.5c-.28 0-.5-.22-.5-.5s.22-.5.5-.5.5.22.5.5-.22.5-.5.5zM19 3l-6 6 2 2 7-7V3z",
    par_impar: false,
};

/// `layers` de Material, el de `pixpin_render::icono::material::LAYERS`.
pub const LAYERS: Icono = Icono {
    d: "M11.99 18.54l-7.37-5.73L3 14.07l9 7 9-7-1.63-1.27-7.38 5.74zM12 16l7.36-5.73L21 9l-9-7-9 7 1.63 1.27L12 16z",
    par_impar: false,
};

/// `dark_mode` de Material, el de `pixpin_render::icono::material::DARK_MODE`.
pub const DARK_MODE: Icono = Icono {
    d: "M12,3c-4.97,0-9,4.03-9,9s4.03,9,9,9s9-4.03,9-9c0-0.46-0.04-0.92-0.1-1.36c-0.98,1.37-2.58,2.26-4.4,2.26 c-2.98,0-5.4-2.42-5.4-5.4c0-1.81,0.89-3.42,2.26-4.4C12.92,3.04,12.46,3,12,3L12,3z",
    par_impar: false,
};

/// `light_mode` de Material, el de `pixpin_render::icono::material::LIGHT_MODE`.
pub const LIGHT_MODE: Icono = Icono {
    d: "M12,7c-2.76,0-5,2.24-5,5s2.24,5,5,5s5-2.24,5-5S14.76,7,12,7L12,7z M2,13l2,0c0.55,0,1-0.45,1-1s-0.45-1-1-1l-2,0 c-0.55,0-1,0.45-1,1S1.45,13,2,13z M20,13l2,0c0.55,0,1-0.45,1-1s-0.45-1-1-1l-2,0c-0.55,0-1,0.45-1,1S19.45,13,20,13z M11,2v2 c0,0.55,0.45,1,1,1s1-0.45,1-1V2c0-0.55-0.45-1-1-1S11,1.45,11,2z M11,20v2c0,0.55,0.45,1,1,1s1-0.45,1-1v-2c0-0.55-0.45-1-1-1 C11.45,19,11,19.45,11,20z M5.99,4.58c-0.39-0.39-1.03-0.39-1.41,0c-0.39,0.39-0.39,1.03,0,1.41l1.06,1.06 c0.39,0.39,1.03,0.39,1.41,0s0.39-1.03,0-1.41L5.99,4.58z M18.36,16.95c-0.39-0.39-1.03-0.39-1.41,0c-0.39,0.39-0.39,1.03,0,1.41 l1.06,1.06c0.39,0.39,1.03,0.39,1.41,0c0.39-0.39,0.39-1.03,0-1.41L18.36,16.95z M19.42,5.99c0.39-0.39,0.39-1.03,0-1.41 c-0.39-0.39-1.03-0.39-1.41,0l-1.06,1.06c-0.39,0.39-0.39,1.03,0,1.41s1.03,0.39,1.41,0L19.42,5.99z M7.05,18.36 c0.39-0.39,0.39-1.03,0-1.41c-0.39-0.39-1.03-0.39-1.41,0l-1.06,1.06c-0.39,0.39-0.39,1.03,0,1.41s1.03,0.39,1.41,0L7.05,18.36z",
    par_impar: false,
};

/// `fullscreen` de Material, el de `pixpin_render::icono::material::FULLSCREEN`.
pub const FULLSCREEN: Icono = Icono {
    d: "M7 14H5v5h5v-2H7v-3zm-2-4h2V7h3V5H5v5zm12 7h-3v2h5v-5h-2v3zM14 5v2h3v3h2V5h-5z",
    par_impar: false,
};

/// `fullscreen_exit` de Material, el de `pixpin_render::icono::material::FULLSCREEN_EXIT`.
pub const FULLSCREEN_EXIT: Icono = Icono {
    d: "M5 16h3v3h2v-5H5v2zm3-8H5v2h5V5H8v3zm6 11h2v-3h3v-2h-5v5zm2-11V5h-2v5h5V8h-3z",
    par_impar: false,
};

/// `push_pin` de Material, el de `pixpin_render::icono::material::PUSH_PIN`.
pub const PUSH_PIN: Icono = Icono {
    d: "M16,9V4l1,0c0.55,0,1-0.45,1-1v0c0-0.55-0.45-1-1-1H7C6.45,2,6,2.45,6,3v0 c0,0.55,0.45,1,1,1l1,0v5c0,1.66-1.34,3-3,3h0v2h5.97v7l1,1l1-1v-7H19v-2h0C17.34,12,16,10.66,16,9z",
    par_impar: true,
};

/// `bookmark_add` de Material, el de `pixpin_render::icono::material::BOOKMARK_ADD`.
pub const BOOKMARK_ADD: Icono = Icono {
    d: "M21,7h-2v2h-2V7h-2V5h2V3h2v2h2V7z M19,21l-7-3l-7,3V5c0-1.1,0.9-2,2-2l7,0c-0.63,0.84-1,1.87-1,3c0,2.76,2.24,5,5,5 c0.34,0,0.68-0.03,1-0.1V21z",
    par_impar: false,
};

/// `close` de Material, el de `pixpin_render::icono::material::CLOSE`.
pub const CLOSE: Icono = Icono {
    d: "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z",
    par_impar: false,
};

/// Todos, por su nombre en `material`: para comprobarlos de una vez.
pub const TODOS: &[(&str, Icono)] = &[
    ("DRAG_INDICATOR", DRAG_INDICATOR),
    ("STRAIGHTEN", STRAIGHTEN),
    ("CROP_FREE", CROP_FREE),
    ("EDIT", EDIT),
    ("VIEW_IN_AR", VIEW_IN_AR),
    ("FONT_DOWNLOAD", FONT_DOWNLOAD),
    ("CONTENT_CUT", CONTENT_CUT),
    ("LAYERS", LAYERS),
    ("DARK_MODE", DARK_MODE),
    ("LIGHT_MODE", LIGHT_MODE),
    ("FULLSCREEN", FULLSCREEN),
    ("FULLSCREEN_EXIT", FULLSCREEN_EXIT),
    ("PUSH_PIN", PUSH_PIN),
    ("CLOSE", CLOSE),
    ("BOOKMARK_ADD", BOOKMARK_ADD),
];

/// **El icono de cada boton de la barra**, el mismo que en los demas
/// visores: el sol en oscuro y la luna en claro (como el lector), y
/// pantalla completa o salir de ella.
pub fn de_boton(boton: &str, claro: bool, pantalla_completa: bool) -> Option<Icono> {
    Some(match boton {
        "asa" => DRAG_INDICATOR,
        "acotar" => STRAIGHTEN,
        "marcos" => CROP_FREE,
        "marcador" => BOOKMARK_ADD,
        "anotar" => EDIT,
        "tres" => VIEW_IN_AR,
        "letras" => FONT_DOWNLOAD,
        "corte" => CONTENT_CUT,
        "capas" => LAYERS,
        "tema" if claro => DARK_MODE,
        "tema" => LIGHT_MODE,
        "pantalla" if pantalla_completa => FULLSCREEN_EXIT,
        "pantalla" => FULLSCREEN,
        "fijar" => PUSH_PIN,
        "cerrar" => CLOSE,
        _ => return None,
    })
}

/// Pinta `icono` en el cuadrado de `lado` pixeles con la esquina en (x, y)
/// (pantalla, y hacia abajo), relleno de `color`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pintar(
    c: &mut Constructor,
    icono: Icono,
    x: f64,
    y: f64,
    lado: f64,
    color: u32,
    capa: f64,
) {
    let tri = triangulos(icono, x, y, lado);
    if !tri.is_empty() {
        c.triangulos(&tri, color, Some(capa));
    }
}

/// Los triangulos que rellenan el icono (de tres en tres), ya en pantalla.
pub(crate) fn triangulos(icono: Icono, x: f64, y: f64, lado: f64) -> Vec<[f64; 2]> {
    let Ok(tramos) = analizar(icono.d) else {
        return Vec::new();
    };
    let k = (lado / 24.0) as f32;
    // Se tesela en pixeles respecto a la esquina (lyon va en f32).
    let p = |q: (f32, f32)| point(q.0 * k, q.1 * k);
    let mut b = Path::builder();
    let (mut abierto, mut actual, mut inicio) = (false, (0.0f32, 0.0f32), (0.0f32, 0.0f32));
    for t in tramos {
        match t {
            Tramo::Mover(q) => {
                if abierto {
                    b.end(true);
                }
                b.begin(p(q));
                (abierto, actual, inicio) = (true, q, q);
            }
            Tramo::Linea(q) => {
                if !abierto {
                    b.begin(p(actual));
                    (abierto, inicio) = (true, actual);
                }
                b.line_to(p(q));
                actual = q;
            }
            Tramo::Cubica { c1, c2, fin } => {
                if !abierto {
                    b.begin(p(actual));
                    (abierto, inicio) = (true, actual);
                }
                b.cubic_bezier_to(p(c1), p(c2), p(fin));
                actual = fin;
            }
            Tramo::Cerrar => {
                if abierto {
                    b.end(true);
                    abierto = false;
                }
                // Tras «z» se sigue desde el principio de la figura.
                actual = inicio;
            }
        }
    }
    if abierto {
        b.end(true);
    }
    let camino = b.build();
    let mut bufs: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let regla = if icono.par_impar {
        FillRule::EvenOdd
    } else {
        FillRule::NonZero
    };
    let r = FillTessellator::new().tessellate_path(
        &camino,
        &FillOptions::tolerance(0.05).with_fill_rule(regla),
        &mut BuffersBuilder::new(&mut bufs, |v: FillVertex| v.position().to_array()),
    );
    if r.is_err() {
        return Vec::new();
    }
    bufs.indices
        .iter()
        .map(|&i| {
            let v = bufs.vertices[i as usize];
            [x + v[0] as f64, y + v[1] as f64]
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn area(t: &[[f64; 2]]) -> f64 {
        t.chunks_exact(3)
            .map(|c| {
                ((c[1][0] - c[0][0]) * (c[2][1] - c[0][1])
                    - (c[2][0] - c[0][0]) * (c[1][1] - c[0][1]))
                    .abs()
                    / 2.0
            })
            .sum()
    }

    #[test]
    fn todos_se_leen_y_pintan_dentro_de_su_cuadrado() {
        for (nombre, i) in TODOS {
            let t = triangulos(*i, 10.0, 20.0, 24.0);
            assert!(!t.is_empty(), "{nombre} no pinta nada");
            assert!(
                t.iter()
                    .all(|p| (9.4..=34.6).contains(&p[0]) && (19.4..=44.6).contains(&p[1])),
                "{nombre} se sale"
            );
        }
    }

    #[test]
    fn los_huecos_se_quedan_huecos() {
        // La cruz de cerrar: dos barras de unos 2 px de ancho, no un cuadrado lleno.
        let cruz = area(&triangulos(CLOSE, 0.0, 0.0, 24.0));
        assert!(cruz > 20.0 && cruz < 80.0, "{cruz}");
        // Pantalla completa: cuatro esquinas, mucho menos que su caja de 14 x 14.
        let esquinas = area(&triangulos(FULLSCREEN, 0.0, 0.0, 24.0));
        assert!(esquinas > 20.0 && esquinas < 100.0, "{esquinas}");
        // Caso negativo: un trazado que no se entiende no pinta nada.
        assert!(
            triangulos(
                Icono {
                    d: "Q",
                    par_impar: false
                },
                0.0,
                0.0,
                24.0
            )
            .is_empty()
        );
    }

    #[test]
    fn cada_boton_tiene_el_de_los_demas_visores() {
        assert_eq!(de_boton("fijar", false, false), Some(PUSH_PIN));
        assert_eq!(de_boton("tema", false, false), Some(LIGHT_MODE));
        assert_eq!(de_boton("tema", true, false), Some(DARK_MODE));
        assert_eq!(de_boton("pantalla", false, true), Some(FULLSCREEN_EXIT));
        assert_eq!(de_boton("nombre", false, false), None);
    }
}
