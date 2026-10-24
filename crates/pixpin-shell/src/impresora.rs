//! **«Imprimir a PixPin»** (15-oct-2026): la impresora «PixPin» es el
//! controlador «Microsoft Print to PDF» que ya trae Windows con un puerto
//! local que es **un fichero** (`<raiz>\cache\impresora\PixPin.pdf`). La crea
//! y la quita `herramientas/impresora-pixpin.ps1` (pide administrador una
//! vez). Aqui vive el lado de la app: ver que llego un PDF, esperar a que este
//! **terminado** y llevarlo a su sitio con un nombre propio.
//!
//! ## Por que un fichero fijo y no una carpeta
//!
//! Un puerto local de Windows escribe siempre en la misma ruta: la impresion
//! siguiente pisaria la anterior. Por eso, en cuanto el PDF esta entero, se
//! **mueve** a `<raiz>\impresos\Impreso AAAA-MM-DD HH.MM.SS.pdf` (mover en el
//! mismo disco es renombrar: instantaneo) y el puerto queda libre.
//!
//! ## Cuando esta terminado (y no a medias)
//!
//! El administrador de impresion escribe el PDF poco a poco y lo tiene abierto
//! mientras tanto. Se da por terminado cuando se cumplen las dos cosas:
//! 1. se puede abrir **en exclusiva** (nadie mas lo tiene abierto), y
//! 2. empieza por `%PDF-` y acaba en `%%EOF` (lo ultimo que escribe).
//!
//! Lo primero solo no basta (entre dos escrituras el fichero puede quedar
//! cerrado un instante en algunos controladores); lo segundo solo tampoco (un
//! `%%EOF` de una revision intermedia). Si en un minuto no llega a estar
//! entero, es basura (un trabajo cancelado): se aparta a `descartados\` para
//! que no tape la impresion siguiente.
//!
//! ## Coste
//!
//! Un hilo que duerme en el aviso de cambios de Windows
//! ([`crate::vigia_carpeta`]); solo mira el disco cuando algo cambio en la
//! carpeta. Sin la impresora instalada la carpeta no existe y no se arranca
//! nada.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::vigia_carpeta::VigiaCarpeta;

/// El nombre del fichero que escribe el puerto de la impresora.
pub const FICHERO_DEL_PUERTO: &str = "PixPin.pdf";
/// El nombre de la impresora en Windows.
pub const NOMBRE_IMPRESORA: &str = "PixPin";

/// La carpeta que vigila la app (y donde escribe el puerto).
pub fn carpeta(raiz: &Path) -> PathBuf {
    raiz.join("cache").join("impresora")
}

/// Donde se quedan los PDF impresos, ya con su nombre.
pub fn carpeta_impresos(raiz: &Path) -> PathBuf {
    raiz.join("impresos")
}

/// Como esta el fichero del puerto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// No hay nada que recoger.
    NoEsta,
    /// Lo estan escribiendo (abierto por otro, o sin su final).
    AMedias,
    /// Entero y cerrado: se puede mover.
    Terminado,
}

/// `true` si `cabeza` y `cola` son las de un PDF entero: empieza por `%PDF-`
/// y lo ultimo (salvo espacios y saltos) es `%%EOF`.
pub fn pdf_entero(cabeza: &[u8], cola: &[u8]) -> bool {
    if !cabeza.starts_with(b"%PDF-") {
        return false;
    }
    let mut fin = cola.len();
    while fin > 0 && matches!(cola[fin - 1], b' ' | b'\r' | b'\n' | b'\t' | 0) {
        fin -= 1;
    }
    cola[..fin].ends_with(b"%%EOF")
}

