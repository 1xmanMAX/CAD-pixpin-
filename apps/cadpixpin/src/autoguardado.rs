//! **El autoguardado y la recuperacion**: cada pocos minutos, si el plano
//! tiene cambios sin guardar, una copia en DXF en
//! `%APPDATA%\CAD-pixpin\autoguardado\<nombre>-<hash>.dxf` (con un `.txt` al
//! lado que dice de que archivo es). Se escribe en otro hilo con una copia
//! del documento; al guardar de verdad (o al cerrar sin guardar) se borra.
//! Si al arrancar queda alguna mas nueva que su original, el inicio la
//! ofrece para recuperarla.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use cad_nucleo::Dibujo;
use cad_nucleo::opencadcodec::{CadDocument, DxfWriter};

/// Cada cuantos minutos (se cambia con la variable de entorno
/// `CADPIXPIN_AUTOGUARDADO`, en minutos; 0 lo apaga).
pub const MINUTOS: u64 = 5;

pub fn intervalo() -> Option<Duration> {
    let m = std::env::var("CADPIXPIN_AUTOGUARDADO")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(MINUTOS);
    (m > 0).then(|| Duration::from_secs(m * 60))
}

pub fn carpeta() -> PathBuf {
    crate::recientes::carpeta().join("autoguardado")
}

/// FNV-1a de 64 bits (estable entre versiones, a diferencia del de `std`).
pub fn hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

/// **El nombre de la copia** de `original` (o de un plano sin nombre, por su
/// numero `id`): el nombre del archivo y un hash de su ruta entera, para que
/// dos `Planta.dwg` de carpetas distintas no se pisen.
pub fn nombre_para(original: Option<&Path>, id: u64) -> String {
    match original {
        Some(r) => {
            let base: String = r
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || "-_ ".contains(c) {
                        c
                    } else {
                        '_'
                    }
                })
                .take(60)
                .collect();
            let clave = r.to_string_lossy().to_lowercase();
            format!("{base}-{:016x}.dxf", hash(&clave))
        }
        None => format!("SinNombre-{id:016x}.dxf"),
    }
}

/// El `.txt` de al lado: la ruta del original (vacio si no tenia nombre).
fn origen_de(copia: &Path) -> PathBuf {
    copia.with_extension("txt")
}

/// **Si una copia merece ofrecerse**: es mas nueva que su original, o el
/// original ya no esta (o nunca tuvo nombre).
pub fn merece(copia: SystemTime, original: Option<SystemTime>) -> bool {
    original.is_none_or(|o| copia > o)
}

/// Un plano que se puede recuperar.
#[derive(Debug, Clone, PartialEq)]
pub struct Recuperable {
    pub copia: PathBuf,
    pub original: Option<PathBuf>,
    pub cuando: SystemTime,
}

impl Recuperable {
    /// Como se llama en la lista.
    pub fn nombre(&self) -> String {
        self.original
            .as_ref()
            .and_then(|o| o.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Plano sin nombre".into())
    }
}

/// Las copias que hay que ofrecer al arrancar, la mas nueva primero.
pub fn buscar() -> Vec<Recuperable> {
    let Ok(dir) = std::fs::read_dir(carpeta()) else {
        return Vec::new();
    };
    let mut v: Vec<Recuperable> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dxf")))
        .filter_map(|copia| {
            let cuando = std::fs::metadata(&copia).ok()?.modified().ok()?;
            let texto = std::fs::read_to_string(origen_de(&copia)).ok()?;
            let original = Some(PathBuf::from(texto.trim())).filter(|o| !o.as_os_str().is_empty());
            let t_original = original
                .as_ref()
                .and_then(|o| std::fs::metadata(o).ok()?.modified().ok());
            merece(cuando, t_original).then_some(Recuperable {
                copia,
                original,
                cuando,
            })
        })
        .collect();
    v.sort_by_key(|r| std::cmp::Reverse(r.cuando));
    v
}

/// Borra una copia y su `.txt`.
pub fn descartar(copia: &Path) {
    let _ = std::fs::remove_file(copia);
    let _ = std::fs::remove_file(origen_de(copia));
}

/// «hace 3 min», «hace 2 h», «hace 4 dias».
pub fn hace(d: Duration) -> String {
    let m = d.as_secs() / 60;
    match m {
        0 => "hace un momento".into(),
        1..=59 => format!("hace {m} min"),
        60..=2879 => format!("hace {} h", m / 60),
        _ => format!("hace {} dias", m / 1440),
    }
}

/// Escribe `doc` en `destino` en otro hilo, pasando por un temporal. Si
/// mientras tanto `vigente` deja de ser `turno` (se guardo de verdad o hay
/// otra copia mas nueva), no deja nada.
fn escribir(
    doc: CadDocument,
    destino: PathBuf,
    original: Option<PathBuf>,
    vigente: Arc<AtomicU64>,
    turno: u64,
) {
    let _ = std::thread::Builder::new()
        .name("autoguardado".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let tmp = destino.with_extension("dxf.tmp");
            let r = std::fs::create_dir_all(carpeta())
                .map_err(|e| e.to_string())
                .and_then(|()| {
                    std::panic::catch_unwind(|| {
                        DxfWriter::new(&doc)
                            .write_to_file(&tmp)
                            .map_err(|e| e.to_string())
                    })
                    .unwrap_or_else(|_| Err("fallo al escribir".into()))
                });
            if let Err(e) = r {
                tracing::warn!(%e, "no se pudo autoguardar");
                let _ = std::fs::remove_file(&tmp);
                return;
            }
            if vigente.load(Ordering::SeqCst) != turno {
                let _ = std::fs::remove_file(&tmp);
                return;
            }
            let origen = original
                .map(|o| o.display().to_string())
                .unwrap_or_default();
            if std::fs::write(origen_de(&destino), origen).is_err()
                || std::fs::rename(&tmp, &destino).is_err()
            {
                let _ = std::fs::remove_file(&tmp);
            }
        });
}

