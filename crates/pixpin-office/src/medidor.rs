//! **Medir texto sin saber de Windows.** La maquetacion pregunta aqui
//! cuanto avanza cada caracter y cuanto sube y baja una letra; quien
//! contesta de verdad es DirectWrite (`pixpin-render::pagina_office`), y en
//! las pruebas un medidor de ancho fijo que hace los resultados exactos.

use crate::pagina::Letra;

/// Lo vertical de una letra a su tamano, en puntos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metricas {
    /// De la linea base hacia arriba.
    pub ascenso: f32,
    /// De la linea base hacia abajo (positivo).
    pub descenso: f32,
    /// El hueco entre renglones que la letra pide ademas (`lineGap`).
    pub hueco: f32,
}

impl Metricas {
    /// El alto de un renglon a interlineado sencillo.
    pub fn renglon(&self) -> f32 {
        self.ascenso + self.descenso + self.hueco
    }
}

/// Quien sabe medir texto.
pub trait Medidor {
    /// Anade a `salida` el avance de cada `char` de `texto` (uno por char,
    /// en puntos) con la letra dada. Sin interletrado: eso lo suma quien
    /// maqueta.
    fn avances(&self, texto: &str, letra: &Letra, salida: &mut Vec<f32>);

    /// Ascenso, descenso y hueco de la letra a su tamano.
    fn metricas(&self, letra: &Letra) -> Metricas;

    /// Si la letra esta instalada (para elegir una alternativa si no).
    fn existe(&self, _familia: &str) -> bool {
        true
    }

    /// El ancho de un texto entero.
    fn ancho(&self, texto: &str, letra: &Letra) -> f32 {
        let mut v = Vec::with_capacity(texto.len());
        self.avances(texto, letra, &mut v);
        v.iter().sum()
    }
}

/// Un medidor de mentira: cada caracter mide `proporcion * tam` (el espacio
/// tambien), sube `0.8 * tam` y baja `0.2 * tam`. Hace que las pruebas de
/// maquetacion den numeros redondos.
#[derive(Debug, Clone, Copy)]
pub struct MedidorFijo {
    pub proporcion: f32,
}

impl Default for MedidorFijo {
    fn default() -> Self {
        MedidorFijo { proporcion: 0.5 }
    }
}

impl Medidor for MedidorFijo {
    fn avances(&self, texto: &str, letra: &Letra, salida: &mut Vec<f32>) {
        salida.extend(texto.chars().map(|_| self.proporcion * letra.tam));
    }

    fn metricas(&self, letra: &Letra) -> Metricas {
        Metricas {
            ascenso: 0.8 * letra.tam,
            descenso: 0.2 * letra.tam,
            hueco: 0.0,
        }
    }
}
