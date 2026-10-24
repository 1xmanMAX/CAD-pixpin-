//! **Alargar una plantilla** (22-oct-2026). El usuario elige en el lienzo
//! una plantilla de serie y pide «damela del 1 al 40»: sale la misma hoja,
//! mas larga, con los numeros del 1 al 40. Si ademas dice «de 10 en 10»,
//! salen cuatro hojas iguales (1–10, 11–20...) con su pie «Hoja 2 de 4».
//!
//! Sus palabras: «no necesariamente cree una nueva lista o elementos con la
//! continuacion, puedo desde el principio pedirle seleccionando el templete
//! y decirle damelo con intervalos de 1 al 40 y que lo genere pero mas
//! grande en la misma hoja; pero si le digo que lo divida de 10 en 10 que me
//! de 4 hojas».
//!
//! Lo que cuenta cada plantilla (filas, horas, dias, viñetas...) lo dice
//! `plantillas::AMPLIABLES`; aqui se comprueba lo pedido, se parte en hojas
//! y se reconoce que plantilla es la elegida (por su titulo).

use std::sync::OnceLock;

use super::plantillas::{self, AMPLIABLES, MESES, Tramo, VINETA_DE_4};
use super::{PREFIJO_ID, SEPARADOR};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{Elemento, Figura};

/// Cuantas hojas como mucho de una vez.
pub const MAX_HOJAS: usize = 60;

/// **Lo que se pide.** Lo que falta se queda como en la de serie.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Intervalo {
    pub desde: Option<u32>,
    pub hasta: Option<u32>,
    /// «De 10 en 10»: cuantos por hoja.
    pub de_a: Option<u32>,
    /// Las semanas del cronograma (su segunda serie).
    pub semanas: Option<(u32, u32)>,
    /// `(ano, mes 1-12)`: el planificador mensual y los habitos, con el
    /// calendario de verdad.
    pub mes: Option<(i32, u32)>,
    /// Cuantos meses seguidos desde `mes`, una hoja cada uno.
    pub meses: Option<u32>,
}

/// Lo que se sabe de cada plantilla de pagina: su clave, su titulo (lo que
/// se lee en la hoja) y su nombre en el menu.
struct Ficha {
    clave: String,
    titulo: String,
    nombre: String,
}

fn fichas() -> &'static [Ficha] {
    static F: OnceLock<Vec<Ficha>> = OnceLock::new();
    F.get_or_init(|| {
        super::catalogo()
            .into_iter()
            .filter_map(|f| {
                let titulo = titulo_de(&f.elementos)?;
                let clave = f.id.strip_prefix(PREFIJO_ID)?.to_string();
                let nombre =
                    super::categoria_de(&f.nombre).map_or(f.nombre.clone(), |(_, n)| n.to_string());
                Some(Ficha {
                    clave,
                    titulo: titulo.to_string(),
                    nombre,
                })
            })
            .collect()
    })
}

