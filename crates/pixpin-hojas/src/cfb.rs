//! El contenedor compuesto de Microsoft (CFB / OLE2, [MS-CFB]) de los `.xls`
//! antiguos: sectores, FAT, mini FAT y directorio. Solo lee, y todo con
//! limites: una cadena de sectores en bucle, un tamano imposible o un fichero
//! cortado dan un error, nunca un panico ni un bucle infinito.
//!
//! Partes traducidas de gridcraft (https://github.com/storytold/gridcraft), MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team and the Gridcraft contributors.
//! (`crates/xlsx/src/cfb.rs`).

/// La firma del principio del fichero.
pub const FIRMA: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

const FIN_DE_CADENA: u32 = 0xFFFF_FFFE;
/// Los sectores de este numero para arriba son marcas (libre, fin, FAT...).
const MAX_SECTOR: u32 = 0xFFFF_FFFA;
const MAX_SECTORES_DIFAT: usize = 1024;
/// Un flujo mayor que esto se rechaza (un libro no lo necesita).
const MAX_FLUJO: u64 = 1 << 30;
const SIN_FLUJO: u32 = 0xFFFF_FFFF;
const ES_FLUJO: u8 = 2;

fn u16_en(b: &[u8], en: usize) -> Option<u16> {
    let s = b.get(en..en.checked_add(2)?)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_en(b: &[u8], en: usize) -> Option<u32> {
    let s = b.get(en..en.checked_add(4)?)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Si los bytes empiezan con la firma de un contenedor compuesto.
pub fn es_cfb(b: &[u8]) -> bool {
    b.starts_with(&FIRMA)
}

struct Entrada {
    nombre: String,
    tipo: u8,
    izq: u32,
    der: u32,
    hijo: u32,
    inicio: u32,
    tam: u64,
}

/// Un contenedor ya interpretado: FAT, mini FAT y directorio.
pub struct Cfb<'a> {
    bytes: &'a [u8],
    tam_sector: usize,
    cota_mini: u64,
    fat: Vec<u32>,
    mini_fat: Vec<u32>,
    entradas: Vec<Entrada>,
}

impl<'a> Cfb<'a> {
    pub fn abrir(b: &'a [u8]) -> Result<Cfb<'a>, String> {
        let mal = |m: &str| format!("contenedor dañado: {m}");
        if !es_cfb(b) {
            return Err(mal("sin firma"));
        }
        let version = u16_en(b, 26).ok_or_else(|| mal("cabecera cortada"))?;
        let desplaz = u16_en(b, 30).ok_or_else(|| mal("cabecera cortada"))?;
        if desplaz != 9 && desplaz != 12 {
            return Err(mal("tamaño de sector no admitido"));
        }
        if u16_en(b, 32) != Some(6) {
            return Err(mal("tamaño de minisector no admitido"));
        }
        let tam_sector = 1usize << desplaz;
        let por = tam_sector / 4;
        // Los sectores que caben de verdad tras la cabecera: acota toda cadena.
        let sectores = (b.len() / tam_sector).saturating_sub(1);
        let sector = |id: u32| -> Option<&[u8]> {
            let ini = (id as usize).checked_add(1)?.checked_mul(tam_sector)?;
            b.get(ini..ini.checked_add(tam_sector)?)
        };
        // DIFAT: 109 sectores de FAT en la cabecera y despues encadenados.
        let mut sectores_fat: Vec<u32> = (0..109)
            .filter_map(|i| u32_en(b, 76 + i * 4))
            .filter(|&s| s < MAX_SECTOR)
            .collect();
        let mut sig = u32_en(b, 68).ok_or_else(|| mal("cabecera cortada"))?;
        let mut saltos = 0usize;
        while sig < MAX_SECTOR {
            saltos += 1;
            if saltos > sectores.min(MAX_SECTORES_DIFAT * 64) {
                return Err(mal("la cadena DIFAT da vueltas"));
            }
            let s = sector(sig).ok_or_else(|| mal("sector DIFAT fuera del fichero"))?;
            sectores_fat.extend(
                (0..por - 1)
                    .filter_map(|i| u32_en(s, i * 4))
                    .filter(|&x| x < MAX_SECTOR),
            );
            sig = u32_en(s, (por - 1) * 4).ok_or_else(|| mal("sector DIFAT cortado"))?;
        }
        if sectores_fat.len() > sectores {
            return Err(mal("mas sectores de FAT que sectores en el fichero"));
        }
        let mut fat = Vec::with_capacity(sectores_fat.len().saturating_mul(por));
        for id in sectores_fat {
            let s = sector(id).ok_or_else(|| mal("sector de FAT fuera del fichero"))?;
            fat.extend((0..por).filter_map(|i| u32_en(s, i * 4)));
        }
        let mut cf = Cfb {
            bytes: b,
            tam_sector,
            cota_mini: u64::from(u32_en(b, 56).unwrap_or(4096)),
            fat,
            mini_fat: Vec::new(),
            entradas: Vec::new(),
        };
        let ini_dir = u32_en(b, 48).ok_or_else(|| mal("cabecera cortada"))?;
        let dir = cf.leer_cadena(ini_dir, u64::MAX)?;
        for e in dir.chunks_exact(128) {
            let largo = usize::from(u16_en(e, 64).unwrap_or(0)).min(64);
            let unidades: Vec<u16> = (0..largo.saturating_sub(2) / 2)
                .filter_map(|i| u16_en(e, i * 2))
                .collect();
            let mut tam = u32_en(e, 120).map(u64::from).unwrap_or(0);
            if version >= 4 {
                tam |= u64::from(u32_en(e, 124).unwrap_or(0)) << 32;
            }
            cf.entradas.push(Entrada {
                nombre: String::from_utf16_lossy(&unidades),
                tipo: e.get(66).copied().unwrap_or(0),
                izq: u32_en(e, 68).unwrap_or(SIN_FLUJO),
                der: u32_en(e, 72).unwrap_or(SIN_FLUJO),
                hijo: u32_en(e, 76).unwrap_or(SIN_FLUJO),
                inicio: u32_en(e, 116).unwrap_or(FIN_DE_CADENA),
                tam,
            });
        }
        if cf.entradas.first().is_none_or(|r| r.tipo != 5) {
            return Err(mal("falta la entrada raiz"));
        }
        let ini_mini = u32_en(b, 60).ok_or_else(|| mal("cabecera cortada"))?;
        if ini_mini < MAX_SECTOR {
            let crudo = cf.leer_cadena(ini_mini, u64::MAX)?;
            cf.mini_fat = (0..crudo.len() / 4)
                .filter_map(|i| u32_en(&crudo, i * 4))
                .collect();
        }
        Ok(cf)
    }

    /// Sigue una cadena de la FAT desde `inicio`, leyendo `limite` bytes como
    /// mucho. Una cadena mas larga que la FAT (un bucle) o que sale del
    /// fichero es un error.
    fn leer_cadena(&self, inicio: u32, limite: u64) -> Result<Vec<u8>, String> {
        let limite = usize::try_from(limite.min(MAX_FLUJO)).unwrap_or(usize::MAX);
        let mut sal = Vec::new();
        let mut id = inicio;
        let mut vistos = vec![false; self.fat.len()];
        while id != FIN_DE_CADENA && sal.len() < limite {
            if id >= MAX_SECTOR {
                return Err("contenedor dañado: cadena de sectores rota".into());
            }
            match vistos.get_mut(id as usize) {
                Some(v) if !*v => *v = true,
                Some(_) => return Err("contenedor dañado: la cadena de sectores da vueltas".into()),
                None => return Err("contenedor dañado: sector que no esta en la FAT".into()),
            }
            let ini = (id as usize)
                .checked_add(1)
                .and_then(|i| i.checked_mul(self.tam_sector));
            let s = ini
                .and_then(|s| self.bytes.get(s..s.checked_add(self.tam_sector)?))
                .ok_or("contenedor dañado: sector fuera del fichero (¿cortado?)")?;
            sal.extend_from_slice(s);
            id = *self
                .fat
                .get(id as usize)
                .ok_or("contenedor dañado: sector que no esta en la FAT")?;
        }
        Ok(sal)
    }

    fn leer_mini(&self, contenedor: &[u8], inicio: u32, limite: usize) -> Result<Vec<u8>, String> {
        let mut sal = Vec::new();
        let mut id = inicio;
        let mut vistos = vec![false; self.mini_fat.len()];
        while id != FIN_DE_CADENA && sal.len() < limite {
            if id >= MAX_SECTOR {
                return Err("contenedor dañado: cadena de minisectores rota".into());
            }
            match vistos.get_mut(id as usize) {
                Some(v) if !*v => *v = true,
                Some(_) => {
                    return Err("contenedor dañado: la cadena de minisectores da vueltas".into());
                }
                None => {
                    return Err("contenedor dañado: minisector que no esta en la mini FAT".into());
                }
            }
            let en = (id as usize).checked_mul(64);
            let s = en
                .and_then(|a| contenedor.get(a..a.checked_add(64)?))
                .ok_or("contenedor dañado: minisector fuera del contenedor")?;
            sal.extend_from_slice(s);
            id = *self
                .mini_fat
                .get(id as usize)
                .ok_or("contenedor dañado: minisector que no esta en la mini FAT")?;
        }
        Ok(sal)
    }

    /// La entrada de un flujo directamente bajo la raiz (sin mirar
    /// mayusculas). Recorre el arbol de hermanos con una cota.
    fn entrada_raiz(&self, nombre: &str) -> Result<Option<&Entrada>, String> {
        let Some(raiz) = self.entradas.first() else {
            return Ok(None);
        };
        let mut pila = vec![raiz.hijo];
        let mut vistas = 0usize;
        while let Some(i) = pila.pop() {
            if i == SIN_FLUJO {
                continue;
            }
            vistas += 1;
            if vistas > self.entradas.len() {
                return Err("contenedor dañado: el arbol del directorio da vueltas".into());
            }
            let Some(e) = self.entradas.get(i as usize) else {
                continue;
            };
            if e.tipo == ES_FLUJO && e.nombre.eq_ignore_ascii_case(nombre) {
                return Ok(Some(e));
            }
            pila.push(e.izq);
            pila.push(e.der);
        }
        Ok(None)
    }

    pub fn tiene_flujo(&self, nombre: &str) -> bool {
        matches!(self.entrada_raiz(nombre), Ok(Some(_)))
    }

    /// El contenido de un flujo directamente bajo la raiz, o `None` si no hay.
    pub fn flujo(&self, nombre: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(raiz) = self.entradas.first() else {
            return Ok(None);
        };
        let Some(e) = self.entrada_raiz(nombre)? else {
            return Ok(None);
        };
        if e.tam > MAX_FLUJO {
            return Err("el flujo pasa de 1 GiB".into());
        }
        let tam = usize::try_from(e.tam).map_err(|_| "flujo demasiado grande".to_string())?;
        let mut datos = if e.tam < self.cota_mini {
            let contenedor = self.leer_cadena(raiz.inicio, raiz.tam)?;
            self.leer_mini(&contenedor, e.inicio, tam)?
        } else {
            self.leer_cadena(e.inicio, e.tam)?
        };
        if datos.len() < tam {
            return Err(
                "contenedor dañado: un flujo es mas corto de lo que dice (¿cortado?)".into(),
            );
        }
        datos.truncate(tam);
        Ok(Some(datos))
    }
}

/// Arma un contenedor de prueba con flujos grandes (en sectores) y chicos (en
/// minisectores). Solo para las pruebas.
#[cfg(test)]
pub(crate) fn construir(flujos: &[(&str, &[u8])]) -> Vec<u8> {
    // Disposicion: sector 0 = FAT, 1 = directorio, 2 = mini FAT, 3 = contenedor
    // de minisectores (el flujo de la raiz); despues los flujos grandes.
    let mut mini: Vec<u8> = Vec::new();
    let mut mini_fat: Vec<u32> = Vec::new();
    let mut grandes: Vec<(usize, Vec<u8>)> = Vec::new();
    // (indice del flujo, inicio, tam)
    let mut destino: Vec<(u32, u64)> = Vec::new();
    let mut siguiente_sector = 4u32;
    for (i, (_, d)) in flujos.iter().enumerate() {
        if (d.len() as u64) < 4096 {
            let ini = mini_fat.len() as u32;
            let n = d.len().div_ceil(64).max(1);
            for k in 0..n {
                mini_fat.push(if k + 1 == n {
                    FIN_DE_CADENA
                } else {
                    ini + k as u32 + 1
                });
            }
            let mut relleno = d.to_vec();
            relleno.resize(n * 64, 0);
            mini.extend(relleno);
            destino.push((ini, d.len() as u64));
        } else {
            let n = d.len().div_ceil(512);
            destino.push((siguiente_sector, d.len() as u64));
            siguiente_sector += n as u32;
            grandes.push((i, d.to_vec()));
        }
    }
    let n_mini_sectores = mini.len().div_ceil(512).max(1);
    // El contenedor de minisectores ocupa sectores consecutivos desde `3`; los
    // grandes empiezan despues, asi que se recolocan.
    let desplazo = (n_mini_sectores as u32).saturating_sub(1);
    for (k, (ini, tam)) in destino.iter_mut().enumerate() {
        if flujos[k].1.len() >= 4096 {
            *ini += desplazo;
            let _ = tam;
        }
    }
    let total = 4
        + desplazo as usize
        + grandes
            .iter()
            .map(|g| g.1.len().div_ceil(512))
            .sum::<usize>();
    let mut b = vec![0u8; 512 * total];
    b.resize(512 * (total + 1), 0);
    b[..8].copy_from_slice(&FIRMA);
    b[24..26].copy_from_slice(&0x3Eu16.to_le_bytes());
    b[26..28].copy_from_slice(&3u16.to_le_bytes());
    b[28..30].copy_from_slice(&0xFFFEu16.to_le_bytes());
    b[30..32].copy_from_slice(&9u16.to_le_bytes());
    b[32..34].copy_from_slice(&6u16.to_le_bytes());
    b[44..48].copy_from_slice(&1u32.to_le_bytes());
    b[48..52].copy_from_slice(&1u32.to_le_bytes());
    b[56..60].copy_from_slice(&4096u32.to_le_bytes());
    b[60..64].copy_from_slice(&2u32.to_le_bytes());
    b[64..68].copy_from_slice(&1u32.to_le_bytes());
    b[68..72].copy_from_slice(&FIN_DE_CADENA.to_le_bytes());
    for i in 0..109 {
        b[76 + i * 4..80 + i * 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    }
    b[76..80].copy_from_slice(&0u32.to_le_bytes());
    // Sector s esta en el byte 512 * (s + 1).
    let en = |s: usize| 512 * (s + 1);
    let mut fat = vec![0xFFFF_FFFFu32; 128];
    fat[0] = 0xFFFF_FFFD;
    fat[1] = FIN_DE_CADENA;
    fat[2] = FIN_DE_CADENA;
    for k in 0..n_mini_sectores {
        fat[3 + k] = if k + 1 == n_mini_sectores {
            FIN_DE_CADENA
        } else {
            3 + k as u32 + 1
        };
    }
    let mut s0 = 3 + n_mini_sectores;
    for (i, d) in &grandes {
        let _ = i;
        let n = d.len().div_ceil(512);
        for k in 0..n {
            fat[s0 + k] = if k + 1 == n {
                FIN_DE_CADENA
            } else {
                (s0 + k + 1) as u32
            };
        }
        b[en(s0)..en(s0) + d.len()].copy_from_slice(d);
        s0 += n;
    }
    for (i, v) in fat.iter().enumerate() {
        b[en(0) + i * 4..en(0) + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    for (i, v) in mini_fat.iter().enumerate() {
        b[en(2) + i * 4..en(2) + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    b[en(3)..en(3) + mini.len()].copy_from_slice(&mini);
    // Directorio: raiz + flujos como hermanos en cadena por la derecha.
    let escribir = |b: &mut Vec<u8>,
                    k: usize,
                    nombre: &str,
                    tipo: u8,
                    der: u32,
                    hijo: u32,
                    ini: u32,
                    tam: u64| {
        let e = en(1) + k * 128;
        let u: Vec<u16> = nombre.encode_utf16().take(31).collect();
        for (j, x) in u.iter().enumerate() {
            b[e + j * 2..e + j * 2 + 2].copy_from_slice(&x.to_le_bytes());
        }
        b[e + 64..e + 66].copy_from_slice(&((u.len() as u16 + 1) * 2).to_le_bytes());
        b[e + 66] = tipo;
        b[e + 68..e + 72].copy_from_slice(&SIN_FLUJO.to_le_bytes());
        b[e + 72..e + 76].copy_from_slice(&der.to_le_bytes());
        b[e + 76..e + 80].copy_from_slice(&hijo.to_le_bytes());
        b[e + 116..e + 120].copy_from_slice(&ini.to_le_bytes());
        b[e + 120..e + 124].copy_from_slice(&(tam as u32).to_le_bytes());
    };
    escribir(
        &mut b,
        0,
        "Root Entry",
        5,
        SIN_FLUJO,
        if flujos.is_empty() { SIN_FLUJO } else { 1 },
        3,
        mini.len() as u64,
    );
    for (k, (nombre, _)) in flujos.iter().enumerate().take(3) {
        let der = if k + 1 < flujos.len().min(3) {
            k as u32 + 2
        } else {
            SIN_FLUJO
        };
        escribir(
            &mut b,
            k + 1,
            nombre,
            ES_FLUJO,
            der,
            SIN_FLUJO,
            destino[k].0,
            destino[k].1,
        );
    }
    b
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lee_flujos_chicos_y_grandes() {
        let grande: Vec<u8> = (0..6000u32).map(|i| (i % 251) as u8).collect();
        let b = construir(&[
            ("Workbook", &grande),
            ("Chico", b"hola mundo"),
            ("Otro", &[7u8; 130]),
        ]);
        let cf = Cfb::abrir(&b).unwrap();
        assert_eq!(cf.flujo("workbook").unwrap().unwrap(), grande);
        assert_eq!(cf.flujo("Chico").unwrap().unwrap(), b"hola mundo");
        assert_eq!(cf.flujo("Otro").unwrap().unwrap(), vec![7u8; 130]);
        assert!(cf.flujo("NoHay").unwrap().is_none());
        assert!(cf.tiene_flujo("Chico") && !cf.tiene_flujo("Nada"));
    }

    #[test]
    fn no_es_un_contenedor() {
        assert!(Cfb::abrir(b"PK\x03\x04 esto es un zip").is_err());
        assert!(Cfb::abrir(&[]).is_err());
        assert!(!es_cfb(b"hola"));
        // Solo la firma, sin cabecera entera.
        assert!(Cfb::abrir(&FIRMA).is_err());
    }

    #[test]
    fn cadenas_en_bucle_y_cabeceras_hostiles_no_hacen_panico() {
        let grande: Vec<u8> = vec![1u8; 6000];
        let base = construir(&[("Workbook", &grande), ("Chico", b"abc")]);
        // Un bucle en la cadena del flujo grande: el sector 4 apunta a si mismo.
        let mut b = base.clone();
        b[512 + 4 * 4..512 + 4 * 4 + 4].copy_from_slice(&4u32.to_le_bytes());
        let cf = Cfb::abrir(&b).unwrap();
        assert!(cf.flujo("Workbook").is_err(), "el bucle se detecta");
        // Un tamano imposible en el directorio.
        let mut b = base.clone();
        let e = 512 * 2 + 128;
        b[e + 120..e + 124].copy_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
        let cf = Cfb::abrir(&b).unwrap();
        assert!(cf.flujo("Workbook").is_err());
        // Campos de cabecera con valores raros y ficheros cortados: nunca un panico.
        for en in [
            26usize,
            30,
            32,
            44,
            48,
            56,
            60,
            68,
            76,
            512,
            516,
            1024 + 64,
            1024 + 76,
            1024 + 116,
            1024 + 120,
        ] {
            for v in [
                0u32,
                1,
                7,
                0xFFFF_FFFA,
                0xFFFF_FFFF,
                0x7FFF_FFFF,
                0xFFFF_FFFE,
            ] {
                let mut c = base.clone();
                c[en..en + 4].copy_from_slice(&v.to_le_bytes());
                for corte in [0usize, 8, 100, 600, 1100, 3000, c.len()] {
                    if let Ok(cf) = Cfb::abrir(&c[..corte.min(c.len())]) {
                        let _ = cf.flujo("Workbook");
                        let _ = cf.flujo("Chico");
                        let _ = cf.tiene_flujo("Chico");
                    }
                }
            }
        }
    }

    #[test]
    fn un_arbol_de_directorio_que_da_vueltas_se_corta() {
        let mut b = construir(&[("Chico", b"abc")]);
        // La entrada 1 se tiene a si misma como hermana derecha.
        let e = 512 * 2 + 128;
        b[e + 72..e + 76].copy_from_slice(&1u32.to_le_bytes());
        b[e + 64..e + 66].copy_from_slice(&4u16.to_le_bytes());
        let cf = Cfb::abrir(&b).unwrap();
        assert!(cf.flujo("Otro").is_err());
    }
}
