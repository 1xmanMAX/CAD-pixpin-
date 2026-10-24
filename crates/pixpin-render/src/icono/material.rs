//! Los iconos de Material Icons que usa el chat de PixPin Android.
//!
//! El movil pinta su chat con `Icons.Filled.*` de Compose, que son los
//! Material Icons de Google en su estilo relleno. Para que las dos apps sean
//! la misma se copian los MISMOS dibujos, no unos parecidos: cada trazado es
//! el `d` del `24px.svg` oficial del repositorio `google/material-design-icons`
//! (carpeta `src/<categoria>/<nombre>/materialicons/`), sin retocar. Un
//! `<circle>` se reescribe como dos arcos, que es la misma figura.
//!
//! Licencia: Apache-2.0 (Copyright Google LLC), compatible con la MIT de
//! PixPin Max y admitida por `deny.toml`. Ver THIRD-PARTY-NOTICES.md.
//!
//! Los `AutoMirrored` de Compose (volver, responder, enviar, reenviar,
//! libro, lista) son el mismo dibujo girado solo en idiomas de derecha a
//! izquierda; aqui no hay ninguno, asi que van tal cual.

use super::{Icono, Pintura, TrazoIcono};

/// La caja de todos: 24 x 24, como el `viewBox` de Material.
const VISTA: (f32, f32, f32, f32) = (0.0, 0.0, 24.0, 24.0);

/// Un trazado relleno del color de quien pinta, que es como se dibujan todos
/// los de Material: sin trazo, solo relleno.
const fn relleno(d: &'static str, par_impar: bool) -> TrazoIcono {
    TrazoIcono {
        d,
        relleno: Pintura::Actual,
        trazo: Pintura::Nada,
        grosor: 0.0,
        extremo_redondo: false,
        union_redonda: false,
        opacidad: 1.0,
        par_impar,
        matriz: None,
        mascara: None,
    }
}

/// `add` (`src/content/add/materialicons/24px.svg`): el «+» de apuntar una
/// tarea nueva (tareas-v5).
pub const ADD: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno("M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z", false)],
};

/// `alarm` (`src/action/alarm/materialicons/24px.svg`).
pub const ALARM: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M22 5.72l-4.6-3.86-1.29 1.53 4.6 3.86L22 5.72zM7.88 3.39L6.6 1.86 2 5.71l1.29 1.53 4.59-3.85zM12.5 8H11v6l4.75 2.85.75-1.23-4-2.37V8zM12 4c-4.97 0-9 4.03-9 9s4.02 9 9 9c4.97 0 9-4.03 9-9s-4.03-9-9-9zm0 16c-3.87 0-7-3.13-7-7s3.13-7 7-7 7 3.13 7 7-3.13 7-7 7z",
        false,
    )],
};

/// `arrow_back` (`src/navigation/arrow_back/materialicons/24px.svg`).
pub const ARROW_BACK: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z",
        false,
    )],
};

/// `arrow_forward` (`src/navigation/arrow_forward/materialicons/24px.svg`).
pub const ARROW_FORWARD: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 4l-1.41 1.41L16.17 11H4v2h12.17l-5.58 5.59L12 20l8-8z",
        false,
    )],
};

/// `attach_file` (`src/editor/attach_file/materialicons/24px.svg`).
pub const ATTACH_FILE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M16.5 6v11.5c0 2.21-1.79 4-4 4s-4-1.79-4-4V5c0-1.38 1.12-2.5 2.5-2.5s2.5 1.12 2.5 2.5v10.5c0 .55-.45 1-1 1s-1-.45-1-1V6H10v9.5c0 1.38 1.12 2.5 2.5 2.5s2.5-1.12 2.5-2.5V5c0-2.21-1.79-4-4-4S7 2.79 7 5v12.5c0 3.04 2.46 5.5 5.5 5.5s5.5-2.46 5.5-5.5V6h-1.5z",
        false,
    )],
};

/// `bookmark_add` (`src/action/bookmark_add/materialicons/24px.svg`): poner
/// un marcador aqui, el de la cabecera del visor (v0.66 del movil).
pub const BOOKMARK_ADD: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M21,7h-2v2h-2V7h-2V5h2V3h2v2h2V7z M19,21l-7-3l-7,3V5c0-1.1,0.9-2,2-2l7,0c-0.63,0.84-1,1.87-1,3c0,2.76,2.24,5,5,5 c0.34,0,0.68-0.03,1-0.1V21z",
        false,
    )],
};

/// `bookmark_border` (`src/action/bookmark_border/materialicons/24px.svg`).
pub const BOOKMARK_BORDER: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M17 3H7c-1.1 0-1.99.9-1.99 2L5 21l7-3 7 3V5c0-1.1-.9-2-2-2zm0 15l-5-2.18L7 18V5h10v13z",
        false,
    )],
};

/// `check_box` (`src/toggle/check_box/materialicons/24px.svg`).
pub const CHECK_BOX: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M19 3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.11 0 2-.9 2-2V5c0-1.1-.89-2-2-2zm-9 14l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z",
        false,
    )],
};

/// `check_box_outline_blank` (`src/toggle/check_box_outline_blank/materialicons/24px.svg`).
pub const CHECK_BOX_OUTLINE_BLANK: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M19 5v14H5V5h14m0-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2z",
        false,
    )],
};

/// `check_circle` (`src/action/check_circle/materialicons/24px.svg`).
pub const CHECK_CIRCLE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z",
        false,
    )],
};

/// `checklist` (`src/editor/checklist/materialicons/24px.svg`).
pub const CHECKLIST: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M22,7h-9v2h9V7z M22,15h-9v2h9V15z M5.54,11L2,7.46l1.41-1.41l2.12,2.12l4.24-4.24l1.41,1.41L5.54,11z M5.54,19L2,15.46 l1.41-1.41l2.12,2.12l4.24-4.24l1.41,1.41L5.54,19z",
        false,
    )],
};

/// `close` (`src/navigation/close/materialicons/24px.svg`).
pub const CLOSE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z",
        false,
    )],
};

/// `compress` (`src/action/compress/materialicons/24px.svg`).
pub const COMPRESS: Icono = Icono {
    vista: VISTA,
    trazos: &[
        relleno(
            "M8 19h3v3h2v-3h3l-4-4-4 4zm8-15h-3V1h-2v3H8l4 4 4-4zM4 9v2h16V9H4z",
            false,
        ),
        relleno("M4 12h16v2H4z", false),
    ],
};

/// `content_copy` (`src/content/content_copy/materialicons/24px.svg`).
pub const CONTENT_COPY: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z",
        false,
    )],
};

/// `content_paste` (`src/content/content_paste/materialicons/24px.svg`).
pub const CONTENT_PASTE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M19 2h-4.18C14.4.84 13.3 0 12 0c-1.3 0-2.4.84-2.82 2H5c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-7 0c.55 0 1 .45 1 1s-.45 1-1 1-1-.45-1-1 .45-1 1-1zm7 18H5V4h2v3h10V4h2v16z",
        false,
    )],
};

