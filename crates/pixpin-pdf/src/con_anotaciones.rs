//! **El PDF con todo lo anotado, y que sigue siendo el PDF.** Puerto de
//! `motor/PdfConAnotaciones.kt` y de `PdfAnotado.anotar/ensanchar/
//! conMarcadores` del movil (v0.97.0, 29-sep-2026).
//!
//! Lo pidio el usuario: el «Exportar» del lector pintaba cada hoja como
//! fotografia (un PDF de cien hojas y 3 MB salia de 14 o 15, sin texto que
//! buscar y borroso al ampliar). Lo que quiere es el PDF de siempre con lo
//! anotado encima. Aqui se hace como en el movil: **se parte del archivo tal
//! cual y se le pega una revision al final** (`union::incremental`): ni un
//! byte del original se mueve, asi que el texto se sigue buscando y
//! copiando, los vectores siguen siendo vectores, las imagenes no se
//! recomprimen y un lector que no sepa de capas lo abre igual.
//!
//! Por hoja, lo mismo que el movil:
//!
//! 1. Si el lector tenia **margenes para anotar**, la hoja se ensancha lo
//!    mismo (`/MediaBox` y `/CropBox`; el origen no se mueve, asi que lo que
//!    habia sigue en su sitio). Solo en hojas sin girar: girada, «a los
//!    lados» es otro eje de la caja y el lector no pone margenes ahi.
//! 2. Lo anotado encima, **como vectores**, en un formulario con su **capa**
//!    («PixPin · hoja N») que **el contenido de la hoja pinta al final**
//!    (`/OC … BDC … EMC`, Android v0.98.2, 30-sep): hasta entonces iba en una
//!    anotacion (un sello), y el lector de PDF de Android —el de PixPin y el
//!    de muchos visores del movil— no pinta anotaciones: el PDF salia
//!    «limpio». Los flujos de la hoja no se reescriben: se le anaden dos
//!    alrededor (`q` antes, y `Q` y lo nuestro despues) y a sus recursos el
//!    formulario y la capa. Se sigue apagando en el panel de capas de
//!    Acrobat, Foxit u Okular. El nombre empieza por «PixPin», que es lo que
//!    busca `cocido` para quitarla si el PDF vuelve a PixPin.
//!
//! Y al final los **marcadores** en el indice (`/Outlines`), detras de los
//! que ya tuviera.
//!
//! **Como se escribe la tinta**: con el mismo `escribir` del PDF del lienzo
//! (caminos, transparencias del resaltador, texto con la letra de la
//! pantalla, grano de las tintas porosas), cada hoja en una pagina de su
//! medida exacta y sin margen ([`escribir::de_hojas_a_medida`]); luego su
//! contenido y sus recursos se copian dentro del original como formulario.
//! Asi no hay un segundo dibujante que pueda discrepar del primero.
//!
//! Lo que no sale se dice con `None`: un PDF cifrado, uno que no se entiende
//! o uno que al volver a leerlo no tiene sus hojas. Mejor no compartir nada
//! que compartir un PDF roto; quien llama decide si cae a las fotos.

use std::collections::{HashMap, VecDeque};

use pixpin_motor2d::exportar::Hoja;
use pixpin_motor2d::pintado::Orden;

use crate::escribir::{self, Pixeles};
use crate::letra::Letra;
use crate::union::{Archivo, Dicc, Valor, en, entero, incremental, numero, poner};

/// Lo que mide de ancho cada hoja en las unidades de lo anotado
/// (`PdfDoc.PAGE_WIDTH` del movil, `vista::ANCHO_HOJA` del lector).
pub const ANCHO_EN_UNIDADES: f32 = 1400.0;

/// Tope de objetos copiados de la tinta, por si algo se referencia sin fin.
const MAX_OBJETOS: usize = 200_000;

/// **Un marcador del indice**: su titulo («⭐ Hoja 3»), la hoja (desde 0) y a
/// que altura de ella, de 0 (arriba) a 1.
#[derive(Debug, Clone, PartialEq)]
pub struct Marcador {
    pub titulo: String,
    pub pagina: usize,
    pub alto: f64,
}

/// **Lo anotado de un PDF.**
pub struct Anotaciones<'a> {
    /// La tinta de la hoja `i` (desde 0), en unidades de
    /// [`ANCHO_EN_UNIDADES`] con el cero en la esquina de arriba a la
    /// izquierda de la hoja: el margen izquierdo son equis negativas.
    pub tinta: &'a dyn Fn(usize) -> Option<Vec<Orden>>,
    /// Lo que mide cada margen del lector, en esas unidades (0: cerrado).
    pub izquierda: f32,
    pub derecha: f32,
    pub marcadores: &'a [Marcador],
    /// Los pixeles de las imagenes que haya en la tinta.
    pub imagenes: &'a dyn Fn(u64) -> Option<Pixeles>,
    /// La letra de la pantalla para el texto de la tinta; sin ella,
    /// Helvetica.
    pub letra: Option<&'a Letra>,
}

