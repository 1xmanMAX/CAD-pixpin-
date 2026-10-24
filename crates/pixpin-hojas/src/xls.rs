//! **Excel 97-2003 (`.xls`, BIFF8) sin Office.** Abre el contenedor
//! compuesto ([`crate::cfb`]), lee el flujo `Workbook` registro a registro
//! ([MS-XLS]) y lo deja en la misma forma que un `.xlsx`: los estilos como un
//! `styles.xml` ya hecho (fuentes, rellenos, bordes, formatos de numero y
//! alineacion, con la paleta del archivo), los textos compartidos como un
//! `sharedStrings.xml` y cada hoja como un [`Hoja`] entero. Asi el visor y el
//! pintor no distinguen de donde viene el libro.
//!
//! Se leen: valores (numeros, RK, textos, logicos, errores y el valor guardado
//! de las formulas), textos compartidos con `CONTINUE`, formatos de numero y
//! fechas (incluido el sistema 1904), estilos (`FONT`, `XF`, `PALETTE`),
//! celdas combinadas, anchos de columna, altos de fila, filas y columnas
//! ocultas, paneles inmovilizados, zoom, cuadricula, hipervinculos y varias
//! hojas. No se leen (se ven sin ellos): formulas como texto, formato
//! condicional, validacion, comentarios, graficos e imagenes.
//!
//! Todo con limites: cada lectura esta acotada y un registro mal formado
//! corta la hoja, no tumba el programa. Los libros cifrados (`FilePass`) y los
//! de Excel 5/95 (BIFF5) se rechazan con un error y el visor de siempre
//! (la vista previa de Office) queda de respaldo.
//!
//! Referencia: gridcraft (https://github.com/storytold/gridcraft),
//! `crates/xlsx/src/xls/`, MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team
//! and the Gridcraft contributors (el marco de registros y las cadenas con
//! `CONTINUE`); los estilos y las hojas, escritos de nuevo contra [MS-XLS].

use std::collections::BTreeMap;

use crate::ErrorHojas;
use crate::cfb::Cfb;
use crate::hoja::{
    Cabecera, Celda, Cola, Columnas, FilaNueva, Hipervinculo, Rango, Valor, VistaHoja,
};
use crate::libro::{TipoHoja, Visibilidad};

const MAX_CELDAS: usize = 5_000_000;
const MAX_CADENAS: usize = 4_000_000;
const MAX_TEXTO: usize = 256 * 1024 * 1024;
const MAX_HOJAS: usize = 4096;
const MAX_XFS: usize = 65_536;
const MAX_FUENTES: usize = 65_536;
const MAX_FORMATOS: usize = 65_536;
const MAX_COMBINADAS: usize = 100_000;
const MAX_ENLACES: usize = 100_000;
/// Registros leidos de todo el flujo: un presupuesto para todas las hojas,
/// que varias hojas apuntando al mismo sitio no lo multipliquen.
const MAX_REGISTROS: usize = 40_000_000;
const MAX_CHARS: usize = 1 << 20;
const MAX_PARTES: usize = 1 << 16;

const BOF: u16 = 0x0809;
const EOF: u16 = 0x000A;
const CONTINUE: u16 = 0x003C;
const BIFF8: u16 = 0x0600;

fn mal(m: impl Into<String>) -> ErrorHojas {
    ErrorHojas::Io(format!("xls: {}", m.into()))
}

// ------------------------------------------------------------ registros

/// Recorre los registros de un flujo.
struct Registros<'a> {
    flujo: &'a [u8],
    pos: usize,
}

impl<'a> Registros<'a> {
    fn nuevo(flujo: &'a [u8], pos: usize) -> Self {
        Registros { flujo, pos }
    }

    /// El siguiente registro (sin sus `CONTINUE`). `None` al acabar o si el
    /// fichero se corta a medias.
    fn siguiente(&mut self) -> Option<(u16, &'a [u8])> {
        let cab = self.flujo.get(self.pos..self.pos.checked_add(4)?)?;
        let id = u16::from_le_bytes([cab[0], cab[1]]);
        let largo = usize::from(u16::from_le_bytes([cab[2], cab[3]]));
        let ini = self.pos + 4;
        let datos = self.flujo.get(ini..ini.checked_add(largo)?)?;
        self.pos = ini + largo;
        Some((id, datos))
    }

    fn id_siguiente(&self) -> Option<u16> {
        let h = self.flujo.get(self.pos..self.pos.checked_add(2)?)?;
        Some(u16::from_le_bytes([h[0], h[1]]))
    }

    /// El registro `datos` y los `CONTINUE` que lo siguen, para las cadenas
    /// que se parten entre registros.
    fn con_continuaciones(&mut self, datos: &'a [u8]) -> Vec<&'a [u8]> {
        let mut partes = vec![datos];
        while self.id_siguiente() == Some(CONTINUE) && partes.len() < MAX_PARTES {
            match self.siguiente() {
                Some((_, mas)) => partes.push(mas),
                None => break,
            }
        }
        partes
    }
}

/// Lee un registro con sus partes (cruza los `CONTINUE` cuando hace falta).
struct Cursor<'a, 'b> {
    partes: &'b [&'a [u8]],
    parte: usize,
    pos: usize,
}

impl<'a, 'b> Cursor<'a, 'b> {
    fn nuevo(partes: &'b [&'a [u8]]) -> Self {
        Cursor {
            partes,
            parte: 0,
            pos: 0,
        }
    }

    /// Pasa a la siguiente parte si la actual se agoto. Devuelve si empezo una.
    fn asentar(&mut self) -> bool {
        let mut movido = false;
        while self
            .partes
            .get(self.parte)
            .is_some_and(|p| self.pos >= p.len())
            && self.parte + 1 < self.partes.len()
        {
            self.parte += 1;
            self.pos = 0;
            movido = true;
        }
        movido
    }

    fn u8(&mut self) -> Option<u8> {
        self.asentar();
        let b = self.partes.get(self.parte)?.get(self.pos).copied()?;
        self.pos += 1;
        Some(b)
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes([self.u8()?, self.u8()?]))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes([
            self.u8()?,
            self.u8()?,
            self.u8()?,
            self.u8()?,
        ]))
    }

    fn saltar(&mut self, n: usize) -> Option<()> {
        if n > self.quedan() {
            return None;
        }
        for _ in 0..n {
            self.u8()?;
        }
        Some(())
    }

    fn quedan(&self) -> usize {
        let aqui = self
            .partes
            .get(self.parte)
            .map_or(0, |p| p.len().saturating_sub(self.pos));
        aqui + self
            .partes
            .iter()
            .skip(self.parte + 1)
            .map(|p| p.len())
            .sum::<usize>()
    }

    /// `cch` caracteres, comprimidos (un byte) o UTF-16. Si pasan a un
    /// `CONTINUE`, esa parte empieza con una marca de compresion nueva
    /// ([MS-XLS] 2.5.293).
    fn caracteres(&mut self, cch: usize, mut alto: bool) -> Option<String> {
        if cch > MAX_CHARS {
            return None;
        }
        let mut unidades: Vec<u16> = Vec::with_capacity(cch.min(self.quedan()));
        while unidades.len() < cch {
            let al_final = self
                .partes
                .get(self.parte)
                .is_none_or(|p| self.pos >= p.len());
            if al_final && self.asentar() {
                alto = self.u8()? & 1 != 0;
            }
            unidades.push(if alto {
                self.u16()?
            } else {
                u16::from(self.u8()?)
            });
        }
        Some(String::from_utf16_lossy(&unidades))
    }

    /// ShortXLUnicodeString: cuenta de 1 byte, marcas, caracteres.
    fn cadena_corta(&mut self) -> Option<String> {
        let cch = usize::from(self.u8()?);
        let alto = self.u8()? & 1 != 0;
        self.caracteres(cch, alto)
    }

    /// XLUnicodeString: cuenta de 2 bytes, marcas, caracteres.
    fn cadena(&mut self) -> Option<String> {
        let cch = usize::from(self.u16()?);
        let alto = self.u8()? & 1 != 0;
        self.caracteres(cch, alto)
    }

    /// XLUnicodeRichExtendedString: el texto; los tramos de formato y el
    /// fonetico se saltan.
    fn cadena_rica(&mut self) -> Option<String> {
        let cch = usize::from(self.u16()?);
        let marcas = self.u8()?;
        let tramos = if marcas & 0x08 != 0 {
            usize::from(self.u16()?)
        } else {
            0
        };
        let ext = if marcas & 0x04 != 0 {
            self.u32()? as usize
        } else {
            0
        };
        let texto = self.caracteres(cch, marcas & 1 != 0)?;
        self.saltar(tramos.saturating_mul(4).saturating_add(ext))?;
        Some(texto)
    }
}

// ------------------------------------------------------------ globales

#[derive(Debug, Clone, Default)]
struct FuenteB {
    altura: u16,
    marcas: u16,
    color: u16,
    peso: u16,
    escrito: u16,
    subrayado: u8,
    nombre: String,
}