/// El titulo de una hoja: su texto grande.
fn titulo_de(elementos: &[Elemento]) -> Option<&str> {
    elementos
        .iter()
        .filter(|e| !e.borrado)
        .filter_map(|e| match &e.figura {
            Figura::Texto { texto, tam, .. } if *tam >= 30.0 => Some((texto.as_str(), *tam)),
            _ => None,
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(t, _)| t)
}

/// **Que plantilla de serie es** lo elegido (su clave), por el titulo de la
/// hoja. Los dos storyboard se titulan igual: el de 4 tiene las viñetas de
/// [`VINETA_DE_4`] de ancho.
pub fn cual_es(elementos: &[Elemento]) -> Option<String> {
    let titulo = titulo_de(elementos)?;
    let f = fichas().iter().find(|f| f.titulo == titulo)?;
    if f.clave.starts_with("storyboard-") {
        let de_4 = elementos
            .iter()
            .any(|e| matches!(e.figura, Figura::Rectangulo) && (e.ancho - VINETA_DE_4).abs() < 0.5);
        return Some(if de_4 { "storyboard-4" } else { "storyboard-6" }.to_string());
    }
    Some(f.clave.clone())
}

/// El nombre de una plantilla en el menu («Lista de tareas»).
pub fn nombre_de(clave: &str) -> String {
    fichas()
        .iter()
        .find(|f| f.clave == clave)
        .map_or_else(|| clave.to_string(), |f| f.nombre.clone())
}

/// La clave de una plantilla de serie por su id (`serie-<clave>`).
pub fn clave_de(f: &FiguraGuardada) -> Option<&str> {
    f.id.strip_prefix(PREFIJO_ID)
}

/// Si esa plantilla se puede alargar.
pub fn se_alarga(clave: &str) -> bool {
    AMPLIABLES.iter().any(|a| a.clave == clave)
}

/// Las que se alargan y lo que cuenta cada una, para el modelo y los avisos:
/// «Cronograma (filas, y semanas), Planificador diario (horas)...».
pub fn las_que_se_alargan() -> String {
    AMPLIABLES
        .iter()
        .map(|a| {
            let extra = match a.clave {
                "cronograma" => ", y semanas",
                "habitos" => ", o un mes",
                _ => "",
            };
            let cuenta = if a.clave == "planificador-mensual" {
                "un mes y cuántos"
            } else {
                a.cuenta
            };
            format!("{} ({cuenta}{extra})", nombre_de(a.clave))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// **Las hojas pedidas**, cada una normalizada al origen y con el tramo en
/// el nombre («Planificación · Lista de tareas (1–40)»). Error, en palabras
/// para el usuario, si la plantilla no se alarga o lo pedido no cabe.
pub fn alargar(clave: &str, iv: &Intervalo) -> Result<Vec<FiguraGuardada>, String> {
    let Some(a) = AMPLIABLES.iter().find(|a| a.clave == clave) else {
        return Err(format!(
            "«{}» no lleva números que alargar. Se alargan: {}",
            nombre_de(clave),
            las_que_se_alargan()
        ));
    };
    if let Some(mes) = iv.mes {
        return por_meses(clave, mes, iv.meses.unwrap_or(1));
    }
    if clave == "planificador-mensual" {
        return Err(
            "el planificador mensual va por meses: di el mes y el año (y cuántos meses seguidos)"
                .into(),
        );
    }
    if iv.semanas.is_some() && clave != "cronograma" {
        return Err("solo el cronograma lleva semanas".into());
    }
    let desde = iv.desde.unwrap_or(a.defecto.0);
    let hasta = iv.hasta.unwrap_or(a.defecto.1.max(desde));
    if hasta < desde {
        return Err(format!("el intervalo va al revés: del {desde} al {hasta}"));
    }
    if desde < a.minimo || hasta > a.maximo {
        return Err(format!(
            "«{}» va del {} al {} como mucho ({})",
            nombre_de(clave),
            a.minimo,
            a.maximo,
            a.cuenta
        ));
    }
    let semanas = match iv.semanas {
        Some((s0, s1)) if s0 == 0 || s1 < s0 || s1 - s0 >= 260 => {
            return Err(format!(
                "las semanas van al revés o son demasiadas: de la {s0} a la {s1}"
            ));
        }
        Some(s) => s,
        None => (1, 12),
    };
    // Lo que se parte: las filas, salvo que solo se hayan pedido semanas.
    let partir_semanas = iv.semanas.is_some() && iv.desde.is_none() && iv.hasta.is_none();
    let (lo, hi) = if partir_semanas {
        semanas
    } else {
        (desde, hasta)
    };
    let trozos = trozos(lo, hi, iv.de_a)?;
    let n = trozos.len() as u32;
    Ok(trozos
        .into_iter()
        .enumerate()
        .map(|(i, (t0, t1))| {
            let mut tr = Tramo::de(desde, hasta);
            tr.semanas = semanas;
            if partir_semanas {
                tr.semanas = (t0, t1);
            } else {
                (tr.desde, tr.hasta) = (t0, t1);
            }
            tr.hoja = (n > 1).then_some((i as u32 + 1, n));
            let rotulo = if partir_semanas {
                format!("semanas {t0}–{t1}")
            } else {
                tr.rotulo()
            };
            con_nombre(
                plantillas::hacer(clave, &tr).expect("es de la tabla"),
                &rotulo,
            )
        })
        .collect())
}

/// `lo..=hi` en trozos de `de_a` (el ultimo, lo que quede).
fn trozos(lo: u32, hi: u32, de_a: Option<u32>) -> Result<Vec<(u32, u32)>, String> {
    let Some(k) = de_a.filter(|&k| k > 0 && k <= hi - lo) else {
        return Ok(vec![(lo, hi)]);
    };
    let v: Vec<(u32, u32)> = (lo..=hi)
        .step_by(k as usize)
        .map(|a| (a, (a + k - 1).min(hi)))
        .collect();
    if v.len() > MAX_HOJAS {
        return Err(format!(
            "serían {} hojas; como mucho {MAX_HOJAS} de una vez",
            v.len()
        ));
    }
    Ok(v)
}

/// Una hoja por mes, desde `mes`: el mensual con sus dias en su sitio, o
/// los habitos con tantas columnas como dias tenga el mes.
fn por_meses(
    clave: &str,
    (ano, mes): (i32, u32),
    cuantos: u32,
) -> Result<Vec<FiguraGuardada>, String> {
    if !matches!(clave, "planificador-mensual" | "habitos") {
        return Err(format!(
            "«{}» no va por meses; por meses van el planificador mensual y los hábitos",
            nombre_de(clave)
        ));
    }
    if !(1..=12).contains(&mes) || !(1900..=2200).contains(&ano) {
        return Err(format!("ese mes no existe: {mes}/{ano}"));
    }
    let cuantos = cuantos.max(1);
    if cuantos as usize > MAX_HOJAS {
        return Err(format!(
            "serían {cuantos} hojas; como mucho {MAX_HOJAS} de una vez"
        ));
    }
    Ok((0..cuantos)
        .map(|i| {
            let k = mes - 1 + i;
            let (a, m) = (ano + (k / 12) as i32, k % 12 + 1);
            let mut tr = Tramo::de(
                1,
                if clave == "habitos" {
                    plantillas::dias_del_mes(a, m)
                } else {
                    1
                },
            );
            tr.mes = Some((a, m));
            tr.hoja = (cuantos > 1).then_some((i + 1, cuantos));
            let rotulo = format!("{} {a}", MESES[m as usize - 1]);
            con_nombre(
                plantillas::hacer(clave, &tr).expect("es de la tabla"),
                &rotulo,
            )
        })
        .collect())
}

/// El tramo en el nombre: la hoja guardada en figuras se distingue de la de
/// serie y dice que lleva.
fn con_nombre(mut f: FiguraGuardada, rotulo: &str) -> FiguraGuardada {
    f.nombre = format!("{} ({rotulo})", f.nombre);
    debug_assert!(f.nombre.contains(SEPARADOR));
    f
}

/// El tramo de la de serie de una plantilla que se alarga («1–42»).
pub fn tramo_de_serie(clave: &str) -> Option<(u32, u32)> {
    AMPLIABLES
        .iter()
        .find(|a| a.clave == clave)
        .map(|a| a.defecto)
}
