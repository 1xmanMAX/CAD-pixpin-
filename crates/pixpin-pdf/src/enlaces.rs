//! **Los enlaces de una hoja del PDF**: los del indice («ver pagina 5») y
//! los de la web.
//!
//! `Windows.Data.Pdf` dibuja la hoja pero no dice donde hay un enlace ni a
//! donde lleva, asi que se leen de las anotaciones de la hoja (`/Annots`,
//! `/Subtype /Link`) con el mismo analizador propio que el texto (`union`):
//!
//! - **A otra hoja**: `/Dest` o una accion `/A << /S /GoTo /D … >>`, con el
//!   destino como lista (`[hoja /XYZ izquierda arriba zoom]`, `/Fit`, `/FitH
//!   arriba`…) o **con nombre** (lo que escribe LaTeX con `hyperref` o Word
//!   con sus marcadores): el nombre se busca en `/Dests` del catalogo o en el
//!   arbol `/Names /Dests`.
//! - **A la web**: `/A << /S /URI /URI (…) >>`.
//!
//! Lo demas (abrir un fichero, JavaScript, otro PDF) no se ofrece: un clic
//! en un documento no ejecuta nada.
//!
//! La caja sale en fracciones de la hoja (0..1 desde arriba a la izquierda,
//! con su `/Rotate`), como las letras de [`crate::texto`].
//!
//! Partes traducidas de pdfcraft (https://github.com/storytold/pdfcraft), MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team and the Pdfcraft contributors.
//! (`links.rs`: `action_of`, el destino por la primera entrada de la lista).

use std::collections::HashMap;

use crate::texto::{a_fraccion, caja_de, giro_de_la_hoja, heredado, numero_de};
use crate::union::{Archivo, Dicc, Valor, en};

/// Cuantos enlaces se leen como mucho de una hoja.
const MAX_ENLACES: usize = 2_000;
/// Hasta donde se baja en el arbol de nombres.
const HONDO_NOMBRES: usize = 24;

/// A donde lleva un enlace.
#[derive(Debug, Clone, PartialEq)]
pub enum Destino {
    /// Una hoja de este PDF (desde 0) y, si lo dice, a que altura de ella
    /// (0 arriba, 1 abajo).
    Hoja { hoja: usize, alto: Option<f32> },
    /// Una direccion de la web (o de correo).
    Web(String),
}

/// Un enlace de la hoja: su caja en fracciones de la hoja y a donde lleva.
#[derive(Debug, Clone, PartialEq)]
pub struct Enlace {
    pub caja: [f32; 4],
    pub destino: Destino,
}

/// **Los enlaces de la hoja `i`** (desde 0). Un PDF cifrado o que no se
/// entiende no da ninguno.
pub fn de_la_hoja(bytes: &[u8], i: usize) -> Vec<Enlace> {
    let Some(archivo) = Archivo::leer(bytes) else {
        return Vec::new();
    };
    if archivo.cifrado() {
        return Vec::new();
    }
    let paginas = archivo.paginas();
    de_la_hoja_en(&archivo, &paginas, i)
}

pub(crate) fn de_la_hoja_en(archivo: &Archivo, paginas: &[u32], i: usize) -> Vec<Enlace> {
    let Some(&n) = paginas.get(i) else {
        return Vec::new();
    };
    let Some(hoja) = archivo.dicc_de(Some(&Valor::Ref(n, 0))) else {
        return Vec::new();
    };
    let caja = caja_de(heredado(archivo, &hoja, b"CropBox"))
        .or_else(|| caja_de(heredado(archivo, &hoja, b"MediaBox")))
        .unwrap_or([0.0, 0.0, 612.0, 792.0]);
    let giro = giro_de_la_hoja(archivo, &hoja);
    let Some(Valor::Lista(anotaciones)) = archivo.resolver(en(&hoja, b"Annots")) else {
        return Vec::new();
    };
    let por_numero: HashMap<u32, usize> =
        paginas.iter().enumerate().map(|(k, n)| (*n, k)).collect();
    let mut nombres = Nombres::default();
    let mut salida = Vec::new();
    for a in anotaciones.iter().take(MAX_ENLACES) {
        let Some(d) = archivo.dicc_de(Some(a)) else {
            continue;
        };
        if !matches!(en(&d, b"Subtype"), Some(Valor::Nombre(s)) if s == b"Link") {
            continue;
        }
        let Some(rect) = caja_de(archivo.resolver(en(&d, b"Rect"))) else {
            continue;
        };
        let Some(destino) = destino_del_enlace(archivo, &d, &por_numero, &mut nombres) else {
            continue;
        };
        let f = a_fraccion(caja, giro, rect);
        // Lo que cae del todo fuera del papel no se puede pulsar.
        if f[2] <= 0.0 || f[0] >= 1.0 || f[3] <= 0.0 || f[1] >= 1.0 {
            continue;
        }
        salida.push(Enlace {
            caja: f.map(|v| v.clamp(0.0, 1.0)),
            destino,
        });
    }
    salida
}