#[derive(Debug, Clone, Copy, Default)]
struct XfB {
    fuente: u16,
    formato: u16,
    horizontal: u8,
    vertical: u8,
    ajustar: bool,
    giro: u8,
    sangria: u8,
    encoger: bool,
    borde: [u8; 4],
    color_borde: [u8; 4],
    diagonal: u8,
    estilo_diagonal: u8,
    color_diagonal: u8,
    trama: u8,
    frente: u8,
    fondo: u8,
}

impl XfB {
    /// Si el estilo pinta algo por si solo (relleno o borde): una celda
    /// vacia con un estilo asi existe; con otro, no hace falta guardarla.
    fn se_ve(&self) -> bool {
        self.trama != 0 || self.borde.iter().any(|&b| b != 0) || self.estilo_diagonal != 0
    }
}

struct Hoja8 {
    pos: usize,
    nombre: String,
    visibilidad: Visibilidad,
    tipo: u8,
}

#[derive(Default)]
struct Globales {
    hojas: Vec<Hoja8>,
    fecha_1904: bool,
    fuentes: Vec<FuenteB>,
    formatos: Vec<(u16, String)>,
    xfs: Vec<XfB>,
    paleta: Option<Vec<[u8; 3]>>,
    cadenas: Vec<String>,
    activa: usize,
}

#[derive(Default)]
struct Presupuesto {
    celdas: usize,
    texto: usize,
    registros: usize,
}

impl Presupuesto {
    fn registro(&mut self) -> Result<(), ErrorHojas> {
        self.registros += 1;
        if self.registros > MAX_REGISTROS {
            return Err(ErrorHojas::Limite("xls: demasiados registros".into()));
        }
        Ok(())
    }

    fn celda(&mut self) -> Result<(), ErrorHojas> {
        self.celdas += 1;
        if self.celdas > MAX_CELDAS {
            return Err(ErrorHojas::Limite(
                "xls: mas de cinco millones de celdas".into(),
            ));
        }
        Ok(())
    }

    fn texto(&mut self, n: usize) -> Result<(), ErrorHojas> {
        self.texto = self.texto.saturating_add(n);
        if self.texto > MAX_TEXTO {
            return Err(ErrorHojas::Limite("xls: mas de 256 MB de texto".into()));
        }
        Ok(())
    }
}

fn bof(id: u16, datos: &[u8]) -> Result<(u16, u16), ErrorHojas> {
    if id != BOF || datos.len() < 4 {
        return Err(mal("un subflujo no empieza con BOF"));
    }
    Ok((
        u16::from_le_bytes([datos[0], datos[1]]),
        u16::from_le_bytes([datos[2], datos[3]]),
    ))
}

fn globales(flujo: &[u8], pres: &mut Presupuesto) -> Result<Globales, ErrorHojas> {
    let mut regs = Registros::nuevo(flujo, 0);
    let (id, datos) = regs.siguiente().ok_or_else(|| mal("flujo vacio"))?;
    let (version, tipo) = bof(id, datos)?;
    if version != BIFF8 {
        return Err(mal(if version == 0x0500 {
            "libro de Excel 5.0/95 (BIFF5): no se lee"
        } else {
            "version de BIFF no admitida (solo Excel 97-2003)"
        }));
    }
    if tipo != 0x0005 {
        return Err(mal("el flujo no empieza con los globales del libro"));
    }
    let mut g = Globales::default();
    let mut posiciones = std::collections::HashSet::new();
    while let Some((id, datos)) = regs.siguiente() {
        pres.registro()?;
        match id {
            EOF => break,
            0x002F => return Err(mal("libro protegido con contraseña")),
            0x0085 => {
                if g.hojas.len() >= MAX_HOJAS {
                    return Err(ErrorHojas::Limite("xls: demasiadas hojas".into()));
                }
                let partes = [datos];
                let mut c = Cursor::nuevo(&partes);
                let pos = c.u32().ok_or_else(|| mal("BOUNDSHEET cortado"))? as usize;
                if !posiciones.insert(pos) {
                    return Err(mal("dos hojas apuntan al mismo subflujo"));
                }
                let estado = c.u8().ok_or_else(|| mal("BOUNDSHEET cortado"))?;
                let tipo = c.u8().ok_or_else(|| mal("BOUNDSHEET cortado"))?;
                let nombre = c
                    .cadena_corta()
                    .ok_or_else(|| mal("nombre de hoja cortado"))?;
                let visibilidad = match estado & 3 {
                    0 => Visibilidad::Visible,
                    1 => Visibilidad::Oculta,
                    _ => Visibilidad::MuyOculta,
                };
                g.hojas.push(Hoja8 {
                    pos,
                    nombre,
                    visibilidad,
                    tipo,
                });
            }
            0x0022 => {
                g.fecha_1904 = datos
                    .get(..2)
                    .is_some_and(|d| u16::from_le_bytes([d[0], d[1]]) == 1)
            }
            0x003D => {
                g.activa = datos
                    .get(10..12)
                    .map_or(0, |d| usize::from(u16::from_le_bytes([d[0], d[1]])))
            }
            0x0031 | 0x0231 => {
                if g.fuentes.len() < MAX_FUENTES {
                    let partes = [datos];
                    let mut c = Cursor::nuevo(&partes);
                    if let Some(f) = fuente(&mut c) {
                        g.fuentes.push(f);
                    }
                }
            }
            0x041E => {
                let partes = regs.con_continuaciones(datos);
                let mut c = Cursor::nuevo(&partes);
                if let (Some(ifmt), Some(codigo)) = (c.u16(), c.cadena()) {
                    pres.texto(codigo.len())?;
                    if g.formatos.len() < MAX_FORMATOS {
                        g.formatos.push((ifmt, codigo));
                    }
                }
            }
            0x00E0 => {
                if g.xfs.len() < MAX_XFS {
                    g.xfs.push(xf(datos));
                }
            }
            0x0092 => g.paleta = paleta(datos),
            0x00FC => {
                let partes = regs.con_continuaciones(datos);
                g.cadenas = cadenas_compartidas(&partes, pres)?;
            }
            _ => {}
        }
    }
    Ok(g)
}

fn fuente(c: &mut Cursor<'_, '_>) -> Option<FuenteB> {
    let altura = c.u16()?;
    let marcas = c.u16()?;
    let color = c.u16()?;
    let peso = c.u16()?;
    let escrito = c.u16()?;
    let subrayado = c.u8()?;
    c.saltar(3)?;
    let nombre = c.cadena_corta().unwrap_or_default();
    Some(FuenteB {
        altura,
        marcas,
        color,
        peso,
        escrito,
        subrayado,
        nombre,
    })
}

