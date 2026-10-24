//! **La tabla de una hoja de calculo, dibujada en el lienzo**
//! (`motor/TablaDibujada.kt` y `clipboard/TableData.kt` del movil).
//!
//! Copiar unas celdas de Excel o de Sheets y que aparezcan aqui como una
//! tabla de verdad es de las cosas que mas tiempo ahorran: lo alternativo es
//! teclear otra vez lo que ya esta escrito, o pegar una captura que no se
//! puede corregir ni exportar a vectores.
//!
//! ## Se dibuja con lo que ya hay, y es a proposito
//!
//! No hay un tipo de elemento «tabla». Una tabla es **un grupo de rayas y
//! textos** de los de siempre: se exporta a SVG y a PDF sin tocar nada, se
//! mueve y se estira con las herramientas de siempre, y viaja al movil y a
//! Excalidraw con su tabla puesta. Lo que se paga es que no se le anade una
//! fila despues: para eso se vuelve a pegar.
//!
//! Y se dibuja **recta**, a tiralineas: una rejilla temblorosa no se lee como
//! una tabla, se lee como suciedad.
//!
//! ## Lo que trae el portapapeles de Windows
//!
//! Excel, Sheets y LibreOffice dejan `CF_UNICODETEXT` con las columnas
//! separadas por tabulador y las filas por salto de linea, y un «HTML Format»
//! con la negrita y las celdas combinadas. El texto basta para una tabla
//! corriente, pero pone una combinada en su primera celda y deja las demas
//! vacias: dibujada asi, una raya parte en tres lo que en la hoja es una
//! celda. Por eso, si hay HTML, manda [`elementos_de_tabla_con_juntas`].
//! Tambien se reconocen las tablas de Markdown (barras) y las columnas
//! alineadas con espacios.

use crate::ecuacion::{Estilo, Medir, elemento_linea, elemento_texto};
use crate::elemento::{ColorRgba, Elemento, Figura};
use crate::relleno::EstiloRelleno;
use crate::vector::Punto2;

pub mod leer;

/// El aire que deja cada celda alrededor de su texto, en px del dibujo.
pub const AIRE_DE_CELDA: f32 = 8.0;
/// Lo menos que mide una columna: una columna vacia mediria cero y la tabla
/// saldria con dos rayas pegadas.
pub const ANCHO_MINIMO_DE_COLUMNA: f32 = 44.0;
/// Y lo menos que mide una fila, por lo mismo.
pub const ALTO_MINIMO_DE_FILA: f32 = 28.0;
/// El fondo de la fila de titulos: un gris muy claro, no un color.
pub const FONDO_DE_CABECERA: ColorRgba = ColorRgba {
    r: 0xf1 as f32 / 255.0,
    g: 0xf3 as f32 / 255.0,
    b: 0xf5 as f32 / 255.0,
    a: 1.0,
};

/// Minimo de filas para no confundir dos lineas cualesquiera con una tabla.
const MIN_FILAS: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Separador {
    Tabulador,
    Barra,
    Espacios,
}

fn partir(linea: &str, sep: Separador) -> Vec<String> {
    match sep {
        // Con tabuladores NO se recorta la linea: en los extremos vive una
        // celda vacia, y quitarla corria los datos de la fila un sitio.
        Separador::Tabulador => linea
            .trim_matches(|c| c == ' ' || c == '\r')
            .split('\t')
            .map(|s| s.trim().to_string())
            .collect(),
        // Las de Markdown traen barra al principio y al final.
        Separador::Barra => linea
            .trim()
            .trim_matches('|')
            .split('|')
            .map(|s| s.trim().to_string())
            .collect(),
        Separador::Espacios => {
            let t = linea.trim();
            let mut v = Vec::new();
            let mut actual = String::new();
            let mut espacios = 0;
            for c in t.chars() {
                if c == ' ' {
                    espacios += 1;
                    continue;
                }
                if espacios >= 2 {
                    v.push(std::mem::take(&mut actual).trim().to_string());
                } else if espacios == 1 {
                    actual.push(' ');
                }
                espacios = 0;
                actual.push(c);
            }
            v.push(actual.trim().to_string());
            v
        }
    }
}