fn destino_del_enlace(
    archivo: &Archivo,
    d: &Dicc,
    por_numero: &HashMap<u32, usize>,
    nombres: &mut Nombres,
) -> Option<Destino> {
    if let Some(dest) = archivo.resolver(en(d, b"Dest")) {
        return a_la_hoja(archivo, dest, por_numero, nombres);
    }
    let accion = archivo.dicc_de(en(d, b"A"))?;
    match en(&accion, b"S") {
        Some(Valor::Nombre(s)) if s == b"URI" => {
            let Some(Valor::Cadena(c)) = archivo.resolver(en(&accion, b"URI")) else {
                return None;
            };
            let bytes = crate::plano::bytes_de_cadena(&c);
            let uri = String::from_utf8_lossy(&bytes).trim().to_string();
            (!uri.is_empty()).then_some(Destino::Web(uri))
        }
        Some(Valor::Nombre(s)) if s == b"GoTo" => {
            let dest = archivo.resolver(en(&accion, b"D"))?;
            a_la_hoja(archivo, dest, por_numero, nombres)
        }
        _ => None,
    }
}

/// Un destino (lista, nombre o cadena con nombre) a hoja y altura.
fn a_la_hoja(
    archivo: &Archivo,
    dest: Valor,
    por_numero: &HashMap<u32, usize>,
    nombres: &mut Nombres,
) -> Option<Destino> {
    let lista = match dest {
        Valor::Lista(l) => l,
        Valor::Nombre(n) => nombres.buscar(archivo, &n)?,
        Valor::Cadena(c) => nombres.buscar(archivo, &crate::plano::bytes_de_cadena(&c))?,
        // `<< /D [...] >>`: un destino con nombre que guarda la lista dentro.
        Valor::Dicc(dd) => match archivo.resolver(en(&dd, b"D"))? {
            Valor::Lista(l) => l,
            _ => return None,
        },
        _ => return None,
    };
    lista_a_hoja(archivo, &lista, por_numero)
}

fn lista_a_hoja(
    archivo: &Archivo,
    lista: &[Valor],
    por_numero: &HashMap<u32, usize>,
) -> Option<Destino> {
    let hoja = match lista.first()? {
        Valor::Ref(n, _) => *por_numero.get(n)?,
        // Un numero de hoja (lo que escriben algunos generadores).
        v @ Valor::Numero(_) => {
            let k = numero_de(v)?;
            if k < 0.0 || k as usize >= por_numero.len() {
                return None;
            }
            k as usize
        }
        _ => return None,
    };
    // La altura: `/XYZ izquierda arriba`, `/FitH arriba` o `/FitBH arriba`,
    // en puntos de la hoja; se pasa a fraccion desde arriba con su caja.
    let arriba = match lista.get(1) {
        Some(Valor::Nombre(x)) if x == b"XYZ" => lista.get(3),
        Some(Valor::Nombre(x)) if x == b"FitH" || x == b"FitBH" => lista.get(2),
        _ => None,
    }
    .and_then(|v| archivo.resolver(Some(v)))
    .and_then(|v| numero_de(&v));
    let alto = arriba.and_then(|a| {
        let n = *por_numero.iter().find(|(_, k)| **k == hoja)?.0;
        let d = archivo.dicc_de(Some(&Valor::Ref(n, 0)))?;
        let caja = caja_de(heredado(archivo, &d, b"CropBox"))
            .or_else(|| caja_de(heredado(archivo, &d, b"MediaBox")))?;
        if giro_de_la_hoja(archivo, &d) != 0 {
            return None;
        }
        Some(((caja[3] - a) / (caja[3] - caja[1])).clamp(0.0, 1.0))
    });
    Some(Destino::Hoja { hoja, alto })
}

