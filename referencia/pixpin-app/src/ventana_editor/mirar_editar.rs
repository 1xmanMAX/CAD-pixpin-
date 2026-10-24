//! **Mirar y editar** (24-oct-2026, el usuario: «cuando entro al canvas
//! siempre entre en modo visualizacion y con el lapiz de la barra lateral
//! pueda activar el modo edicion; en el modo edicion que todos los botones
//! desaparezcan y que sea pantalla completa a excepcion de la barra
//! lateral»).
//!
//! - El lienzo abre siempre **mirando**: arrastrar mueve el dibujo y nada se
//!   elige ni se edita. El riel de la derecha sigue a la vista, con las
//!   marcas y el lapiz.
//! - El **lapiz del riel** (o Alt+R) pasa a editar **donde se este**
//!   (25-oct-2026): en ventana, en ventana; a pantalla completa, a pantalla
//!   completa, y entonces sin la barra del pin. La pantalla completa la
//!   pone y la quita su boton (o F11), no el lapiz.
//! - El lapiz otra vez (o Escape) vuelve a mirar, sin tocar la ventana.
//!
//! Aqui solo lo que se decide sin ventana, para probarlo.

/// **Escape editando** vuelve a mirar si no hay nada a medias (un texto o
/// un arrastre los suelta antes el motor) ni se presenta; en el anotador de
/// pantalla no, que tiene su regla.
pub(super) fn escape_vuelve_a_mirar(
    editando: bool,
    es_lienzo: bool,
    sin_presentar: bool,
    gesto_en_reposo: bool,
) -> bool {
    editando && es_lienzo && sin_presentar && gesto_en_reposo
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn escape_editando_vuelve_a_mirar_solo_sin_nada_a_medias() {
        assert!(escape_vuelve_a_mirar(true, true, true, true));
        assert!(
            !escape_vuelve_a_mirar(false, true, true, true),
            "mirando sigue lo de siempre"
        );
        assert!(
            !escape_vuelve_a_mirar(true, true, true, false),
            "un arrastre a medias lo cancela el motor"
        );
        assert!(
            !escape_vuelve_a_mirar(true, true, false, true),
            "presentando, deja de presentar"
        );
        assert!(
            !escape_vuelve_a_mirar(true, false, true, true),
            "el anotador de pantalla tiene su regla"
        );
    }
}
