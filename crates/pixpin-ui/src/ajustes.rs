//! La ventana de ajustes (v2), como logica pura.
//!
//! Aqui esta donde cae cada cosa y que pasa al pulsarla. No sabe dibujar,
//! no sabe leer el fichero de ajustes y no conoce los textos traducidos:
//! recibe las filas ya hechas y devuelve golpes. Asi la parte en la que mas
//! facil es equivocarse —que un clic acierte el control que se ve, que la
//! lista no se pueda bajar mas alla del final, que el buscador encuentre
//! «Días» escribiendo «dias»— se comprueba en milisegundos y sin ventana.
//!
//! La forma es la de la maqueta `Ajustes2`: a la izquierda el buscador y las
//! secciones, con «Deshacer» y «Listo» abajo; a la derecha el titulo de la
//! seccion con «Restablecer…» y las opciones en tarjetas, cada una con su
//! explicacion debajo.
//!
//! Todas las medidas son pixeles LOGICOS. Multiplicarlas por la escala del
//! monitor es cosa de quien dibuja.

use pixpin_geom::Punto;

/// Ancho de la columna de la izquierda (buscador y secciones).
pub const NAV_ANCHO: f32 = 248.0;
/// Margen interior de esa columna.
pub const NAV_MARGEN: f32 = 10.0;
/// Donde empieza el buscador, debajo del titulo «Ajustes».
pub const BUSCADOR_Y: f32 = 60.0;
pub const BUSCADOR_ALTO: f32 = 40.0;
/// Cada seccion de la columna: 40 de alto y 2 de aire.
pub const SECCION_ALTO: f32 = 40.0;
pub const SECCION_PASO: f32 = 42.0;
/// Los dos botones de abajo de la columna.
pub const PIE_BOTON_ALTO: f32 = 40.0;
/// Margen a los lados del contenido.
pub const CONTENIDO_MARGEN: f32 = 28.0;
/// Alto de la cabecera del contenido (titulo, subtitulo y «Restablecer»).
pub const CABECERA_ALTO: f32 = 96.0;
/// Una opcion: etiqueta y explicacion.
pub const FILA_ALTO: f32 = 60.0;
/// El titulo de un grupo de opciones.
pub const GRUPO_ALTO: f32 = 38.0;
/// Relleno a los lados dentro de una tarjeta.
pub const TARJETA_RELLENO: f32 = 16.0;
/// Alto de los botones y cajas de dentro de una fila.
pub const CONTROL_ALTO: f32 = 36.0;
/// El interruptor.
pub const INTERRUPTOR_ANCHO: f32 = 44.0;
pub const INTERRUPTOR_ALTO: f32 = 26.0;
/// La caja de un atajo.
pub const ATAJO_ANCHO: f32 = 210.0;
/// El numero: «−», el valor y «+».
pub const NUMERO_BOTON: f32 = 36.0;
pub const NUMERO_VALOR: f32 = 84.0;
/// La caja para escribir (anadir un programa).
pub const ENTRADA_ANCHO: f32 = 220.0;
/// Aire entre controles de una misma fila.
pub const HUECO: f32 = 8.0;

/// Lo que se puede tocar en una fila.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    /// **El titulo de un grupo**: no es una opcion, agrupa las que siguen
    /// en una tarjeta. Pulsarlo no hace nada.
    Grupo,
    /// Si o no.
    Interruptor(bool),
    /// Una de varias, excluyentes, en botones seguidos.
    Opcion {
        opciones: Vec<String>,
        elegida: usize,
    },
    /// Un numero con sus topes, que se cambia con «−» y «+». `texto` es como
    /// se ensena («7 días», «Nunca», «Auto»): lo pone quien sabe la unidad.
    Numero {
        valor: u32,
        minimo: u32,
        maximo: u32,
        paso: u32,
        texto: String,
    },
    /// Un atajo: el texto que se ensena y si choca con el de otro comando.
    Atajo { texto: String, choca: bool },
    /// Un boton que hace algo (abrir una carpeta, Configuracion…).
    Boton(String),
    /// Una caja para escribir y su boton («Añadir»). Lo escrito vive en
    /// [`Estado::entrada`], no aqui: la fila se rehace en cada vuelta.
    Entrada { marcador: String, boton: String },
    /// Nada a la derecha (una fila que solo se quita, como un programa de
    /// la lista).
    Nada,
}

