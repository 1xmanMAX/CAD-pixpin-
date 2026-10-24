//! Los gestos del panel tactil de precision: dos dedos para desplazar y
//! pellizco para acercar, cada uno por su lado.
//!
//! Por que DirectManipulation y no la rueda: sin el, Windows convierte el
//! desplazamiento de dos dedos en `WM_MOUSEWHEEL` (el mismo mensaje que la
//! rueda de un raton) y el pellizco en `WM_MOUSEWHEEL` con Ctrl fingido.
//! Distinguir raton de panel mirando solo la rueda es adivinar. Con
//! DirectManipulation, en cuanto el panel de precision pone dos dedos
//! Windows pregunta a la ventana (`DM_POINTERHITTEST`); si se le entrega el
//! contacto (`SetContact`), el gesto ya NO llega como rueda: llega como una
//! transformacion (desplazamiento y escala por separado), inercia incluida.
//! Lo que siga llegando como `WM_MOUSEWHEEL` es un raton (o un panel viejo
//! que no es de precision, que decide `pixpin::navegacion`).
//!
//! Es lo mismo que hacen Chromium y Edge
//! (content/browser/renderer_host/direct_manipulation_helper_win.cc y
//! direct_manipulation_event_handler_win.cc, BSD, Copyright The Chromium
//! Authors): viewport de mentira con `MANUALUPDATE`, `Update` en cada
//! fotograma SOLO mientras dura el gesto, y la transformacion traducida a
//! deltas. `Traductor` es esa traduccion, pura y probada sin pantalla.
//!
//! Coste en reposo: cero. El temporizador que empuja `Update` solo vive
//! entre el primer contacto y el final de la inercia.

use std::cell::{Cell, RefCell};

use pixpin_geom::Punto;

/// `DM_POINTERHITTEST`: Windows pregunta si la ventana quiere un contacto
/// del panel tactil antes de decidir que hacer con el.
pub const DM_POINTERHITTEST: u32 = 0x0250;
/// El temporizador que empuja `Update` durante el gesto. Un numero propio
/// cualquiera: solo tiene que no chocar con otros temporizadores de la
/// ventana de overlay, que hoy no tiene ninguno.
pub const ID_TEMPORIZADOR: usize = 0x5044_4D31;
/// Cada cuanto se empuja `Update` mientras dura el gesto. 8 ms piden el
/// fotograma de 120 Hz; Windows lo redondea a su reloj (15,6 ms sin
/// `timeBeginPeriod`), que sigue siendo mas de 60 pasos por segundo.
pub const PERIODO_MS: u32 = 8;
/// Latidos seguidos sin gesto antes de apagar el temporizador: un toque
/// suelto (un clic con el panel) entrega el contacto pero no llega a
/// empezar interaccion, y sin este tope el temporizador seguiria vivo.
pub const LATIDOS_OCIOSOS: u32 = 30;

/// Un paso del desplazamiento de dos dedos: lo que se movio el CONTENIDO,
/// en pixeles fisicos (positivo: a la derecha y abajo, el dibujo sigue a los
/// dedos segun el sentido que el usuario eligio en Configuracion).
///
/// En dieciseisavos de pixel por lo mismo que `puntero::Muestra`: va dentro
/// de `EventoOverlay`, que es `Copy + Eq`, y un `f32` no es `Eq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Desliz {
    dx16: i32,
    dy16: i32,
}

impl Desliz {
    pub fn nuevo(dx: f32, dy: f32) -> Self {
        Self {
            dx16: (dx * 16.0).round() as i32,
            dy16: (dy * 16.0).round() as i32,
        }
    }
    pub fn dx(&self) -> f32 {
        self.dx16 as f32 / 16.0
    }
    pub fn dy(&self) -> f32 {
        self.dy16 as f32 / 16.0
    }
}

