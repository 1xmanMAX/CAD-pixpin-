//! La **tarjeta de Proyectos** de la ventana del chat, calcada de
//! `ui/Proyectos.kt` de PixPin Android: las cuentas, sin pintar nada.
//!
//! En el movil es un proyecto por pantalla que se pasa como los videos. En
//! el PC eso no se adaptaba a una ventana de escritorio (lo dijo el usuario:
//! «no se esta adaptando a la interfaz de una PC»), asi que la tarjeta vive
//! en el **panel derecho del chat**, el mismo reparto maestro-detalle: la
//! lista de proyectos a la izquierda y, en vez de la conversacion del
//! elegido, su tarjeta. Arriba a la izquierda el circulo de volver del chat;
//! debajo la tarjeta con su nombre, su portada grande y la tira de hojas, y
//! la barra de cristal con lo que se hace a diario. Con el panel mas ancho
//! que alto la barra se pone de pie en el lateral y, si la tarjeta tambien
//! lo es, reparte en dos columnas: el nombre y las miniaturas a un lado, la
//! portada al otro.
//!
//! Todas las medidas son los `dp` del movil, en pixeles logicos: se
//! multiplican por la escala de la ventana como el resto del chat.

use std::ops::Range;

use pixpin_geom::{Punto, Rect};

/// El margen de la tarjeta a los lados (`contentPadding` de 10).
pub const MARGEN: u32 = 10;
/// El hueco que la barra flotante necesita abajo (`SITIO_DE_LA_BARRA`).
pub const SITIO_BARRA: u32 = 80;
/// Y el que necesita de pie, a un lado (`SITIO_DE_LA_BARRA_DE_PIE`).
pub const SITIO_BARRA_DE_PIE: u32 = 78;
/// Lo mas baja que se deja la tarjeta: nombre, una portada que se reconozca
/// y la tira. En un panel mas bajo no se aplasta, se desplaza con la rueda.
pub const TARJETA_MINIMA: u32 = 380;
/// El relleno de la tarjeta por dentro (`padding(14.dp)`).
pub const RELLENO_TARJETA: u32 = 14;
/// Las esquinas de la tarjeta (`Card` de Material 3: 12).
pub const RADIO_TARJETA: u32 = 12;
/// Las esquinas de la portada (`RoundedCornerShape(10.dp)`).
pub const RADIO_PORTADA: u32 = 10;
/// Las esquinas de una miniatura (`RoundedCornerShape(6.dp)`).
pub const RADIO_HOJA: u32 = 6;
/// Una miniatura de la tira (`ANCHO_DE_HOJA` x `ALTO_DE_HOJA`).
pub const ANCHO_HOJA: u32 = 76;
pub const ALTO_HOJA: u32 = 104;
/// El aire de cada miniatura a la derecha y abajo (`padding(end = 8, bottom = 6)`).
pub const HOJA_AIRE_X: u32 = 8;
pub const HOJA_AIRE_Y: u32 = 6;
/// Lo que ocupa una fila de miniaturas con su nombre debajo (`ALTO_DE_FILA`).
pub const ALTO_FILA: u32 = ALTO_HOJA + 24;
/// Lo que ocupa una columna de miniaturas.
pub const ANCHO_COLUMNA: u32 = ANCHO_HOJA + HOJA_AIRE_X;
/// Los botones de icono de la cabecera de la tarjeta (rejilla y tres puntos).
pub const BOTON_ICONO: u32 = 38;
/// De lado, la columna del nombre y las miniaturas (`fillMaxWidth(0.42f)`).
pub const COLUMNA_APAISADA: u32 = 42;
/// Lo que separa esa columna de la portada (`Spacer(width = 12.dp)`).
pub const ENTRE_COLUMNAS: u32 = 12;
/// Entre la cabecera de la tarjeta y lo de debajo, de pie (10) y de lado (8).
pub const BAJO_CABECERA: u32 = 10;
pub const BAJO_CABECERA_APAISADA: u32 = 8;
/// Entre la portada y la tira (`Spacer(height = 8.dp)`).
pub const SOBRE_TIRA: u32 = 8;
/// Un boton de la barra de acciones: icono de 24 y palabra debajo
/// (`BotonDeAccion`, 66 de ancho y 6 de aire arriba y abajo).
pub const BOTON_ACCION_ANCHO: u32 = 66;
pub const BOTON_ACCION_ALTO: u32 = 55;
/// El relleno de la barra de cristal y lo que separa sus botones.
pub const BARRA_RELLENO: u32 = 6;
pub const BARRA_SEPARACION: u32 = 4;
/// Las esquinas de la barra de cristal (`formaDeBarra`, 28).
pub const RADIO_BARRA: u32 = 28;
/// De lado, la barra se aparta del borde (`padding(horizontal = 6.dp)`); de
/// pie, del de abajo (`padding(bottom = 10.dp)`).
pub const BARRA_AIRE_LADO: u32 = 6;
pub const BARRA_AIRE_ABAJO: u32 = 10;

