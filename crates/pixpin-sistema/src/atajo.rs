//! Combinaciones de teclas globales.
//!
//! Vive en `pixpin-sistema` (capa 0) y no en `pixpin-store` por la regla de
//! capas: lo usan `pixpin-shell` (L1, que llama a `RegisterHotKey`) y
//! `pixpin-store` (L2, que lo guarda en los ajustes). Es logica pura: el
//! parseo no necesita Windows y compila en cualquier sistema.
//!
//! Se serializa como texto ("Ctrl+Alt+X") en vez de como estructura para que
//! el fichero TOML sea legible y editable a mano.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// Constantes de Win32, repetidas aqui para que el parseo sea logica pura y no
// arrastre el crate `windows` a los tests.
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;
const VK_F1: u32 = 0x70;
const VK_SNAPSHOT: u32 = 0x2C;
const VK_INSERT: u32 = 0x2D;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modificadores {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Modificadores {
    fn alguno(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.win
    }
}

/// La tecla final de la combinacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tecla {
    /// Siempre en mayuscula.
    Letra(char),
    /// 0..=9
    Digito(u8),
    /// 1..=24
    Funcion(u8),
    /// Impr Pant
    Imprimir,
    Insertar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Atajo {
    pub modificadores: Modificadores,
    pub tecla: Tecla,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ErrorAtajo {
    #[error("la combinacion esta vacia")]
    Vacia,
    #[error("falta la tecla final")]
    SinTecla,
    #[error(
        "hace falta al menos un modificador: una tecla suelta a nivel global \
         impediria escribirla en cualquier otra aplicacion"
    )]
    SinModificador,
    #[error("modificador repetido: `{0}`")]
    ModificadorRepetido(String),
    #[error("tecla desconocida: `{0}`")]
    TeclaDesconocida(String),
}

impl Tecla {
    /// La tecla que corresponde a un codigo virtual de Windows, si es de
    /// las que valen como final de un atajo.
    ///
    /// `None` para lo demas: los modificadores solos, las flechas, Intro,
    /// Escape... Un atajo global con Intro de tecla final se comeria el
    /// Intro de TODAS las aplicaciones, y eso no es un atajo, es un
    /// secuestro. Lo que no esta aqui no se puede grabar desde la ventana
    /// de ajustes, y es a proposito.
    pub fn desde_vk(vk: u32) -> Option<Tecla> {
        match vk {
            0x41..=0x5A => Some(Tecla::Letra(vk as u8 as char)),
            0x30..=0x39 => Some(Tecla::Digito((vk - 0x30) as u8)),
            VK_F1..=0x87 => Some(Tecla::Funcion((vk - VK_F1 + 1) as u8)),
            VK_SNAPSHOT => Some(Tecla::Imprimir),
            VK_INSERT => Some(Tecla::Insertar),
            _ => None,
        }
    }
}

impl Atajo {
    /// El atajo que corresponde a una tecla pulsada con estos
    /// modificadores, o `None` si no vale como atajo.
    ///
    /// Sin ningun modificador tampoco vale, y es la misma regla que
    /// aplica `FromStr` al leer el fichero: un atajo global «A» a secas se
    /// tragaria la A de todos los programas, y «F1» a secas la ayuda de
    /// todos. Las dos vias tienen que decir lo mismo, o un atajo que se
    /// puede grabar desde la ventana no se podria volver a leer del TOML.
    pub fn desde_teclado(vk: u32, modificadores: Modificadores) -> Option<Atajo> {
        let tecla = Tecla::desde_vk(vk)?;
        if !modificadores.alguno() {
            return None;
        }
        Some(Atajo {
            modificadores,
            tecla,
        })
    }

    /// Mascara de modificadores tal como la espera `RegisterHotKey`.
    pub fn modificadores_win32(&self) -> u32 {
        let m = self.modificadores;
        (if m.alt { MOD_ALT } else { 0 })
            | (if m.ctrl { MOD_CONTROL } else { 0 })
            | (if m.shift { MOD_SHIFT } else { 0 })
            | (if m.win { MOD_WIN } else { 0 })
    }