/// Un paso del pellizco: cuanto crece el zoom respecto al paso anterior
/// (1,05 = un 5 % mas cerca) y donde estaba el cursor, en el escritorio
/// virtual. En un panel tactil los dedos no estan en la pantalla: el punto
/// que se queda quieto es el del cursor, como en Edge y en Chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pellizco {
    /// El factor en millonesimas, por el `Eq` de `EventoOverlay`.
    factor_millonesimas: i32,
    pub en: Punto,
}

impl Pellizco {
    pub fn nuevo(factor: f32, en: Punto) -> Self {
        Self {
            factor_millonesimas: (factor * 1_000_000.0).round() as i32,
            en,
        }
    }
    pub fn factor(&self) -> f32 {
        self.factor_millonesimas as f32 / 1_000_000.0
    }
}

/// Un `WM_MOUSEWHEEL`/`WM_MOUSEHWHEEL` con todo lo que trae, para las
/// ventanas que pidieron gestos tactiles: quien decide si es raton o panel
/// (`pixpin::navegacion`) necesita la hora y los modificadores DEL MENSAJE.
/// Ctrl sale de `MK_CONTROL` del propio wparam y no de sondear el teclado:
/// el pellizco de un panel sin DirectManipulation llega como rueda con Ctrl
/// fingido, y ese Ctrl solo es seguro en el mensaje.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuedaCruda {
    /// Positivo: arriba (vertical) o derecha (horizontal). 120 por muesca.
    pub delta: i32,
    pub horizontal: bool,
    /// `GetMessageTime`, milisegundos que dan la vuelta.
    pub ms: u32,
    pub ctrl: bool,
    pub shift: bool,
}

/// Los estados del viewport de DirectManipulation que importan aqui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    Listo,
    Corriendo,
    Inercia,
    /// Construyendo, habilitado, deshabilitado, suspendido, cancelado...
    Otro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fase {
    Nada,
    Desliz,
    Inercia,
    Pellizco,
}

/// Lo que sale de un cambio de la transformacion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Salida {
    /// Pixeles fisicos que se movio el contenido.
    Desliz { dx: f32, dy: f32 },
    /// Factor respecto al paso anterior.
    Pellizco { factor: f32 },
}

/// La maquina de estados de `DirectManipulationEventHandler` de Chromium:
/// de la transformacion ACUMULADA del viewport (escala y desplazamiento) a
/// pasos sueltos de desplazar o de acercar, nunca las dos cosas a la vez.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Traductor {
    fase: Fase,
    escala: f32,
    x: f32,
    y: f32,
}

impl Default for Traductor {
    fn default() -> Self {
        Self {
            fase: Fase::Nada,
            escala: 1.0,
            x: 0.0,
            y: 0.0,
        }
    }
}

/// Igualdad con tolerancia relativa, la `FloatEquals` de Chromium: la escala
/// de un desplazamiento puro vuelve como 0,99999994 y no es un pellizco.
fn casi_igual(a: f32, b: f32) -> bool {
    const EPSILON: f32 = 0.00001;
    (a - b).abs() < EPSILON * a.abs().max(b.abs()).max(EPSILON)
}

impl Traductor {
    /// La transformacion nueva del contenido (`GetContentTransform`: escala
    /// en `[0]`, desplazamiento en `[4]` y `[5]`, pixeles fisicos).
    pub fn contenido(&mut self, escala: f32, x: f32, y: f32) -> Option<Salida> {
        // Windows a veces manda escala 0: dividir por ella seria infinito.
        if escala == 0.0 || !escala.is_finite() || !x.is_finite() || !y.is_finite() {
            return None;
        }
        if casi_igual(escala, self.escala) && x == self.x && y == self.y {
            return None;
        }
        // Un gesto es desplazar O pellizcar. El pellizco lento puede empezar
        // pareciendo un desplazamiento (se deja pasar de desplazar a
        // pellizcar), pero una vez pellizcando el desplazamiento que trae la
        // escala alrededor de los dedos es ruido y no se aplica.
        if casi_igual(escala, 1.0) {
            if self.fase == Fase::Nada {
                self.fase = Fase::Desliz;
            }
        } else {
            self.fase = Fase::Pellizco;
        }
        let salida = match self.fase {
            Fase::Pellizco => Salida::Pellizco {
                factor: escala / self.escala,
            },
            _ => Salida::Desliz {
                dx: x - self.x,
                dy: y - self.y,
            },
        };
        self.escala = escala;
        self.x = x;
        self.y = y;
        match salida {
            Salida::Desliz { dx, dy } if dx == 0.0 && dy == 0.0 => None,
            Salida::Pellizco { factor } if casi_igual(factor, 1.0) => None,
            s => Some(s),
        }
    }

