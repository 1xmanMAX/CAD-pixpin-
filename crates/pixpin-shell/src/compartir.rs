//! **El panel «Compartir» de Windows** para uno o varios ficheros.
//!
//! Es la alternativa de Windows a la hoja de compartir del movil
//! (`ui/HojaDeCompartir.kt`, `ui/CompartirNativo.kt`): el mismo panel que
//! sale en el Explorador con «Compartir», con Correo, Teams, Compartir en
//! proximidad, OneNote y lo que el usuario tenga instalado. No hace falta
//! empaquetar la aplicacion: `IDataTransferManagerInterop` existe justo para
//! que un programa Win32 normal lo abra sobre su ventana.
//!
//! El panel no pide los ficheros al abrirse sino cuando el usuario elige a
//! quien mandarlos (`DataRequested`), asi que se resuelven ANTES —convertir
//! una ruta en `StorageFile` es asincrono— y el manejador solo los entrega.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use windows::ApplicationModel::DataTransfer::{
    DataPackage, DataRequestedEventArgs, DataTransferManager, ShareCompletedEventArgs,
};
use windows::Foundation::TypedEventHandler;
use windows::Storage::{IStorageItem, StorageFile};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::IDataTransferManagerInterop;
use windows::core::{HSTRING, Interface, factory};
use windows_collections::IIterable;

