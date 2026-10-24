//! Icono de la bandeja del sistema y su menu contextual.
//!
//! El icono es la unica presencia visible de PixPin Max cuando no estas
//! capturando. Se retira en `Drop` para que cerrar la aplicacion no deje un
//! icono fantasma que solo desaparece al pasar el raton por encima.

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, GetSystemMetrics, HICON,
    IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTCOLOR, LR_SHARED, LoadIconW, LoadImageW, MF_POPUP,
    MF_SEPARATOR, MF_STRING, PostMessageW, SM_CXSMICON, SM_CYSMICON, SetForegroundWindow,
    TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
};
use windows::core::{HSTRING, PCWSTR, Result as WinResult};

use crate::ventana::WM_BANDEJA;

/// Identificador del icono dentro de nuestra propia ventana. Solo hay uno.
const ID_ICONO: u32 = 1;

thread_local! {
    /// La ventana duena del menu, creada la primera vez que se abre.
    static DUENA_MENU: std::cell::Cell<Option<isize>> = const { std::cell::Cell::new(None) };
}

/// Una ventana normal, invisible y sin boton en la barra de tareas, que solo
/// sirve para que el menu tenga duena.
///
/// El icono avisa a la ventana `HWND_MESSAGE` del bucle, pero una ventana
/// solo-mensajes NO puede pasar a primer plano: `SetForegroundWindow` falla
/// sin decir nada y `TrackPopupMenu` se queda esperando sin ensenar el menu
/// hasta que el programa se activa por otra via. El usuario lo vio asi: el
/// clic derecho no hacia nada y el menu salia de golpe al empezar una
/// captura con Alt. Una ventana de verdad, aunque oculta, si recibe el
/// primer plano que la Shell concede al pulsar el icono.
fn duena_del_menu() -> Option<HWND> {
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, RegisterClassExW, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_POPUP,
    };
    use windows::core::w;

    if let Some(h) = DUENA_MENU.with(|d| d.get()) {
        return Some(HWND(h as *mut _));
    }
    extern "system" fn proc_duena(
        hwnd: HWND,
        mensaje: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> windows::Win32::Foundation::LRESULT {
        // SAFETY: delegar en el procedimiento por defecto con los mismos
        // argumentos que Windows paso.
        unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
    }
    // SAFETY: modulo propio; la clase apunta a un procedimiento estatico.
    // Registrarla dos veces falla sin efecto, y CreateWindowExW informa del
    // fallo real si lo hubiera.
    let hwnd = unsafe {
        let instancia = GetModuleHandleW(None).ok()?;
        let clase = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc_duena),
            hInstance: instancia.into(),
            lpszClassName: w!("PixPinDuenaMenuBandeja"),
            ..Default::default()
        };
        let _ = RegisterClassExW(&clase);
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            w!("PixPinDuenaMenuBandeja"),
            w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instancia.into()),
            None,
        )
        .ok()?
    };
    DUENA_MENU.with(|d| d.set(Some(hwnd.0 as isize)));
    Some(hwnd)
}

/// Textos del menu, ya traducidos por el catalogo Fluent.
pub struct EtiquetasMenu {
    /// Las entradas del menu, cada una con el identificador que llegara en
    /// `Evento::Menu` y su titulo ya traducido. Las monta quien tiene el
    /// catalogo de comandos: la bandeja solo las pinta, y asi anadir una
    /// funcion no obliga a tocar este crate.
    pub acciones: Vec<(u32, String)>,
    /// La ultima entrada va separada del resto: es la de cerrar, y no debe
    /// pulsarse por inercia al buscar otra cosa.
    pub aparte: Option<(u32, String)>,
    /// Titulo de la seccion de grupos ocultos, ya traducido.
    pub grupos_ocultos: String,
    /// Un grupo oculto: su identificador y la etiqueta ya montada
    /// («● Verde (3)»). Vacio = la seccion no aparece.
    pub ocultos: Vec<(u32, String)>,
}

pub struct Bandeja {
    datos: NOTIFYICONDATAW,
}