/// `crop` (`src/image/crop/materialicons/24px.svg`).
pub const CROP: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M17 15h2V7c0-1.1-.9-2-2-2H9v2h8v8zM7 17V1H5v4H1v2h4v10c0 1.1.9 2 2 2h10v4h2v-4h4v-2H7z",
        false,
    )],
};

/// `delete` (`src/action/delete/materialicons/24px.svg`).
pub const DELETE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z",
        false,
    )],
};

/// `description` (`src/action/description/materialicons/24px.svg`).
pub const DESCRIPTION: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M14 2H6c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm2 16H8v-2h8v2zm0-4H8v-2h8v2zm-3-5V3.5L18.5 9H13z",
        false,
    )],
};

/// `draw` (`src/editor/draw/materialicons/24px.svg`).
pub const DRAW: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M18.85,10.39l1.06-1.06c0.78-0.78,0.78-2.05,0-2.83L18.5,5.09c-0.78-0.78-2.05-0.78-2.83,0l-1.06,1.06L18.85,10.39z M13.19,7.56L4,16.76V21h4.24l9.19-9.19L13.19,7.56z M19,17.5c0,2.19-2.54,3.5-5,3.5c-0.55,0-1-0.45-1-1s0.45-1,1-1 c1.54,0,3-0.73,3-1.5c0-0.47-0.48-0.87-1.23-1.2l1.48-1.48C18.32,15.45,19,16.29,19,17.5z M4.58,13.35C3.61,12.79,3,12.06,3,11 c0-1.8,1.89-2.63,3.56-3.36C7.59,7.18,9,6.56,9,6c0-0.41-0.78-1-2-1C5.74,5,5.2,5.61,5.17,5.64C4.82,6.05,4.19,6.1,3.77,5.76 C3.36,5.42,3.28,4.81,3.62,4.38C3.73,4.24,4.76,3,7,3c2.24,0,4,1.32,4,3c0,1.87-1.93,2.72-3.64,3.47C6.42,9.88,5,10.5,5,11 c0,0.31,0.43,0.6,1.07,0.86L4.58,13.35z",
        false,
    )],
};

/// `edit` (`src/image/edit/materialicons/24px.svg`).
pub const EDIT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z",
        false,
    )],
};

/// `emoji_emotions` (`src/social/emoji_emotions/materialicons/24px.svg`).
pub const EMOJI_EMOTIONS: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M11.99,2C6.47,2,2,6.48,2,12c0,5.52,4.47,10,9.99,10C17.52,22,22,17.52,22,12C22,6.48,17.52,2,11.99,2z M8.5,8 C9.33,8,10,8.67,10,9.5S9.33,11,8.5,11S7,10.33,7,9.5S7.67,8,8.5,8z M12,18c-2.28,0-4.22-1.66-5-4h10C16.22,16.34,14.28,18,12,18z M15.5,11c-0.83,0-1.5-0.67-1.5-1.5S14.67,8,15.5,8S17,8.67,17,9.5S16.33,11,15.5,11z",
        false,
    )],
};

/// `folder` (`src/file/folder/materialicons/24px.svg`).
pub const FOLDER: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z",
        false,
    )],
};

/// `forum` (`src/communication/forum/materialicons/24px.svg`).
pub const FORUM: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M21 6h-2v9H6v2c0 .55.45 1 1 1h11l4 4V7c0-.55-.45-1-1-1zm-4 6V3c0-.55-.45-1-1-1H3c-.55 0-1 .45-1 1v14l4-4h10c.55 0 1-.45 1-1z",
        false,
    )],
};

/// `forward` (`src/content/forward/materialicons/24px.svg`).
pub const FORWARD: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno("M12 8V4l8 8-8 8v-4H4V8z", false)],
};

/// `forward_10` (`src/av/forward_10/materialicons/24px.svg`): adelantar diez
/// segundos en el reproductor. Son tres figuras: la flecha en circulo, el
/// «1» —que en el SVG es un `<polygon>` y aqui se escribe como trayecto, los
/// mismos puntos con `M` y las lineas implicitas— y el «0» con su hueco.
pub const FORWARD_10: Icono = Icono {
    vista: VISTA,
    trazos: &[
        relleno(
            "M18,13c0,3.31-2.69,6-6,6s-6-2.69-6-6s2.69-6,6-6v4l5-5l-5-5v4c-4.42,0-8,3.58-8,8c0,4.42,3.58,8,8,8s8-3.58,8-8H18z",
            false,
        ),
        relleno(
            "M10.86,15.94 10.86,11.67 10.77,11.67 9,12.3 9,12.99 10.01,12.68 10.01,15.94z",
            false,
        ),
        relleno(
            "M12.25,13.44v0.74c0,1.9,1.31,1.82,1.44,1.82c0.14,0,1.44,0.09,1.44-1.82v-0.74c0-1.9-1.31-1.82-1.44-1.82 C13.55,11.62,12.25,11.53,12.25,13.44z M14.29,13.32v0.97c0,0.77-0.21,1.03-0.59,1.03c-0.38,0-0.6-0.26-0.6-1.03v-0.97 c0-0.75,0.22-1.01,0.59-1.01C14.07,12.3,14.29,12.57,14.29,13.32z",
            false,
        ),
    ],
};

/// `grid_view` (`src/action/grid_view/materialicons/24px.svg`): la vista
/// de Tareas en cuadrados (tareas-v5).
pub const GRID_VIEW: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3 3v8h8V3H3zm6 6H5V5h4v4zm-6 4v8h8v-8H3zm6 6H5v-4h4v4zm4-16v8h8V3h-8zm6 6h-4V5h4v4zm-6 4v8h8v-8h-8zm6 6h-4v-4h4v4z",
        false,
    )],
};

/// La vista Kanban de Tareas (17-oct-2026): tres columnas de alto distinto,
/// como las listas del tablero. Hecho a mano sobre la rejilla de 24.
pub const VIEW_KANBAN: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3 4h5v16H3V4zM9.5 4h5v10h-5V4zM16 4h5v13h-5V4z",
        false,
    )],
};

/// `hearing` (`src/av/hearing/materialicons/24px.svg`).
pub const HEARING: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M17 20c-.29 0-.56-.06-.76-.15-.71-.37-1.21-.88-1.71-2.38-.51-1.56-1.47-2.29-2.39-3-.79-.61-1.61-1.24-2.32-2.53C9.29 10.98 9 9.93 9 9c0-2.8 2.2-5 5-5s5 2.2 5 5h2c0-3.93-3.07-7-7-7S7 5.07 7 9c0 1.26.38 2.65 1.07 3.9.91 1.65 1.98 2.48 2.85 3.15.81.62 1.39 1.07 1.71 2.05.6 1.82 1.37 2.84 2.73 3.55.51.23 1.07.35 1.64.35 2.21 0 4-1.79 4-4h-2c0 1.1-.9 2-2 2zM7.64 2.64L6.22 1.22C4.23 3.21 3 5.96 3 9s1.23 5.79 3.22 7.78l1.41-1.41C6.01 13.74 5 11.49 5 9s1.01-4.74 2.64-6.36zM11.5 9c0 1.38 1.12 2.5 2.5 2.5s2.5-1.12 2.5-2.5-1.12-2.5-2.5-2.5-2.5 1.12-2.5 2.5z",
        false,
    )],
};

