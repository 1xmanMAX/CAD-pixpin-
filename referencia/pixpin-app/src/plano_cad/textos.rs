//! **Los textos de un plano, con su sitio, y buscar en ellos** (14-oct-2026).
//!
//! Pedido del usuario: «¿donde dice JR. ANTISUYO?» y que el visor lleve
//! ahi. El plano ya leido (`.pxcad`) no guarda letras, solo sus trazos; los
//! textos se sacan del DWG recorriendolo como al dibujarlo
//! (`pixpin_cad::convertir::textos_del_fichero`), en el **proceso aparte**
//! de siempre (un DWG roto no tumba PixPin), y quedan en la cache junto al
//! plano: `<raiz>/cache/cad/<clave>-textos.json`. La segunda pregunta sobre
//! el mismo plano es instantanea.

use std::path::{Path, PathBuf};

pub use pixpin_cad::convertir::TextoDelPlano;

/// La orden con que PixPin se llama a si misma para sacar los textos.
pub const ORDEN: &str = "--cad-textos";

/// En el proceso aparte: lee el plano y escribe sus textos. Codigo de salida.
pub fn en_este_proceso(args: &[std::ffi::OsString]) -> i32 {
    let (Some(entrada), Some(salida)) = (args.first(), args.get(1)) else {
        return 2;
    };
    match pixpin_cad::convertir::textos_del_fichero(Path::new(entrada)) {
        Ok(t) => {
            let salida = PathBuf::from(salida);
            let tmp = salida.with_extension("tmp");
            let ok = std::fs::write(&tmp, a_json(&t)).and_then(|()| std::fs::rename(&tmp, &salida));
            if ok.is_ok() { 0 } else { 3 }
        }
        Err(e) => {
            let _ = std::fs::write(PathBuf::from(salida).with_extension("error"), e);
            1
        }
    }
}

/// `[[texto, x, y, alto], ...]`: corto, y se lee sin tipos con serde.
fn a_json(t: &[TextoDelPlano]) -> String {
    let v: Vec<serde_json::Value> = t
        .iter()
        .map(|t| serde_json::json!([t.texto, t.x, t.y, t.alto]))
        .collect();
    serde_json::Value::Array(v).to_string()
}

fn de_json(s: &str) -> Option<Vec<TextoDelPlano>> {
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    v.as_array()?
        .iter()
        .map(|e| {
            Some(TextoDelPlano {
                texto: e.get(0)?.as_str()?.to_string(),
                x: e.get(1)?.as_f64()?,
                y: e.get(2)?.as_f64()?,
                alto: e.get(3)?.as_f64()?,
            })
        })
        .collect()
}

