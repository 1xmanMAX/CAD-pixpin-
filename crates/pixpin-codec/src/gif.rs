//! GIF animado escrito a mano, sin librerias.
//!
//! Grabar la pantalla en GIF con FFmpeg nos obligaria a decidir sobre la GPL
//! y a arrastrar decenas de megas de binario para producir un formato de
//! 1987. Un GIF es, por dentro, una paleta y un LZW: cabe en un fichero y le
//! basta con `std`. Este modulo es puro —entra un `&[ImagenRgba]`, sale un
//! `Vec<u8>`— porque quien graba decide donde escribirlo.
//!
//! Las decisiones que se tomaron, y por que:
//!
//! - **Una sola paleta global**, no una por fotograma. En una captura de
//!   pantalla los colores casi no cambian de un fotograma al siguiente, asi
//!   que una paleta local costaria 768 bytes por fotograma a cambio de nada.
//! - **Corte por la mediana** sobre un histograma de 5 bits por canal. El
//!   histograma exacto de una captura 4K puede tener millones de entradas y
//!   no cabe en memoria a lo tonto; con 5 bits son 32768 cubetas fijas (1 MB)
//!   y se guarda la suma exacta de cada canal, de modo que el color que
//!   representa a la cubeta es la media real de sus pixeles, no el centro de
//!   la rejilla. La interfaz de Windows es plana: pierde muy poco.
//! - **Se ignora el alfa de la entrada.** El GIF solo sabe de transparencia
//!   binaria y lo que se graba es la pantalla, que es opaca. Mezclar el alfa
//!   aqui seria inventarse un fondo.
//! - **Solo se escribe lo que cambia.** En una captura de pantalla casi todo
//!   esta quieto entre fotograma y fotograma, asi que escribir cada uno
//!   entero es guardar una y otra vez lo mismo. Tres cosas lo evitan, de mas
//!   a menos ahorro:
//!   1. Cada fotograma se recorta al rectangulo que cambio y se escribe como
//!      subimagen con su posicion, componiendose encima de la anterior con el
//!      metodo de descarte 1.
//!   2. Un fotograma identico al anterior no se escribe: su tiempo se suma al
//!      retardo del anterior. Por eso el retardo es por fotograma, no global.
//!   3. Dentro del rectangulo, los pixeles que aun asi no cambiaron se marcan
//!      con el indice transparente, y el LZW los reduce a una tirada larga de
//!      un solo simbolo.
//!
//! El resultado sigue siendo un GIF89a corriente: la composicion la hace el
//! visor, que es lo que el formato lleva pidiendole desde el 89.

use crate::imagen::ImagenRgba;
use std::collections::HashMap;

/// Ajustes de la animacion. El retardo va en centesimas de segundo porque es
/// la unidad del propio formato: traducir aqui desde milisegundos ocultaria
/// que el GIF no sabe de nada mas fino.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcionesGif {
    pub centesimas_por_fotograma: u16,
    pub bucle: bool,
}

impl Default for OpcionesGif {
    fn default() -> Self {
        // 10 centesimas son 10 fotogramas por segundo: el limite de lo que
        // los visores respetan de verdad y suficiente para una demo.
        OpcionesGif {
            centesimas_por_fotograma: 10,
            bucle: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorGif {
    #[error("no hay ningun fotograma que codificar")]
    SinFotogramas,
    #[error("el tamano {ancho}x{alto} no cabe en un GIF (ha de ir de 1x1 a 65535x65535)")]
    TamanoInvalido { ancho: u32, alto: u32 },
    #[error(
        "el fotograma {indice} mide {ancho}x{alto} y el primero mide {ancho_esperado}x{alto_esperado}"
    )]
    TamanosDistintos {
        indice: usize,
        ancho: u32,
        alto: u32,
        ancho_esperado: u32,
        alto_esperado: u32,
    },
    #[error("el fotograma {indice} tiene {tiene} bytes pero {ancho}x{alto} necesita {espera}")]
    TamanoIncoherente {
        indice: usize,
        ancho: u32,
        alto: u32,
        tiene: usize,
        espera: usize,
    },
}

/// Bits por canal del histograma. Cinco dan 32768 cubetas: un vector de 1 MB
/// que se reserva una vez por codificacion.
const BITS_CUBETA: u32 = 5;
const NUM_CUBETAS: usize = 1 << (BITS_CUBETA * 3);
const MAXIMO_COLORES: usize = 256;

/// En una animacion se reserva un indice de la paleta para decir "este pixel
/// no ha cambiado", asi que quedan 255 colores de verdad. Perder uno de 256
/// no se distingue a ojo; lo que se gana es que todo lo quieto que cae dentro
/// del rectangulo que si cambio pase a ser una tirada de un solo simbolo.
const MAXIMO_COLORES_ANIMADO: usize = 255;

/// El retardo de la extension de control grafico son 16 bits en centesimas de
/// segundo, o sea 655,35 s como mucho. Al fusionar fotogramas identicos el
/// retardo se acumula y puede pasarse de ahi; entonces hay que partirlo.
const MAXIMO_CENTESIMAS: u32 = u16::MAX as u32;

/// Que optimizaciones se aplican al ensamblar. En produccion van todas; el
/// campo existe para que las pruebas puedan medir cada una contra el camino
/// de fotogramas enteros, que es la unica forma de afirmar cuanto ahorra sin
/// compararla contra un numero copiado a mano.
#[derive(Clone, Copy)]
struct Tecnicas {
    /// Recortar cada fotograma al rectangulo que cambio y fusionar los que no
    /// cambiaron en nada.
    diferencias: bool,
    /// Marcar como transparentes los pixeles quietos de dentro del recorte.
    transparencia: bool,
}

impl Tecnicas {
    const TODAS: Tecnicas = Tecnicas {
        diferencias: true,
        transparencia: true,
    };
    #[cfg(test)]
    const NINGUNA: Tecnicas = Tecnicas {
        diferencias: false,
        transparencia: false,
    };
    #[cfg(test)]
    const SOLO_DIFERENCIAS: Tecnicas = Tecnicas {
        diferencias: true,
        transparencia: false,
    };
}

/// Trozo de lienzo que ocupa una subimagen, en pixeles del fotograma entero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rectangulo {
    izquierda: u16,
    arriba: u16,
    ancho: u16,
    alto: u16,
}

/// Un fotograma ya comprimido al que todavia le falta saber cuanto dura: no
/// se sabe hasta haber mirado los siguientes, porque los identicos se le
/// suman. De ahi que se escriba siempre con un fotograma de retraso.
struct Pendiente {
    bloque: Vec<u8>,
    /// Indice con el que repintar el pixel (0,0) si hay que alargar la espera
    /// mas alla de lo que cabe en el campo de retardo.
    relleno: u8,
    retardo: u32,
}

/// Codifica los fotogramas como un unico GIF89a animado.
pub fn codificar(fotogramas: &[ImagenRgba], opciones: OpcionesGif) -> Result<Vec<u8>, ErrorGif> {
    codificar_con(fotogramas, opciones, Tecnicas::TODAS)
}

