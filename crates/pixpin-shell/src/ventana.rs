//! La ventana invisible que recibe todo y el bucle que duerme.
//!
//! Es una ventana `HWND_MESSAGE`: no se dibuja, no sale en la barra de tareas
//! y no aparece con Alt+Tab, pero recibe mensajes. Es donde llegan `WM_HOTKEY`
//! y las notificaciones de la bandeja.
//!
//! **El bucle usa `GetMessageW`, que bloquea el hilo hasta que llega algo.**
//! Esa eleccion es la que cumple el objetivo de 0% de CPU en reposo del
//! presupuesto de rendimiento. Un bucle con `PeekMessageW` giraria sin parar y
//! se comeria la bateria de un portatil sin hacer nada.

use std::cell::RefCell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE,
    MSG, PostQuitMessage, RegisterClassExW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_APP, WM_HOTKEY, WNDCLASSEXW,
};
use windows::core::{Result as WinResult, w};

/// Mensaje propio para las notificaciones del icono de bandeja.
pub const WM_BANDEJA: u32 = WM_APP + 1;

/// Mensaje sin contenido cuyo unico fin es hacer girar el bucle principal.
///
/// Las ventanas de los pines tienen su propio WndProc y no producen eventos
/// de esta ventana, asi que lo que dejan pendiente (una peticion del menu,
/// un Ctrl+C) no se atenderia hasta que el usuario pulsara un atajo. Con
/// esto, el pin avisa y el bucle vacia su bandeja de entrada al momento.
pub const WM_DESPERTAR: u32 = WM_APP + 2;

/// Pide al bucle principal que de una vuelta. Seguro desde cualquier hilo.
pub fn despertar(hwnd: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
    // SAFETY: publicar un mensaje propio en la cola de una ventana propia;
    // PostMessageW no bloquea ni toca memoria del llamante.
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_DESPERTAR, WPARAM(0), LPARAM(0));
    }
}

/// Mensaje propio del gancho de raton (`gestos.rs`): Alt + boton pulsado
/// en cualquier sitio de la pantalla. wParam lleva el boton, lParam el
/// punto en pixeles fisicos (x en la palabra baja, y en la alta).
pub const WM_GESTO: u32 = WM_APP + 3;

/// El boton con el que arranco un gesto con Alt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonGesto {
    /// Alt + izquierdo: seleccionar y copiar directo.
    Izquierdo,
    /// Alt + derecho: seleccionar y pinear directo.
    Derecho,
    /// Alt + central: seleccionar y dejar un pin en vivo (D140).
    Central,
    /// Alt + doble clic central: abrir el anotador de pantalla (2026-09-28).
    /// No es un arrastre: el punto es donde se hizo el segundo clic.
    DobleCentral,
}

/// Primer identificador de la seccion «Grupos ocultos»: al elegir uno, el
/// numero de grupo sale de restar esta base (spec 4.3). Por debajo viven los
/// identificadores de los comandos, que los reparte el catalogo.
pub const ID_MENU_GRUPO_BASE: u32 = 200;

/// Primer identificador que YA NO es de un grupo oculto.
///
/// **Existe porque su ausencia costo un fallo mudo.** El tramo de los grupos
/// no tenia tope: era «todo lo que pase de [`ID_MENU_GRUPO_BASE`]». La
/// aplicacion puso su entrada «Editor» en el 900 dando por hecho que un
/// numero alto estaba libre, y cada clic se convertia en «muestra el grupo
/// 700» -900 menos 200-, que no existe. No fallaba nada, no avisaba nadie:
/// simplemente no pasaba nada al pulsar, y el brazo que abria el editor
/// nunca llego a alcanzarse.
///
/// Con tope, los identificadores de arriba vuelven a ser `Menu` y quien
/// quiera reservarse uno solo tiene que ponerlo aqui o mas arriba. Las
/// regiones guardadas viven en el 1000 y estaban a un paso del mismo
/// problema: hoy solo llegan por atajo -que no pasa por `WM_COMMAND`- pero
/// el dia que salieran al menu se habrian comido el mismo silencio.
pub const ID_MENU_GRUPO_TOPE: u32 = 900;

