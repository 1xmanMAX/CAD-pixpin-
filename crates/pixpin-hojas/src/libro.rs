//! El libro: lo que se lee al abrir (lista de hojas, estilos, tema, textos
//! compartidos) y lo que se necesita para ensenar una celda.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::ErrorHojas;
use crate::estilos::{Estilos, Fuente, Horizontal, Rgb, Tema, Xf};
use crate::formato::{Mostrado, Regional};
use crate::hoja::{Celda, ERRORES, Hoja, LectorHoja, Valor};
use crate::paquete::Paquete;
use crate::textos::{Textos, TramoRico};
use crate::xml;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibilidad {
    Visible,
    Oculta,
    MuyOculta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoHoja {
    Hoja,
    /// Una hoja que es solo un grafico (`chartsheet`): se ensena vacia.
    Grafico,
    Otra,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InfoHoja {
    pub nombre: String,
    /// Ruta de su XML dentro del ZIP.
    pub ruta: String,
    pub visibilidad: Visibilidad,
    pub tipo: TipoHoja,
}

/// Cuanto tardo cada paso de abrir (milisegundos), para medir.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tiempos {
    pub zip: f64,
    pub libro: f64,
    pub estilos: f64,
    pub textos: f64,
}

/// Que clase de valor tiene la celda: decide la alineacion por defecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    Numero,
    Texto,
    Logico,
    Error,
}

/// Lo que se ensena en una celda.
#[derive(Debug, Clone, PartialEq)]
pub struct Contenido {
    pub mostrado: Mostrado,
    pub clase: Clase,
    /// Alineacion efectiva (la `General` ya resuelta segun la clase).
    pub horizontal: Horizontal,
    /// Tramos con fuente propia (texto rico), si los hay.
    pub tramos: Option<Vec<TramoRico>>,
}

pub struct Libro {
    /// El ZIP del `.xlsx` (no hay en un `.xls`, que se lee entero al abrir).
    paquete: Option<Paquete>,
    /// Un `.xls` (se lee hoja a hoja al pedirla) y las hojas ya leidas.
    xls: Option<crate::xls::XlsAbierto>,
    precargadas: std::sync::Mutex<std::collections::HashMap<usize, Hoja>>,
    pub hojas: Vec<InfoHoja>,
    pub estilos: Estilos,
    pub textos: Textos,
    pub tema: Arc<Tema>,
    pub fecha_1904: bool,
    pub regional: Regional,
    /// La hoja que estaba delante al guardar.
    pub hoja_activa: usize,
    pub tiempos: Tiempos,
    tam_por_xf: Arc<[f32]>,
    paleta: Arc<[u32]>,
    /// Lo que los minigraficos necesitan de otras hojas y los limites de
    /// eje de cada grupo: se calculan una vez y se guardan.
    mini: std::sync::Mutex<CacheMini>,
}

#[derive(Default)]
struct CacheMini {
    /// Otras hojas abiertas, leidas hasta la fila que hizo falta.
    externas: std::collections::HashMap<usize, Hoja>,
    /// (hoja, grupo) -> (minimo, maximo) de todo el grupo.
    limites: std::collections::HashMap<(String, u32), Option<(f64, f64)>>,
}

impl Libro {
    pub fn abrir(ruta: &Path, regional: Regional) -> Result<Libro, ErrorHojas> {
        let bytes = std::fs::read(ruta).map_err(|e| ErrorHojas::Io(e.to_string()))?;
        Self::desde_bytes(bytes, regional)
    }