/// Las letras, las de Material 3 que usa la pantalla.
pub const NOMBRE_TAM: f32 = 24.0; // headlineSmall
pub const DETALLE_TAM: f32 = 14.0; // bodyMedium
pub const ETIQUETA_TAM: f32 = 11.0; // labelSmall
pub const PORTADA_ETIQUETA_TAM: f32 = 12.0; // labelMedium
/// La letra de una nota en la tira y en la portada (`LETRA_DE_LA_MINIATURA`,
/// `LETRA_DE_LA_PORTADA`).
pub const NOTA_TAM: f32 = 9.0;
pub const NOTA_PORTADA_TAM: f32 = 13.0;

/// **Los pixeles de la portada siguen al hueco, por escalones**
/// (`PASOS_DE_NITIDEZ`): a medida exacta cada tamano de ventana pediria otra
/// pagina pintada y la cache se llenaria de versiones que no vuelven.
pub const PASOS_DE_NITIDEZ: [u32; 5] = [360, 720, 1080, 1440, 2000];

/// El ancho al que pintar la portada para un hueco de `px` pixeles.
pub fn nitidez_para(px: u32) -> u32 {
    PASOS_DE_NITIDEZ
        .iter()
        .copied()
        .find(|p| *p >= px)
        .unwrap_or(PASOS_DE_NITIDEZ[PASOS_DE_NITIDEZ.len() - 1])
}

fn e(v: u32, escala: u32) -> u32 {
    v * escala / 100
}

fn vacio() -> Rect {
    Rect {
        x: 0,
        y: 0,
        ancho: 0,
        alto: 0,
    }
}

/// Como queda repartido el panel derecho con la tarjeta de un proyecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panel {
    /// La franja de arriba, la de la cabecera del chat: ahi va volver.
    pub cabecera: Rect,
    /// Lo que se ve de la tarjeta: lo que se sale se recorta.
    pub hueco: Rect,
    /// Lo alta que es la tarjeta: la del hueco, o la minima si no cabe.
    pub alto_tarjeta: u32,
    /// El panel es mas ancho que alto: la barra va de pie a un lado.
    pub apaisada: bool,
    /// La barra de cristal con las acciones del proyecto.
    pub barra: Rect,
    botones: usize,
    escala: u32,
}

/// Reparte el panel derecho `area` (la columna del chat) con la franja de
/// cabecera de `alto_cabecera` pixeles arriba y `botones` acciones en la
/// barra flotante.
pub fn panel(area: Rect, alto_cabecera: u32, escala: u32, botones: usize) -> Panel {
    let alto_cabecera = alto_cabecera.min(area.alto);
    let cabecera = Rect {
        alto: alto_cabecera,
        ..area
    };
    let debajo = Rect {
        x: area.x,
        y: area.y + alto_cabecera as i32,
        ancho: area.ancho,
        alto: area.alto.saturating_sub(alto_cabecera),
    };
    let apaisada = debajo.ancho > debajo.alto;
    // El sitio de la barra: de pie a la derecha, tumbada abajo.
    let (derecha, abajo) = if apaisada {
        (e(SITIO_BARRA_DE_PIE, escala), e(MARGEN, escala))
    } else {
        (e(MARGEN, escala), e(SITIO_BARRA, escala))
    };
    let hueco = Rect {
        x: debajo.x + e(MARGEN, escala) as i32,
        y: debajo.y,
        ancho: debajo.ancho.saturating_sub(e(MARGEN, escala) + derecha),
        alto: debajo.alto.saturating_sub(abajo),
    };
    Panel {
        cabecera,
        hueco,
        alto_tarjeta: hueco.alto.max(e(TARJETA_MINIMA, escala)),
        apaisada,
        barra: barra_de_acciones(debajo, apaisada, botones, escala),
        botones,
        escala,
    }
}