/// Lo que le puede pasar a la aplicacion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evento {
    /// Se pulso un atajo global. El numero es el identificador con el que se
    /// registro (ver `atajos.rs`).
    Atajo(u32),
    /// Se eligio una entrada del menu de la bandeja. El numero es el mismo
    /// identificador de comando que viaja en `Atajo`: una funcion, un
    /// numero, dos vias de llegada.
    Menu(u32),
    /// Se eligio un grupo oculto en la bandeja: vuelve a la pantalla (D24).
    MostrarGrupo(u32),
    /// Un pin dejo algo pendiente y pide una vuelta del bucle. No hay nada
    /// que hacer con el evento en si: el trabajo va tras el match.
    Despertar,
    /// Clic DERECHO en el icono de la bandeja: el menu de siempre.
    IconoPulsado,
    /// Clic IZQUIERDO en el icono de la bandeja: el panel de la bandeja
    /// (rediseno v2). El menu del derecho queda de respaldo.
    PanelBandeja,
    /// Alt + boton pulsado en la pantalla (gancho de raton): abrir la
    /// captura ya con el arrastre en marcha desde `punto` (fisico).
    Gesto {
        boton: BotonGesto,
        punto: pixpin_geom::Punto,
    },
    /// Otra copia del programa recibio ficheros («Abrir con», o arrastrar
    /// sobre el icono) y nos los pasa, porque solo puede correr una.
    AbrirFicheros(Vec<std::path::PathBuf>),
    /// Otro programa pide algo (`docs/protocolo-pedidos.md`): el JSON del
    /// pedido, ya mirado por encima (`mensajero::validar_pedido`). Lo
    /// interpreta y lo hace la aplicacion, en su bucle.
    Pedido(String),
}

/// Que hacer despues de atender un evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continuar {
    Si,
    No,
}

thread_local! {
    /// Cola de eventos traducidos por el `WndProc`.
    ///
    /// Se usa un `thread_local` en vez de guardar un puntero al callback en
    /// los datos de la ventana porque el `WndProc` es una funcion `extern
    /// "system"` que no puede capturar entorno, y porque toda la interaccion
    /// con ventanas ocurre en el hilo de interfaz por exigencia de Win32.
    ///
    /// Cada evento va con su hora (reloj de `GetTickCount`): un gesto que
    /// espero demasiado aqui ya no se atiende (`gestos::CADUCIDAD_GESTO_MS`).
    static PENDIENTES: RefCell<Vec<(Evento, u32)>> = const { RefCell::new(Vec::new()) };
}

/// Deja un evento en la cola del hilo, con la hora en que se produjo.
fn encolar(evento: Evento, hora: u32) {
    PENDIENTES.with(|p| p.borrow_mut().push((evento, hora)));
}

/// Si un evento sacado de la cola ya no debe atenderse: solo los gestos con
/// Alt caducan (ver `gestos::CADUCIDAD_GESTO_MS`). Un atajo o un clic en la
/// bandeja se atienden siempre, por tarde que lleguen.
fn caducado(evento: &Evento, hora: u32) -> bool {
    let Evento::Gesto { boton, punto } = evento else {
        return false;
    };
    let ahora = crate::gestos::reloj_ms();
    let en_curso = crate::gestos::es_el_gesto_en_curso(punto.x, punto.y);
    if !crate::gestos::gesto_caducado(hora, ahora, en_curso) {
        return false;
    }
    tracing::info!(
        ?boton,
        ?punto,
        espera_ms = crate::gestos::ms_entre(hora, ahora),
        "gesto caducado en la cola: no se reproduce"
    );
    true
}

pub struct VentanaMensajes {
    hwnd: HWND,
}

impl VentanaMensajes {
    pub fn nueva() -> WinResult<Self> {
        const CLASE: windows::core::PCWSTR = w!("PixPinMaxVentanaMensajes");

        // SAFETY: GetModuleHandleW(None) devuelve el modulo del proceso
        // actual, que siempre existe mientras el proceso vive.
        let instancia = unsafe { GetModuleHandleW(None)? };

        let clase = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: CLASE,
            ..Default::default()
        };

