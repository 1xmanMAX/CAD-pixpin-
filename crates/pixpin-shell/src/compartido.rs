//! **Lo que otras apps comparten con PixPin** (15-oct-2026): el destino del
//! panel Compartir de Windows y el menu contextual del Explorador, los dos
//! con el **paquete de identidad** (MSIX con ubicacion externa,
//! `herramientas/paquete-identidad/`, `docs/integraciones-windows.md`).
//!
//! ## Como llega
//!
//! - **Compartir**: Windows lanza `pixpinmax.exe` con una activacion de tipo
//!   `ShareTarget`; la lee [`leer_activacion`] (`AppInstance::GetActivatedEventArgs`).
//!   Sin identidad no hay activacion y no se hace nada: todo sigue como hoy.
//! - **Menu contextual** (los verbos del paquete, y cualquiera que lance la
//!   app asi): `pixpinmax.exe --al-chat <rutas…>` ([`de_argumentos`]).
//!
//! ## A donde va
//!
//! Al **chat** «Mensajes guardados», con el pedido `chat` de siempre (texto,
//! `imagenes` como fotos, el resto como adjuntos): es lo que pidio el
//! coordinador para el PC. Android (`ShareReceiverActivity`) lo saca como pin;
//! aqui el pin ya lo da abrir el fichero con PixPin («Abrir con», doble clic),
//! y lo compartido de otra app (un enlace, una foto de WhatsApp) se perderia
//! al cerrar un pin: en el chat se queda y viaja al movil.
//!
//! Lo compartido se **copia** antes de contestar a Windows
//! (`ReportCompleted`): la app que comparte puede borrar sus temporales en
//! cuanto se le contesta, y la PixPin que corre procesa el pedido despues.

use std::path::{Path, PathBuf};

/// La orden de la linea de mandatos para mandar ficheros al chat.
pub const ORDEN_AL_CHAT: &str = "--al-chat";

/// Lo recibido, ya en el disco y listo para el pedido `chat`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recibido {
    pub texto: String,
    /// Van como fotos del chat.
    pub imagenes: Vec<PathBuf>,
    /// Van como adjuntos, tal cual.
    pub archivos: Vec<PathBuf>,
}

impl Recibido {
    pub fn vacio(&self) -> bool {
        self.texto.trim().is_empty() && self.imagenes.is_empty() && self.archivos.is_empty()
    }

    /// El pedido `chat` (sin `proyecto`: «Mensajes guardados").
    pub fn pedido(&self) -> String {
        serde_json::json!({
            "pixpin": 1,
            "accion": "chat",
            "texto": self.texto,
            "imagenes": self.imagenes,
            "archivos": self.archivos,
        })
        .to_string()
    }

    fn meter(&mut self, ruta: PathBuf) {
        if es_foto(&ruta) {
            self.imagenes.push(ruta);
        } else {
            self.archivos.push(ruta);
        }
    }
}

/// Lo que el chat guarda como foto (el resto, como adjunto: un GIF animado o
/// un SVG se quedan tal cual).
pub fn es_foto(ruta: &Path) -> bool {
    let ext = ruta
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp")
}

/// `pixpinmax.exe --al-chat a.pdf b.png`: las rutas que existen, repartidas.
/// `None` si la orden no esta o no queda ningun fichero.
pub fn de_argumentos(args: &[String]) -> Option<Recibido> {
    let i = args.iter().position(|a| a == ORDEN_AL_CHAT)?;
    let mut r = Recibido::default();
    for a in &args[i + 1..] {
        let p = PathBuf::from(a.trim_matches('"'));
        if p.is_file() {
            r.meter(p);
        }
    }
    (!r.vacio()).then_some(r)
}

/// La extension de una imagen por sus primeros bytes (el panel Compartir da
/// un flujo, sin nombre).
pub fn extension_de_imagen(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if bytes.starts_with(b"BM") {
        "bmp"
    } else if bytes.starts_with(b"GIF8") {
        "gif"
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "webp"
    } else {
        "png"
    }
}