/// **El autoguardado de una mesa.**
pub struct Estado {
    /// El numero del plano sin nombre (para su copia).
    id: u64,
    desde: Instant,
    /// La version del dibujo de la ultima copia.
    version: u64,
    /// La ultima copia escrita (o pedida).
    escrita: Option<PathBuf>,
    /// Una copia que hay que seguir usando (la de un plano recuperado).
    fija: Option<PathBuf>,
    vigente: Arc<AtomicU64>,
}

impl Estado {
    pub fn nuevo() -> Estado {
        let id = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64)
            ^ ((std::process::id() as u64) << 32);
        Estado {
            id,
            desde: Instant::now(),
            version: 0,
            escrita: None,
            fija: None,
            vigente: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Para un plano recuperado de `copia`: se sigue escribiendo ahi.
    pub fn recuperado(copia: PathBuf) -> Estado {
        Estado {
            escrita: Some(copia.clone()),
            fija: Some(copia),
            ..Estado::nuevo()
        }
    }

    /// Si toca hacer una copia ahora: con cambios sin guardar desde la
    /// ultima, pasado el tiempo (un plano nuevo sin tocar no cuenta).
    pub fn toca(&self, d: &Dibujo, ahora: Instant, cada: Duration) -> bool {
        d.sucio()
            && d.version() > 1
            && d.version() != self.version
            && ahora.duration_since(self.desde) >= cada
    }

    /// **Hace la copia** (en otro hilo, con un clon del documento).
    pub fn lanzar(&mut self, d: &Dibujo) {
        let destino = self
            .fija
            .clone()
            .unwrap_or_else(|| carpeta().join(nombre_para(d.ruta.as_deref(), self.id)));
        // La de antes, si era de otro nombre (se guardo como…), sobra.
        if let Some(vieja) = self.escrita.take()
            && vieja != destino
        {
            descartar(&vieja);
        }
        let turno = self.vigente.fetch_add(1, Ordering::SeqCst) + 1;
        escribir(
            // Las referencias externas, como referencias (sin su contenido).
            cad_nucleo::xref::para_guardar(&d.doc).unwrap_or_else(|| d.doc.clone()),
            destino.clone(),
            d.ruta.clone(),
            self.vigente.clone(),
            turno,
        );
        self.escrita = Some(destino);
        self.version = d.version();
        self.desde = Instant::now();
    }

    /// **Se guardo de verdad** (o se cierra sin guardar): la copia sobra.
    pub fn olvidar(&mut self) {
        self.vigente.fetch_add(1, Ordering::SeqCst);
        if let Some(c) = self.escrita.take() {
            descartar(&c);
        }
        self.fija = None;
        self.version = 0;
        self.desde = Instant::now();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn nombres_estables_y_distintos_por_carpeta() {
        let ruta = |c: &str, n: &str| Path::new(c).join(n);
        let a = nombre_para(Some(&ruta("obra", "Planta.dwg")), 0);
        let b = nombre_para(Some(&ruta("otra", "Planta.dwg")), 0);
        let a2 = nombre_para(Some(&ruta("OBRA", "planta.DWG")), 0);
        assert!(a.starts_with("Planta-") && a.ends_with(".dxf"), "{a}");
        assert_ne!(a, b);
        // Windows no distingue mayusculas: el mismo archivo, la misma copia.
        assert_eq!(a.rsplit('-').next(), a2.rsplit('-').next());
        assert_eq!(nombre_para(None, 0xAB), "SinNombre-00000000000000ab.dxf");
        assert!(nombre_para(Some(Path::new("a:b?.dxf")), 0).starts_with("a_b_-"));
    }

    #[test]
    fn solo_las_mas_nuevas_que_su_original() {
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        assert!(merece(t, None));
        assert!(merece(t, Some(t - Duration::from_secs(1))));
        assert!(!merece(t, Some(t)));
        assert!(!merece(t, Some(t + Duration::from_secs(60))));
    }

    #[test]
    fn cuanto_hace() {
        assert_eq!(hace(Duration::from_secs(20)), "hace un momento");
        assert_eq!(hace(Duration::from_secs(5 * 60)), "hace 5 min");
        assert_eq!(hace(Duration::from_secs(3 * 3600)), "hace 3 h");
        assert_eq!(hace(Duration::from_secs(3 * 86400)), "hace 3 dias");
    }

    #[test]
    fn toca_solo_con_cambios_y_pasado_el_tiempo() {
        let mut d = Dibujo::nuevo();
        let est = Estado::nuevo();
        let cada = Duration::from_secs(300);
        let luego = Instant::now() + Duration::from_secs(301);
        // Nuevo y sin tocar: nada que copiar.
        assert!(!est.toca(&d, luego, cada));
        d.editar("x", |t| {
            t.anadir(cad_nucleo::especialidad::linea([0.0, 0.0], [1.0, 0.0]))
        });
        assert!(est.toca(&d, luego, cada));
        assert!(!est.toca(&d, Instant::now(), cada));
        let r = std::env::temp_dir().join("cadpixpin-prueba-autoguardado.dxf");
        d.guardar_como(&r).unwrap();
        assert!(!est.toca(&d, luego, cada));
        let _ = std::fs::remove_file(r);
    }
}