        // SAFETY: `clase` esta completamente inicializada y su `lpszClassName`
        // apunta a un literal estatico. Registrar la misma clase dos veces
        // devuelve 0 y pone ERROR_CLASS_ALREADY_EXISTS, que aqui es benigno:
        // significa que otra VentanaMensajes ya la registro. Si el fallo
        // fuera por otra razon, CreateWindowExW fallara a continuacion (la
        // clase no habria quedado registrada) y ese `?` es el que reporta el
        // error real; ningun fallo se pierde en silencio.
        unsafe {
            let _ = RegisterClassExW(&clase);
        }

        // SAFETY: la clase esta registrada (por esta llamada o por una
        // anterior) y HWND_MESSAGE es el padre valido para una ventana
        // solo-mensajes.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASE,
                w!("PixPin Max"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instancia.into()),
                None,
            )?
        };

        Ok(Self { hwnd })
    }

    pub fn handle(&self) -> HWND {
        self.hwnd
    }

    /// Bucle principal. Bloquea el hilo hasta que `al_recibir` devuelve
    /// `Continuar::No` o llega `WM_QUIT`.
    ///
    /// Toma `&self`, no `self`: si consumiera la ventana, este metodo la
    /// destruiria (via su `Drop`) en cuanto el bucle terminara, es decir
    /// antes de que `main` recupere el control. `bandeja` y los atajos
    /// registrados, que en `main` se declaran despues de la ventana y por
    /// tanto se sueltan antes en el orden inverso normal de Rust, se
    /// habrian soltado entonces contra un `HWND` ya destruido. Con `&self`
    /// la ventana sigue viva hasta que `main` termina de verdad, y el orden
    /// de caida natural (atajos, luego bandeja, luego ventana) es el
    /// correcto.
    pub fn ejecutar(&self, mut al_recibir: impl FnMut(Evento) -> Continuar) {
        let mut mensaje = MSG::default();
        loop {
            // SAFETY: `mensaje` es una estructura propia y valida.
            // GetMessageW devuelve >0 si hay mensaje, 0 al recibir WM_QUIT y
            // -1 si hay error; los dos ultimos casos se tratan igual aqui
            // porque `ejecutar` no devuelve Result y no hay nada mas que
            // hacer salvo dejar de bloquear el hilo.
            let resultado = unsafe { GetMessageW(&mut mensaje, None, 0, 0) };
            if resultado.0 <= 0 {
                break;
            }

            // SAFETY: `mensaje` viene de GetMessageW y es valido.
            unsafe {
                let _ = TranslateMessage(&mensaje);
                DispatchMessageW(&mensaje);
            }

            // Se drena en bucle, no de una sola vez. `DispatchMessageW` de
            // arriba puede haber ejecutado codigo que a su vez entra en un
            // bucle de mensajes anidado (TrackPopupMenu, en
            // bandeja.rs::mostrar_menu, es exactamente eso), y ese bucle
            // anidado puede repartir un WM_COMMAND que el WndProc encola en
            // PENDIENTES mientras el `al_recibir` de mas abajo todavia esta
            // en marcha para el evento que abrio el menu. Con un solo drain
            // ese evento quedaria en la cola hasta el siguiente mensaje de
            // Win32 -- que con `GetMessageW` puede tardar en llegar -- y la
            // aplicacion parece ignorar el clic. Volver a drenar aqui,
            // sin pasar otra vez por `GetMessageW`, lo recoge de inmediato.
            loop {
                let eventos: Vec<(Evento, u32)> =
                    PENDIENTES.with(|p| p.borrow_mut().drain(..).collect());
                if eventos.is_empty() {
                    break;
                }
                for (evento, hora) in eventos {
                    // La hora se mira AQUI, evento a evento, y no al sacar la
                    // tanda: lo que tarde en atenderse el anterior (un
                    // overlay entero) cuenta para el siguiente.
                    if caducado(&evento, hora) {
                        continue;
                    }
                    if al_recibir(evento) == Continuar::No {
                        return;
                    }
                }
            }
        }
    }
}

