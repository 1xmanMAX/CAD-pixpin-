//! **El motor del trazo en curso, aislado del resto del programa.**
//!
//! Lo que pidio el usuario: «si es necesario tenemos que crear un motor de
//! tinta que este aislado y funcione de forma altamente optimizado… para que
//! las tintas funcionen muy bien».
//!
//! Este crate se ocupa SOLO del trazo que se esta dibujando ahora mismo:
//! recibe las muestras del puntero a la frecuencia a la que las manda el
//! lapiz, las suaviza, predice donde va la punta, y dice que parte del
//! contorno hay que rehacer en este fotograma y cual no. No sabe de escena,
//! ni de camara, ni de Direct2D, ni de Windows: es puro y
//! `#![forbid(unsafe_code)]`, asi que se mide y se prueba sin GPU, sin
//! ventana y sin sintetizar entrada.
//!
//! Al soltar el boton, el trazo entero se entrega al motor 2D de siempre
//! (`pixpin-motor2d`), que hace con el exactamente lo que hacia antes: el
//! fichero que se guarda no cambia.
//!
//! **Que NO hace (todavia).** Pintarse en su propia capa de composicion y en
//! su propio hilo de prioridad alta. Eso es la segunda mitad del encargo y
//! es codigo de Windows —`IDCompositionVisual` propio, `BeginDraw` por
//! rectangulo, un `ID2D1Factory` multihilo— que no se puede dar por bueno
//! sin abrir la aplicacion. La forma de este crate esta pensada para que ese
//! paso no le cambie nada: el hilo del lienzo le pasaria muestras por un
//! canal y le pediria `contornos()`; nada de lo de aqui toca la escena.
//!
//! **De donde salen las ideas.** El filtro de 1 euro esta reimplementado
//! desde el articulo de CHI 2012 (`filtro.rs`). El troceado del contorno es
//! la idea de tldraw de acotar el trazo vivo, con codigo propio y sin partir
//! el elemento (`trazo.rs`). El planificador de fotograma es lo que describe
//! Raph Levien en «Swapchains and frame pacing» (`ritmo.rs`). El
//! extrapolador de la punta y sus dos protecciones vienen de lo que ya tenia
//! el proyecto en `pixpin-motor2d::tinta::prediccion`.

#![forbid(unsafe_code)]

pub mod filtro;
pub mod ritmo;
pub mod trazo;

pub use filtro::{FiltroUnEuro, Mandos};
pub use ritmo::{Horizonte, Planificador};
pub use trazo::{PUNTOS_DE_PUNTA, TrazoVivo};

/// Un punto en coordenadas del mundo.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Punto {
    pub x: f32,
    pub y: f32,
}

/// Una muestra del puntero tal como llega: posicion con subpixel, presion si
/// el aparato la da, y la hora real de la muestra.
///
/// La hora es del reloj monotono y la trae el propio mensaje
/// (`POINTER_INFO.PerformanceCount` en el lapiz, la marca del
/// `MOUSEMOVEPOINT` en el raton): NO es la hora a la que el programa se
/// entero. Es la diferencia entre filtrar con el tiempo de verdad y filtrar
/// suponiendo un ritmo fijo que el lapiz no tiene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Muestra {
    pub x: f32,
    pub y: f32,
    pub presion: Option<f32>,
    pub t_ms: f64,
}

/// Como se suaviza el trazo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Suavizado {
    /// Lo de siempre: solo el `streamline` de perfect-freehand. Es el modo
    /// del oraculo de E1 y el que no cambia ni un pixel de lo ya guardado.
    #[default]
    Excalidraw,
    /// El filtro de 1 euro delante de perfect-freehand. Menos «goma» a alta
    /// velocidad; cambia la geometria, por eso no es el de por defecto.
    Natural,
}

/// Lo que el usuario puede tocar desde `pixpinmax.toml`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ajustes {
    pub suavizado: Suavizado,
    pub mandos: Mandos,
    /// Cuantos puntos como mucho tiene un tramo del contorno vivo. A 0 no se
    /// trocea: el contorno entero se rehace en cada fotograma, como antes.
    pub puntos_por_tramo: usize,
    /// Puntos que comparten dos tramos seguidos, para que la union no se vea.
    pub solape: usize,
}

impl Default for Ajustes {
    /// Por defecto, conservador: el suavizado de siempre y el troceado
    /// ENCENDIDO. El troceado no cambia lo que se guarda (solo lo que se
    /// pinta mientras dura el gesto), asi que no hay motivo para dejarlo
    /// apagado; el suavizado si cambia la geometria, y ese va apagado.
    fn default() -> Self {
        Self {
            suavizado: Suavizado::Excalidraw,
            mandos: Mandos::default(),
            puntos_por_tramo: 600,
            solape: 8,
        }
    }
}

/// El motor: muestras que entran, contornos que salen.
#[derive(Debug)]
pub struct MotorTinta {
    ajustes: Ajustes,
    filtro: FiltroUnEuro,
    trazo: TrazoVivo,
    /// Las muestras SIN filtrar. Son la verdad del trazo: lo que se entrega
    /// al motor 2D al soltar. Filtrar lo que se guarda seria perder
    /// informacion que no se puede recuperar.
    crudas: Vec<Muestra>,
    horizonte: Horizonte,
}