    pub fn desde_bytes(bytes: Vec<u8>, regional: Regional) -> Result<Libro, ErrorHojas> {
        let mut tiempos = Tiempos::default();
        let t0 = Instant::now();
        if crate::cfb::es_cfb(&bytes) {
            return Self::desde_xls(&bytes, regional);
        }
        let paquete = Paquete::nuevo(Arc::from(bytes))?;
        tiempos.zip = ms(t0);
        let t0 = Instant::now();
        // El libro: lo dice `_rels/.rels`, o el sitio de siempre.
        let ruta_libro = paquete
            .relaciones("")
            .into_iter()
            .find(|r| r.tipo == "officeDocument" && !r.externa)
            .map(|r| r.destino)
            .filter(|d| paquete.existe(d))
            .unwrap_or_else(|| "xl/workbook.xml".into());
        if ruta_libro.ends_with(".bin") {
            return Err(ErrorHojas::Binario);
        }
        let bytes_libro = paquete.leer(&ruta_libro).ok_or(ErrorHojas::SinLibro)?;
        let raiz = xml::arbol(&bytes_libro);
        let wb = raiz.hijo("workbook").ok_or(ErrorHojas::SinLibro)?;
        let rels = paquete.relaciones(&ruta_libro);
        let fecha_1904 = wb
            .hijo("workbookPr")
            .and_then(|p| p.attr("date1904"))
            .is_some_and(|v| v == "1" || v == "true");
        let mut hojas = Vec::new();
        if let Some(sh) = wb.hijo("sheets") {
            for s in sh.hijos_de("sheet") {
                let Some(r) = s.attr("id").and_then(|id| rels.iter().find(|r| r.id == id)) else {
                    continue;
                };
                let visibilidad = match s.attr("state") {
                    Some("hidden") => Visibilidad::Oculta,
                    Some("veryHidden") => Visibilidad::MuyOculta,
                    _ => Visibilidad::Visible,
                };
                let tipo = match r.tipo.as_str() {
                    "worksheet" => TipoHoja::Hoja,
                    "chartsheet" => TipoHoja::Grafico,
                    _ => TipoHoja::Otra,
                };
                hojas.push(InfoHoja {
                    nombre: s.attr("name").unwrap_or("Hoja").to_string(),
                    ruta: r.destino.clone(),
                    visibilidad,
                    tipo,
                });
            }
        }
        let activa = wb
            .hijo("bookViews")
            .and_then(|b| b.hijo("workbookView"))
            .and_then(|v| v.attr("activeTab"))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let hoja_activa = if hojas
            .get(activa)
            .is_some_and(|h| h.visibilidad == Visibilidad::Visible)
        {
            activa
        } else {
            hojas
                .iter()
                .position(|h| h.visibilidad == Visibilidad::Visible)
                .unwrap_or(0)
        };
        tiempos.libro = ms(t0);
        let t0 = Instant::now();
        let ruta_de = |tipo: &str| {
            rels.iter()
                .find(|r| r.tipo == tipo && !r.externa)
                .map(|r| r.destino.clone())
        };
        let tema = ruta_de("theme")
            .and_then(|r| paquete.leer(&r))
            .map(|b| Tema::leer(&b))
            .unwrap_or_default();
        let estilos = match ruta_de("styles").and_then(|r| paquete.leer(&r)) {
            Some(b) => Estilos::leer(&b, &tema, &regional),
            None => Estilos::por_defecto(&tema, &regional),
        };
        tiempos.estilos = ms(t0);
        let t0 = Instant::now();
        let textos = ruta_de("sharedStrings")
            .and_then(|r| paquete.leer(&r))
            .map(Textos::leer)
            .unwrap_or_default();
        tiempos.textos = ms(t0);
        let tam_por_xf: Arc<[f32]> = Arc::from(estilos.tam_por_xf.clone());
        let paleta: Arc<[u32]> = Arc::from(estilos.paleta.clone());
        Ok(Libro {
            paquete: Some(paquete),
            xls: None,
            precargadas: Default::default(),
            hojas,
            estilos,
            textos,
            tema: Arc::new(tema),
            fecha_1904,
            regional,
            hoja_activa,
            tiempos,
            tam_por_xf,
            paleta,
            mini: Default::default(),
        })
    }

    fn valores_con(
        &self,
        c: &mut CacheMini,
        hoja: &Hoja,
        m: &crate::minigraficos::Minigrafico,
    ) -> Vec<Option<f64>> {
        use crate::minigraficos::valores_de;
        let r = m.origen.rango;
        let Some(nombre) = m
            .origen
            .hoja
            .as_deref()
            .filter(|n| !n.eq_ignore_ascii_case(&hoja.nombre))
        else {
            return valores_de(hoja, r);
        };
        let Some(i) = self
            .hojas
            .iter()
            .position(|h| h.nombre.eq_ignore_ascii_case(nombre) && h.tipo == TipoHoja::Hoja)
        else {
            return Vec::new();
        };
        let otra = c.externas.entry(i).or_insert_with(|| self.abrir_hoja(i));
        // Solo se lee hasta la fila que hace falta (con un tope).
        otra.cargar_hasta_fila(
            r.fila1.min(
                r.fila0
                    .saturating_add(crate::minigraficos::MAX_PUNTOS as u32),
            ),
        );
        valores_de(otra, r)
    }