/// `image` (`src/image/image/materialicons/24px.svg`).
pub const IMAGE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M21 19V5c0-1.1-.9-2-2-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2zM8.5 13.5l2.5 3.01L14.5 12l4.5 6H5l3.5-4.5z",
        false,
    )],
};

/// `ios_share` (`src/social/ios_share/materialicons/24px.svg`).
/// `file_download` (`src/file/file_download/materialicons/24px.svg`): el
/// «Exportar» del timeline, que no es compartir.
pub const FILE_DOWNLOAD: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno("M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z", false)],
};

pub const IOS_SHARE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M16 5l-1.42 1.42-1.59-1.59V16h-1.98V4.83L9.42 6.42 8 5l4-4 4 4zm4 5v11c0 1.1-.9 2-2 2H6c-1.11 0-2-.9-2-2V10c0-1.11.89-2 2-2h3v2H6v11h12V10h-3V8h3c1.1 0 2 .89 2 2z",
        false,
    )],
};

/// `keyboard_arrow_down` (`src/hardware/keyboard_arrow_down/materialicons/24px.svg`).
pub const KEYBOARD_ARROW_DOWN: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M7.41 8.59L12 13.17l4.59-4.58L18 10l-6 6-6-6 1.41-1.41z",
        false,
    )],
};

/// `launch` (`src/action/launch/materialicons/24px.svg`).
pub const LAUNCH: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M19 19H5V5h7V3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2v-7h-2v7zM14 3v2h3.59l-9.83 9.83 1.41 1.41L19 6.41V10h2V3h-7z",
        false,
    )],
};

/// `library_add` (`src/av/library_add/materialicons/24px.svg`).
pub const LIBRARY_ADD: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M4 6H2v14c0 1.1.9 2 2 2h14v-2H4V6zm16-4H8c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-1 9h-4v4h-2v-4H9V9h4V5h2v4h4v2z",
        false,
    )],
};

/// `library_music` (`src/av/library_music/materialicons/24px.svg`).
pub const LIBRARY_MUSIC: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M20 2H8c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-2 5h-3v5.5c0 1.38-1.12 2.5-2.5 2.5S10 13.88 10 12.5s1.12-2.5 2.5-2.5c.57 0 1.08.19 1.5.51V5h4v2zM4 6H2v14c0 1.1.9 2 2 2h14v-2H4V6z",
        false,
    )],
};

/// `lightbulb` (`src/action/lightbulb/materialicons/24px.svg`). Las
/// lecciones aprendidas, como `Icons.Filled.Lightbulb` en el movil.
pub const LIGHTBULB: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M9 21c0 .5.4 1 1 1h4c.6 0 1-.5 1-1v-1H9v1zm3-19C8.1 2 5 5.1 5 9c0 2.4 1.2 4.5 3 5.7V17c0 .5.4 1 1 1h6c.6 0 1-.5 1-1v-2.3c1.8-1.3 3-3.4 3-5.7 0-3.9-3.1-7-7-7z",
        false,
    )],
};

/// `link` (`src/content/link/materialicons/24px.svg`).
pub const LINK: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1h-4V17h4c2.76 0 5-2.24 5-5s-2.24-5-5-5z",
        false,
    )],
};

/// `list` (`src/action/list/materialicons/24px.svg`).
pub const LIST: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3 13h2v-2H3v2zm0 4h2v-2H3v2zm0-8h2V7H3v2zm4 4h14v-2H7v2zm0 4h14v-2H7v2zM7 7v2h14V7H7z",
        false,
    )],
};

/// `lyrics` (`src/av/lyrics/materialicons/24px.svg`).
pub const LYRICS: Icono = Icono {
    vista: VISTA,
    trazos: &[
        relleno(
            "M14,9c0-2.04,1.24-3.79,3-4.57V4c0-1.1-0.9-2-2-2H4C2.9,2,2.01,2.9,2.01,4L2,22l4-4h9c1.1,0,2-0.9,2-2v-2.42 C15.24,12.8,14,11.05,14,9z M10,14H6v-2h4V14z M13,11H6V9h7V11z M13,8H6V6h7V8z",
            false,
        ),
        relleno(
            "M20,6.18C19.69,6.07,19.35,6,19,6c-1.66,0-3,1.34-3,3c0,1.66,1.34,3,3,3s3-1.34,3-3V3h2V1h-4V6.18z",
            false,
        ),
    ],
};

/// `menu_book` (`src/maps/menu_book/materialicons/24px.svg`).
pub const MENU_BOOK: Icono = Icono {
    vista: VISTA,
    trazos: &[
        relleno(
            "M21,5c-1.11-0.35-2.33-0.5-3.5-0.5c-1.95,0-4.05,0.4-5.5,1.5c-1.45-1.1-3.55-1.5-5.5-1.5S2.45,4.9,1,6v14.65 c0,0.25,0.25,0.5,0.5,0.5c0.1,0,0.15-0.05,0.25-0.05C3.1,20.45,5.05,20,6.5,20c1.95,0,4.05,0.4,5.5,1.5c1.35-0.85,3.8-1.5,5.5-1.5 c1.65,0,3.35,0.3,4.75,1.05c0.1,0.05,0.15,0.05,0.25,0.05c0.25,0,0.5-0.25,0.5-0.5V6C22.4,5.55,21.75,5.25,21,5z M21,18.5 c-1.1-0.35-2.3-0.5-3.5-0.5c-1.7,0-4.15,0.65-5.5,1.5V8c1.35-0.85,3.8-1.5,5.5-1.5c1.2,0,2.4,0.15,3.5,0.5V18.5z",
            false,
        ),
        relleno(
            "M17.5,10.5c0.88,0,1.73,0.09,2.5,0.26V9.24C19.21,9.09,18.36,9,17.5,9c-1.7,0-3.24,0.29-4.5,0.83v1.66 C14.13,10.85,15.7,10.5,17.5,10.5z",
            false,
        ),
        relleno(
            "M13,12.49v1.66c1.13-0.64,2.7-0.99,4.5-0.99c0.88,0,1.73,0.09,2.5,0.26V11.9c-0.79-0.15-1.64-0.24-2.5-0.24 C15.8,11.66,14.26,11.96,13,12.49z",
            false,
        ),
        relleno(
            "M17.5,14.33c-1.7,0-3.24,0.29-4.5,0.83v1.66c1.13-0.64,2.7-0.99,4.5-0.99c0.88,0,1.73,0.09,2.5,0.26v-1.52 C19.21,14.41,18.36,14.33,17.5,14.33z",
            false,
        ),
    ],
};

