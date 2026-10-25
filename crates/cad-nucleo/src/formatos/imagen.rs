//! **IMAGEATTACH**: una imagen (PNG, JPG, BMP, TIF, GIF) enlazada en el
//! dibujo como entidad IMAGE con su definicion IMAGEDEF en el diccionario
//! `ACAD_IMAGE_DICT` y su reactor, como la deja AutoCAD: asi se guarda en
//! DWG y DXF y la ven los demas programas. Del fichero solo se lee la
//! cabecera (el tamano en pixeles); la imagen queda enlazada por su ruta.

use std::path::Path;

use opencadcodec::entities::RasterImage;
use opencadcodec::objects::{Dictionary, ImageDefinition, ImageDefinitionReactor, ObjectType};
use opencadcodec::{CadDocument, EntityType, Handle, Vector3};

use crate::dibujo::Dibujo;
use crate::geo::P;

/// Las extensiones que se adjuntan.
pub const EXTENSIONES: &[&str] = &["png", "jpg", "jpeg", "bmp", "tif", "tiff", "gif"];

fn be16(b: &[u8], i: usize) -> Option<u32> {
    Some(u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]) as u32)
}

fn le16(b: &[u8], i: usize) -> Option<u32> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]) as u32)
}

fn be32(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

fn le32(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// **El tamano en pixeles** de una imagen por su cabecera (`None` si no se
/// reconoce el formato o esta danada).
pub fn tamano(b: &[u8]) -> Option<(u32, u32)> {
    let t = if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        (be32(b, 16)?, be32(b, 20)?)
    } else if b.starts_with(&[0xFF, 0xD8]) {
        // JPEG: el primer SOFn.
        let mut i = 2;
        loop {
            while *b.get(i)? != 0xFF {
                i += 1;
            }
            while *b.get(i)? == 0xFF {
                i += 1;
            }
            let m = *b.get(i)?;
            i += 1;
            if matches!(m, 0xD8 | 0x01 | 0xD0..=0xD7) {
                continue;
            }
            let largo = be16(b, i)? as usize;
            if matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
                break (be16(b, i + 5)?, be16(b, i + 3)?);
            }
            i += largo.max(2);
        }
    } else if b.starts_with(b"BM") {
        let w = le32(b, 18)? as i32;
        let h = le32(b, 22)? as i32;
        (w.unsigned_abs(), h.unsigned_abs())
    } else if b.starts_with(b"GIF8") {
        (le16(b, 6)?, le16(b, 8)?)
    } else if b.starts_with(b"II*\0") || b.starts_with(b"MM\0*") {
        let le = b[0] == b'I';
        let u16_ = |i: usize| if le { le16(b, i) } else { be16(b, i) };
        let u32_ = |i: usize| if le { le32(b, i) } else { be32(b, i) };
        let ifd = u32_(4)? as usize;
        let n = u16_(ifd)? as usize;
        let (mut w, mut h) = (None, None);
        for k in 0..n.min(512) {
            let e = ifd + 2 + k * 12;
            let tag = u16_(e)?;
            let tipo = u16_(e + 2)?;
            let v = if tipo == 3 {
                u16_(e + 8)?
            } else {
                u32_(e + 8)?
            };
            match tag {
                256 => w = Some(v),
                257 => h = Some(v),
                _ => {}
            }
        }
        (w?, h?)
    } else {
        return None;
    };
    (t.0 > 0 && t.1 > 0).then_some(t)
}

/// **Lee el tamano** de la imagen de `ruta`.
pub fn tamano_de(ruta: &Path) -> Result<(u32, u32), String> {
    use std::io::Read;
    let mut f = std::fs::File::open(ruta).map_err(|e| format!("no se puede abrir: {e}"))?;
    // La cabecera de un JPEG puede venir tras una miniatura EXIF: hasta 1 MB.
    let mut b = Vec::new();
    f.by_ref()
        .take(1 << 20)
        .read_to_end(&mut b)
        .map_err(|e| e.to_string())?;
    tamano(&b).ok_or_else(|| "no es una imagen PNG, JPG, BMP, TIF o GIF valida".to_string())
}

/// El diccionario `ACAD_IMAGE_DICT` (se crea si no esta).
fn diccionario_de_imagenes(doc: &mut CadDocument) -> Handle {
    let raiz = doc.header.named_objects_dict_handle;
    if let Some(ObjectType::Dictionary(d)) = doc.objects.get(&raiz)
        && let Some(h) = d.get("ACAD_IMAGE_DICT")
        && matches!(doc.objects.get(&h), Some(ObjectType::Dictionary(_)))
    {
        return h;
    }
    let h = doc.allocate_handle();
    let mut d = Dictionary::new();
    d.handle = h;
    d.owner = raiz;
    doc.objects.insert(h, ObjectType::Dictionary(d));
    if let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&raiz) {
        d.add_entry("ACAD_IMAGE_DICT", h);
    }
    h
}

/// Un nombre para la definicion que no este ya en el diccionario.
fn nombre_libre(doc: &CadDocument, dic: Handle, ruta: &Path) -> String {
    let base: String = ruta
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "IMAGEN".into())
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let Some(ObjectType::Dictionary(d)) = doc.objects.get(&dic) else {
        return base;
    };
    let mut nombre = base.clone();
    let mut k = 1;
    while d.get(&nombre).is_some() {
        k += 1;
        nombre = format!("{base}_{k}");
    }
    nombre
}

