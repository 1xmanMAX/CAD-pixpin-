//! **Que imagen es y cuanto mide**, mirando sus bytes y no su nombre.
//!
//! Una foto de la nota puede llamarse `.png` y ser un JPEG (una captura
//! pegada que alguien renombro) o no tener extension: Word abre la imagen
//! por el tipo que dice `[Content_Types].xml`, y si no casa con los bytes la
//! ensena rota o se niega a abrir el documento. Por eso el tipo sale de la
//! firma del fichero, y el tamano de su cabecera (sin descodificarla: es lo
//! unico que hace falta para el `wp:extent`).

/// Los formatos que Word pinta en cualquier version (2007 en adelante) y
/// LibreOffice tambien. WebP, HEIC o SVG no: quien llama los pasa antes a
/// PNG si puede (la aplicacion lo hace con `pixpin_codec`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Png,
    Jpeg,
    Gif,
    Bmp,
}

impl Formato {
    pub fn extension(self) -> &'static str {
        match self {
            Formato::Png => "png",
            Formato::Jpeg => "jpeg",
            Formato::Gif => "gif",
            Formato::Bmp => "bmp",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Formato::Png => "image/png",
            Formato::Jpeg => "image/jpeg",
            Formato::Gif => "image/gif",
            Formato::Bmp => "image/bmp",
        }
    }
}

/// El formato y el tamano en pixeles, o nada si no es una imagen que Word
/// sepa pintar o si la cabecera esta rota (mejor un aviso en el documento
/// que un Word que no abre).
pub fn leer(b: &[u8]) -> Option<(Formato, u32, u32)> {
    let (f, an, al) = if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        // IHDR va siempre la primera: ancho y alto en big endian.
        if b.len() < 24 || &b[12..16] != b"IHDR" {
            return None;
        }
        (Formato::Png, be32(&b[16..20]), be32(&b[20..24]))
    } else if b.starts_with(&[0xFF, 0xD8]) {
        let (an, al) = jpeg(b)?;
        (Formato::Jpeg, an, al)
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        if b.len() < 10 {
            return None;
        }
        (Formato::Gif, le16(&b[6..8]), le16(&b[8..10]))
    } else if b.starts_with(b"BM") {
        if b.len() < 26 {
            return None;
        }
        // El alto es negativo si las filas van de arriba abajo.
        let al = i32::from_le_bytes([b[22], b[23], b[24], b[25]]).unsigned_abs();
        let an = i32::from_le_bytes([b[18], b[19], b[20], b[21]]).unsigned_abs();
        (Formato::Bmp, an, al)
    } else {
        return None;
    };
    (an > 0 && al > 0 && an <= 100_000 && al <= 100_000).then_some((f, an, al))
}

/// Si los bytes son de un WebP (para decir que hay que convertirlo).
pub fn es_webp(b: &[u8]) -> bool {
    b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP"
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn le16(b: &[u8]) -> u32 {
    u16::from_le_bytes([b[0], b[1]]) as u32
}

/// El tamano de un JPEG: el del primer marcador de fotograma (SOF0..SOF15
/// salvo los que no lo son: DHT, JPG y DAC).
fn jpeg(b: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let m = b[i + 1];
        // Relleno y marcadores sin longitud.
        if m == 0xFF {
            i += 1;
            continue;
        }
        if m == 0xD8 || m == 0x01 || (0xD0..=0xD7).contains(&m) {
            i += 2;
            continue;
        }
        let largo = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        if largo < 2 {
            return None;
        }
        if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
            if i + 9 > b.len() {
                return None;
            }
            let al = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
            let an = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
            return Some((an, al));
        }
        i += 2 + largo;
    }
    None
}
