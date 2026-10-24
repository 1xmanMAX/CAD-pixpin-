//! **La caja de exportar**, con lo que trae Windows: el «Guardar como» de
//! siempre con tres mandos mas dentro.
//!
//! El movil tiene una hoja de compartir propia (`ui/HojaDeCompartir.kt`) con
//! el formato, que paginas y cuanto pesara. En Windows ese sitio ya existe y
//! todo el mundo lo conoce: el dialogo de guardar, que deja elegir el tipo de
//! fichero, y que acepta controles propios (`IFileDialogCustomize`). Asi la
//! caja es **una** y es la del sistema: el formato es el tipo de fichero
//! (PNG, SVG, PDF o pagina web), y debajo van «Que» (todo, lo elegido o cada
//! marco), «Escala» (1x, 2x, 3x) y «Fondo transparente». Sin ventana nueva
//! que dibujar ni que mantener, y con el explorador de carpetas de verdad.
//!
//! Aqui viven tambien las otras dos piezas de Win32 de exportar: copiar la
//! imagen al portapapeles en los formatos que entienden los demas programas,
//! y el menu del clic derecho del lienzo.

use std::path::PathBuf;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HWND};
use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFOHEADER};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GHND, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FileSaveDialog, IFileDialogCustomize, IFileSaveDialog, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, MF_SEPARATOR, MF_STRING, SetForegroundWindow,
    TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
};
use windows::core::{HSTRING, Interface, PCWSTR, w};

/// El formato de lo exportado. Es el tipo de fichero del dialogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Png,
    Svg,
    Pdf,
    Html,
}

impl Formato {
    /// En el orden de la lista de tipos del dialogo.
    pub const TODOS: [Formato; 4] = [Formato::Png, Formato::Svg, Formato::Pdf, Formato::Html];

    pub fn extension(self) -> &'static str {
        match self {
            Formato::Png => "png",
            Formato::Svg => "svg",
            Formato::Pdf => "pdf",
            Formato::Html => "html",
        }
    }

    /// El formato de una extension, sin distinguir mayusculas.
    pub fn de_extension(ext: &str) -> Option<Formato> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Formato::Png),
            "svg" => Some(Formato::Svg),
            "pdf" => Some(Formato::Pdf),
            "html" | "htm" => Some(Formato::Html),
            _ => None,
        }
    }
}

/// Que trozo del lienzo. El numero es el id del elemento en el desplegable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Que {
    Todo = 0,
    Seleccion = 1,
    Marcos = 2,
}

/// Lo elegido en la caja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Eleccion {
    pub formato: Formato,
    pub que: Que,
    /// 1, 2 o 3.
    pub escala: u32,
    pub transparente: bool,
}

/// Los rotulos de la caja, ya traducidos por quien llama.
pub struct Rotulos<'a> {
    pub titulo: &'a str,
    pub que: &'a str,
    pub todo: &'a str,
    pub seleccion: &'a str,
    pub marcos: &'a str,
    pub escala: &'a str,
    pub transparente: &'a str,
    pub png: &'a str,
    pub svg: &'a str,
    pub pdf: &'a str,
    pub html: &'a str,
}

// Los ids de los controles propios. Cualquier numero vale mientras no se
// repita dentro del dialogo.
const ID_QUE: u32 = 100;
const ID_ESCALA: u32 = 101;
const ID_TRANSPARENTE: u32 = 102;
const ID_GRUPO_QUE: u32 = 110;
const ID_GRUPO_ESCALA: u32 = 111;

/// Que formato decide el fichero elegido: la extension que escribio la
/// persona manda sobre el tipo de la lista, porque es lo que va a ver en el
/// explorador; si no es de las nuestras, manda la lista y se le pone.
pub fn decidir_formato(ruta: PathBuf, indice_tipo: u32) -> (PathBuf, Formato) {
    let por_lista = Formato::TODOS
        .get(indice_tipo.saturating_sub(1) as usize)
        .copied()
        .unwrap_or(Formato::Png);
    match ruta
        .extension()
        .and_then(|e| e.to_str())
        .and_then(Formato::de_extension)
    {
        Some(f) => (ruta, f),
        None => {
            let mut s = ruta.into_os_string();
            s.push(".");
            s.push(por_lista.extension());
            (PathBuf::from(s), por_lista)
        }
    }
}

/// COM en el hilo mientras dure un dialogo. Si ya estaba, se usa el que
/// habia y no se cierra.
struct Com(bool);

impl Com {
    fn iniciar() -> Com {
        // SAFETY: iniciar COM no tiene precondiciones.
        Com(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: empareja el CoInitializeEx que tuvo exito.
            unsafe { CoUninitialize() };
        }
    }
}