/// Copia `titulo` a `destino` como UTF-16, dejando sitio para el cero final.
///
/// `destino` tiene 128 posiciones; se usan como maximo 127 para dejar la
/// ultima siempre a cero (el NUL que Windows espera al final de `szTip`).
///
/// La capacidad se comprueba por caracter completo (`char::len_utf16`), no
/// por unidad UTF-16 suelta: un caracter fuera del plano basico multilingue
/// ocupa dos unidades (una pareja subrogada), y si no caben las dos enteras
/// en el hueco que queda, el caracter no se escribe en absoluto. Cortar a
/// mitad de una pareja subrogada dejaria el sustituto alto suelto justo
/// antes del cero final: una secuencia UTF-16 invalida aunque el array
/// siguiera terminando en NUL.
/// Copia el texto a un campo de Win32, cortando por donde quepa y
/// dejando el cero final.
///
/// Toma una rodaja y no un array de 128 porque los campos del aviso
/// emergente miden otra cosa: el titulo 64 y el texto 256. Cortar por el
/// tamano del destino y no por un numero escrito a mano es lo unico que
/// evita pisar memoria ajena cuando el campo es mas pequeno.
fn copiar_titulo(destino: &mut [u16], titulo: &str) {
    let tope = destino.len().saturating_sub(1);
    let mut escritos = 0usize;
    for caracter in titulo.chars() {
        let ancho = caracter.len_utf16();
        if escritos + ancho > tope {
            break;
        }
        let mut buf = [0u16; 2];
        for unidad in caracter.encode_utf16(&mut buf) {
            destino[escritos] = *unidad;
            escritos += 1;
        }
    }

    // Aseguramos el cero final explicitamente en vez de asumir que `destino`
    // ya llegaba a cero: esta funcion no debe depender de que su llamador lo
    // haya preinicializado.
    for slot in destino.iter_mut().skip(escritos) {
        *slot = 0;
    }
}

/// Carga el icono propio de PixPin Max: el recurso entero 1, que
/// `apps/pixpin/pixpinmax.rc` incrusta como `1 ICON recursos/pixpinmax.ico`.
///
/// Si no se puede cargar -- en particular en los tests de este mismo crate,
/// que se compilan sin ese `.rc` enlazado, porque solo el binario final de
/// `apps/pixpin` lo incrusta -- se cae en el icono generico de Windows
/// (`IDI_APPLICATION`) en vez de fallar: un icono de bandeja generico es
/// muchisimo mejor que no tener bandeja en absoluto.
fn cargar_icono_app() -> WinResult<HICON> {
    if let Some(icono) = cargar_icono_incrustado() {
        return Ok(icono);
    }

    // SAFETY: IDI_APPLICATION es un icono del sistema siempre disponible;
    // pasar None como instancia indica que es predefinido.
    unsafe { LoadIconW(None, IDI_APPLICATION) }
}

/// El recurso 1 del propio ejecutable, si existe. `None` si el modulo actual
/// no lo tiene (p. ej. en un binario de pruebas sin `.rc` enlazado).
fn cargar_icono_incrustado() -> Option<HICON> {
    // SAFETY: GetModuleHandleW(None) devuelve el modulo del proceso actual,
    // que siempre existe mientras el proceso vive.
    let instancia = unsafe { GetModuleHandleW(None) }.ok()?;

    // `identificador_recurso` nunca se dereferencia: Win32 (la tecnica
    // MAKEINTRESOURCE) usa el valor entero de la direccion, no una cadena,
    // para pasar un identificador de recurso -- aqui 1, que es con el que
    // `pixpinmax.rc` incrusta el icono. `ptr::without_provenance` es la
    // forma moderna y explicita de construir ese puntero-numero sin que
    // parezca (ni a Clippy) un puntero colgante por error de tipeo.
    let identificador_recurso: *const u16 = std::ptr::without_provenance(1);

    // LR_SHARED es obligatorio, no opcional: sin el, LoadImageW devuelve un
    // HICON del que el llamante es propietario y que hay que liberar con
    // DestroyIcon. La primera version de este arreglo (revision final,
    // hallazgo 4) no lo tenia y fugaba un HICON en cada Bandeja::nueva,
    // porque Bandeja::drop solo hace NIM_DELETE, nunca DestroyIcon; la
    // re-revision lo encontro ejecutando. Con LR_SHARED el sistema cachea el
    // icono y conserva su propiedad -- el mismo motivo por el que
    // IDI_APPLICATION (via LoadIconW, mas abajo en cargar_icono_app) tampoco
    // necesita liberarse. Es un uso estandar de LR_SHARED: el icono se carga
    // desde un recurso del modulo (no de un fichero) a un tamaño de sistema
    // fijo, exactamente el caso para el que MSDN lo recomienda.
    // SAFETY: `instancia` es el modulo de este mismo proceso, valido durante
    // toda la llamada. `identificador_recurso` es el puntero-numero
    // construido justo arriba, valido como MAKEINTRESOURCE.
    let cargado = unsafe {
        LoadImageW(
            Some(instancia.into()),
            PCWSTR(identificador_recurso),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR | LR_SHARED,
        )
    };

    let handle = cargado.ok()?;
    Some(HICON(handle.0))
}

