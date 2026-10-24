//! El ZIP del libro: leer partes enteras, leerlas de tiron y sus relaciones.

use std::io::{Cursor, Read};
use std::sync::Arc;

use zip::{CompressionMethod, ZipArchive};

use crate::ErrorHojas;
use crate::xml;

/// Tope de lo que se descomprime de una parte leida entera: una bomba de
/// ZIP no llena la memoria. La hoja grande no pasa por aqui (va de tiron).
const TOPE_PARTE: u64 = 512 * 1024 * 1024;

/// Los topes contra archivos danados o maliciosos. Generosos: un `.xlsx`
/// real de millones de celdas (cientos de MB de XML) cabe de sobra.
#[derive(Debug, Clone, Copy)]
pub struct Limites {
    /// Entradas del ZIP (un libro normal tiene decenas; con muchas imagenes, miles).
    pub entradas: usize,
    /// Lo que puede dar una parte leida entera (`leer`).
    pub parte: u64,
    /// Lo que puede dar una parte leida de tiron (la hoja grande).
    pub parte_en_flujo: u64,
    /// La suma de lo que dicen los tamanos declarados de todas las partes.
    pub total_declarado: u64,
}

impl Default for Limites {
    fn default() -> Self {
        Limites {
            entradas: 20_000,
            parte: TOPE_PARTE,
            parte_en_flujo: 4 * 1024 * 1024 * 1024,
            total_declarado: 16 * 1024 * 1024 * 1024,
        }
    }
}

/// Un lector que corta con error si de el salen mas de `resta` bytes: se
/// mide lo que sale de verdad, no lo que dice el ZIP que va a salir.
struct Tope<R> {
    dentro: R,
    resta: u64,
}

impl<R: Read> Read for Tope<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.dentro.read(buf)?;
        if n as u64 > self.resta {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "la parte descomprimida supera el limite de seguridad",
            ));
        }
        self.resta -= n as u64;
        Ok(n)
    }
}

/// El paquete abierto. Clonarlo es barato: comparte los bytes y el indice.
#[derive(Clone)]
pub struct Paquete {
    zip: ZipArchive<Cursor<Arc<[u8]>>>,
    datos: Arc<[u8]>,
    limites: Limites,
}

/// Una relacion de una parte (`_rels/*.rels`), con el destino ya resuelto a
/// una ruta dentro del ZIP (sin la barra inicial) salvo si es externa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relacion {
    pub id: String,
    /// El ultimo trozo del tipo: `worksheet`, `sharedStrings`, `image`...
    pub tipo: String,
    pub destino: String,
    pub externa: bool,
}

/// Un trozo de los bytes del fichero que se lee como `Read` y es dueno de lo
/// que lee (vive en otro hilo si hace falta).
struct Trozo {
    datos: Arc<[u8]>,
    pos: usize,
    fin: usize,
}

