//! **Las ordenes de capas** de cada dia: CAPA (hacer actual o crear por su
//! nombre), aislar y desaislar, apagar y encender, bloquear y desbloquear,
//! y la capa actual desde un objeto. Las capas no entran en deshacer (como
//! en AutoCAD, salvo con CAPAPREVIA).

use opencadcodec::Handle;

use crate::dibujo::Dibujo;
use crate::especialidad as esp;
use crate::geo::{self, P};
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, Registro};
use crate::ordenes::{self, Grupo, Id, Orden};
use crate::seleccion_rapida as sr;

/// La orden que empieza CAPA ya en «Nueva» (el «+» del panel de capas).
pub const CAPANUEVA: &str = "CAPANUEVA";

macro_rules! capa {
    ($n:literal, [$($a:literal),*], $obj:literal, $et:literal, $ay:literal, $crear:expr) => {
        Registro {
            orden: Orden {
                id: Id::Otra($n),
                nombre: $n,
                alias: &[$($a),*],
                grupo: Grupo::Vista,
                ayuda: $ay,
                con_objetos: $obj,
            },
            etiqueta: $et,
            crear: $crear,
        }
    };
}

pub const REGISTROS: &[Registro] = &[
    capa!(
        "CAPA",
        ["CA", "LAYER"],
        false,
        "Capa",
        "Hacer actual una capa por su nombre, o crear una (Nueva)",
        || Box::new(Capa { modo: Modo::Elegir })
    ),
    Registro {
        orden: Orden {
            id: Id::Otra(CAPANUEVA),
            nombre: CAPANUEVA,
            alias: &[],
            grupo: Grupo::Edicion,
            ayuda: "Crear una capa y hacerla actual",
            con_objetos: false,
        },
        etiqueta: "Capa nueva",
        crear: || Box::new(Capa { modo: Modo::Nueva }),
    },
    capa!(
        "CAPAAISLAR",
        ["LAYISO"],
        true,
        "Aislar capa",
        "Apagar todas las capas menos las de lo elegido",
        || Box::new(PorObjetos(Que::Aislar))
    ),
    capa!(
        "CAPADESAISLAR",
        ["LAYUNISO"],
        false,
        "Desaislar",
        "Encender lo que apago CAPAAISLAR",
        || Box::new(Desaislar)
    ),
    capa!(
        "CAPAOFF",
        ["LAYOFF"],
        true,
        "Apagar capa",
        "Apagar las capas de lo elegido",
        || Box::new(PorObjetos(Que::Apagar))
    ),
    capa!(
        "CAPAON",
        ["LAYON"],
        false,
        "Encender todo",
        "Encender todas las capas",
        || Box::new(EncenderTodas)
    ),
    capa!(
        "CAPAACT",
        ["LAYMCUR"],
        true,
        "Capa del objeto",
        "Hacer actual la capa de un objeto",
        || Box::new(PorObjetos(Que::Actual))
    ),
    capa!(
        "CAPABLOQ",
        ["LAYLCK"],
        true,
        "Bloquear capa",
        "Bloquear las capas de lo elegido: se ven pero no se eligen",
        || Box::new(PorObjetos(Que::Bloquear))
    ),
    capa!(
        "CAPADESBLOQ",
        ["LAYULK"],
        false,
        "Desbloquear",
        "Desbloquear la capa de un objeto (o por su nombre)",
        || Box::new(Desbloquear)
    ),
];

/// El nombre de verdad de la capa `n` (sin importar mayusculas), si esta.
fn nombre_real(d: &Dibujo, n: &str) -> Option<String> {
    d.doc.layers.get(n.trim()).map(|l| l.name.clone())
}