    /// El viewport cambio de estado. Devuelve `true` si hay que devolverlo a
    /// su sitio (`ZoomToRect` a su rectangulo): al acabar cada gesto, para
    /// que el siguiente empiece desde la identidad y nunca choque con un
    /// borde.
    pub fn estado(&mut self, actual: Estado, anterior: Estado) -> bool {
        if actual == anterior {
            return false;
        }
        match actual {
            Estado::Inercia => {
                // Solo un desplazamiento tiene inercia; lo que quede de un
                // pellizco soltado con prisa sigue siendo pellizco.
                if anterior == Estado::Corriendo && self.fase == Fase::Desliz {
                    self.fase = Fase::Inercia;
                }
                false
            }
            // Dedos otra vez en el panel durante la inercia: gesto nuevo.
            Estado::Corriendo if anterior == Estado::Inercia => {
                self.fase = Fase::Nada;
                false
            }
            Estado::Listo => {
                let movido = self.escala != 1.0 || self.x != 0.0 || self.y != 0.0;
                *self = Self::default();
                movido
            }
            _ => false,
        }
    }
}

/// Lo que el viewport le cuenta a la ventana.
pub type Aviso = Box<dyn Fn(Salida)>;

#[cfg(windows)]
mod com {
    use super::*;
    use windows::Win32::Foundation::{HWND, RECT, WPARAM};
    use windows::Win32::Graphics::DirectManipulation::*;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize,
    };
    use windows::Win32::UI::Input::Pointer::GetPointerType;
    use windows::Win32::UI::WindowsAndMessaging::{KillTimer, PT_TOUCHPAD, SetTimer};
    use windows::core::{Ref, implement};

    fn estado_de(s: DIRECTMANIPULATION_STATUS) -> Estado {
        match s {
            DIRECTMANIPULATION_READY => Estado::Listo,
            DIRECTMANIPULATION_RUNNING => Estado::Corriendo,
            DIRECTMANIPULATION_INERTIA => Estado::Inercia,
            _ => Estado::Otro,
        }
    }

    /// Lo que comparten el manejador (que llama Windows) y quien lo creo.
    struct Latido {
        hwnd: HWND,
        vivo: Cell<bool>,
        interactuando: Cell<bool>,
        ociosos: Cell<u32>,
    }

    impl Latido {
        fn encender(&self) {
            self.ociosos.set(0);
            if !self.vivo.get() {
                // SAFETY: temporizador sobre una ventana propia del hilo; si
                // falla, el gesto no avanza pero nada se rompe.
                unsafe { SetTimer(Some(self.hwnd), ID_TEMPORIZADOR, PERIODO_MS, None) };
                self.vivo.set(true);
            }
        }
        fn apagar(&self) {
            if self.vivo.get() {
                // SAFETY: el temporizador es nuestro; si ya no existe, falla
                // y se ignora.
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), ID_TEMPORIZADOR);
                }
                self.vivo.set(false);
            }
        }
    }

    #[implement(
        IDirectManipulationViewportEventHandler,
        IDirectManipulationInteractionEventHandler
    )]
    struct Manejador {
        traductor: RefCell<Traductor>,
        rect: RECT,
        latido: std::rc::Rc<Latido>,
        avisar: Aviso,
    }

    impl IDirectManipulationViewportEventHandler_Impl for Manejador_Impl {
        fn OnViewportStatusChanged(
            &self,
            viewport: Ref<IDirectManipulationViewport>,
            current: DIRECTMANIPULATION_STATUS,
            previous: DIRECTMANIPULATION_STATUS,
        ) -> windows::core::Result<()> {
            let volver = self
                .traductor
                .borrow_mut()
                .estado(estado_de(current), estado_de(previous));
            if volver {
                if let Some(v) = viewport.as_ref() {
                    let r = self.rect;
                    // SAFETY: viewport vivo que nos pasa Windows.
                    unsafe {
                        v.ZoomToRect(
                            r.left as f32,
                            r.top as f32,
                            r.right as f32,
                            r.bottom as f32,
                            false,
                        )?;
                    }
                }
            }
            if current == DIRECTMANIPULATION_READY && !self.latido.interactuando.get() {
                self.latido.apagar();
            }
            Ok(())
        }

        fn OnViewportUpdated(
            &self,
            _viewport: Ref<IDirectManipulationViewport>,
        ) -> windows::core::Result<()> {
            Ok(())
        }

        fn OnContentUpdated(
            &self,
            _viewport: Ref<IDirectManipulationViewport>,
            content: Ref<IDirectManipulationContent>,
        ) -> windows::core::Result<()> {
            let Some(content) = content.as_ref() else {
                return Ok(());
            };
            let mut m = [0.0f32; 6];
            // SAFETY: contenido vivo que nos pasa Windows; seis floats.
            unsafe { content.GetContentTransform(&mut m)? };
            let salida = self.traductor.borrow_mut().contenido(m[0], m[4], m[5]);
            if let Some(s) = salida {
                (self.avisar)(s);
            }
            Ok(())
        }
    }

    impl IDirectManipulationInteractionEventHandler_Impl for Manejador_Impl {
        fn OnInteraction(
            &self,
            _viewport: Ref<IDirectManipulationViewport2>,
            interaction: DIRECTMANIPULATION_INTERACTION_TYPE,
        ) -> windows::core::Result<()> {
            if interaction == DIRECTMANIPULATION_INTERACTION_BEGIN {
                self.latido.interactuando.set(true);
                self.latido.encender();
            } else if interaction == DIRECTMANIPULATION_INTERACTION_END {
                self.latido.interactuando.set(false);
            }
            Ok(())
        }
    }

    /// DirectManipulation enganchado a una ventana, solo para el panel
    /// tactil de precision. Se suelta al soltarlo.
    pub struct GestosTactiles {
        hwnd: HWND,
        manager: IDirectManipulationManager,
        update: IDirectManipulationUpdateManager,
        viewport: IDirectManipulationViewport,
        cookie: u32,
        latido: std::rc::Rc<Latido>,
        /// ULTIMO campo a proposito: Rust suelta los campos en orden, y el
        /// apartamento COM tiene que cerrarse despues de soltar los objetos.
        _com: Apartamento,
    }

    /// Cierra el `CoInitializeEx` que abrio `GestosTactiles::nuevo`, si lo
    /// abrio el.
    struct Apartamento(bool);

    impl Drop for Apartamento {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: empareja el CoInitializeEx que tuvo exito.
                unsafe { CoUninitialize() };
            }
        }
    }

    impl GestosTactiles {
        /// Todo como `DirectManipulationHelper::CreateInstanceImpl`.
        pub fn nuevo(
            hwnd: HWND,
            ancho: i32,
            alto: i32,
            avisar: Aviso,
        ) -> windows::core::Result<Self> {
            // SAFETY: sin precondiciones. Si el hilo ya estaba en otro
            // apartamento falla y se sigue: DirectManipulation dira si puede.
            let com =
                Apartamento(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok());
            // Si montar falla, `com` se suelta con el error y cierra el
            // apartamento.
            Self::montar(hwnd, ancho, alto, avisar, com)
        }

        fn montar(
            hwnd: HWND,
            ancho: i32,
            alto: i32,
            avisar: Aviso,
            com: Apartamento,
        ) -> windows::core::Result<Self> {
            // SAFETY: llamadas COM documentadas sobre objetos recien creados
            // y una ventana propia de este hilo.
            unsafe {
                let manager: IDirectManipulationManager =
                    CoCreateInstance(&DirectManipulationManager, None, CLSCTX_INPROC_SERVER)?;
                let update: IDirectManipulationUpdateManager = manager.GetUpdateManager()?;
                let viewport: IDirectManipulationViewport =
                    manager.CreateViewport(None::<&IDirectManipulationFrameInfoProvider>, hwnd)?;
                let preparar = || -> windows::core::Result<()> {
                    viewport.ActivateConfiguration(
                        DIRECTMANIPULATION_CONFIGURATION_INTERACTION
                            | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_X
                            | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_Y
                            | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_INERTIA
                            | DIRECTMANIPULATION_CONFIGURATION_RAILS_X
                            | DIRECTMANIPULATION_CONFIGURATION_RAILS_Y
                            | DIRECTMANIPULATION_CONFIGURATION_SCALING,
                    )?;
                    // Viewport de mentira: nadie lo pinta, solo se lee.
                    viewport
                        .SetViewportOptions(DIRECTMANIPULATION_VIEWPORT_OPTIONS_MANUALUPDATE)?;
                    Ok(())
                };
                if let Err(e) = preparar() {
                    let _ = viewport.Abandon();
                    return Err(e);
                }
                let rect = RECT {
                    left: 0,
                    top: 0,
                    right: ancho.max(1),
                    bottom: alto.max(1),
                };
                let latido = std::rc::Rc::new(Latido {
                    hwnd,
                    vivo: Cell::new(false),
                    interactuando: Cell::new(false),
                    ociosos: Cell::new(0),
                });
                let manejador: IDirectManipulationViewportEventHandler = Manejador {
                    traductor: RefCell::new(Traductor::default()),
                    rect,
                    latido: latido.clone(),
                    avisar,
                }
                .into();
                let seguir = || -> windows::core::Result<u32> {
                    viewport.SetViewportRect(&rect)?;
                    let cookie = viewport.AddEventHandler(Some(hwnd), &manejador)?;
                    if let Err(e) = manager.Activate(hwnd) {
                        let _ = viewport.RemoveEventHandler(cookie);
                        return Err(e);
                    }
                    if let Err(e) = viewport.Enable() {
                        let _ = manager.Deactivate(hwnd);
                        let _ = viewport.RemoveEventHandler(cookie);
                        return Err(e);
                    }
                    let _ = update.Update(None::<&IDirectManipulationFrameInfoProvider>);
                    Ok(cookie)
                };
                let cookie = match seguir() {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = viewport.Abandon();
                        return Err(e);
                    }
                };
                Ok(Self {
                    hwnd,
                    manager,
                    update,
                    viewport,
                    cookie,
                    latido,
                    _com: com,
                })
            }
        }

        /// `DM_POINTERHITTEST`: si el contacto es del panel tactil de
        /// precision, se lo queda DirectManipulation. Cualquier otro (tacto
        /// en pantalla, lapiz) sigue su camino de siempre.
        pub fn tocar(&self, wparam: WPARAM) {
            let id = (wparam.0 & 0xFFFF) as u32;
            let mut tipo = Default::default();
            // SAFETY: consulta del puntero que Windows acaba de anunciar.
            let es_panel = unsafe { GetPointerType(id, &mut tipo) }.is_ok() && tipo == PT_TOUCHPAD;
            if es_panel {
                // SAFETY: viewport vivo; el id es el del mensaje.
                if unsafe { self.viewport.SetContact(id) }.is_ok() {
                    self.latido.encender();
                }
            }
        }

        /// `WM_TIMER`: un fotograma de mentira para que el gesto avance.
        pub fn latido(&self) {
            // SAFETY: gestor vivo; los avisos salen por el manejador en este
            // mismo hilo.
            unsafe {
                let _ = self
                    .update
                    .Update(None::<&IDirectManipulationFrameInfoProvider>);
            }
            if self.latido.interactuando.get() {
                return;
            }
            // SAFETY: consulta del viewport vivo.
            let estado = unsafe { self.viewport.GetStatus() }
                .map(estado_de)
                .unwrap_or(Estado::Otro);
            if estado != Estado::Corriendo && estado != Estado::Inercia {
                let n = self.latido.ociosos.get() + 1;
                self.latido.ociosos.set(n);
                if n >= LATIDOS_OCIOSOS {
                    self.latido.apagar();
                }
            }
        }

        /// Si el temporizador esta vivo (para las pruebas: en reposo, no).
        pub fn latiendo(&self) -> bool {
            self.latido.vivo.get()
        }
    }

    impl Drop for GestosTactiles {
        fn drop(&mut self) {
            self.latido.apagar();
            // SAFETY: el orden de `DirectManipulationHelper::Destroy`.
            unsafe {
                let _ = self.viewport.Stop();
                let _ = self.viewport.RemoveEventHandler(self.cookie);
                let _ = self.viewport.Abandon();
                let _ = self.manager.Deactivate(self.hwnd);
            }
        }
    }
}