fn barra_de_acciones(debajo: Rect, apaisada: bool, n: usize, escala: u32) -> Rect {
    if n == 0 {
        return vacio();
    }
    let n = n as u32;
    let largo = n * e(BOTON_ACCION_ANCHO, escala).max(1);
    let entre = (n - 1) * e(BARRA_SEPARACION, escala);
    let relleno = 2 * e(BARRA_RELLENO, escala);
    if apaisada {
        let ancho = e(BOTON_ACCION_ANCHO, escala) + relleno;
        let alto = n * e(BOTON_ACCION_ALTO, escala) + entre + relleno;
        Rect {
            x: debajo.derecha() - (e(BARRA_AIRE_LADO, escala) + ancho) as i32,
            y: debajo.y + (debajo.alto as i32 - alto as i32) / 2,
            ancho,
            alto,
        }
    } else {
        let ancho = largo + entre + relleno;
        let alto = e(BOTON_ACCION_ALTO, escala) + relleno;
        Rect {
            x: debajo.x + (debajo.ancho as i32 - ancho as i32) / 2,
            y: debajo.abajo() - (e(BARRA_AIRE_ABAJO, escala) + alto) as i32,
            ancho,
            alto,
        }
    }
}

impl Panel {
    /// Lo que se puede bajar la tarjeta: nada si cabe entera.
    pub fn tope(&self) -> i32 {
        self.alto_tarjeta.saturating_sub(self.hueco.alto) as i32
    }

    /// La tarjeta bajada `desplazamiento` pixeles (ya sujeto entre cero y el
    /// tope). Puede salirse del hueco: quien pinta recorta.
    pub fn tarjeta(&self, desplazamiento: i32) -> Rect {
        Rect {
            x: self.hueco.x,
            y: self.hueco.y - desplazamiento.clamp(0, self.tope()),
            ancho: self.hueco.ancho,
            alto: self.alto_tarjeta,
        }
    }

    /// El boton `n` de la barra de acciones.
    pub fn boton(&self, n: usize) -> Rect {
        if n >= self.botones || self.barra.ancho == 0 {
            return vacio();
        }
        let e = |v: u32| e(v, self.escala);
        let relleno = e(BARRA_RELLENO) as i32;
        let n = n as i32;
        if self.apaisada {
            Rect {
                x: self.barra.x + relleno,
                y: self.barra.y + relleno + n * (e(BOTON_ACCION_ALTO) + e(BARRA_SEPARACION)) as i32,
                ancho: e(BOTON_ACCION_ANCHO),
                alto: e(BOTON_ACCION_ALTO),
            }
        } else {
            Rect {
                x: self.barra.x
                    + relleno
                    + n * (e(BOTON_ACCION_ANCHO) + e(BARRA_SEPARACION)) as i32,
                y: self.barra.y + relleno,
                ancho: e(BOTON_ACCION_ANCHO),
                alto: e(BOTON_ACCION_ALTO),
            }
        }
    }

    /// Que boton de la barra hay bajo el punto.
    pub fn boton_en(&self, p: Punto) -> Option<usize> {
        (0..self.botones).find(|n| self.boton(*n).contiene(p))
    }
}

/// Como va repartida por dentro la tarjeta de un proyecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tarjeta {
    /// La tarjeta sin su relleno.
    pub contenido: Rect,
    /// Mas ancha que alta: nombre y miniaturas a un lado, portada al otro.
    pub apaisada: bool,
    /// El nombre y lo que lleva anotado debajo.
    pub texto: Rect,
    /// El boton de rejilla / una pagina (vacio sin hojas) y el de tres puntos.
    pub rejilla: Rect,
    pub menu: Rect,
    /// La portada, o la rejilla entera si esta puesta.
    pub hojas: Rect,
    /// Donde va la tira: una fila bajo la portada, o las filas bajo el nombre
    /// si la tarjeta es apaisada. Vacia en rejilla o sin hojas.
    pub tira: Rect,
}