fn xf(d: &[u8]) -> XfB {
    let u16_en = |i: usize| {
        d.get(i..i + 2)
            .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
    };
    let u32_en = |i: usize| {
        d.get(i..i + 4)
            .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let alin = d.get(6).copied().unwrap_or(0);
    let sangria_byte = d.get(8).copied().unwrap_or(0);
    let b1 = u32_en(10);
    let b2 = u32_en(14);
    let colores = u16_en(18);
    XfB {
        fuente: u16_en(0),
        formato: u16_en(2),
        horizontal: alin & 7,
        ajustar: alin & 8 != 0,
        vertical: (alin >> 4) & 7,
        giro: d.get(7).copied().unwrap_or(0),
        sangria: sangria_byte & 0x0F,
        encoger: sangria_byte & 0x10 != 0,
        borde: [
            (b1 & 0xF) as u8,
            ((b1 >> 4) & 0xF) as u8,
            ((b1 >> 8) & 0xF) as u8,
            ((b1 >> 12) & 0xF) as u8,
        ],
        color_borde: [
            ((b1 >> 16) & 0x7F) as u8,
            ((b1 >> 23) & 0x7F) as u8,
            (b2 & 0x7F) as u8,
            ((b2 >> 7) & 0x7F) as u8,
        ],
        diagonal: ((b1 >> 30) & 3) as u8,
        color_diagonal: ((b2 >> 14) & 0x7F) as u8,
        estilo_diagonal: ((b2 >> 21) & 0xF) as u8,
        trama: ((b2 >> 26) & 0x3F) as u8,
        frente: (colores & 0x7F) as u8,
        fondo: ((colores >> 7) & 0x7F) as u8,
    }
}

fn paleta(d: &[u8]) -> Option<Vec<[u8; 3]>> {
    let n = usize::from(u16::from_le_bytes([*d.first()?, *d.get(1)?]));
    let v: Vec<[u8; 3]> = d
        .get(2..)?
        .chunks_exact(4)
        .take(n.min(56))
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    (!v.is_empty()).then_some(v)
}

fn cadenas_compartidas(
    partes: &[&[u8]],
    pres: &mut Presupuesto,
) -> Result<Vec<String>, ErrorHojas> {
    let mut c = Cursor::nuevo(partes);
    let _total = c.u32().ok_or_else(|| mal("SST cortado"))?;
    let unicas = c.u32().ok_or_else(|| mal("SST cortado"))? as usize;
    if unicas > MAX_CADENAS {
        return Err(ErrorHojas::Limite(
            "xls: demasiados textos compartidos".into(),
        ));
    }
    // Cada texto ocupa al menos tres bytes: la cuenta no puede pasar de lo que hay.
    let mut v = Vec::with_capacity(unicas.min(c.quedan() / 3));
    for _ in 0..unicas {
        if c.quedan() == 0 {
            break;
        }
        // Un texto roto corta la lista: lo ya leido se queda.
        let Some(s) = c.cadena_rica() else { break };
        pres.texto(s.len())?;
        v.push(s);
    }
    Ok(v)
}

// ---------------------------------------------------------------- hojas

/// Una hoja ya leida, lista para armarla con [`crate::Hoja::desde_filas`].
pub struct HojaXls {
    pub nombre: String,
    pub visibilidad: Visibilidad,
    pub tipo: TipoHoja,
    pub cabecera: Cabecera,
    pub cola: Cola,
    pub textos: Vec<String>,
    pub filas: Vec<FilaNueva>,
}

/// Un libro `.xls` leido.
pub struct LibroXls {
    pub estilos_xml: Vec<u8>,
    pub textos_xml: Vec<u8>,
    pub fecha_1904: bool,
    pub hojas: Vec<HojaXls>,
    pub activa: usize,
}

/// Los errores de celda de BIFF a su indice en [`crate::hoja::ERRORES`].
fn error_de(codigo: u8) -> u8 {
    match codigo {
        0x00 => 0,
        0x07 => 1,
        0x0F => 2,
        0x17 => 3,
        0x1D => 4,
        0x24 => 5,
        0x2A => 6,
        _ => 2,
    }
}

fn rk(v: u32) -> f64 {
    let n = if v & 2 != 0 {
        f64::from((v as i32) >> 2)
    } else {
        f64::from_bits(u64::from(v & 0xFFFF_FFFC) << 32)
    };
    if v & 1 != 0 { n / 100.0 } else { n }
}

#[derive(Default)]
struct FilaTmp {
    alto: f32,
    oculta: bool,
    xf: Option<u32>,
    celdas: Vec<Celda>,
}

struct Estado {
    filas: BTreeMap<u32, FilaTmp>,
    textos: Vec<String>,
    cabecera: Cabecera,
    cola: Cola,
    zoom: u32,
    congelada: bool,
    division: (u32, u32),
    visible: Vec<bool>,
}

impl Estado {
    fn poner(
        &mut self,
        fila: u32,
        col: u32,
        xf: u32,
        valor: Valor,
        pres: &mut Presupuesto,
    ) -> Result<(), ErrorHojas> {
        // Una celda vacia solo se guarda si su estilo pinta algo.
        if valor == Valor::Vacio && !self.visible.get(xf as usize).copied().unwrap_or(false) {
            return Ok(());
        }
        if fila >= 1_048_576 || col >= 16_384 {
            return Ok(());
        }
        pres.celda()?;
        let f = self.filas.entry(fila).or_insert_with(|| FilaTmp {
            alto: f32::NAN,
            ..Default::default()
        });
        f.celdas.push(Celda { col, xf, valor });
        Ok(())
    }

    fn texto_propio(&mut self, s: String, pres: &mut Presupuesto) -> Result<Valor, ErrorHojas> {
        pres.texto(s.len())?;
        self.textos.push(s);
        Ok(Valor::Propio(self.textos.len() as u32 - 1))
    }
}

fn leer_hoja(
    flujo: &[u8],
    h: &Hoja8,
    g: &Globales,
    pres: &mut Presupuesto,
) -> Result<HojaXls, ErrorHojas> {
    let tipo = if h.tipo == 0 {
        TipoHoja::Hoja
    } else {
        TipoHoja::Grafico
    };
    let vacia = |cabecera| HojaXls {
        nombre: h.nombre.clone(),
        visibilidad: h.visibilidad,
        tipo,
        cabecera,
        cola: Cola::default(),
        textos: Vec::new(),
        filas: Vec::new(),
    };
    if h.tipo != 0 {
        return Ok(vacia(Cabecera::default()));
    }
    let mut regs = Registros::nuevo(flujo, h.pos);
    let Some((id, datos)) = regs.siguiente() else {
        return Err(mal("la hoja apunta fuera del flujo"));
    };
    let (version, dt) = bof(id, datos)?;
    if version != BIFF8 || dt != 0x0010 {
        // No es una hoja de calculo: se deja vacia.
        return Ok(vacia(Cabecera::default()));
    }
    let visible: Vec<bool> = g.xfs.iter().map(|x| x.se_ve()).collect();
    let mut e = Estado {
        filas: BTreeMap::new(),
        textos: Vec::new(),
        cabecera: Cabecera::default(),
        cola: Cola::default(),
        zoom: 100,
        congelada: false,
        division: (0, 0),
        visible,
    };
    let mut profundidad = 0u32;
    let mut formula_pendiente: Option<(u32, u32, u32)> = None;
    while let Some((id, datos)) = regs.siguiente() {
        pres.registro()?;
        let u16_en = |i: usize| {
            datos
                .get(i..i + 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
        };
        let u32_en = |i: usize| {
            datos
                .get(i..i + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        // El valor de texto de una formula viene en el STRING que le sigue.
        if id != 0x0207 {
            formula_pendiente = formula_pendiente.filter(|_| id == CONTINUE);
        }
        match id {
            BOF => profundidad += 1,
            EOF => {
                if profundidad == 0 {
                    break;
                }
                profundidad -= 1;
            }
            _ if profundidad > 0 => {}
            0x0200 => {
                if let (Some(f0), Some(f1), Some(c0), Some(c1)) =
                    (u32_en(0), u32_en(4), u16_en(8), u16_en(10))
                {
                    if f1 > f0 && c1 > c0 {
                        e.cabecera.dimension = Some(Rango {
                            fila0: f0,
                            col0: u32::from(c0),
                            fila1: (f1 - 1).min(1_048_575),
                            col1: u32::from(c1 - 1),
                        });
                    }
                }
            }
            0x0208 => {
                // ROW: fila, alto, marcas y estilo.
                let (Some(fila), Some(alto), Some(marcas)) = (u16_en(0), u16_en(6), u16_en(12))
                else {
                    continue;
                };
                let ghost = marcas & 0x80 != 0;
                let ixfe = u16_en(14).map(|x| u32::from(x & 0x0FFF));
                let f = e.filas.entry(u32::from(fila)).or_insert_with(|| FilaTmp {
                    alto: f32::NAN,
                    ..Default::default()
                });
                // El bit alto dice «alto por defecto».
                if alto & 0x8000 == 0 {
                    f.alto = f32::from(alto) / 20.0;
                }
                f.oculta = marcas & 0x20 != 0;
                if ghost {
                    f.xf = ixfe.filter(|&x| x != 0 && x != 15);
                }
            }
            0x0203 => {
                // NUMBER
                if let (Some(r), Some(c), Some(x), Some(v)) =
                    (u16_en(0), u16_en(2), u16_en(4), datos.get(6..14))
                {
                    let n = f64::from_le_bytes(v.try_into().unwrap_or([0; 8]));
                    let valor = if n.is_finite() {
                        Valor::Numero(n)
                    } else {
                        Valor::Vacio
                    };
                    e.poner(u32::from(r), u32::from(c), u32::from(x), valor, pres)?;
                }
            }
            0x027E => {
                if let (Some(r), Some(c), Some(x), Some(v)) =
                    (u16_en(0), u16_en(2), u16_en(4), u32_en(6))
                {
                    e.poner(
                        u32::from(r),
                        u32::from(c),
                        u32::from(x),
                        Valor::Numero(rk(v)),
                        pres,
                    )?;
                }
            }
            0x00BD => {
                // MULRK: fila, primera columna, (xf, rk)*, ultima columna.
                if let (Some(r), Some(c0)) = (u16_en(0), u16_en(2)) {
                    let cuerpo = datos.get(4..datos.len().saturating_sub(2)).unwrap_or(&[]);
                    for (i, t) in cuerpo.chunks_exact(6).enumerate() {
                        let x = u16::from_le_bytes([t[0], t[1]]);
                        let v = u32::from_le_bytes([t[2], t[3], t[4], t[5]]);
                        e.poner(
                            u32::from(r),
                            u32::from(c0) + i as u32,
                            u32::from(x),
                            Valor::Numero(rk(v)),
                            pres,
                        )?;
                    }
                }
            }
            0x00FD => {
                if let (Some(r), Some(c), Some(x), Some(i)) =
                    (u16_en(0), u16_en(2), u16_en(4), u32_en(6))
                {
                    e.poner(
                        u32::from(r),
                        u32::from(c),
                        u32::from(x),
                        Valor::Compartido(i),
                        pres,
                    )?;
                }
            }
            0x0204 | 0x00D6 => {
                // LABEL y RSTRING (el texto va tras fila, columna y estilo).
                let partes = regs.con_continuaciones(datos);
                let mut cur = Cursor::nuevo(&partes);
                if let (Some(r), Some(c), Some(x), Some(s)) =
                    (cur.u16(), cur.u16(), cur.u16(), cur.cadena())
                {
                    let v = e.texto_propio(s, pres)?;
                    e.poner(u32::from(r), u32::from(c), u32::from(x), v, pres)?;
                }
            }
            0x0205 => {
                if let (Some(r), Some(c), Some(x), Some(v), Some(es_error)) =
                    (u16_en(0), u16_en(2), u16_en(4), datos.get(6), datos.get(7))
                {
                    let valor = if *es_error != 0 {
                        Valor::Error(error_de(*v))
                    } else {
                        Valor::Logico(*v != 0)
                    };
                    e.poner(u32::from(r), u32::from(c), u32::from(x), valor, pres)?;
                }
            }
            0x0201 => {
                if let (Some(r), Some(c), Some(x)) = (u16_en(0), u16_en(2), u16_en(4)) {
                    e.poner(u32::from(r), u32::from(c), u32::from(x), Valor::Vacio, pres)?;
                }
            }
            0x00BE => {
                // MULBLANK: fila, primera columna, xf*, ultima columna.
                if let (Some(r), Some(c0)) = (u16_en(0), u16_en(2)) {
                    let cuerpo = datos.get(4..datos.len().saturating_sub(2)).unwrap_or(&[]);
                    for (i, t) in cuerpo.chunks_exact(2).enumerate() {
                        e.poner(
                            u32::from(r),
                            u32::from(c0) + i as u32,
                            u32::from(u16::from_le_bytes([t[0], t[1]])),
                            Valor::Vacio,
                            pres,
                        )?;
                    }
                }
            }
            0x0006 => {
                // FORMULA: solo se usa el valor guardado.
                let (Some(r), Some(c), Some(x), Some(res)) =
                    (u16_en(0), u16_en(2), u16_en(4), datos.get(6..14))
                else {
                    continue;
                };
                let (r, c, x) = (u32::from(r), u32::from(c), u32::from(x));
                if res[6] == 0xFF && res[7] == 0xFF {
                    match res[0] {
                        0 => formula_pendiente = Some((r, c, x)),
                        1 => e.poner(r, c, x, Valor::Logico(res[2] != 0), pres)?,
                        2 => e.poner(r, c, x, Valor::Error(error_de(res[2])), pres)?,
                        _ => {
                            let v = e.texto_propio(String::new(), pres)?;
                            e.poner(r, c, x, v, pres)?;
                        }
                    }
                } else {
                    let n = f64::from_le_bytes(res.try_into().unwrap_or([0; 8]));
                    e.poner(
                        r,
                        c,
                        x,
                        if n.is_finite() {
                            Valor::Numero(n)
                        } else {
                            Valor::Vacio
                        },
                        pres,
                    )?;
                }
            }
            0x0207 => {
                if let Some((r, c, x)) = formula_pendiente.take() {
                    let partes = regs.con_continuaciones(datos);
                    let mut cur = Cursor::nuevo(&partes);
                    if let Some(s) = cur.cadena() {
                        let v = e.texto_propio(s, pres)?;
                        e.poner(r, c, x, v, pres)?;
                    }
                }
            }
            0x007D => {
                // COLINFO: primera, ultima, ancho en 1/256, estilo, marcas.
                if let (Some(c0), Some(c1), Some(w), Some(x), Some(m)) =
                    (u16_en(0), u16_en(2), u16_en(4), u16_en(6), u16_en(8))
                {
                    if c0 <= c1 && e.cabecera.columnas.len() < 20_000 {
                        e.cabecera.columnas.push(Columnas {
                            min: u32::from(c0),
                            max: u32::from(c1).min(16_383),
                            ancho: Some(f64::from(w) / 256.0),
                            oculta: m & 1 != 0,
                            xf: u32::from(x),
                        });
                    }
                }
            }
            0x0055 => e.cabecera.ancho_base = u16_en(0).map(f64::from).filter(|v| *v > 0.0),
            0x0099 => {
                e.cabecera.ancho_defecto =
                    u16_en(0).map(|v| f64::from(v) / 256.0).filter(|v| *v > 0.0)
            }
            0x0225 => {
                e.cabecera.alto_defecto = u16_en(2)
                    .map(|v| f32::from(v & 0x7FFF) / 20.0)
                    .filter(|v| *v > 0.0)
            }
            0x00E5 => {
                if let Some(n) = u16_en(0) {
                    for t in datos
                        .get(2..)
                        .unwrap_or(&[])
                        .chunks_exact(8)
                        .take(usize::from(n))
                    {
                        if e.cola.combinadas.len() >= MAX_COMBINADAS {
                            break;
                        }
                        let v = |i: usize| u32::from(u16::from_le_bytes([t[i], t[i + 1]]));
                        let (f0, f1, c0, c1) = (v(0), v(2), v(4), v(6));
                        if f0 <= f1 && c0 <= c1 && (f0 != f1 || c0 != c1) {
                            e.cola.combinadas.push(Rango {
                                fila0: f0,
                                col0: c0,
                                fila1: f1,
                                col1: c1,
                            });
                        }
                    }
                }
            }
            0x023E => {
                // WINDOW2: cuadricula, encabezados, congelada, derecha a izquierda.
                if let (Some(m), Some(fila), Some(col)) = (u16_en(0), u16_en(2), u16_en(4)) {
                    let v = &mut e.cabecera.vista;
                    v.cuadricula = m & 0x0002 != 0;
                    v.encabezados = m & 0x0004 != 0;
                    v.derecha_a_izquierda = m & 0x0040 != 0;
                    e.congelada = m & 0x0008 != 0;
                    v.primera_fila = u32::from(fila);
                    v.primera_columna = u32::from(col);
                }
            }
            0x0041 => {
                // PANE: columnas y filas de la division, y la esquina de la zona que se mueve.
                if let (Some(x), Some(y), Some(fila), Some(col)) =
                    (u16_en(0), u16_en(2), u16_en(4), u16_en(6))
                {
                    e.division = (u32::from(x), u32::from(y));
                    e.cabecera.vista.primera_fila = u32::from(fila);
                    e.cabecera.vista.primera_columna = u32::from(col);
                }
            }
            0x00A0 => {
                if let (Some(n), Some(d)) = (u16_en(0), u16_en(2)) {
                    if d != 0 {
                        e.zoom = (u32::from(n) * 100 / u32::from(d)).clamp(10, 400);
                    }
                }
            }
            0x01B8 if e.cola.hipervinculos.len() < MAX_ENLACES => {
                if let Some(h) = enlace(datos) {
                    e.cola.hipervinculos.push(h);
                }
            }
            _ => {}
        }
    }
    let v: &mut VistaHoja = &mut e.cabecera.vista;
    v.zoom = e.zoom;
    if e.congelada {
        v.columnas_fijas = e.division.0.min(256);
        v.filas_fijas = e.division.1.min(65_536);
    } else {
        // Sin congelar, la division no inmoviliza nada.
        v.columnas_fijas = 0;
        v.filas_fijas = 0;
    }
    e.cola.combinadas.sort_by_key(|r| (r.fila0, r.col0));
    let mut filas = Vec::with_capacity(e.filas.len());
    for (indice, mut f) in e.filas {
        f.celdas.sort_by_key(|c| c.col);
        // La ultima gana si una celda sale dos veces.
        f.celdas.dedup_by(|b, a| a.col == b.col);
        filas.push(FilaNueva {
            indice,
            alto: f.alto,
            oculta: f.oculta,
            xf: f.xf,
            sin_alto: false,
            celdas: f.celdas,
        });
    }
    Ok(HojaXls {
        nombre: h.nombre.clone(),
        visibilidad: h.visibilidad,
        tipo,
        cabecera: e.cabecera,
        cola: e.cola,
        textos: e.textos,
        filas,
    })
}

/// HLINK ([MS-XLS] 2.4.140): el rango y a donde lleva.
fn enlace(d: &[u8]) -> Option<Hipervinculo> {
    let u16_en = |i: usize| d.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let (f0, f1, c0, c1) = (u16_en(0)?, u16_en(2)?, u16_en(4)?, u16_en(6)?);
    if f0 > f1 || c0 > c1 {
        return None;
    }
    let rango = Rango {
        fila0: u32::from(f0),
        col0: u32::from(c0),
        fila1: u32::from(f1),
        col1: u32::from(c1),
    };
    let mut p = 8 + 16 + 4;
    let marcas = u32::from_le_bytes(d.get(p..p + 4)?.try_into().ok()?);
    p += 4;
    // Una cadena con su cuenta (en caracteres, con el nulo) delante.
    let cadena = |p: &mut usize| -> Option<String> {
        let n = u32::from_le_bytes(d.get(*p..*p + 4)?.try_into().ok()?) as usize;
        *p += 4;
        let bytes = d.get(*p..p.checked_add(n.checked_mul(2)?)?)?;
        *p += n * 2;
        let u: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .take_while(|&c| c != 0)
            .collect();
        Some(String::from_utf16_lossy(&u))
    };
    let ayuda = if marcas & 0x10 != 0 {
        cadena(&mut p).filter(|s| !s.is_empty())
    } else {
        None
    };
    if marcas & 0x80 != 0 {
        cadena(&mut p)?;
    }
    let mut destino = String::new();
    let mut externo = false;
    if marcas & 0x01 != 0 {
        if marcas & 0x100 != 0 {
            destino = cadena(&mut p)?;
        } else {
            let guid = d.get(p..p + 16)?;
            p += 16;
            if guid[..4] == [0xE0, 0xC9, 0xEA, 0x79] {
                // Moniker de URL: largo en bytes y la URL en UTF-16.
                let largo = u32::from_le_bytes(d.get(p..p + 4)?.try_into().ok()?) as usize;
                p += 4;
                let bytes = d.get(p..p.checked_add(largo)?)?;
                p += largo;
                let u: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .take_while(|&c| c != 0)
                    .collect();
                destino = String::from_utf16_lossy(&u);
            } else if guid[..4] == [0x03, 0x03, 0x00, 0x00] {
                // Moniker de fichero: cuantos «..\», y la ruta en ANSI.
                let atras = usize::from(u16_en(p)?);
                p += 2;
                let largo = u32::from_le_bytes(d.get(p..p + 4)?.try_into().ok()?) as usize;
                p += 4;
                let bytes = d.get(p..p.checked_add(largo)?)?;
                p += largo;
                let ruta: String = bytes
                    .iter()
                    .take_while(|&&b| b != 0)
                    .map(|&b| b as char)
                    .collect();
                destino = format!("{}{}", "..\\".repeat(atras.min(32)), ruta);
            } else {
                return None;
            }
            externo = true;
        }
    }
    if marcas & 0x08 != 0 {
        // Un sitio dentro del libro (`Hoja2!A1`) o dentro del destino.
        // Cuando hay dos GUID de por medio (moniker), el lugar va despues de ellos.
        if let Some(l) = lugar(d, p, marcas) {
            if destino.is_empty() {
                destino = l;
            } else if !l.is_empty() {
                destino = format!("{destino}#{l}");
            }
        }
    }
    if destino.is_empty() {
        return None;
    }
    Some(Hipervinculo {
        rango,
        destino,
        externo,
        ayuda,
    })
}

/// La cadena de lugar de un HLINK: va al final, tras el moniker.
fn lugar(d: &[u8], desde: usize, marcas: u32) -> Option<String> {
    let mut p = desde;
    // Tras un moniker de URL puede haber 24 bytes extra (GUID de serie) dentro de su largo ya contado.
    if marcas & 0x01 != 0 && marcas & 0x100 == 0 {
        // Ya estamos justo despues del moniker.
    }
    let n = u32::from_le_bytes(d.get(p..p + 4)?.try_into().ok()?) as usize;
    p += 4;
    let bytes = d.get(p..p.checked_add(n.checked_mul(2)?)?)?;
    let u: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .take_while(|&c| c != 0)
        .collect();
    Some(String::from_utf16_lossy(&u))
}

// -------------------------------------------------------------- salida

fn escapar(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            '\u{FFFE}' | '\u{FFFF}' => {}
            c => o.push(c),
        }
    }
    o
}

fn estilo_borde(d: u8) -> &'static str {
    match d {
        1 => "thin",
        2 => "medium",
        3 => "dashed",
        4 => "dotted",
        5 => "thick",
        6 => "double",
        7 => "hair",
        8 => "mediumDashed",
        9 => "dashDot",
        10 => "mediumDashDot",
        11 => "dashDotDot",
        12 => "mediumDashDotDot",
        13 => "slantDashDot",
        _ => "none",
    }
}

fn trama(f: u8) -> &'static str {
    match f {
        0 => "none",
        1 => "solid",
        2 => "mediumGray",
        3 => "darkGray",
        4 => "lightGray",
        5 => "darkHorizontal",
        6 => "darkVertical",
        7 => "darkDown",
        8 => "darkUp",
        9 => "darkGrid",
        10 => "darkTrellis",
        11 => "lightHorizontal",
        12 => "lightVertical",
        13 => "lightDown",
        14 => "lightUp",
        15 => "lightGrid",
        16 => "lightTrellis",
        17 => "gray125",
        18 => "gray0625",
        _ => "none",
    }
}

