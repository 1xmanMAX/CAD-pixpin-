//! Uniones de directorio (junctions): una carpeta que en realidad esta en
//! otro sitio del mismo equipo.
//!
//! Es lo que deja guardar un proyecto donde el usuario quiera sin tocar las
//! decenas de sitios que escriben en `proyectos/<id>`: esa carpeta pasa a
//! ser una union que apunta a la elegida, y todo lo que escribe en ella
//! escribe alli.
//!
//! Union y no enlace simbolico a proposito: un enlace simbolico pide el
//! privilegio de crearlos (o el modo desarrollador), y una union la puede
//! crear cualquier usuario sobre sus propias carpetas. La contrapartida es
//! que una union solo apunta a discos locales; por eso `es_de_red` va aqui.
//!
//! **Fuera de Windows** (Mac, Linux) no hay uniones, pero cualquier usuario
//! puede crear enlaces simbolicos, que hacen lo mismo: misma API, mismas
//! garantias (quitar solo quita el enlace; uno roto no se repara solo).

use std::io;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

#[cfg(windows)]
use windows::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::GetDriveTypeW;
#[cfg(windows)]
use windows::Win32::System::IO::DeviceIoControl;
#[cfg(windows)]
use windows::core::HSTRING;

/// `IO_REPARSE_TAG_MOUNT_POINT`: la etiqueta de una union.
#[cfg(windows)]
const ETIQUETA_UNION: u32 = 0xA000_0003;
/// `FSCTL_SET_REPARSE_POINT`.
#[cfg(windows)]
const PONER_REPARSE: u32 = 0x0009_00A4;
/// `FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS`: abrir la
/// carpeta misma y no lo que hubiera detras.
#[cfg(windows)]
const ABRIR_LA_CARPETA: u32 = 0x0020_0000 | 0x0200_0000;
/// `FILE_ATTRIBUTE_REPARSE_POINT`.
#[cfg(windows)]
const ATRIBUTO_REPARSE: u32 = 0x400;
/// `DRIVE_REMOTE` de `GetDriveTypeW`.
#[cfg(windows)]
const UNIDAD_DE_RED: u32 = 4;

/// Crea en `enlace` (que no puede existir) una union que apunta a `destino`.
///
/// `destino` tiene que ser una ruta absoluta de un disco local; no hace falta
/// que exista ya, y eso se aprovecha para rehacer la union de un proyecto
/// cuyo disco esta desenchufado: mejor una union rota, que falla al
/// escribir, que una carpeta vacia nueva donde se perderian los mensajes.
pub fn crear(enlace: &Path, destino: &Path) -> io::Result<()> {
    let destino = sin_prefijo_largo(destino);
    if !destino.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "el destino de una union tiene que ser absoluto",
        ));
    }
    if es_de_red(&destino) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "una union no puede apuntar a una unidad de red",
        ));
    }
    crear_en_el_sistema(enlace, &destino)
}

#[cfg(windows)]
fn crear_en_el_sistema(enlace: &Path, destino: &Path) -> io::Result<()> {
    std::fs::create_dir(enlace)?;
    let hecho = poner_reparse(enlace, destino);
    if hecho.is_err() {
        // La carpeta vacia que se creo para colgarle la union no puede
        // quedarse: pasaria por la carpeta del proyecto y se escribiria en
        // ella como si nada.
        let _ = std::fs::remove_dir(enlace);
    }
    hecho
}

/// Un enlace simbolico: el sistema no deja crearlo si `enlace` ya existe,
/// igual que la union.
#[cfg(unix)]
fn crear_en_el_sistema(enlace: &Path, destino: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(destino, enlace)
}

#[cfg(not(any(windows, unix)))]
fn crear_en_el_sistema(_enlace: &Path, _destino: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "este sistema no tiene uniones ni enlaces",
    ))
}