/// Una linea de la ventana, con sus textos ya traducidos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fila {
    pub etiqueta: String,
    /// La explicacion corta de debajo. Vacia: no hay.
    pub ayuda: String,
    pub control: Control,
    /// Distinta del valor de fabrica: lleva el punto azul y el boton de
    /// volver a el.
    pub cambiado: bool,
    /// Lleva boton de quitar (la fila es algo de una lista del usuario).
    pub quitar: bool,
}

impl Fila {
    pub fn grupo(titulo: impl Into<String>) -> Self {
        Fila {
            etiqueta: titulo.into(),
            ayuda: String::new(),
            control: Control::Grupo,
            cambiado: false,
            quitar: false,
        }
    }

    pub fn es_grupo(&self) -> bool {
        self.control == Control::Grupo
    }

    pub fn alto(&self) -> f32 {
        if self.es_grupo() {
            GRUPO_ALTO
        } else {
            FILA_ALTO
        }
    }
}

/// La parte de una fila que se ha tocado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parte {
    Alternar,
    Elegir(usize),
    Menos,
    Mas,
    /// Empezar a grabar el atajo.
    Capturar,
    /// El boton de un `Control::Boton` o el de una `Control::Entrada`.
    Pulsar,
    /// La caja de escribir de una `Control::Entrada`.
    Escribir,
    /// Volver al valor de fabrica (solo si `cambiado`).
    Restablecer,
    /// Quitar la fila de su lista (solo si `quitar`).
    Quitar,
}

/// Lo que el usuario acaba de tocar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Golpe {
    /// Una seccion de la columna.
    Seccion(usize),
    /// La caja del buscador.
    Buscador,
    /// La «×» que vacia el buscador.
    BorrarBusqueda,
    /// Una parte de una fila.
    Fila(usize, Parte),
    /// «Restablecer <seccion>».
    RestablecerSeccion,
    Deshacer,
    /// El boton azul de abajo: cerrar (todo se guarda al cerrar).
    Listo,
}

/// Donde va lo que se escribe.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Foco {
    #[default]
    Nada,
    Buscador,
    /// La caja de escribir de la fila `n`.
    Entrada(usize),
}

/// Estado de la ventana entre un evento y el siguiente.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Estado {
    pub seccion: usize,
    /// Cuanto se ha bajado la lista, en pixeles logicos.
    pub desplazamiento: i32,
    /// La fila cuyo atajo se esta grabando. Mientras tiene valor, las teclas
    /// son lo que se graba, no atajos de la ventana.
    pub capturando: Option<usize>,
    pub foco: Foco,
    /// Lo escrito en el buscador. Con algo, la lista ensena los resultados
    /// de TODAS las secciones en vez de la elegida.
    pub busqueda: String,
    /// Lo escrito en la caja de anadir.
    pub entrada: String,
}

impl Estado {
    pub fn buscando(&self) -> bool {
        !self.busqueda.trim().is_empty()
    }
}

/// Un rectangulo en coordenadas logicas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recta {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl Recta {
    pub fn contiene(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.ancho && y >= self.y && y < self.y + self.alto
    }
}

// --- La columna de la izquierda -------------------------------------------

pub fn buscador() -> Recta {
    Recta {
        x: NAV_MARGEN,
        y: BUSCADOR_Y,
        ancho: NAV_ANCHO - 2.0 * NAV_MARGEN,
        alto: BUSCADOR_ALTO,
    }
}

/// La «×» que vacia el buscador, en su extremo derecho (donde, vacio, va la
/// chapita «Ctrl F»).
pub fn borrar_busqueda() -> Recta {
    let b = buscador();
    Recta {
        x: b.x + b.ancho - 40.0,
        y: b.y,
        ancho: 40.0,
        alto: b.alto,
    }
}