#[derive(Debug, thiserror::Error)]
pub enum ErrorCompartir {
    #[error("no hay nada que compartir")]
    Nada,
    #[error("no se pudo preparar {ruta} para compartir: {fuente}")]
    Fichero {
        ruta: String,
        #[source]
        fuente: windows::core::Error,
    },
    #[error("Windows no pudo abrir el panel de compartir: {0}")]
    Panel(#[source] windows::core::Error),
}

/// La lista de ficheros, para el manejador, que Windows pide `Send`.
///
/// windows-rs no marca `Send` las interfaces de WinRT, pero esta lista es
/// la coleccion de serie del propio crate (`IIterable::from`), hecha en Rust
/// y sin afinidad de hilo, y los `StorageFile` que lleva son agiles: se
/// pueden usar desde el hilo en que Windows llame al manejador.
struct Libre(IIterable<IStorageItem>);
// SAFETY: ver el comentario de `Libre`: objetos agiles, sin afinidad de hilo.
unsafe impl Send for Libre {}
// SAFETY: lo mismo; el manejador solo la lee.
unsafe impl Sync for Libre {}

thread_local! {
    /// El manejador puesto la ultima vez: hay que quitarlo antes de poner el
    /// siguiente, o al compartir otra cosa el panel recibiria tambien lo de
    /// antes.
    static PUESTO: RefCell<Option<(DataTransferManager, i64)>> = const { RefCell::new(None) };
}

/// **Abre el panel Compartir de Windows** sobre `hwnd` con estos ficheros.
///
/// Vuelve en cuanto el panel esta abierto; lo que el usuario haga despues
/// lo hace Windows. Se llama desde el hilo de la ventana, con COM iniciado.
pub fn compartir(hwnd: HWND, rutas: &[PathBuf], titulo: &str) -> Result<(), ErrorCompartir> {
    abrir_panel(hwnd, rutas, titulo, None)
}

/// Lo que se llama cuando el panel termina, se envie o se cancele.
pub type AlTerminar = std::sync::Arc<dyn Fn() + Send + Sync>;

/// **Como [`compartir`], y avisa cuando el panel termina**: al elegir a
/// quien mandarlo (`ShareCompleted`) o al cerrarlo sin mandar nada
/// (`ShareCanceled`).
///
/// Lo necesita quien abre el panel desde una ventana que se quiere quitar
/// de en medio despues (la hoja de compartir): el panel cuelga de esa
/// ventana, y cerrarla antes de que el usuario elija se lo llevaria por
/// delante. En un Windows sin esos avisos (anterior a la 2004) no llega
/// nunca, y quien espera tiene que tener su propio limite.
pub fn compartir_avisando(
    hwnd: HWND,
    rutas: &[PathBuf],
    titulo: &str,
    al_terminar: AlTerminar,
) -> Result<(), ErrorCompartir> {
    abrir_panel(hwnd, rutas, titulo, Some(al_terminar))
}

fn abrir_panel(
    hwnd: HWND,
    rutas: &[PathBuf],
    titulo: &str,
    al_terminar: Option<AlTerminar>,
) -> Result<(), ErrorCompartir> {
    if rutas.is_empty() {
        return Err(ErrorCompartir::Nada);
    }
    let mut elementos: Vec<Option<IStorageItem>> = Vec::with_capacity(rutas.len());
    for ruta in rutas {
        elementos.push(Some(fichero(ruta)?));
    }
    let lista = Libre(IIterable::from(elementos));
    let titulo = HSTRING::from(titulo);

    let interop: IDataTransferManagerInterop =
        factory::<DataTransferManager, IDataTransferManagerInterop>()
            .map_err(ErrorCompartir::Panel)?;
    // SAFETY: `hwnd` es una ventana viva del llamante; el interop solo la usa
    // para colgar el panel encima.
    let gestor: DataTransferManager =
        unsafe { interop.GetForWindow(hwnd) }.map_err(ErrorCompartir::Panel)?;

    let manejador =
        TypedEventHandler::<DataTransferManager, DataRequestedEventArgs>::new(move |_, args| {
            // El envoltorio entero, no su campo: con la captura por campos
            // de Rust 2021 el cierre se llevaria el `IIterable` suelto.
            let lista = &lista;
            let Some(args) = args.as_ref() else {
                return Ok(());
            };
            let datos = args.Request()?.Data()?;
            datos.Properties()?.SetTitle(&titulo)?;
            datos.SetStorageItems(&lista.0, true)?;
            if let Some(avisar) = &al_terminar {
                // Si este Windows no tiene alguno de los dos avisos, se
                // comparte igual: el aviso es un extra, no una condicion.
                let a = avisar.clone();
                let _ = datos.ShareCompleted(&TypedEventHandler::<
                    DataPackage,
                    ShareCompletedEventArgs,
                >::new(move |_, _| {
                    a();
                    Ok(())
                }));
                let a = avisar.clone();
                let _ = datos.ShareCanceled(&TypedEventHandler::<
                    DataPackage,
                    windows::core::IInspectable,
                >::new(move |_, _| {
                    a();
                    Ok(())
                }));
            }
            Ok(())
        });
    let ficha = gestor
        .DataRequested(&manejador)
        .map_err(ErrorCompartir::Panel)?;

    PUESTO.with(|p| {
        if let Some((viejo, token)) = p.borrow_mut().take() {
            let _ = viejo.RemoveDataRequested(token);
        }
        *p.borrow_mut() = Some((gestor, ficha));
    });

    // SAFETY: igual que arriba.
    unsafe { interop.ShowShareUIForWindow(hwnd) }.map_err(ErrorCompartir::Panel)
}

/// Una ruta del disco como `StorageItem`. Es asincrono en WinRT; se espera
/// aqui porque es un fichero local y tarda milisegundos.
fn fichero(ruta: &Path) -> Result<IStorageItem, ErrorCompartir> {
    let error = |fuente| ErrorCompartir::Fichero {
        ruta: ruta.display().to_string(),
        fuente,
    };
    let f: StorageFile =
        StorageFile::GetFileFromPathAsync(&HSTRING::from(con_barras_de_windows(ruta)))
            .and_then(|op| op.join())
            .map_err(error)?;
    f.cast().map_err(error)
}

/// La ruta con `\` en todas partes. Las rutas que llegan del movil son
/// relativas con `/` (`proyectos/doc-1.pdf`) y se pegan a una del PC con
/// `\`: el resto de Windows lo acepta, pero `GetFileFromPathAsync` rechaza
/// la mezcla con un «path too long» (0x800700A1) y el panel no se abria.
fn con_barras_de_windows(ruta: &Path) -> String {
    ruta.to_string_lossy().replace('/', "\\")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_fichero_con_barras_del_movil_se_encuentra() {
        let _com = crate::ComDelHilo::iniciar();
        let dir = std::env::temp_dir().join("pixpin-compartir-barras");
        std::fs::create_dir_all(dir.join("proyectos")).unwrap();
        std::fs::write(dir.join("proyectos").join("doc-1.pdf"), b"%PDF-1.4").unwrap();
        // Como la deja `ruta_real` con un `pdfOrigen` del movil: mitad `\`,
        // mitad `/`.
        let mezclada = dir.join("proyectos/doc-1.pdf");
        assert!(fichero(&mezclada).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn las_barras_se_vuelven_de_windows_y_lo_demas_no_cambia() {
        assert_eq!(
            con_barras_de_windows(Path::new(r"C:\a\b/c/d.pdf")),
            r"C:\a\b\c\d.pdf"
        );
        assert_eq!(
            con_barras_de_windows(Path::new(r"C:\a\b.pdf")),
            r"C:\a\b.pdf"
        );
    }

    #[test]
    fn sin_ficheros_no_se_abre_nada() {
        let r = compartir(HWND::default(), &[], "nada");
        assert!(matches!(r, Err(ErrorCompartir::Nada)));
    }

    #[test]
    fn un_fichero_que_no_existe_se_dice_con_su_ruta() {
        let _com = crate::ComDelHilo::iniciar();
        let ruta = std::env::temp_dir().join("pixpin-no-existe-para-compartir.png");
        let _ = std::fs::remove_file(&ruta);
        match compartir(HWND::default(), std::slice::from_ref(&ruta), "x") {
            Err(ErrorCompartir::Fichero { ruta: r, .. }) => {
                assert!(r.contains("pixpin-no-existe-para-compartir"))
            }
            otra => panic!("se esperaba un fallo del fichero, y salio {otra:?}"),
        }
    }
}