    /// Codigo de tecla virtual tal como lo espera `RegisterHotKey`.
    ///
    /// Los campos de `Tecla` son publicos (lo exige el spec), asi que esta
    /// funcion no puede asumir que sus valores vinieron de `FromStr`. Toda la
    /// aritmetica se hace en `u32` (nunca en `u8`) para que un valor fuera de
    /// rango construido a mano (p. ej. `Tecla::Funcion(0)`) de un resultado
    /// definido en vez de entrar en panico o desbordar en silencio.
    pub fn tecla_win32(&self) -> u32 {
        match self.tecla {
            // Se normaliza a mayuscula por si se construye `Tecla::Letra`
            // a mano con una minuscula.
            Tecla::Letra(c) => c.to_ascii_uppercase() as u32,
            Tecla::Digito(d) => u32::from(b'0') + u32::from(d),
            Tecla::Funcion(n) => VK_F1 + u32::from(n).saturating_sub(1),
            Tecla::Imprimir => VK_SNAPSHOT,
            Tecla::Insertar => VK_INSERT,
        }
    }
}

impl FromStr for Atajo {
    type Err = ErrorAtajo;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        let texto = texto.trim();
        if texto.is_empty() {
            return Err(ErrorAtajo::Vacia);
        }

        let partes: Vec<&str> = texto.split('+').map(str::trim).collect();
        if partes.iter().any(|p| p.is_empty()) {
            return Err(ErrorAtajo::SinTecla);
        }

        let (ultima, previas) = partes.split_last().expect("nunca vacio tras el trim");

        let mut modificadores = Modificadores::default();
        for parte in previas {
            let campo = match parte.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => &mut modificadores.ctrl,
                "alt" => &mut modificadores.alt,
                "shift" | "mayus" => &mut modificadores.shift,
                "win" => &mut modificadores.win,
                _ => return Err(ErrorAtajo::TeclaDesconocida((*parte).to_string())),
            };
            if *campo {
                return Err(ErrorAtajo::ModificadorRepetido((*parte).to_string()));
            }
            *campo = true;
        }

        if !modificadores.alguno() {
            return Err(ErrorAtajo::SinModificador);
        }

        let tecla = parsear_tecla(ultima)?;
        Ok(Atajo {
            modificadores,
            tecla,
        })
    }
}

fn parsear_tecla(texto: &str) -> Result<Tecla, ErrorAtajo> {
    let arriba = texto.to_ascii_uppercase();

    if arriba.len() == 1 {
        let c = arriba.chars().next().expect("longitud comprobada");
        if c.is_ascii_alphabetic() {
            return Ok(Tecla::Letra(c));
        }
        if let Some(d) = c.to_digit(10) {
            return Ok(Tecla::Digito(d as u8));
        }
    }

    if let Some(resto) = arriba.strip_prefix('F') {
        if let Ok(n) = resto.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Ok(Tecla::Funcion(n));
            }
        }
    }

    match arriba.as_str() {
        "IMPR" | "IMPRPANT" | "PRTSC" => Ok(Tecla::Imprimir),
        "INS" | "INSERT" | "INSERTAR" => Ok(Tecla::Insertar),
        _ => Err(ErrorAtajo::TeclaDesconocida(texto.to_string())),
    }
}

impl fmt::Display for Atajo {
    /// Forma canonica: siempre en el orden Ctrl, Alt, Shift, Win.
    ///
    /// El orden fijo es lo que hace que la ida y vuelta sea estable y que dos
    /// ficheros de ajustes equivalentes se vean iguales.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.modificadores;
        if m.ctrl {
            write!(f, "Ctrl+")?;
        }
        if m.alt {
            write!(f, "Alt+")?;
        }
        if m.shift {
            write!(f, "Shift+")?;
        }
        if m.win {
            write!(f, "Win+")?;
        }
        match self.tecla {
            Tecla::Letra(c) => write!(f, "{c}"),
            Tecla::Digito(d) => write!(f, "{d}"),
            Tecla::Funcion(n) => write!(f, "F{n}"),
            Tecla::Imprimir => write!(f, "Impr"),
            Tecla::Insertar => write!(f, "Ins"),
        }
    }
}