/// `mic` (`src/av/mic/materialicons/24px.svg`).
pub const MIC: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 14c1.66 0 2.99-1.34 2.99-3L15 5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3zm5.3-3c0 3-2.54 5.1-5.3 5.1S6.7 14 6.7 11H5c0 3.41 2.72 6.23 6 6.72V21h2v-3.28c3.28-.48 6-3.3 6-6.72h-1.7z",
        false,
    )],
};

/// `more_vert` (`src/navigation/more_vert/materialicons/24px.svg`).
pub const MORE_VERT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 8c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm0 2c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0 6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z",
        false,
    )],
};

/// `edit_note` (`src/editor/edit_note/materialicons/24px.svg`): editar el
/// TEXTO (las celdas de una hoja, una nota). El lapiz a secas (`EDIT`) es
/// anotar encima, en todos los visores: no se confunden (19-oct-2026).
pub const EDIT_NOTE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3 10h11v2H3v-2zm0-2h11V6H3v2zm0 8h7v-2H3v2zm15.01-3.13.71-.71c.39-.39 1.02-.39 1.41 0l.71.71c.39.39.39 1.02 0 1.41l-.71.71-2.12-2.12zm-.71.71-5.3 5.3V21h2.12l5.3-5.3-2.12-2.12z",
        false,
    )],
};

/// `crop_free` (`src/image/crop_free/materialicons/24px.svg`): el marco
/// para imprimir de los visores (las cuatro esquinas de un recuadro).
pub const CROP_FREE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3 5v4h2V5h4V3H5c-1.1 0-2 .9-2 2zm2 10H3v4c0 1.1.9 2 2 2h4v-2H5v-4zm14 4h-4v2h4c1.1 0 2-.9 2-2v-4h-2v4zm0-16h-4v2h4v4h2V5c0-1.1-.9-2-2-2z",
        false,
    )],
};

/// `open_in_new` (`src/action/open_in_new/materialicons/24px.svg`).
pub const OPEN_IN_NEW: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M19 19H5V5h7V3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2v-7h-2v7zM14 3v2h3.59l-9.83 9.83 1.41 1.41L19 6.41V10h2V3h-7z",
        false,
    )],
};

/// `palette` (`src/image/palette/materialicons/24px.svg`).
pub const PALETTE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12,2C6.49,2,2,6.49,2,12s4.49,10,10,10c1.38,0,2.5-1.12,2.5-2.5c0-0.61-0.23-1.2-0.64-1.67c-0.08-0.1-0.13-0.21-0.13-0.33 c0-0.28,0.22-0.5,0.5-0.5H16c3.31,0,6-2.69,6-6C22,6.04,17.51,2,12,2z M17.5,13c-0.83,0-1.5-0.67-1.5-1.5c0-0.83,0.67-1.5,1.5-1.5 s1.5,0.67,1.5,1.5C19,12.33,18.33,13,17.5,13z M14.5,9C13.67,9,13,8.33,13,7.5C13,6.67,13.67,6,14.5,6S16,6.67,16,7.5 C16,8.33,15.33,9,14.5,9z M5,11.5C5,10.67,5.67,10,6.5,10S8,10.67,8,11.5C8,12.33,7.33,13,6.5,13S5,12.33,5,11.5z M11,7.5 C11,8.33,10.33,9,9.5,9S8,8.33,8,7.5C8,6.67,8.67,6,9.5,6S11,6.67,11,7.5z",
        false,
    )],
};

/// `pause` (`src/av/pause/materialicons/24px.svg`).
pub const PAUSE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno("M6 19h4V5H6v14zm8-14v14h4V5h-4z", false)],
};

/// `photo_library` (`src/image/photo_library/materialicons/24px.svg`). La
/// galeria de capturas, como `Icons.Filled.PhotoLibrary` en el movil.
pub const PHOTO_LIBRARY: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M22 16V4c0-1.1-.9-2-2-2H8c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2zm-11-4l2.03 2.71L16 11l4 5H8l3-4zM2 6v14c0 1.1.9 2 2 2h14v-2H4V6H2z",
        false,
    )],
};

/// `play_arrow` (`src/av/play_arrow/materialicons/24px.svg`).
pub const PLAY_ARROW: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno("M8 5v14l11-7z", false)],
};

/// `public` (`src/social/public/materialicons/24px.svg`).
pub const PUBLIC: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z",
        false,
    )],
};

/// `push_pin` (`src/content/push_pin/materialicons/24px.svg`).
pub const PUSH_PIN: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M16,9V4l1,0c0.55,0,1-0.45,1-1v0c0-0.55-0.45-1-1-1H7C6.45,2,6,2.45,6,3v0 c0,0.55,0.45,1,1,1l1,0v5c0,1.66-1.34,3-3,3h0v2h5.97v7l1,1l1-1v-7H19v-2h0C17.34,12,16,10.66,16,9z",
        true,
    )],
};

/// `record_voice_over` (`src/action/record_voice_over/materialicons/24px.svg`).
pub const RECORD_VOICE_OVER: Icono = Icono {
    vista: VISTA,
    trazos: &[
        relleno(
            "M9 15c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4zm7.76-9.64l-1.68 1.69c.84 1.18.84 2.71 0 3.89l1.68 1.69c2.02-2.02 2.02-5.07 0-7.27zM20.07 2l-1.63 1.63c2.77 3.02 2.77 7.56 0 10.74L20.07 16c3.9-3.89 3.91-9.95 0-14z",
            false,
        ),
        relleno("M5 9a4 4 0 1 0 8 0a4 4 0 1 0 -8 0z", false),
    ],
};

/// `replay_10` (`src/av/replay_10/materialicons/24px.svg`): retroceder diez
/// segundos. Las mismas tres figuras que `forward_10`, con la flecha al
/// otro lado; aqui el «1» ya viene como trayecto en el SVG.
pub const REPLAY_10: Icono = Icono {
    vista: VISTA,
    trazos: &[
        relleno(
            "M11.99,5V1l-5,5l5,5V7c3.31,0,6,2.69,6,6s-2.69,6-6,6s-6-2.69-6-6h-2c0,4.42,3.58,8,8,8s8-3.58,8-8S16.41,5,11.99,5z",
            false,
        ),
        relleno(
            "M10.89,16h-0.85v-3.26l-1.01,0.31v-0.69l1.77-0.63h0.09V16z",
            false,
        ),
        relleno(
            "M15.17,14.24c0,0.32-0.03,0.6-0.1,0.82s-0.17,0.42-0.29,0.57s-0.28,0.26-0.45,0.33s-0.37,0.1-0.59,0.1 s-0.41-0.03-0.59-0.1s-0.33-0.18-0.46-0.33s-0.23-0.34-0.3-0.57s-0.11-0.5-0.11-0.82V13.5c0-0.32,0.03-0.6,0.1-0.82 s0.17-0.42,0.29-0.57s0.28-0.26,0.45-0.33s0.37-0.1,0.59-0.1s0.41,0.03,0.59,0.1c0.18,0.07,0.33,0.18,0.46,0.33 s0.23,0.34,0.3,0.57s0.11,0.5,0.11,0.82V14.24z M14.32,13.38c0-0.19-0.01-0.35-0.04-0.48s-0.07-0.23-0.12-0.31 s-0.11-0.14-0.19-0.17s-0.16-0.05-0.25-0.05s-0.18,0.02-0.25,0.05s-0.14,0.09-0.19,0.17s-0.09,0.18-0.12,0.31 s-0.04,0.29-0.04,0.48v0.97c0,0.19,0.01,0.35,0.04,0.48s0.07,0.24,0.12,0.32s0.11,0.14,0.19,0.17s0.16,0.05,0.25,0.05 s0.18-0.02,0.25-0.05s0.14-0.09,0.19-0.17s0.09-0.19,0.11-0.32s0.04-0.29,0.04-0.48V13.38z",
            false,
        ),
    ],
};