/// **La mejor rejilla que sale de un texto pegado**, o `None` si no es una
/// tabla (`TableData.grid` del movil).
///
/// Se prueba cada separador y se exige que la MAYORIA de las filas coincidan
/// en numero de columnas: un texto corriente puede llevar algun tabulador o
/// alguna barra suelta, pero no una rejilla regular. Gana el separador que de
/// mas columnas coincidentes.
pub fn rejilla_de_texto(texto: &str) -> Option<Vec<Vec<String>>> {
    let lineas: Vec<&str> = texto
        .split('\n')
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect();
    if lineas.len() < MIN_FILAS {
        return None;
    }
    let mut mejor: Option<Vec<Vec<String>>> = None;
    let mut puntos_mejor = 0;
    for sep in [Separador::Tabulador, Separador::Barra, Separador::Espacios] {
        let filas: Vec<Vec<String>> = lineas
            .iter()
            .map(|l| partir(l, sep))
            // Fuera la linea de guiones de Markdown.
            .filter(|f| {
                !f.iter()
                    .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
            })
            .collect();
        if filas.len() < MIN_FILAS {
            continue;
        }
        let mut cuentas: std::collections::BTreeMap<usize, usize> = Default::default();
        for f in &filas {
            *cuentas.entry(f.len()).or_default() += 1;
        }
        let Some((&cols, &veces)) = cuentas.iter().max_by_key(|(_, v)| **v) else {
            continue;
        };
        // Al menos dos columnas, dos filas con el mismo ancho, y la mayoria.
        if cols < 2 || veces < MIN_FILAS || veces * 2 < filas.len() {
            continue;
        }
        let puntos = veces * cols;
        if puntos > puntos_mejor {
            puntos_mejor = puntos;
            mejor = Some(filas);
        }
    }
    let filas = mejor?;
    let ancho = filas.iter().map(Vec::len).max().unwrap_or(0);
    Some(
        filas
            .into_iter()
            .map(|f| {
                (0..ancho)
                    .map(|i| f.get(i).cloned().unwrap_or_default())
                    .collect()
            })
            .collect(),
    )
}

/// La rejilla con todas las filas del mismo ancho y sin filas vacias al
/// final: lo que llega del portapapeles no viene cuadrado.
pub fn rejilla_regular(filas: &[Vec<String>]) -> Vec<Vec<String>> {
    let mut utiles: Vec<&Vec<String>> = filas.iter().collect();
    while utiles
        .last()
        .is_some_and(|f| f.iter().all(|c| c.trim().is_empty()))
    {
        utiles.pop();
    }
    let ancho = utiles.iter().map(|f| f.len()).max().unwrap_or(0);
    if utiles.is_empty() || ancho == 0 {
        return Vec::new();
    }
    utiles
        .into_iter()
        .map(|f| {
            (0..ancho)
                .map(|i| f.get(i).map_or(String::new(), |c| c.trim().to_string()))
                .collect()
        })
        .collect()
}

/// Una celda para dibujar: su texto, su negrita y cuanto ocupa. Las que tapa
/// una combinada van marcadas como `tapada` y no se dibujan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Celda {
    pub texto: String,
    pub negrita: bool,
    pub filas: usize,
    pub columnas: usize,
    pub tapada: bool,
}

impl Celda {
    /// Una celda corriente, de las de la rejilla de texto.
    pub fn de(texto: impl Into<String>) -> Celda {
        Celda {
            texto: texto.into(),
            negrita: false,
            filas: 1,
            columnas: 1,
            tapada: false,
        }
    }
}