/// **Adjunta la imagen** de `ruta` con su esquina de abajo a la izquierda
/// en `p`, `ancho` unidades de ancho (el alto sale de sus pixeles) y girada
/// `giro` radianes. Un paso de deshacer (la definicion queda en el
/// diccionario aunque se deshaga, como en AutoCAD hasta limpiar).
pub fn adjuntar(
    d: &mut Dibujo,
    ruta: &Path,
    pixeles: (u32, u32),
    p: P,
    ancho: f64,
    giro: f64,
) -> Result<Handle, String> {
    if !(ancho.is_finite() && ancho > 0.0) {
        return Err("el ancho tiene que ser mayor que cero".into());
    }
    let ruta_txt = ruta.to_string_lossy().into_owned();
    let (wpx, hpx) = (pixeles.0 as f64, pixeles.1 as f64);
    let alto = ancho * hpx / wpx;
    let doc = &mut d.doc;
    let dic = diccionario_de_imagenes(doc);
    let nombre = nombre_libre(doc, dic, ruta);
    let mut def = ImageDefinition::new(ruta_txt.clone());
    def.handle = doc.allocate_handle();
    def.owner = dic;
    def.size_in_pixels = pixeles;
    def.pixel_size = (1.0, 1.0);
    def.is_loaded = true;
    let hd = def.handle;
    doc.objects.insert(hd, ObjectType::ImageDefinition(def));
    if let Some(ObjectType::Dictionary(dd)) = doc.objects.get_mut(&dic) {
        dd.add_entry(nombre, hd);
    }
    let hr = doc.allocate_handle();
    let mut im = RasterImage::with_size(
        &ruta_txt,
        Vector3::new(p[0], p[1], 0.0),
        wpx,
        hpx,
        ancho,
        alto,
    );
    im.definition_handle = Some(hd);
    im.definition_reactor_handle = Some(hr);
    if giro.abs() > 1e-15 {
        im.rotate(giro);
    }
    let capa = d.capa_actual();
    let h = d
        .editar("IMAGEN", |t| {
            t.anadir_en(EntityType::RasterImage(im), &capa)
        })
        .ok_or("no se pudo anadir la imagen")?;
    let mut reactor = ImageDefinitionReactor::new(h);
    reactor.handle = hr;
    reactor.owner = h;
    d.doc
        .objects
        .insert(hr, ObjectType::ImageDefinitionReactor(reactor));
    Ok(h)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un PNG de `w` por `h` (solo la cabecera; basta para el tamano).
    pub fn png(w: u32, h: u32) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[8, 2, 0, 0, 0, 0, 0, 0, 0]);
        v
    }

    #[test]
    fn cabeceras() {
        assert_eq!(tamano(&png(640, 480)), Some((640, 480)));
        let mut bmp = b"BM".to_vec();
        bmp.resize(26, 0);
        bmp[18..22].copy_from_slice(&100u32.to_le_bytes());
        bmp[22..26].copy_from_slice(&(-50i32).to_le_bytes());
        assert_eq!(tamano(&bmp), Some((100, 50)));
        let jpg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xC0, 0, 11, 8, 0, 20, 0, 30, 3, 0, 0, 0,
        ];
        assert_eq!(tamano(&jpg), Some((30, 20)));
        let gif = b"GIF89a\x0a\x00\x05\x00";
        assert_eq!(tamano(gif), Some((10, 5)));
        let mut tif = b"II*\0\x08\0\0\0".to_vec();
        tif.extend_from_slice(&2u16.to_le_bytes());
        for (tag, v) in [(256u16, 7u32), (257, 9)] {
            tif.extend_from_slice(&tag.to_le_bytes());
            tif.extend_from_slice(&4u16.to_le_bytes());
            tif.extend_from_slice(&1u32.to_le_bytes());
            tif.extend_from_slice(&v.to_le_bytes());
        }
        assert_eq!(tamano(&tif), Some((7, 9)));
        assert_eq!(tamano(b"nada"), None);
        assert_eq!(tamano(&png(0, 10)), None);
    }

    #[test]
    fn se_guarda_y_vuelve_en_dxf_y_dwg() {
        let dir = std::env::temp_dir().join(format!("cad-nucleo-imagen-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let foto = dir.join("foto.png");
        std::fs::write(&foto, png(200, 100)).unwrap();
        let mut d = Dibujo::nuevo();
        let px = tamano_de(&foto).unwrap();
        let h = adjuntar(&mut d, &foto, px, [10.0, 20.0], 50.0, 0.0).unwrap();
        let Some(EntityType::RasterImage(im)) = d.entidad(h) else {
            panic!()
        };
        assert!((im.width() - 50.0).abs() < 1e-9 && (im.height() - 25.0).abs() < 1e-9);
        for ext in ["dxf", "dwg"] {
            let ruta = dir.join(format!("con_imagen.{ext}"));
            d.guardar_como(&ruta).unwrap();
            let d2 = Dibujo::abrir(&ruta).unwrap();
            let im = d2
                .doc
                .model_space_entities()
                .find_map(|e| match e {
                    EntityType::RasterImage(i) => Some(i.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("la imagen vuelve del {ext}"));
            assert!((im.insertion_point.x - 10.0).abs() < 1e-9, "{ext}");
            assert!((im.width() - 50.0).abs() < 1e-6, "{ext}: {}", im.width());
            let def = im.definition_handle.and_then(|hd| d2.doc.objects.get(&hd));
            let Some(ObjectType::ImageDefinition(def)) = def else {
                panic!("la definicion vuelve del {ext}")
            };
            assert!(def.file_name.ends_with("foto.png"), "{ext}");
            assert_eq!(def.size_in_pixels, (200, 100), "{ext}");
        }
        // Una segunda con el mismo nombre de fichero no pisa la primera.
        adjuntar(&mut d, &foto, px, [0.0, 0.0], 10.0, 1.0).unwrap();
        assert!(adjuntar(&mut d, &foto, px, [0.0, 0.0], 0.0, 0.0).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