fn codificar_con(
    fotogramas: &[ImagenRgba],
    opciones: OpcionesGif,
    tecnicas: Tecnicas,
) -> Result<Vec<u8>, ErrorGif> {
    let (ancho, alto) = validar(fotogramas)?;

    // Un solo fotograma no tiene nada debajo por lo que asomar, asi que no
    // tiene sentido gastarle un color de la paleta a la transparencia.
    let hay_hueco = tecnicas.transparencia && fotogramas.len() > 1;

    let mut entradas = histograma(fotogramas);
    let maximo = if hay_hueco {
        MAXIMO_COLORES_ANIMADO
    } else {
        MAXIMO_COLORES
    };
    let paleta = paleta_por_mediana(&mut entradas, maximo);
    let tabla = tabla_de_busqueda(&entradas, &paleta);

    // El hueco va justo detras del ultimo color real: asi nunca choca con un
    // indice que la tabla de busqueda pueda devolver.
    let hueco = hay_hueco.then_some(paleta.len() as u8);
    let bits_paleta = bits_de_paleta(paleta.len() + usize::from(hay_hueco));
    // La especificacion prohibe un tamano de codigo minimo menor que 2: con
    // uno solo no quedarian huecos para los codigos de limpieza y de fin.
    let bits_codigo = bits_paleta.max(2);

    let mut salida = Vec::new();
    escribir_cabecera(&mut salida, ancho, alto, &paleta, bits_paleta);
    if opciones.bucle {
        escribir_netscape(&mut salida);
    }

    let entero = Rectangulo {
        izquierda: 0,
        arriba: 0,
        ancho,
        alto,
    };
    let centesimas = u32::from(opciones.centesimas_por_fotograma);
    // El primero se escribe entero por narices: es el unico que no tiene
    // ningun fotograma debajo sobre el que componerse.
    let mut anterior = indices_de(&fotogramas[0], &tabla);
    let mut pendiente = Pendiente {
        bloque: bloque_de_imagen(
            entero,
            &recortar(&anterior, None, ancho, entero, None),
            bits_codigo,
        ),
        relleno: hueco.unwrap_or(anterior[0]),
        retardo: centesimas,
    };

    for fotograma in &fotogramas[1..] {
        let actual = indices_de(fotograma, &tabla);
        let cambio = if tecnicas.diferencias {
            rectangulo_cambiado(&anterior, &actual, ancho)
        } else {
            Some(entero)
        };
        match cambio {
            // Fusion: un fotograma identico al anterior no aporta un solo
            // pixel, solo tiempo, y el tiempo se guarda en el de antes.
            None => pendiente.retardo += centesimas,
            Some(rectangulo) => {
                volcar(&mut salida, &pendiente, hueco, bits_codigo);
                let recorte = recortar(&actual, Some(&anterior), ancho, rectangulo, hueco);
                pendiente = Pendiente {
                    bloque: bloque_de_imagen(rectangulo, &recorte, bits_codigo),
                    relleno: hueco.unwrap_or(actual[0]),
                    retardo: centesimas,
                };
            }
        }
        anterior = actual;
    }
    volcar(&mut salida, &pendiente, hueco, bits_codigo);
    salida.push(0x3B);
    Ok(salida)
}

/// Medidas comunes a todos los fotogramas, o el motivo por el que no las hay.
fn validar(fotogramas: &[ImagenRgba]) -> Result<(u16, u16), ErrorGif> {
    let primero = fotogramas.first().ok_or(ErrorGif::SinFotogramas)?;
    let (ancho, alto) = (primero.ancho, primero.alto);
    if ancho == 0 || alto == 0 || ancho > u16::MAX as u32 || alto > u16::MAX as u32 {
        return Err(ErrorGif::TamanoInvalido { ancho, alto });
    }
    for (indice, fotograma) in fotogramas.iter().enumerate() {
        if fotograma.ancho != ancho || fotograma.alto != alto {
            return Err(ErrorGif::TamanosDistintos {
                indice,
                ancho: fotograma.ancho,
                alto: fotograma.alto,
                ancho_esperado: ancho,
                alto_esperado: alto,
            });
        }
        let espera = fotograma.bytes_esperados();
        if fotograma.pixeles.len() != espera {
            return Err(ErrorGif::TamanoIncoherente {
                indice,
                ancho: fotograma.ancho,
                alto: fotograma.alto,
                tiene: fotograma.pixeles.len(),
                espera,
            });
        }
    }
    Ok((ancho as u16, alto as u16))
}

// --- Diferencias entre fotogramas ---------------------------------------

fn indices_de(fotograma: &ImagenRgba, tabla: &[u8]) -> Vec<u8> {
    fotograma
        .pixeles
        .chunks_exact(4)
        .map(|pixel| tabla[clave(pixel[0], pixel[1], pixel[2])])
        .collect()
}

/// El rectangulo minimo que cubre todo lo que cambio, o `None` si no cambio
/// nada. Se compara sobre los indices de paleta y no sobre el RGBA de origen
/// porque dos colores distintos que caen en el mismo indice escriben el mismo
/// byte: mirar el original daria por cambiado lo que en el fichero no lo esta.
fn rectangulo_cambiado(anterior: &[u8], actual: &[u8], ancho: u16) -> Option<Rectangulo> {
    let ancho_fila = ancho as usize;
    let (mut izquierda, mut derecha) = (ancho_fila, 0usize);
    let (mut arriba, mut abajo) = (usize::MAX, 0usize);
    let filas = anterior
        .chunks_exact(ancho_fila)
        .zip(actual.chunks_exact(ancho_fila));
    for (y, (vieja, nueva)) in filas.enumerate() {
        // Descartar la fila entera de un golpe es lo que hace barato el caso
        // normal de una captura: casi todas las filas estan quietas.
        if vieja == nueva {
            continue;
        }
        let primero = vieja.iter().zip(nueva).position(|(a, b)| a != b);
        let ultimo = vieja
            .iter()
            .rev()
            .zip(nueva.iter().rev())
            .position(|(a, b)| a != b);
        let (Some(primero), Some(ultimo)) = (primero, ultimo) else {
            continue;
        };
        izquierda = izquierda.min(primero);
        derecha = derecha.max(ancho_fila - 1 - ultimo);
        arriba = arriba.min(y);
        abajo = abajo.max(y);
    }
    if arriba == usize::MAX {
        return None;
    }
    Some(Rectangulo {
        izquierda: izquierda as u16,
        arriba: arriba as u16,
        ancho: (derecha - izquierda + 1) as u16,
        alto: (abajo - arriba + 1) as u16,
    })
}

/// Los indices del rectangulo, fila a fila. Si se pasan el fotograma anterior
/// y un indice de hueco, los pixeles que dentro del rectangulo siguen igual se
/// sustituyen por el hueco.
fn recortar(
    actual: &[u8],
    anterior: Option<&[u8]>,
    ancho: u16,
    rectangulo: Rectangulo,
    hueco: Option<u8>,
) -> Vec<u8> {
    let ancho_fila = ancho as usize;
    let primera = rectangulo.izquierda as usize;
    let ultima = primera + rectangulo.ancho as usize;
    let arriba = rectangulo.arriba as usize;
    let mut salida = Vec::with_capacity(rectangulo.ancho as usize * rectangulo.alto as usize);
    for y in arriba..arriba + rectangulo.alto as usize {
        let franja = &actual[y * ancho_fila + primera..y * ancho_fila + ultima];
        match (anterior, hueco) {
            (Some(previo), Some(vacio)) => {
                let vieja = &previo[y * ancho_fila + primera..y * ancho_fila + ultima];
                salida.extend(
                    franja
                        .iter()
                        .zip(vieja)
                        .map(|(&nuevo, &viejo)| if nuevo == viejo { vacio } else { nuevo }),
                );
            }
            _ => salida.extend_from_slice(franja),
        }
    }
    salida
}

/// Escribe el fotograma que estaba esperando a saber cuanto dura.
fn volcar(salida: &mut Vec<u8>, pendiente: &Pendiente, hueco: Option<u8>, bits_codigo: u8) {
    let primero = pendiente.retardo.min(MAXIMO_CENTESIMAS);
    escribir_control_grafico(salida, primero as u16, hueco);
    salida.extend_from_slice(&pendiente.bloque);
    let mut resto = pendiente.retardo - primero;
    while resto > 0 {
        let trozo = resto.min(MAXIMO_CENTESIMAS);
        escribir_control_grafico(salida, trozo as u16, hueco);
        // Una subimagen de un pixel que repinta lo que ya habia en (0,0): no
        // cambia nada de lo que se ve, solo estira la espera en tramos que si
        // caben en los 16 bits del campo de retardo.
        let punto = Rectangulo {
            izquierda: 0,
            arriba: 0,
            ancho: 1,
            alto: 1,
        };
        salida.extend_from_slice(&bloque_de_imagen(punto, &[pendiente.relleno], bits_codigo));
        resto -= trozo;
    }
}