/// **La tabla dibujada** con su esquina en `origen`. Las columnas se anchan
/// segun el texto mas largo de cada una, no a partes iguales. Vacia si no hay
/// nada que dibujar. Quien la pone en la escena la agrupa (se estampa como
/// una figura de la biblioteca), asi se coge y se mueve de una vez.
pub fn elementos_de_tabla(
    filas: &[Vec<String>],
    estilo: &Estilo,
    origen: Punto2,
    medir: Medir<'_>,
    con_cabecera: bool,
) -> Vec<Elemento> {
    let celdas: Vec<Vec<Celda>> = rejilla_regular(filas)
        .into_iter()
        .map(|f| f.into_iter().map(Celda::de).collect())
        .collect();
    elementos_de_tabla_con_juntas(&celdas, estilo, origen, medir, con_cabecera)
}

/// **La tabla dibujada con sus celdas combinadas** (lo que trae el HTML de
/// Sheets o de Excel): una combinada no lleva rayas por dentro, su texto se
/// centra en todo lo que ocupa y la negrita se conserva. `celdas` es una
/// rejilla regular: todas las filas con el mismo numero de columnas.
pub fn elementos_de_tabla_con_juntas(
    celdas: &[Vec<Celda>],
    estilo: &Estilo,
    origen: Punto2,
    medir: Medir<'_>,
    con_cabecera: bool,
) -> Vec<Elemento> {
    elementos_de_tabla_con_aspecto(
        celdas,
        estilo,
        origen,
        medir,
        con_cabecera,
        &Aspecto::default(),
    )
}

/// **Lo que se midio de una tabla de una foto** (16-oct, «que quede casi
/// identica»): el ancho de cada columna, el alto de cada fila, el fondo de
/// cada celda, el borde y el aire a la izquierda del texto. Lo que no se
/// midio (`None`) lo calcula la tabla como siempre.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Aspecto {
    pub anchos: Option<Vec<f32>>,
    pub altos: Option<Vec<f32>>,
    pub fondos: Option<Vec<Vec<Option<ColorRgba>>>>,
    /// Color y grosor de las rayas.
    pub borde: Option<(ColorRgba, f32)>,
    pub aire: Option<f32>,
}