/// Mira el fichero del puerto sin tocarlo: lo abre en exclusiva (si otro lo
/// tiene abierto, esta a medias) y lee el principio y el final.
pub fn estado(ruta: &Path) -> Estado {
    if !ruta.is_file() {
        return Estado::NoEsta;
    }
    // `share_mode(0)`: falla con «violacion de uso compartido» mientras el
    // administrador de impresion lo tenga abierto para escribir.
    let Ok(mut f) = OpenOptions::new().read(true).share_mode(0).open(ruta) else {
        return Estado::AMedias;
    };
    let mut cabeza = [0u8; 8];
    let leidos = f.read(&mut cabeza).unwrap_or(0);
    let largo = f.metadata().map(|m| m.len()).unwrap_or(0);
    let tramo = largo.min(1024);
    let mut cola = vec![0u8; tramo as usize];
    if f.seek(SeekFrom::End(-(tramo as i64))).is_err() || f.read_exact(&mut cola).is_err() {
        return Estado::AMedias;
    }
    if pdf_entero(&cabeza[..leidos], &cola) {
        Estado::Terminado
    } else {
        Estado::AMedias
    }
}

/// Fecha y hora civiles de unos milisegundos «locales» desde 1970
/// (`entorno::ahora_local_ms`): (año, mes, dia, hora, minuto, segundo).
pub fn civil(ms: i64) -> (i64, u32, u32, u32, u32, u32) {
    let seg = ms.div_euclid(1000);
    let dias = seg.div_euclid(86_400);
    let s = seg.rem_euclid(86_400);
    // Algoritmo de Howard Hinnant (dias desde 1970 -> fecha gregoriana).
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        y,
        m,
        d,
        (s / 3600) as u32,
        ((s / 60) % 60) as u32,
        (s % 60) as u32,
    )
}

/// El nombre de un impreso: `Impreso AAAA-MM-DD HH.MM.SS.pdf` (con `(2)`,
/// `(3)`... si en ese segundo ya habia otro).
pub fn nombre_libre(dir: &Path, ms_local: i64) -> PathBuf {
    let (y, mo, d, h, mi, s) = civil(ms_local);
    let base = format!("Impreso {y:04}-{mo:02}-{d:02} {h:02}.{mi:02}.{s:02}");
    let mut ruta = dir.join(format!("{base}.pdf"));
    let mut n = 2;
    while ruta.exists() {
        ruta = dir.join(format!("{base} ({n}).pdf"));
        n += 1;
    }
    ruta
}

/// Si el PDF del puerto esta terminado, lo mueve a `impresos` con su nombre
/// y devuelve la ruta nueva. `None` si no hay nada o aun esta a medias.
pub fn recoger(carpeta: &Path, impresos: &Path, ms_local: i64) -> Option<PathBuf> {
    let puerto = carpeta.join(FICHERO_DEL_PUERTO);
    if estado(&puerto) != Estado::Terminado {
        return None;
    }
    std::fs::create_dir_all(impresos).ok()?;
    let destino = nombre_libre(impresos, ms_local);
    match std::fs::rename(&puerto, &destino) {
        Ok(()) => Some(destino),
        // Otro disco (modo portable con la raiz en un USB y el puerto no):
        // copiar y borrar.
        Err(_) => {
            std::fs::copy(&puerto, &destino).ok()?;
            let _ = std::fs::remove_file(&puerto);
            Some(destino)
        }
    }
}

/// Aparta un fichero del puerto que nunca llego a estar entero, para que no
/// tape la impresion siguiente. Devuelve donde quedo.
pub fn descartar(carpeta: &Path, ms_local: i64) -> Option<PathBuf> {
    let puerto = carpeta.join(FICHERO_DEL_PUERTO);
    let dir = carpeta.join("descartados");
    std::fs::create_dir_all(&dir).ok()?;
    let destino = nombre_libre(&dir, ms_local);
    std::fs::rename(&puerto, &destino).ok()?;
    Some(destino)
}

/// Cuanto se espera a un PDF a medias antes de darlo por basura.
pub const PACIENCIA: Duration = Duration::from_secs(60);