/// **Abre la caja de exportar.** `inicial` es lo que sale puesto (lo ultimo
/// que se eligio); «Lo elegido» y «Cada marco» solo se ofrecen si los hay.
///
/// `None` si se cancela o el dialogo falla: no se exporta nada.
pub fn pedir_exportacion(
    propietaria: HWND,
    nombre_sugerido: &str,
    rotulos: &Rotulos<'_>,
    inicial: Eleccion,
    hay_seleccion: bool,
    hay_marcos: bool,
) -> Option<(PathBuf, Eleccion)> {
    let _com = Com::iniciar();
    let nombres: Vec<HSTRING> = [rotulos.png, rotulos.svg, rotulos.pdf, rotulos.html]
        .iter()
        .map(|s| HSTRING::from(*s))
        .collect();
    let patrones = [w!("*.png"), w!("*.svg"), w!("*.pdf"), w!("*.html;*.htm")];
    let filtros: Vec<COMDLG_FILTERSPEC> = nombres
        .iter()
        .zip(patrones)
        .map(|(n, p)| COMDLG_FILTERSPEC {
            pszName: PCWSTR(n.as_ptr()),
            pszSpec: p,
        })
        .collect();
    let indice_inicial = Formato::TODOS
        .iter()
        .position(|f| *f == inicial.formato)
        .unwrap_or(0) as u32
        + 1;
    // SAFETY: COM iniciado arriba; el dialogo y sus cadenas viven durante
    // toda la funcion (`nombres` y los HSTRING locales siguen en pie).
    unsafe {
        let dialogo: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        dialogo.SetFileTypes(&filtros).ok()?;
        dialogo.SetFileTypeIndex(indice_inicial).ok()?;
        // Con una extension por omision, el dialogo cambia la del nombre al
        // cambiar de tipo, que es lo que se espera al elegir «SVG».
        let ext = HSTRING::from(inicial.formato.extension());
        dialogo.SetDefaultExtension(PCWSTR(ext.as_ptr())).ok()?;
        let titulo = HSTRING::from(rotulos.titulo);
        let _ = dialogo.SetTitle(PCWSTR(titulo.as_ptr()));
        let nombre = HSTRING::from(nombre_sugerido);
        dialogo.SetFileName(PCWSTR(nombre.as_ptr())).ok()?;

        // Los mandos propios. Si el sistema no los admite, la caja sigue
        // valiendo con lo que venia puesto.
        let mandos: Option<IFileDialogCustomize> = dialogo.cast().ok();
        if let Some(m) = &mandos {
            let que = HSTRING::from(rotulos.que);
            let _ = m.StartVisualGroup(ID_GRUPO_QUE, PCWSTR(que.as_ptr()));
            let _ = m.AddComboBox(ID_QUE);
            let opciones = [
                (Que::Todo, rotulos.todo, true),
                (Que::Seleccion, rotulos.seleccion, hay_seleccion),
                (Que::Marcos, rotulos.marcos, hay_marcos),
            ];
            for (q, texto, vale) in opciones {
                if vale {
                    let t = HSTRING::from(texto);
                    let _ = m.AddControlItem(ID_QUE, q as u32, PCWSTR(t.as_ptr()));
                }
            }
            let puesta = match inicial.que {
                Que::Seleccion if hay_seleccion => Que::Seleccion,
                Que::Marcos if hay_marcos => Que::Marcos,
                _ => Que::Todo,
            };
            let _ = m.SetSelectedControlItem(ID_QUE, puesta as u32);
            let _ = m.EndVisualGroup();
            let escala = HSTRING::from(rotulos.escala);
            let _ = m.StartVisualGroup(ID_GRUPO_ESCALA, PCWSTR(escala.as_ptr()));
            let _ = m.AddComboBox(ID_ESCALA);
            for k in 1..=3u32 {
                let t = HSTRING::from(format!("{k}\u{d7}"));
                let _ = m.AddControlItem(ID_ESCALA, k, PCWSTR(t.as_ptr()));
            }
            let _ = m.SetSelectedControlItem(ID_ESCALA, inicial.escala.clamp(1, 3));
            let _ = m.EndVisualGroup();
            let transparente = HSTRING::from(rotulos.transparente);
            let _ = m.AddCheckButton(
                ID_TRANSPARENTE,
                PCWSTR(transparente.as_ptr()),
                inicial.transparente,
            );
        }

        // Show devuelve Err al cancelar: es el camino normal, no un fallo.
        dialogo.Show(Some(propietaria)).ok()?;
        let item = dialogo.GetResult().ok()?;
        let texto = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let ruta = texto.to_string().ok();
        // Memoria de COM: se libera aqui, antes de cualquier `?`.
        CoTaskMemFree(Some(texto.as_ptr() as *const _));
        let ruta = PathBuf::from(ruta?);
        let indice = dialogo.GetFileTypeIndex().unwrap_or(indice_inicial);
        let (ruta, formato) = decidir_formato(ruta, indice);
        let mut eleccion = Eleccion { formato, ..inicial };
        if let Some(m) = &mandos {
            eleccion.que = match m.GetSelectedControlItem(ID_QUE) {
                Ok(1) => Que::Seleccion,
                Ok(2) => Que::Marcos,
                _ => Que::Todo,
            };
            eleccion.escala = m.GetSelectedControlItem(ID_ESCALA).unwrap_or(1).clamp(1, 3);
            eleccion.transparente = m
                .GetCheckButtonState(ID_TRANSPARENTE)
                .map(|b| b.as_bool())
                .unwrap_or(inicial.transparente);
        }
        Some((ruta, eleccion))
    }
}

