//! **Los mandos de reproductor de Windows** para lo que se escucha en voz
//! alta (`SystemMediaTransportControls`).
//!
//! Es el equivalente en Windows de la notificacion de reproductor del movil
//! (`ui/LeyendoEnVozService.kt`, v0.89 `1e66199`: «que el audio continue al
//! salir y se ponga en pantalla como si estuviera reproduciendo un audio»).
//! Windows ensena estos mandos en el recuadro del volumen y en la pantalla
//! de bloqueo, con el titulo del documento, y les manda **las teclas
//! multimedia del teclado** (play/pausa, anterior, siguiente, parar): no hay
//! que registrar ninguna tecla.
//!
//! Una aplicacion de escritorio los pide por una ventana
//! (`ISystemMediaTransportControlsInterop::GetForWindow`). La lectura vive
//! fuera de las ventanas de los lectores —sigue con el lector cerrado—, asi
//! que se crea aqui una ventana propia que nunca se ensena, en el hilo que
//! lee. Los botones llegan en hilos de WinRT y se pasan por un canal: quien
//! los usa los recoge en su bucle ([`MandosDelSistema::botones`]).

use std::sync::mpsc::{self, Receiver};

use windows::Foundation::TypedEventHandler;
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
    SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE, PeekMessageW,
    RegisterClassExW, TranslateMessage, WINDOW_STYLE, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{HSTRING, factory, w};

/// Lo que se pulso en los mandos de Windows (o en el teclado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonDelSistema {
    Tocar,
    Pausar,
    Siguiente,
    Anterior,
    Parar,
}

/// El boton de Windows como uno de los nuestros; `None` si no se usa
/// (rebobinar, canales...).
pub fn boton_de(b: SystemMediaTransportControlsButton) -> Option<BotonDelSistema> {
    match b {
        SystemMediaTransportControlsButton::Play => Some(BotonDelSistema::Tocar),
        SystemMediaTransportControlsButton::Pause => Some(BotonDelSistema::Pausar),
        SystemMediaTransportControlsButton::Next => Some(BotonDelSistema::Siguiente),
        SystemMediaTransportControlsButton::Previous => Some(BotonDelSistema::Anterior),
        SystemMediaTransportControlsButton::Stop => Some(BotonDelSistema::Parar),
        _ => None,
    }
}

/// La ventana escondida a la que Windows cuelga los mandos.
unsafe extern "system" fn procedimiento(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: se reenvia tal cual lo que llego a esta ventana.
    unsafe { DefWindowProcW(h, m, w, l) }
}

/// **Los mandos, puestos.** Viven en el hilo que los creo: hay que llamar a
/// [`MandosDelSistema::bombear`] en su bucle. Al soltarse, se van de la
/// pantalla.
pub struct MandosDelSistema {
    hwnd: HWND,
    smtc: SystemMediaTransportControls,
    token: i64,
    botones: Receiver<BotonDelSistema>,
}

