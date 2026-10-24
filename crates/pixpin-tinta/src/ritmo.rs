//! Cuando empezar a pintar, y cuanto adelantar la punta.
//!
//! **El problema (B1).** Hoy el fotograma empieza en cuanto la senal de
//! latencia del swapchain se dispara —o sea, justo despues de la composicion
//! anterior—, se pinta en unos milisegundos, y el resultado se queda
//! esperando quieto los 10 o 13 ms que faltan para que DWM lo componga. Los
//! puntos que se pintaron llegan a la pantalla con TODA esa espera encima.
//! Raph Levien lo cuenta al reves de como suena: cuanto mas rapido pintas,
//! mas latencia tienes, porque mas rato esperas con el fotograma hecho.
//!
//! **La salida.** Empezar a pintar en `plazo − p99(pintar) − margen`, no
//! nada mas poder. El plazo lo da `IDCompositionDevice::GetFrameStatistics`
//! (`nextEstimatedFrameTime`); el p99 se mide; el margen se sube solo cuando
//! se pierde un refresco y baja despacio cuando no.
//!
//! **Y la prediccion.** El horizonte de la punta predicha tiene que valer lo
//! que valga la latencia de verdad, no 28 ms fijos: si con B1 la latencia
//! baja y el horizonte no, la punta se adelanta de mas y luego se retrae,
//! que se ve como un pelo. `Horizonte` lo mide.
//!
//! Todo aqui es puro y se prueba sin GPU: son decisiones sobre numeros.

/// Cuantas duraciones de pintado se recuerdan para sacar el percentil.
const MUESTRAS: usize = 120;
/// El margen no baja de aqui: por debajo, cualquier hipo del planificador de
/// Windows pierde el refresco.
const MARGEN_MINIMO_MS: f32 = 1.0;
/// Ni sube de aqui: mas margen que esto es volver a esperar quieto, que es
/// justo lo que se viene a quitar.
const MARGEN_MAXIMO_MS: f32 = 6.0;
/// Lo que sube el margen al perder un refresco.
const CASTIGO_MS: f32 = 1.5;
/// Y lo que baja por cada fotograma que llega a tiempo. Mucho mas despacio
/// de lo que sube: perder un refresco se ve, esperar 0,05 ms de mas no.
const ALIVIO_MS: f32 = 0.05;

/// Decide a que hora empezar el fotograma.
#[derive(Debug, Clone)]
pub struct Planificador {
    duraciones: Vec<f32>,
    siguiente: usize,
    margen_ms: f32,
    perdidos: u32,
}

impl Default for Planificador {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Planificador {
    pub fn nuevo() -> Self {
        Self {
            duraciones: Vec::with_capacity(MUESTRAS),
            siguiente: 0,
            margen_ms: 2.0,
            perdidos: 0,
        }
    }

    /// Anota lo que tardo en pintarse y presentarse un fotograma.
    pub fn anotar(&mut self, ms: f32) {
        if !ms.is_finite() || ms < 0.0 {
            return;
        }
        if self.duraciones.len() < MUESTRAS {
            self.duraciones.push(ms);
        } else {
            self.duraciones[self.siguiente] = ms;
            self.siguiente = (self.siguiente + 1) % MUESTRAS;
        }
    }

    /// Un fotograma no llego a tiempo: el margen sube.
    pub fn perdido(&mut self) {
        self.perdidos += 1;
        self.margen_ms = (self.margen_ms + CASTIGO_MS).min(MARGEN_MAXIMO_MS);
    }

    /// Un fotograma llego a tiempo: el margen baja un poquito.
    pub fn a_tiempo(&mut self) {
        self.margen_ms = (self.margen_ms - ALIVIO_MS).max(MARGEN_MINIMO_MS);
    }

    pub fn perdidos(&self) -> u32 {
        self.perdidos
    }

    pub fn margen_ms(&self) -> f32 {
        self.margen_ms
    }

    /// El percentil 99 de lo que cuesta pintar, en milisegundos. Sin
    /// historia devuelve 0: el primer fotograma se pinta en cuanto puede,
    /// que es el comportamiento de antes.
    pub fn p99_ms(&self) -> f32 {
        if self.duraciones.is_empty() {
            return 0.0;
        }
        let mut v = self.duraciones.clone();
        v.sort_by(|a, b| a.partial_cmp(b).expect("ya se filtraron los no finitos"));
        let i = ((v.len() as f32 * 0.99).ceil() as usize).min(v.len()) - 1;
        v[i]
    }

    /// Cuanto hay que esperar, en milisegundos, antes de empezar a pintar.
    ///
    /// `hasta_el_plazo_ms` es lo que falta para la siguiente composicion de
    /// DWM. Devuelve 0 cuando ya no da tiempo a esperar —que es el caso
    /// normal en un equipo lento: ahi este planificador no cambia nada—, y
    /// nunca espera mas de lo que falta.
    pub fn esperar_ms(&self, hasta_el_plazo_ms: f32) -> f32 {
        if !hasta_el_plazo_ms.is_finite() || hasta_el_plazo_ms <= 0.0 {
            return 0.0;
        }
        let reserva = self.p99_ms() + self.margen_ms;
        (hasta_el_plazo_ms - reserva).clamp(0.0, hasta_el_plazo_ms)
    }
}

/// Cuantas latencias se recuerdan para sacar la mediana del horizonte.
const LATENCIAS: usize = 60;

/// El horizonte de la prediccion, medido en vez de supuesto.
#[derive(Debug, Clone, Default)]
pub struct Horizonte {
    latencias: Vec<f32>,
    siguiente: usize,
}

impl Horizonte {
    pub fn nuevo() -> Self {
        Self::default()
    }