/// **El PDF `base` con lo anotado**, o `None` si no se puede (ver la
/// cabecera). Sin nada que poner sale `base` tal cual: es el PDF limpio.
///
/// `base` tiene que ser el PDF **sin nada anotado dentro** (en un proyecto,
/// su copia limpia; si trae capas de PixPin cocidas, cortadas antes con
/// `cocido::largo_sin_lo_cocido`), o lo anotado saldria dos veces.
pub fn hacer(base: &[u8], a: &Anotaciones) -> Option<Vec<u8>> {
    let archivo = Archivo::leer(base)?;
    if archivo.cifrado() {
        return None;
    }
    let paginas = archivo.paginas();
    if paginas.is_empty() {
        return None;
    }
    let izquierda = if a.izquierda.is_finite() {
        a.izquierda.max(0.0)
    } else {
        0.0
    };
    let derecha = if a.derecha.is_finite() {
        a.derecha.max(0.0)
    } else {
        0.0
    };

    // Lo de cada hoja: su caja, su giro y lo anotado.
    struct DeLaHoja {
        numero: u32,
        dicc: Dicc,
        caja: [f64; 4],
        giro: i32,
        /// Los puntos que se abren a cada lado.
        izq: f64,
        der: f64,
        tinta: Vec<Orden>,
    }
    let mut hojas = Vec::with_capacity(paginas.len());
    for (i, &n) in paginas.iter().enumerate() {
        let Some(dicc) = archivo.dicc_de(Some(&Valor::Ref(n, 0))) else {
            continue;
        };
        let Some(caja) = caja_de(&archivo, &dicc) else {
            continue;
        };
        let giro = giro_de(&archivo, &dicc);
        let ancho_pt = if de_lado(giro) {
            caja[3] - caja[1]
        } else {
            caja[2] - caja[0]
        };
        let por_unidad = ancho_pt / ANCHO_EN_UNIDADES as f64;
        let (izq, der) = if giro == 0 {
            (izquierda as f64 * por_unidad, derecha as f64 * por_unidad)
        } else {
            (0.0, 0.0)
        };
        let tinta = (a.tinta)(i).unwrap_or_default();
        hojas.push(DeLaHoja {
            numero: n,
            dicc,
            caja,
            giro,
            izq,
            der,
            tinta,
        });
    }
    let validos: Vec<&Marcador> = a
        .marcadores
        .iter()
        .filter(|m| m.pagina < paginas.len())
        .collect();
    let hay_tinta = hojas.iter().any(|h| !h.tinta.is_empty());
    let hay_margen = hojas.iter().any(|h| h.izq > 0.0 || h.der > 0.0);
    if !hay_tinta && !hay_margen && validos.is_empty() {
        return Some(base.to_vec());
    }

    let mut siguiente = archivo.siguiente_libre();
    let mut nuevos: Vec<(u32, u16, Valor)> = Vec::new();

    // La tinta de todas las hojas, escrita de una vez: la letra y las
    // transparencias se comparten entre hojas.
    let con_tinta: Vec<usize> = (0..hojas.len())
        .filter(|&k| !hojas[k].tinta.is_empty())
        .collect();
    let mut formularios: HashMap<usize, (Valor, [f64; 4])> = HashMap::new();
    if !con_tinta.is_empty() {
        let mut papeles = Vec::with_capacity(con_tinta.len());
        let mut medidas = Vec::with_capacity(con_tinta.len());
        for &k in &con_tinta {
            let h = &hojas[k];
            let (ancho_pt, alto_pt) = medida_vista(h.caja, h.giro);
            let por_unidad = ancho_pt / ANCHO_EN_UNIDADES as f64;
            let alto_u = (ANCHO_EN_UNIDADES as f64 * alto_pt / ancho_pt) as f32;
            let izq_u = (h.izq / por_unidad) as f32;
            let der_u = (h.der / por_unidad) as f32;
            papeles.push(Hoja {
                nombre: String::new(),
                caja: (-izq_u, 0.0, ANCHO_EN_UNIDADES + der_u, alto_u),
                ordenes: h.tinta.clone(),
                marcos: Vec::new(),
                granos: Vec::new(),
                grafitos: Vec::new(),
            });
            medidas.push(((ancho_pt + h.izq + h.der) as f32, alto_pt as f32));
        }
        let escrito = escribir::de_hojas_a_medida(&papeles, &medidas, a.imagenes, a.letra)?;
        let tinta = Archivo::leer(&escrito)?;
        let suyas = tinta.paginas();
        let mut numero_de: HashMap<u32, u32> = HashMap::new();
        let mut pendientes: VecDeque<u32> = VecDeque::new();
        for (j, &k) in con_tinta.iter().enumerate() {
            let h = &hojas[k];
            let pagina = tinta.dicc_de(Some(&Valor::Ref(*suyas.get(j)?, 0)))?;
            let Some(Valor::Flujo(d, datos)) = tinta.resolver(en(&pagina, b"Contents")) else {
                return None;
            };
            let recursos = trasladar(
                en(&pagina, b"Resources")?,
                &mut numero_de,
                &mut pendientes,
                &mut siguiente,
            );
            let (w, alto) = (medidas[j].0 as f64, medidas[j].1 as f64);
            let (m, rect) = matriz_y_rect(h.caja, h.giro, h.izq, h.der);
            let mut forma: Dicc = d.into_iter().filter(|(k, _)| k != b"Length").collect();
            poner(&mut forma, b"Type", nombre(b"XObject"));
            poner(&mut forma, b"Subtype", nombre(b"Form"));
            poner(&mut forma, b"FormType", numero(1));
            poner(&mut forma, b"BBox", lista(&[0.0, 0.0, w, alto]));
            poner(&mut forma, b"Matrix", lista(&m));
            poner(&mut forma, b"Resources", recursos);
            formularios.insert(k, (Valor::Flujo(forma, datos), rect));
        }
        // Todo lo que la tinta alcanza (letra, transparencias, imagenes).
        while let Some(n) = pendientes.pop_front() {
            if nuevos.len() > MAX_OBJETOS {
                return None;
            }
            let v = tinta.objeto(n).unwrap_or(Valor::Nulo);
            let t = trasladar(&v, &mut numero_de, &mut pendientes, &mut siguiente);
            nuevos.push((numero_de[&n], 0, t));
        }
    }

    // Cada hoja: ensanchada si toca, con su anotacion si tiene tinta.
    let mut capas = Vec::new();
    for (k, h) in hojas.iter().enumerate() {
        let formulario = formularios.remove(&k);
        let ensanchar = h.izq > 0.0 || h.der > 0.0;
        if formulario.is_none() && !ensanchar {
            continue;
        }
        let mut pagina = h.dicc.clone();
        if ensanchar {
            let [x0, y0, x1, y1] = h.caja;
            let media = caja_heredada(&archivo, &h.dicc, b"MediaBox").unwrap_or(h.caja);
            poner(
                &mut pagina,
                b"MediaBox",
                lista(&[
                    media[0].min(x0 - h.izq),
                    media[1],
                    media[2].max(x1 + h.der),
                    media[3],
                ]),
            );
            poner(
                &mut pagina,
                b"CropBox",
                lista(&[x0 - h.izq, y0, x1 + h.der, y1]),
            );
        }
        if let Some((forma, _)) = formulario {
            let hoja = paginas.iter().position(|&n| n == h.numero).unwrap_or(k) + 1;
            let nombre_capa = format!("PixPin · hoja {hoja}");
            let (capa, n_forma, abrir, pintar) =
                (siguiente, siguiente + 1, siguiente + 2, siguiente + 3);
            siguiente += 4;
            nuevos.push((
                capa,
                0,
                Valor::Dicc(vec![
                    (b"Type".to_vec(), nombre(b"OCG")),
                    (b"Name".to_vec(), texto_pdf(&nombre_capa)),
                ]),
            ));
            let Valor::Flujo(mut d, datos) = forma else {
                return None;
            };
            poner(&mut d, b"OC", Valor::Ref(capa, 0));
            nuevos.push((n_forma, 0, Valor::Flujo(d, datos)));
            // **La pagina la pinta al final, marcada como de la capa**
            // (`PdfAnotado.anotar` de Android v0.98.2). Antes iba en una
            // anotacion (un sello) y el lector de PDF de Android —el de
            // PixPin y el de muchos visores del movil— no pinta anotaciones:
            // el PDF exportado salia «limpio». Dentro del contenido lo pinta
            // todo el mundo, y en los lectores de escritorio la capa se sigue
            // apagando. Dos flujos nuevos alrededor de los suyos: la pila
            // grafica que dejen a medias no mueve el dibujo. La caja del
            // formulario y su matriz son las de antes: cae donde el sello.
            let nombre_forma = format!("{PREFIJO}T{n_forma}");
            let nombre_de_capa = format!("{PREFIJO}OC{capa}");
            nuevos.push((abrir, 0, Valor::Flujo(Vec::new(), b"q\n".to_vec())));
            nuevos.push((
                pintar,
                0,
                Valor::Flujo(
                    Vec::new(),
                    format!("Q\nq\n/OC /{nombre_de_capa} BDC\n/{nombre_forma} Do\nEMC\nQ\n")
                        .into_bytes(),
                ),
            ));
            let mut contenidos = vec![Valor::Ref(abrir, 0)];
            contenidos.extend(contenidos_de(&archivo, &h.dicc));
            contenidos.push(Valor::Ref(pintar, 0));
            poner(&mut pagina, b"Contents", Valor::Lista(contenidos));
            let recursos = con_lo_nuestro(
                &archivo,
                &h.dicc,
                (nombre_forma.as_bytes(), n_forma),
                (nombre_de_capa.as_bytes(), capa),
            );
            poner(&mut pagina, b"Resources", Valor::Dicc(recursos));
            capas.push(capa);
        }
        nuevos.push((h.numero, 0, Valor::Dicc(pagina)));
    }

    // El catalogo: las capas nuevas (o el panel no las ensena) y el indice.
    let Some(&Valor::Ref(n_catalogo, g_catalogo)) = en(archivo.trailer(), b"Root") else {
        return None;
    };
    let mut catalogo = archivo.raiz()?;
    let mut tocado = false;
    if !capas.is_empty() {
        let propiedades = anunciar_capas(&archivo, &catalogo, &capas);
        poner(&mut catalogo, b"OCProperties", propiedades);
        tocado = true;
    }
    if !validos.is_empty() {
        let mut con_indice = |c: &mut Dicc| -> Option<()> {
            let objetos = indice(
                &archivo,
                c,
                &paginas,
                &hojas
                    .iter()
                    .map(|h| (h.numero, h.caja, h.giro))
                    .collect::<Vec<_>>(),
                &validos,
                &mut siguiente,
            )?;
            nuevos.extend(objetos);
            Some(())
        };
        con_indice(&mut catalogo)?;
        tocado = true;
    }
    if tocado {
        nuevos.push((n_catalogo, g_catalogo, Valor::Dicc(catalogo)));
    }

    let salida = incremental(&archivo, base, &nuevos, siguiente);
    // Se vuelve a leer: si no tiene sus hojas, no se entrega.
    let comprobado = Archivo::leer(&salida)?.paginas().len();
    (comprobado == paginas.len()).then_some(salida)
}

