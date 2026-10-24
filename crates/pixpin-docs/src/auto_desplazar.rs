//! **Leer con la pagina que sube sola** (Android v0.88 `98a50b9`, en el PDF
//! v0.92 `800db67`: `motor/AutoDesplazar.kt`). El usuario: «un scroll que
//! determine yo la velocidad, y que me muestre en cuanto terminare de leer
//! con esa velocidad».
//!
//! La velocidad no va en pixeles sino **en palabras por minuto**, que es
//! como se mide leer: asi no cambia al agrandar la letra (el documento se
//! alarga, pero las palabras son las mismas) y el «terminas en» sale
//! directo. Los pixeles por segundo se sacan de cuantas palabras tiene el
//! documento y cuanto mide. Aqui van las cuentas, sin ventanas; el bucle
//! esta en cada lector (`visor.rs`, `lector_pdf.rs`).

/// De leer despacio a hojear; el paso de los botones − y +.
pub const MINIMO: u32 = 80;
pub const MAXIMO: u32 = 700;
pub const PASO: u32 = 20;
/// Lo que lee de media un adulto en silencio.
pub const POR_DEFECTO: u32 = 230;

pub fn valida(ppm: u32) -> u32 {
    ppm.clamp(MINIMO, MAXIMO)
}

/// Un paso mas rapido (`sentido` +1) o mas lento (−1), redondeando al paso
/// para no quedar en 237.
pub fn otra(ppm: u32, sentido: i32) -> u32 {
    let abajo = (ppm / PASO * PASO) as i64;
    let siguiente = if sentido < 0 && abajo != ppm as i64 {
        abajo
    } else {
        abajo + sentido as i64 * PASO as i64
    };
    valida(siguiente.clamp(0, u32::MAX as i64) as u32)
}

/// **Cuantas unidades por segundo** hay que subir para leer `ppm` palabras
/// por minuto en un documento de `palabras` palabras que mide `alto`. Cero
/// si no hay nada que leer.
pub fn pixeles_por_segundo(ppm: u32, palabras: usize, alto: f32) -> f32 {
    if palabras == 0 || alto <= 0.0 {
        0.0
    } else {
        alto / palabras as f32 * ppm as f32 / 60.0
    }
}

/// **Los minutos que quedan** a `ppm`, con `progreso` de 0 a 1 (ver
/// [`progreso`]). Lo que ya se ve en pantalla al final cuenta como aun por
/// leer, asi que no se da por acabado antes.
pub fn minutos_que_quedan(palabras: usize, progreso: f32, ppm: u32) -> f32 {
    if palabras == 0 || ppm == 0 {
        0.0
    } else {
        palabras as f32 * (1.0 - progreso.clamp(0.0, 1.0)) / ppm as f32
    }
}

/// «menos de 1 min», «12 min», «1 h 05 min», «terminado».
pub fn rotulo(minutos: f32) -> String {
    rotulo_con(minutos, "menos de 1 min", "terminado")
}

/// [`rotulo`] con las dos frases que cambian de idioma.
pub fn rotulo_con(minutos: f32, menos_de_uno: &str, terminado: &str) -> String {
    let m = minutos.ceil() as i64;
    if minutos <= 0.0 {
        terminado.to_string()
    } else if m <= 1 && minutos < 1.0 {
        menos_de_uno.to_string()
    } else if m < 60 {
        format!("{m} min")
    } else {
        format!("{} h {:02} min", m / 60, m % 60)
    }
}

/// **La hora a la que se acaba**, «18:40», contando desde `ahora_en_minutos`
/// (minutos desde las 00:00 de hoy). Si pasa de medianoche se da la vuelta.
pub fn hora_de_terminar(ahora_en_minutos: i64, minutos: f32) -> String {
    let fin = (ahora_en_minutos + minutos.ceil() as i64).rem_euclid(24 * 60);
    format!("{}:{:02}", fin / 60, fin % 60)
}

/// **Por donde se va** (`Lectura.progreso`): `corrido` lo bajado, `alto` lo
/// que mide el documento y `vista` lo que se ve de el. Arriba del todo es
/// 0 y **al final, 1**: cuando ya se ve la ultima linea.
pub fn progreso(corrido: f32, alto: f32, vista: f32) -> f32 {
    let recorrido = alto - vista;
    if recorrido <= 0.0 {
        0.0
    } else {
        (corrido / recorrido).clamp(0.0, 1.0)
    }
}