    /// Anota la latencia de un fotograma: la hora estimada de composicion
    /// menos la hora de la muestra mas nueva que entro en el.
    pub fn anotar(&mut self, ms: f32) {
        if !ms.is_finite() || ms < 0.0 {
            return;
        }
        if self.latencias.len() < LATENCIAS {
            self.latencias.push(ms);
        } else {
            self.latencias[self.siguiente] = ms;
            self.siguiente = (self.siguiente + 1) % LATENCIAS;
        }
    }

    /// La mediana de lo medido, acotada entre `minimo` y `maximo`. Sin
    /// medidas devuelve `por_defecto`: los primeros fotogramas de un trazo
    /// predicen con la constante de siempre y no con un cero que dejaria la
    /// punta pegada al ultimo punto.
    ///
    /// Mediana y no media: una sola composicion perdida dispara la media y
    /// dejaria la punta adelantada durante todo el trazo siguiente.
    pub fn ms(&self, por_defecto: f32, minimo: f32, maximo: f32) -> f32 {
        if self.latencias.is_empty() {
            return por_defecto.clamp(minimo, maximo);
        }
        let mut v = self.latencias.clone();
        v.sort_by(|a, b| a.partial_cmp(b).expect("ya se filtraron los no finitos"));
        v[v.len() / 2].clamp(minimo, maximo)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn con_un_pintado_barato_se_espera_casi_hasta_el_plazo() {
        // El caso que motiva B1: pintar cuesta 3 ms y faltan 16 para la
        // composicion. Esperar 11 hace que los puntos lleguen 11 ms mas
        // frescos sin perder el refresco.
        let mut p = Planificador::nuevo();
        for _ in 0..120 {
            p.anotar(3.0);
        }
        let espera = p.esperar_ms(16.0);
        assert!((10.0..=12.0).contains(&espera), "{espera}");
    }

    #[test]
    fn si_pintar_cuesta_mas_que_el_refresco_no_se_espera_nada() {
        // Caso negativo, y el importante para el equipo suelo: en una HD
        // 4000 saturada el planificador no puede empeorar nada, porque no
        // llega a esperar.
        let mut p = Planificador::nuevo();
        for _ in 0..120 {
            p.anotar(25.0);
        }
        assert_eq!(p.esperar_ms(16.0), 0.0);
    }

    #[test]
    fn el_percentil_no_se_deja_arrastrar_por_la_media() {
        let mut p = Planificador::nuevo();
        for _ in 0..118 {
            p.anotar(2.0);
        }
        p.anotar(30.0);
        p.anotar(30.0);
        // La media seria 2,5 ms y reservar eso perderia esos dos refrescos;
        // el p99 ve el pico y reserva para el.
        assert!(p.p99_ms() >= 30.0, "p99 = {}", p.p99_ms());
        // Caso negativo: un pico entre 120 no llega al percentil 99, y eso
        // es lo correcto — reservar por un solo tiron de 30 ms costaria
        // latencia en los otros 119 fotogramas.
        let mut q = Planificador::nuevo();
        for _ in 0..119 {
            q.anotar(2.0);
        }
        q.anotar(30.0);
        assert_eq!(q.p99_ms(), 2.0);
    }

    #[test]
    fn perder_refrescos_sube_el_margen_y_acertar_lo_baja_despacio() {
        let mut p = Planificador::nuevo();
        let inicial = p.margen_ms();
        p.perdido();
        assert!(p.margen_ms() > inicial);
        let tras_fallo = p.margen_ms();
        p.a_tiempo();
        assert!(p.margen_ms() < tras_fallo);
        assert!(
            p.margen_ms() > inicial,
            "un solo acierto no borra un fallo: {}",
            p.margen_ms()
        );
        for _ in 0..1_000 {
            p.a_tiempo();
        }
        assert_eq!(p.margen_ms(), MARGEN_MINIMO_MS, "el margen tiene suelo");
    }

    #[test]
    fn un_plazo_absurdo_no_hace_esperar() {
        // Caso negativo: `GetFrameStatistics` puede devolver una hora vieja
        // o rara si la ventana esta tapada. Esperar un numero negativo o un
        // infinito colgaria el bucle.
        let p = Planificador::nuevo();
        assert_eq!(p.esperar_ms(-5.0), 0.0);
        assert_eq!(p.esperar_ms(f32::NAN), 0.0);
        assert_eq!(p.esperar_ms(f32::INFINITY), 0.0);
    }

    #[test]
    fn el_horizonte_sin_medidas_usa_la_constante_de_siempre() {
        let h = Horizonte::nuevo();
        assert_eq!(h.ms(28.0, 4.0, 40.0), 28.0);
    }

    #[test]
    fn el_horizonte_es_la_mediana_y_no_lo_mueve_un_tiron_suelto() {
        let mut h = Horizonte::nuevo();
        for _ in 0..59 {
            h.anotar(14.0);
        }
        h.anotar(200.0);
        assert_eq!(h.ms(28.0, 4.0, 40.0), 14.0);
    }

    #[test]
    fn el_horizonte_medido_queda_dentro_de_los_topes() {
        let mut h = Horizonte::nuevo();
        for _ in 0..60 {
            h.anotar(300.0);
        }
        assert_eq!(h.ms(28.0, 4.0, 40.0), 40.0, "no se predice medio segundo");
        // Caso negativo: una latencia negativa (dos relojes distintos) se
        // tira, no se cree.
        let mut h = Horizonte::nuevo();
        h.anotar(-10.0);
        assert_eq!(h.ms(28.0, 4.0, 40.0), 28.0);
    }
}