pub fn rect_de_seccion(i: usize) -> Recta {
    let b = buscador();
    Recta {
        x: NAV_MARGEN,
        y: b.y + b.alto + 12.0 + i as f32 * SECCION_PASO,
        ancho: NAV_ANCHO - 2.0 * NAV_MARGEN,
        alto: SECCION_ALTO,
    }
}

/// «Deshacer» y «Listo», abajo de la columna, en ese orden de arriba abajo.
/// «Listo» va SIEMPRE en el mismo sitio: la esquina de abajo.
pub fn botones_del_pie(alto: f32) -> (Recta, Recta) {
    let listo = Recta {
        x: NAV_MARGEN,
        y: alto - 16.0 - PIE_BOTON_ALTO,
        ancho: NAV_ANCHO - 2.0 * NAV_MARGEN,
        alto: PIE_BOTON_ALTO,
    };
    let deshacer = Recta {
        y: listo.y - PIE_BOTON_ALTO - 6.0,
        ..listo
    };
    (deshacer, listo)
}

// --- El contenido ---------------------------------------------------------

/// La zona donde corre la lista de opciones.
pub fn contenido(ancho: f32, alto: f32) -> Recta {
    Recta {
        x: NAV_ANCHO + CONTENIDO_MARGEN,
        y: CABECERA_ALTO,
        ancho: (ancho - NAV_ANCHO - 2.0 * CONTENIDO_MARGEN).max(0.0),
        alto: (alto - CABECERA_ALTO - 12.0).max(0.0),
    }
}

/// Un ancho para un texto sin medirlo: lo justo para que la logica pura
/// sepa donde cae cada boton. Quien dibuja centra el texto dentro.
pub fn ancho_estimado(texto: &str, tam: f32) -> f32 {
    texto.chars().count() as f32 * tam * 0.55
}

/// El boton «Restablecer <seccion>», arriba a la derecha.
pub fn boton_restablecer_seccion(ancho: f32, rotulo: &str) -> Recta {
    let w = (ancho_estimado(rotulo, 13.0) + 56.0).clamp(140.0, 300.0);
    Recta {
        x: ancho - CONTENIDO_MARGEN - w,
        y: 26.0,
        ancho: w,
        alto: 40.0,
    }
}

/// Lo que mide la lista entera.
pub fn alto_total(filas: &[Fila]) -> f32 {
    filas.iter().map(Fila::alto).sum()
}

/// Lo maximo que se puede bajar la lista sin dejar hueco al final. Cero
/// cuando todo cabe.
pub fn desplazamiento_maximo(filas: &[Fila], zona: Recta) -> i32 {
    (alto_total(filas) - zona.alto).max(0.0) as i32
}

pub fn limitar_desplazamiento(valor: i32, filas: &[Fila], zona: Recta) -> i32 {
    valor.clamp(0, desplazamiento_maximo(filas, zona))
}

/// Donde cae la fila `i`, ya descontado el desplazamiento. Puede quedar
/// fuera de la zona: quien dibuja decide si la pinta. El acierto del raton
/// y el pintado usan la misma cuenta.
pub fn rect_de_fila(filas: &[Fila], i: usize, desplazamiento: i32, zona: Recta) -> Recta {
    let arriba: f32 = filas[..i].iter().map(Fila::alto).sum();
    Recta {
        x: zona.x,
        y: zona.y + arriba - desplazamiento as f32,
        ancho: zona.ancho,
        alto: filas[i].alto(),
    }
}

/// El ancho de un boton de una eleccion.
fn ancho_de_opcion(texto: &str) -> f32 {
    (ancho_estimado(texto, 13.0) + 26.0).clamp(48.0, 150.0)
}

/// Un rectangulo de `ancho` x `alto` centrado en la fila, acabando en `derecha`.
fn caja(fila: Recta, derecha: f32, ancho: f32, alto: f32) -> Recta {
    Recta {
        x: derecha - ancho,
        y: fila.y + (fila.alto - alto) / 2.0,
        ancho,
        alto,
    }
}