impl Read for Trozo {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(self.fin.saturating_sub(self.pos));
        buf[..n].copy_from_slice(&self.datos[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// El lado que lee de [`Paquete::lector_en_hilo`].
struct EnHilo {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    actual: Vec<u8>,
    pos: usize,
}

impl Read for EnHilo {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while self.pos >= self.actual.len() {
            match self.rx.recv() {
                Ok(v) => {
                    self.actual = v;
                    self.pos = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let n = buf.len().min(self.actual.len() - self.pos);
        buf[..n].copy_from_slice(&self.actual[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl Paquete {
    pub fn nuevo(datos: Arc<[u8]>) -> Result<Self, ErrorHojas> {
        Self::con_limites(datos, Limites::default())
    }

    pub fn con_limites(datos: Arc<[u8]>, limites: Limites) -> Result<Self, ErrorHojas> {
        let mut zip =
            ZipArchive::new(Cursor::new(datos.clone())).map_err(|_| ErrorHojas::NoEsZip)?;
        if zip.len() > limites.entradas {
            return Err(ErrorHojas::Limite(format!(
                "el ZIP tiene {} entradas (el maximo es {})",
                zip.len(),
                limites.entradas
            )));
        }
        let mut total = 0u64;
        for i in 0..zip.len() {
            if let Ok(f) = zip.by_index_raw(i) {
                total = total.saturating_add(f.size());
            }
        }
        if total > limites.total_declarado {
            return Err(ErrorHojas::Limite(format!(
                "las partes declaran {total} bytes descomprimidos (el maximo es {})",
                limites.total_declarado
            )));
        }
        Ok(Self {
            zip,
            datos,
            limites,
        })
    }

    fn indice(&self, ruta: &str) -> Option<usize> {
        let ruta = ruta.trim_start_matches('/');
        if let Some(i) = self.zip.index_for_name(ruta) {
            return Some(i);
        }
        // Hay generadores que cambian mayusculas o usan barras invertidas.
        let buscada = ruta.replace('\\', "/").to_ascii_lowercase();
        let nombre = self
            .zip
            .file_names()
            .find(|n| n.replace('\\', "/").to_ascii_lowercase() == buscada)?;
        self.zip.index_for_name(nombre)
    }

    pub fn existe(&self, ruta: &str) -> bool {
        self.indice(ruta).is_some()
    }

    /// La parte entera descomprimida, o `None` si no esta o no se puede leer.
    pub fn leer(&self, ruta: &str) -> Option<Vec<u8>> {
        let i = self.indice(ruta)?;
        let mut zip = self.zip.clone();
        let f = zip.by_index(i).ok()?;
        let tope = f.size().min(self.limites.parte);
        let mut v = Vec::with_capacity(tope.min(64 * 1024 * 1024) as usize);
        // Se lee de mas (uno) para notar que sale mas de lo que dice el ZIP.
        f.take(tope + 1).read_to_end(&mut v).ok()?;
        if v.len() as u64 > tope {
            return None;
        }
        Some(v)
    }

    /// La parte para leer de tiron mientras se descomprime: dueno de lo
    /// suyo, `Send`, sin cargar la parte entera.
    pub fn lector(&self, ruta: &str) -> Option<Box<dyn Read + Send>> {
        let i = self.indice(ruta)?;
        let mut zip = self.zip.clone();
        let f = zip.by_index_raw(i).ok()?;
        if f.encrypted() {
            return None;
        }
        let inicio = usize::try_from(f.data_start()).ok()?;
        let fin = inicio.checked_add(usize::try_from(f.compressed_size()).ok()?)?;
        if fin > self.datos.len() {
            return None;
        }
        let metodo = f.compression();
        drop(f);
        let trozo = Trozo {
            datos: self.datos.clone(),
            pos: inicio,
            fin,
        };
        match metodo {
            CompressionMethod::Stored => Some(Box::new(trozo)),
            CompressionMethod::Deflated => Some(Box::new(Tope {
                dentro: flate2::read::DeflateDecoder::new(trozo),
                resta: self.limites.parte_en_flujo,
            })),
            _ => None,
        }
    }

    /// Cuanto ocupa comprimida una parte.
    pub fn comprimido(&self, ruta: &str) -> Option<u64> {
        let i = self.indice(ruta)?;
        let mut zip = self.zip.clone();
        let f = zip.by_index_raw(i).ok()?;
        Some(f.compressed_size())
    }

    /// Como [`Paquete::lector`], pero descomprimiendo en otro hilo mientras
    /// quien lee va procesando: con una hoja grande, descomprimir y leer el
    /// XML cuestan parecido, y asi se solapan. El hilo acaba solo al llegar
    /// al final o cuando se suelta el lector.
    pub fn lector_en_hilo(&self, ruta: &str) -> Option<Box<dyn Read + Send>> {
        let mut fuente = self.lector(ruta)?;
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(8);
        std::thread::Builder::new()
            .name("pixpin-hojas-inflar".into())
            .spawn(move || {
                loop {
                    let mut v = vec![0u8; 1 << 20];
                    let mut n = 0;
                    while n < v.len() {
                        match fuente.read(&mut v[n..]) {
                            Ok(0) => break,
                            Ok(k) => n += k,
                            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                            Err(_) => break,
                        }
                    }
                    if n == 0 {
                        break;
                    }
                    v.truncate(n);
                    let corto = n < (1 << 20);
                    if tx.send(v).is_err() || corto {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Box::new(EnHilo {
            rx,
            actual: Vec::new(),
            pos: 0,
        }))
    }

    /// Las relaciones de una parte (`xl/workbook.xml` -> `xl/_rels/workbook.xml.rels`).
    pub fn relaciones(&self, parte: &str) -> Vec<Relacion> {
        let parte = parte.trim_start_matches('/');
        let (dir, nombre) = match parte.rfind('/') {
            Some(i) => (&parte[..i + 1], &parte[i + 1..]),
            None => ("", parte),
        };
        let ruta = format!("{dir}_rels/{nombre}.rels");
        let Some(bytes) = self.leer(&ruta) else {
            return Vec::new();
        };
        let raiz = xml::arbol(&bytes);
        let mut v = Vec::new();
        let mut rels = Vec::new();
        raiz.todos("Relationship", &mut rels);
        for r in rels {
            let externa = r
                .attr("TargetMode")
                .is_some_and(|m| m.eq_ignore_ascii_case("External"));
            let destino = r.attr("Target").unwrap_or_default();
            v.push(Relacion {
                id: r.attr("Id").unwrap_or_default().to_string(),
                tipo: r
                    .attr("Type")
                    .unwrap_or_default()
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_string(),
                destino: if externa {
                    destino.to_string()
                } else {
                    resolver(parte, destino)
                },
                externa,
            });
        }
        v
    }
}

/// Resuelve `destino` relativo a la carpeta de la parte `base`.
pub fn resolver(base: &str, destino: &str) -> String {
    let destino = destino.replace('\\', "/");
    let mut trozos: Vec<&str> = Vec::new();
    if !destino.starts_with('/') {
        if let Some(i) = base.rfind('/') {
            trozos.extend(base[..i].split('/').filter(|s| !s.is_empty()));
        }
    }
    for t in destino.split('/') {
        match t {
            "" | "." => {}
            ".." => {
                trozos.pop();
            }
            t => trozos.push(t),
        }
    }
    trozos.join("/")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn resuelve_rutas_relativas_y_absolutas() {
        assert_eq!(
            resolver("xl/workbook.xml", "worksheets/sheet1.xml"),
            "xl/worksheets/sheet1.xml"
        );
        assert_eq!(
            resolver("xl/workbook.xml", "/xl/worksheets/sheet1.xml"),
            "xl/worksheets/sheet1.xml"
        );
        assert_eq!(
            resolver("xl/drawings/drawing1.xml", "../media/image1.png"),
            "xl/media/image1.png"
        );
        assert_eq!(resolver("xl/a.xml", "../../../../x"), "x");
    }

    fn zip_de(partes: &[(&str, Vec<u8>)]) -> Vec<u8> {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let op = zip::write::SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated);
        for (n, d) in partes {
            w.start_file(*n, op).unwrap();
            w.write_all(d).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn un_libro_normal_abre_y_se_lee_entero_y_de_tiron() {
        let z = zip_de(&[("a.xml", b"<a/>".to_vec()), ("b.xml", vec![b'x'; 100_000])]);
        let p = Paquete::nuevo(Arc::from(z)).unwrap();
        assert_eq!(p.leer("a.xml").unwrap(), b"<a/>");
        let mut v = Vec::new();
        p.lector("b.xml").unwrap().read_to_end(&mut v).unwrap();
        assert_eq!(v.len(), 100_000);
    }

    #[test]
    fn demasiadas_entradas_se_rechazan_con_un_error_claro() {
        let partes: Vec<(String, Vec<u8>)> =
            (0..30).map(|i| (format!("p{i}.xml"), vec![1])).collect();
        let refs: Vec<(&str, Vec<u8>)> = partes
            .iter()
            .map(|(n, d)| (n.as_str(), d.clone()))
            .collect();
        let z: Arc<[u8]> = Arc::from(zip_de(&refs));
        let lim = Limites {
            entradas: 10,
            ..Limites::default()
        };
        let Err(ErrorHojas::Limite(m)) = Paquete::con_limites(z.clone(), lim) else {
            panic!("debia rechazarse")
        };
        assert!(m.contains("30 entradas"), "{m}");
        assert!(Paquete::nuevo(z).is_ok());
    }

    #[test]
    fn una_bomba_zip_se_corta_midiendo_lo_que_sale() {
        // 2 MB de ceros ocupan ~2 KB comprimidos.
        let z = zip_de(&[("bomba.xml", vec![0u8; 2 * 1024 * 1024])]);
        assert!(z.len() < 10_000);
        let lim = Limites {
            parte: 100_000,
            parte_en_flujo: 100_000,
            ..Limites::default()
        };
        let p = Paquete::con_limites(Arc::from(z.clone()), lim).unwrap();
        assert!(p.leer("bomba.xml").is_none());
        let mut v = Vec::new();
        let e = p
            .lector("bomba.xml")
            .unwrap()
            .read_to_end(&mut v)
            .unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
        assert!(v.len() <= 100_000);
        // El total declarado tambien se vigila al abrir.
        let lim = Limites {
            total_declarado: 1000,
            ..Limites::default()
        };
        assert!(matches!(
            Paquete::con_limites(Arc::from(z), lim),
            Err(ErrorHojas::Limite(_))
        ));
    }

    #[test]
    fn un_zip_que_miente_en_su_tamano_no_pasa() {
        let mut z = zip_de(&[("m.xml", vec![0u8; 1_000_000])]);
        // Tamano descomprimido declarado en el directorio central: 10 bytes.
        let firma = [0x50, 0x4b, 0x01, 0x02];
        let k = z.windows(4).position(|w| w == firma).unwrap();
        z[k + 24..k + 28].copy_from_slice(&10u32.to_le_bytes());
        let p = Paquete::nuevo(Arc::from(z)).unwrap();
        assert!(p.leer("m.xml").is_none());
    }
}
