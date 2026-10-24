//! Seleccionar texto reconocido con el raton, como en un documento (P4.4).
//!
//! El motor de Windows entrega PALABRAS con su recuadro, no caracteres, asi
//! que la seleccion mas fina que se puede hacer bien es por palabra.
//! Adivinar donde cae cada letra midiendo el texto daria recuadros que no
//! cuadran con lo que se ve en la imagen, y eso se nota en cuanto la fuente
//! no es la que supusiste.
//!
//! Lo dificil no es marcar palabras sueltas: es que arrastrar de la mitad
//! de un renglon a la mitad de otro tres mas abajo seleccione lo que
//! seleccionaria un documento — el final del primero, los de en medio
//! ENTEROS, y el principio del ultimo. Eso es lo que hay aqui, puro y
//! comprobable sin abrir una ventana.

use crate::{Punto, Rect};

/// Una palabra colocada en la imagen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palabra {
    pub caja: Rect,
    pub texto: String,
}

/// Un renglon con sus palabras, en orden de lectura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renglon {
    pub palabras: Vec<Palabra>,
}

impl Renglon {
    /// El recuadro que ocupa el renglon entero.
    pub fn caja(&self) -> Option<Rect> {
        let mut caja: Option<Rect> = None;
        for p in &self.palabras {
            caja = Some(match caja {
                None => p.caja,
                Some(a) => a.union(p.caja),
            });
        }
        caja
    }
}

/// Donde empieza o acaba una seleccion: el renglon y la palabra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sitio {
    pub renglon: usize,
    pub palabra: usize,
}

/// Lo que hay bajo el cursor, para decidir la forma del puntero.
///
/// Se usa una banda mas alta que el propio renglon: acertar el recuadro
/// exacto de una palabra obliga a afinar el raton, y para «hay texto aqui»
/// basta con estar a su altura.
pub fn hay_texto_en(renglones: &[Renglon], punto: Punto, holgura: i32) -> bool {
    sitio_en(renglones, punto, holgura).is_some()
}

/// La palabra bajo el punto, con holgura vertical.
///
/// Si el punto cae a la altura de un renglon pero fuera de sus palabras
/// —en el hueco entre dos, o pasado el final— se devuelve la palabra mas
/// cercana de ese renglon. Es lo que hace que arrastrar por el margen
/// derecho siga seleccionando hasta el final de la linea, como en
/// cualquier documento.
pub fn sitio_en(renglones: &[Renglon], punto: Punto, holgura: i32) -> Option<Sitio> {
    for (i, renglon) in renglones.iter().enumerate() {
        let Some(caja) = renglon.caja() else { continue };
        if punto.y < caja.arriba() - holgura || punto.y > caja.abajo() + holgura {
            continue;
        }
        // A la altura de este renglon. La palabra: la que contiene el punto
        // en horizontal, o la mas cercana.
        let mut mejor: Option<(usize, i32)> = None;
        for (j, p) in renglon.palabras.iter().enumerate() {
            let distancia = if punto.x < p.caja.izquierda() {
                p.caja.izquierda() - punto.x
            } else if punto.x > p.caja.derecha() {
                punto.x - p.caja.derecha()
            } else {
                0
            };
            if mejor.is_none_or(|(_, d)| distancia < d) {
                mejor = Some((j, distancia));
            }
        }
        return mejor.map(|(j, _)| Sitio {
            renglon: i,
            palabra: j,
        });
    }
    None
}

/// Las palabras que quedan seleccionadas al arrastrar de `desde` a `hasta`.
///
/// Devuelve un sitio por palabra, en orden de lectura. Arrastrar hacia
/// atras selecciona lo mismo que hacia delante: al usuario le da igual por
/// donde empezo, y un arrastre que no marca nada por ir al reves parece
/// que la funcion esta rota.
pub fn seleccion(renglones: &[Renglon], desde: Sitio, hasta: Sitio) -> Vec<Sitio> {
    let (a, b) = if desde <= hasta {
        (desde, hasta)
    } else {
        (hasta, desde)
    };
    let mut fuera = Vec::new();
    for renglon in a.renglon..=b.renglon.min(renglones.len().saturating_sub(1)) {
        let Some(r) = renglones.get(renglon) else {
            break;
        };
        // El primero empieza donde se pincho; los de en medio, enteros.
        let primera = if renglon == a.renglon { a.palabra } else { 0 };
        // El ultimo acaba donde se solto; los de en medio, hasta el final.
        let ultima = if renglon == b.renglon {
            b.palabra
        } else {
            r.palabras.len().saturating_sub(1)
        };
        for palabra in primera..=ultima.min(r.palabras.len().saturating_sub(1)) {
            if palabra < r.palabras.len() {
                fuera.push(Sitio { renglon, palabra });
            }
        }
    }
    fuera
}

