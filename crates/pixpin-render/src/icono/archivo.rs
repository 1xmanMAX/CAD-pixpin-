//! **El icono de un archivo: una hoja de su color con la esquina doblada y
//! la extension escrita** (`ui/IconoDeArchivo.kt` del movil, v0.98.4), como
//! los de Telegram (`media_doc_*`). El color lo decide quien llama (la tabla
//! `ColorDeExtension` vive en `pixpin-ui`); aqui solo la forma, copiada
//! medida a medida del `Canvas` del movil:
//!
//! - la hoja ocupa la caja entera; esquinas redondeadas de radio
//!   `0,14 * ancho` (curvas cuadraticas, como `quadraticTo`) salvo la de
//!   arriba a la derecha, cortada en diagonal a `0,3 * ancho`;
//! - el doblez es un triangulo blanco al 38 % con la puntita redondeada
//!   (`r * 0,6`);
//! - el rotulo, en semi-negrita, a `0,3 * lado` con tres letras o menos y a
//!   `0,25 * lado` con cuatro, centrado y bajado `0,16 * lado` de relleno
//!   arriba (o sea, su centro `0,08 * lado` por debajo del de la hoja).
//!
//! La forma se aplana a un poligono y se rellena con `Pintor::poligono`:
//! seis tramos por curva bastan a 44 px y evitan otra geometria cacheada.

use crate::lienzo::{Pintor, RectF};
use crate::motor::Color;

/// Lo que mide la esquina redondeada respecto al ancho (`r = w * 0.14f`).
pub const RADIO: f32 = 0.14;
/// Lo que mide la esquina doblada respecto al ancho (`pliegue = w * 0.3f`).
pub const PLIEGUE: f32 = 0.3;
/// El blanco del doblez (`Color.White.copy(alpha = 0.38f)`).
pub const ALFA_DEL_DOBLEZ: f32 = 0.38;
/// La letra oscura que va sobre los colores claros (`0xFF3A2B00`).
pub const TEXTO_OSCURO: Color = Color {
    r: 0x3A as f32 / 255.0,
    g: 0x2B as f32 / 255.0,
    b: 0.0,
    a: 1.0,
};
/// La familia del rotulo: el `FontWeight.SemiBold` del movil.
pub const LETRA_DEL_ROTULO: &str = "Segoe UI Semibold";

/// Tramos con que se aplana cada curva.
const TRAMOS: usize = 6;

/// Una cuadratica de `a` a `b` con control `c`, sin el primer punto.
fn curva(v: &mut Vec<(f32, f32)>, a: (f32, f32), c: (f32, f32), b: (f32, f32)) {
    for k in 1..=TRAMOS {
        let t = k as f32 / TRAMOS as f32;
        let u = 1.0 - t;
        v.push((
            u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
            u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
        ));
    }
}

/// El contorno de la hoja dentro de `caja`, en el orden del `Path` del movil.
pub fn contorno_hoja(caja: RectF) -> Vec<(f32, f32)> {
    let (x, y, w, h) = (caja.x, caja.y, caja.ancho, caja.alto);
    let r = w * RADIO;
    let pliegue = w * PLIEGUE;
    let mut v = Vec::with_capacity(6 + 3 * TRAMOS);
    v.push((x + r, y));
    v.push((x + w - pliegue, y));
    v.push((x + w, y + pliegue));
    v.push((x + w, y + h - r));
    curva(
        &mut v,
        (x + w, y + h - r),
        (x + w, y + h),
        (x + w - r, y + h),
    );
    v.push((x + r, y + h));
    curva(&mut v, (x + r, y + h), (x, y + h), (x, y + h - r));
    v.push((x, y + r));
    curva(&mut v, (x, y + r), (x, y), (x + r, y));
    // El ultimo punto repite el primero: el poligono ya se cierra solo.
    v.pop();
    v
}

/// El doblez de la esquina de arriba a la derecha.
pub fn contorno_doblez(caja: RectF) -> Vec<(f32, f32)> {
    let (x, y, w) = (caja.x, caja.y, caja.ancho);
    let r = w * RADIO;
    let pliegue = w * PLIEGUE;
    let mut v = Vec::with_capacity(3 + TRAMOS);
    v.push((x + w - pliegue, y));
    v.push((x + w - pliegue, y + pliegue - r * 0.6));
    curva(
        &mut v,
        (x + w - pliegue, y + pliegue - r * 0.6),
        (x + w - pliegue, y + pliegue),
        (x + w - pliegue + r * 0.6, y + pliegue),
    );
    v.push((x + w, y + pliegue));
    v
}

