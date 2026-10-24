//! **Una imagen del PDF comprimida con Flate, como PNG para la web.**
//!
//! La pagina web de un PDF (`plano_web`) solo pasaba las fotos JPEG, que van
//! tal cual; cualquier otra imagen mandaba la hoja entera como fotografia.
//! Y un trabajo hecho en Word lleva sus capturas y sus graficos en Flate:
//! en la tesis del usuario, 60 de 143 hojas salian como foto por una sola
//! imagen. Flate es lo mismo que comprime un PNG, asi que la imagen pasa a
//! PNG **sin perder un pixel**: se deshace el predictor si lo trae, se junta
//! su transparencia (`/SMask`) y se escribe. Nada de rasterizar la hoja.

use std::io::{Read, Write};

/// **Deshace el predictor PNG** de unas filas del PDF (`/Predictor` 10-15):
/// cada fila lleva delante su tipo, y `bpp` son los bytes de un pixel (los
/// filtros miran el pixel de la izquierda, no el byte). `None` si no cuadra.
pub(crate) fn desfiltrar(datos: &[u8], bytes_fila: usize, bpp: usize) -> Option<Vec<u8>> {
    if bytes_fila == 0 || bpp == 0 {
        return None;
    }
    let fila = bytes_fila + 1;
    let filas = datos.len() / fila;
    let mut salida = vec![0u8; filas * bytes_fila];
    for f in 0..filas {
        let tipo = datos[f * fila];
        let origen = &datos[f * fila + 1..(f + 1) * fila];
        let (antes, ahora) = salida.split_at_mut(f * bytes_fila);
        let arriba = if f > 0 {
            Some(&antes[(f - 1) * bytes_fila..])
        } else {
            None
        };
        let actual = &mut ahora[..bytes_fila];
        for i in 0..bytes_fila {
            let izq = if i >= bpp { actual[i - bpp] } else { 0 };
            let sup = arriba.map_or(0, |a| a[i]);
            let sup_izq = if i >= bpp {
                arriba.map_or(0, |a| a[i - bpp])
            } else {
                0
            };
            let x = origen[i];
            actual[i] = match tipo {
                0 => x,
                1 => x.wrapping_add(izq),
                2 => x.wrapping_add(sup),
                3 => x.wrapping_add(((izq as u16 + sup as u16) / 2) as u8),
                4 => {
                    let p = izq as i16 + sup as i16 - sup_izq as i16;
                    let (pa, pb, pc) = (
                        (p - izq as i16).abs(),
                        (p - sup as i16).abs(),
                        (p - sup_izq as i16).abs(),
                    );
                    let pred = if pa <= pb && pa <= pc {
                        izq
                    } else if pb <= pc {
                        sup
                    } else {
                        sup_izq
                    };
                    x.wrapping_add(pred)
                }
                _ => return None,
            };
        }
    }
    Some(salida)
}

/// Descomprime Flate. Un flujo cortado al final aun da lo que llevaba.
pub(crate) fn inflar(datos: &[u8]) -> Option<Vec<u8>> {
    let mut z = flate2::read::ZlibDecoder::new(datos);
    let mut fuera = Vec::new();
    if z.read_to_end(&mut fuera).is_err() && fuera.is_empty() {
        return None;
    }
    Some(fuera)
}