/// **Junta lo compartido** y lo deja en `destino`: el texto y el enlace en un
/// solo texto (sin repetir el enlace si el texto ya lo trae), la imagen suelta
/// como fichero y cada fichero copiado (las carpetas no van). Es lo que hace
/// la activacion real, sin WinRT, para probarlo con datos falsos.
pub fn juntar(
    texto: Option<String>,
    enlace: Option<String>,
    imagen: Option<Vec<u8>>,
    ficheros: &[PathBuf],
    destino: &Path,
) -> std::io::Result<Recibido> {
    let mut r = Recibido::default();
    let texto = texto
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    let enlace = enlace
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    r.texto = match (texto, enlace) {
        (Some(t), Some(e)) if !t.contains(&e) => format!("{t}\n{e}"),
        (Some(t), _) => t,
        (None, Some(e)) => e,
        (None, None) => String::new(),
    };
    let hay_ficheros = ficheros.iter().any(|f| f.is_file());
    // Una app que comparte una foto suele dar el fichero Y la imagen suelta:
    // la suelta solo si no vino ningun fichero.
    let imagen = imagen.filter(|b| !b.is_empty() && !hay_ficheros);
    if imagen.is_some() || hay_ficheros {
        std::fs::create_dir_all(destino)?;
    }
    if let Some(bytes) = imagen {
        let ruta = libre(
            destino,
            &format!("Compartida.{}", extension_de_imagen(&bytes)),
        );
        std::fs::write(&ruta, bytes)?;
        r.meter(ruta);
    }
    for f in ficheros.iter().filter(|f| f.is_file()) {
        let nombre = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "fichero".into());
        let ruta = libre(destino, &nombre);
        std::fs::copy(f, &ruta)?;
        r.meter(ruta);
    }
    Ok(r)
}

/// `dir/nombre`, o `dir/nombre (2).ext`... si ya estaba.
fn libre(dir: &Path, nombre: &str) -> PathBuf {
    let ruta = dir.join(nombre);
    if !ruta.exists() {
        return ruta;
    }
    let p = Path::new(nombre);
    let base = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{base} ({n}){ext}")))
        .find(|r| !r.exists())
        .unwrap_or(ruta)
}

/// `true` si este proceso corre con identidad de paquete (el MSIX con
/// ubicacion externa esta registrado y Windows lo lanzo como tal).
pub fn tiene_identidad() -> bool {
    use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
    use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
    let mut largo = 0u32;
    // SAFETY: con buffer nulo y largo 0 solo escribe el largo necesario.
    let r = unsafe { GetCurrentPackageFullName(&mut largo, None) };
    // Sin paquete: APPMODEL_ERROR_NO_PACKAGE (15700).
    r == ERROR_INSUFFICIENT_BUFFER
}

/// **La activacion por Compartir**, si este arranque lo es. Lee lo
/// compartido, lo copia a `destino` y le contesta a Windows. `None` sin
/// identidad, si el arranque es otro o no vino nada.
pub fn leer_activacion(destino: &Path) -> Option<Recibido> {
    use windows::ApplicationModel::Activation::{ActivationKind, ShareTargetActivatedEventArgs};
    use windows::ApplicationModel::AppInstance;
    use windows::core::Interface;

    if !tiene_identidad() {
        return None;
    }
    let args = AppInstance::GetActivatedEventArgs().ok()?;
    if args.Kind().ok()? != ActivationKind::ShareTarget {
        return None;
    }
    let op = args
        .cast::<ShareTargetActivatedEventArgs>()
        .ok()?
        .ShareOperation()
        .ok()?;
    let _ = op.ReportStarted();
    let leido = leer_paquete(&op, destino);
    let _ = op.ReportDataRetrieved();
    let r = leido.and_then(|(texto, enlace, imagen, ficheros)| {
        juntar(texto, enlace, imagen, &ficheros, destino).ok()
    });
    match &r {
        Some(_) => {
            let _ = op.ReportCompleted();
        }
        None => {
            let _ = op.ReportError(&windows::core::HSTRING::from(
                "PixPin no pudo leer lo compartido",
            ));
        }
    }
    r.filter(|r| !r.vacio())
}

type Partes = (
    Option<String>,
    Option<String>,
    Option<Vec<u8>>,
    Vec<PathBuf>,
);