/// **Si un nombre de capa vale**: no vacio, sin los caracteres que no
/// admiten DWG ni DXF y de 255 como mucho.
pub fn nombre_valido(n: &str) -> bool {
    let n = n.trim();
    !n.is_empty()
        && n.chars().count() <= 255
        && !n
            .chars()
            .any(|c| c.is_control() || "<>/\\\":;?*|,=`".contains(c))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Modo {
    /// Escribir una capa para hacerla actual, o una opcion.
    Elegir,
    Nueva,
    Actual,
}

/// CAPA: lo que se escribe es el nombre tal cual (con espacios).
struct Capa {
    modo: Modo,
}

impl Capa {
    fn hacer_actual(cx: &mut Contexto, n: &str) -> Flujo {
        match nombre_real(cx.dibujo, n) {
            Some(real) => {
                cx.dibujo.poner_capa_actual(&real);
                cx.decir(format!("Capa actual: {real}"));
                Flujo::Termina
            }
            None => {
                cx.decir(format!(
                    "No hay una capa «{}» (escriba N para crearla)",
                    n.trim()
                ));
                Flujo::Sigue
            }
        }
    }

    fn crear(cx: &mut Contexto, n: &str) -> Flujo {
        let n = n.trim();
        if !nombre_valido(n) {
            cx.decir(format!(
                "«{n}» no vale como nombre de capa (sin < > / \\ \" : ; ? * | , = `)"
            ));
            return Flujo::Sigue;
        }
        if let Some(real) = nombre_real(cx.dibujo, n) {
            cx.dibujo.poner_capa_actual(&real);
            cx.decir(format!("La capa {real} ya estaba: queda como actual"));
            return Flujo::Termina;
        }
        cx.dibujo.asegurar_capa(n, esp::color_de_capa(n));
        cx.dibujo.poner_capa_actual(n);
        cx.decir(format!("Capa nueva y actual: {n}"));
        Flujo::Termina
    }
}

impl Herramienta for Capa {
    fn indicacion(&self, cx: &Lectura) -> String {
        match self.modo {
            Modo::Elegir => format!(
                "Capa a hacer actual o [Nueva/Actual] <{}>:",
                cx.dibujo.capa_actual()
            ),
            Modo::Nueva => "Nombre de la capa nueva:".into(),
            Modo::Actual => "Capa a hacer actual:".into(),
        }
    }

    fn pide_texto(&self) -> bool {
        true
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        match self.modo {
            Modo::Elegir => match ordenes::sin_tildes(t.trim()).to_uppercase().as_str() {
                "N" | "NUEVA" | "NEW" => {
                    self.modo = Modo::Nueva;
                    Flujo::Sigue
                }
                "A" | "ACTUAL" | "S" | "SET" => {
                    self.modo = Modo::Actual;
                    Flujo::Sigue
                }
                _ => Capa::hacer_actual(cx, t),
            },
            Modo::Nueva => Capa::crear(cx, t),
            Modo::Actual => Capa::hacer_actual(cx, t),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Que {
    Aislar,
    Apagar,
    Actual,
    Bloquear,
}

/// Las que trabajan con las capas de lo elegido.
struct PorObjetos(Que);

impl Herramienta for PorObjetos {
    fn indicacion(&self, _cx: &Lectura) -> String {
        String::new()
    }

    fn al_tener_objetos(&mut self, cx: &mut Contexto) -> Flujo {
        let capas = sr::capas_de(cx.dibujo, cx.seleccion);
        let Some(primera) = capas.first().cloned() else {
            return Flujo::Termina;
        };
        let actual = cx.dibujo.capa_actual();
        let es = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        match self.0 {
            Que::Aislar => {
                let mut apagadas = Vec::new();
                for l in cx.dibujo.doc.layers.iter_mut() {
                    if !l.is_off() && !capas.iter().any(|c| es(c, &l.name)) {
                        l.turn_off();
                        apagadas.push(l.name.clone());
                    }
                }
                if !capas.iter().any(|c| es(c, &actual)) {
                    cx.dibujo.poner_capa_actual(&primera);
                }
                cx.dibujo.tocar();
                cx.decir(format!(
                    "{} aislada(s): {} capas apagadas",
                    capas.join(", "),
                    apagadas.len()
                ));
                for a in apagadas {
                    if !cx.memoria.capas_aisladas.contains(&a) {
                        cx.memoria.capas_aisladas.push(a);
                    }
                }
                cx.seleccion.clear();
            }
            Que::Apagar => {
                let mut n = 0;
                for c in &capas {
                    if es(c, &actual) {
                        cx.decir(format!("La capa actual ({c}) no se apaga"));
                        continue;
                    }
                    cx.dibujo.con_capa(c, |l| l.turn_off());
                    n += 1;
                }
                if n > 0 {
                    cx.decir(format!("{n} capas apagadas"));
                }
                cx.seleccion.clear();
            }
            Que::Actual => {
                cx.dibujo.poner_capa_actual(&primera);
                cx.decir(format!("Capa actual: {primera}"));
            }
            Que::Bloquear => {
                for c in &capas {
                    cx.dibujo.con_capa(c, |l| l.lock());
                }
                cx.decir(format!("Bloqueada(s): {}", capas.join(", ")));
                cx.seleccion.clear();
            }
        }
        Flujo::Termina
    }
}

struct Desaislar;

impl Herramienta for Desaislar {
    fn indicacion(&self, _cx: &Lectura) -> String {
        String::new()
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        let capas = std::mem::take(&mut cx.memoria.capas_aisladas);
        if capas.is_empty() {
            cx.decir("No hay capas aisladas");
            return Flujo::Termina;
        }
        let n = capas
            .iter()
            .filter(|c| cx.dibujo.con_capa(c, |l| l.turn_on()))
            .count();
        cx.decir(format!("{n} capas encendidas de nuevo"));
        Flujo::Termina
    }
}

struct EncenderTodas;

impl Herramienta for EncenderTodas {
    fn indicacion(&self, _cx: &Lectura) -> String {
        String::new()
    }

    fn al_empezar(&mut self, cx: &mut Contexto) -> Flujo {
        let mut n = 0;
        for l in cx.dibujo.doc.layers.iter_mut() {
            if l.is_off() {
                l.turn_on();
                n += 1;
            }
        }
        if n > 0 {
            cx.dibujo.tocar();
        }
        cx.memoria.capas_aisladas.clear();
        cx.decir(format!("{n} capas encendidas"));
        Flujo::Termina
    }
}

/// Lo dibujado mas cerca de `p`, aunque este en una capa bloqueada (que no
/// se puede elegir con un clic normal), a un tiro de ~0,5 % del dibujo.
fn bajo_el_punto(d: &Dibujo, p: P) -> Option<Handle> {
    let ind = d.indice();
    let c = ind.caja()?;
    let r = ((c[2] - c[0]).hypot(c[3] - c[1]) / 200.0).max(1e-6);
    let zona = geo::ensanchar([p[0], p[1], p[0], p[1]], r);
    ind.en(&zona)
        .into_iter()
        .map(|e| {
            let dist = if e.formas.is_empty() {
                if geo::en_caja(&e.caja, p) {
                    0.0
                } else {
                    f64::INFINITY
                }
            } else {
                e.formas
                    .iter()
                    .map(|f| f.distancia(p))
                    .fold(f64::INFINITY, f64::min)
            };
            (dist, e.handle)
        })
        .filter(|(dist, _)| *dist <= r)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, h)| h)
}

/// CAPADESBLOQ: un clic en lo bloqueado (o su nombre).
struct Desbloquear;

impl Desbloquear {
    fn capa(cx: &mut Contexto, n: &str) -> Flujo {
        let Some(real) = nombre_real(cx.dibujo, n) else {
            cx.decir(format!("No hay una capa «{}»", n.trim()));
            return Flujo::Sigue;
        };
        let bloqueada = cx
            .dibujo
            .doc
            .layers
            .get(&real)
            .is_some_and(|l| l.is_locked());
        if !bloqueada {
            cx.decir(format!("La capa {real} no estaba bloqueada"));
            return Flujo::Sigue;
        }
        cx.dibujo.con_capa(&real, |l| l.unlock());
        cx.decir(format!("Desbloqueada: {real}"));
        Flujo::Termina
    }
}

impl Herramienta for Desbloquear {
    fn indicacion(&self, _cx: &Lectura) -> String {
        "Designe un objeto de la capa bloqueada (o escriba su nombre):".into()
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        match bajo_el_punto(cx.dibujo, p) {
            Some(h) => {
                let capa = sr::capas_de(cx.dibujo, &[h]).pop().unwrap_or_default();
                Desbloquear::capa(cx, &capa)
            }
            None => {
                cx.decir("Ahi no hay nada");
                Flujo::Sigue
            }
        }
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        Desbloquear::capa(cx, t)
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        Desbloquear::capa(cx, &v.to_string())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::Sesion;

    fn visible(s: &Sesion, c: &str) -> bool {
        s.dibujo.doc.layers.get(c).unwrap().is_visible()
    }

    /// Una linea en A-MUROS (y = 0) y otra en E-ESTRUCTURAS (y = 10).
    fn sesion() -> Sesion {
        let mut s = Sesion::nueva(Dibujo::nuevo());
        for (capa, y) in [("A-MUROS", 0.0), ("E-ESTRUCTURAS", 10.0)] {
            s.dibujo
                .editar("x", |t| t.anadir_en(esp::linea([0.0, y], [100.0, y]), capa));
        }
        s
    }

    #[test]
    fn nombres_de_capa() {
        assert!(nombre_valido("A-MUROS 2"));
        assert!(nombre_valido("Ñandú"));
        assert!(!nombre_valido(""));
        assert!(!nombre_valido("   "));
        assert!(!nombre_valido("a/b"));
        assert!(!nombre_valido("a*"));
        assert!(!nombre_valido(&"x".repeat(256)));
    }

    #[test]
    fn capa_actual_nueva_y_errores() {
        let mut s = sesion();
        s.escribir("ca");
        assert!(s.pide_texto());
        s.escribir("a-muros");
        assert_eq!(s.dibujo.capa_actual(), "A-MUROS");
        assert_eq!(s.activa(), None);
        // Una que no esta: lo dice y sigue pidiendo.
        s.escribir("layer");
        s.escribir("NO HAY");
        assert!(s.activa().is_some());
        assert!(s.registro.last().unwrap().contains("No hay"));
        s.escribir("n");
        s.escribir("a/b");
        assert!(s.activa().is_some());
        s.escribir("Mi capa 1");
        assert_eq!(s.activa(), None);
        assert_eq!(s.dibujo.capa_actual(), "Mi capa 1");
        // CAPANUEVA (el + del panel) con una que ya esta: solo la hace actual.
        let n = s.dibujo.doc.layers.len();
        s.orden(Id::Otra(CAPANUEVA));
        s.escribir("a-muros");
        assert_eq!(s.dibujo.doc.layers.len(), n);
        assert_eq!(s.dibujo.capa_actual(), "A-MUROS");
        // Actual: solo las que estan.
        s.escribir("ca");
        s.escribir("a");
        s.escribir("0");
        assert_eq!(s.dibujo.capa_actual(), "0");
    }

    #[test]
    fn aislar_y_desaislar() {
        let mut s = sesion();
        s.escribir("layuniso");
        assert!(s.registro.last().unwrap().contains("No hay"));
        s.clic([50.0, 0.0], 1.0);
        s.escribir("layiso");
        assert!(visible(&s, "A-MUROS"));
        assert!(!visible(&s, "E-ESTRUCTURAS"));
        assert!(!visible(&s, "0"));
        assert_eq!(s.dibujo.capa_actual(), "A-MUROS");
        assert!(s.seleccion.is_empty());
        s.escribir("layuniso");
        assert!(visible(&s, "E-ESTRUCTURAS") && visible(&s, "0"));
        assert!(s.memoria.capas_aisladas.is_empty());
    }

    #[test]
    fn apagar_encender_y_la_actual_no_se_apaga() {
        let mut s = sesion();
        s.clic([50.0, 10.0], 1.0);
        s.escribir("layoff");
        assert!(!visible(&s, "E-ESTRUCTURAS"));
        s.dibujo.poner_capa_actual("A-MUROS");
        s.clic([50.0, 0.0], 1.0);
        s.escribir("layoff");
        assert!(visible(&s, "A-MUROS"));
        s.escribir("layon");
        assert!(visible(&s, "E-ESTRUCTURAS"));
    }

    #[test]
    fn actual_desde_un_objeto() {
        let mut s = sesion();
        s.escribir("laymcur");
        assert!(s.pide_objetos());
        s.clic([50.0, 10.0], 1.0);
        s.escribir("");
        assert_eq!(s.dibujo.capa_actual(), "E-ESTRUCTURAS");
    }

    #[test]
    fn bloquear_y_desbloquear_con_un_clic() {
        let mut s = sesion();
        s.clic([50.0, 0.0], 1.0);
        s.escribir("laylck");
        assert!(s.dibujo.doc.layers.get("A-MUROS").unwrap().is_locked());
        // Ya no se elige con un clic.
        s.clic([50.0, 0.0], 1.0);
        assert!(s.seleccion.is_empty());
        s.escribir("layulk");
        // Lejos de todo: sigue pidiendo.
        s.clic([50.0, 500.0], 1.0);
        assert!(s.activa().is_some());
        // Una capa que no esta bloqueada: tampoco.
        s.escribir("E-ESTRUCTURAS");
        assert!(s.activa().is_some());
        s.clic([50.0, 0.2], 1.0);
        assert_eq!(s.activa(), None);
        assert!(!s.dibujo.doc.layers.get("A-MUROS").unwrap().is_locked());
    }
}