#[cfg(windows)]
fn poner_reparse(enlace: &Path, destino: &Path) -> io::Result<()> {
    // El nombre de sustitucion es el del gestor de objetos (`\??\C:\...`); el
    // de mostrar, la ruta de siempre. Los dos acaban en su NUL.
    let mut sustituto: Vec<u16> = r"\??\".encode_utf16().collect();
    sustituto.extend(destino.as_os_str().encode_wide());
    let mostrar: Vec<u16> = destino.as_os_str().encode_wide().collect();
    let bytes_sust = (sustituto.len() * 2) as u16;
    let bytes_most = (mostrar.len() * 2) as u16;
    // Cabecera de MountPointReparseBuffer: 4 u16 de desplazamientos y largos.
    let largo_datos = 8 + bytes_sust + 2 + bytes_most + 2;
    let mut b: Vec<u8> = Vec::with_capacity(8 + largo_datos as usize);
    b.extend_from_slice(&ETIQUETA_UNION.to_le_bytes());
    b.extend_from_slice(&largo_datos.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes()); // desplazamiento del sustituto
    b.extend_from_slice(&bytes_sust.to_le_bytes());
    b.extend_from_slice(&(bytes_sust + 2).to_le_bytes()); // del de mostrar
    b.extend_from_slice(&bytes_most.to_le_bytes());
    for u in sustituto
        .iter()
        .chain([&0u16])
        .chain(mostrar.iter())
        .chain([&0u16])
    {
        b.extend_from_slice(&u.to_le_bytes());
    }
    let fichero = std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(ABRIR_LA_CARPETA)
        .open(enlace)?;
    // SAFETY: el handle es de `fichero`, que vive hasta el final; el buffer
    // es un REPARSE_DATA_BUFFER completo con su largo, y la llamada no
    // devuelve nada ni retiene punteros.
    unsafe {
        DeviceIoControl(
            HANDLE(fichero.as_raw_handle()),
            PONER_REPARSE,
            Some(b.as_ptr().cast()),
            b.len() as u32,
            None,
            0,
            None,
            None,
        )
    }
    .map_err(|e| io::Error::from_raw_os_error(e.code().0 & 0xFFFF))
}

/// Si `ruta` es una union (viva o rota). Mira la entrada misma, sin
/// seguirla.
#[cfg(windows)]
pub fn es_union(ruta: &Path) -> bool {
    std::fs::symlink_metadata(ruta)
        .is_ok_and(|m| m.file_attributes() & ATRIBUTO_REPARSE != 0 && m.is_symlink())
        && destino(ruta).is_some()
}

/// Si `ruta` es un enlace simbolico (vivo o roto). Mira la entrada misma,
/// sin seguirla.
#[cfg(not(windows))]
pub fn es_union(ruta: &Path) -> bool {
    std::fs::symlink_metadata(ruta).is_ok_and(|m| m.is_symlink()) && destino(ruta).is_some()
}

/// Adonde apunta la union `ruta`, aunque el destino no exista ahora.
pub fn destino(ruta: &Path) -> Option<PathBuf> {
    std::fs::read_link(ruta).ok().map(|d| sin_prefijo_largo(&d))
}

/// Quita la union y SOLO la union: lo que hay en su destino no se toca.
///
/// Falla si `ruta` no es una union, para que un error de quien llama no
/// acabe borrando una carpeta de verdad.
pub fn quitar(ruta: &Path) -> io::Result<()> {
    if !es_union(ruta) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no es una union de directorio",
        ));
    }
    quitar_en_el_sistema(ruta)
}

/// `RemoveDirectoryW` sobre un punto de reparse quita el punto: no entra.
#[cfg(windows)]
fn quitar_en_el_sistema(ruta: &Path) -> io::Result<()> {
    std::fs::remove_dir(ruta)
}

/// Un enlace simbolico es un fichero: `unlink` lo quita sin entrar.
#[cfg(not(windows))]
fn quitar_en_el_sistema(ruta: &Path) -> io::Result<()> {
    std::fs::remove_file(ruta)
}

/// Si `ruta` esta en una unidad de red: `\\servidor\...` o una letra
/// conectada a una carpeta compartida.
///
/// Una union no puede apuntar ahi, y aunque pudiera, un proyecto en la red
/// deja de abrirse en cuanto se cae la WiFi.
pub fn es_de_red(ruta: &Path) -> bool {
    let texto = ruta.to_string_lossy();
    if let Some(resto) = texto.strip_prefix(r"\\?\") {
        if resto.len() >= 4 && resto[..4].eq_ignore_ascii_case(r"UNC\") {
            return true;
        }
        return unidad_de_red(resto);
    }
    if texto.starts_with(r"\\") || texto.starts_with("//") {
        return true;
    }
    unidad_de_red(&texto)
}

#[cfg(windows)]
fn unidad_de_red(texto: &str) -> bool {
    let mut letras = texto.chars();
    let (Some(letra), Some(':')) = (letras.next(), letras.next()) else {
        return false;
    };
    if !letra.is_ascii_alphabetic() {
        return false;
    }
    let raiz = HSTRING::from(format!("{letra}:\\"));
    // SAFETY: la cadena vive durante la llamada, que no la retiene.
    unsafe { GetDriveTypeW(&raiz) == UNIDAD_DE_RED }
}

/// Fuera de Windows no hay letras de unidad: solo cuentan las rutas
/// `\\servidor` y `//servidor`, que ya se miraron antes.
#[cfg(not(windows))]
fn unidad_de_red(_texto: &str) -> bool {
    false
}