/// Las palabras de un texto: lo que hay entre blancos (el `CONTAR` del movil,
/// `/[^\s]+/g`).
pub fn contar_palabras(texto: &str) -> usize {
    texto.split_whitespace().count()
}

/// **El bucle**, sin reloj propio: cada lector lo llama en cada vuelta con
/// la hora y la velocidad en pixeles de pantalla por segundo, y dice cuantos
/// pixeles enteros bajar. Lo que no llega a un pixel se guarda para la
/// siguiente (`sobra` del movil): a poca velocidad no va a saltos ni se
/// para, y el texto no tiembla entre dos pixeles.
#[derive(Debug, Clone, PartialEq)]
pub struct Marcha {
    pub ppm: u32,
    /// La pagina esta subiendo (con la barra puesta se puede pausar).
    pub en_marcha: bool,
    antes_ms: Option<u64>,
    sobra: f32,
}

impl Marcha {
    pub fn nueva(ppm: u32) -> Self {
        Self {
            ppm: valida(ppm),
            en_marcha: true,
            antes_ms: None,
            sobra: 0.0,
        }
    }

    /// Pausa o sigue. Al seguir no se cuenta el rato parado.
    pub fn alternar(&mut self) {
        self.en_marcha = !self.en_marcha;
        self.antes_ms = None;
        self.sobra = 0.0;
    }

    /// Mas deprisa (+1) o mas despacio (−1).
    pub fn cambiar(&mut self, sentido: i32) {
        self.ppm = otra(self.ppm, sentido);
    }

    /// Lo que hay que bajar ahora, en pixeles enteros. `quieta` es que algo
    /// la para sin cerrarla (anotando, escuchando): no corre ni acumula, y
    /// al volver no salta lo que se estuvo parado. Un hueco largo entre dos
    /// vueltas cuenta como mucho una decima, como en el movil.
    pub fn paso(&mut self, ahora_ms: u64, px_por_segundo: f32, quieta: bool) -> i32 {
        if !self.en_marcha || quieta || px_por_segundo <= 0.0 {
            self.antes_ms = None;
            self.sobra = 0.0;
            return 0;
        }
        let Some(antes) = self.antes_ms.replace(ahora_ms) else {
            return 0;
        };
        let dt = (ahora_ms.saturating_sub(antes) as f32 / 1000.0).min(0.1);
        self.sobra += px_por_segundo * dt;
        let n = self.sobra.floor();
        self.sobra -= n;
        n as i32
    }