/// Los estilos del libro como un `styles.xml`.
fn xml_estilos(g: &Globales) -> String {
    let mut s = String::with_capacity(64 * 1024);
    s.push_str("<styleSheet>");
    if let Some(p) = &g.paleta {
        // La paleta del archivo reemplaza los colores 8 en adelante.
        s.push_str("<colors><indexedColors>");
        for i in 0..8 {
            let c = crate::estilos::PALETA[i];
            s.push_str(&format!("<rgbColor rgb=\"FF{c:06X}\"/>"));
        }
        for i in 0..56 {
            let c = match p.get(i) {
                Some(c) => (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]),
                None => crate::estilos::PALETA[8 + i],
            };
            s.push_str(&format!("<rgbColor rgb=\"FF{c:06X}\"/>"));
        }
        s.push_str("</indexedColors></colors>");
    }
    if !g.formatos.is_empty() {
        s.push_str("<numFmts>");
        for (id, codigo) in &g.formatos {
            s.push_str(&format!(
                "<numFmt numFmtId=\"{id}\" formatCode=\"{}\"/>",
                escapar(codigo)
            ));
        }
        s.push_str("</numFmts>");
    }
    s.push_str("<fonts>");
    if g.fuentes.is_empty() {
        s.push_str("<font><sz val=\"10\"/><name val=\"Arial\"/></font>");
    }
    for f in &g.fuentes {
        s.push_str("<font>");
        if f.peso >= 700 {
            s.push_str("<b/>");
        }
        if f.marcas & 0x0002 != 0 {
            s.push_str("<i/>");
        }
        if f.marcas & 0x0008 != 0 {
            s.push_str("<strike/>");
        }
        match f.subrayado {
            0 => {}
            2 | 0x22 => s.push_str("<u val=\"double\"/>"),
            _ => s.push_str("<u/>"),
        }
        match f.escrito {
            1 => s.push_str("<vertAlign val=\"superscript\"/>"),
            2 => s.push_str("<vertAlign val=\"subscript\"/>"),
            _ => {}
        }
        let pt = (f64::from(f.altura) / 20.0).clamp(1.0, 409.0);
        s.push_str(&format!("<sz val=\"{pt}\"/>"));
        if f.color != 0x7FFF && f.color < 66 {
            s.push_str(&format!("<color indexed=\"{}\"/>", f.color));
        }
        s.push_str(&format!("<name val=\"{}\"/></font>", escapar(&f.nombre)));
    }
    s.push_str("</fonts><fills>");
    for x in &g.xfs {
        s.push_str(&format!(
            "<fill><patternFill patternType=\"{}\"><fgColor indexed=\"{}\"/><bgColor indexed=\"{}\"/></patternFill></fill>",
            trama(x.trama),
            x.frente,
            x.fondo
        ));
    }
    if g.xfs.is_empty() {
        s.push_str("<fill><patternFill patternType=\"none\"/></fill>");
    }
    s.push_str("</fills><borders>");
    for x in &g.xfs {
        let lado = |nombre: &str, estilo: u8, color: u8| -> String {
            if estilo == 0 {
                format!("<{nombre}/>")
            } else {
                format!(
                    "<{nombre} style=\"{}\"><color indexed=\"{color}\"/></{nombre}>",
                    estilo_borde(estilo)
                )
            }
        };
        let (sube, baja) = (x.diagonal & 2 != 0, x.diagonal & 1 != 0);
        s.push_str(&format!(
            "<border diagonalUp=\"{}\" diagonalDown=\"{}\">{}{}{}{}{}</border>",
            u8::from(sube),
            u8::from(baja),
            lado("left", x.borde[0], x.color_borde[0]),
            lado("right", x.borde[1], x.color_borde[1]),
            lado("top", x.borde[2], x.color_borde[2]),
            lado("bottom", x.borde[3], x.color_borde[3]),
            lado(
                "diagonal",
                if x.diagonal != 0 {
                    x.estilo_diagonal
                } else {
                    0
                },
                x.color_diagonal
            )
        ));
    }
    if g.xfs.is_empty() {
        s.push_str("<border/>");
    }
    s.push_str("</borders><cellXfs>");
    for (i, x) in g.xfs.iter().enumerate() {
        // El indice 4 de fuente no existe en BIFF: de ahi en adelante van desplazados.
        let fuente = if x.fuente > 4 { x.fuente - 1 } else { x.fuente };
        let horizontal = match x.horizontal {
            1 => "left",
            2 => "center",
            3 => "right",
            4 => "fill",
            5 => "justify",
            6 => "centerContinuous",
            7 => "distributed",
            _ => "general",
        };
        let vertical = match x.vertical {
            0 => "top",
            1 => "center",
            3 => "justify",
            4 => "distributed",
            _ => "bottom",
        };
        s.push_str(&format!(
            "<xf numFmtId=\"{}\" fontId=\"{fuente}\" fillId=\"{i}\" borderId=\"{i}\"><alignment horizontal=\"{horizontal}\" vertical=\"{vertical}\" wrapText=\"{}\" shrinkToFit=\"{}\" indent=\"{}\" textRotation=\"{}\"/></xf>",
            x.formato,
            u8::from(x.ajustar),
            u8::from(x.encoger),
            x.sangria,
            x.giro
        ));
    }
    s.push_str("</cellXfs></styleSheet>");
    s
}