/// El texto de una seleccion, listo para el portapapeles.
///
/// Las palabras del mismo renglon van separadas por un espacio y los
/// renglones por un salto de linea. Sin el salto, dos lineas de una tabla
/// se pegarian como una sola frase y no habria forma de saber donde
/// acababa cada una.
pub fn texto_de(renglones: &[Renglon], seleccion: &[Sitio]) -> String {
    let mut fuera = String::new();
    let mut renglon_previo: Option<usize> = None;
    for sitio in seleccion {
        let Some(p) = renglones
            .get(sitio.renglon)
            .and_then(|r| r.palabras.get(sitio.palabra))
        else {
            continue;
        };
        match renglon_previo {
            None => {}
            Some(previo) if previo == sitio.renglon => fuera.push(' '),
            Some(_) => fuera.push('\n'),
        }
        fuera.push_str(&p.texto);
        renglon_previo = Some(sitio.renglon);
    }
    fuera
}

/// Los recuadros que hay que pintar para una seleccion.
///
/// Se junta en uno lo que sea seguido dentro del mismo renglon: pintar una
/// caja por palabra deja rayas blancas en los espacios y no parece una
/// seleccion, parece un subrayado roto.
pub fn recuadros_de(renglones: &[Renglon], seleccion: &[Sitio]) -> Vec<Rect> {
    let mut fuera: Vec<Rect> = Vec::new();
    let mut anterior: Option<Sitio> = None;
    for sitio in seleccion {
        let Some(p) = renglones
            .get(sitio.renglon)
            .and_then(|r| r.palabras.get(sitio.palabra))
        else {
            continue;
        };
        let seguido =
            anterior.is_some_and(|a| a.renglon == sitio.renglon && a.palabra + 1 == sitio.palabra);
        match (seguido, fuera.last_mut()) {
            (true, Some(ultimo)) => *ultimo = ultimo.union(p.caja),
            _ => fuera.push(p.caja),
        }
        anterior = Some(*sitio);
    }
    fuera
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn palabra(x: i32, y: i32, ancho: u32, texto: &str) -> Palabra {
        Palabra {
            caja: Rect {
                x,
                y,
                ancho,
                alto: 20,
            },
            texto: texto.into(),
        }
    }

    /// Tres renglones de tres palabras, uno debajo de otro.
    fn pagina() -> Vec<Renglon> {
        vec![
            Renglon {
                palabras: vec![
                    palabra(10, 10, 40, "uno"),
                    palabra(60, 10, 40, "dos"),
                    palabra(110, 10, 40, "tres"),
                ],
            },
            Renglon {
                palabras: vec![
                    palabra(10, 40, 40, "cuatro"),
                    palabra(60, 40, 40, "cinco"),
                    palabra(110, 40, 40, "seis"),
                ],
            },
            Renglon {
                palabras: vec![
                    palabra(10, 70, 40, "siete"),
                    palabra(60, 70, 40, "ocho"),
                    palabra(110, 70, 40, "nueve"),
                ],
            },
        ]
    }

    #[test]
    fn se_acierta_la_palabra_bajo_el_cursor() {
        let p = pagina();
        assert_eq!(
            sitio_en(&p, Punto { x: 70, y: 45 }, 4),
            Some(Sitio {
                renglon: 1,
                palabra: 1
            })
        );
    }

    #[test]
    fn en_el_hueco_entre_palabras_manda_la_mas_cercana() {
        // Sin esto, arrastrar por los espacios daria saltos: la seleccion
        // se pararia en el hueco en vez de seguir a la palabra de al lado.
        let p = pagina();
        // x=55 esta entre «uno» (acaba en 50) y «dos» (empieza en 60): mas
        // cerca de «uno» por un pelo.
        assert_eq!(
            sitio_en(&p, Punto { x: 53, y: 15 }, 4),
            Some(Sitio {
                renglon: 0,
                palabra: 0
            })
        );
        // Y pasado el final del renglon, la ultima: es lo que hace que
        // arrastrar por el margen derecho llegue al final de la linea.
        assert_eq!(
            sitio_en(&p, Punto { x: 900, y: 15 }, 4),
            Some(Sitio {
                renglon: 0,
                palabra: 2
            })
        );
    }

    #[test]
    fn lejos_del_texto_no_hay_nada() {
        // Caso negativo: si esto devolviera algo, el cursor se pondria en
        // barra de texto por toda la imagen y arrastrar para mover el pin
        // empezaria a seleccionar.
        let p = pagina();
        assert_eq!(sitio_en(&p, Punto { x: 70, y: 200 }, 4), None);
        assert!(!hay_texto_en(&p, Punto { x: 70, y: 200 }, 4));
        assert!(hay_texto_en(&p, Punto { x: 70, y: 45 }, 4));
    }

    #[test]
    fn sin_texto_reconocido_no_se_selecciona_nada() {
        assert_eq!(sitio_en(&[], Punto { x: 0, y: 0 }, 4), None);
        let vacio = vec![Renglon { palabras: vec![] }];
        assert_eq!(sitio_en(&vacio, Punto { x: 0, y: 0 }, 4), None);
    }

    #[test]
    fn una_sola_palabra_se_selecciona_sola() {
        let p = pagina();
        let s = Sitio {
            renglon: 1,
            palabra: 1,
        };
        assert_eq!(seleccion(&p, s, s), vec![s]);
        assert_eq!(texto_de(&p, &seleccion(&p, s, s)), "cinco");
    }

    #[test]
    fn de_media_linea_a_media_linea_coge_los_de_en_medio_enteros() {
        // Es lo que hace que se parezca a un documento y no a marcar
        // recuadros sueltos: el final del primero, los de en medio ENTEROS,
        // y el principio del ultimo.
        let p = pagina();
        let desde = Sitio {
            renglon: 0,
            palabra: 2,
        };
        let hasta = Sitio {
            renglon: 2,
            palabra: 0,
        };
        let texto = texto_de(&p, &seleccion(&p, desde, hasta));
        assert_eq!(texto, "tres\ncuatro cinco seis\nsiete");
    }

    #[test]
    fn arrastrar_hacia_atras_selecciona_lo_mismo() {
        // Al usuario le da igual por donde empezo, y un arrastre que no
        // marca nada por ir al reves parece que la funcion esta rota.
        let p = pagina();
        let a = Sitio {
            renglon: 0,
            palabra: 1,
        };
        let b = Sitio {
            renglon: 1,
            palabra: 1,
        };
        assert_eq!(seleccion(&p, a, b), seleccion(&p, b, a));
        assert_eq!(texto_de(&p, &seleccion(&p, b, a)), "dos tres\ncuatro cinco");
    }

    #[test]
    fn los_renglones_se_separan_con_un_salto() {
        // Sin el salto, dos lineas de una tabla se pegarian como una sola
        // frase y no habria forma de saber donde acababa cada una.
        let p = pagina();
        let todo = seleccion(
            &p,
            Sitio {
                renglon: 0,
                palabra: 0,
            },
            Sitio {
                renglon: 2,
                palabra: 2,
            },
        );
        assert_eq!(
            texto_de(&p, &todo),
            "uno dos tres\ncuatro cinco seis\nsiete ocho nueve"
        );
    }

    #[test]
    fn lo_seguido_se_pinta_de_una_pieza() {
        // Una caja por palabra deja rayas blancas en los espacios: no
        // parece una seleccion, parece un subrayado roto.
        let p = pagina();
        let dentro_de_un_renglon = seleccion(
            &p,
            Sitio {
                renglon: 0,
                palabra: 0,
            },
            Sitio {
                renglon: 0,
                palabra: 2,
            },
        );
        let cajas = recuadros_de(&p, &dentro_de_un_renglon);
        assert_eq!(cajas.len(), 1, "tres palabras seguidas son una caja");
        assert_eq!(cajas[0].izquierda(), 10);
        assert_eq!(cajas[0].derecha(), 150);
        // Y tres renglones enteros son tres cajas, no una.
        let tres = seleccion(
            &p,
            Sitio {
                renglon: 0,
                palabra: 0,
            },
            Sitio {
                renglon: 2,
                palabra: 2,
            },
        );
        assert_eq!(recuadros_de(&p, &tres).len(), 3);
    }

    #[test]
    fn una_seleccion_vacia_no_da_ni_texto_ni_cajas() {
        let p = pagina();
        assert_eq!(texto_de(&p, &[]), "");
        assert!(recuadros_de(&p, &[]).is_empty());
    }

    #[test]
    fn un_sitio_que_ya_no_existe_no_revienta() {
        // Caso negativo: la seleccion se guarda por indices, y si el texto
        // se vuelve a reconocer con menos palabras esos indices quedan
        // colgando. Tiene que salir texto vacio, no un panico.
        let p = pagina();
        let inventado = Sitio {
            renglon: 99,
            palabra: 99,
        };
        assert_eq!(texto_de(&p, &[inventado]), "");
        assert!(recuadros_de(&p, &[inventado]).is_empty());
        assert!(seleccion(&p, inventado, inventado).is_empty());
    }
}