    /// Los valores de un minigrafico (sin resolver vacias). Si los datos
    /// estan en otra hoja, se abre esa hoja una vez y se guarda.
    pub fn valores_mini(
        &self,
        hoja: &Hoja,
        m: &crate::minigraficos::Minigrafico,
    ) -> Vec<Option<f64>> {
        let Ok(mut c) = self.mini.lock() else {
            return Vec::new();
        };
        self.valores_con(&mut c, hoja, m)
    }

    /// El eje vertical de un minigrafico, con los limites del grupo si los
    /// pide (calculados una sola vez por grupo y hoja).
    pub fn eje_mini(
        &self,
        hoja: &Hoja,
        m: &crate::minigraficos::Minigrafico,
        valores: &[Option<f64>],
    ) -> Option<(f64, f64)> {
        use crate::minigraficos::{LimiteEje, eje_vertical, extremos, puntos_de};
        let g = hoja.cola.grupos_mini.get(m.grupo as usize)?;
        let propios = extremos(valores);
        let de_grupo = if g.min_tipo == LimiteEje::Grupo || g.max_tipo == LimiteEje::Grupo {
            let Ok(mut c) = self.mini.lock() else {
                return eje_vertical(g, propios, None);
            };
            let clave = (hoja.nombre.clone(), m.grupo);
            match c.limites.get(&clave) {
                Some(l) => *l,
                // Con la hoja a medias, los datos pueden no estar todos: no se guarda.
                None if !hoja.completa => None,
                None => {
                    let mut e: Option<(f64, f64)> = None;
                    for otro in hoja.cola.minigraficos.iter().filter(|o| o.grupo == m.grupo) {
                        let v = self.valores_con(&mut c, hoja, otro);
                        if let Some((a, b)) = extremos(&puntos_de(g, &v)) {
                            e = Some(e.map_or((a, b), |(x, y)| (x.min(a), y.max(b))));
                        }
                    }
                    c.limites.insert(clave, e);
                    e
                }
            }
        } else {
            None
        };
        eje_vertical(g, propios, de_grupo)
    }

    /// Un `.xls` antiguo (BIFF8) sin Office ([`crate::xls`]): al abrir se
    /// leen los globales (estilos, textos, lista de hojas) y la hoja activa;
    /// cada hoja se lee cuando se pide y queda guardada.
    fn desde_xls(bytes: &[u8], regional: Regional) -> Result<Libro, ErrorHojas> {
        let mut tiempos = Tiempos::default();
        let t0 = Instant::now();
        let mut x = crate::xls::XlsAbierto::abrir(bytes)?;
        tiempos.zip = ms(t0);
        let t0 = Instant::now();
        let tema = Tema::default();
        let estilos = Estilos::leer(&x.estilos_xml, &tema, &regional);
        tiempos.estilos = ms(t0);
        let t0 = Instant::now();
        let textos = Textos::leer(std::mem::take(&mut x.textos_xml));
        tiempos.textos = ms(t0);
        let hojas: Vec<InfoHoja> = x
            .hojas()
            .into_iter()
            .map(|(nombre, visibilidad, tipo)| InfoHoja {
                nombre,
                ruta: String::new(),
                visibilidad,
                tipo,
            })
            .collect();
        let tam_por_xf: Arc<[f32]> = Arc::from(estilos.tam_por_xf.clone());
        let paleta: Arc<[u32]> = Arc::from(estilos.paleta.clone());
        let (fecha_1904, hoja_activa) = (x.fecha_1904(), x.activa);
        Ok(Libro {
            paquete: None,
            xls: Some(x),
            precargadas: Default::default(),
            hojas,
            estilos,
            textos,
            tema: Arc::new(tema),
            fecha_1904,
            regional,
            hoja_activa,
            tiempos,
            tam_por_xf,
            paleta,
            mini: Default::default(),
        })
    }