    /// Se llego al final: se para sola (la barra sigue, con el play).
    pub fn al_final(&mut self) {
        self.en_marcha = false;
        self.antes_ms = None;
        self.sobra = 0.0;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cerca(a: f32, b: f32) {
        assert!((a - b).abs() < 0.001, "{a} no es {b}");
    }

    // Los cuatro de `AutoDesplazarTest.kt`, con los mismos numeros.

    #[test]
    fn la_velocidad_en_palabras_por_minuto_se_vuelve_pixeles_por_segundo() {
        // 6000 palabras en 60000 px: 10 px por palabra; a 240/min, 4 por segundo → 40 px/s.
        cerca(pixeles_por_segundo(240, 6000, 60000.0), 40.0);
        // Con la letra al doble el documento mide el doble y va el doble de rapido: se lee igual.
        cerca(pixeles_por_segundo(240, 6000, 120000.0), 80.0);
        assert_eq!(pixeles_por_segundo(240, 0, 60000.0), 0.0);
        assert_eq!(pixeles_por_segundo(240, 6000, 0.0), 0.0, "sin alto, quieta");
    }

    #[test]
    fn lo_que_queda_baja_con_lo_leido_y_con_la_velocidad() {
        cerca(minutos_que_quedan(6000, 0.0, 240), 25.0);
        cerca(minutos_que_quedan(6000, 0.5, 240), 12.5);
        cerca(minutos_que_quedan(6000, 1.0, 240), 0.0);
        cerca(minutos_que_quedan(6000, 0.5, 300), 10.0);
        // Un progreso fuera de 0..1 no da minutos negativos ni de mas.
        cerca(minutos_que_quedan(6000, 1.7, 240), 0.0);
        cerca(minutos_que_quedan(6000, -3.0, 240), 25.0);
        assert_eq!(minutos_que_quedan(0, 0.0, 240), 0.0);
    }

    #[test]
    fn los_rotulos_se_leen_bien() {
        assert_eq!(rotulo(0.4), "menos de 1 min");
        assert_eq!(rotulo(11.2), "12 min");
        assert_eq!(rotulo(65.0), "1 h 05 min");
        assert_eq!(rotulo(0.0), "terminado");
        assert_eq!(hora_de_terminar(18 * 60 + 28, 11.2), "18:40");
        assert_eq!(hora_de_terminar(23 * 60 + 50, 20.0), "0:10");
    }

    #[test]
    fn los_botones_van_de_paso_en_paso_sin_salirse() {
        assert_eq!(otra(230, 1), 240);
        assert_eq!(otra(230, -1), 220);
        assert_eq!(otra(220, -1), 200);
        assert_eq!(otra(MINIMO, -1), MINIMO);
        assert_eq!(otra(MAXIMO, 1), MAXIMO);
        assert_eq!(valida(5), MINIMO);
        assert_eq!(valida(9000), MAXIMO);
    }

    #[test]
    fn el_progreso_llega_a_uno_cuando_se_ve_el_final() {
        assert_eq!(progreso(0.0, 1000.0, 400.0), 0.0);
        cerca(progreso(300.0, 1000.0, 400.0), 0.5);
        assert_eq!(progreso(600.0, 1000.0, 400.0), 1.0);
        // Cabe entero: no hay nada que correr.
        assert_eq!(progreso(0.0, 300.0, 400.0), 0.0);
    }

    #[test]
    fn las_palabras_se_cuentan_entre_blancos() {
        assert_eq!(contar_palabras("  Hola,  mundo\n otra\tlinea "), 4);
        assert_eq!(contar_palabras(" \n\t "), 0);
    }

    #[test]
    fn el_bucle_guarda_lo_que_no_llega_a_un_pixel() {
        let mut m = Marcha::nueva(230);
        // La primera vuelta solo apunta la hora.
        assert_eq!(m.paso(1000, 30.0, false), 0);
        // 16 ms a 30 px/s son 0,48 px: nada aun, pero se guarda.
        assert_eq!(m.paso(1016, 30.0, false), 0);
        assert_eq!(m.paso(1032, 30.0, false), 0);
        assert_eq!(
            m.paso(1048, 30.0, false),
            1,
            "tres vueltas pasan de un pixel"
        );
        // Un parón largo cuenta una decima como mucho: 3 px, no 300.
        assert_eq!(m.paso(11_048, 30.0, false), 3);
    }

    #[test]
    fn parada_no_corre_ni_salta_al_volver() {
        let mut m = Marcha::nueva(230);
        m.paso(0, 100.0, false);
        assert_eq!(m.paso(50, 100.0, true), 0, "anotando no corre");
        assert_eq!(m.paso(5_000, 100.0, false), 0, "al volver, primero la hora");
        assert_eq!(m.paso(5_055, 100.0, false), 5);
        m.alternar();
        assert!(!m.en_marcha);
        assert_eq!(m.paso(5_100, 100.0, false), 0, "en pausa no corre");
        m.alternar();
        m.paso(9_000, 100.0, false);
        assert_eq!(m.paso(9_015, 100.0, false), 1);
        m.al_final();
        assert_eq!(m.paso(9_030, 100.0, false), 0, "al final se para sola");
        // Sin velocidad (sin palabras) no se mueve.
        let mut q = Marcha::nueva(230);
        q.paso(0, 0.0, false);
        assert_eq!(q.paso(100, 0.0, false), 0);
    }

    #[test]
    fn la_velocidad_guardada_se_acota() {
        assert_eq!(Marcha::nueva(10).ppm, MINIMO);
        let mut m = Marcha::nueva(MAXIMO);
        m.cambiar(1);
        assert_eq!(m.ppm, MAXIMO);
        m.cambiar(-1);
        assert_eq!(m.ppm, MAXIMO - PASO);
    }
}