/// **Los marcadores que ya trae el PDF**, en su indice (`/Outlines`): lo
/// inverso de lo que escribe [`hacer`] (`PdfAnotado.marcadoresDelIndice` de
/// Android v0.98.2). Un PDF exportado desde PixPin lleva ahi sus
/// marcadores, y el lector los recoge la primera vez que lo abre; tambien
/// los de cualquier otro documento. Van en orden de lectura, con los de
/// dentro de cada uno detras de el, hasta `tope`. Los que apuntan a un
/// destino con nombre o a otra cosa que una hoja se saltan. Un PDF cifrado
/// o que no se entiende no da ninguno.
pub fn marcadores_del_indice(bytes: &[u8], tope: usize) -> Vec<Marcador> {
    let Some(archivo) = Archivo::leer(bytes) else {
        return Vec::new();
    };
    if archivo.cifrado() {
        return Vec::new();
    }
    let Some(raiz) = archivo
        .raiz()
        .and_then(|c| archivo.dicc_de(en(&c, b"Outlines")))
    else {
        return Vec::new();
    };
    let paginas = archivo.paginas();
    let por_numero: HashMap<u32, usize> =
        paginas.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let mut salida = Vec::new();
    let mut vistos = std::collections::HashSet::new();
    // Sin recursion: una pila de «siguiente por mirar» y su hondura.
    let mut pila: Vec<(Option<Valor>, usize)> = vec![(en(&raiz, b"First").cloned(), 0)];
    while let Some((actual, fondo)) = pila.pop() {
        let mut r = actual;
        while let Some(Valor::Ref(n, _)) = r {
            if salida.len() >= tope || fondo >= 16 || !vistos.insert(n) {
                break;
            }
            let Some(m) = archivo.dicc_de(Some(&Valor::Ref(n, 0))) else {
                break;
            };
            let destino = archivo.resolver(en(&m, b"Dest")).or_else(|| {
                archivo
                    .dicc_de(en(&m, b"A"))
                    .filter(|a| matches!(en(a, b"S"), Some(Valor::Nombre(s)) if s == b"GoTo"))
                    .and_then(|a| archivo.resolver(en(&a, b"D")))
            });
            if let Some(Valor::Lista(l)) = destino
                && let Some(Valor::Ref(p, _)) = l.first()
                && let Some(&pagina) = por_numero.get(p)
            {
                let titulo = match archivo.resolver(en(&m, b"Title")) {
                    Some(Valor::Cadena(c)) => {
                        crate::plano::texto_de_cadena(&crate::plano::bytes_de_cadena(&c))
                    }
                    _ => String::new(),
                };
                // Con `/XYZ izquierda arriba zoom`, la altura; si no, lo alto de la hoja.
                let arriba = match l.get(1) {
                    Some(Valor::Nombre(x)) if x == b"XYZ" => {
                        l.get(3).and_then(|v| real_de(&archivo, v))
                    }
                    _ => None,
                };
                let dicc = archivo.dicc_de(Some(&Valor::Ref(*p, 0)));
                let caja = dicc.as_ref().and_then(|d| caja_de(&archivo, d));
                let alto = match (arriba, caja) {
                    (Some(a), Some(c)) if c[3] > c[1] => {
                        ((c[3] - a) / (c[3] - c[1])).clamp(0.0, 1.0)
                    }
                    _ => 0.0,
                };
                salida.push(Marcador {
                    titulo: titulo.trim().to_string(),
                    pagina,
                    alto,
                });
            }
            // Los de dentro, justo detras de este; los hermanos, despues.
            let siguiente = en(&m, b"Next").cloned();
            if let Some(hijo) = en(&m, b"First").cloned() {
                pila.push((siguiente, fondo));
                pila.push((Some(hijo), fondo + 1));
                break;
            }
            r = siguiente;
        }
    }
    salida
}