/// `reply` (`src/content/reply/materialicons/24px.svg`).
pub const REPLY: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M10 9V5l-7 7 7 7v-4.1c5 0 8.5 1.6 11 5.1-1-5-4-10-11-11z",
        false,
    )],
};

/// `settings` (`src/action/settings/materialicons/24px.svg`): el engranaje
/// de la cabecera del visor, al lado del marcador (v0.66 del movil).
pub const SETTINGS: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M19.14,12.94c0.04-0.3,0.06-0.61,0.06-0.94c0-0.32-0.02-0.64-0.07-0.94l2.03-1.58c0.18-0.14,0.23-0.41,0.12-0.61 l-1.92-3.32c-0.12-0.22-0.37-0.29-0.59-0.22l-2.39,0.96c-0.5-0.38-1.03-0.7-1.62-0.94L14.4,2.81c-0.04-0.24-0.24-0.41-0.48-0.41 h-3.84c-0.24,0-0.43,0.17-0.47,0.41L9.25,5.35C8.66,5.59,8.12,5.92,7.63,6.29L5.24,5.33c-0.22-0.08-0.47,0-0.59,0.22L2.74,8.87 C2.62,9.08,2.66,9.34,2.86,9.48l2.03,1.58C4.84,11.36,4.8,11.69,4.8,12s0.02,0.64,0.07,0.94l-2.03,1.58 c-0.18,0.14-0.23,0.41-0.12,0.61l1.92,3.32c0.12,0.22,0.37,0.29,0.59,0.22l2.39-0.96c0.5,0.38,1.03,0.7,1.62,0.94l0.36,2.54 c0.05,0.24,0.24,0.41,0.48,0.41h3.84c0.24,0,0.44-0.17,0.47-0.41l0.36-2.54c0.59-0.24,1.13-0.56,1.62-0.94l2.39,0.96 c0.22,0.08,0.47,0,0.59-0.22l1.92-3.32c0.12-0.22,0.07-0.47-0.12-0.61L19.14,12.94z M12,15.6c-1.98,0-3.6-1.62-3.6-3.6 s1.62-3.6,3.6-3.6s3.6,1.62,3.6,3.6S13.98,15.6,12,15.6z",
        false,
    )],
};

/// `search` (`src/action/search/materialicons/24px.svg`).
pub const SEARCH: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z",
        false,
    )],
};

/// `send` (`src/content/send/materialicons/24px.svg`).
pub const SEND: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno("M2.01 21L23 12 2.01 3 2 10l15 2-15 2z", false)],
};

/// `stop` (`src/av/stop/materialicons/24px.svg`).
pub const STOP: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno("M6 6h12v12H6z", false)],
};

/// `subtitles` (`src/av/subtitles/materialicons/24px.svg`).
pub const SUBTITLES: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zM4 12h4v2H4v-2zm10 6H4v-2h10v2zm6 0h-4v-2h4v2zm0-4H10v-2h10v2z",
        false,
    )],
};

/// `table_chart` (`src/editor/table_chart/materialicons/24px.svg`).
pub const TABLE_CHART: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M10 10.02h5V21h-5zM17 21h3c1.1 0 2-.9 2-2v-9h-5v11zm3-18H5c-1.1 0-2 .9-2 2v3h19V5c0-1.1-.9-2-2-2zM3 19c0 1.1.9 2 2 2h3V10H3v9z",
        false,
    )],
};

/// `view_in_ar` (`src/action/view_in_ar/materialicons/24px.svg`).
pub const VIEW_IN_AR: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M18.25 7.6l-5.5-3.18c-.46-.27-1.04-.27-1.5 0L5.75 7.6c-.46.27-.75.76-.75 1.3v6.35c0 .54.29 1.03.75 1.3l5.5 3.18c.46.27 1.04.27 1.5 0l5.5-3.18c.46-.27.75-.76.75-1.3V8.9c0-.54-.29-1.03-.75-1.3zM7 14.96v-4.62l4 2.32v4.61l-4-2.31zm5-4.03L8 8.61l4-2.31 4 2.31-4 2.32zm1 6.34v-4.61l4-2.32v4.62l-4 2.31zM7 2H3.5C2.67 2 2 2.67 2 3.5V7h2V4h3V2zm10 0h3.5c.83 0 1.5.67 1.5 1.5V7h-2V4h-3V2zM7 22H3.5c-.83 0-1.5-.67-1.5-1.5V17h2v3h3v2zm10 0h3.5c.83 0 1.5-.67 1.5-1.5V17h-2v3h-3v2z",
        false,
    )],
};

/// `wifi` (`src/notification/wifi/materialicons/24px.svg`).
pub const WIFI: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M1 9l2 2c4.97-4.97 13.03-4.97 18 0l2-2C16.93 2.93 7.08 2.93 1 9zm8 8l3 3 3-3c-1.65-1.66-4.34-1.66-6 0zm-4-4l2 2c2.76-2.76 7.24-2.76 10 0l2-2C15.14 9.14 8.87 9.14 5 13z",
        false,
    )],
};

// --- Los de la voz (B7, B9, B11): banderita, llamada, persona, altavoz y
// repetir, los mismos `Icons.Filled.*` que `LetraActivity`,
// `LlamadaSecretaActivity` y `PronunciarActivity`.

/// `flag` (`src/content/flag/materialicons/24px.svg`).
pub const FLAG: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno("M14.4 6L14 4H5v17h2v-7h5.6l.4 2h7V6z", false)],
};

/// `call` (`src/communication/call/materialicons/24px.svg`).
pub const CALL: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M20.01 15.38c-1.23 0-2.42-.2-3.53-.56-.35-.12-.74-.03-1.01.24l-1.57 1.97c-2.83-1.35-5.48-3.9-6.89-6.83l1.95-1.66c.27-.28.35-.67.24-1.02-.37-1.11-.56-2.3-.56-3.53 0-.54-.45-.99-.99-.99H4.19C3.65 3 3 3.24 3 3.99 3 13.28 10.73 21 20.01 21c.71 0 .99-.63.99-1.18v-3.45c0-.54-.45-.99-.99-.99z",
        false,
    )],
};