impl Bandeja {
    pub fn nueva(hwnd: HWND, titulo: &str) -> WinResult<Self> {
        let icono = cargar_icono_app()?;

        let mut datos = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: ID_ICONO,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_BANDEJA,
            hIcon: icono,
            ..Default::default()
        };

        copiar_titulo(&mut datos.szTip, titulo);

        // SAFETY: `datos` esta completamente inicializada y su cbSize es
        // correcto. El llamante de `Bandeja::nueva` debe garantizar que
        // `hwnd` (y por tanto `datos.hWnd`) es una ventana valida de este
        // proceso; esta funcion lo recibe sin comprobarlo.
        unsafe {
            Shell_NotifyIconW(NIM_ADD, &datos).ok()?;
        }

        Ok(Self { datos })
    }

    /// Cambia el texto que sale al pasar el raton por el icono.
    ///
    /// Es como se dice que los atajos estan silenciados (P1.7). No hay
    /// un segundo icono en el ejecutable, y anadirlo por un estado que
    /// dura un rato no compensa; el aviso emergente de abajo es lo que
    /// se ve en el momento, y esto es lo que queda para comprobarlo
    /// despues sin abrir el menu.
    pub fn poner_titulo(&mut self, titulo: &str) -> WinResult<()> {
        copiar_titulo(&mut self.datos.szTip, titulo);
        self.datos.uFlags = NIF_TIP;
        // SAFETY: `datos` sigue siendo la misma estructura que se dio de
        // alta, con su cbSize y su uID; solo cambia el texto.
        let r = unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.datos).ok() };
        // Se devuelven las banderas de siempre: si se quedaran en NIF_TIP,
        // el siguiente NIM_MODIFY perderia el icono y el mensaje.
        self.datos.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        r
    }

    /// Un aviso emergente junto al reloj.
    ///
    /// Windows puede silenciarlos (en «Asistente de concentracion» o en
    /// los ajustes de notificaciones), asi que esto NO puede ser la unica
    /// forma de enterarse de algo. Vale para confirmar lo que el usuario
    /// acaba de pedir, no para pedirle nada.
    pub fn avisar(&mut self, titulo: &str, texto: &str) -> WinResult<()> {
        copiar_titulo(&mut self.datos.szInfoTitle, titulo);
        copiar_titulo(&mut self.datos.szInfo, texto);
        self.datos.uFlags = NIF_INFO;
        // SAFETY: igual que arriba; los dos textos acaban de escribirse.
        let r = unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.datos).ok() };
        self.datos.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        r
    }

    /// Muestra el menu contextual donde este el raton.
    pub fn mostrar_menu(&self, hwnd: HWND, etiquetas: &EtiquetasMenu) -> WinResult<()> {
        // SAFETY: no se pasan argumentos que dependan de memoria ajena; crea
        // un menu nuevo del que esta funcion es responsable de destruir.
        let menu = unsafe { CreatePopupMenu()? };

        // A partir de aqui el menu ya existe y debe destruirse siempre, tanto
        // si el resto de la funcion tiene exito como si no. Por eso los pasos
        // intermedios devuelven su resultado a traves de esta clausura en vez
        // de propagar el error con `?` directamente desde el cuerpo de la
        // funcion, que saltaria la llamada a DestroyMenu de mas abajo: es el
        // mismo defecto que la revision de la Tarea 6 encontro en appdata().
        let resultado = (|| -> WinResult<()> {
            // SAFETY: `menu` es el recien creado arriba y sigue vivo durante
            // todo este cierre; las cadenas se pasan como HSTRING, que
            // gestiona su propia memoria durante la llamada.
            unsafe {
                for (id, titulo) in &etiquetas.acciones {
                    AppendMenuW(
                        menu,
                        MF_STRING,
                        *id as usize,
                        &HSTRING::from(titulo.as_str()),
                    )?;
                }
                // Los grupos ocultos, si los hay: es la UNICA via de vuelta
                // para unos pines que ya no estan en pantalla (D24).
                if !etiquetas.ocultos.is_empty() {
                    AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
                    let sub = CreatePopupMenu()?;
                    for (id, etiqueta) in &etiquetas.ocultos {
                        AppendMenuW(
                            sub,
                            MF_STRING,
                            (crate::ventana::ID_MENU_GRUPO_BASE + id) as usize,
                            &HSTRING::from(etiqueta.as_str()),
                        )?;
                    }
                    // El submenu pasa a ser del padre: se destruye con el.
                    AppendMenuW(
                        menu,
                        MF_POPUP,
                        sub.0 as usize,
                        &HSTRING::from(etiquetas.grupos_ocultos.as_str()),
                    )?;
                }

                if let Some((id, titulo)) = &etiquetas.aparte {
                    AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
                    AppendMenuW(
                        menu,
                        MF_STRING,
                        *id as usize,
                        &HSTRING::from(titulo.as_str()),
                    )?;
                }
            }

            let mut punto = POINT::default();
            // SAFETY: `punto` es una estructura propia y valida a la que
            // escribir.
            unsafe {
                GetCursorPos(&mut punto)?;
            }

            // Sin esta llamada el menu no se cierra al hacer clic fuera. Es
            // un requisito documentado de TrackPopupMenu que se olvida a
            // menudo, y tiene que ir antes de TrackPopupMenu para que surta
            // efecto. La duena es la ventana oculta, no `hwnd` (ver
            // `duena_del_menu`).
            let duena = duena_del_menu().unwrap_or(hwnd);
            // SAFETY: `duena` es una ventana propia, viva mientras dure la
            // llamada.
            let _ = unsafe { SetForegroundWindow(duena) };

            // Con TPM_RETURNCMD el menu devuelve lo elegido en vez de mandar
            // WM_COMMAND a su duena, que no tiene bucle propio. Se reenvia a
            // la ventana del bucle como si viniera de alli: el camino de
            // siempre, `Evento::Menu`.
            // SAFETY: `menu` sigue vivo, `duena` es una ventana propia y
            // `punto` ya se ha inicializado con GetCursorPos.
            let elegido = unsafe {
                TrackPopupMenu(
                    menu,
                    TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
                    punto.x,
                    punto.y,
                    None,
                    duena,
                    None,
                )
            };
            if elegido.0 != 0 {
                // SAFETY: `hwnd` es la ventana del bucle, viva; WM_COMMAND
                // lleva el identificador en la palabra baja de wParam.
                let _ = unsafe {
                    PostMessageW(
                        Some(hwnd),
                        windows::Win32::UI::WindowsAndMessaging::WM_COMMAND,
                        WPARAM(elegido.0 as usize),
                        LPARAM(0),
                    )
                };
            }
            let hwnd = duena;

            // El otro medio del workaround documentado por Microsoft para
            // TrackPopupMenu con iconos de notificacion (el primero es el
            // SetForegroundWindow de mas arriba, que va antes). Sin este
            // WM_NULL de mas, en ciertas combinaciones el menu puede no
            // cerrarse del todo si el usuario no mueve el raton tras hacer
            // clic en un elemento o fuera del menu. Tiene que ir despues de
            // TrackPopupMenu, no antes.
            // SAFETY: `hwnd` es la ventana propia, valida mientras dure la
            // llamada; WM_NULL no lleva ningun payload que pueda ser
            // invalido.
            let _ = unsafe { PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0)) };

            Ok(())
        })();

        // SAFETY: `menu` viene de CreatePopupMenu de mas arriba y todavia no
        // se ha destruido; se destruye aqui exactamente una vez, tanto si el
        // cierre anterior tuvo exito como si fallo a mitad, para no dejar un
        // handle de menu fugado.
        unsafe {
            let _ = DestroyMenu(menu);
        }

        resultado
    }
}