/// Reparte la tarjeta `pagina`. `alto_texto` es lo que miden el nombre y su
/// linea de debajo, que solo sabe quien los compone.
pub fn tarjeta(
    pagina: Rect,
    escala: u32,
    alto_texto: u32,
    hay_hojas: bool,
    rejilla: bool,
) -> Tarjeta {
    let r = e(RELLENO_TARJETA, escala);
    let contenido = Rect {
        x: pagina.x + r as i32,
        y: pagina.y + r as i32,
        ancho: pagina.ancho.saturating_sub(2 * r),
        alto: pagina.alto.saturating_sub(2 * r),
    };
    let apaisada = contenido.ancho > contenido.alto;
    let columna = if apaisada {
        Rect {
            ancho: contenido.ancho * COLUMNA_APAISADA / 100,
            ..contenido
        }
    } else {
        contenido
    };
    let boton = e(BOTON_ICONO, escala);
    let alto_cabecera = alto_texto.max(boton).min(columna.alto);
    let menu = Rect {
        x: columna.derecha() - boton as i32,
        y: columna.y,
        ancho: boton,
        alto: boton,
    };
    let boton_rejilla = if hay_hojas {
        Rect {
            x: menu.x - boton as i32,
            ..menu
        }
    } else {
        vacio()
    };
    let hasta = if hay_hojas { boton_rejilla.x } else { menu.x };
    let texto = Rect {
        x: columna.x,
        y: columna.y,
        ancho: (hasta - columna.x).max(0) as u32,
        alto: alto_cabecera,
    };
    let con_tira = hay_hojas && !rejilla;
    let (hojas, tira) = if apaisada {
        let x = columna.derecha() + e(ENTRE_COLUMNAS, escala) as i32;
        let hojas = Rect {
            x,
            y: contenido.y,
            ancho: (contenido.derecha() - x).max(0) as u32,
            alto: contenido.alto,
        };
        let y = columna.y + (alto_cabecera + e(BAJO_CABECERA_APAISADA, escala)) as i32;
        let tira = if con_tira {
            Rect {
                x: columna.x,
                y,
                ancho: columna.ancho,
                alto: (columna.abajo() - y).max(0) as u32,
            }
        } else {
            vacio()
        };
        (hojas, tira)
    } else {
        let y = contenido.y + (alto_cabecera + e(BAJO_CABECERA, escala)) as i32;
        let resto = Rect {
            x: contenido.x,
            y,
            ancho: contenido.ancho,
            alto: (contenido.abajo() - y).max(0) as u32,
        };
        if con_tira {
            let fila = e(ALTO_FILA, escala).min(resto.alto);
            let tira = Rect {
                y: resto.abajo() - fila as i32,
                alto: fila,
                ..resto
            };
            let hojas = Rect {
                alto: resto.alto.saturating_sub(fila + e(SOBRE_TIRA, escala)),
                ..resto
            };
            (hojas, tira)
        } else {
            (resto, vacio())
        }
    };
    Tarjeta {
        contenido,
        apaisada,
        texto,
        rejilla: boton_rejilla,
        menu,
        hojas,
        tira,
    }
}

/// Como se reparten las miniaturas en su hueco: cuantas filas y columnas, y
/// en que orden van las hojas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mosaico {
    pub filas: u32,
    pub columnas: u32,
    /// Por columnas (la 1 y la 2 en la primera, la 3 y la 4 en la siguiente,
    /// `TiraEnFilas`) o por filas (la primera mitad arriba, `RejillaDeHojas`).
    pub por_columnas: bool,
    cuantas: usize,
}

fn columnas_para(cuantas: usize, filas: u32) -> u32 {
    (cuantas as u32).div_ceil(filas.max(1))
}

/// Una sola fila, la tira de debajo de la portada (`TiraDeHojas`).
pub fn tira(cuantas: usize) -> Mosaico {
    Mosaico {
        filas: 1,
        columnas: cuantas as u32,
        por_columnas: true,
        cuantas,
    }
}

/// Las miniaturas bajo el nombre de una tarjeta apaisada (`TiraEnFilas`):
/// tantas filas como quepan, hasta cuatro, y al menos dos columnas antes de
/// abrir otra fila.
pub fn tira_en_filas(alto: u32, cuantas: usize, escala: u32) -> Mosaico {
    let caben = (alto / e(ALTO_FILA, escala).max(1)).clamp(1, 4);
    let filas = caben.min((cuantas as u32).div_ceil(2).max(1));
    Mosaico {
        filas,
        columnas: columnas_para(cuantas, filas),
        por_columnas: true,
        cuantas,
    }
}