// Serde va por texto para que el TOML quede legible y editable a mano.
impl Serialize for Atajo {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Atajo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        texto.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn de_tecla_virtual_a_atajo_y_vuelta() {
        // La conversion inversa tiene que cuadrar con la directa para
        // TODAS las teclas que se pueden escribir: si no, un atajo grabado
        // en la ventana se registraria con otra tecla distinta.
        let todas = [
            "Ctrl+A",
            "Alt+Z",
            "Ctrl+Alt+0",
            "Shift+9",
            "Ctrl+F1",
            "Alt+F12",
            "Shift+F24",
            "Ctrl+Impr",
            "Alt+Ins",
        ];
        for texto in todas {
            let a: Atajo = texto.parse().unwrap();
            let vuelta = Atajo::desde_teclado(a.tecla_win32(), a.modificadores).unwrap();
            assert_eq!(vuelta, a, "{texto}");
        }
    }

    #[test]
    fn sin_modificador_no_es_atajo_y_es_la_misma_regla_que_al_leer() {
        // Caso negativo: «A» a secas como atajo global se tragaria la A
        // de todos los programas. Y tiene que coincidir con `FromStr`: lo
        // que se graba desde la ventana debe poder leerse del fichero.
        assert_eq!(Atajo::desde_teclado(0x41, Modificadores::default()), None);
        assert!("A".parse::<Atajo>().is_err());
        assert_eq!(Atajo::desde_teclado(VK_F1, Modificadores::default()), None);
        assert!("F1".parse::<Atajo>().is_err());
    }

    #[test]
    fn intro_escape_y_flechas_no_son_teclas_de_atajo() {
        // Caso negativo: Intro como tecla final secuestraria el Intro de
        // todas las aplicaciones. Lo mismo Escape y las flechas.
        let ctrl = Modificadores {
            ctrl: true,
            ..Modificadores::default()
        };
        for vk in [0x0D, 0x1B, 0x25, 0x26, 0x27, 0x28, 0x20, 0x08, 0x09] {
            assert_eq!(Atajo::desde_teclado(vk, ctrl), None, "vk {vk:#x}");
        }
    }

    #[test]
    fn parsea_la_combinacion_por_defecto() {
        let a: Atajo = "Ctrl+Alt+X".parse().unwrap();
        assert!(a.modificadores.ctrl);
        assert!(a.modificadores.alt);
        assert!(!a.modificadores.shift);
        assert!(!a.modificadores.win);
        assert_eq!(a.tecla, Tecla::Letra('X'));
    }

    #[test]
    fn el_formato_es_reversible() {
        // Ojo: se escriben ya en forma canonica (Ctrl, Alt, Shift, Win). Ese
        // orden fijo es lo que hace estable la ida y vuelta.
        for texto in ["Ctrl+Alt+X", "Ctrl+Shift+F12", "Alt+Win+9", "Ctrl+Impr"] {
            let a: Atajo = texto.parse().unwrap();
            assert_eq!(a.to_string(), texto, "no sobrevive la ida y vuelta");
        }
    }

    #[test]
    fn canonicaliza_el_orden_de_los_modificadores_al_escribir() {
        // A diferencia del test de arriba, aqui la ENTRADA no esta en orden
        // canonico: si `Display` se limitara a repetir el orden de entrada
        // en vez de canonicalizar, este test lo detectaria.
        for (entrada, canonico) in [
            ("Win+Alt+9", "Alt+Win+9"),
            ("Shift+Ctrl+X", "Ctrl+Shift+X"),
            ("Win+Shift+Alt+Ctrl+F1", "Ctrl+Alt+Shift+Win+F1"),
        ] {
            let a: Atajo = entrada.parse().unwrap();
            assert_eq!(
                a.to_string(),
                canonico,
                "\"{entrada}\" deberia canonicalizarse a \"{canonico}\""
            );
        }
    }

    #[test]
    fn no_distingue_mayusculas_al_leer() {
        let a: Atajo = "ctrl+alt+x".parse().unwrap();
        let b: Atajo = "CTRL+ALT+X".parse().unwrap();
        assert_eq!(a, b);
        // Pero al escribir siempre sale en la forma canonica.
        assert_eq!(a.to_string(), "Ctrl+Alt+X");
    }

    #[test]
    fn rechaza_combinaciones_invalidas() {
        // Sin tecla final.
        assert!("Ctrl+".parse::<Atajo>().is_err());
        // Sin ningun modificador: RegisterHotKey lo aceptaria, pero secuestrar
        // una tecla suelta a nivel global rompe la escritura en cualquier app.
        assert!("X".parse::<Atajo>().is_err());
        // Tecla desconocida.
        assert!("Ctrl+Alt+Inexistente".parse::<Atajo>().is_err());
        // Modificador repetido.
        assert!("Ctrl+Ctrl+X".parse::<Atajo>().is_err());
        // Vacio.
        assert!("".parse::<Atajo>().is_err());
    }