/// **Las partes que se pueden tocar de una fila**, de derecha a izquierda:
/// quitar (si hay), el control, y volver al de fabrica (si cambio).
///
/// Es lo que usan a la vez el pintado y el acierto del raton, para que no
/// puedan discrepar.
pub fn partes_de_fila(fila: &Fila, r: Recta) -> Vec<(Parte, Recta)> {
    let mut v = Vec::new();
    if fila.es_grupo() {
        return v;
    }
    let mut derecha = r.x + r.ancho - TARJETA_RELLENO;
    if fila.quitar {
        let q = caja(r, derecha, CONTROL_ALTO, CONTROL_ALTO);
        derecha = q.x - HUECO;
        v.push((Parte::Quitar, q));
    }
    match &fila.control {
        Control::Grupo | Control::Nada => {}
        Control::Interruptor(_) => {
            let c = caja(r, derecha, INTERRUPTOR_ANCHO, INTERRUPTOR_ALTO);
            derecha = c.x - HUECO;
            v.push((Parte::Alternar, c));
        }
        Control::Opcion { opciones, .. } => {
            // Fondo de 3 px alrededor, como el segmentado de la maqueta.
            let anchos: Vec<f32> = opciones.iter().map(|o| ancho_de_opcion(o)).collect();
            let total: f32 =
                anchos.iter().sum::<f32>() + 2.0 * opciones.len().saturating_sub(1) as f32;
            let mut x = derecha - 3.0 - total;
            for (i, w) in anchos.iter().enumerate() {
                let c = Recta {
                    x,
                    y: r.y + (r.alto - 32.0) / 2.0,
                    ancho: *w,
                    alto: 32.0,
                };
                v.push((Parte::Elegir(i), c));
                x += w + 2.0;
            }
            derecha -= total + 6.0 + HUECO;
        }
        Control::Numero { .. } => {
            let mas = caja(r, derecha, NUMERO_BOTON, CONTROL_ALTO);
            let menos = caja(r, mas.x - NUMERO_VALOR, NUMERO_BOTON, CONTROL_ALTO);
            derecha = menos.x - HUECO;
            v.push((Parte::Menos, menos));
            v.push((Parte::Mas, mas));
        }
        Control::Atajo { .. } => {
            let c = caja(r, derecha, ATAJO_ANCHO, CONTROL_ALTO);
            derecha = c.x - HUECO;
            v.push((Parte::Capturar, c));
        }
        Control::Boton(rotulo) => {
            let w = (ancho_estimado(rotulo, 13.0) + 48.0).clamp(80.0, 340.0);
            let c = caja(r, derecha, w, CONTROL_ALTO);
            derecha = c.x - HUECO;
            v.push((Parte::Pulsar, c));
        }
        Control::Entrada { boton, .. } => {
            let w = (ancho_estimado(boton, 13.0) + 32.0).clamp(80.0, 200.0);
            let b = caja(r, derecha, w, CONTROL_ALTO);
            let e = caja(r, b.x - HUECO, ENTRADA_ANCHO, CONTROL_ALTO);
            derecha = e.x - HUECO;
            v.push((Parte::Escribir, e));
            v.push((Parte::Pulsar, b));
        }
    }
    if fila.cambiado {
        v.push((
            Parte::Restablecer,
            caja(r, derecha, CONTROL_ALTO, CONTROL_ALTO),
        ));
    }
    v
}

/// Lo que queda a la izquierda de los controles para la etiqueta y la
/// explicacion.
pub fn ancho_de_textos(fila: &Fila, r: Recta) -> f32 {
    let izquierda = r.x + TARJETA_RELLENO;
    let tope = partes_de_fila(fila, r)
        .iter()
        .map(|(_, c)| c.x)
        .fold(r.x + r.ancho - TARJETA_RELLENO, f32::min);
    // La opcion entera tiene 3 px de fondo por fuera.
    (tope - 12.0 - izquierda).max(60.0)
}

