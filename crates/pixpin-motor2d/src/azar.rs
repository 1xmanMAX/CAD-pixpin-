//! El azar reproducible del trazo "hecho a mano".
//!
//! Un dibujo a mano alzada necesita aleatoriedad —una linea recta perfecta no
//! parece dibujada—, pero esa aleatoriedad tiene que ser **exactamente la
//! misma** cada vez que se abre el documento. Si no, el dibujo cambia de
//! aspecto al reabrirlo y deja de ser el dibujo de nadie.
//!
//! La solucion es la clasica: cada elemento guarda su semilla, y su geometria
//! se regenera desde ella. Dos reglas que hay que respetar al pie de la letra:
//!
//! 1. **Un generador por elemento**, sembrado con su semilla. Compartir un
//!    generador entre elementos hace que el aspecto de uno dependa de cuantos
//!    se dibujaron antes, que es justo lo que se quiere evitar.
//! 2. **La formula, literal.** Es el LCG de Lehmer/Park-Miller con
//!    multiplicador 48271, pero con una particularidad: enmascara con
//!    `& 0x7FFF_FFFF` en vez de aplicar el modulo `2^31 - 1`, y normaliza
//!    dividiendo por `2^31`. Un "LCG equivalente" bien escrito daria una
//!    secuencia DISTINTA y dibujos distintos. El proyecto Android replica
//!    esta misma formula; asi los tres se ven igual.

/// Generador congruencial de Lehmer, replicado bit a bit.
#[derive(Debug, Clone)]
pub struct Azar {
    semilla: u32,
}

impl Azar {
    /// Con semilla 0 el generador original degeneraba en aleatoriedad real.
    /// Aqui se sustituye por 1, que es ademas el valor que el original asigna
    /// al restaurar un elemento antiguo sin semilla: un documento nunca puede
    /// dibujarse distinto en dos aperturas, ni siquiera uno corrupto.
    pub fn nuevo(semilla: u32) -> Self {
        Self {
            semilla: if semilla == 0 { 1 } else { semilla },
        }
    }

    /// El siguiente valor en `[0, 1)`.
    pub fn siguiente(&mut self) -> f32 {
        // wrapping_mul reproduce la multiplicacion truncada a 32 bits del
        // original; el AND con 0x7FFF_FFFF (no el modulo) y la division por
        // 2^31 (no por 2^31-1) son igual de deliberados.
        self.semilla = self.semilla.wrapping_mul(48271) & 0x7FFF_FFFF;
        self.semilla as f32 / 2_147_483_648.0
    }

    /// El mismo paso que [`Self::siguiente`], en doble precision: el `Double`
    /// del `Rand` del movil y de rough.js. Lo usa `rough`, que copia el
    /// generador del movil numero a numero; en `f32` el valor perderia siete
    /// bits y las figuras se moverian centesimas de pixel respecto de alla.
    pub fn siguiente_f64(&mut self) -> f64 {
        self.semilla = self.semilla.wrapping_mul(48271) & 0x7FFF_FFFF;
        self.semilla as f64 / 2_147_483_648.0
    }

    /// Un valor en `[-mitad, +mitad)`: el desplazamiento tipico de un punto
    /// "dibujado a mano".
    pub fn desvio(&mut self, mitad: f32) -> f32 {
        (self.siguiente() * 2.0 - 1.0) * mitad
    }

    /// Una semilla nueva a partir de esta, para el elemento que se cree
    /// ahora. Asi el documento entero sigue siendo reproducible desde una
    /// sola raiz, y no hace falta un generador global de proceso.
    pub fn semilla_derivada(&mut self) -> u32 {
        self.semilla = self.semilla.wrapping_mul(48271) & 0x7FFF_FFFF;
        self.semilla.max(1)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_secuencia_es_la_del_original_bit_a_bit() {
        // Los tres primeros estados desde la semilla 1, calculados a mano con
        // la formula: 1*48271 = 48271; 48271*48271 = 2330089441, que cabe en
        // 32 bits y al enmascarar con 0x7FFF_FFFF da 182605793; y de ahi
        // 182605793*48271 truncado a 32 bits y enmascarado.
        let mut a = Azar::nuevo(1);
        let primero = a.siguiente();
        let segundo = a.siguiente();

        assert_eq!(
            primero,
            48271.0 / 2_147_483_648.0,
            "el primer valor sale de multiplicar la semilla por 48271"
        );
        let esperado_2 = (48271u32.wrapping_mul(48271) & 0x7FFF_FFFF) as f32 / 2_147_483_648.0;
        assert_eq!(segundo, esperado_2);
    }

    #[test]
    fn la_misma_semilla_da_la_misma_secuencia_siempre() {
        // La invariante que sostiene todo: reabrir un documento lo dibuja
        // igual. Sin esto, cada apertura cambiaria el aspecto de cada trazo.
        let secuencia = |s| {
            let mut a = Azar::nuevo(s);
            (0..50).map(|_| a.siguiente()).collect::<Vec<_>>()
        };
        assert_eq!(secuencia(12345), secuencia(12345));
    }

    #[test]
    fn semillas_distintas_dan_secuencias_distintas() {
        // Caso negativo del anterior: un generador que devolviera siempre lo
        // mismo tambien pasaria "la misma semilla da lo mismo", y todos los
        // elementos del dibujo saldrian identicos.
        let secuencia = |s| {
            let mut a = Azar::nuevo(s);
            (0..50).map(|_| a.siguiente()).collect::<Vec<_>>()
        };
        assert_ne!(secuencia(12345), secuencia(999));
    }

    #[test]
    fn todos_los_valores_caen_en_el_rango() {
        let mut a = Azar::nuevo(7);
        for _ in 0..10_000 {
            let v = a.siguiente();
            assert!((0.0..1.0).contains(&v), "fuera de rango: {v}");
        }
    }

    #[test]
    fn la_semilla_cero_no_degenera_en_azar_real() {
        // El original caia en aleatoriedad no reproducible con semilla 0.
        // Aqui dos generadores con semilla 0 deben coincidir.
        let mut a = Azar::nuevo(0);
        let mut b = Azar::nuevo(0);
        let sa: Vec<_> = (0..10).map(|_| a.siguiente()).collect();
        let sb: Vec<_> = (0..10).map(|_| b.siguiente()).collect();
        assert_eq!(sa, sb);
    }

    #[test]
    fn el_desvio_se_reparte_a_ambos_lados_del_cero() {
        let mut a = Azar::nuevo(3);
        let muestras: Vec<f32> = (0..1000).map(|_| a.desvio(2.0)).collect();
        assert!(muestras.iter().all(|v| (-2.0..2.0).contains(v)));
        assert!(
            muestras.iter().any(|v| *v < 0.0),
            "nunca desvia a la izquierda"
        );
        assert!(
            muestras.iter().any(|v| *v > 0.0),
            "nunca desvia a la derecha"
        );
        // Y la media ronda cero: un desvio sesgado torceria todos los trazos
        // hacia el mismo lado.
        let media = muestras.iter().sum::<f32>() / muestras.len() as f32;
        assert!(media.abs() < 0.15, "el desvio esta sesgado: media {media}");
    }

    #[test]
    fn las_semillas_derivadas_nunca_son_cero() {
        // Una semilla 0 seria el unico valor que el generador no puede usar.
        let mut a = Azar::nuevo(1);
        for _ in 0..10_000 {
            assert_ne!(a.semilla_derivada(), 0);
        }
    }
}