/// Todas las hojas a la vez (`RejillaDeHojas`): hasta seis filas, y cuatro
/// columnas antes de abrir otra fila.
pub fn rejilla(alto: u32, cuantas: usize, escala: u32) -> Mosaico {
    let caben = (alto / e(ALTO_FILA, escala).max(1)).clamp(1, 6);
    let filas = caben.min((cuantas as u32).div_ceil(4).max(1));
    Mosaico {
        filas,
        columnas: columnas_para(cuantas, filas),
        por_columnas: false,
        cuantas,
    }
}

impl Mosaico {
    /// Que hoja va en la columna `x`, fila `f`, si hay alguna.
    pub fn indice(&self, x: u32, f: u32) -> Option<usize> {
        if x >= self.columnas || f >= self.filas {
            return None;
        }
        let i = if self.por_columnas {
            x * self.filas + f
        } else {
            f * self.columnas + x
        } as usize;
        (i < self.cuantas).then_some(i)
    }

    /// Lo que mide la tira entera a lo ancho.
    pub fn ancho_total(&self, escala: u32) -> u32 {
        self.columnas * e(ANCHO_COLUMNA, escala)
    }

    /// El mayor desplazamiento que tiene sentido: el que deja la ultima
    /// columna pegada a la derecha del hueco.
    pub fn desplazamiento_maximo(&self, ancho: u32, escala: u32) -> i32 {
        self.ancho_total(escala).saturating_sub(ancho) as i32
    }

    /// La miniatura de la columna `x`, fila `f` (sin su nombre de debajo).
    pub fn celda(&self, area: Rect, x: u32, f: u32, desplazamiento: i32, escala: u32) -> Rect {
        Rect {
            x: area.x + (x * e(ANCHO_COLUMNA, escala)) as i32 - desplazamiento,
            y: area.y + (f * e(ALTO_FILA, escala)) as i32,
            ancho: e(ANCHO_HOJA, escala),
            alto: e(ALTO_HOJA, escala),
        }
    }

    /// Las columnas que se ven, con una de mas por cada lado para que la
    /// cortada no desaparezca de golpe.
    pub fn columnas_visibles(&self, ancho: u32, desplazamiento: i32, escala: u32) -> Range<u32> {
        let paso = e(ANCHO_COLUMNA, escala).max(1) as i32;
        let desde = (desplazamiento.max(0) / paso) as u32;
        let hasta = ((desplazamiento.max(0) + ancho as i32) / paso + 1) as u32;
        desde.min(self.columnas)..hasta.min(self.columnas)
    }

    /// Que hoja hay bajo el punto, si hay alguna.
    pub fn en(&self, area: Rect, p: Punto, desplazamiento: i32, escala: u32) -> Option<usize> {
        if !area.contiene(p) {
            return None;
        }
        for x in self.columnas_visibles(area.ancho, desplazamiento, escala) {
            for f in 0..self.filas {
                if self.celda(area, x, f, desplazamiento, escala).contiene(p) {
                    return self.indice(x, f);
                }
            }
        }
        None
    }

    /// Donde esta la hoja `i`: su columna y su fila.
    pub fn posicion(&self, i: usize) -> Option<(u32, u32)> {
        if i >= self.cuantas {
            return None;
        }
        let i = i as u32;
        Some(if self.por_columnas {
            (i / self.filas.max(1), i % self.filas.max(1))
        } else {
            (i % self.columnas.max(1), i / self.columnas.max(1))
        })
    }

    /// El desplazamiento que deja a la vista la hoja `i`, moviendose lo justo.
    pub fn para_ver(&self, i: usize, ancho: u32, desplazamiento: i32, escala: u32) -> i32 {
        let Some((x, _)) = self.posicion(i) else {
            return desplazamiento;
        };
        let paso = e(ANCHO_COLUMNA, escala) as i32;
        let izquierda = x as i32 * paso;
        let derecha = izquierda + paso;
        let d = if izquierda < desplazamiento {
            izquierda
        } else if derecha > desplazamiento + ancho as i32 {
            derecha - ancho as i32
        } else {
            desplazamiento
        };
        d.clamp(0, self.desplazamiento_maximo(ancho, escala).max(0))
    }
}

