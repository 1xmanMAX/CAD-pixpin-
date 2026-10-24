//! **El menu nativo con submenus y miniaturas** (21-oct-2026): el de las
//! figuras del lienzo, agrupado por categoria y con un dibujito de cada
//! figura al lado de su nombre.
//!
//! Sigue siendo el menu de Windows ([`crate::exportar::menu_emergente`]),
//! que ya sabe de teclado, de accesibilidad y de bordes de pantalla; aqui
//! solo se le anaden los submenus (`MF_POPUP`) y la imagen de cada entrada
//! (`MENUITEMINFOW::hbmpItem`, un DIB de 32 bits premultiplicado).

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HBITMAP,
    HGDIOBJ,
};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetMenuItemCount, HMENU, MENUITEMINFOW,
    MF_MENUBARBREAK, MF_POPUP, MF_SEPARATOR, MF_STRING, MIIM_BITMAP, SetForegroundWindow,
    SetMenuItemInfoW, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
};
use windows::core::{HSTRING, PCWSTR};

/// Una imagen chica, RGBA sin premultiplicar, de arriba abajo.
#[derive(Clone)]
pub struct Miniatura {
    pub ancho: u32,
    pub alto: u32,
    pub rgba: Vec<u8>,
}

/// Una entrada: con `hijas`, un submenu; con id cero y sin hijas, una raya.
#[derive(Clone, Default)]
pub struct EntradaRica {
    pub id: u32,
    pub texto: String,
    pub imagen: Option<Miniatura>,
    pub hijas: Vec<EntradaRica>,
}

impl EntradaRica {
    pub fn de(id: u32, texto: impl Into<String>) -> EntradaRica {
        EntradaRica {
            id,
            texto: texto.into(),
            ..Default::default()
        }
    }

    pub fn raya() -> EntradaRica {
        EntradaRica::default()
    }

    pub fn submenu(texto: impl Into<String>, hijas: Vec<EntradaRica>) -> EntradaRica {
        EntradaRica {
            texto: texto.into(),
            hijas,
            ..Default::default()
        }
    }
}

/// El DIB de una miniatura: BGRA premultiplicado, de arriba abajo.
fn bitmap(m: &Miniatura) -> Option<HBITMAP> {
    if m.ancho == 0 || m.alto == 0 || m.rgba.len() < (m.ancho * m.alto * 4) as usize {
        return None;
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: m.ancho as i32,
            biHeight: -(m.alto as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY: `bits` apunta a `ancho * alto * 4` bytes del DIB recien creado,
    // que se llenan una vez y no se vuelven a tocar.
    unsafe {
        let h = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(h.0));
            return None;
        }
        let destino = std::slice::from_raw_parts_mut(bits as *mut u8, m.rgba.len());
        for (d, s) in destino.chunks_exact_mut(4).zip(m.rgba.chunks_exact(4)) {
            let a = s[3] as u32;
            d[0] = (s[2] as u32 * a / 255) as u8;
            d[1] = (s[1] as u32 * a / 255) as u8;
            d[2] = (s[0] as u32 * a / 255) as u8;
            d[3] = s[3];
        }
        Some(h)
    }
}

/// Llena `menu` con `entradas` (y sus submenus), apuntando los bitmaps que
/// hay que soltar al acabar.
///
/// SAFETY: `menu` es un menu vivo; las cadenas se copian al anadirlas.
/// Cuantas entradas con miniatura caben en una columna: un submenu mas
/// largo se reparte en columnas (`MF_MENUBARBREAK`), como una galeria, en
/// vez de salirse de la pantalla.
const POR_COLUMNA: usize = 10;

unsafe fn llenar(menu: HMENU, entradas: &[EntradaRica], bitmaps: &mut Vec<HBITMAP>) {
    let con_imagen = entradas.iter().filter(|e| e.imagen.is_some()).count();
    let columnas = con_imagen.div_ceil(POR_COLUMNA).max(1);
    let alto = entradas.len().div_ceil(columnas);
    for (i, e) in entradas.iter().enumerate() {
        let salto = if con_imagen > POR_COLUMNA && i > 0 && i % alto == 0 {
            MF_MENUBARBREAK
        } else {
            Default::default()
        };
        let texto = HSTRING::from(e.texto.as_str());
        // SAFETY: lo de la funcion.
        unsafe {
            if !e.hijas.is_empty() {
                let Ok(sub) = CreatePopupMenu() else { continue };
                llenar(sub, &e.hijas, bitmaps);
                let _ = AppendMenuW(
                    menu,
                    MF_POPUP | MF_STRING,
                    sub.0 as usize,
                    PCWSTR(texto.as_ptr()),
                );
            } else if e.id == 0 {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                continue;
            } else {
                let _ = AppendMenuW(
                    menu,
                    MF_STRING | salto,
                    e.id as usize,
                    PCWSTR(texto.as_ptr()),
                );
            }
            if let Some(b) = e.imagen.as_ref().and_then(bitmap) {
                let pos = GetMenuItemCount(Some(menu)) - 1;
                let info = MENUITEMINFOW {
                    cbSize: size_of::<MENUITEMINFOW>() as u32,
                    fMask: MIIM_BITMAP,
                    hbmpItem: b,
                    ..Default::default()
                };
                let _ = SetMenuItemInfoW(menu, pos as u32, true, &info);
                bitmaps.push(b);
            }
        }
    }
}

/// **El menu**, donde se diga: la entrada elegida, o `None` si se cierra
/// sin elegir.
pub fn menu_rico(propietaria: HWND, x: i32, y: i32, entradas: &[EntradaRica]) -> Option<u32> {
    let mut bitmaps = Vec::new();
    // SAFETY: menu propio que se destruye (con sus submenus) antes de salir,
    // y despues sus bitmaps, que `DestroyMenu` no suelta.
    unsafe {
        let _ = ReleaseCapture();
        let menu = CreatePopupMenu().ok()?;
        llenar(menu, entradas, &mut bitmaps);
        let _ = SetForegroundWindow(propietaria);
        let elegido = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
            x,
            y,
            None,
            propietaria,
            None,
        );
        let _ = DestroyMenu(menu);
        for b in bitmaps {
            let _ = DeleteObject(HGDIOBJ(b.0));
        }
        (elegido.0 > 0).then_some(elegido.0 as u32)
    }
}