// ---------------------------------------------------------------------------
// La caja, el giro y la matriz

fn de_lado(giro: i32) -> bool {
    giro == 90 || giro == 270
}

/// Lo que mide la hoja **como se ve**, en puntos.
fn medida_vista(caja: [f64; 4], giro: i32) -> (f64, f64) {
    let (w, h) = (caja[2] - caja[0], caja[3] - caja[1]);
    if de_lado(giro) { (h, w) } else { (w, h) }
}

/// Una caja heredada del arbol (`/MediaBox`, `/CropBox`), con las esquinas
/// ordenadas.
fn caja_heredada(archivo: &Archivo, pagina: &Dicc, clave: &[u8]) -> Option<[f64; 4]> {
    let mut d = Some(pagina.clone());
    let mut saltos = 0;
    while let Some(actual) = d {
        if saltos > 32 {
            return None;
        }
        saltos += 1;
        if let Some(Valor::Lista(l)) = archivo.resolver(en(&actual, clave)) {
            let n: Vec<f64> = l.iter().filter_map(|v| real_de(archivo, v)).collect();
            if n.len() >= 4 {
                return Some([
                    n[0].min(n[2]),
                    n[1].min(n[3]),
                    n[0].max(n[2]),
                    n[1].max(n[3]),
                ]);
            }
        }
        d = archivo.dicc_de(en(&actual, b"Parent"));
    }
    None
}