    #[test]
    fn traduce_a_los_codigos_de_win32() {
        let a: Atajo = "Ctrl+Alt+X".parse().unwrap();
        // MOD_ALT = 0x0001, MOD_CONTROL = 0x0002
        assert_eq!(a.modificadores_win32(), 0x0001 | 0x0002);
        // El codigo virtual de una letra es su mayuscula ASCII.
        assert_eq!(a.tecla_win32(), 'X' as u32);

        let f: Atajo = "Ctrl+Shift+F12".parse().unwrap();
        // VK_F1 = 0x70, luego F12 = 0x7B
        assert_eq!(f.tecla_win32(), 0x7B);

        let d: Atajo = "Ctrl+5".parse().unwrap();
        assert_eq!(d.tecla, Tecla::Digito(5));
        // El codigo virtual de un digito es su caracter ASCII.
        assert_eq!(d.tecla_win32(), '5' as u32);
        assert_eq!(d.to_string(), "Ctrl+5", "no sobrevive la ida y vuelta");
        assert_eq!("Ctrl+5".parse::<Atajo>().unwrap(), d);

        let p: Atajo = "Ctrl+Impr".parse().unwrap();
        assert_eq!(p.tecla, Tecla::Imprimir);
        assert_eq!(p.tecla_win32(), 0x2C); // VK_SNAPSHOT
        assert_eq!(p.to_string(), "Ctrl+Impr", "no sobrevive la ida y vuelta");
        assert_eq!("Ctrl+Impr".parse::<Atajo>().unwrap(), p);

        let i: Atajo = "Ctrl+Ins".parse().unwrap();
        assert_eq!(i.tecla, Tecla::Insertar);
        assert_eq!(i.tecla_win32(), 0x2D); // VK_INSERT
        assert_eq!(i.to_string(), "Ctrl+Ins", "no sobrevive la ida y vuelta");
        assert_eq!("Ctrl+Ins".parse::<Atajo>().unwrap(), i);
    }

    #[test]
    fn tecla_win32_no_entra_en_panico_con_valores_fuera_de_rango() {
        // Los campos de Tecla son publicos (lo exige el spec), asi que se
        // pueden construir valores que `parsear_tecla` nunca produciria
        // (Funcion solo sale de ahi en el rango 1..=24). tecla_win32() debe
        // devolver algo definido, no entrar en panico ni desbordar en
        // silencio.
        let sin_pasar_por_el_parser = Atajo {
            modificadores: Modificadores {
                ctrl: true,
                ..Default::default()
            },
            tecla: Tecla::Funcion(0),
        };
        // n.saturating_sub(1) con n = 0 da 0, asi que el resultado es VK_F1.
        assert_eq!(sin_pasar_por_el_parser.tecla_win32(), 0x70);

        // Digito con un valor que desbordaria un u8 si la suma se hiciera en
        // espacio u8 (b'0' + d con d >= 208).
        let digito_grande = Atajo {
            modificadores: Modificadores {
                ctrl: true,
                ..Default::default()
            },
            tecla: Tecla::Digito(250),
        };
        assert_eq!(digito_grande.tecla_win32(), u32::from(b'0') + 250);
    }

    #[test]
    fn tecla_win32_normaliza_letras_minusculas_construidas_a_mano() {
        // El comentario en la definicion de Tecla::Letra dice "siempre en
        // mayuscula", pero el campo es publico: si alguien lo construye a
        // mano con una minuscula, tecla_win32() debe normalizar en vez de
        // devolver un codigo de tecla equivocado en silencio.
        let minuscula = Atajo {
            modificadores: Modificadores {
                ctrl: true,
                ..Default::default()
            },
            tecla: Tecla::Letra('x'),
        };
        assert_eq!(minuscula.tecla_win32(), 'X' as u32);
    }

    #[test]
    fn se_serializa_como_texto_plano() {
        let a: Atajo = "Ctrl+Alt+X".parse().unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, "\"Ctrl+Alt+X\"");
        let vuelta: Atajo = serde_json::from_str(&json).unwrap();
        assert_eq!(a, vuelta);
    }
}