impl Drop for Bandeja {
    fn drop(&mut self) {
        // SAFETY: `datos` describe un icono añadido por nosotros con NIM_ADD y
        // aun no retirado; este tipo no es Clone ni Copy, asi que se retira
        // exactamente una vez. Ademas, quien posea este valor debe
        // garantizar que `datos.hWnd` sigue siendo una ventana viva en este
        // momento: NIM_DELETE contra un HWND ya destruido no falla de forma
        // ruidosa, simplemente no limpia nada. Por eso en `main()` el orden
        // de declaracion deja que `Bandeja` se suelte antes que
        // `VentanaMensajes` (Rust destruye en orden inverso al de
        // declaracion).
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.datos);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ventana::VentanaMensajes;

    #[test]
    fn copiar_titulo_no_parte_una_pareja_subrogada() {
        // El caso exacto que reprodujo la revision: 126 caracteres ASCII
        // (llenan justo hasta la posicion 125, escritos=126) seguidos de un
        // emoji de una sola pareja subrogada, cuyas dos unidades UTF-16 caen
        // a caballo del corte de 127.
        let mut titulo = String::new();
        for _ in 0..126 {
            titulo.push('a');
        }
        titulo.push('\u{1F600}');

        let mut destino = [0u16; 128];
        copiar_titulo(&mut destino, &titulo);

        assert_eq!(
            destino[127], 0,
            "szTip debe terminar en cero en la ultima posicion"
        );

        // Ninguna unidad debe quedar como sustituto alto o bajo suelto: todo
        // sustituto alto (0xD800..=0xDBFF) debe ir seguido de su pareja baja
        // (0xDC00..=0xDFFF), y ningun sustituto bajo debe aparecer sin un
        // alto justo delante.
        let mut i = 0;
        while i < destino.len() {
            let unidad = destino[i];
            if (0xD800..=0xDBFF).contains(&unidad) {
                let siguiente = destino.get(i + 1).copied().unwrap_or(0);
                assert!(
                    (0xDC00..=0xDFFF).contains(&siguiente),
                    "sustituto alto suelto en la posicion {i} (0x{unidad:04x}), \
                     seguido de 0x{siguiente:04x} en vez de su pareja baja"
                );
                i += 2;
            } else if (0xDC00..=0xDFFF).contains(&unidad) {
                panic!(
                    "sustituto bajo suelto en la posicion {i} (0x{unidad:04x}) \
                     sin un sustituto alto delante"
                );
            } else {
                i += 1;
            }
        }
    }

    #[test]
    // Shell_NotifyIconW(NIM_ADD) lo atiende el shell (Explorer) de una
    // sesion de escritorio interactiva; un runner de CI hospedado no suele
    // tener una, y la llamada puede fallar ahi de forma nada representativa
    // de un fallo real. Se ejecuta a mano con `cargo test -- --ignored`.
    #[ignore = "necesita una sesion de escritorio interactiva con Explorer. cargo test -- --ignored"]
    fn se_anade_y_se_retira_el_icono() {
        let v = VentanaMensajes::nueva().unwrap();
        let b = Bandeja::nueva(v.handle(), "PixPin Max — prueba").expect("deberia añadirse");
        drop(b);

        // Poder añadir un segundo icono tras retirar el primero demuestra que
        // el Drop hizo su trabajo y no quedo un fantasma en la bandeja.
        let otra = Bandeja::nueva(v.handle(), "PixPin Max — prueba 2").expect("y otra vez");
        drop(otra);
    }
}