/// **El papel que se ve**: el `/CropBox` recortado al `/MediaBox`, que es lo
/// que pinta Windows y sobre lo que se anoto; sin `/CropBox`, el
/// `/MediaBox`. `None` si no tiene medida.
fn caja_de(archivo: &Archivo, pagina: &Dicc) -> Option<[f64; 4]> {
    let media = caja_heredada(archivo, pagina, b"MediaBox")?;
    let caja = match caja_heredada(archivo, pagina, b"CropBox") {
        Some(c) => [
            c[0].max(media[0]),
            c[1].max(media[1]),
            c[2].min(media[2]),
            c[3].min(media[3]),
        ],
        None => media,
    };
    (caja[2] - caja[0] >= 1.0 && caja[3] - caja[1] >= 1.0).then_some(caja)
}

/// El giro, heredado, en 0, 90, 180 o 270: hay archivos que escriben `-90` o
/// `630`, y los dos son 270.
fn giro_de(archivo: &Archivo, pagina: &Dicc) -> i32 {
    let mut d = Some(pagina.clone());
    let mut saltos = 0;
    while let Some(actual) = d {
        if saltos > 32 {
            break;
        }
        saltos += 1;
        if let Some(v) = archivo
            .resolver(en(&actual, b"Rotate"))
            .and_then(|v| real_de(archivo, &v))
        {
            return ((v as i64).rem_euclid(360) / 90 * 90) as i32;
        }
        d = archivo.dicc_de(en(&actual, b"Parent"));
    }
    0
}

/// **La matriz que lleva el papel de la tinta a la hoja**, y el rectangulo
/// de la anotacion. El papel de la tinta mide la hoja como se ve (mas los
/// margenes), con el origen abajo a la izquierda; la hoja del PDF puede estar
/// girada, y entonces «como se ve» es su caja dada la vuelta. Las cuatro
/// esquinas caen en las cuatro esquinas (`matrizDePagina` del movil).
fn matriz_y_rect(caja: [f64; 4], giro: i32, izq: f64, der: f64) -> ([f64; 6], [f64; 4]) {
    let [x0, y0, x1, y1] = caja;
    let m = match giro {
        90 => [0.0, 1.0, -1.0, 0.0, x1, y0],
        180 => [-1.0, 0.0, 0.0, -1.0, x1, y1],
        270 => [0.0, -1.0, 1.0, 0.0, x0, y1],
        _ => [1.0, 0.0, 0.0, 1.0, x0 - izq, y0],
    };
    (m, [x0 - izq, y0, x1 + der, y1])
}

/// Como empiezan los nombres de lo nuestro en los recursos de la hoja
/// (`PdfAnotado.PREFIJO` del movil): `PxT<n>` el formulario de la tinta y
/// `PxOC<n>` su capa, con el numero de su objeto, asi que no chocan con los
/// de la hoja ni con los de otra tanda.
const PREFIJO: &str = "Px";