/// `call_end` (`src/communication/call_end/materialicons/24px.svg`).
pub const CALL_END: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 9c-1.6 0-3.15.25-4.6.72v3.1c0 .39-.23.74-.56.9-.98.49-1.87 1.12-2.66 1.85-.18.18-.43.28-.7.28-.28 0-.53-.11-.71-.29L.29 13.08c-.18-.17-.29-.42-.29-.7 0-.28.11-.53.29-.71C3.34 8.78 7.46 7 12 7s8.66 1.78 11.71 4.67c.18.18.29.43.29.71 0 .28-.11.53-.29.71l-2.48 2.48c-.18.18-.43.29-.71.29-.27 0-.52-.11-.7-.28-.79-.74-1.69-1.36-2.67-1.85-.33-.16-.56-.5-.56-.9v-3.1C15.15 9.25 13.6 9 12 9z",
        false,
    )],
};

/// `person` (`src/social/person/materialicons/24px.svg`).
pub const PERSON: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z",
        false,
    )],
};

/// `volume_up` (`src/av/volume_up/materialicons/24px.svg`).
pub const VOLUME_UP: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3 9v6h4l5 5V4L7 9H3zm13.5 3c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z",
        false,
    )],
};

/// `replay` (`src/av/replay/materialicons/24px.svg`).
pub const REPLAY: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 5V1L7 6l5 5V7c3.31 0 6 2.69 6 6s-2.69 6-6 6-6-2.69-6-6H4c0 4.42 3.58 8 8 8s8-3.58 8-8-3.58-8-8-8z",
        false,
    )],
};

/// `undo` (`src/content/undo/materialicons/24px.svg`): «Deshacer» de una
/// tarea quitada en la ventana de tareas.
pub const UNDO: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12.5 8c-2.65 0-5.05.99-6.9 2.6L2 7v9h9l-3.62-3.62c1.39-1.16 3.16-1.88 5.12-1.88 3.54 0 6.55 2.31 7.6 5.5l2.37-.78C21.08 11.03 17.15 8 12.5 8z",
        false,
    )],
};

/// `keyboard_arrow_up` (`src/hardware/keyboard_arrow_up/materialicons/24px.svg`):
/// subir una tarea en su lista.
pub const KEYBOARD_ARROW_UP: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M7.41 15.41L12 10.83l4.59 4.58L18 14l-6-6-6 6z",
        false,
    )],
};

/// `calendar_month` (`src/action/calendar_month/materialicons/24px.svg`):
/// el calendario del timeline.
pub const CALENDAR_MONTH: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M19 4h-1V2h-2v2H8V2H6v2H5c-1.11 0-1.99.9-1.99 2L3 20c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 16H5V10h14v10zM9 14H7v-2h2v2zm4 0h-2v-2h2v2zm4 0h-2v-2h2v2zm-8 4H7v-2h2v2zm4 0h-2v-2h2v2zm4 0h-2v-2h2v2z",
        false,
    )],
};

/// `timeline` (`src/action/timeline/materialicons/24px.svg`): el boton y la
/// entrada de la bandeja del timeline.
pub const TIMELINE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M23 8c0 1.1-.9 2-2 2-.18 0-.35-.02-.51-.07l-3.56 3.55c.05.16.07.34.07.52 0 1.1-.9 2-2 2s-2-.9-2-2c0-.18.02-.36.07-.52l-2.55-2.55c-.16.05-.34.07-.52.07s-.36-.02-.52-.07l-4.55 4.56c.05.16.07.33.07.51 0 1.1-.9 2-2 2s-2-.9-2-2 .9-2 2-2c.18 0 .35.02.51.07l4.56-4.55C8.02 9.36 8 9.18 8 9c0-1.1.9-2 2-2s2 .9 2 2c0 .18-.02.36-.07.52l2.55 2.55c.16-.05.34-.07.52-.07s.36.02.52.07l3.55-3.56C19.02 8.35 19 8.18 19 8c0-1.1.9-2 2-2s2 .9 2 2z",
        false,
    )],
};

/// `chevron_left` y `chevron_right` (`src/navigation/…/materialicons/24px.svg`):
/// el mes anterior y el siguiente del calendario del timeline.
pub const CHEVRON_LEFT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M15.41 7.41L14 6l-6 6 6 6 1.41-1.41L10.83 12z",
        false,
    )],
};

pub const CHEVRON_RIGHT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M10 6L8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z",
        false,
    )],
};

/// `redo` (`src/content/redo/materialicons/24px.svg`): «Rehacer» en la hoja
/// del pin.
pub const REDO: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M18.4 10.6C16.55 8.99 14.15 8 11.5 8c-4.65 0-8.58 3.03-9.96 7.22L3.9 16c1.05-3.19 4.05-5.5 7.6-5.5 1.95 0 3.73.72 5.12 1.88L13 16h9V7l-3.6 3.6z",
        false,
    )],
};

/// `visibility` (`src/action/visibility/materialicons/24px.svg`): «Ver», de
/// vuelta del modo editar de la hoja del pin.
pub const VISIBILITY: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z",
        false,
    )],
};

/// `format_bold` (`src/editor/format_bold/materialicons/24px.svg`): la
/// negrita de la hoja del pin.
pub const FORMAT_BOLD: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M15.6 10.79c.97-.67 1.65-1.77 1.65-2.79 0-2.26-1.75-4-4-4H7v14h7.04c2.09 0 3.71-1.7 3.71-3.79 0-1.52-.86-2.82-2.15-3.42zM10 6.5h3c.83 0 1.5.67 1.5 1.5s-.67 1.5-1.5 1.5h-3v-3zm3.5 9H10v-3h3.5c.83 0 1.5.67 1.5 1.5s-.67 1.5-1.5 1.5z",
        false,
    )],
};

/// `format_italic` (`src/editor/format_italic/materialicons/24px.svg`): la
/// cursiva de la hoja del pin.
pub const FORMAT_ITALIC: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M10 4v3h2.21l-3.42 8H6v3h8v-3h-2.21l3.42-8H18V4z",
        false,
    )],
};

/// `format_underlined` (`src/editor/format_underlined/materialicons/24px.svg`).
pub const FORMAT_UNDERLINED: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 17c3.31 0 6-2.69 6-6V3h-2.5v8c0 1.93-1.57 3.5-3.5 3.5S8.5 12.93 8.5 11V3H6v8c0 3.31 2.69 6 6 6zm-7 2v2h14v-2H5z",
        false,
    )],
};

/// `format_strikethrough` (`src/editor/format_strikethrough/materialicons/24px.svg`).
pub const FORMAT_STRIKETHROUGH: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M10 19h4v-3h-4v3zM5 4v3h5v3h4V7h5V4H5zM3 14h18v-2H3v2z",
        false,
    )],
};