/// `\\?\C:\x` → `C:\x`. Lo que devuelve `read_link` y `canonicalize` lleva
/// ese prefijo, y compararlo con una ruta normal daria siempre distinto.
pub fn sin_prefijo_largo(ruta: &Path) -> PathBuf {
    let texto = ruta.to_string_lossy();
    match texto.strip_prefix(r"\\?\") {
        Some(resto)
            if !resto
                .get(..4)
                .is_some_and(|p| p.eq_ignore_ascii_case(r"UNC\")) =>
        {
            PathBuf::from(resto)
        }
        _ => ruta.to_path_buf(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(etiqueta: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-union-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn una_union_creada_se_sigue_hasta_su_destino() {
        let d = temporal("seguir");
        let destino = d.join("de verdad");
        std::fs::create_dir(&destino).unwrap();
        std::fs::write(destino.join("a.txt"), b"hola").unwrap();
        let enlace = d.join("enlace");
        crear(&enlace, &destino).unwrap();
        assert!(es_union(&enlace));
        assert_eq!(super::destino(&enlace).as_deref(), Some(destino.as_path()));
        // Leer y escribir por la union es leer y escribir en el destino.
        assert_eq!(std::fs::read(enlace.join("a.txt")).unwrap(), b"hola");
        std::fs::write(enlace.join("b.txt"), b"x").unwrap();
        assert!(destino.join("b.txt").is_file());
        // Caso negativo: una carpeta normal no es una union.
        assert!(!es_union(&destino));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn quitar_una_union_no_toca_lo_que_hay_en_su_destino() {
        let d = temporal("quitar");
        let destino = d.join("del usuario");
        std::fs::create_dir(&destino).unwrap();
        std::fs::write(destino.join("plano.png"), b"png").unwrap();
        let enlace = d.join("enlace");
        crear(&enlace, &destino).unwrap();
        quitar(&enlace).unwrap();
        assert!(!enlace.exists() && std::fs::symlink_metadata(&enlace).is_err());
        assert!(destino.join("plano.png").is_file());
        // Caso negativo: quitar no borra una carpeta de verdad.
        assert!(quitar(&destino).is_err());
        assert!(destino.join("plano.png").is_file());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn remove_dir_all_sobre_una_union_no_recorre_su_destino() {
        // Es lo que hacen las pruebas al limpiar y lo que haria quien purgara
        // la papelera: si siguiera la union, borraria la carpeta del usuario.
        let d = temporal("remove-all");
        let destino = d.join("del usuario");
        std::fs::create_dir(&destino).unwrap();
        std::fs::write(destino.join("plano.png"), b"png").unwrap();
        let raiz = d.join("raiz");
        std::fs::create_dir_all(raiz.join("proyectos")).unwrap();
        let enlace = raiz.join("proyectos").join("p1");
        crear(&enlace, &destino).unwrap();
        // La union sola...
        std::fs::remove_dir_all(&enlace).unwrap();
        assert!(destino.join("plano.png").is_file());
        // ...y la union dentro de un arbol que se borra entero.
        crear(&enlace, &destino).unwrap();
        std::fs::remove_dir_all(&raiz).unwrap();
        assert!(!raiz.exists());
        assert!(destino.join("plano.png").is_file());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn una_union_rota_no_se_repara_sola_al_crear_carpetas_por_ella() {
        // El disco desenchufado: la union apunta a algo que no esta, y quien
        // escribe el primer mensaje hace `create_dir_all`. Tiene que fallar,
        // no crear una carpeta local vacia en su sitio.
        let d = temporal("rota");
        let destino = d.join("usb").join("proyecto");
        let enlace = d.join("enlace");
        crear(&enlace, &destino).unwrap();
        assert!(es_union(&enlace));
        assert!(!enlace.is_dir());
        assert!(std::fs::create_dir_all(enlace.join("archivos")).is_err());
        assert!(std::fs::create_dir_all(&enlace).is_err());
        assert!(es_union(&enlace), "sigue siendo la union, no una carpeta");
        assert!(!destino.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn las_rutas_de_red_se_reconocen_y_las_locales_no() {
        assert!(es_de_red(Path::new(r"\\servidor\compartida\obra")));
        assert!(es_de_red(Path::new(r"\\?\UNC\servidor\compartida")));
        assert!(!es_de_red(Path::new(r"C:\Users")));
        assert!(!es_de_red(Path::new(r"\\?\C:\Users")));
        assert!(!es_de_red(Path::new("relativa")));
        // Caso negativo: crear una union hacia la red se rechaza sin tocar
        // el disco.
        let d = temporal("red");
        let enlace = d.join("enlace");
        assert!(crear(&enlace, Path::new(r"\\servidor\compartida\obra")).is_err());
        assert!(std::fs::symlink_metadata(&enlace).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