// --- Cuantizacion -------------------------------------------------------

/// Cubeta del histograma ya resumida: su color medio y cuantos pixeles cayeron
/// en ella. El corte por la mediana trabaja sobre estas, no sobre los pixeles.
#[derive(Clone, Copy)]
struct Entrada {
    color: [u8; 3],
    cuenta: u64,
    clave: u32,
}

#[derive(Clone, Copy, Default)]
struct Acumulador {
    cuenta: u64,
    suma: [u64; 3],
}

fn clave(r: u8, g: u8, b: u8) -> usize {
    let desplazamiento = 8 - BITS_CUBETA;
    let (r, g, b) = (
        (r as usize) >> desplazamiento,
        (g as usize) >> desplazamiento,
        (b as usize) >> desplazamiento,
    );
    (r << (BITS_CUBETA * 2)) | (g << BITS_CUBETA) | b
}

fn histograma(fotogramas: &[ImagenRgba]) -> Vec<Entrada> {
    let mut cubetas = vec![Acumulador::default(); NUM_CUBETAS];
    for fotograma in fotogramas {
        for pixel in fotograma.pixeles.chunks_exact(4) {
            let cubeta = &mut cubetas[clave(pixel[0], pixel[1], pixel[2])];
            cubeta.cuenta += 1;
            for (suma, canal) in cubeta.suma.iter_mut().zip(pixel) {
                *suma += *canal as u64;
            }
        }
    }
    cubetas
        .iter()
        .enumerate()
        .filter(|(_, cubeta)| cubeta.cuenta > 0)
        .map(|(indice, cubeta)| Entrada {
            color: [
                (cubeta.suma[0] / cubeta.cuenta) as u8,
                (cubeta.suma[1] / cubeta.cuenta) as u8,
                (cubeta.suma[2] / cubeta.cuenta) as u8,
            ],
            cuenta: cubeta.cuenta,
            clave: indice as u32,
        })
        .collect()
}

/// Corte por la mediana: se parte una y otra vez la caja con mas pixeles
/// hasta tener `maximo` cajas, y cada caja aporta un color a la paleta.
/// Reordena `entradas`, porque las cajas son tramos contiguos de ese vector.
fn paleta_por_mediana(entradas: &mut [Entrada], maximo: usize) -> Vec<[u8; 3]> {
    if entradas.is_empty() {
        // Solo pasa si no hay pixeles, y validar() ya lo impide; aun asi la
        // paleta nunca puede quedar vacia o el GIF no tendria tabla de color.
        return vec![[0, 0, 0]];
    }
    let mut cajas = vec![(0usize, entradas.len())];
    while cajas.len() < maximo {
        // Se parte la caja mas poblada: es la que mas pixeles empeora si se
        // queda con un solo color para todos ellos.
        let mut elegida: Option<(usize, u64)> = None;
        for (indice, &(inicio, fin)) in cajas.iter().enumerate() {
            if fin - inicio < 2 {
                continue;
            }
            let peso: u64 = entradas[inicio..fin].iter().map(|e| e.cuenta).sum();
            if elegida.is_none_or(|(_, mejor)| peso > mejor) {
                elegida = Some((indice, peso));
            }
        }
        let Some((indice, _)) = elegida else {
            // Todas las cajas tienen un solo color: la imagen tiene menos
            // colores que el maximo y ya no hay nada que repartir.
            break;
        };
        let (inicio, fin) = cajas[indice];
        let corte = inicio + partir(&mut entradas[inicio..fin]);
        cajas[indice] = (inicio, corte);
        cajas.push((corte, fin));
    }
    cajas
        .iter()
        .map(|&(inicio, fin)| color_medio(&entradas[inicio..fin]))
        .collect()
}

/// Ordena la caja por su canal mas ancho y devuelve por donde partirla, en
/// indices relativos a la propia caja. Siempre deja al menos un color a cada
/// lado, o el bucle de arriba no terminaria.
fn partir(caja: &mut [Entrada]) -> usize {
    let canal = canal_mas_ancho(caja);
    caja.sort_unstable_by_key(|entrada| entrada.color[canal]);
    let total: u64 = caja.iter().map(|entrada| entrada.cuenta).sum();
    let mut acumulado = 0u64;
    for (indice, entrada) in caja.iter().enumerate() {
        acumulado += entrada.cuenta;
        if acumulado * 2 >= total {
            return (indice + 1).clamp(1, caja.len() - 1);
        }
    }
    caja.len() / 2
}

fn canal_mas_ancho(caja: &[Entrada]) -> usize {
    let mut mejor = 0;
    let mut mejor_rango = 0u8;
    for canal in 0..3 {
        let minimo = caja.iter().map(|e| e.color[canal]).min().unwrap_or(0);
        let maximo = caja.iter().map(|e| e.color[canal]).max().unwrap_or(0);
        if maximo - minimo >= mejor_rango {
            mejor_rango = maximo - minimo;
            mejor = canal;
        }
    }
    mejor
}

/// Media de la caja pesada por pixeles: un color que aparece mil veces manda
/// mas sobre el resultado que otro que aparece una.
fn color_medio(caja: &[Entrada]) -> [u8; 3] {
    let total: u64 = caja.iter().map(|entrada| entrada.cuenta).sum();
    if total == 0 {
        return [0, 0, 0];
    }
    let mut color = [0u8; 3];
    for (canal, valor) in color.iter_mut().enumerate() {
        let suma: u64 = caja
            .iter()
            .map(|entrada| entrada.color[canal] as u64 * entrada.cuenta)
            .sum();
        // Se redondea al entero mas cercano en vez de truncar: truncar
        // oscurece toda la imagen medio nivel.
        *valor = ((suma + total / 2) / total) as u8;
    }
    color
}

/// Para cada cubeta con pixeles, el color de la paleta que menos se le parece
/// de lejos. Se resuelve una vez por cubeta —como mucho 32768— y no por
/// pixel, que en 4K serian ocho millones de busquedas entre 256 colores.
fn tabla_de_busqueda(entradas: &[Entrada], paleta: &[[u8; 3]]) -> Vec<u8> {
    let mut tabla = vec![0u8; NUM_CUBETAS];
    for entrada in entradas {
        let mut mejor = 0usize;
        let mut mejor_distancia = u32::MAX;
        for (indice, color) in paleta.iter().enumerate() {
            let distancia = distancia_al_cuadrado(entrada.color, *color);
            if distancia < mejor_distancia {
                mejor_distancia = distancia;
                mejor = indice;
            }
        }
        tabla[entrada.clave as usize] = mejor as u8;
    }
    tabla
}

fn distancia_al_cuadrado(uno: [u8; 3], otro: [u8; 3]) -> u32 {
    (0..3)
        .map(|canal| {
            let diferencia = uno[canal] as i32 - otro[canal] as i32;
            (diferencia * diferencia) as u32
        })
        .sum()
}

/// Bits que hacen falta para indexar la paleta. La tabla de color del GIF
/// siempre ocupa una potencia de dos, con un minimo de dos colores.
fn bits_de_paleta(colores: usize) -> u8 {
    let mut bits = 1u8;
    while (1usize << bits) < colores {
        bits += 1;
    }
    bits
}

// --- Ensamblado del fichero ---------------------------------------------

fn escribir_u16(salida: &mut Vec<u8>, valor: u16) {
    salida.extend_from_slice(&valor.to_le_bytes());
}