/// `format_color_fill` (`src/editor/format_color_fill/materialicons/24px.svg`):
/// resaltar (el fondo de la celda).
pub const FORMAT_COLOR_FILL: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M16.56 8.94L7.62 0 6.21 1.41l2.38 2.38-5.15 5.15c-.59.59-.59 1.54 0 2.12l5.5 5.5c.29.29.68.44 1.06.44s.77-.15 1.06-.44l5.5-5.5c.59-.58.59-1.53 0-2.12zM5.21 10L10 5.21 14.79 10H5.21zM19 11.5s-2 2.17-2 3.5c0 1.1.9 2 2 2s2-.9 2-2c0-1.33-2-3.5-2-3.5zM2 20h20v4H2v-4z",
        false,
    )],
};

/// `dark_mode` (`src/device/dark_mode/materialicons/24px.svg`): la luna de
/// la barra de los pines-lector, para pasar los visores a oscuro.
pub const DARK_MODE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12,3c-4.97,0-9,4.03-9,9s4.03,9,9,9s9-4.03,9-9c0-0.46-0.04-0.92-0.1-1.36c-0.98,1.37-2.58,2.26-4.4,2.26 c-2.98,0-5.4-2.42-5.4-5.4c0-1.81,0.89-3.42,2.26-4.4C12.92,3.04,12.46,3,12,3L12,3z",
        false,
    )],
};

/// `format_color_text` (`src/editor/format_color_text/materialicons/24px.svg`):
/// el color de la letra.
pub const FORMAT_COLOR_TEXT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M2 20h20v4H2v-4zm3.49-3h2.42l1.27-3.58h5.65L16.09 17h2.42L13.25 3h-2.5L5.49 17zm4.42-5.61l2.03-5.79h.12l2.03 5.79H9.91z",
        false,
    )],
};

/// `format_align_left` (`src/editor/format_align_left/materialicons/24px.svg`).
pub const FORMAT_ALIGN_LEFT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M15 15H3v2h12v-2zm0-8H3v2h12V7zM3 13h18v-2H3v2zm0 8h18v-2H3v2zM3 3v2h18V3H3z",
        false,
    )],
};

/// `format_align_center` (`src/editor/format_align_center/materialicons/24px.svg`).
pub const FORMAT_ALIGN_CENTER: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M7 15v2h10v-2H7zm-4 6h18v-2H3v2zm0-8h18v-2H3v2zm4-6v2h10V7H7zM3 3v2h18V3H3z",
        false,
    )],
};

/// `format_align_right` (`src/editor/format_align_right/materialicons/24px.svg`).
pub const FORMAT_ALIGN_RIGHT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3 21h18v-2H3v2zm6-4h12v-2H9v2zm-6-4h18v-2H3v2zm6-4h12V7H9v2zM3 3v2h18V3H3z",
        false,
    )],
};

/// `wrap_text` (`src/editor/wrap_text/materialicons/24px.svg`): ajustar el
/// texto a la celda.
pub const WRAP_TEXT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M4 19h6v-2H4v2zM20 5H4v2h16V5zm-3 6H4v2h13.25c1.1 0 2 .9 2 2s-.9 2-2 2H15v-2l-3 3 3 3v-2h2c2.21 0 4-1.79 4-4s-1.79-4-4-4z",
        false,
    )],
};

/// `format_clear` (`src/editor/format_clear/materialicons/24px.svg`): borrar
/// el formato.
pub const FORMAT_CLEAR: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3.27 5L2 6.27l6.97 6.97L6.5 19h3l1.57-3.66L16.73 21 18 19.73 3.55 5.27 3.27 5zM6 5v.18L8.82 8h2.4l-.72 1.68 2.1 2.1L14.21 8H20V5H6z",
        false,
    )],
};

/// `numbers` (`src/editor/numbers/materialicons/24px.svg`): el formato del
/// numero (general, moneda, porcentaje, fecha).
pub const NUMBERS: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M20.5 10l.5-2h-4l1-4h-2l-1 4h-4l1-4h-2L9 8H5l-.5 2h4l-1 4h-4L3 16h4l-1 4h2l1-4h4l-1 4h2l1-4h4l.5-2h-4l1-4h4zm-7 4h-4l1-4h4l-1 4z",
        false,
    )],
};

/// `light_mode` (`src/device/light_mode/materialicons/24px.svg`): el sol,
/// para volver a claro.
pub const LIGHT_MODE: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12,7c-2.76,0-5,2.24-5,5s2.24,5,5,5s5-2.24,5-5S14.76,7,12,7L12,7z M2,13l2,0c0.55,0,1-0.45,1-1s-0.45-1-1-1l-2,0 c-0.55,0-1,0.45-1,1S1.45,13,2,13z M20,13l2,0c0.55,0,1-0.45,1-1s-0.45-1-1-1l-2,0c-0.55,0-1,0.45-1,1S19.45,13,20,13z M11,2v2 c0,0.55,0.45,1,1,1s1-0.45,1-1V2c0-0.55-0.45-1-1-1S11,1.45,11,2z M11,20v2c0,0.55,0.45,1,1,1s1-0.45,1-1v-2c0-0.55-0.45-1-1-1 C11.45,19,11,19.45,11,20z M5.99,4.58c-0.39-0.39-1.03-0.39-1.41,0c-0.39,0.39-0.39,1.03,0,1.41l1.06,1.06 c0.39,0.39,1.03,0.39,1.41,0s0.39-1.03,0-1.41L5.99,4.58z M18.36,16.95c-0.39-0.39-1.03-0.39-1.41,0c-0.39,0.39-0.39,1.03,0,1.41 l1.06,1.06c0.39,0.39,1.03,0.39,1.41,0c0.39-0.39,0.39-1.03,0-1.41L18.36,16.95z M19.42,5.99c0.39-0.39,0.39-1.03,0-1.41 c-0.39-0.39-1.03-0.39-1.41,0l-1.06,1.06c-0.39,0.39-0.39,1.03,0,1.41s1.03,0.39,1.41,0L19.42,5.99z M7.05,18.36 c0.39-0.39,0.39-1.03,0-1.41c-0.39-0.39-1.03-0.39-1.41,0l-1.06,1.06c-0.39,0.39-0.39,1.03,0,1.41s1.03,0.39,1.41,0L7.05,18.36z",
        false,
    )],
};