    /// Abre la hoja `i` y lee su cabecera (columnas, vista, alto por
    /// defecto). Las filas se piden despues: [`Hoja::cargar_hasta_fila`].
    pub fn abrir_hoja(&self, i: usize) -> Hoja {
        let Some(info) = self.hojas.get(i) else {
            return Hoja::nueva(String::new(), None);
        };
        // Un `.xls` se lee hoja a hoja la primera vez y se guarda; se entrega una copia.
        if let Some(x) = &self.xls {
            let Ok(mut guardadas) = self.precargadas.lock() else {
                return Hoja::nueva(info.nombre.clone(), None);
            };
            let h = guardadas.entry(i).or_insert_with(|| match x.hoja(i) {
                Ok(h) => Hoja::desde_filas(h.nombre, h.cabecera, h.cola, h.textos, h.filas),
                // Una hoja que no se entiende se ve vacia (la activa ya se comprobo al abrir).
                Err(_) => Hoja::desde_filas(
                    info.nombre.clone(),
                    Default::default(),
                    Default::default(),
                    Vec::new(),
                    Vec::new(),
                ),
            });
            return h.copia_entera();
        }
        let lector = if info.tipo == TipoHoja::Hoja && self.paquete.is_some() {
            let Some(paquete) = &self.paquete else {
                return Hoja::nueva(info.nombre.clone(), None);
            };
            LectorHoja::nuevo(
                paquete,
                &info.ruta,
                self.tam_por_xf.clone(),
                self.estilos.fuente_normal.tam,
                crate::rejilla::alto_por_fuente(&self.estilos.fuente_normal),
                self.tema.clone(),
                self.paleta.clone(),
            )
        } else {
            None
        };
        let mut h = Hoja::nueva(info.nombre.clone(), lector);
        // La primera tanda trae solo la cabecera.
        if let Some(l) = h.tomar_lector() {
            let mut l = l;
            let t = l.leer(0, 0);
            let fin = t.fin;
            h.recibir(t);
            if !fin {
                h.devolver_lector(l);
            }
        }
        h
    }

    /// Los bytes de una parte del ZIP (las imagenes de los dibujos).
    pub fn parte(&self, ruta: &str) -> Option<Vec<u8>> {
        self.paquete.as_ref()?.leer(ruta)
    }

    pub fn xf(&self, c: &Celda) -> &Xf {
        self.estilos.xf(c.xf)
    }

    pub fn fuente_de(&self, xf: &Xf) -> &Fuente {
        self.estilos.fuente(xf)
    }

    /// El texto que ensena la celda, con su color forzado y su alineacion.
    pub fn contenido(&self, hoja: &Hoja, c: &Celda) -> Option<Contenido> {
        let xf = self.estilos.xf(c.xf);
        let (mostrado, clase, tramos) = match c.valor {
            Valor::Vacio => return None,
            Valor::Numero(v) => (
                xf.formato.numero(v, self.fecha_1904, &self.regional),
                Clase::Numero,
                None,
            ),
            Valor::Compartido(i) => {
                let t = self.textos.texto(i);
                let tramos =
                    self.textos
                        .rico(i, self.estilos.fuente(xf), &self.tema, &self.estilos.paleta);
                (xf.formato.texto(&t), Clase::Texto, tramos)
            }
            Valor::Propio(i) => {
                let t = hoja
                    .textos
                    .get(i as usize)
                    .map(String::as_str)
                    .unwrap_or("");
                (xf.formato.texto(t), Clase::Texto, None)
            }
            Valor::Logico(b) => {
                let t = match (b, self.regional.espanol) {
                    (true, false) => "TRUE",
                    (false, false) => "FALSE",
                    (true, true) => "VERDADERO",
                    (false, true) => "FALSO",
                };
                (
                    Mostrado {
                        texto: t.into(),
                        ..Default::default()
                    },
                    Clase::Logico,
                    None,
                )
            }
            Valor::Error(e) => {
                let t = if self.regional.espanol {
                    nombre_error_es(e)
                } else {
                    ERRORES.get(e as usize).copied().unwrap_or("#VALUE!")
                };
                (
                    Mostrado {
                        texto: t.into(),
                        ..Default::default()
                    },
                    Clase::Error,
                    None,
                )
            }
        };
        let horizontal = match xf.alineacion.horizontal {
            Horizontal::General => match clase {
                Clase::Numero if !xf.comilla => Horizontal::Derecha,
                Clase::Logico | Clase::Error => Horizontal::Centro,
                _ => Horizontal::Izquierda,
            },
            h => h,
        };
        // Un texto rico con seccion de texto que anade cosas ya no casa con
        // sus tramos: se pinta plano.
        let tramos = tramos
            .filter(|t| t.iter().map(|x| x.texto.as_str()).collect::<String>() == mostrado.texto);
        Some(Contenido {
            mostrado,
            clase,
            horizontal,
            tramos,
        })
    }