/// **Vigila la carpeta de la impresora** en un hilo propio y llama a
/// `al_llegar` con la ruta de cada PDF terminado, ya movido a `impresos`.
/// `None` si la carpeta no existe (la impresora no esta instalada).
pub fn vigilar(
    carpeta: PathBuf,
    impresos: PathBuf,
    ahora_local_ms: fn() -> i64,
    al_llegar: impl Fn(PathBuf) + Send + 'static,
) -> Option<std::thread::JoinHandle<()>> {
    let vigia = VigiaCarpeta::nueva(&carpeta)?;
    std::thread::Builder::new()
        .name("impresora-pixpin".into())
        .spawn(move || {
            // Lo que se imprimio con la app cerrada, primero.
            let mut desde: Option<Instant> = Some(Instant::now());
            loop {
                if desde.is_none() {
                    // Dormir hasta que cambie algo (o un rato, por si el
                    // aviso se perdiera).
                    vigia.esperar(Duration::from_secs(300));
                    desde = Some(Instant::now());
                }
                let puerto = carpeta.join(FICHERO_DEL_PUERTO);
                match estado(&puerto) {
                    Estado::NoEsta => desde = None,
                    Estado::Terminado => {
                        if let Some(r) = recoger(&carpeta, &impresos, ahora_local_ms()) {
                            tracing::info!(ruta = ?r, "impreso recibido");
                            al_llegar(r);
                        }
                        desde = None;
                    }
                    Estado::AMedias => {
                        if desde.is_some_and(|t| t.elapsed() > PACIENCIA) {
                            tracing::warn!("impreso a medias demasiado tiempo; se aparta");
                            let _ = descartar(&carpeta, ahora_local_ms());
                            desde = None;
                        } else {
                            // Escribiendo: mirar de nuevo enseguida.
                            vigia.esperar(Duration::from_millis(150));
                        }
                    }
                }
            }
        })
        .ok()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::Write;

    /// Un PDF minimo de verdad (una pagina en blanco), como los de muestra.
    const PDF: &[u8] = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]>>endobj\n\
