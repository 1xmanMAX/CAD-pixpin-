//! Un aviso del sistema junto al reloj, para cuando llega la hora de algo.
//!
//! ## Por que la bandeja y no `ToastNotification`
//!
//! `Windows.UI.Notifications.ToastNotification` es la notificacion moderna, la
//! que sigue en el Centro de notificaciones despues de desaparecer. A cambio
//! exige que el programa tenga un **AppUserModelID registrado**: o viene de una
//! instalacion empaquetada (MSIX), o hay que escribir un acceso directo en el
//! menu Inicio con esa propiedad puesta y llamar a `SetCurrentProcessExplicitAppUserModelID`
//! antes de nada. Sin eso, `CreateToastNotifier` falla con
//! `E_INVALIDARG`/`ELEMENT_NOT_FOUND` y el usuario no ve absolutamente nada.
//!
//! PixPin Max es un ejecutable suelto que se copia y se ejecuta: no tiene
//! instalador, y ponerle uno para que salga un globo no es el trato. Por eso se
//! usa `Shell_NotifyIcon` con `NIF_INFO`, que es la misma via que ya usa
//! [`crate::bandeja::Bandeja::avisar`], funciona sin registrar nada y en Windows
//! 10 y 11 el propio sistema la pinta **como un toast**, con su entrada en el
//! Centro de notificaciones incluida.
//!
//! ## Esto no puede ser la unica forma de enterarse
//!
//! Windows silencia estos avisos cuando el usuario tiene puesta la
//! concentracion, el modo presentacion o simplemente apagadas las
//! notificaciones del programa. Un recordatorio que solo sale por aqui es un
//! recordatorio que se pierde: quien llame debe ademas sacar el pin en
//! pantalla (`Pines::pinear_nota`), que es lo que hace el movil
//! (`pin/RecordatorioReceiver.kt`: «no una notificacion... el pin vuelve a la
//! pantalla»).

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{HICON, IDI_APPLICATION, LoadIconW, WM_NULL};
use windows::core::Result as WinResult;

/// El identificador con el que [`crate::bandeja::Bandeja`] da de alta su icono.
///
/// Esta repetido a proposito y no importado: el de `bandeja` es privado, y
/// abrirlo obligaria a tocar ese fichero, que en esta tanda lo lleva otro. Si
/// alguna vez cambia alli, hay que cambiarlo aqui; por eso vale 1, que es el
/// valor que nadie mueve, y por eso se dice en voz alta.
pub const ID_DE_LA_BANDEJA: u32 = 1;

/// Identificador del icono que [`Aviso::con_icono_propio`] da de alta. Tiene
/// que ser distinto del de la bandeja o el segundo alta pisa la primera.
const ID_PROPIO: u32 = 2;

/// Quien saca avisos emergentes junto al reloj.
pub struct Aviso {
    datos: NOTIFYICONDATAW,
    /// Si el icono lo dimos de alta nosotros y por tanto hay que retirarlo.
    propio: bool,
}

impl Aviso {
    /// Avisa **sobre el icono que la bandeja ya tiene puesto**.
    ///
    /// Es lo normal en el programa: no anade un segundo icono a la bandeja y no
    /// necesita el valor `Bandeja` prestado, asi que quien avisa no tiene que
    /// pedirlo por toda la pila de llamadas.
    ///
    /// `hwnd` tiene que ser la MISMA ventana con la que se creo la bandeja: el
    /// par (ventana, identificador) es lo que Windows usa para encontrar el
    /// icono. Con otra ventana la llamada falla sin efecto.
    pub fn sobre_la_bandeja(hwnd: HWND) -> Aviso {
        Aviso {
            datos: NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: ID_DE_LA_BANDEJA,
                ..Default::default()
            },
            propio: false,
        }
    }

    /// Da de alta un icono propio para poder avisar sin bandeja.
    ///
    /// Es la salida para una ventana suelta (una prueba a mano, o un arranque
    /// en el que la bandeja no pudo crearse). **Deja un icono mas en la barra**
    /// mientras viva este valor, asi que no se usa cuando hay bandeja.
    pub fn con_icono_propio(hwnd: HWND) -> WinResult<Aviso> {
        // SAFETY: `IDI_APPLICATION` es un recurso del propio sistema; con
        // `None` como modulo no se toca memoria ajena. El icono es compartido
        // y no hay que destruirlo.
        let icono = unsafe { LoadIconW(None, IDI_APPLICATION)? };

        let datos = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: ID_PROPIO,
            uFlags: NIF_ICON,
            // Sin mensaje de vuelta: este icono no tiene menu ni responde a
            // clics. WM_NULL es el «no me avises de nada».
            uCallbackMessage: WM_NULL,
            hIcon: HICON(icono.0),
            ..Default::default()
        };

        // SAFETY: `datos` esta entera y su `cbSize` es el correcto. El llamante
        // garantiza que `hwnd` es una ventana viva de este proceso.
        unsafe { Shell_NotifyIconW(NIM_ADD, &datos).ok()? };

        Ok(Aviso {
            datos,
            propio: true,
        })
    }

    /// Saca el globo con `titulo` y `texto`.
    ///
    /// Los dos textos se recortan a lo que caben los campos de Windows (63 y
    /// 255 caracteres utiles), sin partir por la mitad una pareja subrogada: un
    /// emoji a caballo del corte deja una unidad UTF-16 suelta y el shell pinta
    /// el rombo del caracter roto.
    pub fn mostrar(&mut self, titulo: &str, texto: &str) -> WinResult<()> {
        copiar(&mut self.datos.szInfoTitle, titulo);
        copiar(&mut self.datos.szInfo, texto);
        let antes = self.datos.uFlags;
        self.datos.uFlags = NIF_INFO;
        self.datos.dwInfoFlags = NIIF_INFO;
        // SAFETY: la estructura describe un icono dado de alta (por la bandeja
        // o por nosotros) y sigue con su `cbSize` y su `uID`; solo cambian los
        // dos textos del globo.
        let r = unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.datos).ok() };
        // Se devuelven las banderas de antes: dejarlas en NIF_INFO haria que el
        // siguiente NIM_MODIFY o el NIM_DELETE hablaran de otra cosa.
        self.datos.uFlags = antes;
        r
    }
}