/// El DIB de una imagen RGBA **sobre blanco**: el formato clasico no tiene
/// transparencia de verdad y medio mundo pintaria de negro lo transparente.
/// Quien sepa leer transparencia coge el PNG, que va al lado.
pub fn dib_sobre_blanco(ancho: u32, alto: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let espera = ancho as usize * alto as usize * 4;
    if ancho == 0 || alto == 0 || rgba.len() != espera {
        return None;
    }
    let cabecera = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: ancho as i32,
        biHeight: alto as i32,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: espera as u32,
        ..Default::default()
    };
    let mut v = Vec::with_capacity(size_of::<BITMAPINFOHEADER>() + espera);
    // SAFETY: BITMAPINFOHEADER es un POD inicializado entero; se copian sus
    // bytes y no se guarda el puntero.
    v.extend_from_slice(unsafe {
        std::slice::from_raw_parts(
            &cabecera as *const BITMAPINFOHEADER as *const u8,
            size_of::<BITMAPINFOHEADER>(),
        )
    });
    let paso = ancho as usize * 4;
    // Las filas de abajo arriba, como las quiere un DIB clasico.
    for fila in (0..alto as usize).rev() {
        for p in rgba[fila * paso..(fila + 1) * paso].chunks_exact(4) {
            let a = p[3] as u32;
            let sobre = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
            v.extend_from_slice(&[sobre(p[2]), sobre(p[1]), sobre(p[0]), 255]);
        }
    }
    Some(v)
}

/// **Copia una imagen al portapapeles como lo hace Excalidraw**: el PNG (con
/// su transparencia, que es lo que pegan Word, PowerPoint, el navegador o
/// Figma) y el mapa de bits clasico para los demas; y el SVG si se da, con
/// su tipo de la web (`image/svg+xml`), que es lo que leen las aplicaciones
/// de dibujo vectorial al pegar.
pub fn copiar_imagen(ancho: u32, alto: u32, rgba: &[u8], png: &[u8], svg: Option<&str>) -> bool {
    let Some(dib) = dib_sobre_blanco(ancho, alto, rgba) else {
        return false;
    };
    const CF_DIB: u32 = 8;
    // SAFETY: nombres constantes terminados en cero.
    let (f_png, f_svg) = unsafe {
        (
            RegisterClipboardFormatW(w!("PNG")),
            RegisterClipboardFormatW(w!("image/svg+xml")),
        )
    };
    let mut cargas: Vec<(u32, &[u8])> = vec![(CF_DIB, &dib)];
    if f_png != 0 && !png.is_empty() {
        cargas.push((f_png, png));
    }
    if let (Some(s), true) = (svg, f_svg != 0) {
        cargas.push((f_svg, s.as_bytes()));
    }
    publicar(&cargas)
}

/// Abre el portapapeles una vez, lo vacia y cede cada carga. Todo se reserva
/// antes de tocarlo: si falta memoria para la segunda, el portapapeles del
/// usuario no se ha vaciado para nada.
fn publicar(cargas: &[(u32, &[u8])]) -> bool {
    let mut bloques = Vec::with_capacity(cargas.len());
    for (formato, datos) in cargas {
        // SAFETY: reserva de memoria movible del tamano exacto; se copia con
        // el bloque bloqueado y se desbloquea en seguida.
        let h = unsafe {
            let Ok(h) = GlobalAlloc(GHND, datos.len().max(1)) else {
                for (_, h) in bloques {
                    let _ = GlobalFree(Some(h));
                }
                return false;
            };
            let p = GlobalLock(h) as *mut u8;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(datos.as_ptr(), p, datos.len());
                let _ = GlobalUnlock(h);
            }
            h
        };
        bloques.push((*formato, h));
    }
    // SAFETY: sesion unica de portapapeles; lo que se cede pasa a ser del
    // sistema y lo que no, se libera aqui.
    unsafe {
        if OpenClipboard(None).is_err() {
            for (_, h) in bloques {
                let _ = GlobalFree(Some(h));
            }
            return false;
        }
        let _ = EmptyClipboard();
        let mut todo = true;
        for (formato, h) in bloques {
            if SetClipboardData(formato, Some(HANDLE(h.0))).is_err() {
                let _ = GlobalFree(Some(h));
                todo = false;
            }
        }
        let _ = CloseClipboard();
        todo
    }
}