impl MandosDelSistema {
    /// Los crea, encendidos y sin titulo. `None` si Windows no los da (un
    /// Windows sin WinRT, o sin escritorio). COM/WinRT tiene que estar
    /// iniciado en el hilo.
    pub fn nuevos() -> Option<MandosDelSistema> {
        const CLASE: windows::core::PCWSTR = w!("PixPinMaxMandosDeVoz");
        // SAFETY: el modulo del proceso existe mientras el proceso vive.
        let instancia = unsafe { GetModuleHandleW(None) }.ok()?;
        let clase = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: CLASE,
            ..Default::default()
        };
        // SAFETY: clase completa con un nombre estatico. Registrarla dos
        // veces falla sin mas (ya esta), y si fallara de verdad lo dice
        // `CreateWindowExW` de abajo.
        unsafe {
            let _ = RegisterClassExW(&clase);
        }
        // SAFETY: la clase esta registrada; una ventana emergente sin
        // tamano que nunca se ensena (no una solo-mensajes: a esas Windows
        // no les da mandos).
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                CLASE,
                w!("PixPin Max"),
                WINDOW_STYLE(WS_POPUP.0),
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instancia.into()),
                None,
            )
        }
        .ok()?;
        let hecho = (|| -> windows::core::Result<(SystemMediaTransportControls, i64, Receiver<BotonDelSistema>)> {
            let interop =
                factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
            // SAFETY: `hwnd` es nuestra y vive hasta el `Drop`.
            let smtc: SystemMediaTransportControls = unsafe { interop.GetForWindow(hwnd) }?;
            smtc.SetIsEnabled(true)?;
            smtc.SetIsPlayEnabled(true)?;
            smtc.SetIsPauseEnabled(true)?;
            smtc.SetIsNextEnabled(true)?;
            smtc.SetIsPreviousEnabled(true)?;
            smtc.SetIsStopEnabled(true)?;
            let (tx, rx) = mpsc::channel();
            let manejador = TypedEventHandler::<
                SystemMediaTransportControls,
                SystemMediaTransportControlsButtonPressedEventArgs,
            >::new(move |_, args| {
                if let Some(args) = args.as_ref()
                    && let Some(b) = boton_de(args.Button()?)
                {
                    let _ = tx.send(b);
                }
                Ok(())
            });
            let token = smtc.ButtonPressed(&manejador)?;
            Ok((smtc, token, rx))
        })();
        match hecho {
            Ok((smtc, token, botones)) => Some(MandosDelSistema {
                hwnd,
                smtc,
                token,
                botones,
            }),
            Err(e) => {
                tracing::info!(?e, "Windows no dio los mandos de reproductor");
                // SAFETY: ventana nuestra, recien creada.
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
                None
            }
        }
    }

    /// **Lo que ensena Windows**: si suena, el titulo del documento y debajo
    /// por donde va («Parrafo 3 de 40 · Elvira»), como la notificacion del
    /// movil (titulo, artista y album «PixPin»).
    pub fn poner(&self, sonando: bool, titulo: &str, detalle: &str) {
        let r = (|| -> windows::core::Result<()> {
            self.smtc.SetPlaybackStatus(if sonando {
                MediaPlaybackStatus::Playing
            } else {
                MediaPlaybackStatus::Paused
            })?;
            let d = self.smtc.DisplayUpdater()?;
            d.SetType(MediaPlaybackType::Music)?;
            let m = d.MusicProperties()?;
            m.SetTitle(&HSTRING::from(titulo))?;
            m.SetArtist(&HSTRING::from(detalle))?;
            m.SetAlbumTitle(&HSTRING::from("PixPin"))?;
            d.Update()
        })();
        if let Err(e) = r {
            tracing::debug!(?e, "no se pudieron poner los mandos de reproductor");
        }
    }

    /// Los botones pulsados desde la ultima vez.
    pub fn botones(&self) -> Vec<BotonDelSistema> {
        self.botones.try_iter().collect()
    }

    /// Atiende los mensajes de la ventana escondida (y de COM) de este hilo.
    pub fn bombear(&self) {
        let mut msg = MSG::default();
        // SAFETY: bucle de mensajes corriente del hilo que creo la ventana.
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

impl Drop for MandosDelSistema {
    fn drop(&mut self) {
        let _ = self.smtc.RemoveButtonPressed(self.token);
        if let Ok(d) = self.smtc.DisplayUpdater() {
            let _ = d.ClearAll();
            let _ = d.Update();
        }
        let _ = self.smtc.SetPlaybackStatus(MediaPlaybackStatus::Closed);
        let _ = self.smtc.SetIsEnabled(false);
        // SAFETY: la ventana es nuestra y se destruye en su hilo (el que la
        // creo, el mismo que suelta esto).
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_botones_de_windows_son_los_del_movil() {
        assert_eq!(
            boton_de(SystemMediaTransportControlsButton::Play),
            Some(BotonDelSistema::Tocar)
        );
        assert_eq!(
            boton_de(SystemMediaTransportControlsButton::Next),
            Some(BotonDelSistema::Siguiente)
        );
        assert_eq!(
            boton_de(SystemMediaTransportControlsButton::Previous),
            Some(BotonDelSistema::Anterior)
        );
        assert_eq!(
            boton_de(SystemMediaTransportControlsButton::Stop),
            Some(BotonDelSistema::Parar)
        );
        // Lo que el lector no tiene no hace nada.
        assert_eq!(boton_de(SystemMediaTransportControlsButton::Rewind), None);
        assert_eq!(
            boton_de(SystemMediaTransportControlsButton::ChannelUp),
            None
        );
    }

    /// **Windows da los mandos a una ventana escondida** y acepta el titulo.
    /// Aparece un momento en el recuadro del volumen: por eso no corre con
    /// las de siempre (`cargo test -- --ignored mandos_de_verdad`).
    #[test]
    #[ignore]
    fn mandos_de_verdad() {
        use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
        // SAFETY: inicio de COM en el hilo de la prueba.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let m = MandosDelSistema::nuevos().expect("Windows tiene que dar los mandos");
        m.poner(true, "Prueba de PixPin", "Párrafo 1 de 2");
        m.bombear();
        assert!(m.botones().is_empty());
        let estado = m.smtc.PlaybackStatus().unwrap();
        assert_eq!(estado, MediaPlaybackStatus::Playing);
        let titulo = m
            .smtc
            .DisplayUpdater()
            .unwrap()
            .MusicProperties()
            .unwrap()
            .Title()
            .unwrap();
        assert_eq!(titulo.to_string(), "Prueba de PixPin");
        drop(m);
    }
}
