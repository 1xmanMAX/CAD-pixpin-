//! **Juntar las copias que lanza el Explorador** con varios ficheros
//! elegidos.
//!
//! Un verbo del menu contextual clasico (`HKCU\Software\Classes\*\shell`)
//! con una linea de ordenes no recibe todos los ficheros elegidos: el
//! Explorador lanza **una copia por fichero**, casi a la vez. Para «Pinear»
//! da igual, pero «Copiar texto (OCR)» de tres capturas copiaria solo la
//! ultima en llegar, «Al chat» mandaria tres mensajes y «Preguntar al
//! asistente» abriria tres cajas.
//!
//! Asi que cada copia apunta sus rutas en un fichero de una carpeta comun
//! (`%TEMP%\pixpin-juntar\<clave>`) y luego pide un mutex con nombre. La
//! primera que lo consigue espera a que dejen de llegar ficheros (un rato
//! corto de silencio), se los lleva todos y suelta el mutex. Las demas,
//! cuando lo consiguen, miran si su fichero sigue alli: si ya no esta, se
//! lo llevo la primera y se van sin hacer nada; si sigue (llegaron tarde),
//! hacen lo mismo con lo que quede. Ningun fichero se queda huerfano.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
use windows::core::HSTRING;

/// Cuanto silencio (ningun fichero nuevo) da por llegadas a todas las
/// copias. El Explorador las lanza en unas decenas de milisegundos.
pub const SILENCIO: Duration = Duration::from_millis(200);

/// Lo mas que se espera a que dejen de llegar.
pub const TOPE: Duration = Duration::from_secs(3);

/// La carpeta de una clave (el verbo y lo que no es fichero).
pub fn carpeta(clave: &str) -> PathBuf {
    std::env::temp_dir()
        .join("pixpin-juntar")
        .join(nombre_seguro(clave))
}

/// Un nombre de carpeta y de mutex a partir de la clave: solo letras y
/// cifras, y un resumen para que dos claves distintas no choquen.
fn nombre_seguro(clave: &str) -> String {
    let limpio: String = clave
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(24)
        .collect();
    // FNV-1a: estable entre procesos (el `DefaultHasher` no lo es).
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in clave.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{limpio}-{h:016x}")
}

/// **Junta las rutas de todas las copias lanzadas con la misma `clave`.**
/// `Some(todas)` para la copia que se queda con el trabajo (en el orden en
/// que llegaron); `None` para las que se van porque otra ya lo hara.
pub fn juntar(clave: &str, mias: &[String]) -> Option<Vec<String>> {
    juntar_en(
        &carpeta(clave),
        &format!("Local\\PixPinMax-juntar-{}", nombre_seguro(clave)),
        mias,
        SILENCIO,
    )
}

/// [`juntar`] con su carpeta, su mutex y su silencio (las pruebas).
pub fn juntar_en(
    dir: &Path,
    mutex: &str,
    mias: &[String],
    silencio: Duration,
) -> Option<Vec<String>> {
    // Sin donde apuntar, cada copia hace lo suyo: peor que juntar, pero
    // mejor que no hacer nada.
    if std::fs::create_dir_all(dir).is_err() {
        return Some(mias.to_vec());
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mio = dir.join(format!("{nanos:020}-{}.txt", std::process::id()));
    if std::fs::write(&mio, mias.join("\n")).is_err() {
        return Some(mias.to_vec());
    }
    let nombre = HSTRING::from(mutex);
    // SAFETY: nombre vivo durante la llamada; sin atributos de seguridad.
    let Ok(h) = (unsafe { CreateMutexW(None, false, &nombre) }) else {
        let _ = std::fs::remove_file(&mio);
        return Some(mias.to_vec());
    };
    // SAFETY: `h` es nuestra; esperar no toca memoria.
    let r = unsafe { WaitForSingleObject(h, 10_000) };
    let mio_sigue = mio.is_file();
    let resultado = if r != WAIT_OBJECT_0 && r != WAIT_ABANDONED {
        // No se consiguio a tiempo: se hace lo propio, sin juntar.
        let _ = std::fs::remove_file(&mio);
        Some(mias.to_vec())
    } else if !mio_sigue {
        None
    } else {
        Some(recoger(dir, silencio))
    };
    if r == WAIT_OBJECT_0 || r == WAIT_ABANDONED {
        // SAFETY: lo tenemos; soltarlo deja pasar a la siguiente copia.
        let _ = unsafe { ReleaseMutex(h) };
    }
    // SAFETY: manija nuestra, no se usa despues.
    let _ = unsafe { CloseHandle(h) };
    resultado
}

/// Espera a que no lleguen mas y se lleva todas (borrando sus ficheros).
fn recoger(dir: &Path, silencio: Duration) -> Vec<String> {
    let inicio = Instant::now();
    let mut cuantos = contar(dir);
    let mut quieto = Instant::now();
    while quieto.elapsed() < silencio && inicio.elapsed() < TOPE {
        std::thread::sleep(Duration::from_millis(25));
        let ahora = contar(dir);
        if ahora != cuantos {
            cuantos = ahora;
            quieto = Instant::now();
        }
    }
    let mut ficheros: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|l| {
            l.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "txt"))
                .collect()
        })
        .unwrap_or_default();
    ficheros.sort();
    let mut todas = Vec::new();
    for f in ficheros {
        if let Ok(t) = std::fs::read_to_string(&f) {
            todas.extend(t.lines().filter(|l| !l.is_empty()).map(str::to_string));
        }
        let _ = std::fs::remove_file(&f);
    }
    todas
}

fn contar(dir: &Path) -> usize {
    std::fs::read_dir(dir).map(|l| l.count()).unwrap_or(0)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn varias_copias_a_la_vez_dan_un_solo_lote() {
        let dir = std::env::temp_dir().join(format!("pixpin-juntar-prueba-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mutex = format!("Local\\PixPinMax-juntar-prueba-{}", std::process::id());
        let hilos: Vec<_> = (0..5)
            .map(|i| {
                let (dir, mutex) = (dir.clone(), mutex.clone());
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(i * 15));
                    juntar_en(
                        &dir,
                        &mutex,
                        &[format!("C:\\f{i}.png")],
                        Duration::from_millis(150),
                    )
                })
            })
            .collect();
        let r: Vec<Option<Vec<String>>> = hilos.into_iter().map(|h| h.join().unwrap()).collect();
        let lotes: Vec<&Vec<String>> = r.iter().flatten().collect();
        // Todas las rutas salen una sola vez, entre todos los lotes.
        let mut todas: Vec<String> = lotes.iter().flat_map(|l| l.iter().cloned()).collect();
        todas.sort();
        assert_eq!(
            todas,
            (0..5).map(|i| format!("C:\\f{i}.png")).collect::<Vec<_>>()
        );
        // Y llegando casi a la vez, un solo lote.
        assert_eq!(lotes.len(), 1, "{r:?}");
        assert_eq!(contar(&dir), 0, "no queda ningun fichero huerfano");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn caso_negativo_claves_distintas_no_se_mezclan() {
        assert_ne!(nombre_seguro("chat"), nombre_seguro("pinear"));
        assert_ne!(carpeta("chat --proyecto A"), carpeta("chat --proyecto B"));
        assert!(
            nombre_seguro("a\\b:c*?")
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        );
    }
}