/// Un cuadro de aviso sobre la ventana, para decir que no habia nada que
/// exportar o que algo fallo. Modal y con propietaria: el editor tapa la
/// pantalla y un aviso suelto quedaria debajo.
pub fn informar(propietaria: HWND, titulo: &str, mensaje: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
    let (t, m) = (HSTRING::from(titulo), HSTRING::from(mensaje));
    // SAFETY: cadenas propias vivas durante la llamada; la propietaria es del
    // llamante y esta viva mientras el cuadro es modal.
    unsafe {
        let _ = MessageBoxW(Some(propietaria), &m, &t, MB_OK | MB_ICONINFORMATION);
    }
}

/// Una entrada del menu del clic derecho: su id y su rotulo. Un id cero es
/// una raya de separacion.
pub struct EntradaMenu {
    pub id: u32,
    pub texto: String,
}

/// **El menu del clic derecho**, el nativo de Windows: sale donde esta el
/// raton y devuelve la entrada elegida, o `None` si se cierra sin elegir.
/// Nativo y no dibujado porque en el lienzo solo hace falta una lista corta,
/// y el de Windows ya sabe de teclado, de accesibilidad y de bordes de
/// pantalla.
pub fn menu_emergente(propietaria: HWND, x: i32, y: i32, entradas: &[EntradaMenu]) -> Option<u32> {
    let textos: Vec<HSTRING> = entradas
        .iter()
        .map(|e| HSTRING::from(e.texto.as_str()))
        .collect();
    // SAFETY: menu propio que se destruye antes de salir; las cadenas viven
    // durante la llamada. Se suelta la captura del raton (la cogio el boton
    // derecho) porque con ella puesta el menu no recibe los clics.
    unsafe {
        let _ = ReleaseCapture();
        let menu = CreatePopupMenu().ok()?;
        for (e, t) in entradas.iter().zip(&textos) {
            if e.id == 0 {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            } else {
                let _ = AppendMenuW(menu, MF_STRING, e.id as usize, PCWSTR(t.as_ptr()));
            }
        }
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
        (elegido.0 > 0).then_some(elegido.0 as u32)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_extension_escrita_manda_sobre_el_tipo_de_la_lista() {
        let (r, f) = decidir_formato(PathBuf::from("C:\\a\\casa.SVG"), 1);
        assert_eq!(f, Formato::Svg);
        assert_eq!(r, PathBuf::from("C:\\a\\casa.SVG"));
        // Sin extension nuestra, manda la lista y se le pone.
        let (r, f) = decidir_formato(PathBuf::from("C:\\a\\casa"), 3);
        assert_eq!(f, Formato::Pdf);
        assert_eq!(r, PathBuf::from("C:\\a\\casa.pdf"));
        let (r, f) = decidir_formato(PathBuf::from("C:\\a\\casa.v2"), 4);
        assert_eq!(f, Formato::Html);
        assert_eq!(r, PathBuf::from("C:\\a\\casa.v2.html"));
        // Caso negativo: un indice fuera de la lista no revienta.
        assert_eq!(decidir_formato(PathBuf::from("x"), 99).1, Formato::Png);
    }

    #[test]
    fn el_dib_va_sobre_blanco_y_de_abajo_arriba() {
        // Dos pixeles en columna: arriba transparente, abajo rojo opaco.
        let rgba = [0, 0, 0, 0, 255, 0, 0, 255];
        let v = dib_sobre_blanco(1, 2, &rgba).expect("bien contado");
        let datos = &v[size_of::<BITMAPINFOHEADER>()..];
        // Primera fila del DIB = la de abajo = rojo, en BGRA.
        assert_eq!(&datos[..4], &[0, 0, 255, 255]);
        // Lo transparente sale blanco, no negro.
        assert_eq!(&datos[4..], &[255, 255, 255, 255]);
        // Caso negativo: pixeles mal contados no dan DIB.
        assert!(dib_sobre_blanco(2, 2, &rgba).is_none());
    }
}