#[cfg(windows)]
pub use com::GestosTactiles;

#[cfg(test)]
mod pruebas {
    use super::*;

    fn desliz(s: Option<Salida>) -> (f32, f32) {
        match s {
            Some(Salida::Desliz { dx, dy }) => (dx, dy),
            otra => panic!("se esperaba un desliz, llego {otra:?}"),
        }
    }

    #[test]
    fn dos_dedos_dan_pasos_de_desplazamiento_en_las_dos_direcciones_a_la_vez() {
        let mut t = Traductor::default();
        assert_eq!(desliz(t.contenido(1.0, 10.0, -4.0)), (10.0, -4.0));
        // Acumulado 25,-10: el paso es la diferencia, en diagonal.
        assert_eq!(desliz(t.contenido(1.0, 25.0, -10.0)), (15.0, -6.0));
    }

    #[test]
    fn deslizar_nunca_da_zoom_aunque_la_escala_tiemble_en_la_ultima_cifra() {
        let mut t = Traductor::default();
        for i in 1..20 {
            let s = t.contenido(0.999_999_94, i as f32 * 3.0, 0.0);
            assert!(matches!(s, Some(Salida::Desliz { .. })), "{i}: {s:?}");
        }
    }

    #[test]
    fn un_pellizco_da_el_factor_respecto_al_paso_anterior() {
        let mut t = Traductor::default();
        let Some(Salida::Pellizco { factor }) = t.contenido(1.2, 0.0, 0.0) else {
            panic!()
        };
        assert!((factor - 1.2).abs() < 1e-6);
        let Some(Salida::Pellizco { factor }) = t.contenido(1.5, -30.0, -20.0) else {
            panic!()
        };
        assert!((factor - 1.25).abs() < 1e-6);
    }