/// El tamano de la letra del rotulo para un icono de `lado`.
pub fn tam_del_rotulo(lado: f32, rotulo: &str) -> f32 {
    if rotulo.chars().count() <= 3 {
        lado * 0.3
    } else {
        lado * 0.25
    }
}

/// Donde va el centro del rotulo: el de la hoja, bajado `0,08 * lado`.
pub fn centro_del_rotulo(caja: RectF) -> (f32, f32) {
    (
        caja.x + caja.ancho / 2.0,
        caja.y + caja.alto / 2.0 + caja.alto * 0.08,
    )
}

impl Pintor<'_> {
    /// **Pinta el icono de un archivo** en `caja`: la hoja de `color`, su
    /// doblez y `rotulo` (la extension, ya cortada) en `texto`.
    pub fn icono_de_archivo(&self, caja: RectF, color: Color, rotulo: &str, texto: Color) {
        self.poligono(&contorno_hoja(caja), color);
        self.poligono(
            &contorno_doblez(caja),
            Color {
                a: ALFA_DEL_DOBLEZ,
                ..Color::BLANCO
            },
        );
        if rotulo.is_empty() {
            return;
        }
        let letra = crate::letras::Letra::de(LETRA_DEL_ROTULO);
        let tam = tam_del_rotulo(caja.alto, rotulo);
        let (w, h) = self.medir_con_letra(rotulo, tam, crate::letras::SIN_PARTIR, &letra);
        let (cx, cy) = centro_del_rotulo(caja);
        self.texto_con_letra(
            rotulo,
            cx - w / 2.0,
            cy - h / 2.0,
            tam,
            crate::letras::SIN_PARTIR,
            &letra,
            texto,
        );
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const CAJA: RectF = RectF {
        x: 10.0,
        y: 20.0,
        ancho: 44.0,
        alto: 44.0,
    };

    #[test]
    fn la_hoja_no_se_sale_de_su_caja_y_tiene_la_esquina_cortada() {
        let v = contorno_hoja(CAJA);
        for &(x, y) in &v {
            assert!(
                (10.0..=54.0).contains(&x) && (20.0..=64.0).contains(&y),
                "{x},{y}"
            );
        }
        // La esquina de arriba a la derecha NO esta: se corta en diagonal
        // de (54 - 13,2, 20) a (54, 20 + 13,2).
        assert!(!v.contains(&(54.0, 20.0)));
        assert!(v.contains(&(54.0 - 44.0 * PLIEGUE, 20.0)));
        assert!(v.contains(&(54.0, 20.0 + 44.0 * PLIEGUE)));
        // Las otras tres, redondeadas: la de abajo a la derecha pasa cerca
        // de la esquina sin tocarla.
        assert!(!v.contains(&(54.0, 64.0)));
        assert!(!v.contains(&(10.0, 64.0)));
        assert!(!v.contains(&(10.0, 20.0)));
    }

    #[test]
    fn el_doblez_cabe_en_la_esquina_cortada() {
        let d = contorno_doblez(CAJA);
        let p = 44.0 * PLIEGUE;
        for &(x, y) in &d {
            assert!(x >= 54.0 - p - 0.01 && y <= 20.0 + p + 0.01, "{x},{y}");
        }
        assert_eq!(d.first(), Some(&(54.0 - p, 20.0)));
        assert_eq!(d.last(), Some(&(54.0, 20.0 + p)));
    }

    #[test]
    fn la_letra_encoge_con_cuatro_letras_y_baja_un_poco() {
        assert!((tam_del_rotulo(44.0, "pdf") - 13.2).abs() < 1e-4);
        assert!((tam_del_rotulo(44.0, "xlsx") - 11.0).abs() < 1e-4);
        assert!((tam_del_rotulo(44.0, "") - 13.2).abs() < 1e-4);
        let (cx, cy) = centro_del_rotulo(CAJA);
        assert!((cx - 32.0).abs() < 1e-4);
        assert!(cy > 42.0, "por debajo del centro: {cy}");
    }
}
