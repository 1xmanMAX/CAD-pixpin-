//! pixpin-shell — ver docs/superpowers/specs/2026-08-09-pixpin-pc-master-design.md
//!
//! Este crate habla con el sistema operativo o con librerias C. El `unsafe`
//! esta permitido, pero cada bloque lleva su comentario `// SAFETY:`.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod abrir;
pub mod abrir_con;
pub mod abrir_con_otra;
pub mod arranque;
pub mod asociaciones;
/// Movido a `pixpin-sistema` (portable); el nombre de siempre sigue valiendo.
pub use pixpin_sistema::atajo;
pub mod atajos;
pub mod aviso;
pub mod bandeja;
pub mod caja_de_texto;
pub mod colocacion;
pub mod compartido;
pub mod compartir;
pub mod consola;
pub mod dialogo;
pub mod elegir;
pub mod elegir_carpeta;
pub mod encima;
pub mod entorno;
pub mod entrada;
pub mod espacio_explorador;
pub mod explorador;
pub mod exportar;
pub mod gestos;
pub mod gestos_tactiles;
pub mod guardar;
pub mod hechos;
pub mod impresora;
pub mod imprimir;
pub mod instancia;
pub mod jump_list;
pub mod juntar;
pub mod marco_zona;
pub mod mdns;
pub mod mensajero;
pub mod menu_rico;
pub mod notificacion;
pub mod orden;
pub mod overlay;
pub mod pantalla_de;
pub mod papelera;
pub mod pegar_en_flow;
pub mod powerpoint;
pub mod primer_plano;
pub mod prioridad;
pub mod puntero;
pub mod regional;
pub mod reproductor_sistema;
pub mod subir;
pub mod uia;
/// Movido a `pixpin-sistema` (portable); el nombre de siempre sigue valiendo.
pub use pixpin_sistema::union;
pub mod ventana;
pub mod ventanas_visibles;
pub mod vigia_carpeta;
pub mod vista_previa;

pub use abrir::{abrir, abrir_ubicacion};
pub use arranque::ErrorArranque;
pub use atajo::{Atajo, ErrorAtajo, Modificadores, Tecla};
pub use atajos::{
    AtajosRegistrados, ID_ANOTAR, ID_ANOTAR_CONGELADA, ID_COPIAR, ID_CUENTAGOTAS, ID_PIN,
    ID_REGION, ID_SCROLL, registrar,
};
pub use bandeja::{Bandeja, EtiquetasMenu};
pub use dialogo::{confirmar_destructivo, mostrar_error_fatal, preguntar};
pub use encima::{Fijada, alternar_ventana_bajo_el_cursor};
pub use entorno::{
    appdata, directorio_del_ejecutable, locale_del_sistema, posicion_del_cursor, tema_claro,
};
pub use entrada::{
    boton_del_raton_pulsado, escape_pulsado, modificadores, modificadores_pulsados, rueda_en,
};
pub use explorador::{ComDelHilo, seleccion_del_explorador};
pub use gestos::{GanchoRaton, PausaGestos, gesto_en_curso};
pub use instancia::{ErrorInstanciaUnica, InstanciaUnica, adquirir_instancia_unica};
pub use overlay::esperar_composicion;
pub use ventana::{
    BotonGesto, Continuar, Evento, VentanaMensajes, WM_BANDEJA, WM_GESTO, despertar,
};

/// Un menu contextual sencillo donde este el raton: cada entrada es su
/// identificador y su rotulo ya traducido. Devuelve el elegido, o `None` si
/// se cerro sin elegir.
///
/// Vive aqui y no en `pixpin-pin` porque ya lo quieren dos sitios (el pin
/// tiene el suyo, con submenu de grupos; el chat quiere uno llano). Las dos
/// trampas de siempre: el menu se destruye SIEMPRE, y `SetForegroundWindow`
/// va antes de `TrackPopupMenu` o el menu no se cierra al pulsar fuera.
pub fn menu_llano(
    hwnd: windows::Win32::Foundation::HWND,
    entradas: &[(u32, String)],
) -> Option<u32> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_STRING, SetForegroundWindow,
        TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
    };
    use windows::core::HSTRING;

    if entradas.is_empty() {
        return None;
    }
    // SAFETY: el menu se crea y se destruye aqui; las cadenas viven como
    // HSTRING durante la llamada y el hwnd es una ventana propia.
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        for (id, rotulo) in entradas {
            let texto = HSTRING::from(rotulo.as_str());
            if AppendMenuW(menu, MF_STRING, *id as usize, &texto).is_err() {
                let _ = DestroyMenu(menu);
                return None;
            }
        }
        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        let _ = SetForegroundWindow(hwnd);
        let elegido = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            p.x,
            p.y,
            None,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        if elegido.0 == 0 {
            None
        } else {
            Some(elegido.0 as u32)
        }
    }
}

/// Azar del sistema, el bueno: `BCryptGenRandom`.
///
/// Hace falta para el codigo de seis cifras de un envio y para el nonce del
/// canal cifrado. Un azar sacado del reloj seria adivinable, y de esos dos
/// numeros depende que nadie mas pueda conectarse al envio.
pub fn azar(destino: &mut [u8]) -> bool {
    use windows::Win32::Security::Cryptography::{
        BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
    };
    // SAFETY: el buffer es valido durante la llamada y su largo es el suyo;
    // con la bandera del RNG preferido no hace falta abrir un algoritmo.
    let estado = unsafe { BCryptGenRandom(None, destino, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    estado.is_ok()
}