/// **Los textos de `plano`**: de la cache o leyendolo aparte (bloquea: un
/// plano grande tarda unos segundos la primera vez).
pub fn del_plano(raiz: &Path, plano: &Path) -> Result<Vec<TextoDelPlano>, String> {
    let carpeta = raiz.join("cache").join("cad");
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let clave = super::clave(plano).ok_or("no se puede leer el plano")?;
    let destino = carpeta.join(format!("{clave}-textos.json"));
    if let Some(t) = std::fs::read_to_string(&destino)
        .ok()
        .and_then(|s| de_json(&s))
    {
        return Ok(t);
    }
    // En las pruebas el ejecutable no sabe la orden: aqui mismo.
    if cfg!(test) {
        let t = pixpin_cad::convertir::textos_del_fichero(plano)?;
        let _ = std::fs::write(&destino, a_json(&t));
        return Ok(t);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut orden = std::process::Command::new(exe);
    orden.arg(ORDEN).arg(plano).arg(&destino);
    {
        use std::os::windows::process::CommandExt;
        orden.creation_flags(0x0800_0000);
    }
    let estado = orden.status().map_err(|e| e.to_string())?;
    if !estado.success() {
        let motivo = std::fs::read_to_string(destino.with_extension("error")).unwrap_or_default();
        let _ = std::fs::remove_file(destino.with_extension("error"));
        return Err(format!("no se pudo leer el plano: {motivo}"));
    }
    std::fs::read_to_string(&destino)
        .ok()
        .and_then(|s| de_json(&s))
        .ok_or_else(|| "el plano leido no se entiende".into())
}

/// Una coincidencia: el texto entero que la contiene y su sitio.
#[derive(Debug, Clone, PartialEq)]
pub struct Hallado {
    pub texto: String,
    pub x: f64,
    pub y: f64,
    pub alto: f64,
}

/// Minusculas, sin tildes ni puntos y con los espacios de uno en uno: «JR.
/// ANTISUYO» encuentra «Jr Antisuyo».
pub fn plegar(s: &str) -> String {
    let t: String = pixpin_ia::acciones::normalizar(s)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// **Busca `consulta` en los textos**: primero las que la contienen entera;
/// si no hay, las que tienen todas sus palabras. Las mas grandes antes (el
/// nombre de una calle rotulado en grande es lo que se busca, no la misma
/// palabra en una nota al pie). Como mucho `max`.
pub fn buscar(textos: &[TextoDelPlano], consulta: &str, max: usize) -> Vec<Hallado> {
    let q = plegar(consulta);
    if q.is_empty() {
        return Vec::new();
    }
    let palabras: Vec<&str> = q.split(' ').collect();
    let mut enteras: Vec<&TextoDelPlano> = Vec::new();
    let mut sueltas: Vec<&TextoDelPlano> = Vec::new();
    for t in textos {
        let p = plegar(&t.texto);
        if p.contains(&q) {
            enteras.push(t);
        } else if palabras.iter().all(|w| {
            p.split(' ')
                .any(|x| x == *w || (w.len() >= 4 && x.starts_with(w)))
        }) {
            sueltas.push(t);
        }
    }
    let mut v = if enteras.is_empty() { sueltas } else { enteras };
    v.sort_by(|a, b| b.alto.total_cmp(&a.alto));
    v.into_iter()
        .take(max)
        .map(|t| Hallado {
            texto: t.texto.clone(),
            x: t.x,
            y: t.y,
            alto: t.alto,
        })
        .collect()
}

/// La vista para ensenar lo hallado: centrada en el texto y con unas 40
/// letras de ancho (se lee y se ve lo de alrededor).
pub fn vista_de(h: &Hallado) -> [f64; 3] {
    let largo = h.texto.chars().count().max(4) as f64;
    let ancho = (h.alto * 40.0).max(h.alto * largo * 1.6).max(1e-6);
    [h.x + h.alto * largo * 0.35, h.y + h.alto * 0.5, ancho]
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn t(texto: &str, x: f64, alto: f64) -> TextoDelPlano {
        TextoDelPlano {
            texto: texto.into(),
            x,
            y: 10.0,
            alto,
        }
    }

    #[test]
    fn se_encuentra_sin_mirar_mayusculas_tildes_ni_puntos_y_el_grande_primero() {
        let ts = vec![
            t("ver Jr Antisuyo en nota", 1.0, 1.0),
            t("JR. ANTISUYO", 50.0, 5.0),
            t("Av. Tahuantinsuyo", 9.0, 5.0),
        ];
        let h = buscar(&ts, "jr antisuyo", 5);
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].texto, "JR. ANTISUYO");
        // Por palabras sueltas, si no esta entera.
        let h = buscar(&[t("ANTISUYO (JR.)", 0.0, 2.0)], "Jr. Antisuyo", 5);
        assert_eq!(h.len(), 1);
        // Caso negativo.
        assert!(buscar(&ts, "Collasuyo", 5).is_empty());
        assert!(buscar(&ts, "  ", 5).is_empty());
    }

    #[test]
    fn el_json_de_la_cache_va_y_vuelve() {
        let ts = vec![t("Señal «R-1»", 1.5, 2.25)];
        assert_eq!(de_json(&a_json(&ts)).unwrap(), ts);
        assert!(de_json("[[1,2]]").is_none());
        let v = vista_de(&Hallado {
            texto: "JR. ANTISUYO".into(),
            x: 100.0,
            y: 50.0,
            alto: 2.0,
        });
        assert!(v[2] >= 80.0 && v[0] > 100.0);
    }
}