/// Todos, para poder comprobarlos de una vez.
pub const TODOS: &[Icono] = &[
    EDIT_NOTE,
    CROP_FREE,
    MORE_VERT,
    STRAIGHTEN,
    LAYERS,
    CONTENT_CUT,
    FONT_DOWNLOAD,
    FORMAT_ITALIC,
    FORMAT_UNDERLINED,
    FORMAT_STRIKETHROUGH,
    FORMAT_COLOR_FILL,
    FORMAT_COLOR_TEXT,
    FORMAT_ALIGN_LEFT,
    FORMAT_ALIGN_CENTER,
    FORMAT_ALIGN_RIGHT,
    WRAP_TEXT,
    FORMAT_CLEAR,
    NUMBERS,
    DARK_MODE,
    LIGHT_MODE,
    FULLSCREEN,
    FULLSCREEN_EXIT,
    DRAG_INDICATOR,
    ADD,
    GRID_VIEW,
    CALENDAR_MONTH,
    TIMELINE,
    CHEVRON_LEFT,
    CHEVRON_RIGHT,
    LIGHTBULB,
    PHOTO_LIBRARY,
    FLAG,
    CALL,
    CALL_END,
    PERSON,
    VOLUME_UP,
    REPLAY,
    UNDO,
    REDO,
    VISIBILITY,
    FORMAT_BOLD,
    ALARM,
    ARROW_BACK,
    ARROW_FORWARD,
    ATTACH_FILE,
    BOOKMARK_ADD,
    BOOKMARK_BORDER,
    CHECK_BOX,
    CHECK_BOX_OUTLINE_BLANK,
    CHECK_CIRCLE,
    CHECKLIST,
    CLOSE,
    COMPRESS,
    CONTENT_COPY,
    CONTENT_PASTE,
    CROP,
    DELETE,
    DESCRIPTION,
    DRAW,
    EDIT,
    EMOJI_EMOTIONS,
    FOLDER,
    FORUM,
    FORWARD,
    FORWARD_10,
    HEARING,
    IMAGE,
    FILE_DOWNLOAD,
    IOS_SHARE,
    KEYBOARD_ARROW_DOWN,
    KEYBOARD_ARROW_UP,
    LAUNCH,
    LIBRARY_ADD,
    LIBRARY_MUSIC,
    LINK,
    LIST,
    LYRICS,
    MENU_BOOK,
    MIC,
    MORE_VERT,
    OPEN_IN_NEW,
    PALETTE,
    PAUSE,
    PLAY_ARROW,
    PUBLIC,
    PUSH_PIN,
    RECORD_VOICE_OVER,
    REPLAY_10,
    REPLY,
    SEARCH,
    SEND,
    SETTINGS,
    STOP,
    SUBTITLES,
    TABLE_CHART,
    VIEW_IN_AR,
    WIFI,
];

/// `fullscreen` (`src/navigation/fullscreen/materialicons/24px.svg`): los
/// lectores, de pin a pantalla completa.
pub const FULLSCREEN: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M7 14H5v5h5v-2H7v-3zm-2-4h2V7h3V5H5v5zm12 7h-3v2h5v-5h-2v3zM14 5v2h3v3h2V5h-5z",
        false,
    )],
};

/// `fullscreen_exit` (`src/navigation/fullscreen_exit/materialicons/24px.svg`).
pub const FULLSCREEN_EXIT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M5 16h3v3h2v-5H5v2zm3-8H5v2h5V5H8v3zm6 11h2v-3h3v-2h-5v5zm2-11V5h-2v5h5V8h-3z",
        false,
    )],
};

/// `drag_indicator` (`src/action/drag_indicator/materialicons/24px.svg`): el
/// asa de la barra de un pin, de donde se arrastra la ventana.
pub const DRAG_INDICATOR: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M11 18c0 1.1-.9 2-2 2s-2-.9-2-2 .9-2 2-2 2 .9 2 2zm-2-8c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0-6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm6 4c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm0 2c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0 6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z",
        false,
    )],
};

/// `straighten` (`src/image/straighten/materialicons/24px.svg`): la regla
/// de acotar en el visor de planos.
pub const STRAIGHTEN: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M21 6H3c-1.1 0-2 .9-2 2v8c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm0 10H3V8h2v4h2V8h2v4h2V8h2v4h2V8h2v4h2V8h2v8z",
        false,
    )],
};

/// `layers` (`src/maps/layers/materialicons/24px.svg`): los niveles y las
/// categorias del visor de modelos 3D.
pub const LAYERS: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M11.99 18.54l-7.37-5.73L3 14.07l9 7 9-7-1.63-1.27-7.38 5.74zM12 16l7.36-5.73L21 9l-9-7-9 7 1.63 1.27L12 16z",
        false,
    )],
};

/// `content_cut` (`src/content/content_cut/materialicons/24px.svg`): la
/// caja de seccion del visor de modelos 3D (los `<circle>` del original son
/// `fill="none"`, no pintan).
pub const CONTENT_CUT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M9.64 7.64c.23-.5.36-1.05.36-1.64 0-2.21-1.79-4-4-4S2 3.79 2 6s1.79 4 4 4c.59 0 1.14-.13 1.64-.36L10 12l-2.36 2.36C7.14 14.13 6.59 14 6 14c-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4c0-.59-.13-1.14-.36-1.64L12 14l7 7h3v-1L9.64 7.64zM6 8c-1.1 0-2-.89-2-2s.9-2 2-2 2 .89 2 2-.9 2-2 2zm0 12c-1.1 0-2-.89-2-2s.9-2 2-2 2 .89 2 2-.9 2-2 2zm6-7.5c-.28 0-.5-.22-.5-.5s.22-.5.5-.5.5.22.5.5-.22.5-.5.5zM19 3l-6 6 2 2 7-7V3z",
        false,
    )],
};

/// `font_download` (`src/content/font_download/materialicons/24px.svg`): el
/// boton de las letras (fuentes) del visor de planos.
pub const FONT_DOWNLOAD: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M9.93 13.5h4.14L12 7.98zM20 2H4c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-4.05 16.5l-1.14-3H9.17l-1.12 3H5.96l5.11-13h1.86l5.11 13h-2.09z",
        false,
    )],
};

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::trayecto_svg::analizar;

    #[test]
    fn todos_los_trazados_de_material_se_leen_y_dibujan_algo() {
        // Un trazado que no se entiende no pinta nada, y el boton se quedaria
        // en blanco sin que ninguna otra cosa lo avisara.
        for icono in TODOS {
            assert!(!icono.trazos.is_empty());
            for t in icono.trazos {
                let tramos = analizar(t.d).unwrap_or_else(|e| panic!("{:?}: {e:?}", t.d));
                assert!(tramos.len() >= 2, "{}", t.d);
            }
        }
    }

    #[test]
    fn todos_caben_en_su_caja_de_veinticuatro() {
        // Caso negativo: un trazado copiado a otra escala (el de 48 px, por
        // ejemplo) se saldria de la caja y el icono saldria recortado.
        use crate::trayecto_svg::Tramo;
        for icono in TODOS {
            for t in icono.trazos {
                for tramo in analizar(t.d).unwrap() {
                    let puntos = match tramo {
                        Tramo::Mover(p) | Tramo::Linea(p) => vec![p],
                        Tramo::Cubica { c1, c2, fin } => vec![c1, c2, fin],
                        Tramo::Cerrar => vec![],
                    };
                    for (x, y) in puntos {
                        assert!(
                            (-0.5..=24.5).contains(&x) && (-0.5..=24.5).contains(&y),
                            "{} sale a ({x}, {y})",
                            t.d
                        );
                    }
                }
            }
        }
    }
}