/// **Los flujos de contenido de la hoja**, en orden: `/Contents` es uno o
/// una lista, y la lista puede ir en su propio objeto.
fn contenidos_de(archivo: &Archivo, pagina: &Dicc) -> Vec<Valor> {
    match en(pagina, b"Contents") {
        Some(Valor::Lista(l)) => l.clone(),
        Some(r @ Valor::Ref(..)) => match archivo.resolver(Some(r)) {
            Some(Valor::Lista(l)) => l,
            Some(_) => vec![r.clone()],
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// **Los recursos de la hoja con los nuestros**: los suyos (heredados del
/// arbol si no los lleva: sus padres no se tocan) tal cual, y en `/XObject`
/// el formulario de la tinta y en `/Properties` su capa, junto a los que
/// hubiera (`fusionarRecursos` del movil). Va escrito en la hoja.
fn con_lo_nuestro(
    archivo: &Archivo,
    pagina: &Dicc,
    forma: (&[u8], u32),
    capa: (&[u8], u32),
) -> Dicc {
    let mut recursos = recursos_heredados(archivo, pagina).unwrap_or_default();
    let mut anadir = |clave: &[u8], nombre: &[u8], n: u32| {
        let mut d = archivo.dicc_de(en(&recursos, clave)).unwrap_or_default();
        poner(&mut d, nombre, Valor::Ref(n, 0));
        poner(&mut recursos, clave, Valor::Dicc(d));
    };
    anadir(b"XObject", forma.0, forma.1);
    anadir(b"Properties", capa.0, capa.1);
    recursos
}

/// `/Resources` de la hoja o, si no lo lleva, del primero de sus padres que
/// lo tenga.
fn recursos_heredados(archivo: &Archivo, pagina: &Dicc) -> Option<Dicc> {
    let mut d = Some(pagina.clone());
    let mut saltos = 0;
    while let Some(actual) = d {
        if saltos > 32 {
            return None;
        }
        saltos += 1;
        if let Some(r) = archivo.dicc_de(en(&actual, b"Resources")) {
            return Some(r);
        }
        d = archivo.dicc_de(en(&actual, b"Parent"));
    }
    None
}

// ---------------------------------------------------------------------------
// El catalogo: capas e indice

/// **Las capas nuevas, junto a las que hubiera** (`anunciarLaCapa` del
/// movil): una capa que no figura en `/OCProperties` no sale en el panel, y
/// hay lectores que no dibujan lo que la lleva. Un PDF de AutoCAD llega con
/// las suyas y perderlas seria romperle el archivo. Nacen encendidas.
fn anunciar_capas(archivo: &Archivo, catalogo: &Dicc, capas: &[u32]) -> Valor {
    let lista_de =
        |d: Option<&Dicc>, clave: &[u8]| match d.and_then(|d| archivo.resolver(en(d, clave))) {
            Some(Valor::Lista(l)) => l,
            _ => Vec::new(),
        };
    let propiedades = archivo.dicc_de(en(catalogo, b"OCProperties"));
    let por_defecto = propiedades
        .as_ref()
        .and_then(|p| archivo.dicc_de(en(p, b"D")));
    let nuevas: Vec<Valor> = capas.iter().map(|&c| Valor::Ref(c, 0)).collect();
    let mut todas = lista_de(propiedades.as_ref(), b"OCGs");
    todas.extend(nuevas.iter().cloned());
    let mut orden = lista_de(por_defecto.as_ref(), b"Order");
    orden.extend(nuevas.iter().cloned());
    let mut encendidas = lista_de(por_defecto.as_ref(), b"ON");
    encendidas.extend(nuevas);
    let mut d = por_defecto.unwrap_or_default();
    poner(&mut d, b"Order", Valor::Lista(orden));
    poner(&mut d, b"ON", Valor::Lista(encendidas));
    let mut p = propiedades.unwrap_or_default();
    poner(&mut p, b"OCGs", Valor::Lista(todas));
    poner(&mut p, b"D", Valor::Dicc(d));
    Valor::Dicc(p)
}

/// **Los marcadores en el indice** (`conMarcadores` del movil), detras de
/// los que ya tuviera: cambia `catalogo` si hace falta y devuelve los objetos
/// nuevos o cambiados.
fn indice(
    archivo: &Archivo,
    catalogo: &mut Dicc,
    paginas: &[u32],
    cajas: &[(u32, [f64; 4], i32)],
    marcadores: &[&Marcador],
    siguiente: &mut u32,
) -> Option<Vec<(u32, u16, Valor)>> {
    let mut objetos = Vec::new();
    let existente = match en(catalogo, b"Outlines") {
        Some(Valor::Ref(n, _)) => archivo.dicc_de(Some(&Valor::Ref(*n, 0))).map(|d| (*n, d)),
        _ => None,
    };
    let numero_raiz = match &existente {
        Some((n, _)) => *n,
        None => {
            *siguiente += 1;
            *siguiente - 1
        }
    };
    let nuestros: Vec<u32> = marcadores
        .iter()
        .map(|_| {
            *siguiente += 1;
            *siguiente - 1
        })
        .collect();
    let ultimo_viejo = existente.as_ref().and_then(|(_, d)| match en(d, b"Last") {
        Some(Valor::Ref(n, g)) => Some((*n, *g)),
        _ => None,
    });
    for (k, m) in marcadores.iter().enumerate() {
        let pagina = *paginas.get(m.pagina)?;
        // La altura, de arriba abajo en la hoja; girada, arriba sin mas.
        let arriba = cajas
            .iter()
            .find(|(n, _, _)| *n == pagina)
            .filter(|(_, _, giro)| *giro == 0)
            .map(|(_, c, _)| c[3] - m.alto.clamp(0.0, 1.0) * (c[3] - c[1]));
        let destino = Valor::Lista(vec![
            Valor::Ref(pagina, 0),
            nombre(b"XYZ"),
            Valor::Nulo,
            arriba.map_or(Valor::Nulo, real),
            Valor::Nulo,
        ]);
        let mut d: Dicc = vec![
            (b"Title".to_vec(), texto_pdf(&m.titulo)),
            (b"Parent".to_vec(), Valor::Ref(numero_raiz, 0)),
            (b"Dest".to_vec(), destino),
        ];
        if k > 0 {
            d.push((b"Prev".to_vec(), Valor::Ref(nuestros[k - 1], 0)));
        } else if let Some((n, g)) = ultimo_viejo {
            d.push((b"Prev".to_vec(), Valor::Ref(n, g)));
        }
        if let Some(sig) = nuestros.get(k + 1) {
            d.push((b"Next".to_vec(), Valor::Ref(*sig, 0)));
        }
        objetos.push((nuestros[k], 0, Valor::Dicc(d)));
    }
    // El ultimo de los que habia apunta al primero nuestro.
    if let Some((n, g)) = ultimo_viejo
        && let Some(mut d) = archivo.dicc_de(Some(&Valor::Ref(n, g)))
    {
        poner(&mut d, b"Next", Valor::Ref(nuestros[0], 0));
        objetos.push((n, g, Valor::Dicc(d)));
    }
    let raiz = existente.as_ref().map(|(_, d)| d.clone());
    let cuenta = raiz
        .as_ref()
        .and_then(|d| entero(en(d, b"Count")))
        .unwrap_or(0)
        .max(0);
    let mut r = raiz.unwrap_or_default();
    poner(&mut r, b"Type", nombre(b"Outlines"));
    if en(&r, b"First").is_none() {
        poner(&mut r, b"First", Valor::Ref(nuestros[0], 0));
    }
    poner(&mut r, b"Last", Valor::Ref(*nuestros.last()?, 0));
    poner(&mut r, b"Count", numero(cuenta + nuestros.len() as i64));
    objetos.push((numero_raiz, 0, Valor::Dicc(r)));
    if existente.is_none() {
        poner(catalogo, b"Outlines", Valor::Ref(numero_raiz, 0));
        // Que el lector abra con el panel de marcadores, si el documento no
        // decia otra cosa.
        if en(catalogo, b"PageMode").is_none() {
            poner(catalogo, b"PageMode", nombre(b"UseOutlines"));
        }
    }
    Some(objetos)
}

// ---------------------------------------------------------------------------
// Valores

fn nombre(n: &[u8]) -> Valor {
    Valor::Nombre(n.to_vec())
}

/// Un real corto: cuatro decimales como mucho y sin ceros de sobra.
fn real(v: f64) -> Valor {
    let v = if v.is_finite() { v } else { 0.0 };
    let r = (v * 10_000.0).round() / 10_000.0;
    let s = if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        format!("{r:.4}").trim_end_matches('0').to_string()
    };
    Valor::Numero(s.into_bytes())
}

fn lista(v: &[f64]) -> Valor {
    Valor::Lista(v.iter().map(|x| real(*x)).collect())
}

fn real_de(archivo: &Archivo, v: &Valor) -> Option<f64> {
    match archivo.resolver(Some(v))? {
        Valor::Numero(t) => std::str::from_utf8(&t).ok()?.parse().ok(),
        _ => None,
    }
}

/// Un texto del PDF en UTF-16 con su marca delante, en hexadecimal: la unica
/// codificacion que admite acentos y emoticonos en cualquier lector.
fn texto_pdf(s: &str) -> Valor {
    let mut o = String::from("<FEFF");
    for u in s.encode_utf16() {
        o.push_str(&format!("{u:04X}"));
    }
    o.push('>');
    Valor::Cadena(o.into_bytes())
}

/// Un valor del PDF de la tinta con sus referencias renumeradas para el
/// original; lo que alcanza queda en `pendientes` para copiarlo despues.
fn trasladar(
    v: &Valor,
    numero_de: &mut HashMap<u32, u32>,
    pendientes: &mut VecDeque<u32>,
    siguiente: &mut u32,
) -> Valor {
    match v {
        Valor::Ref(n, _) => {
            let nuevo = *numero_de.entry(*n).or_insert_with(|| {
                pendientes.push_back(*n);
                let s = *siguiente;
                *siguiente += 1;
                s
            });
            Valor::Ref(nuevo, 0)
        }
        Valor::Lista(l) => Valor::Lista(
            l.iter()
                .map(|x| trasladar(x, numero_de, pendientes, siguiente))
                .collect(),
        ),
        Valor::Dicc(d) => Valor::Dicc(
            d.iter()
                .map(|(k, x)| (k.clone(), trasladar(x, numero_de, pendientes, siguiente)))
                .collect(),
        ),
        Valor::Flujo(d, datos) => Valor::Flujo(
            d.iter()
                .map(|(k, x)| (k.clone(), trasladar(x, numero_de, pendientes, siguiente)))
                .collect(),
            datos.clone(),
        ),
        otro => otro.clone(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un PDF con estos objetos (el primero es el 1) y su tabla clasica.
    fn pdf_de(objetos: &[&str]) -> Vec<u8> {
        let mut s = b"%PDF-1.4\n".to_vec();
        let mut donde = Vec::new();
        for (i, o) in objetos.iter().enumerate() {
            donde.push(s.len());
            s.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let inicio = s.len();
        let mut t = format!("xref\n0 {}\n0000000000 65535 f \n", objetos.len() + 1);
        for d in donde {
            t.push_str(&format!("{d:010} 00000 n \n"));
        }
        t.push_str(&format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{inicio}\n%%EOF\n",
            objetos.len() + 1
        ));
        s.extend_from_slice(t.as_bytes());
        s
    }

    #[test]
    fn el_indice_se_recorre_en_orden_de_lectura_con_los_de_dentro_detras_de_su_padre() {
        // 1 catalogo, 2 arbol, 3 y 4 hojas (heredan la caja), 5 raiz del
        // indice, 6 «Uno» (hijo 7 «Uno.a»), 8 con destino con nombre (se
        // salta), 9 «Dos» por `/A GoTo` y en literal Latin-1.
        let b = pdf_de(&[
            "<< /Type /Catalog /Pages 2 0 R /Outlines 5 0 R >>",
            "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 100 200] >>",
            "<< /Type /Page /Parent 2 0 R >>",
            "<< /Type /Page /Parent 2 0 R >>",
            "<< /Type /Outlines /First 6 0 R /Last 9 0 R /Count 3 >>",
            "<< /Title <FEFF0055006E006F> /Parent 5 0 R /Dest [3 0 R /XYZ null 150 null] /First 7 0 R /Last 7 0 R /Next 8 0 R >>",
            "<< /Title (Uno.a) /Parent 6 0 R /Dest [4 0 R /Fit] >>",
            "<< /Title (Con nombre) /Parent 5 0 R /Dest (capitulo1) /Prev 6 0 R /Next 9 0 R >>",
            "<< /Title (Dos a\\361o) /Parent 5 0 R /A << /S /GoTo /D [4 0 R /XYZ 0 50 0] >> /Prev 8 0 R >>",
        ]);
        let m = marcadores_del_indice(&b, 50);
        let resumen: Vec<(&str, usize)> = m.iter().map(|x| (x.titulo.as_str(), x.pagina)).collect();
        assert_eq!(resumen, vec![("Uno", 0), ("Uno.a", 1), ("Dos año", 1)]);
        assert!(
            (m[0].alto - 0.25).abs() < 1e-9 && m[1].alto == 0.0 && (m[2].alto - 0.75).abs() < 1e-9,
            "{m:?}"
        );
        // Caso negativo: un indice que se muerde la cola no da vueltas sin fin.
        let b = pdf_de(&[
            "<< /Type /Catalog /Pages 2 0 R /Outlines 4 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 100 200] >>",
            "<< /Type /Page /Parent 2 0 R >>",
            "<< /Type /Outlines /First 5 0 R >>",
            "<< /Title (Bucle) /Dest [3 0 R /Fit] /Next 5 0 R /First 5 0 R >>",
        ]);
        assert_eq!(marcadores_del_indice(&b, 50).len(), 1);
    }

    #[test]
    fn la_hoja_con_recursos_heredados_y_contenidos_en_su_objeto_se_lee_entera() {
        let b = pdf_de(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 100 200] /Resources << /Font << /F1 6 0 R >> /XObject 7 0 R >> >>",
            "<< /Type /Page /Parent 2 0 R /Contents 4 0 R >>",
            "[5 0 R 5 0 R]",
            "<< /Length 0 >>\nstream\n\nendstream",
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            "<< /Im9 5 0 R >>",
        ]);
        let a = Archivo::leer(&b).unwrap();
        let hoja = a.dicc_de(Some(&Valor::Ref(3, 0))).unwrap();
        assert_eq!(
            contenidos_de(&a, &hoja),
            vec![Valor::Ref(5, 0), Valor::Ref(5, 0)]
        );
        let r = con_lo_nuestro(&a, &hoja, (b"PxT20", 20), (b"PxOC21", 21));
        let fuentes = a.dicc_de(en(&r, b"Font")).unwrap();
        assert!(en(&fuentes, b"F1").is_some(), "la letra heredada sigue");
        let xo = a.dicc_de(en(&r, b"XObject")).unwrap();
        assert_eq!(
            en(&xo, b"Im9"),
            Some(&Valor::Ref(5, 0)),
            "sus formularios siguen"
        );
        assert_eq!(en(&xo, b"PxT20"), Some(&Valor::Ref(20, 0)));
        let props = a.dicc_de(en(&r, b"Properties")).unwrap();
        assert_eq!(en(&props, b"PxOC21"), Some(&Valor::Ref(21, 0)));
        // Caso negativo: una hoja sin contenido no inventa ninguno.
        assert!(contenidos_de(&a, &Vec::new()).is_empty());
    }

    #[test]
    fn los_reales_van_cortos_y_sin_ceros_de_sobra() {
        assert_eq!(real(2.0), Valor::Numero(b"2".to_vec()));
        assert_eq!(real(-459.0), Valor::Numero(b"-459".to_vec()));
        assert_eq!(real(0.5), Valor::Numero(b"0.5".to_vec()));
        assert_eq!(real(f64::NAN), Valor::Numero(b"0".to_vec()));
    }

    #[test]
    fn la_matriz_lleva_cada_esquina_del_papel_a_la_suya() {
        // Hoja de 612 x 792 girada 90: se ve de 792 x 612.
        let caja = [0.0, 0.0, 612.0, 792.0];
        let aplicar =
            |m: [f64; 6], x: f64, y: f64| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        let (m, _) = matriz_y_rect(caja, 90, 0.0, 0.0);
        // Arriba a la izquierda de lo que se ve (0, 612) es abajo a la
        // izquierda de la hoja sin girar.
        assert_eq!(aplicar(m, 0.0, 612.0), (0.0, 0.0));
        assert_eq!(aplicar(m, 792.0, 612.0), (0.0, 792.0));
        let (m, _) = matriz_y_rect(caja, 270, 0.0, 0.0);
        assert_eq!(aplicar(m, 0.0, 612.0), (612.0, 792.0));
        let (m, _) = matriz_y_rect(caja, 180, 0.0, 0.0);
        assert_eq!(aplicar(m, 0.0, 792.0), (612.0, 0.0));
        // Sin girar, con margen: el cero del papel es el borde del margen.
        let (m, rect) = matriz_y_rect(caja, 0, 459.0, 0.0);
        assert_eq!(aplicar(m, 0.0, 0.0), (-459.0, 0.0));
        assert_eq!(rect, [-459.0, 0.0, 612.0, 792.0]);
    }
}