    #[test]
    fn pellizcando_el_desplazamiento_que_trae_la_escala_no_mueve_el_lienzo() {
        // Caso negativo: DirectManipulation escala alrededor de los dedos y
        // mueve el origen; eso no es un desplazamiento del usuario.
        let mut t = Traductor::default();
        let _ = t.contenido(1.1, 0.0, 0.0);
        let s = t.contenido(1.1, 40.0, 40.0);
        assert_eq!(s, None, "ni desliz ni factor 1");
    }

    #[test]
    fn un_pellizco_lento_que_empezo_como_desliz_pasa_a_pellizco_y_no_vuelve() {
        let mut t = Traductor::default();
        assert!(matches!(
            t.contenido(1.0, 2.0, 1.0),
            Some(Salida::Desliz { .. })
        ));
        assert!(matches!(
            t.contenido(1.05, 2.0, 1.0),
            Some(Salida::Pellizco { .. })
        ));
        // Vuelve a escala 1 sin soltar: sigue siendo pellizco (factor < 1).
        assert!(matches!(
            t.contenido(1.0, 9.0, 9.0),
            Some(Salida::Pellizco { .. })
        ));
    }

    #[test]
    fn la_inercia_sigue_desplazando_y_un_dedo_nuevo_empieza_otro_gesto() {
        let mut t = Traductor::default();
        let _ = t.contenido(1.0, 10.0, 0.0);
        assert!(!t.estado(Estado::Inercia, Estado::Corriendo));
        assert_eq!(desliz(t.contenido(1.0, 18.0, 0.0)), (8.0, 0.0));
        assert!(!t.estado(Estado::Corriendo, Estado::Inercia));
        // Gesto nuevo, pero la transformacion sigue acumulada: el paso es
        // la diferencia, no un salto desde cero.
        assert_eq!(desliz(t.contenido(1.0, 20.0, 0.0)), (2.0, 0.0));
    }