impl Drop for VentanaMensajes {
    fn drop(&mut self) {
        // SAFETY: `hwnd` viene de CreateWindowExW y no se ha destruido antes;
        // este tipo no es Clone ni Copy, asi que se destruye exactamente una
        // vez.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Traduce mensajes de Win32 a `Evento` y los deja en la cola del hilo.
///
/// No hace trabajo real: cuanto antes vuelva, mas fluido va todo lo demas.
extern "system" fn procedimiento(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{WM_COMMAND, WM_COPYDATA, WM_DESTROY};

    let evento = match mensaje {
        WM_HOTKEY => Some(Evento::Atajo(wparam.0 as u32)),
        // Ficheros que nos manda una segunda copia antes de irse.
        WM_COPYDATA => {
            // Un pedido de otro programa. Se contesta en el acto (1, 2 o 3)
            // y el trabajo queda para el bucle: quien manda esta parado en
            // su SendMessage hasta que volvamos.
            // SAFETY: dentro del procesamiento del mensaje, con la
            // estructura viva; la funcion copia el texto y no guarda nada.
            if let Some(leido) = unsafe { crate::mensajero::pedido_de_copydata(lparam) } {
                return match leido {
                    Ok(json) => {
                        encolar(Evento::Pedido(json), crate::gestos::reloj_ms());
                        LRESULT(crate::mensajero::respuesta::ACEPTADO)
                    }
                    Err(rechazo) => LRESULT(rechazo),
                };
            }
            // SAFETY: estamos DENTRO del procesamiento del mensaje, que es
            // exactamente cuando Windows garantiza que la estructura sigue
            // viva. La funcion copia lo que necesita y no guarda el puntero.
            let rutas = unsafe { crate::mensajero::ficheros_de_copydata(lparam) };
            if rutas.is_empty() {
                None
            } else {
                // Se contesta 1 para que el otro proceso sepa que llegaron
                // y no arranque una copia entera para nada.
                encolar(Evento::AbrirFicheros(rutas), crate::gestos::reloj_ms());
                return LRESULT(1);
            }
        }
        WM_COMMAND => evento_de_menu((wparam.0 & 0xFFFF) as u32),
        // Esta comparacion asume la semantica "clasica" de Shell_NotifyIconW,
        // donde lParam es directamente el mensaje del raton (WM_LBUTTONUP,
        // etc). Si en el futuro el icono de la bandeja se registra con
        // NOTIFYICON_VERSION_4 (via Shell_NotifyIconW + NIM_SETVERSION),
        // lParam pasa a llevar las coordenadas del cursor empaquetadas en la
        // palabra baja/alta en vez del mensaje, y esta igualdad exacta deja
        // de reconocer el clic sin dar ningun error visible. Quien conecte
        // la bandeja (tarea futura) debe revisar esto si cambia la version
        // notificada.
        WM_DESPERTAR => Some(Evento::Despertar),
        WM_GESTO => {
            let boton = match wparam.0 {
                0 => BotonGesto::Izquierdo,
                1 => BotonGesto::Derecho,
                crate::gestos::GESTO_DOBLE_CENTRAL => BotonGesto::DobleCentral,
                _ => BotonGesto::Central,
            };
            // Las coordenadas van en dos palabras de 16 bits con signo: un
            // monitor a la izquierda del principal tiene x negativa.
            let x = (lparam.0 & 0xFFFF) as u16 as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
            Some(Evento::Gesto {
                boton,
                punto: pixpin_geom::Punto { x, y },
            })
        }
        WM_BANDEJA => evento_de_bandeja(lparam.0 as u32),
        WM_DESTROY => {
            // SAFETY: llamada sin argumentos que solo encola WM_QUIT en la
            // cola de mensajes de este hilo; no toca memoria ajena.
            unsafe { PostQuitMessage(0) };
            None
        }
        _ => None,
    };

    if let Some(evento) = evento {
        // La hora de un gesto es la de su `PostMessage` (la del gancho), no
        // la de ahora: si el hilo estaba ocupado sin bombear, ya va tarde.
        let hora = if mensaje == WM_GESTO {
            crate::gestos::hora_del_mensaje()
        } else {
            crate::gestos::reloj_ms()
        };
        encolar(evento, hora);
        // Un mensaje ENVIADO desde otro hilo (la Shell manda asi el aviso del
        // icono de la bandeja) se atiende DENTRO de GetMessageW, que no
        // vuelve: el evento quedaba en la cola hasta que llegara un mensaje
        // encolado cualquiera. El usuario lo vivia como que el clic derecho
        // no abria el menu y este salia de golpe al hacer un gesto con Alt.
        // Un WM_NULL encolado hace volver a GetMessageW y el bucle drena.
        // SAFETY: una consulta sin argumentos y un PostMessage a la ventana
        // propia, viva mientras se procesa su propio mensaje.
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{InSendMessage, PostMessageW, WM_NULL};
            if InSendMessage().as_bool() {
                let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            }
        }
        return LRESULT(0);
    }

    // SAFETY: delegar en el procedimiento por defecto siempre es valido para
    // los mensajes que no tratamos; hwnd, mensaje, wparam y lparam son los
    // que Windows paso a este WndProc.
    unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
}

/// Saca de la cola los atajos globales pendientes y devuelve sus ids.
///
/// Lo usa el bucle modal del overlay: mientras un overlay esta abierto, un
/// atajo pulsado NO puede quedarse esperando en esta cola, porque al cerrar
/// el overlay se atenderia y lo volveria a abrir. En vez de eso se le
/// entrega al overlay, que decide (la capa viva alterna el modo pasante,
/// D50; el overlay de captura lo ignora).
/// Que evento produce elegir una entrada del menu, por su identificador.
///
/// El tramo `ID_MENU_GRUPO_BASE..ID_MENU_GRUPO_TOPE` es de los grupos
/// ocultos; **todo lo demas es una entrada normal**, incluidos los numeros
/// por encima del tope. Que el tramo tenga los dos extremos y no solo el de
/// abajo es lo que impide que una entrada de arriba se convierta en silencio
/// en «muestra un grupo que no existe»: ver [`ID_MENU_GRUPO_TOPE`].
///
/// El cero no es una entrada: Windows lo manda por otras razones.
///
/// Vive aparte de `procedimiento` por lo mismo que `evento_de_bandeja`:
/// aquella pide un `HWND` de verdad y esta es una decision pura sobre un
/// numero, asi que se puede probar sin escritorio.
fn evento_de_menu(id: u32) -> Option<Evento> {
    match id {
        0 => None,
        c if (ID_MENU_GRUPO_BASE..ID_MENU_GRUPO_TOPE).contains(&c) => {
            Some(Evento::MostrarGrupo(c - ID_MENU_GRUPO_BASE))
        }
        c => Some(Evento::Menu(c)),
    }
}

/// Que evento produce un clic sobre el icono de la bandeja.
///
/// `mensaje_raton` es lo que Windows deja en `lParam` con la semantica
/// «clasica» de `Shell_NotifyIconW`: el mensaje del raton tal cual.
///
/// **El derecho abre el menu, y no es un capricho:** en Windows el menu
/// contextual de un icono de bandeja se pide con el derecho, y esa es la
/// mano que va sola. Durante un tiempo aqui solo se atendio el izquierdo,
/// asi que quien hacia lo que hace todo el mundo no obtenia nada -ni menu,
/// ni error, ni pista- y el programa parecia muerto. El izquierdo abre el
/// panel de la bandeja (rediseno v2), como el centro de control de Windows.
///
/// Cualquier otro mensaje -pasar por encima, mover, doble clic- no es
/// asunto nuestro: se devuelve `None` y lo atiende el procedimiento por
/// defecto.
///
/// Vive aparte de `procedimiento` para poder probarse: aquella es una
/// `extern "system"` que pide un `HWND` de verdad, y esta es una decision
/// pura sobre un numero.
fn evento_de_bandeja(mensaje_raton: u32) -> Option<Evento> {
    use windows::Win32::UI::WindowsAndMessaging::{WM_LBUTTONUP, WM_RBUTTONUP};
    match mensaje_raton {
        WM_RBUTTONUP => Some(Evento::IconoPulsado),
        WM_LBUTTONUP => Some(Evento::PanelBandeja),
        _ => None,
    }
}

pub fn tomar_atajos_pendientes() -> Vec<u32> {
    PENDIENTES.with(|p| {
        let mut cola = p.borrow_mut();
        let mut ids = Vec::new();
        cola.retain(|(e, _)| match e {
            Evento::Atajo(id) => {
                ids.push(*id);
                false
            }
            _ => true,
        });
        ids
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::IsWindow;

    /// Un pedido de verdad, por `WM_COPYDATA`, a una ventana de mensajes de
    /// la prueba: contesta en el acto y deja el JSON en la cola del hilo.
    ///
    /// Se manda con `enviar_pedido_a` a la ventana PROPIA y no con
    /// `enviar_pedido`, que busca por la clase: con la PixPin del usuario
    /// abierta, la prueba le escribiria en el chat.
    #[test]
    fn un_pedido_llega_por_copydata_y_queda_en_la_cola() {
        use crate::mensajero::{enviar_pedido_a, respuesta};
        let v = VentanaMensajes::nueva().expect("deberia poder crearse");
        // Lo que hubiera de otra prueba en este hilo no cuenta.
        PENDIENTES.with(|p| p.borrow_mut().clear());

        let bueno = r#"{"pixpin":1,"accion":"chat","texto":"hola ñu"}"#;
        assert_eq!(enviar_pedido_a(v.handle(), bueno), respuesta::ACEPTADO);
        // Casos negativos: se rechazan en el acto y NO llegan al bucle.
        assert_eq!(
            enviar_pedido_a(v.handle(), r#"{"pixpin":9,"accion":"chat"}"#),
            respuesta::VERSION_NO
        );
        assert_eq!(
            enviar_pedido_a(v.handle(), r#"{"pixpin":1,"accion":"nada"}"#),
            respuesta::NO_SE_ENTIENDE
        );
        assert_eq!(
            enviar_pedido_a(v.handle(), "{roto"),
            respuesta::NO_SE_ENTIENDE
        );

        let en_cola: Vec<Evento> =
            PENDIENTES.with(|p| p.borrow_mut().drain(..).map(|(e, _)| e).collect());
        assert_eq!(en_cola, vec![Evento::Pedido(bueno.to_string())]);
    }

    /// El fallo del 10-oct: los gestos que esperaron segundos en la cola
    /// salian todos al liberarse el hilo. El bucle los salta; lo reciente y
    /// lo que no es un gesto (un atajo, por viejo que sea) llega igual.
    #[test]
    fn el_bucle_salta_los_gestos_caducados_y_atiende_lo_demas() {
        use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_NULL};
        let v = VentanaMensajes::nueva().expect("deberia poder crearse");
        PENDIENTES.with(|p| p.borrow_mut().clear());
        let ahora = crate::gestos::reloj_ms();
        let gesto = |x| Evento::Gesto {
            boton: BotonGesto::Derecho,
            punto: pixpin_geom::Punto { x, y: 10 },
        };
        encolar(gesto(1), ahora.wrapping_sub(9157));
        encolar(gesto(2), ahora);
        encolar(gesto(3), ahora.wrapping_sub(2016));
        encolar(Evento::Atajo(7), ahora.wrapping_sub(60_000));
        // Algo en la cola de Win32 para que GetMessageW vuelva y se drene.
        // SAFETY: mensaje nulo a la ventana propia, viva.
        unsafe {
            let _ = PostMessageW(Some(v.handle()), WM_NULL, WPARAM(0), LPARAM(0));
        }
        let mut atendidos = Vec::new();
        v.ejecutar(|e| {
            let fin = matches!(e, Evento::Atajo(_));
            atendidos.push(e);
            if fin { Continuar::No } else { Continuar::Si }
        });
        assert_eq!(atendidos, vec![gesto(2), Evento::Atajo(7)]);
    }

    #[test]
    fn se_crea_y_se_destruye_sin_fugas() {
        let v = VentanaMensajes::nueva().expect("deberia poder crearse");
        let hwnd = v.handle();
        assert!(!hwnd.is_invalid(), "el handle no puede ser invalido");

        // SAFETY: `hwnd` viene de CreateWindowExW y sigue vivo (v todavia no
        // se ha soltado). Consultar con IsWindow un handle vivo es siempre
        // valido y no toma posesion de nada.
        let sigue_viva = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            sigue_viva,
            "la ventana debe existir para Windows mientras v esta viva"
        );

        drop(v);

        // Este es el aserto que hace real el nombre del test: comprueba que
        // Drop destruyo de verdad la ventana, no solo que el handle guardado
        // dejo de ser invalido (eso ya lo era desde el principio y no lo
        // cambia soltar `v`). Sin esto, un Drop que nunca llamara a
        // DestroyWindow pasaria el test igual.
        //
        // SAFETY: `hwnd` es el valor numerico de un handle que ya se destruyo;
        // consultar un handle destruido con IsWindow es una operacion valida
        // que precisamente sirve para comprobar que ya no existe, no un uso
        // despues de liberar memoria (IsWindow no dereferencia el puntero,
        // solo busca la entrada en la tabla de ventanas del sistema).
        let sigue_reconocida = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            !sigue_reconocida,
            "tras soltar v, Windows ya no debe reconocer el handle"
        );

        // Crear una segunda tras destruir la primera comprueba que la clase de
        // ventana se registra de forma reentrante y que no queda basura.
        let otra = VentanaMensajes::nueva().expect("la segunda tambien");
        assert!(!otra.handle().is_invalid());
    }

    /// Comprueba la PRECONDICION de la que depende el arreglo del hallazgo
    /// 2, no el arreglo en si.
    ///
    /// Importante, para no prometer mas de lo que da: este test **no llama
    /// a `ejecutar`**. Suelta `registrados`, `bandeja` y `ventana` a mano,
    /// en el mismo orden en que `main()` los declara. Con eso comprueba,
    /// via `IsWindow` en cada paso de la caida, que ese orden de
    /// declaracion por si solo -- sin que nada mueva `ventana` fuera de su
    /// sitio -- ya basta para que Rust suelte hotkeys -> bandeja -> ventana
    /// (la ventana sigue viva cuando `Bandeja` y `AtajosRegistrados` se
    /// sueltan, y solo deja de existir cuando `VentanaMensajes` se suelta al
    /// final). Pasaria igual con la firma antigua `ejecutar(self, ...)`,
    /// porque esa firma no interviene aqui en absoluto.
    ///
    /// Lo que de verdad arreglo el hallazgo 2 -- que `ejecutar` tomaba
    /// `self` por valor, asi que devolver del bucle destruia la ventana
    /// *dentro* de esa llamada, antes de que `main` recuperase el control,
    /// y `bandeja`/`atajos` se soltaban entonces contra un `HWND` ya muerto
    /// -- es la firma `pub fn ejecutar(&self, ...)` de mas arriba en este
    /// mismo fichero; no hay un test que ejercite ese camino exacto (haria
    /// falta lanzar el bucle real y cerrarlo desde dentro). Este test cubre
    /// la mitad verificable sin eso: que la precondicion sobre la que se
    /// apoya el arreglo es cierta.
    ///
    /// Necesita una sesion de escritorio interactiva (bandeja de Explorer,
    /// RegisterHotKey), igual que las pruebas de `bandeja.rs` y
    /// `atajos.rs`. Se ejecuta a mano con `cargo test -- --ignored`.
    #[test]
    #[ignore = "necesita una sesion de escritorio interactiva. cargo test -- --ignored"]
    fn bandeja_y_atajos_se_sueltan_con_la_ventana_todavia_viva() {
        use crate::atajo::Atajo;
        use crate::atajos;
        use crate::bandeja::Bandeja;

        let ventana = VentanaMensajes::nueva().expect("deberia poder crearse");
        let hwnd = ventana.handle();

        let bandeja =
            Bandeja::nueva(hwnd, "PixPin Max — prueba de orden").expect("deberia añadirse");

        let raro: Atajo = "Ctrl+Alt+Shift+F21".parse().unwrap();
        let (registrados, fallidos) = atajos::registrar(hwnd, &[(atajos::ID_REGION, raro)]);
        assert!(fallidos.is_empty(), "no deberia chocar: {fallidos:?}");

        // Orden de declaracion: ventana, bandeja, registrados. Soltamos en
        // el orden inverso que Rust usaria al final del scope (y que main()
        // usa de verdad), pero paso a paso para poder comprobar el estado
        // de la ventana entre medias.

        // SAFETY: `hwnd` sigue vivo (nada lo ha destruido todavia);
        // consultar con IsWindow un handle vivo es siempre valido.
        let viva_al_empezar = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            viva_al_empezar,
            "precondicion: la ventana debe existir antes de soltar nada"
        );

        drop(registrados);
        // SAFETY: `hwnd` sigue siendo la ventana de `ventana`, que todavia
        // no se ha soltado.
        let viva_tras_atajos = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            viva_tras_atajos,
            "hallazgo 2: tras soltar los atajos (UnregisterHotKey), la \
             ventana debe seguir existiendo"
        );

        drop(bandeja);
        // SAFETY: igual que arriba: `ventana` sigue viva.
        let viva_tras_bandeja = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            viva_tras_bandeja,
            "hallazgo 2: tras soltar la bandeja (Shell_NotifyIconW NIM_DELETE), \
             la ventana debe seguir existiendo"
        );