fn xml_textos(cadenas: &[String]) -> String {
    let mut s = String::with_capacity(cadenas.iter().map(|c| c.len() + 40).sum::<usize>() + 32);
    s.push_str("<sst>");
    for c in cadenas {
        s.push_str("<si><t xml:space=\"preserve\">");
        s.push_str(&escapar(c));
        s.push_str("</t></si>");
    }
    s.push_str("</sst>");
    s
}

/// Si los bytes son un `.xls` que se puede leer (contenedor con un flujo
/// `Workbook`).
pub fn es_xls(bytes: &[u8]) -> bool {
    Cfb::abrir(bytes).is_ok_and(|c| c.tiene_flujo("Workbook"))
}

/// Un `.xls` abierto: los globales ya leidos (estilos, textos, lista de
/// hojas) y el flujo guardado para leer cada hoja cuando se pida. Abrir no
/// lee las hojas: solo la activa, para saber que el libro se entiende.
pub struct XlsAbierto {
    flujo: Vec<u8>,
    g: Globales,
    pub estilos_xml: Vec<u8>,
    pub textos_xml: Vec<u8>,
    pub activa: usize,
}

impl XlsAbierto {
    pub fn abrir(bytes: &[u8]) -> Result<XlsAbierto, ErrorHojas> {
        let cf = Cfb::abrir(bytes).map_err(mal)?;
        let flujo = match cf.flujo("Workbook").map_err(mal)? {
            Some(f) => f,
            None if cf.tiene_flujo("Book") => {
                return Err(mal("libro de Excel 5.0/95 (BIFF5): no se lee"));
            }
            None => return Err(mal("no tiene flujo Workbook: no es un libro de Excel")),
        };
        let mut pres = Presupuesto::default();
        let g = globales(&flujo, &mut pres)?;
        if g.hojas.is_empty() {
            return Err(mal("el libro no tiene hojas"));
        }
        let activa = if g
            .hojas
            .get(g.activa)
            .is_some_and(|h| h.visibilidad == Visibilidad::Visible)
        {
            g.activa
        } else {
            g.hojas
                .iter()
                .position(|h| h.visibilidad == Visibilidad::Visible)
                .unwrap_or(0)
        };
        let estilos_xml = xml_estilos(&g).into_bytes();
        let textos_xml = xml_textos(&g.cadenas).into_bytes();
        let libro = XlsAbierto {
            flujo,
            g,
            estilos_xml,
            textos_xml,
            activa,
        };
        // Se lee la hoja activa ya: si no se entiende, el libro no se abre y
        // queda la vista previa de Office de respaldo.
        libro.hoja(activa)?;
        Ok(libro)
    }