/// Que se ha tocado al pulsar en `punto`, en coordenadas logicas.
///
/// Primero la columna y la cabecera, que estan SIEMPRE encima; la lista
/// solo dentro de su zona: una fila bajada por debajo de la cabecera no
/// roba los clics del boton de restablecer.
#[allow(clippy::too_many_arguments)] // la ventana entera: medidas, filas y estado
pub fn golpe_en(
    punto: Punto,
    ancho: f32,
    alto: f32,
    cuantas_secciones: usize,
    filas: &[Fila],
    estado: &Estado,
    hay_deshacer: bool,
    restablecer_seccion: Option<Recta>,
) -> Option<Golpe> {
    let (x, y) = (punto.x as f32, punto.y as f32);
    if x < NAV_ANCHO {
        if estado.buscando() && borrar_busqueda().contiene(x, y) {
            return Some(Golpe::BorrarBusqueda);
        }
        if buscador().contiene(x, y) {
            return Some(Golpe::Buscador);
        }
        let (deshacer, listo) = botones_del_pie(alto);
        if listo.contiene(x, y) {
            return Some(Golpe::Listo);
        }
        if hay_deshacer && deshacer.contiene(x, y) {
            return Some(Golpe::Deshacer);
        }
        return (0..cuantas_secciones)
            .find(|&i| {
                let r = rect_de_seccion(i);
                r.contiene(x, y) && r.y + r.alto <= deshacer.y
            })
            .map(Golpe::Seccion);
    }
    if let Some(r) = restablecer_seccion
        && r.contiene(x, y)
    {
        return Some(Golpe::RestablecerSeccion);
    }
    let zona = contenido(ancho, alto);
    if !zona.contiene(x, y) {
        return None;
    }
    for (i, fila) in filas.iter().enumerate() {
        let r = rect_de_fila(filas, i, estado.desplazamiento, zona);
        if !r.contiene(x, y) {
            continue;
        }
        if let Some((parte, _)) = partes_de_fila(fila, r)
            .into_iter()
            .find(|(_, c)| c.contiene(x, y))
        {
            return Some(Golpe::Fila(i, parte));
        }
        // Un interruptor tambien se cambia pulsando su fila entera, como en
        // Windows y en el movil: el objetivo pequeno se queda corto.
        if matches!(fila.control, Control::Interruptor(_)) {
            return Some(Golpe::Fila(i, Parte::Alternar));
        }
        return None;
    }
    None
}

/// El numero que resulta de pulsar «−» o «+», sin salirse de sus topes. Se
/// para en el tope en vez de dar la vuelta.
pub fn numero_tras(control: &Control, subir: bool) -> Option<u32> {
    let Control::Numero {
        valor,
        minimo,
        maximo,
        paso,
        ..
    } = control
    else {
        return None;
    };
    let nuevo = if subir {
        valor.saturating_add(*paso)
    } else {
        valor.saturating_sub(*paso)
    };
    Some(nuevo.clamp(*minimo, *maximo))
}

// --- El buscador ----------------------------------------------------------

/// Un texto en minusculas y sin tildes, para que «dias» encuentre «Días».
pub fn normalizar(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            otro => otro,
        })
        .collect()
}

/// Si una fila sale al buscar `aguja`: TODAS las palabras tienen que estar
/// en la etiqueta o en la explicacion (asi «gif fotogramas» afina en vez de
/// ensanchar). Los titulos de grupo no salen solos.
pub fn coincide(aguja: &str, fila: &Fila) -> bool {
    if fila.es_grupo() {
        return false;
    }
    let pajar = normalizar(&format!("{} {}", fila.etiqueta, fila.ayuda));
    let palabras: Vec<String> = normalizar(aguja)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    !palabras.is_empty() && palabras.iter().all(|p| pajar.contains(p.as_str()))
}

