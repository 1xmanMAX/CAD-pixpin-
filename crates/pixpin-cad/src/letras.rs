//! **Las letras del usuario y las que faltan** (13-oct-2026, como el boton
//! «Aa» del visor de planos de Android, v0.120): un plano que llega de un
//! compañero pide a menudo una SHX de su AutoCAD o una TrueType que este
//! equipo no tiene. Sus textos salen entonces con otra parecida (las de
//! Hershey de PixPin, Arial Narrow) y pueden no caber en sus recuadros.
//!
//! Aqui se apunta **que letras pide el plano y no estan** (para decirlo en
//! el visor) y se busca en **la carpeta de letras de PixPin**
//! (`<raiz>/fuentes-shx`, el mismo nombre que en el movil), donde el usuario
//! deja las SHX, TTF, OTF y TTC que quiera. Esa carpeta **manda** sobre las
//! de Autodesk, las de Windows y las de Hershey.
//!
//! La lectura del plano va en otro proceso (`--cad-convertir`), asi que la
//! carpeta se le pasa por la linea de mandatos y se pone aqui con
//! [`poner_carpeta`] antes de leer. Lo que falta vuelve en
//! `Modelo::sin_dibujar` con el prefijo [`FUENTE_QUE_FALTA`], igual que en
//! Android, y el plano leido se guarda con [`huella`] en su nombre: con una
//! letra nueva, se vuelve a leer.
//!
//! Traido casi tal cual de `herramientas/pixpin-cad-android/src/lib.rs`
//! (`del_usuario`, `familias_del_usuario`, `falta`, `revisar_ttf`...).

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Como se apunta en `sin_dibujar` una letra que falta (lo lee el visor).
pub const FUENTE_QUE_FALTA: &str = "fuente que falta: ";

/// Las letras que se pueden añadir, por su terminacion.
pub const TIPOS_DE_LETRA: [&str; 4] = ["shx", "ttf", "otf", "ttc"];

/// El nombre de la carpeta de letras del usuario dentro de la raiz de PixPin.
pub const CARPETA: &str = "fuentes-shx";

static CARPETA_DEL_USUARIO: Mutex<Option<PathBuf>> = Mutex::new(None);
static FALTAN: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// La carpeta de letras de PixPin dentro de `raiz`.
pub fn carpeta_en(raiz: &Path) -> PathBuf {
    raiz.join(CARPETA)
}

/// Dice donde estan las letras del usuario (antes de leer un plano).
pub fn poner_carpeta(carpeta: Option<PathBuf>) {
    if let Ok(mut c) = CARPETA_DEL_USUARIO.lock() {
        *c = carpeta;
    }
}

/// Las carpetas de letras del usuario (hoy, una o ninguna).
pub fn carpetas_del_usuario() -> Vec<PathBuf> {
    CARPETA_DEL_USUARIO
        .lock()
        .ok()
        .and_then(|c| c.clone())
        .filter(|c| c.is_dir())
        .into_iter()
        .collect()
}

/// Las letras que puso el usuario en `carpeta`, por nombre.
pub fn letras_en(carpeta: &Path) -> Vec<PathBuf> {
    let Ok(d) = std::fs::read_dir(carpeta) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = d
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && es_letra(p))
        .collect();
    v.sort_by_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    v
}

/// Si el fichero es una letra (por su terminacion).
pub fn es_letra(ruta: &Path) -> bool {
    let ext = ruta
        .extension()
        .map(|x| x.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    TIPOS_DE_LETRA.contains(&ext.as_str())
}

/// **Si el fichero es de verdad una letra**: su terminacion y sus primeros
/// bytes (las SHX empiezan por «AutoCAD-86»; las TrueType, OpenType y
/// colecciones por su etiqueta). Lo que viaja entre aparatos se mira asi:
/// un fichero cualquiera rebautizado `.shx` no pasa.
pub fn parece_letra(ruta: &Path) -> bool {
    if !es_letra(ruta) {
        return false;
    }
    let mut cabeza = [0u8; 10];
    let leidos = std::fs::File::open(ruta).and_then(|mut f| {
        use std::io::Read;
        f.read(&mut cabeza)
    });
    let Ok(n) = leidos else { return false };
    let c = &cabeza[..n];
    c.starts_with(b"AutoCAD-86")
        || [&[0u8, 1, 0, 0][..], b"true", b"typ1", b"OTTO", b"ttcf"]
            .iter()
            .any(|m| c.starts_with(m))
}

/// **Una huella corta de las letras del usuario** (nombres y pesos, como
/// `huellaDeLetras` de Android): cambia si se añade o se quita una. Vacia si
/// no hay ninguna, para que el nombre del plano leido no cambie sin letras.
pub fn huella(carpeta: &Path) -> String {
    let l = letras_en(carpeta);
    if l.is_empty() {
        return String::new();
    }
    let texto: Vec<String> = l
        .iter()
        .map(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let peso = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            format!("{n}:{peso}")
        })
        .collect();
    // FNV-1a de 64 bits, como la clave del plano; ocho cifras bastan.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.join("|").bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")[..8].to_string()
}