/// El interruptor Chat ↔ Proyectos ↔ Figuras de la barra de titulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interruptor {
    pub caja: Rect,
    pub chat: Rect,
    pub proyectos: Rect,
    /// La galeria de las figuras guardadas (24-oct-2026).
    pub figuras: Rect,
}

/// Lo que mide cada parte del interruptor.
pub const INTERRUPTOR_MITAD: u32 = 76;
/// Lo que se le quita al alto de la barra, repartido arriba y abajo.
pub const INTERRUPTOR_AIRE_Y: u32 = 3;
/// La letra de sus dos palabras.
pub const INTERRUPTOR_TAM: f32 = 11.0;

/// El interruptor, centrado en la barra de titulo y sin pisar sus botones:
/// en una ventana estrecha se corre a la izquierda de ellos. `hasta` es donde
/// empiezan los botones de minimizar, maximizar y cerrar; `desde`, donde acaba
/// el nombre del programa.
pub fn interruptor(barra: Rect, desde: i32, hasta: i32, escala: u32) -> Interruptor {
    let aire = e(INTERRUPTOR_AIRE_Y, escala);
    // En una ventana estrecha las tres partes se encogen para caber entre el
    // nombre del programa y los botones de la ventana.
    let sitio = (hasta - desde - aire as i32).max(0) as u32 / 3;
    let mitad = e(INTERRUPTOR_MITAD, escala).min(sitio.max(1));
    let ancho = 3 * mitad;
    let alto = barra.alto.saturating_sub(2 * aire).max(1);
    let centrado = barra.x + (barra.ancho as i32 - ancho as i32) / 2;
    let x = centrado.min(hasta - ancho as i32 - aire as i32).max(desde);
    let caja = Rect {
        x,
        y: barra.y + aire as i32,
        ancho,
        alto,
    };
    Interruptor {
        caja,
        chat: Rect {
            ancho: mitad,
            ..caja
        },
        proyectos: Rect {
            x: caja.x + mitad as i32,
            ancho: mitad,
            ..caja
        },
        figuras: Rect {
            x: caja.x + 2 * mitad as i32,
            ancho: mitad,
            ..caja
        },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn r(x: i32, y: i32, ancho: u32, alto: u32) -> Rect {
        Rect { x, y, ancho, alto }
    }

    fn p(x: i32, y: i32) -> Punto {
        Punto { x, y }
    }

    /// El panel derecho de una ventana de 1100 x 720 con la lista al ancho
    /// de fabrica: 708 de ancho bajo la barra de 24.
    fn panel_ancho() -> Panel {
        panel(r(392, 24, 708, 696), 54, 100, 4)
    }

    #[test]
    fn en_un_panel_ancho_la_barra_se_pone_de_pie_a_la_derecha_y_centrada() {
        let a = panel_ancho();
        assert!(a.apaisada);
        // Cuatro botones de 55 con 4 entre ellos y 6 de relleno arriba y abajo.
        assert_eq!(a.barra.ancho, 66 + 12);
        assert_eq!(a.barra.alto, 4 * 55 + 3 * 4 + 12);
        assert_eq!(a.barra.derecha(), 392 + 708 - 6);
        let debajo_y = 24 + 54;
        let debajo_alto = 696 - 54;
        assert_eq!(
            a.barra.y,
            debajo_y + (debajo_alto - a.barra.alto as i32) / 2
        );
        // La tarjeta deja a la derecha el sitio de la barra de pie.
        assert_eq!(a.hueco.derecha(), 392 + 708 - 78);
        assert_eq!(a.hueco.x, 392 + 10);
    }

    #[test]
    fn la_tarjeta_empieza_bajo_la_cabecera_del_chat_donde_va_volver() {
        let a = panel_ancho();
        assert_eq!(a.cabecera, r(392, 24, 708, 54));
        assert_eq!(a.hueco.y, a.cabecera.abajo());
        assert!(
            !a.hueco.contiene(p(392 + 20, 24 + 20)),
            "volver no queda debajo de la tarjeta"
        );
    }

    #[test]
    fn en_un_panel_estrecho_y_alto_la_barra_va_tumbada_abajo() {
        let a = panel(r(0, 24, 480, 876), 54, 100, 4);
        assert!(!a.apaisada);
        assert_eq!(a.barra.alto, 55 + 12);
        assert_eq!(a.barra.abajo(), 24 + 876 - 10);
        assert_eq!(a.barra.ancho, 4 * 66 + 3 * 4 + 12);
        // La tarjeta deja abajo el sitio de la barra, y nada de asomo: no
        // hay otro proyecto debajo, se cambia desde la lista.
        assert_eq!(a.hueco.alto, 876 - 54 - 80);
        assert!(a.hueco.abajo() <= a.barra.y);
    }

    #[test]
    fn si_la_tarjeta_cabe_no_se_desplaza() {
        let a = panel_ancho();
        assert_eq!(a.tope(), 0);
        assert_eq!(
            a.tarjeta(0),
            a.tarjeta(500),
            "sin nada que bajar la rueda no la mueve"
        );
        assert_eq!(a.tarjeta(0).alto, a.hueco.alto);
    }

    #[test]
    fn en_un_panel_bajo_la_tarjeta_no_se_aplasta_y_la_rueda_la_recorre() {
        let a = panel(r(300, 24, 700, 300), 54, 100, 4);
        assert_eq!(a.alto_tarjeta, TARJETA_MINIMA);
        assert_eq!(a.tope(), (TARJETA_MINIMA - a.hueco.alto) as i32);
        assert_eq!(a.tarjeta(40).y, a.hueco.y - 40);
        // No se sale por ningun lado.
        assert_eq!(a.tarjeta(-30).y, a.hueco.y);
        assert_eq!(a.tarjeta(10_000).abajo(), a.hueco.abajo());
    }

    #[test]
    fn los_botones_de_la_barra_se_encuentran_por_donde_se_pintan() {
        for a in [panel_ancho(), panel(r(0, 24, 480, 876), 54, 100, 4)] {
            for n in 0..4 {
                let b = a.boton(n);
                assert_eq!(a.boton_en(p(b.x + 3, b.y + 3)), Some(n));
            }
            assert_eq!(a.boton(4), vacio(), "no hay quinto boton");
            assert_eq!(a.boton_en(p(a.hueco.x + 5, a.hueco.y + 5)), None);
        }
    }

    #[test]
    fn un_panel_sin_sitio_no_da_medidas_negativas() {
        let a = panel(r(0, 24, 10, 20), 54, 100, 4);
        assert_eq!(a.cabecera.alto, 20);
        assert_eq!(a.hueco.ancho, 0);
        assert!(a.tope() >= 0);
    }

    #[test]
    fn una_tarjeta_ancha_pone_nombre_y_tira_a_un_lado_y_la_portada_al_otro() {
        let t = tarjeta(r(10, 76, 936, 650), 100, 60, true, false);
        assert!(t.apaisada);
        let columna = (936 - 28) * 42 / 100;
        assert_eq!(t.texto.x, 24);
        assert_eq!(t.menu.derecha(), 24 + columna);
        assert_eq!(t.rejilla.derecha(), t.menu.x);
        assert_eq!(t.hojas.x, 24 + columna + 12);
        assert_eq!(t.hojas.alto, 650 - 28, "la portada tiene todo el alto");
        assert_eq!(t.tira.y, t.texto.abajo() + 8);
        assert_eq!(t.tira.abajo(), t.contenido.abajo());
    }

    #[test]
    fn una_tarjeta_alta_pone_la_tira_de_una_fila_bajo_la_portada() {
        let t = tarjeta(r(10, 76, 460, 700), 100, 60, true, false);
        assert!(!t.apaisada);
        assert_eq!(t.tira.alto, ALTO_FILA);
        assert_eq!(t.tira.abajo(), t.contenido.abajo());
        assert_eq!(t.hojas.abajo() + 8, t.tira.y);
        assert_eq!(t.hojas.y, t.texto.abajo() + 10);
    }

    #[test]
    fn en_rejilla_o_sin_hojas_no_hay_tira_y_sin_hojas_tampoco_boton_de_rejilla() {
        let t = tarjeta(r(10, 76, 936, 650), 100, 60, true, true);
        assert_eq!(t.tira, vacio());
        let t = tarjeta(r(10, 76, 936, 650), 100, 60, false, false);
        assert_eq!(t.tira, vacio());
        assert_eq!(t.rejilla, vacio());
        assert_eq!(t.texto.derecha(), t.menu.x);
    }

    #[test]
    fn la_tira_en_filas_va_por_columnas_y_no_abre_fila_sin_dos_columnas() {
        // Sitio para cuatro filas, pero con tres hojas solo hacen falta dos.
        let m = tira_en_filas(4 * ALTO_FILA + 5, 3, 100);
        assert_eq!((m.filas, m.columnas), (2, 2));
        assert_eq!(m.indice(0, 0), Some(0));
        assert_eq!(m.indice(0, 1), Some(1), "la 2 debajo de la 1");
        assert_eq!(m.indice(1, 0), Some(2));
        assert_eq!(m.indice(1, 1), None, "no hay cuarta");
        // Con muchas, no mas de cuatro filas.
        assert_eq!(tira_en_filas(10 * ALTO_FILA, 200, 100).filas, 4);
    }

    #[test]
    fn la_rejilla_va_por_filas_con_cuatro_columnas_antes_de_abrir_otra() {
        let m = rejilla(10 * ALTO_FILA, 6, 100);
        assert_eq!((m.filas, m.columnas), (2, 3));
        assert_eq!(m.indice(0, 1), Some(3), "la segunda mitad abajo");
        assert_eq!(rejilla(10 * ALTO_FILA, 200, 100).filas, 6);
        assert_eq!(rejilla(10, 4, 100).filas, 1, "sin sitio, una fila");
    }

    #[test]
    fn pulsar_una_miniatura_la_encuentra_aunque_la_tira_este_corrida() {
        let area = r(100, 100, 400, ALTO_FILA);
        let m = tira(20);
        let corrida = 3 * ANCHO_COLUMNA as i32;
        let c = m.celda(area, 5, 0, corrida, 100);
        assert_eq!(m.en(area, p(c.x + 2, c.y + 2), corrida, 100), Some(5));
        // El aire entre dos miniaturas no es ninguna.
        assert_eq!(m.en(area, p(c.derecha() + 2, c.y + 2), corrida, 100), None);
        assert_eq!(m.en(area, p(10, 10), corrida, 100), None);
    }

    #[test]
    fn llevar_la_tira_a_una_hoja_se_mueve_lo_justo() {
        let m = tira(30);
        let paso = ANCHO_COLUMNA as i32;
        // Ya a la vista: no se mueve.
        assert_eq!(m.para_ver(2, 400, 0, 100), 0);
        // Por la derecha: la deja pegada al borde derecho.
        assert_eq!(m.para_ver(10, 400, 0, 100), 11 * paso - 400);
        // Por la izquierda: la deja pegada al izquierdo.
        assert_eq!(m.para_ver(1, 400, 5 * paso, 100), paso);
        // Una que no existe no mueve nada.
        assert_eq!(m.para_ver(99, 400, 7, 100), 7);
    }

    #[test]
    fn la_nitidez_de_la_portada_va_por_escalones() {
        assert_eq!(nitidez_para(300), 360);
        assert_eq!(nitidez_para(721), 1080);
        assert_eq!(nitidez_para(5000), 2000, "no mas que el ultimo escalon");
    }

    #[test]
    fn el_interruptor_va_centrado_y_no_pisa_los_botones_de_la_ventana() {
        let barra = r(0, 0, 1024, 24);
        let i = interruptor(barra, 80, 1024 - 108, 100);
        assert_eq!(i.caja.x, (1024 - 3 * 76) / 2);
        assert_eq!(i.chat.derecha(), i.proyectos.x);
        assert_eq!(i.proyectos.derecha(), i.figuras.x);
        assert_eq!(i.figuras.derecha(), i.caja.derecha());
        assert!(i.caja.alto < 24);
        // Estrecha: se corre a la izquierda de los botones, pero nunca encima
        // del nombre del programa.
        let estrecha = interruptor(r(0, 0, 380, 24), 80, 380 - 108, 100);
        assert!(estrecha.caja.derecha() <= 380 - 108);
        assert!(estrecha.caja.x >= 80);
    }
}