/// Lo que se escribe en una caja: un caracter mas. Los de control (los que
/// manda Windows con Ctrl+F, Intro o Retroceso) no son texto.
pub fn escribir(texto: &mut String, c: char) -> bool {
    if c.is_control() || texto.chars().count() >= 120 {
        return false;
    }
    texto.push(c);
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const ANCHO: f32 = 1000.0;
    const ALTO: f32 = 680.0;

    fn punto_en(r: Recta) -> Punto {
        Punto {
            x: (r.x + r.ancho / 2.0) as i32,
            y: (r.y + r.alto / 2.0) as i32,
        }
    }

    fn fila(etiqueta: &str, control: Control) -> Fila {
        Fila {
            etiqueta: etiqueta.into(),
            ayuda: format!("ayuda de {etiqueta}"),
            control,
            cambiado: false,
            quitar: false,
        }
    }

    fn filas() -> Vec<Fila> {
        vec![
            Fila::grupo("Al capturar"),
            fila(
                "Temporizador",
                Control::Numero {
                    valor: 3,
                    minimo: 0,
                    maximo: 30,
                    paso: 1,
                    texto: "3 s".into(),
                },
            ),
            fila(
                "Formato del color",
                Control::Opcion {
                    opciones: vec!["HEX".into(), "RGB".into(), "HSL".into()],
                    elegida: 0,
                },
            ),
            fila("Apilar en la esquina", Control::Interruptor(true)),
            Fila::grupo("Programas ignorados"),
            Fila {
                quitar: true,
                ..fila("keepassxc.exe", Control::Nada)
            },
            fila(
                "Añadir",
                Control::Entrada {
                    marcador: "programa.exe".into(),
                    boton: "Añadir".into(),
                },
            ),
        ]
    }

    fn golpe(p: Punto, f: &[Fila], e: &Estado) -> Option<Golpe> {
        golpe_en(
            p,
            ANCHO,
            ALTO,
            9,
            f,
            e,
            true,
            Some(boton_restablecer_seccion(ANCHO, "Restablecer Captura")),
        )
    }

    #[test]
    fn cada_seccion_de_la_columna_se_acierta_y_mide_al_menos_40() {
        let e = Estado::default();
        for i in 0..9 {
            let r = rect_de_seccion(i);
            assert!(r.alto >= 40.0);
            assert_eq!(golpe(punto_en(r), &filas(), &e), Some(Golpe::Seccion(i)));
        }
        // Caso negativo: el titulo «Ajustes», encima del buscador, no es nada.
        assert_eq!(golpe(Punto { x: 40, y: 20 }, &filas(), &e), None);
    }

    #[test]
    fn el_buscador_se_acierta_y_su_equis_solo_existe_buscando() {
        let mut e = Estado::default();
        let x = punto_en(borrar_busqueda());
        // Caso negativo: vacio, la equis es parte de la caja.
        assert_eq!(golpe(x, &filas(), &e), Some(Golpe::Buscador));
        e.busqueda = "gif".into();
        assert_eq!(golpe(x, &filas(), &e), Some(Golpe::BorrarBusqueda));
        let izquierda = Punto {
            x: 40,
            y: (BUSCADOR_Y + 20.0) as i32,
        };
        assert_eq!(golpe(izquierda, &filas(), &e), Some(Golpe::Buscador));
    }

    #[test]
    fn listo_va_abajo_y_deshacer_solo_cuando_hay_algo() {
        let e = Estado::default();
        let (deshacer, listo) = botones_del_pie(ALTO);
        assert!(listo.y + listo.alto <= ALTO && listo.y > deshacer.y);
        assert_eq!(golpe(punto_en(listo), &filas(), &e), Some(Golpe::Listo));
        assert_eq!(
            golpe(punto_en(deshacer), &filas(), &e),
            Some(Golpe::Deshacer)
        );
        // Caso negativo: sin nada que deshacer, ese hueco no hace nada.
        let sin = golpe_en(
            punto_en(deshacer),
            ANCHO,
            ALTO,
            9,
            &filas(),
            &e,
            false,
            None,
        );
        assert_eq!(sin, None);
    }

    #[test]
    fn el_numero_tiene_menos_y_mas_por_separado() {
        let f = filas();
        let zona = contenido(ANCHO, ALTO);
        let r = rect_de_fila(&f, 1, 0, zona);
        let partes = partes_de_fila(&f[1], r);
        let menos = partes.iter().find(|(p, _)| *p == Parte::Menos).unwrap().1;
        let mas = partes.iter().find(|(p, _)| *p == Parte::Mas).unwrap().1;
        assert!(menos.x + menos.ancho < mas.x, "el valor va entre los dos");
        let e = Estado::default();
        assert_eq!(
            golpe(punto_en(menos), &f, &e),
            Some(Golpe::Fila(1, Parte::Menos))
        );
        assert_eq!(
            golpe(punto_en(mas), &f, &e),
            Some(Golpe::Fila(1, Parte::Mas))
        );
        // Caso negativo: entre los dos (donde se lee el valor) no se pulsa nada.
        let entre = Punto {
            x: ((menos.x + menos.ancho + mas.x) / 2.0) as i32,
            y: (r.y + r.alto / 2.0) as i32,
        };
        assert_eq!(golpe(entre, &f, &e), None);
    }

    #[test]
    fn cada_opcion_tiene_su_sitio_y_no_se_pisan() {
        let f = filas();
        let zona = contenido(ANCHO, ALTO);
        let r = rect_de_fila(&f, 2, 0, zona);
        let partes = partes_de_fila(&f[2], r);
        assert_eq!(partes.len(), 3);
        for w in partes.windows(2) {
            assert!(w[0].1.x + w[0].1.ancho <= w[1].1.x);
        }
        let e = Estado::default();
        for (i, (_, c)) in partes.iter().enumerate() {
            assert!(c.ancho >= 44.0);
            assert_eq!(
                golpe(punto_en(*c), &f, &e),
                Some(Golpe::Fila(2, Parte::Elegir(i)))
            );
        }
    }

    #[test]
    fn un_interruptor_se_cambia_tambien_desde_su_etiqueta() {
        let f = filas();
        let zona = contenido(ANCHO, ALTO);
        let r = rect_de_fila(&f, 3, 0, zona);
        let e = Estado::default();
        let etiqueta = Punto {
            x: (r.x + 40.0) as i32,
            y: (r.y + 20.0) as i32,
        };
        assert_eq!(
            golpe(etiqueta, &f, &e),
            Some(Golpe::Fila(3, Parte::Alternar))
        );
        // Caso negativo: la etiqueta de un numero no hace nada.
        let r1 = rect_de_fila(&f, 1, 0, zona);
        let etiqueta1 = Punto {
            x: (r1.x + 40.0) as i32,
            y: (r1.y + 20.0) as i32,
        };
        assert_eq!(golpe(etiqueta1, &f, &e), None);
    }

    #[test]
    fn el_boton_de_volver_al_de_fabrica_sale_solo_si_cambio() {
        let mut f = filas();
        let zona = contenido(ANCHO, ALTO);
        let r = rect_de_fila(&f, 1, 0, zona);
        // Caso negativo: de fabrica no hay boton.
        assert!(
            !partes_de_fila(&f[1], r)
                .iter()
                .any(|(p, _)| *p == Parte::Restablecer)
        );
        f[1].cambiado = true;
        let partes = partes_de_fila(&f[1], r);
        let (_, vuelta) = partes
            .iter()
            .find(|(p, _)| *p == Parte::Restablecer)
            .unwrap();
        let menos = partes.iter().find(|(p, _)| *p == Parte::Menos).unwrap().1;
        assert!(
            vuelta.x + vuelta.ancho <= menos.x,
            "a la izquierda del control"
        );
        assert!(vuelta.ancho >= 36.0);
        assert_eq!(
            golpe(punto_en(*vuelta), &f, &Estado::default()),
            Some(Golpe::Fila(1, Parte::Restablecer))
        );
    }

    #[test]
    fn un_programa_de_la_lista_se_quita_y_la_caja_de_anadir_se_escribe() {
        let f = filas();
        let zona = contenido(ANCHO, ALTO);
        let e = Estado::default();
        let r = rect_de_fila(&f, 5, 0, zona);
        let (_, q) = partes_de_fila(&f[5], r)[0];
        assert_eq!(
            golpe(punto_en(q), &f, &e),
            Some(Golpe::Fila(5, Parte::Quitar))
        );
        let r = rect_de_fila(&f, 6, 0, zona);
        let partes = partes_de_fila(&f[6], r);
        assert_eq!(partes[0].0, Parte::Escribir);
        assert_eq!(partes[1].0, Parte::Pulsar);
        assert_eq!(
            golpe(punto_en(partes[0].1), &f, &e),
            Some(Golpe::Fila(6, Parte::Escribir))
        );
        assert_eq!(
            golpe(punto_en(partes[1].1), &f, &e),
            Some(Golpe::Fila(6, Parte::Pulsar))
        );
        // Caso negativo: un titulo de grupo no tiene partes ni se pulsa.
        assert!(partes_de_fila(&f[4], rect_de_fila(&f, 4, 0, zona)).is_empty());
        assert_eq!(golpe(punto_en(rect_de_fila(&f, 4, 0, zona)), &f, &e), None);
    }

    #[test]
    fn la_lista_se_baja_justo_hasta_el_final_y_no_tapa_la_cabecera() {
        let zona = contenido(ANCHO, ALTO);
        let pocas = filas();
        assert_eq!(desplazamiento_maximo(&pocas, zona), 0);
        let mut muchas = Vec::new();
        for _ in 0..10 {
            muchas.extend(filas());
        }
        let tope = desplazamiento_maximo(&muchas, zona);
        assert_eq!(tope, (alto_total(&muchas) - zona.alto) as i32);
        assert_eq!(limitar_desplazamiento(99_999, &muchas, zona), tope);
        assert_eq!(limitar_desplazamiento(-5, &muchas, zona), 0);
        // Bajada, una fila pasa por debajo del boton de restablecer: el
        // clic es del boton, no de la fila.
        let e = Estado {
            desplazamiento: 200,
            ..Estado::default()
        };
        let b = boton_restablecer_seccion(ANCHO, "Restablecer Captura");
        assert_eq!(
            golpe(punto_en(b), &muchas, &e),
            Some(Golpe::RestablecerSeccion)
        );
        // Caso negativo: sin boton (buscando) ese sitio no hace nada.
        let sin = golpe_en(punto_en(b), ANCHO, ALTO, 9, &muchas, &e, true, None);
        assert_eq!(sin, None);
    }

    #[test]
    fn los_numeros_se_paran_en_sus_topes() {
        let c = |valor| Control::Numero {
            valor,
            minimo: 0,
            maximo: 30,
            paso: 5,
            texto: String::new(),
        };
        assert_eq!(numero_tras(&c(10), true), Some(15));
        assert_eq!(numero_tras(&c(30), true), Some(30));
        assert_eq!(numero_tras(&c(3), false), Some(0));
        // Caso negativo: lo que no es un numero no da numero.
        assert_eq!(numero_tras(&Control::Interruptor(true), true), None);
    }

    #[test]
    fn el_buscador_no_distingue_tildes_ni_mayusculas_y_pide_todas_las_palabras() {
        let f = fila("Días hasta borrar", Control::Nada);
        assert!(coincide("dias", &f));
        assert!(coincide("DÍAS borrar", &f));
        assert!(coincide("ayuda", &f), "tambien busca en la explicacion");
        // Casos negativos: una palabra que no esta, vacio, y los grupos.
        assert!(!coincide("dias gif", &f));
        assert!(!coincide("   ", &f));
        assert!(!coincide("capturar", &Fila::grupo("Al capturar")));
    }

    #[test]
    fn escribir_ignora_los_caracteres_de_control() {
        let mut t = String::new();
        assert!(escribir(&mut t, 'k'));
        assert!(escribir(&mut t, 'é'));
        // Caso negativo: Ctrl+F (0x06), Intro y Retroceso no son texto.
        assert!(!escribir(&mut t, '\u{6}'));
        assert!(!escribir(&mut t, '\r'));
        assert!(!escribir(&mut t, '\u{8}'));
        assert_eq!(t, "ké");
    }
}