impl MotorTinta {
    pub fn nuevo(ajustes: Ajustes) -> Self {
        Self {
            filtro: FiltroUnEuro::nuevo(ajustes.mandos),
            trazo: TrazoVivo::nuevo(ajustes.puntos_por_tramo, ajustes.solape),
            crudas: Vec::new(),
            horizonte: Horizonte::nuevo(),
            ajustes,
        }
    }

    pub fn ajustes(&self) -> Ajustes {
        self.ajustes
    }

    /// Empieza un trazo. No hereda nada del anterior: ni velocidad, ni
    /// posicion filtrada, ni tramos.
    pub fn empezar(&mut self) {
        self.filtro.reiniciar();
        self.trazo.empezar();
        self.crudas.clear();
    }

    /// Una muestra del puntero. Devuelve el punto tal como se va a PINTAR
    /// (filtrado o no, segun el ajuste).
    ///
    /// Las muestras que llegan hacia atras en el tiempo se tiran: el
    /// historial de `GetPointerFrameInfoHistory` viene del mas nuevo al mas
    /// viejo y, si alguien lo recorre en ese orden, un filtro que se fia de
    /// `dt` daria saltos.
    pub fn muestra(&mut self, m: Muestra) -> Option<Punto> {
        if let Some(u) = self.crudas.last()
            && m.t_ms < u.t_ms
        {
            return None;
        }
        self.crudas.push(m);
        let (x, y) = match self.ajustes.suavizado {
            Suavizado::Excalidraw => (m.x, m.y),
            Suavizado::Natural => self.filtro.filtrar(m.x, m.y, m.t_ms),
        };
        self.trazo.anadir(Muestra { x, y, ..m });
        Some(Punto { x, y })
    }

    /// Las muestras crudas: lo que se entrega al motor 2D al soltar.
    pub fn crudas(&self) -> &[Muestra] {
        &self.crudas
    }

    /// El trazo vivo, para pedirle los contornos.
    pub fn trazo(&self) -> &TrazoVivo {
        &self.trazo
    }

    pub fn trazo_mut(&mut self) -> &mut TrazoVivo {
        &mut self.trazo
    }

    /// El horizonte de la prediccion, para anotarle la latencia medida.
    pub fn horizonte_mut(&mut self) -> &mut Horizonte {
        &mut self.horizonte
    }

    pub fn horizonte_ms(&self, por_defecto: f32) -> f32 {
        self.horizonte.ms(por_defecto, 4.0, 40.0)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn m(i: usize, x: f32) -> Muestra {
        Muestra {
            x,
            y: 0.0,
            presion: Some(0.5),
            t_ms: i as f64 * 4.0,
        }
    }

    #[test]
    fn en_modo_excalidraw_lo_que_se_pinta_es_exactamente_lo_que_llego() {
        // La paridad con el oraculo de E1 depende de esto: en el modo de
        // siempre, el motor nuevo no puede mover ni un punto.
        let mut mt = MotorTinta::nuevo(Ajustes::default());
        mt.empezar();
        for i in 0..20 {
            let p = mt.muestra(m(i, i as f32 * 7.0)).unwrap();
            assert_eq!(p.x, i as f32 * 7.0);
        }
    }

    #[test]
    fn en_modo_natural_se_suaviza_lo_que_se_pinta_pero_no_lo_que_se_guarda() {
        let mut mt = MotorTinta::nuevo(Ajustes {
            suavizado: Suavizado::Natural,
            mandos: Mandos {
                corte_minimo: 0.5,
                beta: 0.0,
                corte_velocidad: 1.0,
            },
            ..Default::default()
        });
        mt.empezar();
        mt.muestra(m(0, 0.0));
        let pintado = mt.muestra(m(1, 100.0)).unwrap();
        assert!(pintado.x < 100.0, "se suavizo: {}", pintado.x);
        // Y aun asi lo que se guarda es el punto de verdad.
        assert_eq!(mt.crudas().last().unwrap().x, 100.0);
    }

    #[test]
    fn una_muestra_del_pasado_se_tira() {
        // Caso negativo: el historial del lapiz llega del mas nuevo al mas
        // viejo. Colarla haria que `dt` saliera negativo.
        let mut mt = MotorTinta::nuevo(Ajustes::default());
        mt.empezar();
        mt.muestra(m(5, 50.0));
        assert_eq!(mt.muestra(m(2, 20.0)), None);
        assert_eq!(mt.crudas().len(), 1);
    }

    #[test]
    fn empezar_otro_trazo_no_hereda_nada_del_anterior() {
        let mut mt = MotorTinta::nuevo(Ajustes::default());
        mt.empezar();
        for i in 0..1_000 {
            mt.muestra(m(i, i as f32));
        }
        mt.empezar();
        assert!(mt.crudas().is_empty());
        assert!(mt.trazo().vacio());
        assert!(mt.trazo().congelados().is_empty());
    }

    #[test]
    fn lo_de_por_defecto_trocea_pero_no_suaviza() {
        // El ajuste conservador: lo que no cambia el aspecto se enciende, lo
        // que si lo cambia se deja apagado.
        let a = Ajustes::default();
        assert_eq!(a.suavizado, Suavizado::Excalidraw);
        assert_eq!(a.puntos_por_tramo, 600);
    }
}
