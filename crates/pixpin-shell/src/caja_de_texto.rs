//! **Una caja para escribir o pegar un texto largo**, con el control `EDIT`
//! de toda la vida de Windows.
//!
//! La piden tres pantallas de la voz: corregir la letra de una nota (el lapiz
//! de `LetraActivity`, que en el movil abre su editor de Markdown), el texto
//! del telepronter («Escribir o pegar el texto») y el texto guia de
//! Pronunciar. Las tres son lo mismo: un texto de varios parrafos que se
//! escribe, se pega y se corrige. El `EDIT` multilinea ya trae todo eso
//! —seleccion, deshacer, Ctrl+C/V, el IME, la rueda— sin una linea nuestra,
//! y no pesa nada: viene con `user32`.
//!
//! Va en **su propio hilo**, con su bucle de mensajes, y devuelve lo escrito
//! por un canal: quien la abre (el chat, el telepronter) sigue pintando y no
//! se queda esperando. `None` es «cancelado».

use std::sync::mpsc::{Receiver, channel};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DeleteObject,
    FF_DONTCARE, FW_NORMAL, GetDC, GetDeviceCaps, GetStockObject, HFONT, HGDIOBJ, LOGPIXELSY,
    OUT_DEFAULT_PRECIS, ReleaseDC, WHITE_BRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, ES_AUTOVSCROLL, ES_MULTILINE, ES_WANTRETURN, GetClientRect, GetMessageW,
    GetWindowTextLengthW, GetWindowTextW, HMENU, IDCANCEL, IDOK, IsDialogMessageW, MSG, MoveWindow,
    PostQuitMessage, RegisterClassW, SW_SHOW, SendMessageW, SetForegroundWindow, ShowWindow,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_KEYDOWN,
    WM_SETFONT, WM_SIZE, WNDCLASSW, WS_BORDER, WS_CHILD, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW,
    WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

/// Lo que se ensena: titulo de la ventana, texto de partida y los dos
/// rotulos de los botones (ya traducidos: este crate no tiene catalogo).
#[derive(Debug, Clone)]
pub struct Pedido {
    pub titulo: String,
    pub texto: String,
    pub guardar: String,
    pub cancelar: String,
}