impl Drop for Aviso {
    fn drop(&mut self) {
        if !self.propio {
            // El icono es de la bandeja: retirarlo aqui la dejaria sin el suyo.
            return;
        }
        // SAFETY: solo se llega aqui cuando este valor dio de alta el icono con
        // NIM_ADD. `Aviso` no es `Clone` ni `Copy`, asi que se retira una vez.
        // Quien lo posea debe soltarlo antes de destruir `datos.hWnd`.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.datos);
        }
    }
}

/// Copia `texto` a `destino` como UTF-16 dejando siempre el cero final.
///
/// Se mide por caracter completo (`char::len_utf16`) y no por unidad suelta:
/// asi un caracter de fuera del plano basico —un emoji— o entra entero o no
/// entra. Es la misma cuenta que hace `bandeja::copiar_titulo`, repetida aqui
/// porque aquella es privada de su modulo.
fn copiar(destino: &mut [u16], texto: &str) {
    destino.fill(0);
    if destino.is_empty() {
        return;
    }
    let cabe = destino.len() - 1;
    let mut escritos = 0usize;
    for c in texto.chars() {
        let largo = c.len_utf16();
        if escritos + largo > cabe {
            break;
        }
        escritos += c.encode_utf16(&mut destino[escritos..]).len();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_texto_corto_se_copia_entero_y_acaba_en_cero() {
        let mut destino = [0u16; 8];
        copiar(&mut destino, "hola");
        let leido = String::from_utf16_lossy(&destino[..4]);
        assert_eq!(leido, "hola");
        assert_eq!(destino[4], 0, "detras del texto va el cero");
    }

    #[test]
    fn un_texto_mas_largo_que_el_campo_se_recorta_sin_partir_el_emoji() {
        // 6 huecos utiles (el septimo es el cero). Cinco letras y un emoji, que
        // son dos unidades UTF-16: no cabe, asi que tiene que quedarse fuera
        // entero en vez de dejar medio suelto.
        let mut destino = [0u16; 7];
        copiar(&mut destino, "abcde\u{1F600}");
        assert_eq!(String::from_utf16_lossy(&destino[..5]), "abcde");
        assert_eq!(
            destino[5], 0,
            "el emoji no cabia entero: no debe quedar media pareja subrogada"
        );
        assert_eq!(destino[6], 0, "y el cero final sigue en su sitio");
    }

    #[test]
    fn un_destino_vacio_no_revienta() {
        let mut destino: [u16; 0] = [];
        copiar(&mut destino, "lo que sea");
    }

    #[test]
    fn el_texto_de_antes_no_asoma_por_detras_del_nuevo() {
        // El campo se reusa entre avisos: si no se limpiara, un aviso largo
        // seguido de uno corto dejaria la cola del primero detras del cero, y
        // eso se ve en cuanto alguien lee el campo entero.
        let mut destino = [0u16; 16];
        copiar(&mut destino, "recordatorio");
        copiar(&mut destino, "ya");
        assert!(
            destino[2..].iter().all(|u| *u == 0),
            "detras del texto nuevo tiene que estar todo a cero"
        );
    }
}