/// **Copia las letras elegidas** a `carpeta`, con su nombre (que es lo que
/// pide el plano: «romans.shx», «ARIALN.TTF»), por un temporal. Lo que no es
/// una letra se salta. Devuelve cuantas.
pub fn anadir(carpeta: &Path, rutas: &[PathBuf]) -> usize {
    if std::fs::create_dir_all(carpeta).is_err() {
        return 0;
    }
    let mut n = 0;
    for r in rutas {
        let Some(nombre) = r.file_name() else {
            continue;
        };
        if !r.is_file() || !es_letra(r) {
            continue;
        }
        let destino = carpeta.join(nombre);
        let tmp = carpeta.join(format!("{}.tmp", nombre.to_string_lossy()));
        if std::fs::copy(r, &tmp).is_ok() && std::fs::rename(&tmp, &destino).is_ok() {
            n += 1;
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
    }
    n
}

/// Quita todas las letras del usuario («Quitar las mias»). Devuelve cuantas.
pub fn quitar_todas(carpeta: &Path) -> usize {
    letras_en(carpeta)
        .iter()
        .filter(|p| std::fs::remove_file(p).is_ok())
        .count()
}

/// El fichero `nombre` (sin mirar mayusculas) en las carpetas del usuario.
/// Una ruta entera no se busca: esa ya dice donde esta.
pub fn del_usuario(nombre: &str) -> Option<PathBuf> {
    let base = nombre.rsplit(['/', '\\']).next().unwrap_or(nombre).trim();
    if base.is_empty() || Path::new(nombre).is_absolute() {
        return None;
    }
    for c in carpetas_del_usuario() {
        let Ok(d) = std::fs::read_dir(&c) else {
            continue;
        };
        for e in d.flatten() {
            if e.file_name().to_string_lossy().eq_ignore_ascii_case(base) {
                return Some(e.path());
            }
        }
    }
    None
}

/// **Las familias de las TrueType del usuario**, por su nombre de verdad (la
/// tabla `name`: «arial narrow», y con «bold» si es negrita), como las
/// nombra el registro de Windows. Se rehace si cambia lo que hay en la
/// carpeta (el proceso puede vivir mientras el usuario añade letras).
pub fn familias_del_usuario() -> HashMap<String, PathBuf> {
    static CACHE: Mutex<Option<(String, HashMap<String, PathBuf>)>> = Mutex::new(None);
    let carpetas = carpetas_del_usuario();
    let firma: String = carpetas
        .iter()
        .map(|c| format!("{}#{}", c.display(), huella(c)))
        .collect();
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((f, m)) = g.as_ref()
        && *f == firma
    {
        return m.clone();
    }
    let mut mapa = HashMap::new();
    for c in &carpetas {
        for ruta in letras_en(c) {
            let ext = ruta
                .extension()
                .map(|x| x.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if ext == "shx" {
                continue;
            }
            let Ok(datos) = std::fs::read(&ruta) else {
                continue;
            };
            let Ok(cara) = ttf_parser::Face::parse(&datos, 0) else {
                continue;
            };
            let nombre = |id: u16| {
                cara.names()
                    .into_iter()
                    .filter(|n| n.name_id == id && n.is_unicode())
                    .find_map(|n| n.to_string())
            };
            let Some(familia) = nombre(ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
                .or_else(|| nombre(ttf_parser::name_id::FAMILY))
            else {
                continue;
            };
            let sub = nombre(ttf_parser::name_id::TYPOGRAPHIC_SUBFAMILY)
                .or_else(|| nombre(ttf_parser::name_id::SUBFAMILY))
                .unwrap_or_default()
                .to_lowercase();
            let familia = familia.trim().to_lowercase();
            let clave = if sub.is_empty() || sub == "regular" || sub == "normal" {
                familia.clone()
            } else {
                format!("{familia} {sub}")
            };
            mapa.insert(clave, ruta.clone());
            if sub.contains("bold") {
                mapa.insert(format!("{familia} negrita"), ruta.clone());
            }
        }
    }
    *g = Some((firma, mapa.clone()));
    mapa
}

/// Apunta una letra que el plano pide y no esta.
pub fn falta(nombre: &str) {
    let n = nombre.trim();
    if !n.is_empty()
        && let Ok(mut f) = FALTAN.lock()
    {
        f.insert(n.to_string());
    }
}

/// Una SHX que no esta: solo si de verdad es SHX (la reserva la conoce o
/// termina en .shx); un estilo «Standard» sin fichero no cuenta.
pub fn falta_shx(clave: &str) {
    if crate::shx::reserva_para(clave).is_some() {
        let base = clave.rsplit(['/', '\\']).next().unwrap_or(clave);
        falta(&if base.to_ascii_lowercase().ends_with(".shx") {
            base.to_string()
        } else {
            format!("{base}.shx")
        });
    }
}

/// Una TrueType pedida por su fichero (el `font_file` de un estilo) que no
/// esta ni en las del usuario ni en la carpeta de Windows ni registrada.
pub fn revisar_ttf(clave: &str, sistema: &Path) {
    let c = clave.trim().to_ascii_lowercase();
    if c.starts_with("ttf:") || !(c.ends_with(".ttf") || c.ends_with(".otf") || c.ends_with(".ttc"))
    {
        return;
    }
    let base = c.rsplit(['\\', '/']).next().unwrap_or(&c).to_string();
    if !ruta_de_fuente(sistema, &base).is_file() {
        falta(&base);
    }
}

/// Donde leer la letra `nombre` (un fichero como «arialn.ttf», o una ruta
/// entera): la del usuario si la hay, si no la de `sistema`.
pub fn ruta_de_fuente(sistema: &Path, nombre: &str) -> PathBuf {
    if let Some(r) = del_usuario(nombre) {
        return r;
    }
    sistema.join(nombre)
}

/// Lo apuntado desde la ultima vez, y se vacia.
pub fn sacar_lo_que_falta() -> Vec<String> {
    FALTAN
        .lock()
        .map(|mut f| std::mem::take(&mut *f).into_iter().collect())
        .unwrap_or_default()
}

/// **Lee un plano con las letras del usuario** (las de `carpeta`) y apunta
/// en su `sin_dibujar` las que pide y no estan (`convertir_a` de Android).
pub fn convertir_con_letras(
    entrada: &Path,
    carpeta: Option<PathBuf>,
) -> Result<crate::modelo::Modelo, String> {
    poner_carpeta(carpeta);
    sacar_lo_que_falta();
    let (mut modelo, _) = crate::convertir::convertir_fichero(entrada)?;
    for f in sacar_lo_que_falta() {
        modelo
            .sin_dibujar
            .push((format!("{FUENTE_QUE_FALTA}{f}"), 1));
    }
    Ok(modelo)
}

/// Las letras que faltan, de lo que el plano apunto en `sin_dibujar`.
pub fn las_que_faltan(sin_dibujar: &[(String, u32)]) -> Vec<String> {
    sin_dibujar
        .iter()
        .filter_map(|(t, _)| t.strip_prefix(FUENTE_QUE_FALTA))
        .map(str::to_string)
        .collect()
}

/// Lo global (la carpeta y lo que falta) lo comparten las pruebas de aqui y
/// las de `texto`: una a la vez.
#[cfg(test)]
pub(crate) static UNA_A_LA_VEZ: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod pruebas_letras_del_usuario {
    use super::*;

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-letras-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// La de Android (`pruebas_letras_del_usuario`), con la carpeta de
    /// Windows en lugar de la del telefono.
    #[test]
    fn la_letra_del_usuario_se_encuentra_y_no_falta() {
        let _g = UNA_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let d = carpeta_de_prueba("encontrar");
        std::fs::write(d.join("MiaLetra.TTF"), b"x").unwrap();
        poner_carpeta(Some(d.clone()));
        assert_eq!(del_usuario("mialetra.ttf"), Some(d.join("MiaLetra.TTF")));
        sacar_lo_que_falta();
        let sistema = Path::new("C:\\no\\existe");
        revisar_ttf("mialetra.ttf", sistema);
        revisar_ttf("rara.ttf", sistema);
        assert_eq!(sacar_lo_que_falta(), vec!["rara.ttf".to_string()]);
        // Ya se vacio.
        assert!(sacar_lo_que_falta().is_empty());
        poner_carpeta(None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sin_carpeta_o_con_ruta_entera_no_se_busca_en_las_del_usuario() {
        let _g = UNA_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let d = carpeta_de_prueba("negativo");
        std::fs::write(d.join("romans.shx"), b"x").unwrap();
        poner_carpeta(None);
        assert_eq!(del_usuario("romans.shx"), None);
        poner_carpeta(Some(d.clone()));
        assert_eq!(del_usuario("ROMANS.SHX"), Some(d.join("romans.shx")));
        // Una ruta entera dice ya donde esta; un nombre vacio no es nada.
        assert_eq!(del_usuario("C:\\Fuentes\\romans.shx"), None);
        assert_eq!(del_usuario("  "), None);
        // Un estilo sin SHX («Standard») no cuenta como letra que falta; un
        // .ttf pedido como «ttf:<ruta>» tampoco (ese ya se resolvio).
        sacar_lo_que_falta();
        falta_shx("standard");
        revisar_ttf("ttf:C:\\x\\rara.ttf", Path::new("C:\\no"));
        revisar_ttf("arial", Path::new("C:\\no"));
        assert!(sacar_lo_que_falta().is_empty());
        falta_shx("simplex");
        assert_eq!(sacar_lo_que_falta(), vec!["simplex.shx".to_string()]);
        poner_carpeta(None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn anadir_copia_solo_letras_y_la_huella_cambia() {
        let d = carpeta_de_prueba("anadir");
        let fuera = carpeta_de_prueba("anadir-origen");
        assert_eq!(huella(&d), "", "sin letras, sin huella");
        std::fs::write(fuera.join("isocp.shx"), b"AutoCAD-86 shapes 1.0").unwrap();
        std::fs::write(fuera.join("ArialN.TTF"), b"ttf").unwrap();
        std::fs::write(fuera.join("leeme.txt"), b"no").unwrap();
        let n = anadir(
            &d,
            &[
                fuera.join("isocp.shx"),
                fuera.join("ArialN.TTF"),
                fuera.join("leeme.txt"),
                fuera.join("no-esta.shx"),
            ],
        );
        assert_eq!(n, 2);
        let nombres: Vec<String> = letras_en(&d)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            nombres,
            vec!["ArialN.TTF".to_string(), "isocp.shx".to_string()]
        );
        let h1 = huella(&d);
        assert_eq!(h1.len(), 8);
        std::fs::write(fuera.join("romans.shx"), b"otra").unwrap();
        anadir(&d, &[fuera.join("romans.shx")]);
        assert_ne!(huella(&d), h1);
        assert_eq!(quitar_todas(&d), 3);
        assert_eq!(huella(&d), "");
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&fuera);
    }

    #[test]
    fn solo_parece_letra_lo_que_lo_es_por_dentro() {
        let d = carpeta_de_prueba("parece");
        std::fs::write(
            d.join("romans.shx"),
            include_bytes!("../fuentes-shx/romans.shx"),
        )
        .unwrap();
        std::fs::write(d.join("libre.ttf"), [0u8, 1, 0, 0, 0, 9]).unwrap();
        std::fs::write(d.join("libre.otf"), b"OTTO\0\x01").unwrap();
        assert!(
            parece_letra(&d.join("romans.shx"))
                && parece_letra(&d.join("libre.ttf"))
                && parece_letra(&d.join("libre.otf"))
        );
        // Casos negativos: un programa rebautizado, un fichero vacio, otra
        // terminacion aunque por dentro sea una SHX, y uno que no esta.
        std::fs::write(d.join("trampa.shx"), b"MZ\x90\0programa").unwrap();
        std::fs::write(d.join("vacia.ttf"), b"").unwrap();
        std::fs::write(
            d.join("romans.txt"),
            include_bytes!("../fuentes-shx/romans.shx"),
        )
        .unwrap();
        for n in ["trampa.shx", "vacia.ttf", "romans.txt", "no-esta.shx"] {
            assert!(!parece_letra(&d.join(n)), "{n}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn las_que_faltan_salen_de_sin_dibujar() {
        let v = vec![
            (format!("{FUENTE_QUE_FALTA}romans.shx"), 1),
            ("sin dibujo: AeccDbSurfaceTin".to_string(), 3),
            (format!("{FUENTE_QUE_FALTA}arialn.ttf"), 1),
        ];
        assert_eq!(
            las_que_faltan(&v),
            vec!["romans.shx".to_string(), "arialn.ttf".to_string()]
        );
        assert!(las_que_faltan(&[]).is_empty());
    }
}