fn escribir_cabecera(
    salida: &mut Vec<u8>,
    ancho: u16,
    alto: u16,
    paleta: &[[u8; 3]],
    bits_paleta: u8,
) {
    salida.extend_from_slice(b"GIF89a");
    escribir_u16(salida, ancho);
    escribir_u16(salida, alto);
    // Hay tabla global (bit 7), la profundidad de origen se declara igual que
    // la de la paleta, no se ordena por frecuencia (bit 3) y los tres bits
    // bajos dicen que la tabla tiene 2^(n+1) colores.
    let campo = (bits_paleta - 1) & 0b111;
    salida.push(0b1000_0000 | (campo << 4) | campo);
    salida.push(0); // Color de fondo: el primero de la paleta.
    salida.push(0); // Relacion de aspecto: pixeles cuadrados.
    for indice in 0..(1usize << bits_paleta) {
        let color = paleta.get(indice).copied().unwrap_or([0, 0, 0]);
        salida.extend_from_slice(&color);
    }
}

/// La extension de aplicacion NETSCAPE2.0, que es como se pide un bucle
/// infinito: el formato nunca tuvo una forma oficial de decirlo.
fn escribir_netscape(salida: &mut Vec<u8>) {
    salida.extend_from_slice(&[0x21, 0xFF, 0x0B]);
    salida.extend_from_slice(b"NETSCAPE2.0");
    salida.extend_from_slice(&[0x03, 0x01]);
    escribir_u16(salida, 0); // Cero repeticiones significa "sin fin".
    salida.push(0);
}

fn escribir_control_grafico(salida: &mut Vec<u8>, centesimas: u16, hueco: Option<u8>) {
    salida.extend_from_slice(&[0x21, 0xF9, 0x04]);
    // Metodo de descarte 1 ("dejar lo pintado"): es lo que hace que una
    // subimagen se componga encima de lo anterior en vez de borrarlo, y sin
    // eso no se podria escribir solo el rectangulo que cambio.
    salida.push(0b0000_0100 | u8::from(hueco.is_some()));
    escribir_u16(salida, centesimas);
    salida.push(hueco.unwrap_or(0));
    salida.push(0);
}

fn bloque_de_imagen(rectangulo: Rectangulo, indices: &[u8], bits_codigo: u8) -> Vec<u8> {
    let mut salida = Vec::new();
    salida.push(0x2C);
    escribir_u16(&mut salida, rectangulo.izquierda);
    escribir_u16(&mut salida, rectangulo.arriba);
    escribir_u16(&mut salida, rectangulo.ancho);
    escribir_u16(&mut salida, rectangulo.alto);
    salida.push(0); // Sin tabla local ni entrelazado.
    salida.push(bits_codigo);
    let comprimido = comprimir_lzw(indices, bits_codigo);
    for trozo in comprimido.chunks(255) {
        salida.push(trozo.len() as u8);
        salida.extend_from_slice(trozo);
    }
    salida.push(0); // Sub-bloque vacio: fin de los datos de la imagen.
    salida
}

// --- LZW ----------------------------------------------------------------

/// Los codigos se empaquetan de bit menos significativo a mas significativo y
/// pueden cruzar la frontera del byte, asi que hace falta un acumulador.
#[derive(Default)]
struct EscritorBits {
    bytes: Vec<u8>,
    acumulador: u32,
    bits: u8,
}

impl EscritorBits {
    fn escribir(&mut self, codigo: u16, ancho: u8) {
        self.acumulador |= (codigo as u32) << self.bits;
        self.bits += ancho;
        while self.bits >= 8 {
            self.bytes.push((self.acumulador & 0xFF) as u8);
            self.acumulador >>= 8;
            self.bits -= 8;
        }
    }

    fn terminar(mut self) -> Vec<u8> {
        if self.bits > 0 {
            self.bytes.push((self.acumulador & 0xFF) as u8);
        }
        self.bytes
    }
}