fn leer_paquete(
    op: &windows::ApplicationModel::DataTransfer::ShareTarget::ShareOperation,
    destino: &Path,
) -> Option<Partes> {
    use windows::ApplicationModel::DataTransfer::StandardDataFormats;
    use windows::Storage::Streams::{DataReader, IRandomAccessStreamWithContentType};
    use windows::Storage::{IStorageItem, StorageFile};
    use windows::core::{HSTRING, Interface};

    /// Un flujo de WinRT entero a memoria.
    fn todo(flujo: &IRandomAccessStreamWithContentType) -> windows::core::Result<Vec<u8>> {
        let largo = u32::try_from(flujo.Size()?).unwrap_or(u32::MAX);
        let lector = DataReader::CreateDataReader(flujo)?;
        let leidos = lector.LoadAsync(largo)?.join()?;
        let mut v = vec![0u8; leidos as usize];
        lector.ReadBytes(&mut v)?;
        Ok(v)
    }

    let datos = op.Data().ok()?;
    let tiene =
        |f: windows::core::Result<HSTRING>| f.and_then(|f| datos.Contains(&f)).unwrap_or(false);

    let texto = if tiene(StandardDataFormats::Text()) {
        datos
            .GetTextAsync()
            .and_then(|o| o.join())
            .ok()
            .map(|h| h.to_string())
    } else {
        None
    };
    let enlace = if tiene(StandardDataFormats::WebLink()) {
        datos
            .GetWebLinkAsync()
            .and_then(|o| o.join())
            .and_then(|u| u.AbsoluteUri())
            .ok()
            .map(|h| h.to_string())
    } else {
        None
    };
    let imagen = if tiene(StandardDataFormats::Bitmap()) {
        datos
            .GetBitmapAsync()
            .and_then(|o| o.join())
            .and_then(|r| r.OpenReadAsync())
            .and_then(|o| o.join())
            .and_then(|f| todo(&f))
            .ok()
    } else {
        None
    };
    let mut ficheros = Vec::new();
    if tiene(StandardDataFormats::StorageItems()) {
        if let Ok(items) = datos.GetStorageItemsAsync().and_then(|o| o.join()) {
            for i in 0..items.Size().unwrap_or(0) {
                let Ok(item): windows::core::Result<IStorageItem> = items.GetAt(i) else {
                    continue;
                };
                let ruta = item
                    .Path()
                    .map(|p| PathBuf::from(p.to_string()))
                    .unwrap_or_default();
                if ruta.is_file() {
                    ficheros.push(ruta);
                    continue;
                }
                // Un fichero sin ruta en el disco (de una app de la tienda):
                // se lee entero y se deja en una carpeta aparte.
                let copiado = (|| -> windows::core::Result<PathBuf> {
                    let f = item.cast::<StorageFile>()?;
                    let bytes = todo(&f.OpenReadAsync()?.join()?)?;
                    let tmp = destino.join("winrt");
                    let _ = std::fs::create_dir_all(&tmp);
                    let p = libre(&tmp, &f.Name()?.to_string());
                    std::fs::write(&p, bytes).map_err(|_| windows::core::Error::empty())?;
                    Ok(p)
                })();
                if let Ok(p) = copiado {
                    ficheros.push(p);
                }
            }
        }
    }
    Some((texto, enlace, imagen, ficheros))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn dir(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-compartido-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// La activacion con datos falsos: un enlace con su texto, una foto y un
    /// PDF compartidos desde «otra app», que los borra al contestarle.
    #[test]
    fn compartir_texto_enlace_foto_y_pdf_va_al_chat() {
        let d = dir("todo");
        let origen = d.join("otra-app");
        std::fs::create_dir_all(origen.join("carpeta")).unwrap();
        let foto = origen.join("foto.JPG");
        let pdf = origen.join("plano.pdf");
        std::fs::write(&foto, [0xFF, 0xD8, 0xFF, 0xE0, 1, 2]).unwrap();
        std::fs::write(&pdf, b"%PDF-1.4 falso %%EOF").unwrap();
        let destino = d.join("cache").join("compartido").join("1");
        let r = juntar(
            Some("  mira esto ".into()),
            Some("https://ejemplo.pe/obra".into()),
            Some(b"\x89PNGsuelta".to_vec()),
            &[
                foto.clone(),
                pdf.clone(),
                origen.join("carpeta"),
                origen.join("no-esta.txt"),
            ],
            &destino,
        )
        .unwrap();
        assert_eq!(r.texto, "mira esto\nhttps://ejemplo.pe/obra");
        // Con ficheros, la imagen suelta (la misma foto) no se duplica.
        assert_eq!(r.imagenes, vec![destino.join("foto.JPG")]);
        assert_eq!(r.archivos, vec![destino.join("plano.pdf")]);
        // La app que compartio borra lo suyo: lo nuestro sigue.
        std::fs::remove_dir_all(&origen).unwrap();
        assert!(r.imagenes[0].is_file() && r.archivos[0].is_file());

        let v: serde_json::Value = serde_json::from_str(&r.pedido()).unwrap();
        assert_eq!(v["pixpin"], 1);
        assert_eq!(v["accion"], "chat");
        assert!(
            v.get("proyecto").is_none(),
            "sin proyecto: Mensajes guardados"
        );
        assert_eq!(
            v["archivos"][0].as_str().unwrap(),
            destino.join("plano.pdf").to_str().unwrap()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn compartir_solo_una_imagen_o_solo_un_enlace() {
        let d = dir("suelta");
        let r = juntar(None, None, Some(b"\x89PNG\r\n\x1a\n".to_vec()), &[], &d).unwrap();
        assert_eq!(r.imagenes, vec![d.join("Compartida.png")]);
        let r2 = juntar(None, None, Some(vec![0xFF, 0xD8, 0xFF, 0]), &[], &d).unwrap();
        assert_eq!(r2.imagenes, vec![d.join("Compartida.jpg")]);
        // Otra imagen igual de nombre: no pisa la primera.
        let r3 = juntar(None, None, Some(b"\x89PNG..".to_vec()), &[], &d).unwrap();
        assert_eq!(r3.imagenes, vec![d.join("Compartida (2).png")]);

        let e = juntar(
            Some("https://a.pe".into()),
            Some("https://a.pe".into()),
            None,
            &[],
            &d,
        )
        .unwrap();
        assert_eq!(e.texto, "https://a.pe", "el enlace no se repite");
        assert!(e.imagenes.is_empty() && e.archivos.is_empty());
        // Nada de nada: vacio (no se manda pedido).
        let n = juntar(Some("   ".into()), None, Some(vec![]), &[], &d.join("nada")).unwrap();
        assert!(n.vacio());
        assert!(
            !d.join("nada").exists(),
            "sin nada que guardar no crea carpetas"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn al_chat_por_linea_de_mandatos() {
        let d = dir("args");
        let png = d.join("a.png");
        let xlsx = d.join("b.xlsx");
        std::fs::write(&png, b"x").unwrap();
        std::fs::write(&xlsx, b"x").unwrap();
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let r = de_argumentos(&args(&[
            ORDEN_AL_CHAT,
            png.to_str().unwrap(),
            &format!("\"{}\"", xlsx.display()),
            d.join("no-esta.pdf").to_str().unwrap(),
            d.to_str().unwrap(),
        ]))
        .unwrap();
        assert_eq!(r.imagenes, vec![png]);
        assert_eq!(r.archivos, vec![xlsx]);
        assert!(r.texto.is_empty());
        // Sin la orden, o sin ficheros que existan: nada.
        assert!(de_argumentos(&args(&["--atajo", "capturar"])).is_none());
        assert!(de_argumentos(&args(&[ORDEN_AL_CHAT, "Z:\\no\\esta.pdf"])).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn extension_por_los_primeros_bytes() {
        assert_eq!(extension_de_imagen(b"GIF89a"), "gif");
        assert_eq!(extension_de_imagen(b"BM...."), "bmp");
        assert_eq!(extension_de_imagen(b"RIFF\0\0\0\0WEBPVP8 "), "webp");
        assert_eq!(extension_de_imagen(b"??"), "png");
    }

    /// Las pruebas no corren empaquetadas: sin identidad, nada de
    /// activacion (y nada de esperar ni de fallar).
    #[test]
    fn sin_identidad_no_hay_activacion() {
        assert!(!tiene_identidad());
        assert!(leer_activacion(&std::env::temp_dir()).is_none());
    }
}