    #[test]
    fn al_acabar_el_gesto_se_pide_volver_a_la_identidad_y_se_empieza_de_cero() {
        let mut t = Traductor::default();
        let _ = t.contenido(1.3, 5.0, 5.0);
        assert!(t.estado(Estado::Listo, Estado::Corriendo));
        // El `ZoomToRect` devuelve una transformacion identidad: no mueve.
        assert_eq!(t.contenido(1.0, 0.0, 0.0), None);
        // Y el siguiente gesto es desplazar, no un pellizco heredado.
        assert!(matches!(
            t.contenido(1.0, 3.0, 0.0),
            Some(Salida::Desliz { .. })
        ));
    }

    #[test]
    fn un_listo_sin_movimiento_no_pide_volver() {
        // Caso negativo: el segundo READY (el del propio `ZoomToRect`) no
        // puede pedir otro `ZoomToRect`, o se encadenarian sin fin.
        let mut t = Traductor::default();
        assert!(!t.estado(Estado::Listo, Estado::Corriendo));
        assert!(!t.estado(Estado::Listo, Estado::Listo));
    }

    #[test]
    fn una_escala_cero_o_no_finita_se_ignora() {
        let mut t = Traductor::default();
        assert_eq!(t.contenido(0.0, 10.0, 10.0), None);
        assert_eq!(t.contenido(f32::NAN, 10.0, 10.0), None);
        assert_eq!(t.contenido(1.0, f32::INFINITY, 0.0), None);
        // Y no ensucia el estado: el siguiente paso cuenta desde el origen.
        assert_eq!(desliz(t.contenido(1.0, 1.0, 0.0)), (1.0, 0.0));
    }