/// LZW del GIF, que no es el de TIFF: aqui el ancho de codigo crece cuando el
/// *siguiente* codigo por repartir ya no cabe, sin el desfase de una unidad
/// que usa TIFF. El descodificador va siempre una entrada por detras del
/// codificador, y de ahi que la comprobacion se haga justo despues de emitir.
fn comprimir_lzw(indices: &[u8], bits_codigo: u8) -> Vec<u8> {
    let limpieza = 1u16 << bits_codigo;
    let fin = limpieza + 1;
    let mut escritor = EscritorBits::default();
    let mut ancho = bits_codigo + 1;
    let mut siguiente = fin + 1;
    let mut diccionario: HashMap<(u16, u8), u16> = HashMap::new();

    // El flujo empieza siempre por una limpieza para que el descodificador
    // no dependa de lo que hubiera antes.
    escritor.escribir(limpieza, ancho);

    let mut resto = indices.iter();
    let Some(&primero) = resto.next() else {
        escritor.escribir(fin, ancho);
        return escritor.terminar();
    };
    let mut actual = primero as u16;
    for &indice in resto {
        if let Some(&codigo) = diccionario.get(&(actual, indice)) {
            actual = codigo;
            continue;
        }
        escritor.escribir(actual, ancho);
        if siguiente == 1u16 << ancho {
            if ancho < 12 {
                ancho += 1;
                diccionario.insert((actual, indice), siguiente);
                siguiente += 1;
            } else {
                // Diccionario lleno y sin bits para crecer: se limpia y se
                // vuelve a empezar. No se anade nada tras la limpieza porque
                // el descodificador trata el codigo siguiente como raiz.
                escritor.escribir(limpieza, ancho);
                diccionario.clear();
                ancho = bits_codigo + 1;
                siguiente = fin + 1;
            }
        } else {
            diccionario.insert((actual, indice), siguiente);
            siguiente += 1;
        }
        actual = indice as u16;
    }
    escritor.escribir(actual, ancho);
    // Tras leer ese ultimo codigo el descodificador aun anade una entrada, y
    // con ella puede ensancharse: el codigo de fin ha de ir ya con el ancho
    // nuevo o lo leeria descolocado.
    if siguiente == 1u16 << ancho && ancho < 12 {
        ancho += 1;
    }
    escritor.escribir(fin, ancho);
    escritor.terminar()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn imagen(ancho: u32, alto: u32, colorear: impl Fn(u32, u32) -> [u8; 4]) -> ImagenRgba {
        let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
        for y in 0..alto {
            for x in 0..ancho {
                pixeles.extend_from_slice(&colorear(x, y));
            }
        }
        ImagenRgba {
            ancho,
            alto,
            pixeles,
        }
    }

    fn plana(ancho: u32, alto: u32, color: [u8; 4]) -> ImagenRgba {
        imagen(ancho, alto, |_, _| color)
    }

    /// Generador deterministico: las pruebas del LZW necesitan una entrada
    /// larga y poco comprimible, pero que sea siempre la misma.
    fn pseudoaleatorios(cuantos: usize, modulo: u32) -> Vec<u8> {
        let mut estado = 0x1234_5678u32;
        (0..cuantos)
            .map(|_| {
                estado = estado.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((estado >> 16) % modulo) as u8
            })
            .collect()
    }

    // --- Secuencia sintetica de captura de pantalla ---------------------

    /// Medidas y duracion parecidas a las de la grabacion real que motivo la
    /// optimizacion (587x248, 120 fotogramas): un escritorio quieto con una
    /// sola cosa moviendose encima.
    const ANCHO_PANTALLA: u32 = 600;
    const ALTO_PANTALLA: u32 = 250;
    const FOTOGRAMAS_PANTALLA: usize = 60;
    /// Lo que se mueve: un recuadro pequeno, del tamano de un cursor con su
    /// tooltip. Si ocupara media pantalla no habria nada que ahorrar y la
    /// prueba no estaria midiendo un caso real.
    const ANCHO_MOVIL: u32 = 26;
    const ALTO_MOVIL: u32 = 18;

    /// Fondo fijo con textura: barra de titulo, panel lateral, renglones de
    /// texto y un recuadro con degradado, que es lo que en una captura de
    /// verdad obliga a la paleta a usar sus 256 colores.
    fn fondo(x: u32, y: u32) -> [u8; 4] {
        if y < 26 {
            [58, 62, 70, 255]
        } else if x < 118 {
            if y % 32 < 3 {
                [206, 209, 216, 255]
            } else {
                [235, 236, 240, 255]
            }
        } else if x >= 400 && y >= 130 {
            [
                (x % 251) as u8,
                ((y * 3) % 241) as u8,
                ((x + y) % 233) as u8,
                255,
            ]
        } else if y % 15 < 2 && (x + y) % 89 > 11 {
            [70, 74, 82, 255]
        } else {
            [252, 252, 252, 255]
        }
    }

    fn secuencia_de_pantalla(cuantos: usize) -> Vec<ImagenRgba> {
        (0..cuantos)
            .map(|n| {
                let paso = n as u32;
                let movil_x = 130 + (paso * 7) % (ANCHO_PANTALLA - 160 - ANCHO_MOVIL);
                let movil_y = 40 + (paso * 3) % (ALTO_PANTALLA - 60 - ALTO_MOVIL);
                imagen(ANCHO_PANTALLA, ALTO_PANTALLA, |x, y| {
                    if x >= movil_x
                        && x < movil_x + ANCHO_MOVIL
                        && y >= movil_y
                        && y < movil_y + ALTO_MOVIL
                    {
                        [250, 200, 40, 255]
                    } else {
                        fondo(x, y)
                    }
                })
            })
            .collect()
    }

    /// La misma pantalla pero con dos cosas moviendose en esquinas opuestas.
    /// Es lo normal en una captura de verdad —el cursor por un lado, el punto
    /// de insercion parpadeando por otro— y el rectangulo que cubre a las dos
    /// es casi la pantalla entera aunque entre ellas no cambie nada.
    fn secuencia_con_dos_moviles(cuantos: usize) -> Vec<ImagenRgba> {
        (0..cuantos)
            .map(|n| {
                let paso = n as u32;
                let (izquierdo_x, izquierdo_y) = (130 + (paso * 7) % 200, 40 + (paso * 3) % 60);
                let (derecho_x, derecho_y) = (430 + (paso * 5) % 120, 160 + (paso * 2) % 50);
                imagen(ANCHO_PANTALLA, ALTO_PANTALLA, |x, y| {
                    let dentro = |esquina_x: u32, esquina_y: u32| {
                        x >= esquina_x
                            && x < esquina_x + ANCHO_MOVIL
                            && y >= esquina_y
                            && y < esquina_y + ALTO_MOVIL
                    };
                    if dentro(izquierdo_x, izquierdo_y) || dentro(derecho_x, derecho_y) {
                        [250, 200, 40, 255]
                    } else {
                        fondo(x, y)
                    }
                })
            })
            .collect()
    }

    // --- Lector de GIF minimo, solo para poder afirmar cosas -------------

    struct BloqueImagen {
        izquierda: u16,
        arriba: u16,
        ancho: u16,
        alto: u16,
        bits_codigo: u8,
        datos: Vec<u8>,
    }

    struct GifLeido {
        ancho: u16,
        alto: u16,
        colores_paleta: usize,
        paleta: Vec<[u8; 3]>,
        retardos: Vec<u16>,
        descartes: Vec<u8>,
        huecos: Vec<Option<u8>>,
        imagenes: Vec<BloqueImagen>,
        bucle: bool,
    }

    /// Recorre los bloques de verdad en vez de buscar bytes sueltos: 0x2C y
    /// 0x21 aparecen tambien dentro de los datos comprimidos, asi que contar
    /// apariciones daria falsos positivos.
    fn leer_gif(bytes: &[u8]) -> GifLeido {
        assert_eq!(&bytes[..6], b"GIF89a");
        let ancho = u16::from_le_bytes([bytes[6], bytes[7]]);
        let alto = u16::from_le_bytes([bytes[8], bytes[9]]);
        let campo = bytes[10];
        assert_eq!(campo & 0b1000_0000, 0b1000_0000, "falta la tabla global");
        let colores_paleta = 1usize << ((campo & 0b111) + 1);
        let mut cursor = 13;
        let paleta = bytes[cursor..cursor + colores_paleta * 3]
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        cursor += colores_paleta * 3;

        let mut leido = GifLeido {
            ancho,
            alto,
            colores_paleta,
            paleta,
            retardos: Vec::new(),
            descartes: Vec::new(),
            huecos: Vec::new(),
            imagenes: Vec::new(),
            bucle: false,
        };
        loop {
            match bytes[cursor] {
                0x3B => {
                    assert_eq!(cursor, bytes.len() - 1, "hay basura tras el terminador");
                    return leido;
                }
                0x21 => {
                    let etiqueta = bytes[cursor + 1];
                    cursor += 2;
                    if etiqueta == 0xFF && bytes[cursor + 1..cursor + 12] == *b"NETSCAPE2.0" {
                        leido.bucle = true;
                    }
                    if etiqueta == 0xF9 {
                        let banderas = bytes[cursor + 1];
                        leido.descartes.push((banderas >> 2) & 0b111);
                        leido
                            .retardos
                            .push(u16::from_le_bytes([bytes[cursor + 2], bytes[cursor + 3]]));
                        leido.huecos.push(if banderas & 1 == 1 {
                            Some(bytes[cursor + 4])
                        } else {
                            None
                        });
                    }
                    cursor = saltar_subbloques(bytes, cursor);
                }
                0x2C => {
                    let izquierda = u16::from_le_bytes([bytes[cursor + 1], bytes[cursor + 2]]);
                    let arriba = u16::from_le_bytes([bytes[cursor + 3], bytes[cursor + 4]]);
                    let ancho = u16::from_le_bytes([bytes[cursor + 5], bytes[cursor + 6]]);
                    let alto = u16::from_le_bytes([bytes[cursor + 7], bytes[cursor + 8]]);
                    assert_eq!(bytes[cursor + 9] & 0b1000_0000, 0, "no se usa tabla local");
                    let bits_codigo = bytes[cursor + 10];
                    let mut cursor_datos = cursor + 11;
                    let mut datos = Vec::new();
                    while bytes[cursor_datos] != 0 {
                        let largo = bytes[cursor_datos] as usize;
                        datos.extend_from_slice(&bytes[cursor_datos + 1..cursor_datos + 1 + largo]);
                        cursor_datos += 1 + largo;
                    }
                    leido.imagenes.push(BloqueImagen {
                        izquierda,
                        arriba,
                        ancho,
                        alto,
                        bits_codigo,
                        datos,
                    });
                    cursor = cursor_datos + 1;
                }
                otro => panic!("bloque desconocido {otro:#04X} en {cursor}"),
            }
        }
    }

    /// Desde el primer byte de longitud, devuelve la posicion siguiente al
    /// sub-bloque vacio que cierra la cadena.
    fn saltar_subbloques(bytes: &[u8], mut cursor: usize) -> usize {
        while bytes[cursor] != 0 {
            cursor += 1 + bytes[cursor] as usize;
        }
        cursor + 1
    }

    /// Compone la animacion como lo haria un visor —cada subimagen encima de
    /// lo anterior, saltandose los pixeles transparentes— y devuelve el
    /// lienzo tras cada fotograma. Sin esto no se puede afirmar que recortar
    /// no haya roto lo que se ve.
    fn componer(leido: &GifLeido) -> Vec<Vec<[u8; 3]>> {
        let ancho = leido.ancho as usize;
        let mut lienzo = vec![[0u8; 3]; ancho * leido.alto as usize];
        let mut fotogramas = Vec::new();
        for (numero, bloque) in leido.imagenes.iter().enumerate() {
            let indices = descomprimir_lzw(&bloque.datos, bloque.bits_codigo);
            assert_eq!(
                indices.len(),
                bloque.ancho as usize * bloque.alto as usize,
                "la subimagen {numero} no trae los pixeles que dice"
            );
            let hueco = leido.huecos[numero];
            for (fila, renglon) in indices.chunks_exact(bloque.ancho as usize).enumerate() {
                for (columna, &indice) in renglon.iter().enumerate() {
                    if Some(indice) == hueco {
                        continue;
                    }
                    let x = bloque.izquierda as usize + columna;
                    let y = bloque.arriba as usize + fila;
                    lienzo[y * ancho + x] = leido.paleta[indice as usize];
                }
            }
            fotogramas.push(lienzo.clone());
        }
        fotogramas
    }

    /// Descompresor de LZW escrito aparte, a partir de la especificacion. Es
    /// la unica forma de comprobar que el compresor dice lo que cree decir:
    /// una prueba contra su propio codigo no probaria nada.
    fn descomprimir_lzw(datos: &[u8], bits_codigo: u8) -> Vec<u8> {
        let limpieza = 1u16 << bits_codigo;
        let fin = limpieza + 1;
        let mut tabla: Vec<Vec<u8>> = Vec::new();
        let reiniciar = |tabla: &mut Vec<Vec<u8>>| {
            tabla.clear();
            for valor in 0..=fin {
                tabla.push(if valor < limpieza {
                    vec![valor as u8]
                } else {
                    Vec::new()
                });
            }
        };
        reiniciar(&mut tabla);

        let mut ancho = bits_codigo + 1;
        let mut salida = Vec::new();
        let mut anterior: Option<u16> = None;
        let mut bit = 0usize;
        loop {
            if bit + ancho as usize > datos.len() * 8 {
                panic!("el flujo se acaba sin codigo de fin");
            }
            let mut codigo = 0u16;
            for desplazamiento in 0..ancho {
                let posicion = bit + desplazamiento as usize;
                let valor = (datos[posicion / 8] >> (posicion % 8)) & 1;
                codigo |= u16::from(valor) << desplazamiento;
            }
            bit += ancho as usize;

            if codigo == limpieza {
                reiniciar(&mut tabla);
                ancho = bits_codigo + 1;
                anterior = None;
                continue;
            }
            if codigo == fin {
                return salida;
            }
            let secuencia = match anterior {
                None => tabla[codigo as usize].clone(),
                Some(previo) => {
                    let mut nueva = tabla[previo as usize].clone();
                    if (codigo as usize) < tabla.len() {
                        nueva.push(tabla[codigo as usize][0]);
                    } else {
                        // El caso en que el codificador usa una entrada que el
                        // descodificador aun no ha creado.
                        nueva.push(tabla[previo as usize][0]);
                    }
                    tabla.push(nueva);
                    if tabla.len() == 1 << ancho && ancho < 12 {
                        ancho += 1;
                    }
                    if (codigo as usize) < tabla.len() - 1 {
                        tabla[codigo as usize].clone()
                    } else {
                        tabla[tabla.len() - 1].clone()
                    }
                }
            };
            salida.extend_from_slice(&secuencia);
            anterior = Some(codigo);
        }
    }

    #[test]
    fn un_gif_de_un_fotograma_empieza_por_la_firma_gif89a_y_acaba_en_el_terminador() {
        let bytes = codificar(&[plana(4, 3, [200, 30, 30, 255])], OpcionesGif::default()).unwrap();
        assert_eq!(&bytes[..6], b"GIF89a");
        assert_eq!(*bytes.last().unwrap(), 0x3B);
    }

    #[test]
    fn un_fotograma_de_un_pixel_declara_medir_uno_por_uno() {
        let bytes = codificar(&[plana(1, 1, [10, 20, 30, 255])], OpcionesGif::default()).unwrap();
        let leido = leer_gif(&bytes);
        assert_eq!((leido.ancho, leido.alto), (1, 1));
        assert_eq!(leido.imagenes.len(), 1);
        assert_eq!((leido.imagenes[0].ancho, leido.imagenes[0].alto), (1, 1));
    }

    #[test]
    fn una_imagen_de_miles_de_colores_se_queda_en_256_o_menos() {
        // Un degradado de 64x64 con los tres canales moviendose: bastantes
        // mas de 256 colores distintos.
        let fotograma = imagen(64, 64, |x, y| {
            [(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8, 255]
        });
        let distintos: std::collections::HashSet<[u8; 3]> = fotograma
            .pixeles
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2]])
            .collect();
        assert!(
            distintos.len() > 256,
            "la prueba necesita mas de 256 colores"
        );

        let bytes = codificar(&[fotograma], OpcionesGif::default()).unwrap();
        let leido = leer_gif(&bytes);
        assert!(leido.colores_paleta <= 256);
        assert_eq!(leido.paleta.len(), leido.colores_paleta);
        // Y ningun indice se sale de la tabla que se acaba de declarar.
        let indices = descomprimir_lzw(&leido.imagenes[0].datos, leido.imagenes[0].bits_codigo);
        assert_eq!(indices.len(), 64 * 64);
        assert!(indices.iter().all(|i| (*i as usize) < leido.colores_paleta));
    }

    #[test]
    fn dos_fotogramas_dan_dos_bloques_de_imagen_con_el_retardo_pedido() {
        let bytes = codificar(
            &[plana(2, 2, [255, 0, 0, 255]), plana(2, 2, [0, 0, 255, 255])],
            OpcionesGif {
                centesimas_por_fotograma: 7,
                bucle: true,
            },
        )
        .unwrap();
        let leido = leer_gif(&bytes);
        assert_eq!(leido.imagenes.len(), 2);
        assert_eq!(leido.retardos, [7, 7]);
    }

    #[test]
    fn el_bucle_infinito_solo_se_declara_si_se_pide() {
        let fotogramas = [plana(2, 2, [1, 2, 3, 255])];
        let con = codificar(
            &fotogramas,
            OpcionesGif {
                centesimas_por_fotograma: 5,
                bucle: true,
            },
        )
        .unwrap();
        assert!(leer_gif(&con).bucle);
        // Caso negativo: sin bucle no debe aparecer la extension NETSCAPE2.0,
        // o los visores repetirian la animacion para siempre.
        let sin = codificar(
            &fotogramas,
            OpcionesGif {
                centesimas_por_fotograma: 5,
                bucle: false,
            },
        )
        .unwrap();
        assert!(!leer_gif(&sin).bucle);
        assert!(!sin.windows(11).any(|v| v == b"NETSCAPE2.0"));
    }

    #[test]
    fn los_colores_de_una_imagen_sencilla_sobreviven_a_la_cuantizacion() {
        let fotograma = imagen(2, 2, |x, y| match (x, y) {
            (0, 0) => [255, 0, 0, 255],
            (1, 0) => [0, 255, 0, 255],
            (0, 1) => [0, 0, 255, 255],
            _ => [255, 255, 255, 255],
        });
        let bytes = codificar(&[fotograma], OpcionesGif::default()).unwrap();
        let leido = leer_gif(&bytes);
        let indices = descomprimir_lzw(&leido.imagenes[0].datos, leido.imagenes[0].bits_codigo);
        let pintados: Vec<[u8; 3]> = indices.iter().map(|i| leido.paleta[*i as usize]).collect();
        assert_eq!(
            pintados,
            [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]]
        );
    }

    #[test]
    fn el_lzw_comprimido_vuelve_a_ser_los_mismos_indices() {
        // Cuatro simbolos y un tamano de codigo minimo de 2: el ancho tiene
        // que crecer varias veces a lo largo del flujo.
        let indices = pseudoaleatorios(5_000, 4);
        let comprimido = comprimir_lzw(&indices, 2);
        assert_eq!(descomprimir_lzw(&comprimido, 2), indices);

        // Y algo cortisimo, donde el diccionario apenas arranca.
        let cortos = [0u8, 0, 1, 1, 0, 1, 0, 0];
        assert_eq!(descomprimir_lzw(&comprimir_lzw(&cortos, 2), 2), cortos);

        // Un solo simbolo: el compresor emite limpieza, codigo y fin.
        assert_eq!(descomprimir_lzw(&comprimir_lzw(&[3], 2), 2), [3]);
    }

    #[test]
    fn el_lzw_se_recupera_cuando_el_diccionario_se_llena() {
        // Con 256 simbolos al azar el diccionario llega a las 4096 entradas
        // mucho antes del final, asi que este flujo pasa por una limpieza.
        let indices = pseudoaleatorios(60_000, 256);
        let comprimido = comprimir_lzw(&indices, 8);
        assert_eq!(descomprimir_lzw(&comprimido, 8), indices);
    }

    #[test]
    fn los_indices_de_un_fotograma_se_leen_enteros_desde_el_gif() {
        let fotograma = imagen(37, 21, |x, y| {
            [(x * 7) as u8, (y * 11) as u8, ((x ^ y) * 3) as u8, 255]
        });
        let bytes = codificar(&[fotograma], OpcionesGif::default()).unwrap();
        let leido = leer_gif(&bytes);
        let indices = descomprimir_lzw(&leido.imagenes[0].datos, leido.imagenes[0].bits_codigo);
        assert_eq!(indices.len(), 37 * 21);
    }

    #[test]
    fn los_datos_comprimidos_van_en_bloques_de_255_como_mucho() {
        // Ruido de 200x200: seguro que no cabe en un solo sub-bloque.
        let ruido = pseudoaleatorios(200 * 200 * 4, 256);
        let fotograma = ImagenRgba {
            ancho: 200,
            alto: 200,
            pixeles: ruido,
        };
        let bytes = codificar(&[fotograma], OpcionesGif::default()).unwrap();
        // Si algun sub-bloque midiera de mas, el lector se perderia y el
        // terminador no caeria justo al final.
        let leido = leer_gif(&bytes);
        assert_eq!(leido.imagenes.len(), 1);
    }

    #[test]
    fn sin_fotogramas_no_hay_gif() {
        // Caso negativo: una grabacion que no llego a capturar nada no puede
        // dar un fichero, porque ni siquiera se sabria de que tamano es.
        assert!(matches!(
            codificar(&[], OpcionesGif::default()),
            Err(ErrorGif::SinFotogramas)
        ));
    }

    #[test]
    fn los_fotogramas_han_de_medir_todos_lo_mismo() {
        // Caso negativo: el GIF declara el tamano una sola vez, en la
        // cabecera; un fotograma de otro tamano no tendria donde decirlo.
        let error = codificar(
            &[plana(4, 4, [0, 0, 0, 255]), plana(4, 5, [0, 0, 0, 255])],
            OpcionesGif::default(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ErrorGif::TamanosDistintos {
                indice: 1,
                alto: 5,
                ..
            }
        ));
    }

    #[test]
    fn un_fotograma_vacio_o_gigante_no_se_codifica() {
        // Caso negativo: cero pixeles no es una imagen.
        assert!(matches!(
            codificar(
                &[ImagenRgba {
                    ancho: 0,
                    alto: 4,
                    pixeles: Vec::new()
                }],
                OpcionesGif::default()
            ),
            Err(ErrorGif::TamanoInvalido { .. })
        ));
        // Caso negativo: el formato guarda las medidas en 16 bits, asi que
        // pasarse de 65535 daria un fichero que miente sobre su tamano.
        assert!(matches!(
            codificar(
                &[ImagenRgba {
                    ancho: 70_000,
                    alto: 1,
                    pixeles: Vec::new()
                }],
                OpcionesGif::default()
            ),
            Err(ErrorGif::TamanoInvalido { .. })
        ));
    }

    #[test]
    fn un_fotograma_con_menos_pixeles_de_los_que_dice_no_se_codifica() {
        // Caso negativo: leer 3x3 pixeles de un buffer de 2 seria leer fuera,
        // asi que se rechaza antes de tocarlo.
        let error = codificar(
            &[ImagenRgba {
                ancho: 3,
                alto: 3,
                pixeles: vec![0; 8],
            }],
            OpcionesGif::default(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ErrorGif::TamanoIncoherente {
                indice: 0,
                tiene: 8,
                espera: 36,
                ..
            }
        ));
    }

    // --- Lo que cambia entre fotogramas ---------------------------------

    #[test]
    fn un_fondo_quieto_no_se_vuelve_a_escribir() {
        let fotogramas = secuencia_de_pantalla(FOTOGRAMAS_PANTALLA);
        let opciones = OpcionesGif::default();
        let enteros = codificar_con(&fotogramas, opciones, Tecnicas::NINGUNA).unwrap();
        let recortados = codificar(&fotogramas, opciones).unwrap();

        // El liston esta en 5 porque es lo que se pidio como minimo aceptable;
        // lo medido de verdad sobre esta secuencia son 514870 bytes escribiendo
        // los fotogramas enteros contra 18366 recortandolos, o sea x28,03.
        let factor = enteros.len() as f64 / recortados.len() as f64;
        assert!(
            factor >= 5.0,
            "escribir solo lo que cambia deberia reducir el fichero al menos 5 veces: \
             {} bytes enteros contra {} bytes recortados, factor {factor:.2}",
            enteros.len(),
            recortados.len()
        );

        // Y lo reducido sigue siendo un GIF con todos sus fotogramas: aqui no
        // hay dos consecutivos iguales, asi que no se fusiona ninguno.
        let leido = leer_gif(&recortados);
        assert_eq!(leido.imagenes.len(), FOTOGRAMAS_PANTALLA);
        assert_eq!((leido.ancho, leido.alto), (600, 250));
        assert_eq!(
            leido.retardos.iter().map(|r| u32::from(*r)).sum::<u32>(),
            FOTOGRAMAS_PANTALLA as u32 * u32::from(opciones.centesimas_por_fotograma),
            "el tiempo total de la animacion no puede cambiar al optimizarla"
        );
        // Todos menos el primero han de ser subimagenes mas pequenas que el
        // lienzo; si alguna volviera a medir lo mismo, el recorte no estaria
        // haciendo nada.
        assert!(
            leido.imagenes[1..]
                .iter()
                .all(|i| i.ancho < leido.ancho && i.alto < leido.alto),
            "algun fotograma se escribio entero"
        );
        assert!(
            leido.descartes.iter().all(|d| *d == 1),
            "las subimagenes solo se componen bien con el descarte 1"
        );
    }

    #[test]
    fn la_transparencia_paga_cuando_los_cambios_estan_desperdigados() {
        let opciones = OpcionesGif::default();
        let fotogramas = secuencia_con_dos_moviles(FOTOGRAMAS_PANTALLA);
        let sin = codificar_con(&fotogramas, opciones, Tecnicas::SOLO_DIFERENCIAS)
            .unwrap()
            .len();
        let con = codificar(&fotogramas, opciones).unwrap().len();
        // Con los cambios en dos esquinas el rectangulo que los cubre es casi
        // la pantalla entera; sin transparencia se reescribe todo lo quieto
        // que hay entre medias. Medido: 167969 bytes sin ella, 43469 con ella.
        assert!(
            con * 2 < sin,
            "la transparencia deberia recortar a menos de la mitad cuando los \
             cambios estan lejos: {sin} bytes sin ella, {con} con ella"
        );

        // Caso negativo: si lo unico que se mueve es un recuadro compacto, el
        // rectangulo ya viene ajustado y casi nada de lo que hay dentro esta
        // quieto, asi que la transparencia no ahorra nada y encima cuesta el
        // color de paleta que se le reservo. Se acepta ese peaje —medido en
        // un 3,6%— porque el caso de arriba es el que se da en una captura de
        // verdad y ahi vale casi cuatro veces el fichero.
        let compacta = secuencia_de_pantalla(FOTOGRAMAS_PANTALLA);
        let sin = codificar_con(&compacta, opciones, Tecnicas::SOLO_DIFERENCIAS)
            .unwrap()
            .len();
        let con = codificar(&compacta, opciones).unwrap().len();
        assert!(
            con < sin * 11 / 10,
            "el peaje de la transparencia en el caso compacto no deberia pasar \
             del 10%: {sin} bytes sin ella, {con} con ella"
        );
    }

    #[test]
    fn los_fotogramas_recortados_se_componen_hasta_dar_la_imagen_original() {
        // Pocos colores a proposito: asi la paleta los guarda exactos y se
        // puede comparar el resultado pixel a pixel en vez de "parecido".
        let colores: [[u8; 4]; 3] = [[20, 20, 20, 255], [240, 240, 240, 255], [200, 40, 40, 255]];
        let fotogramas: Vec<ImagenRgba> = (0..8)
            .map(|n: u32| {
                imagen(24, 16, |x, y| {
                    if x >= n * 2 && x < n * 2 + 5 && (4..9).contains(&y) {
                        colores[2]
                    } else if y < 3 {
                        colores[0]
                    } else {
                        colores[1]
                    }
                })
            })
            .collect();
        let bytes = codificar(&fotogramas, OpcionesGif::default()).unwrap();
        let leido = leer_gif(&bytes);
        let compuestos = componer(&leido);
        assert_eq!(compuestos.len(), fotogramas.len());
        for (numero, (compuesto, original)) in compuestos.iter().zip(&fotogramas).enumerate() {
            let esperado: Vec<[u8; 3]> = original
                .pixeles
                .chunks_exact(4)
                .map(|p| [p[0], p[1], p[2]])
                .collect();
            assert_eq!(
                *compuesto, esperado,
                "el fotograma {numero} no se compone bien"
            );
        }
    }

    #[test]
    fn solo_se_escribe_el_rectangulo_que_cambio() {
        // Un lienzo grande donde solo se mueve un cuadrado de 4x4 en el mismo
        // sitio: el recorte tiene que ser justo ese cuadrado.
        let hacer = |color: [u8; 4]| {
            imagen(40, 30, |x, y| {
                if (10..14).contains(&x) && (5..9).contains(&y) {
                    color
                } else {
                    [255, 255, 255, 255]
                }
            })
        };
        let bytes = codificar(
            &[hacer([255, 255, 255, 255]), hacer([0, 0, 0, 255])],
            OpcionesGif::default(),
        )
        .unwrap();
        let leido = leer_gif(&bytes);
        assert_eq!(leido.imagenes.len(), 2);
        let segundo = &leido.imagenes[1];
        assert_eq!(
            (
                segundo.izquierda,
                segundo.arriba,
                segundo.ancho,
                segundo.alto
            ),
            (10, 5, 4, 4)
        );
    }

    #[test]
    fn un_fotograma_identico_al_anterior_no_se_escribe_y_suma_su_retardo() {
        let quieto = plana(8, 8, [30, 90, 150, 255]);
        let otro = plana(8, 8, [200, 60, 20, 255]);
        let bytes = codificar(
            &[quieto.clone(), quieto.clone(), quieto.clone(), otro],
            OpcionesGif {
                centesimas_por_fotograma: 5,
                bucle: true,
            },
        )
        .unwrap();
        let leido = leer_gif(&bytes);
        assert_eq!(
            leido.imagenes.len(),
            2,
            "los tres quietos son un solo bloque"
        );
        assert_eq!(leido.retardos, [15, 5]);

        // Caso negativo: si los fotogramas si cambian, no se fusiona ninguno
        // aunque el cambio sea de un solo pixel.
        let mut cambiante = plana(8, 8, [30, 90, 150, 255]);
        let variados: Vec<ImagenRgba> = (0..3)
            .map(|n| {
                cambiante.pixeles[n * 4] = 250;
                cambiante.clone()
            })
            .collect();
        let leido = leer_gif(&codificar(&variados, OpcionesGif::default()).unwrap());
        assert_eq!(leido.imagenes.len(), 3);
    }

    #[test]
    fn el_retardo_fusionado_se_parte_cuando_no_cabe_en_16_bits() {
        // 700 fotogramas identicos a 100 centesimas son 70000, por encima de
        // las 65535 que caben en el campo: hay que repartirlo en dos esperas.
        let quieto = plana(4, 4, [10, 10, 10, 255]);
        let fotogramas = vec![quieto; 700];
        let bytes = codificar(
            &fotogramas,
            OpcionesGif {
                centesimas_por_fotograma: 100,
                bucle: true,
            },
        )
        .unwrap();
        let leido = leer_gif(&bytes);
        assert_eq!(leido.retardos, [65535, 4465]);
        assert_eq!(
            leido.retardos.iter().map(|r| u32::from(*r)).sum::<u32>(),
            70_000
        );
        // El segundo bloque solo existe para esperar: mide un pixel.
        assert_eq!(leido.imagenes.len(), 2);
        assert_eq!((leido.imagenes[1].ancho, leido.imagenes[1].alto), (1, 1));
        // Y no cambia lo que se ve: el pixel que repinta es el que ya habia.
        let compuestos = componer(&leido);
        assert_eq!(compuestos[0], compuestos[1]);
    }

    #[test]
    fn una_animacion_reserva_un_indice_para_lo_que_no_cambia_y_una_imagen_suelta_no() {
        let mut fotogramas = vec![plana(6, 6, [10, 20, 30, 255])];
        let leido = leer_gif(&codificar(&fotogramas, OpcionesGif::default()).unwrap());
        // Caso negativo: con un solo fotograma no hay nada debajo por lo que
        // asomar, asi que declarar transparencia solo gastaria un color.
        assert_eq!(leido.huecos, [None]);

        fotogramas.push(plana(6, 6, [200, 30, 30, 255]));
        let leido = leer_gif(&codificar(&fotogramas, OpcionesGif::default()).unwrap());
        assert!(leido.huecos.iter().all(|h| h.is_some()));
        // Y el indice del hueco cae dentro de la tabla declarada, o el visor
        // no sabria a que se refiere.
        assert!(
            leido
                .huecos
                .iter()
                .flatten()
                .all(|h| (*h as usize) < leido.colores_paleta)
        );
    }

    #[test]
    fn el_rectangulo_cambiado_encuentra_los_bordes_exactos() {
        // Un lienzo de 5x4 con dos pixeles tocados en esquinas distintas: el
        // rectangulo ha de ser el que cabe a los dos, no uno por cada uno.
        let ancho = 5usize;
        let anterior = vec![0u8; ancho * 4];
        let mut actual = anterior.clone();
        actual[ancho + 2] = 9;
        actual[3 * ancho + 4] = 9;
        assert_eq!(
            rectangulo_cambiado(&anterior, &actual, ancho as u16),
            Some(Rectangulo {
                izquierda: 2,
                arriba: 1,
                ancho: 3,
                alto: 3
            })
        );
        // Caso negativo: dos fotogramas iguales no tienen rectangulo, y de
        // ahi sale la fusion.
        assert_eq!(
            rectangulo_cambiado(&anterior, &anterior, ancho as u16),
            None
        );
    }
}