        drop(ventana);
        // SAFETY: `hwnd` es el valor numerico de un handle ya destruido;
        // consultarlo con IsWindow es la operacion valida que comprueba
        // precisamente eso.
        let reconocida_tras_ventana = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            !reconocida_tras_ventana,
            "tras soltar la ventana, ya no debe existir"
        );
    }
    /// El fallo real: el clic DERECHO sobre el icono de la bandeja no abria
    /// el menu, porque solo se atendia `WM_LBUTTONUP`. Quien hacia lo que se
    /// hace en Windows -pedir el menu contextual con el derecho- no obtenia
    /// nada: ni menu, ni error, ni pista. El programa parecia muerto.
    #[test]
    fn el_derecho_abre_el_menu_y_el_izquierdo_el_panel() {
        use windows::Win32::UI::WindowsAndMessaging::{WM_LBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONUP};

        assert_eq!(
            evento_de_bandeja(WM_RBUTTONUP),
            Some(Evento::IconoPulsado),
            "el derecho es con el que se pide un menu contextual en Windows"
        );
        assert_eq!(
            evento_de_bandeja(WM_LBUTTONUP),
            Some(Evento::PanelBandeja),
            "el izquierdo abre el panel de la bandeja (v2); el menu sigue en el derecho"
        );
        // Pasar el raton por encima llega como mensaje igual que un clic, y
        // abrir el menu al pasar por encima seria peor que no abrirlo nunca.
        assert_eq!(evento_de_bandeja(WM_MOUSEMOVE), None);
    }

    /// El fallo que dejo el editor inalcanzable durante toda una fase.
    ///
    /// El tramo de los grupos ocultos no tenia tope -era «todo lo que pase
    /// de 200»- asi que la entrada «Editor», que la aplicacion puso en el
    /// 900, se convertia en `MostrarGrupo(700)`. El grupo 700 no existe, no
    /// fallaba nada y no avisaba nadie: al pulsar simplemente no pasaba nada.
    #[test]
    fn una_entrada_por_encima_del_tramo_de_grupos_no_se_confunde_con_un_grupo() {
        assert_eq!(
            evento_de_menu(900),
            Some(Evento::Menu(900)),
            "900 esta por encima del tramo de grupos: es una entrada normal"
        );
        // Las regiones guardadas viven en el 1000 y estaban a un paso del
        // mismo silencio: hoy solo llegan por atajo, pero el dia que salgan
        // al menu tienen que llegar como entrada.
        assert_eq!(evento_de_menu(1000), Some(Evento::Menu(1000)));
    }

    #[test]
    fn el_tramo_de_los_grupos_sigue_siendo_suyo() {
        assert_eq!(evento_de_menu(200), Some(Evento::MostrarGrupo(0)));
        assert_eq!(evento_de_menu(899), Some(Evento::MostrarGrupo(699)));
        // Justo por debajo del tramo estan los comandos del catalogo.
        assert_eq!(evento_de_menu(199), Some(Evento::Menu(199)));
        assert_eq!(evento_de_menu(3), Some(Evento::Menu(3)));
        // El cero no es una entrada: Windows lo manda por otras razones.
        assert_eq!(evento_de_menu(0), None);
    }
}