    #[test]
    fn desliz_y_pellizco_caben_en_un_evento_eq_sin_perder_precision_util() {
        let d = Desliz::nuevo(3.53, -0.06);
        // Medio dieciseisavo de error como mucho.
        assert!((d.dx() - 3.53).abs() <= 1.0 / 32.0, "{}", d.dx());
        assert!((d.dy() - -0.06).abs() <= 1.0 / 32.0, "{}", d.dy());
        let p = Pellizco::nuevo(1.012_345, Punto { x: 4, y: 5 });
        assert!((p.factor() - 1.012_345).abs() < 1e-6);
    }

    /// DirectManipulation de verdad sobre una ventana que NUNCA se muestra:
    /// se crea, no late en reposo y se suelta sin dejar nada. Lo que no se
    /// puede probar sin dedos es el gesto en si (lo prueba el usuario).
    #[cfg(windows)]
    #[test]
    fn directmanipulation_se_engancha_a_una_ventana_oculta_y_no_late_en_reposo() {
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_POPUP,
        };
        use windows::core::w;
        // SAFETY: ventana de la clase de sistema STATIC, oculta, destruida
        // al final de la prueba en este mismo hilo.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!(""),
                WS_POPUP,
                0,
                0,
                200,
                100,
                None,
                None,
                None,
                None,
            )
        }
        .expect("ventana oculta");
        let g = GestosTactiles::nuevo(hwnd, 200, 100, Box::new(|_| {}))
            .expect("DirectManipulation deberia existir en Windows 10/11");
        assert!(!g.latiendo(), "en reposo no hay temporizador");
        // Un latido suelto (un WM_TIMER rezagado) no rompe nada.
        g.latido();
        drop(g);
        // SAFETY: la ventana es nuestra.
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }
}