fn crc32(partes: &[&[u8]]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for p in partes {
        for &b in *p {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
    }
    !c
}

fn trozo(salida: &mut Vec<u8>, tipo: &[u8; 4], datos: &[u8]) {
    salida.extend_from_slice(&(datos.len() as u32).to_be_bytes());
    salida.extend_from_slice(tipo);
    salida.extend_from_slice(datos);
    salida.extend_from_slice(&crc32(&[tipo, datos]).to_be_bytes());
}

/// **Un PNG de 8 bits por canal** con estos pixeles, fila a fila y sin
/// relleno. `canales`: 1 gris, 2 gris con alfa, 3 color, 4 color con alfa;
/// con `paleta` (RGB, hasta 256 colores) los pixeles son indices y
/// `canales` es 1. `None` si las cuentas no cuadran.
pub(crate) fn escribir(
    ancho: u32,
    alto: u32,
    canales: u8,
    pixeles: &[u8],
    paleta: Option<&[u8]>,
) -> Option<Vec<u8>> {
    let bytes_fila = ancho as usize * canales as usize;
    if ancho == 0 || alto == 0 || pixeles.len() != bytes_fila * alto as usize {
        return None;
    }
    let tipo = match (canales, paleta.is_some()) {
        (1, true) => 3u8,
        (1, false) => 0,
        (2, false) => 4,
        (3, false) => 2,
        (4, false) => 6,
        _ => return None,
    };
    let mut filas = Vec::with_capacity(pixeles.len() + alto as usize);
    for f in pixeles.chunks_exact(bytes_fila) {
        filas.push(0u8);
        filas.extend_from_slice(f);
    }
    let mut z = flate2::write::ZlibEncoder::new(
        Vec::with_capacity(filas.len() / 2),
        flate2::Compression::default(),
    );
    z.write_all(&filas).ok()?;
    let comprimido = z.finish().ok()?;
    let mut s = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut cabecera = Vec::with_capacity(13);
    cabecera.extend_from_slice(&ancho.to_be_bytes());
    cabecera.extend_from_slice(&alto.to_be_bytes());
    cabecera.extend_from_slice(&[8, tipo, 0, 0, 0]);
    trozo(&mut s, b"IHDR", &cabecera);
    if let Some(p) = paleta {
        if p.is_empty() || p.len() % 3 != 0 || p.len() > 768 {
            return None;
        }
        trozo(&mut s, b"PLTE", p);
    }
    trozo(&mut s, b"IDAT", &comprimido);
    trozo(&mut s, b"IEND", &[]);
    Some(s)
}

/// Junta los pixeles de color con su alfa (uno por pixel): de 1 a 2 canales
/// o de 3 a 4. Con paleta hay que expandir antes (ver [`sin_paleta`]).
pub(crate) fn con_alfa(color: &[u8], canales: usize, alfa: &[u8]) -> Option<Vec<u8>> {
    if canales == 0 || color.len() != alfa.len() * canales {
        return None;
    }
    let mut out = Vec::with_capacity(color.len() + alfa.len());
    for (px, a) in color.chunks_exact(canales).zip(alfa) {
        out.extend_from_slice(px);
        out.push(*a);
    }
    Some(out)
}

/// Los indices de una paleta RGB, ya como color. Un indice pasado de la
/// paleta sale negro, como lo pinta un lector.
pub(crate) fn sin_paleta(indices: &[u8], paleta: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(indices.len() * 3);
    for &i in indices {
        let k = i as usize * 3;
        out.extend_from_slice(paleta.get(k..k + 3).unwrap_or(&[0, 0, 0]));
    }
    out
}

/// **De 1, 2 o 4 bits por pixel a 8**, fila a fila (cada fila empieza en
/// byte nuevo). En gris se estira a 0-255 (un bit: negro o blanco); los
/// `indices` de una paleta se quedan como estan.
pub(crate) fn a_ocho_bits(datos: &[u8], ancho: usize, bits: usize, indices: bool) -> Vec<u8> {
    let fila = (ancho * bits).div_ceil(8);
    let tope = (1u16 << bits) - 1;
    let mut out = Vec::with_capacity(ancho * datos.len() / fila.max(1));
    for f in datos.chunks(fila.max(1)) {
        for x in 0..ancho {
            let bit = x * bits;
            let byte = f.get(bit / 8).copied().unwrap_or(0);
            let v = (byte >> (8 - bits - bit % 8)) as u16 & tope;
            out.push(if indices {
                v as u8
            } else {
                (v * 255 / tope) as u8
            });
        }
    }
    out
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_predictor_se_deshace_mirando_el_pixel_de_la_izquierda_y_no_el_byte() {
        // Dos pixeles RGB por fila; la fila con «Sub» suma el pixel de al
        // lado (tres bytes atras).
        let datos = [
            1u8, 10, 20, 30, 1, 1, 1, /* fila 2, «Up» */ 2, 1, 1, 1, 0, 0, 0,
        ];
        let hecho = desfiltrar(&datos, 6, 3).unwrap();
        assert_eq!(hecho, vec![10, 20, 30, 11, 21, 31, 11, 21, 31, 11, 21, 31]);
        // Caso negativo: un tipo de filtro que no existe.
        assert!(desfiltrar(&[9, 0, 0, 0], 3, 3).is_none());
    }

    #[test]
    fn el_png_escrito_lo_lee_windows_con_sus_colores_y_su_alfa() {
        let color = [255u8, 0, 0, 0, 0, 255];
        let pixeles = con_alfa(&color, 3, &[255, 0]).unwrap();
        let png = escribir(2, 1, 4, &pixeles, None).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        let ruta = std::env::temp_dir().join(format!("pixpin-png-{}.png", std::process::id()));
        std::fs::write(&ruta, &png).unwrap();
        let _com = crate::ComDelHilo::nuevo();
        let img = pixpin_codec::imagen::cargar(&ruta).expect("Windows lo lee");
        let _ = std::fs::remove_file(&ruta);
        assert_eq!((img.ancho, img.alto), (2, 1));
        assert_eq!(&img.pixeles[..4], &[255, 0, 0, 255]);
        assert_eq!(img.pixeles[7], 0, "el segundo es transparente");
        // Caso negativo: pixeles que no cuadran con la medida.
        assert!(escribir(3, 1, 4, &pixeles, None).is_none());
    }

    #[test]
    fn una_paleta_se_escribe_o_se_expande() {
        let paleta = [0u8, 0, 0, 255, 255, 255];
        assert!(escribir(2, 1, 1, &[0, 1], Some(&paleta)).is_some());
        assert_eq!(sin_paleta(&[1, 7], &paleta), vec![255, 255, 255, 0, 0, 0]);
    }
}

#[cfg(test)]
mod pruebas_de_los_bits {
    use super::*;

    #[test]
    fn un_bit_por_pixel_sale_negro_o_blanco_y_cada_fila_empieza_en_byte_nuevo() {
        // Tres pixeles por fila: 101 y 011.
        assert_eq!(
            a_ocho_bits(&[0b1010_0000, 0b0110_0000], 3, 1, false),
            vec![255, 0, 255, 0, 255, 255]
        );
        // Los indices de una paleta se quedan como indices.
        assert_eq!(a_ocho_bits(&[0b1101_0000], 2, 2, true), vec![3, 1]);
    }
}