    /// El estilo de una tabla: el propio del libro o uno integrado.
    pub fn estilo_de_tabla(&self, t: &crate::tablas::Tabla) -> Option<crate::estilos::EstiloTabla> {
        self.estilos
            .estilos_tabla
            .get(&t.estilo)
            .cloned()
            .or_else(|| crate::tablas::integrado(&t.estilo, &self.tema))
    }

    /// Lo que el formato condicional hace a una celda con valor (`None` si
    /// nada).
    pub fn efecto_condicional(
        &self,
        hoja: &Hoja,
        fila: u32,
        c: &Celda,
    ) -> Option<crate::condicional::Efecto> {
        if hoja.cola.condicionales.is_empty() {
            return None;
        }
        let mut ef = crate::condicional::Efecto::default();
        let mut alguno = false;
        let mut texto: Option<String> = None;
        for cf in &hoja.cola.condicionales {
            // Una celda sin valor solo importa a las reglas de «vacias».
            if (c.valor == Valor::Vacio && !cf.con_vacios) || !cf.aplica(fila, c.col) {
                continue;
            }
            alguno = true;
            // El texto solo se forma si alguna regla lo mira.
            let t = if cf.necesita_texto {
                texto
                    .get_or_insert_with(|| self.texto_crudo(hoja, c))
                    .as_str()
            } else {
                ""
            };
            if cf.evaluar(hoja, c, t, &self.estilos.dxfs, &mut ef) {
                break;
            }
        }
        (alguno && ef != crate::condicional::Efecto::default()).then_some(ef)
    }

    /// El texto de una celda sin formato (para copiar o buscar).
    pub fn texto_crudo(&self, hoja: &Hoja, c: &Celda) -> String {
        match c.valor {
            Valor::Vacio => String::new(),
            Valor::Numero(v) => crate::formato::general(v, &self.regional),
            Valor::Compartido(i) => self.textos.texto(i),
            Valor::Propio(i) => hoja.textos.get(i as usize).cloned().unwrap_or_default(),
            Valor::Logico(b) => if b { "TRUE" } else { "FALSE" }.into(),
            Valor::Error(e) => ERRORES.get(e as usize).copied().unwrap_or("#VALUE!").into(),
        }
    }

    /// El color con el que se pinta el texto de la celda (automatico = negro).
    pub fn color_texto(&self, xf: &Xf, mostrado: &Mostrado) -> Rgb {
        if let Some(c) = mostrado.color {
            return Rgb {
                r: c[0],
                g: c[1],
                b: c[2],
            };
        }
        self.estilos.fuente(xf).color.unwrap_or(Rgb::NEGRO)
    }
}

fn nombre_error_es(e: u8) -> &'static str {
    match e {
        0 => "#¡NULO!",
        1 => "#¡DIV/0!",
        2 => "#¡VALOR!",
        3 => "#¡REF!",
        4 => "#¿NOMBRE?",
        5 => "#¡NUM!",
        6 => "#N/D",
        7 => "#OBTENIENDO_DATOS",
        8 => "#¡DESBORDAMIENTO!",
        9 => "#¡CALC!",
        _ => "#¡CAMPO!",
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// El numero de serie (sistema 1900) de una fecha civil.
pub fn serie_de_fecha(y: i64, m: u32, d: u32) -> Option<f64> {
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Dias desde 1899-12-30, con el 29-feb-1900 falso de Excel.
    let dias = |y: i64, m: u32, d: u32| -> i64 {
        let y2 = if m <= 2 { y - 1 } else { y };
        let era = y2.div_euclid(400);
        let yoe = y2 - era * 400;
        let mp = (m as i64 + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d as i64 - 1;
        era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468
    };
    let n = dias(y, m, d) - dias(1899, 12, 30);
    Some(if n <= 60 { n - 1 } else { n } as f64)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn series_de_fecha() {
        assert_eq!(serie_de_fecha(1900, 1, 1), Some(1.0));
        assert_eq!(serie_de_fecha(1900, 3, 1), Some(61.0));
        assert_eq!(serie_de_fecha(2023, 3, 15), Some(45000.0));
        assert_eq!(serie_de_fecha(2023, 13, 1), None);
    }
}