/// Abre la caja y vuelve en el acto. Por el canal llega **una sola vez**
/// `Some(texto)` al guardar o `None` al cancelar o cerrar; si la ventana no
/// se pudo crear, el canal se cierra sin mandar nada.
pub fn abrir(pedido: Pedido) -> Receiver<Option<String>> {
    let (enviar, recibir) = channel();
    let lanzado = std::thread::Builder::new()
        .name("caja-de-texto".into())
        .spawn(move || {
            let r = correr(&pedido);
            let _ = enviar.send(r);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar la caja de texto");
    }
    recibir
}

/// Los saltos de linea que quiere el `EDIT` (`\r\n`) desde los de dentro.
pub fn a_windows(texto: &str) -> String {
    texto.replace("\r\n", "\n").replace('\n', "\r\n")
}

/// Y de vuelta: dentro de PixPin los parrafos van con `\n`, como en el movil.
pub fn de_windows(texto: &str) -> String {
    texto.replace("\r\n", "\n")
}

const ID_EDIT: i32 = 100;

thread_local! {
    /// Lo que quedo al pulsar Guardar. Solo lo toca el hilo de la caja.
    static RESULTADO: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    /// Los tres controles, para colocarlos en `WM_SIZE`.
    static HIJOS: std::cell::Cell<[isize; 3]> = const { std::cell::Cell::new([0; 3]) };
}

/// Los puntos por pulgada de la pantalla, para que la caja y su letra no
/// salgan diminutas en un monitor al 150 %.
fn dpi() -> i32 {
    // SAFETY: el DC de la pantalla se pide y se devuelve en el acto; solo
    // se consulta.
    unsafe {
        let dc = GetDC(None);
        let v = GetDeviceCaps(Some(dc), LOGPIXELSY);
        ReleaseDC(None, dc);
        v.max(96)
    }
}

fn ancho(v: &str) -> Vec<u16> {
    v.encode_utf16().chain(std::iter::once(0)).collect()
}

fn correr(p: &Pedido) -> Option<String> {
    RESULTADO.with(|r| *r.borrow_mut() = None);
    // SAFETY: todo lo que se crea aqui es de este hilo y muere con su
    // bucle: la clase (registrada una vez; repetir el registro falla sin
    // dano), la ventana y sus hijos, y la fuente, que se borra al final.
    unsafe {
        let instancia = GetModuleHandleW(None).ok()?;
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: w!("PixPinCajaDeTexto"),
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(GetStockObject(WHITE_BRUSH).0),
            ..Default::default()
        };
        RegisterClassW(&clase);
        let titulo = ancho(&p.titulo);
        let ventana = CreateWindowExW(
            // Encima: se abre desde ventanas que a veces lo estan (el
            // telepronter a pantalla completa) y quedar detras es perderla.
            WS_EX_TOPMOST,
            w!("PixPinCajaDeTexto"),
            PCWSTR(titulo.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            720,
            560,
            None,
            None,
            Some(instancia.into()),
            None,
        )
        .ok()?;
        let dpi = dpi();
        let e = |v: i32| v * dpi / 96;
        // Ajustar al DPI: la ventana nacio con 720x560 de 96 ppp.
        let _ = MoveWindow(ventana, e(80), e(80), e(720), e(560), false);
        let fuente = CreateFontW(
            -e(16),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            FF_DONTCARE.0 as u32,
            w!("Segoe UI"),
        );
        let texto = ancho(&a_windows(&p.texto));
        let edit = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            PCWSTR(texto.as_ptr()),
            WS_CHILD
                | WS_VISIBLE
                | WS_BORDER
                | WS_VSCROLL
                | WS_TABSTOP
                | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32),
            0,
            0,
            10,
            10,
            Some(ventana),
            Some(HMENU(ID_EDIT as isize as *mut _)),
            Some(instancia.into()),
            None,
        )
        .ok()?;
        let boton = |rotulo: &str, id: i32, estilo: i32| {
            let r = ancho(rotulo);
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                PCWSTR(r.as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(estilo as u32),
                0,
                0,
                10,
                10,
                Some(ventana),
                Some(HMENU(id as isize as *mut _)),
                Some(instancia.into()),
                None,
            )
            .ok()
        };
        let guardar = boton(&p.guardar, IDOK.0, BS_DEFPUSHBUTTON);
        let cancelar = boton(&p.cancelar, IDCANCEL.0, BS_PUSHBUTTON);
        for h in [Some(edit), guardar, cancelar].into_iter().flatten() {
            SendMessageW(
                h,
                WM_SETFONT,
                Some(WPARAM(fuente.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        HIJOS.with(|c| {
            c.set([
                edit.0 as isize,
                guardar.map(|h| h.0 as isize).unwrap_or(0),
                cancelar.map(|h| h.0 as isize).unwrap_or(0),
            ])
        });
        colocar(ventana);
        let _ = ShowWindow(ventana, SW_SHOW);
        let _ = SetForegroundWindow(ventana);
        let _ = SetFocus(Some(edit));

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            // Ctrl+Intro guarda desde dentro del texto (Intro solo es
            // renglon nuevo), y Escape cancela via `IsDialogMessage`.
            if msg.message == WM_KEYDOWN
                && msg.wParam.0 == 0x0D
                && GetKeyState(VK_CONTROL.0 as i32) < 0
            {
                guardar_y_cerrar(ventana);
                continue;
            }
            if IsDialogMessageW(ventana, &msg).as_bool() {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = DeleteObject(HGDIOBJ(fuente.0));
        let _: HFONT = fuente;
    }
    RESULTADO.with(|r| r.borrow_mut().take())
}

/// Lee el `EDIT` y cierra.
///
/// # Safety
/// `ventana` es la de la caja, en su propio hilo.
unsafe fn guardar_y_cerrar(ventana: HWND) {
    let edit = HWND(HIJOS.with(|c| c.get())[0] as *mut _);
    // SAFETY: consultas y copia sobre un control vivo de este hilo, con un
    // bufer del largo que el propio control dice mas el cero.
    unsafe {
        let largo = GetWindowTextLengthW(edit).max(0) as usize;
        let mut buf = vec![0u16; largo + 1];
        let n = GetWindowTextW(edit, &mut buf).max(0) as usize;
        let texto = de_windows(&String::from_utf16_lossy(&buf[..n]));
        RESULTADO.with(|r| *r.borrow_mut() = Some(texto));
        let _ = DestroyWindow(ventana);
    }
}

/// El texto ocupa todo menos una franja abajo con los dos botones a la
/// derecha.
fn colocar(ventana: HWND) {
    let [edit, guardar, cancelar] = HIJOS.with(|c| c.get());
    // SAFETY: medidas y movimientos de controles vivos de este hilo.
    unsafe {
        let dpi = dpi();
        let e = |v: i32| v * dpi / 96;
        let mut r = RECT::default();
        let _ = GetClientRect(ventana, &mut r);
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        let pie = e(52);
        let margen = e(10);
        let (bw, bh) = (e(110), e(32));
        let _ = MoveWindow(
            HWND(edit as *mut _),
            margen,
            margen,
            (w - 2 * margen).max(10),
            (h - pie - margen).max(10),
            true,
        );
        let y = h - pie + (pie - bh) / 2;
        let _ = MoveWindow(HWND(cancelar as *mut _), w - margen - bw, y, bw, bh, true);
        let _ = MoveWindow(
            HWND(guardar as *mut _),
            w - 2 * margen - 2 * bw,
            y,
            bw,
            bh,
            true,
        );
    }
}

extern "system" fn procedimiento(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: el procedimiento de la ventana de la caja; solo recibe
    // mensajes de su propio hilo y llama a funciones de user32 sobre ella.
    unsafe {
        match msg {
            WM_SIZE => {
                colocar(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                if id == IDOK.0 {
                    guardar_y_cerrar(hwnd);
                } else if id == IDCANCEL.0 {
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_parrafos_van_con_retorno_de_carro_a_la_caja_y_vuelven_sin_el() {
        assert_eq!(a_windows("uno\n\ndos"), "uno\r\n\r\ndos");
        // Lo que ya venia con `\r\n` no se duplica.
        assert_eq!(a_windows("uno\r\ndos"), "uno\r\ndos");
        assert_eq!(de_windows("uno\r\n\r\ndos"), "uno\n\ndos");
        assert_eq!(de_windows(&a_windows("a\nb\n")), "a\nb\n");
    }

    #[test]
    fn un_texto_vacio_va_y_vuelve_vacio() {
        assert_eq!(a_windows(""), "");
        assert_eq!(de_windows(""), "");
    }
}