trailer<</Root 1 0 R>>\n%%EOF\r\n";

    fn carpeta_de_prueba(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-impresora-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn pdf_entero_mira_principio_y_final() {
        assert!(pdf_entero(b"%PDF-1.7", b"...\n%%EOF"));
        assert!(pdf_entero(b"%PDF-1.7", b"...%%EOF\r\n \0"));
        assert!(!pdf_entero(b"%PDF-1.7", b"...%%EO"));
        assert!(!pdf_entero(b"%PDF-1.7", b"%%EOF\nmas cosas"));
        assert!(!pdf_entero(b"PK\x03\x04", b"%%EOF"));
        assert!(!pdf_entero(b"", b""));
    }

    #[test]
    fn civil_da_la_fecha() {
        assert_eq!(civil(0), (1970, 1, 1, 0, 0, 0));
        // 2026-10-15 14:03:22 (sin huso: ya es «local»).
        assert_eq!(civil(1_792_073_002_000), (2026, 10, 15, 14, 3, 22));
        // 29 de febrero de un bisiesto.
        assert_eq!(civil(951_782_400_000), (2000, 2, 29, 0, 0, 0));
    }

    #[test]
    fn a_medias_mientras_lo_escriben_y_terminado_al_cerrarlo() {
        let d = carpeta_de_prueba("medias");
        let puerto = d.join(FICHERO_DEL_PUERTO);
        assert_eq!(estado(&puerto), Estado::NoEsta);
        // El administrador de impresion lo tiene abierto, sin compartir la
        // escritura, y aun no ha escrito el final.
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .share_mode(1)
            .open(&puerto)
            .unwrap();
        f.write_all(&PDF[..40]).unwrap();
        f.flush().unwrap();
        assert_eq!(estado(&puerto), Estado::AMedias);
        // Escribe el resto pero aun no lo cierra: sigue a medias.
        f.write_all(&PDF[40..]).unwrap();
        f.flush().unwrap();
        assert_eq!(estado(&puerto), Estado::AMedias);
        drop(f);
        assert_eq!(estado(&puerto), Estado::Terminado);
        // Cerrado pero cortado (un trabajo cancelado): a medias.
        std::fs::write(&puerto, &PDF[..PDF.len() - 8]).unwrap();
        assert_eq!(estado(&puerto), Estado::AMedias);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn recoger_mueve_con_nombre_propio_y_no_pisa() {
        let d = carpeta_de_prueba("recoger");
        let impresos = d.join("impresos");
        let ms = 1_792_073_002_000;
        // Nada que recoger.
        assert!(recoger(&d, &impresos, ms).is_none());
        // A medias: no se toca.
        std::fs::write(d.join(FICHERO_DEL_PUERTO), &PDF[..30]).unwrap();
        assert!(recoger(&d, &impresos, ms).is_none());
        assert!(d.join(FICHERO_DEL_PUERTO).is_file());
        // Entero: se mueve.
        std::fs::write(d.join(FICHERO_DEL_PUERTO), PDF).unwrap();
        let r = recoger(&d, &impresos, ms).unwrap();
        assert_eq!(r, impresos.join("Impreso 2026-10-15 14.03.22.pdf"));
        assert_eq!(std::fs::read(&r).unwrap(), PDF);
        assert!(!d.join(FICHERO_DEL_PUERTO).exists());
        // Otro en el mismo segundo: no pisa al primero.
        std::fs::write(d.join(FICHERO_DEL_PUERTO), PDF).unwrap();
        let r2 = recoger(&d, &impresos, ms).unwrap();
        assert_eq!(r2, impresos.join("Impreso 2026-10-15 14.03.22 (2).pdf"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// De punta a punta: el hilo ve un PDF de muestra que «se imprime» a
    /// trozos en la carpeta y avisa una sola vez, con el PDF ya entero.
    #[test]
    fn vigilar_avisa_cuando_llega_entero() {
        let d = carpeta_de_prueba("vigilar");
        let impresos = d.join("impresos");
        // Sin carpeta, no se vigila nada.
        assert!(vigilar(d.join("no-esta"), impresos.clone(), || 0, |_| {}).is_none());
        // Un PDF de muestra de verdad si esta, si no el minimo.
        let muestra = std::path::Path::new(r"E:\pixpin-demo-om")
            .read_dir()
            .ok()
            .and_then(|mut it| {
                it.find_map(|e| {
                    let p = e.ok()?.path();
                    let es_pdf = p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf"));
                    (es_pdf && p.metadata().ok()?.len() < 20_000_000).then_some(p)
                })
            })
            .and_then(|p| std::fs::read(p).ok())
            .filter(|b| pdf_entero(&b[..8.min(b.len())], &b[b.len().saturating_sub(1024)..]))
            .unwrap_or_else(|| PDF.to_vec());
        let (tx, rx) = std::sync::mpsc::channel();
        let empezo = Instant::now();
        vigilar(
            d.clone(),
            impresos.clone(),
            || 1_792_073_002_000,
            move |r| {
                let _ = tx.send((r, empezo.elapsed()));
            },
        )
        .unwrap();
        // «Imprimir»: abierto en exclusiva de escritura, a trozos.
        let puerto = d.join(FICHERO_DEL_PUERTO);
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .share_mode(1)
            .open(&puerto)
            .unwrap();
        let mitad = muestra.len() / 2;
        f.write_all(&muestra[..mitad]).unwrap();
        f.flush().unwrap();
        std::thread::sleep(Duration::from_millis(400));
        assert!(rx.try_recv().is_err(), "avisó con el PDF a medias");
        f.write_all(&muestra[mitad..]).unwrap();
        let cerrado = Instant::now();
        drop(f);
        let (r, _) = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("no avisó del PDF terminado");
        let latencia = cerrado.elapsed();
        eprintln!(
            "PDF de {} bytes recogido {latencia:?} despues de cerrarse",
            muestra.len()
        );
        assert_eq!(std::fs::read(&r).unwrap(), muestra);
        assert!(r.starts_with(&impresos));
        assert!(
            rx.recv_timeout(Duration::from_millis(500)).is_err(),
            "avisó dos veces"
        );
        // La carpeta se queda: el hilo sigue vivo hasta que acabe la prueba.
    }
}