    pub fn fecha_1904(&self) -> bool {
        self.g.fecha_1904
    }

    /// Nombre, visibilidad y tipo de cada hoja.
    pub fn hojas(&self) -> Vec<(String, Visibilidad, TipoHoja)> {
        self.g
            .hojas
            .iter()
            .map(|h| {
                (
                    h.nombre.clone(),
                    h.visibilidad,
                    if h.tipo == 0 {
                        TipoHoja::Hoja
                    } else {
                        TipoHoja::Grafico
                    },
                )
            })
            .collect()
    }

    /// Lee la hoja `i` (cada llamada la vuelve a leer: quien la pide la guarda).
    pub fn hoja(&self, i: usize) -> Result<HojaXls, ErrorHojas> {
        let h = self
            .g
            .hojas
            .get(i)
            .ok_or_else(|| mal("hoja que no existe"))?;
        leer_hoja(&self.flujo, h, &self.g, &mut Presupuesto::default())
    }
}

/// Lee un libro `.xls` entero (todas las hojas).
pub fn leer(bytes: &[u8]) -> Result<LibroXls, ErrorHojas> {
    let a = XlsAbierto::abrir(bytes)?;
    let mut hojas = Vec::new();
    for i in 0..a.g.hojas.len() {
        hojas.push(a.hoja(i)?);
    }
    Ok(LibroXls {
        fecha_1904: a.g.fecha_1904,
        hojas,
        activa: a.activa,
        estilos_xml: a.estilos_xml,
        textos_xml: a.textos_xml,
    })
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;

    /// Un registro BIFF.
    pub fn reg(id: u16, datos: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend((datos.len() as u16).to_le_bytes());
        v.extend(datos);
        v
    }

    fn cadena_corta(s: &str) -> Vec<u8> {
        let mut v = vec![s.len() as u8, 0];
        v.extend(s.bytes());
        v
    }

    fn xf_bytes(
        fuente: u16,
        formato: u16,
        alin: u8,
        borde: u32,
        relleno: u32,
        colores: u16,
    ) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend(fuente.to_le_bytes());
        v.extend(formato.to_le_bytes());
        v.extend(0u16.to_le_bytes());
        v.push(alin);
        v.push(0);
        v.push(0);
        v.push(0);
        v.extend(borde.to_le_bytes());
        v.extend(relleno.to_le_bytes());
        v.extend(colores.to_le_bytes());
        v
    }

    fn fuente_bytes(altura: u16, peso: u16, nombre: &str) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend(altura.to_le_bytes());
        v.extend(0u16.to_le_bytes());
        v.extend(8u16.to_le_bytes());
        v.extend(peso.to_le_bytes());
        v.extend(0u16.to_le_bytes());
        v.extend([0, 0, 0, 0]);
        v.extend(cadena_corta(nombre));
        v
    }

    /// Un libro minimo: dos hojas, estilos, textos compartidos partidos en
    /// `CONTINUE`, numeros, RK, fechas, combinadas, anchos y altos.
    pub fn libro_de_prueba() -> Vec<u8> {
        let bof_g = reg(BOF, &[0, 6, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let bof_h = reg(BOF, &[0, 6, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let mut g: Vec<u8> = bof_g;
        g.extend(reg(0x0031, &fuente_bytes(200, 400, "Arial")));
        g.extend(reg(0x0031, &fuente_bytes(280, 700, "Calibri")));
        g.extend(reg(0x0031, &fuente_bytes(200, 400, "Arial")));
        g.extend(reg(0x0031, &fuente_bytes(200, 400, "Arial")));
        g.extend(reg(0x0031, &fuente_bytes(200, 400, "Arial")));
        g.extend(reg(0x0031, &fuente_bytes(240, 400, "Courier")));
        let mut fmt = 164u16.to_le_bytes().to_vec();
        fmt.extend(5u16.to_le_bytes());
        fmt.push(0);
        fmt.extend(b"0.00%");
        g.extend(reg(0x041E, &fmt));
        // XF 0: por defecto; 1: negrita centrada con borde fino y relleno; 2: formato 164; 3: fecha 14; 4: fuente del indice 5 (la sexta).
        g.extend(reg(0x00E0, &xf_bytes(0, 0, 0x20, 0, 0, 0)));
        g.extend(reg(
            0x00E0,
            &xf_bytes(
                1,
                0,
                0x02 | 0x08 | 0x10,
                0x0000_1111 | (8 << 16) | (8 << 23),
                (1 << 26) | 13,
                13 | (64 << 7),
            ),
        ));
        g.extend(reg(0x00E0, &xf_bytes(0, 164, 0, 0, 0, 0)));
        g.extend(reg(0x00E0, &xf_bytes(0, 14, 0, 0, 0, 0)));
        g.extend(reg(0x00E0, &xf_bytes(6, 0, 0, 0, 0, 0)));
        // SST: tres textos; el segundo se parte en dos CONTINUE y cambia de compresion.
        let mut sst = 3u32.to_le_bytes().to_vec();
        sst.extend(3u32.to_le_bytes());
        sst.extend([2, 0, 0]);
        sst.extend(b"Hi");
        sst.extend([4, 0, 0]);
        sst.extend(b"ab");
        g.extend(reg(0x00FC, &sst));
        let mut cont = vec![1u8, 0xE7, 0, b'd', 0];
        cont.extend([3, 0, 0]);
        cont.extend(b"fin");
        g.extend(reg(CONTINUE, &cont));
        let tam_globales = g.len() + 2 * (4 + 8 + 8) + 4;
        // BOUNDSHEET x2 con la posicion de cada hoja.
        let mut hoja1: Vec<u8> = bof_h.clone();
        hoja1.extend(reg(0x0200, &[0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 4, 0, 0, 0]));
        hoja1.extend(reg(0x0055, &8u16.to_le_bytes()));
        let mut col = 1u16.to_le_bytes().to_vec();
        col.extend(1u16.to_le_bytes());
        col.extend((20u16 * 256).to_le_bytes());
        col.extend(0u16.to_le_bytes());
        col.extend(0u16.to_le_bytes());
        col.extend(0u16.to_le_bytes());
        hoja1.extend(reg(0x007D, &col));
        // ROW 0: alto 30 pt; ROW 1: oculta; ROW 2 con alto por defecto.
        let fila = |n: u16, alto: u16, marcas: u16| {
            let mut r = n.to_le_bytes().to_vec();
            r.extend(0u16.to_le_bytes());
            r.extend(4u16.to_le_bytes());
            r.extend(alto.to_le_bytes());
            r.extend([0, 0, 0, 0]);
            r.extend(marcas.to_le_bytes());
            r.extend(0u16.to_le_bytes());
            r
        };
        hoja1.extend(reg(0x0208, &fila(0, 600, 0)));
        hoja1.extend(reg(0x0208, &fila(1, 255, 0x20)));
        hoja1.extend(reg(0x0208, &fila(2, 0x80FF, 0)));
        let num = |r: u16, c: u16, x: u16, v: f64| {
            let mut d = r.to_le_bytes().to_vec();
            d.extend(c.to_le_bytes());
            d.extend(x.to_le_bytes());
            d.extend(v.to_le_bytes());
            d
        };
        hoja1.extend(reg(0x0203, &num(0, 0, 1, 1234.5)));
        let mut sstc = |r: u16, c: u16, i: u32| {
            let mut d = r.to_le_bytes().to_vec();
            d.extend(c.to_le_bytes());
            d.extend(0u16.to_le_bytes());
            d.extend(i.to_le_bytes());
            hoja1.extend(reg(0x00FD, &d));
        };
        sstc(0, 1, 1);
        sstc(0, 2, 2);
        // RK entero 7 (fInt, <<2 | 2) y RK *100 de 12,5 (1250 entero con fX100).
        let mut d = 1u16.to_le_bytes().to_vec();
        d.extend(0u16.to_le_bytes());
        d.extend(2u16.to_le_bytes());
        d.extend(((7u32 << 2) | 2).to_le_bytes());
        hoja1.extend(reg(0x027E, &d));
        let mut d = 1u16.to_le_bytes().to_vec();
        d.extend(1u16.to_le_bytes());
        d.extend(3u16.to_le_bytes());
        d.extend(((1250u32 << 2) | 3).to_le_bytes());
        hoja1.extend(reg(0x027E, &d));
        // MULRK en la fila 2, columnas 0 y 1.
        let mut d = 2u16.to_le_bytes().to_vec();
        d.extend(0u16.to_le_bytes());
        for v in [5u32, 6] {
            d.extend(0u16.to_le_bytes());
            d.extend(((v << 2) | 2).to_le_bytes());
        }
        d.extend(1u16.to_le_bytes());
        hoja1.extend(reg(0x00BD, &d));
        // Un logico y un error; una formula con resultado.
        let mut d = 3u16.to_le_bytes().to_vec();
        d.extend(0u16.to_le_bytes());
        d.extend(0u16.to_le_bytes());
        d.extend([1, 0]);
        hoja1.extend(reg(0x0205, &d));
        let mut d = 3u16.to_le_bytes().to_vec();
        d.extend(1u16.to_le_bytes());
        d.extend(0u16.to_le_bytes());
        d.extend([0x07, 1]);
        hoja1.extend(reg(0x0205, &d));
        let mut d = 3u16.to_le_bytes().to_vec();
        d.extend(2u16.to_le_bytes());
        d.extend(0u16.to_le_bytes());
        d.extend(42.0f64.to_le_bytes());
        d.extend([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        hoja1.extend(reg(0x0006, &d));
        // Una formula de texto: su STRING sigue.
        let mut d = 3u16.to_le_bytes().to_vec();
        d.extend(3u16.to_le_bytes());
        d.extend(0u16.to_le_bytes());
        d.extend([0, 0, 0, 0, 0, 0, 0xFF, 0xFF]);
        d.extend([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        hoja1.extend(reg(0x0006, &d));
        let mut d = 3u16.to_le_bytes().to_vec();
        d.extend([0, 0]);
        d.extend(b"ok");
        hoja1.extend(reg(0x0207, &[2, 0, 0, b'o', b'k']));
        let _ = d;
        // Una celda vacia con relleno (se guarda) y una sin nada (no).
        let blanco = |c: u16, x: u16| {
            let mut d = 4u16.to_le_bytes().to_vec();
            d.extend(c.to_le_bytes());
            d.extend(x.to_le_bytes());
            d
        };
        hoja1.extend(reg(0x0201, &blanco(0, 1)));
        hoja1.extend(reg(0x0201, &blanco(1, 0)));
        // Combinada B1:C1... (fila 5, columnas 0 a 2) y una de una sola celda.
        let mut m = 2u16.to_le_bytes().to_vec();
        for (f0, f1, c0, c1) in [(5u16, 5u16, 0u16, 2u16), (6, 6, 0, 0)] {
            for v in [f0, f1, c0, c1] {
                m.extend(v.to_le_bytes());
            }
        }
        hoja1.extend(reg(0x00E5, &m));
        // Cuadricula apagada, panel congelado 1 fila, zoom 150 %.
        let mut w = (0x0004u16 | 0x0008).to_le_bytes().to_vec();
        w.extend([0, 0, 0, 0]);
        hoja1.extend(reg(0x023E, &w));
        let mut p = 0u16.to_le_bytes().to_vec();
        p.extend(1u16.to_le_bytes());
        p.extend(1u16.to_le_bytes());
        p.extend(0u16.to_le_bytes());
        hoja1.extend(reg(0x0041, &p));
        hoja1.extend(reg(0x00A0, &[3, 0, 2, 0]));
        hoja1.extend(reg(EOF, &[]));
        let mut hoja2: Vec<u8> = bof_h;
        hoja2.extend(reg(EOF, &[]));
        let pos1 = tam_globales as u32;
        let pos2 = pos1 + hoja1.len() as u32;
        let bs = |pos: u32, estado: u8, nombre: &str| {
            let mut d = pos.to_le_bytes().to_vec();
            d.push(estado);
            d.push(0);
            d.extend(cadena_corta(nombre));
            reg(0x0085, &d)
        };
        g.extend(bs(pos1, 0, "Datos"));
        g.extend(bs(pos2, 1, "Oculta"));
        g.extend(reg(EOF, &[]));
        // El calculo de `tam_globales` es una cota: se corrige con el largo real.
        let real = g.len() as u32;
        let delta = real as i64 - pos1 as i64;
        let g_final = {
            let mut g2 = g.clone();
            // Reescribir las posiciones en los dos BOUNDSHEET.
            let buscar = |g: &[u8], pos: u32| g.windows(4).position(|w| w == pos.to_le_bytes());
            let a = buscar(&g2, pos2).unwrap();
            g2[a..a + 4].copy_from_slice(&((pos2 as i64 + delta) as u32).to_le_bytes());
            let a = buscar(&g2, pos1).unwrap();
            g2[a..a + 4].copy_from_slice(&((pos1 as i64 + delta) as u32).to_le_bytes());
            g2
        };
        let mut todo = g_final;
        todo.extend(hoja1);
        todo.extend(hoja2);
        crate::cfb::construir(&[("Workbook", &todo)])
    }

    #[test]
    fn lee_valores_textos_y_estilos() {
        let b = libro_de_prueba();
        assert!(es_xls(&b));
        let l = leer(&b).unwrap();
        assert_eq!(l.hojas.len(), 2);
        assert_eq!(l.hojas[0].nombre, "Datos");
        assert_eq!(l.hojas[1].visibilidad, Visibilidad::Oculta);
        let h = &l.hojas[0];
        let fila = |n: u32| {
            h.filas
                .iter()
                .find(|f| f.indice == n)
                .unwrap_or_else(|| panic!("fila {n}"))
        };
        let f0 = fila(0);
        assert_eq!(f0.alto, 30.0);
        assert_eq!(
            f0.celdas[0],
            Celda {
                col: 0,
                xf: 1,
                valor: Valor::Numero(1234.5)
            }
        );
        assert_eq!(f0.celdas[1].valor, Valor::Compartido(1));
        assert_eq!(f0.celdas.len(), 3, "la celda vacia con relleno cuenta");
        assert!(fila(1).oculta);
        let f1 = fila(1);
        assert_eq!(f1.celdas[0].valor, Valor::Numero(7.0));
        assert_eq!(f1.celdas[1].valor, Valor::Numero(12.5));
        let f2 = fila(2);
        assert!(f2.alto.is_nan(), "alto por defecto");
        assert_eq!(
            (f2.celdas[0].valor, f2.celdas[1].valor),
            (Valor::Numero(5.0), Valor::Numero(6.0))
        );
        let f3 = fila(3);
        assert_eq!(f3.celdas[0].valor, Valor::Logico(true));
        assert_eq!(f3.celdas[1].valor, Valor::Error(1));
        assert_eq!(
            f3.celdas[2].valor,
            Valor::Numero(42.0),
            "valor guardado de la formula"
        );
        assert!(matches!(f3.celdas[3].valor, Valor::Propio(0)));
        assert_eq!(h.textos, vec!["ok".to_string()]);
        // La fila 4 solo tenia una celda vacia con el estilo por defecto: no existe.
        assert!(
            !h.filas
                .iter()
                .any(|f| f.indice == 4 && f.celdas.iter().any(|c| c.col == 1))
        );
        // Cabecera y cola.
        assert_eq!(
            h.cabecera.columnas,
            vec![Columnas {
                min: 1,
                max: 1,
                ancho: Some(20.0),
                oculta: false,
                xf: 0
            }]
        );
        assert_eq!(h.cabecera.ancho_base, Some(8.0));
        assert!(!h.cabecera.vista.cuadricula && h.cabecera.vista.encabezados);
        assert_eq!(
            (
                h.cabecera.vista.filas_fijas,
                h.cabecera.vista.columnas_fijas
            ),
            (1, 0)
        );
        assert_eq!(h.cabecera.vista.zoom, 150);
        assert_eq!(
            h.cola.combinadas,
            vec![Rango {
                fila0: 5,
                col0: 0,
                fila1: 5,
                col1: 2
            }]
        );
        // Los textos compartidos con CONTINUE y cambio de compresion.
        let sst = String::from_utf8(l.textos_xml.clone()).unwrap();
        assert!(
            sst.contains(">Hi<") && sst.contains(">abçd<") && sst.contains(">fin<"),
            "{sst}"
        );
    }

    #[test]
    fn los_estilos_salen_como_en_el_archivo() {
        use crate::estilos::{Estilos, Horizontal, Relleno, Rgb};
        use crate::formato::Regional;
        let l = leer(&libro_de_prueba()).unwrap();
        let r = Regional::en_us();
        let e = Estilos::leer(&l.estilos_xml, &crate::estilos::Tema::default(), &r);
        assert_eq!(e.xfs.len(), 5);
        // XF 1: negrita Calibri 14, centrada, ajustada, borde fino rojo oscuro, relleno rojo.
        let x = e.xf(1);
        let f = e.fuente(x);
        assert!(f.negrita && f.tam == 14.0 && &*f.nombre == "Calibri");
        assert_eq!(x.alineacion.horizontal, Horizontal::Centro);
        assert!(x.alineacion.ajustar);
        let b = e.borde(x);
        assert_eq!(b.izquierda.estilo, crate::estilos::EstiloLinea::Fino);
        assert_eq!(b.izquierda.color, Rgb::de_u32(crate::estilos::PALETA[8]));
        assert_eq!(
            e.relleno(x),
            Relleno::Liso(Rgb::de_u32(crate::estilos::PALETA[13]))
        );
        // XF 2: formato propio 0.00%; XF 3: fecha integrada.
        assert_eq!(e.xf(2).formato.numero(0.256, false, &r).texto, "25.60%");
        assert!(e.xf(3).formato.es_fecha);
        // XF 4: la fuente 6 del archivo es la sexta de la lista (el indice 4 no existe).
        assert_eq!(&*e.fuente(e.xf(4)).nombre, "Courier");
    }

    #[test]
    fn libros_rotos_dan_error_y_nunca_panico() {
        let b = libro_de_prueba();
        // No es un xls.
        assert!(leer(b"nada de nada").is_err());
        assert!(!es_xls(b"PK\x03\x04"));
        // Cortado en cualquier punto: error o un libro parcial, nunca un panico.
        let mut ok_o_err = 0;
        for corte in (0..b.len()).step_by(37) {
            let _ = leer(&b[..corte]);
            ok_o_err += 1;
        }
        assert!(ok_o_err > 10);
        // Bytes cambiados a mano en cada posicion de los primeros registros.
        let flujo_en = 512 * 5;
        for en in (flujo_en..(flujo_en + 700).min(b.len())).step_by(3) {
            for v in [0u8, 0xFF, 0x80, 0x7F] {
                let mut c = b.clone();
                c[en] = v;
                let _ = leer(&c);
            }
        }
        // Un BIFF5 se rechaza.
        let mut v5 = reg(BOF, &[0, 5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        v5.extend(reg(EOF, &[]));
        let c = crate::cfb::construir(&[("Workbook", &v5)]);
        assert!(leer(&c).is_err());
        // Un libro cifrado (FilePass) tambien.
        let mut v = reg(BOF, &[0, 6, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        v.extend(reg(0x002F, &[1, 0, 1, 0, 1, 0]));
        v.extend(reg(EOF, &[]));
        let c = crate::cfb::construir(&[("Workbook", &v)]);
        assert!(matches!(leer(&c), Err(ErrorHojas::Io(m)) if m.contains("contraseña")));
    }

    #[test]
    fn dos_hojas_en_el_mismo_sitio_o_sin_hojas_se_rechazan() {
        let mut g = reg(BOF, &[0, 6, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let mut d = 100u32.to_le_bytes().to_vec();
        d.extend([0, 0]);
        d.extend(cadena_corta("A"));
        g.extend(reg(0x0085, &d));
        g.extend(reg(0x0085, &d));
        g.extend(reg(EOF, &[]));
        assert!(leer(&crate::cfb::construir(&[("Workbook", &g)])).is_err());
        // Sin hojas.
        let mut g = reg(BOF, &[0, 6, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        g.extend(reg(EOF, &[]));
        assert!(leer(&crate::cfb::construir(&[("Workbook", &g)])).is_err());
    }

    /// Muestra una hoja de un `.xls` de verdad: `XLS_PRUEBA=ruta cargo test ... -- --ignored mostrar`.
    #[test]
    #[ignore]
    fn mostrar_un_xls_de_verdad() {
        let Ok(ruta) = std::env::var("XLS_PRUEBA") else {
            return;
        };
        let b = std::fs::read(ruta).unwrap();
        let l = leer(&b).unwrap();
        for h in &l.hojas {
            println!("hoja {} ({} filas)", h.nombre, h.filas.len());
            for f in h.filas.iter().take(12) {
                println!(
                    "  fila {} alto {} oculta {} -> {:?}",
                    f.indice, f.alto, f.oculta, f.celdas
                );
            }
        }
    }

    #[test]
    fn rk_numeros() {
        assert_eq!(rk((7 << 2) | 2), 7.0);
        assert_eq!(rk(((-3i32 as u32) << 2) | 2), -3.0);
        assert_eq!(rk((250 << 2) | 3), 2.5);
        // Flotante: los 30 bits altos de un f64 (1.5 = 0x3FF8_0000_0000_0000).
        assert_eq!(rk(0x3FF8_0000), 1.5);
        assert_eq!(rk(0x3FF8_0000 | 1), 0.015);
    }

    #[test]
    fn enlaces_de_url_y_de_lugar() {
        // URL moniker.
        let mut d = Vec::new();
        for v in [2u16, 2, 1, 1] {
            d.extend(v.to_le_bytes());
        }
        d.extend([0u8; 16]);
        d.extend(2u32.to_le_bytes());
        d.extend((0x01u32 | 0x02).to_le_bytes());
        d.extend([
            0xE0, 0xC9, 0xEA, 0x79, 0xF9, 0xBA, 0xCE, 0x11, 0x8C, 0x82, 0x00, 0xAA, 0x00, 0x4B,
            0xA9, 0x0B,
        ]);
        let url: Vec<u8> = "https://a.pe/x\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        d.extend((url.len() as u32).to_le_bytes());
        d.extend(url);
        let h = enlace(&d).unwrap();
        assert_eq!(h.destino, "https://a.pe/x");
        assert!(h.externo);
        assert_eq!(
            h.rango,
            Rango {
                fila0: 2,
                col0: 1,
                fila1: 2,
                col1: 1
            }
        );
        // Solo lugar (dentro del libro).
        let mut d = Vec::new();
        for v in [0u16, 0, 0, 0] {
            d.extend(v.to_le_bytes());
        }
        d.extend([0u8; 16]);
        d.extend(2u32.to_le_bytes());
        d.extend(0x08u32.to_le_bytes());
        let lug: Vec<u8> = "Hoja2!A1\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        d.extend(9u32.to_le_bytes());
        d.extend(lug);
        let h = enlace(&d).unwrap();
        assert_eq!((h.destino.as_str(), h.externo), ("Hoja2!A1", false));
        // Roto: sin destino o cortado.
        assert!(enlace(&d[..20]).is_none());
        assert!(enlace(&[]).is_none());
        let mut roto = d.clone();
        roto[28..32].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        assert!(enlace(&roto).is_none());
    }
}