/// Los destinos con nombre, leidos una vez por hoja cuando hace falta.
#[derive(Default)]
struct Nombres {
    leidos: Option<HashMap<Vec<u8>, Vec<Valor>>>,
}

impl Nombres {
    fn buscar(&mut self, archivo: &Archivo, nombre: &[u8]) -> Option<Vec<Valor>> {
        let tabla = self
            .leidos
            .get_or_insert_with(|| todos_los_nombres(archivo));
        tabla.get(nombre).cloned()
    }
}

/// `/Dests` del catalogo (PDF 1.1) y el arbol `/Names /Dests` (1.2+),
/// juntos: nombre -> lista del destino.
fn todos_los_nombres(archivo: &Archivo) -> HashMap<Vec<u8>, Vec<Valor>> {
    let mut salida = HashMap::new();
    let Some(raiz) = archivo.raiz() else {
        return salida;
    };
    let lista_de = |v: Valor| -> Option<Vec<Valor>> {
        match v {
            Valor::Lista(l) => Some(l),
            Valor::Dicc(d) => match archivo.resolver(en(&d, b"D"))? {
                Valor::Lista(l) => Some(l),
                _ => None,
            },
            _ => None,
        }
    };
    if let Some(viejos) = archivo.dicc_de(en(&raiz, b"Dests")) {
        for (k, v) in viejos {
            if let Some(l) = archivo.resolver(Some(&v)).and_then(lista_de) {
                salida.insert(k, l);
            }
        }
    }
    let Some(arbol) = archivo
        .dicc_de(en(&raiz, b"Names"))
        .and_then(|n| archivo.dicc_de(en(&n, b"Dests")))
    else {
        return salida;
    };
    // Sin recursion: una pila de nodos con su hondura.
    let mut pila = vec![(arbol, 0usize)];
    let mut vistos = 0usize;
    while let Some((nodo, hondo)) = pila.pop() {
        vistos += 1;
        if hondo > HONDO_NOMBRES || vistos > 100_000 {
            break;
        }
        if let Some(Valor::Lista(pares)) = archivo.resolver(en(&nodo, b"Names")) {
            for par in pares.chunks_exact(2) {
                let Valor::Cadena(c) = &par[0] else { continue };
                if let Some(l) = archivo.resolver(Some(&par[1])).and_then(lista_de) {
                    salida.insert(crate::plano::bytes_de_cadena(c), l);
                }
            }
        }
        if let Some(Valor::Lista(hijos)) = archivo.resolver(en(&nodo, b"Kids")) {
            for h in hijos {
                if let Some(d) = archivo.dicc_de(Some(&h)) {
                    pila.push((d, hondo + 1));
                }
            }
        }
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un PDF con estos objetos (el primero es el 1), sin indice: el lector
    /// lo reconstruye.
    fn pdf(objetos: &[&str]) -> Vec<u8> {
        let mut s = String::from("%PDF-1.4\n");
        for (i, o) in objetos.iter().enumerate() {
            s.push_str(&format!("{} 0 obj\n{o}\nendobj\n", i + 1));
        }
        s.push_str("trailer\n<< /Root 1 0 R >>\n%%EOF\n");
        s.into_bytes()
    }

    #[test]
    fn los_enlaces_llevan_a_su_hoja_a_su_altura_y_a_la_web() {
        // 1 catalogo, 2 arbol, 3 y 4 hojas de 100 x 200; la 3 con cuatro
        // enlaces: a la 4 con /XYZ, a la web, con nombre en el arbol y con
        // nombre en /Dests; y uno que no es un enlace.
        let b = pdf(&[
            "<< /Type /Catalog /Pages 2 0 R /Names << /Dests 6 0 R >> /Dests << /viejo [4 0 R /Fit] >> >>",
            "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 100 200] >>",
            "<< /Type /Page /Parent 2 0 R /Annots [5 0 R 7 0 R 8 0 R 9 0 R 10 0 R] >>",
            "<< /Type /Page /Parent 2 0 R >>",
            "<< /Type /Annot /Subtype /Link /Rect [10 180 50 190] /Dest [4 0 R /XYZ 0 150 0] >>",
            "<< /Kids [11 0 R] >>",
            "<< /Type /Annot /Subtype /Link /Rect [10 10 90 20] /A << /S /URI /URI (https://example.org/a b) >> >>",
            "<< /Subtype /Link /Rect [0 0 10 10] /A << /S /GoTo /D (cap1) >> >>",
            "<< /Subtype /Link /Rect [0 0 10 10] /Dest /viejo >>",
            "<< /Subtype /Text /Rect [0 0 10 10] >>",
            "<< /Names [(cap1) << /D [3 0 R /FitH 100] >>] >>",
        ]);
        let e = de_la_hoja(&b, 0);
        assert_eq!(e.len(), 4, "{e:?}");
        assert_eq!(
            e[0].destino,
            Destino::Hoja {
                hoja: 1,
                alto: Some(0.25)
            }
        );
        let c = e[0].caja;
        assert!(
            (c[0] - 0.1).abs() < 1e-6 && (c[1] - 0.05).abs() < 1e-6 && (c[3] - 0.1).abs() < 1e-6,
            "{c:?}"
        );
        assert_eq!(e[1].destino, Destino::Web("https://example.org/a b".into()));
        assert_eq!(
            e[2].destino,
            Destino::Hoja {
                hoja: 0,
                alto: Some(0.5)
            }
        );
        assert_eq!(
            e[3].destino,
            Destino::Hoja {
                hoja: 1,
                alto: None
            }
        );
        // Casos negativos: una hoja sin enlaces, una que no existe, y un PDF
        // que no lo es.
        assert!(de_la_hoja(&b, 1).is_empty());
        assert!(de_la_hoja(&b, 7).is_empty());
        assert!(de_la_hoja(b"no soy un pdf", 0).is_empty());
    }

    #[test]
    fn un_destino_a_una_hoja_que_no_existe_o_a_otro_programa_no_se_ofrece() {
        let b = pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 100 200] >>",
            "<< /Type /Page /Parent 2 0 R /Annots [4 0 R 5 0 R 6 0 R 7 0 R] >>",
            "<< /Subtype /Link /Rect [0 0 10 10] /Dest [99 0 R /Fit] >>",
            "<< /Subtype /Link /Rect [0 0 10 10] /A << /S /Launch /F (calc.exe) >> >>",
            "<< /Subtype /Link /Rect [0 0 10 10] /Dest (no-existe) >>",
            "<< /Subtype /Link /Rect [500 500 600 600] /A << /S /URI /URI (https://fuera) >> >>",
        ]);
        assert!(de_la_hoja(&b, 0).is_empty());
    }

    #[test]
    fn en_una_hoja_girada_la_caja_gira_con_ella() {
        let b = pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 100 200] >>",
            "<< /Type /Page /Parent 2 0 R /Rotate 90 /Annots [<< /Subtype /Link /Rect [0 0 10 20] /A << /S /URI /URI (https://x) >> >>] >>",
        ]);
        let e = de_la_hoja(&b, 0);
        assert_eq!(e.len(), 1);
        // Abajo a la izquierda sin girar es arriba a la izquierda girada 90.
        let c = e[0].caja;
        assert!(c[0] < 0.01 && c[1] < 0.01, "{c:?}");
        assert!(
            (c[2] - 0.1).abs() < 1e-6 && (c[3] - 0.1).abs() < 1e-6,
            "{c:?}"
        );
    }
}