/// **La misma tabla, con su [`Aspecto`]**: un solo motor para la tabla
/// pegada, la del asistente y la copiada de una foto.
pub fn elementos_de_tabla_con_aspecto(
    celdas: &[Vec<Celda>],
    estilo: &Estilo,
    origen: Punto2,
    medir: Medir<'_>,
    con_cabecera: bool,
    aspecto: &Aspecto,
) -> Vec<Elemento> {
    let n_filas = celdas.len();
    let columnas = celdas.first().map_or(0, Vec::len);
    if n_filas == 0 || columnas == 0 || celdas.iter().any(|f| f.len() != columnas) {
        return Vec::new();
    }
    let t = estilo.tam;
    // Quien manda en cada hueco: la celda de arriba a la izquierda de su
    // combinada. Lo que no tapa nadie es de si mismo.
    let mut dueno: Vec<Vec<(usize, usize)>> = (0..n_filas)
        .map(|f| (0..columnas).map(|c| (f, c)).collect())
        .collect();
    for (f, fila) in celdas.iter().enumerate() {
        for (c, celda) in fila.iter().enumerate() {
            if celda.tapada {
                continue;
            }
            let (alto, ancho) = (celda.filas.max(1), celda.columnas.max(1));
            for df in 0..alto.min(n_filas - f) {
                for dc in 0..ancho.min(columnas - c) {
                    dueno[f + df][c + dc] = (f, c);
                }
            }
        }
    }
    let tramo = |f: usize, c: usize| {
        let celda = &celdas[f][c];
        (
            celda.filas.max(1).min(n_filas - f),
            celda.columnas.max(1).min(columnas - c),
        )
    };
    let visibles: Vec<(usize, usize)> = (0..n_filas)
        .flat_map(|f| (0..columnas).map(move |c| (f, c)))
        .filter(|&(f, c)| dueno[f][c] == (f, c))
        .collect();
    // Con la letra con que se pintara y con su negrita: una celda medida en
    // otra letra, o en redonda, se sale de su caja. Sin medidor de verdad
    // (las pruebas puras), el que se pasa.
    let medida_de = |celda: &Celda| {
        if crate::texto::hay_medidor() {
            let e = crate::texto::EstiloDeTexto {
                negrita: celda.negrita,
                ..Default::default()
            };
            crate::texto::medida(&celda.texto, t, &estilo.familia, e)
        } else {
            medir(&celda.texto, t)
        }
    };
    let medidas: std::collections::HashMap<(usize, usize), (f32, f32)> = visibles
        .iter()
        .map(|&(f, c)| ((f, c), medida_de(&celdas[f][c])))
        .collect();
    // Primero lo que piden las celdas sueltas; luego una combinada que no
    // quepa estira su ultima columna (o fila), como hace Sheets.
    let mut anchos = vec![ANCHO_MINIMO_DE_COLUMNA; columnas];
    let mut altos = vec![ALTO_MINIMO_DE_FILA; n_filas];
    for &(f, c) in &visibles {
        let (alto, ancho) = tramo(f, c);
        let (w, h) = medidas[&(f, c)];
        if ancho == 1 {
            anchos[c] = anchos[c].max(w + AIRE_DE_CELDA * 2.0);
        }
        if alto == 1 {
            altos[f] = altos[f].max(h + AIRE_DE_CELDA * 2.0);
        }
    }
    for &(f, c) in &visibles {
        let (alto, ancho) = tramo(f, c);
        let (w, h) = medidas[&(f, c)];
        let tiene: f32 = anchos[c..c + ancho].iter().sum();
        if w + AIRE_DE_CELDA * 2.0 > tiene {
            anchos[c + ancho - 1] += w + AIRE_DE_CELDA * 2.0 - tiene;
        }
        let tiene: f32 = altos[f..f + alto].iter().sum();
        if h + AIRE_DE_CELDA * 2.0 > tiene {
            altos[f + alto - 1] += h + AIRE_DE_CELDA * 2.0 - tiene;
        }
    }
    // Lo medido en la foto manda sobre lo calculado (si cuadra en numero).
    if let Some(a) = aspecto.anchos.as_ref().filter(|a| a.len() == columnas) {
        anchos = a.clone();
    }
    if let Some(a) = aspecto.altos.as_ref().filter(|a| a.len() == n_filas) {
        altos = a.clone();
    }
    let (color_borde, grosor) = aspecto.borde.unwrap_or((estilo.color, 1.0));
    let aire = aspecto.aire.unwrap_or(AIRE_DE_CELDA);
    let ancho_total: f32 = anchos.iter().sum();
    let alto_total: f32 = altos.iter().sum();
    // Donde empieza cada columna y cada fila, acumulado una vez: calcularlo
    // dos veces es pedir que una raya quede a medio pixel de la otra.
    let izquierdas: Vec<f32> = std::iter::once(origen.x)
        .chain(anchos.iter().scan(origen.x, |a, w| {
            *a += w;
            Some(*a)
        }))
        .collect();
    let arribas: Vec<f32> = std::iter::once(origen.y)
        .chain(altos.iter().scan(origen.y, |a, h| {
            *a += h;
            Some(*a)
        }))
        .collect();

    let caja = |x: f32, y: f32, ancho: f32, alto: f32| Elemento {
        figura: Figura::Rectangulo,
        x,
        y,
        ancho,
        alto,
        trazo: color_borde,
        opacidad: estilo.opacidad,
        grosor,
        rugosidad: 0.0,
        redondo: false,
        relleno: None,
        ..Default::default()
    };
    let mut out = Vec::new();
    // Los fondos medidos, celda a celda (debajo de todo); sin ellos, el gris
    // de la cabecera de siempre.
    let fondos = aspecto
        .fondos
        .as_ref()
        .filter(|f| f.len() == n_filas && f.iter().all(|x| x.len() == columnas));
    if let Some(fondos) = fondos {
        for &(f, c) in &visibles {
            let Some(color) = fondos[f][c] else { continue };
            let (alto, ancho) = tramo(f, c);
            out.push(Elemento {
                relleno: Some(color),
                estilo_relleno: EstiloRelleno::Solido,
                trazo: ColorRgba { a: 0.0, ..color },
                ..caja(
                    izquierdas[c],
                    arribas[f],
                    izquierdas[c + ancho] - izquierdas[c],
                    arribas[f + alto] - arribas[f],
                )
            });
        }
    } else if con_cabecera && n_filas > 1 {
        out.push(Elemento {
            relleno: Some(FONDO_DE_CABECERA),
            estilo_relleno: EstiloRelleno::Solido,
            trazo: ColorRgba {
                a: 0.0,
                ..estilo.color
            },
            ..caja(origen.x, origen.y, ancho_total, altos[0])
        });
    }
    out.push(caja(origen.x, origen.y, ancho_total, alto_total));
    let estilo_borde = Estilo {
        color: color_borde,
        ..estilo.clone()
    };
    let raya = |a: (f32, f32), b: (f32, f32)| {
        elemento_linea(
            vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
            grosor,
            &estilo_borde,
        )
    };
    // Las rayas de dentro, a tramos: se corta donde una combinada cruza la
    // frontera y lo seguido va en una sola raya (una tabla sin combinadas
    // queda como siempre, una raya por frontera).
    for c in 1..columnas {
        let mut desde: Option<usize> = None;
        for f in 0..=n_filas {
            let separa = f < n_filas && dueno[f][c - 1] != dueno[f][c];
            match (separa, desde) {
                (true, None) => desde = Some(f),
                (false, Some(d)) => {
                    out.push(raya(
                        (izquierdas[c], arribas[d]),
                        (izquierdas[c], arribas[f]),
                    ));
                    desde = None;
                }
                _ => {}
            }
        }
    }
    for f in 1..n_filas {
        let mut desde: Option<usize> = None;
        for c in 0..=columnas {
            let separa = c < columnas && dueno[f - 1][c] != dueno[f][c];
            match (separa, desde) {
                (true, None) => desde = Some(c),
                (false, Some(d)) => {
                    out.push(raya(
                        (izquierdas[d], arribas[f]),
                        (izquierdas[c], arribas[f]),
                    ));
                    desde = None;
                }
                _ => {}
            }
        }
    }
    // El contenido. Las celdas vacias no dejan elemento: un texto sin nada
    // es invisible y luego roba clics.
    for &(f, c) in &visibles {
        let celda = &celdas[f][c];
        if celda.texto.trim().is_empty() {
            continue;
        }
        let (alto, _) = tramo(f, c);
        let (w, h) = medidas[&(f, c)];
        let alto_de_celda = arribas[f + alto] - arribas[f];
        let mut e = elemento_texto(
            &celda.texto,
            izquierdas[c] + aire,
            // Centrado en todo el alto que ocupa.
            arribas[f] + (alto_de_celda - h) / 2.0,
            (w, h),
            t,
            false,
            estilo,
        );
        e.extras.alineacion = Some(crate::texto::AlineacionTexto::Izquierda);
        e.extras.negrita = celda.negrita;
        out.push(e);
    }
    out
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn medir(texto: &str, tam: f32) -> (f32, f32) {
        (texto.chars().count() as f32 * tam * 0.5, tam * 1.2)
    }

    fn estilo() -> Estilo {
        Estilo {
            color: ColorRgba::opaco(0.0, 0.0, 0.0),
            tam: 20.0,
            opacidad: 1.0,
            familia: "Segoe UI".into(),
        }
    }

    fn filas(v: &[&[&str]]) -> Vec<Vec<String>> {
        v.iter()
            .map(|f| f.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    #[test]
    fn lo_copiado_de_excel_es_una_rejilla_con_sus_celdas_vacias_en_su_sitio() {
        // Excel deja tabuladores y la ultima fila con salto de linea.
        let r = rejilla_de_texto("Nombre\tImporte\tIVA\r\nAna\t12,5\t\r\n\t7\t21\r\n").unwrap();
        assert_eq!(r.len(), 3);
        assert_eq!(r[1], vec!["Ana", "12,5", ""]);
        // La celda vacia del principio no corre la fila.
        assert_eq!(r[2], vec!["", "7", "21"]);
    }

    #[test]
    fn una_tabla_de_markdown_y_una_de_columnas_alineadas_tambien() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
        let r = rejilla_de_texto(md).unwrap();
        assert_eq!(r, filas(&[&["a", "b"], &["1", "2"], &["3", "4"]]));
        let alineada = "punto   x     y\nA       10    20\nB       30    40";
        assert_eq!(
            rejilla_de_texto(alineada).unwrap()[2],
            vec!["B", "30", "40"]
        );
    }

    #[test]
    fn un_texto_corriente_no_es_una_tabla() {
        assert!(rejilla_de_texto("hola que tal").is_none());
        assert!(
            rejilla_de_texto("una linea\notra linea con\tun tabulador suelto\ny otra mas\ny otra")
                .is_none()
        );
        assert!(rejilla_de_texto("").is_none());
        assert!(
            rejilla_de_texto("a\tb").is_none(),
            "una fila sola no es tabla"
        );
    }

    #[test]
    fn las_columnas_se_anchan_con_su_texto_mas_largo() {
        let f = filas(&[&["Nombre", "N"], &["Una descripcion larga", "1"]]);
        let v = elementos_de_tabla(&f, &estilo(), Punto2::nuevo(0.0, 0.0), &medir, true);
        let marco = v
            .iter()
            .find(|e| matches!(e.figura, Figura::Rectangulo) && e.relleno.is_none())
            .unwrap();
        let (larga, _) = medir("Una descripcion larga", 20.0);
        assert!(
            (marco.ancho - (larga + 2.0 * AIRE_DE_CELDA + ANCHO_MINIMO_DE_COLUMNA)).abs() < 0.01
        );
        // Una raya vertical (dos columnas) y una horizontal (dos filas).
        assert_eq!(
            v.iter()
                .filter(|e| matches!(e.figura, Figura::Linea { .. }))
                .count(),
            2
        );
        // La cabecera: el primer elemento, con fondo gris y sin trazo.
        assert_eq!(v[0].relleno, Some(FONDO_DE_CABECERA));
        assert_eq!(v[0].trazo.a, 0.0);
        // Todo recto.
        assert!(v.iter().all(|e| e.rugosidad == 0.0));
    }

    #[test]
    fn las_celdas_vacias_no_dejan_texto_y_sin_cabecera_no_hay_fondo() {
        let f = filas(&[&["a", ""], &["", "d"], &["", ""]]);
        let v = elementos_de_tabla(&f, &estilo(), Punto2::nuevo(10.0, 10.0), &medir, false);
        assert_eq!(
            v.iter()
                .filter(|e| matches!(e.figura, Figura::Texto { .. }))
                .count(),
            2
        );
        assert!(v.iter().all(|e| e.relleno.is_none()));
        // La fila vacia del final se cae: dos filas, una raya horizontal.
        let marco = &v[0];
        let fila = (medir("a", 20.0).1 + 2.0 * AIRE_DE_CELDA).max(ALTO_MINIMO_DE_FILA);
        assert!((marco.alto - 2.0 * fila).abs() < 0.01, "{}", marco.alto);
        assert!(
            elementos_de_tabla(&[], &estilo(), Punto2::nuevo(0.0, 0.0), &medir, true).is_empty()
        );
    }

    fn tapada() -> Celda {
        Celda {
            tapada: true,
            ..Celda::de("")
        }
    }

    fn rayas(v: &[Elemento]) -> Vec<(Punto2, Punto2)> {
        v.iter()
            .filter_map(|e| match &e.figura {
                // `elemento_linea` deja los puntos en coordenadas del dibujo.
                Figura::Linea { puntos, .. } => Some((puntos[0], puntos[1])),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn una_combinada_de_tres_filas_no_lleva_rayas_por_dentro_y_su_texto_va_centrado() {
        // DESCRIPCION | MONTO
        // RUBY (x3)   | 690 / 687.5 / 685.4
        let celdas = vec![
            vec![
                Celda {
                    negrita: true,
                    ..Celda::de("DESCRIPCION")
                },
                Celda {
                    negrita: true,
                    ..Celda::de("MONTO")
                },
            ],
            vec![
                Celda {
                    filas: 3,
                    ..Celda::de("RUBY")
                },
                Celda::de("690"),
            ],
            vec![tapada(), Celda::de("687.5")],
            vec![tapada(), Celda::de("685.4")],
        ];
        let v = elementos_de_tabla_con_juntas(
            &celdas,
            &estilo(),
            Punto2::nuevo(0.0, 0.0),
            &medir,
            false,
        );
        let marco = &v[0];
        let fila = (medir("a", 20.0).1 + 2.0 * AIRE_DE_CELDA).max(ALTO_MINIMO_DE_FILA);
        let col0 =
            (medir("DESCRIPCION", 20.0).0 + 2.0 * AIRE_DE_CELDA).max(ANCHO_MINIMO_DE_COLUMNA);
        // La vertical, entera; horizontales: la de la cabecera entera y dos
        // que solo cruzan la columna del monto.
        let r = rayas(&v);
        assert_eq!(r.len(), 4, "{r:?}");
        let horizontales: Vec<_> = r.iter().filter(|(a, b)| (a.y - b.y).abs() < 0.01).collect();
        assert_eq!(horizontales.len(), 3);
        let cortas = horizontales
            .iter()
            .filter(|(a, _)| (a.x - col0).abs() < 0.01)
            .count();
        assert_eq!(
            cortas, 2,
            "las rayas de las filas 2 y 3 empiezan tras la combinada"
        );
        assert!(
            horizontales
                .iter()
                .all(|(a, b)| a.x.min(b.x) >= -0.01 && a.x.max(b.x) <= marco.ancho + 0.01)
        );
        // El texto de la combinada, centrado en sus tres filas.
        let ruby = v
            .iter()
            .find(|e| matches!(&e.figura, Figura::Texto { texto, .. } if texto == "RUBY"))
            .unwrap();
        let centro = ruby.y + ruby.alto / 2.0;
        assert!((centro - (fila + 1.5 * fila)).abs() < 0.01, "{centro}");
        // La negrita viaja; lo demas no la lleva.
        let negritas: Vec<_> = v.iter().filter(|e| e.extras.negrita).collect();
        assert_eq!(negritas.len(), 2);
        // Las tapadas no dejan texto.
        assert_eq!(
            v.iter()
                .filter(|e| matches!(e.figura, Figura::Texto { .. }))
                .count(),
            6
        );
    }

    #[test]
    fn una_combinada_de_columnas_ancha_estira_la_ultima_que_ocupa() {
        let largo = "un titulo muy largo que no cabe";
        let celdas = vec![
            vec![
                Celda {
                    columnas: 2,
                    ..Celda::de(largo)
                },
                tapada(),
            ],
            vec![Celda::de("a"), Celda::de("b")],
        ];
        let v = elementos_de_tabla_con_juntas(
            &celdas,
            &estilo(),
            Punto2::nuevo(0.0, 0.0),
            &medir,
            false,
        );
        let marco = &v[0];
        assert!((marco.ancho - (medir(largo, 20.0).0 + 2.0 * AIRE_DE_CELDA)).abs() < 0.01);
        // La vertical solo en la segunda fila.
        let verticales: Vec<_> = rayas(&v)
            .into_iter()
            .filter(|(a, b)| (a.x - b.x).abs() < 0.01)
            .collect();
        assert_eq!(verticales.len(), 1);
        assert!(verticales[0].0.y.min(verticales[0].1.y) > 0.01);
    }

    #[test]
    fn una_rejilla_torcida_no_se_dibuja() {
        let celdas = vec![vec![Celda::de("a"), Celda::de("b")], vec![Celda::de("c")]];
        assert!(
            elementos_de_tabla_con_juntas(
                &celdas,
                &estilo(),
                Punto2::nuevo(0.0, 0.0),
                &medir,
                false
            )
            .is_empty()
        );
    }
}
