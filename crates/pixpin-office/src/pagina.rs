//! **El modelo de lo que se pinta**: una pagina (o diapositiva) ya maquetada,
//! con todo en su sitio y en puntos (1/72 de pulgada), origen arriba a la
//! izquierda de la hoja.
//!
//! Es lo unico que comparten la maquetacion (pura, en este crate) y el
//! pintor (`pixpin-render::pagina_office`): el pintor no sabe nada de Word
//! ni de PowerPoint, solo de textos, rectangulos, lineas, trayectos e
//! imagenes ya colocados.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Un color con alfa, en 8 bits por canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const NEGRO: Color = Color::rgb(0, 0, 0);
    pub const BLANCO: Color = Color::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255 }
    }

    /// De `RRGGBB` (lo que escriben Word y PowerPoint). `None` si no lo es.
    pub fn hex(s: &str) -> Option<Color> {
        let s = s.trim().trim_start_matches('#');
        if s.len() != 6 || !s.is_ascii() {
            return None;
        }
        let v = u32::from_str_radix(s, 16).ok()?;
        Some(Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }

    pub fn con_alfa(self, a: f32) -> Color {
        Color {
            a: (a.clamp(0.0, 1.0) * self.a as f32).round() as u8,
            ..self
        }
    }
}

/// La letra de un texto: familia tal como la pide el documento, tamano en
/// puntos y estilo. El medidor y el pintor deciden la sustituta si falta.
#[derive(Debug, Clone, PartialEq)]
pub struct Letra {
    pub familia: Arc<str>,
    pub tam: f32,
    pub negrita: bool,
    pub cursiva: bool,
}

impl Letra {
    pub fn nueva(familia: &str, tam: f32) -> Letra {
        Letra {
            familia: Arc::from(familia),
            tam,
            negrita: false,
            cursiva: false,
        }
    }
}

/// Un rectangulo en puntos.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl Rect {
    pub const fn nuevo(x: f32, y: f32, ancho: f32, alto: f32) -> Rect {
        Rect { x, y, ancho, alto }
    }
    pub fn derecha(&self) -> f32 {
        self.x + self.ancho
    }
    pub fn abajo(&self) -> f32 {
        self.y + self.alto
    }
    pub fn contiene(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.derecha() && y >= self.y && y <= self.abajo()
    }
}

/// Como se trazan una linea o un borde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Discontinuo {
    #[default]
    Solida,
    Rayas,
    Puntos,
    RayaPunto,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trazo {
    pub color: Color,
    pub grosor: f32,
    pub discontinuo: Discontinuo,
}

impl Trazo {
    pub fn solido(color: Color, grosor: f32) -> Trazo {
        Trazo {
            color,
            grosor,
            discontinuo: Discontinuo::Solida,
        }
    }
}

/// Como se rellena una figura.
#[derive(Debug, Clone, PartialEq)]
pub enum Relleno {
    Solido(Color),
    /// Degradado lineal: paradas (posicion 0..1, color) y el angulo en
    /// grados (0 = de izquierda a derecha, 90 = de arriba abajo).
    Lineal {
        paradas: Vec<(f32, Color)>,
        angulo: f32,
    },
    /// Degradado radial desde el centro de la caja.
    Radial {
        paradas: Vec<(f32, Color)>,
    },
}

impl Relleno {
    /// El color que mejor resume el relleno (el de un degradado, su media).
    pub fn color_medio(&self) -> Color {
        match self {
            Relleno::Solido(c) => *c,
            Relleno::Lineal { paradas, .. } | Relleno::Radial { paradas } => {
                if paradas.is_empty() {
                    return Color::BLANCO;
                }
                let n = paradas.len() as u32;
                let s = paradas.iter().fold([0u32; 4], |a, (_, c)| {
                    [
                        a[0] + c.r as u32,
                        a[1] + c.g as u32,
                        a[2] + c.b as u32,
                        a[3] + c.a as u32,
                    ]
                });
                Color {
                    r: (s[0] / n) as u8,
                    g: (s[1] / n) as u8,
                    b: (s[2] / n) as u8,
                    a: (s[3] / n) as u8,
                }
            }
        }
    }
}

/// Un tramo de un trayecto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tramo {
    Linea(f32, f32),
    /// Bezier cubica: dos puntos de control y el final.
    Bezier((f32, f32), (f32, f32), (f32, f32)),
}

/// Como se tine una figura respecto al relleno de la forma (los
/// `fill="darken"`, `"lighten"`, `"darkenLess"` y `"lightenLess"` de los
/// trayectos de DrawingML: las caras del cubo y del bisel). Salvo `Ninguna`,
/// el pintor las rellena aparte, encima del relleno de la forma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sombra {
    #[default]
    Ninguna,
    Oscura,
    OscuraSuave,
    Clara,
    ClaraSuave,
    /// Mismo color que la forma, pero en una geometria aparte: para que dos
    /// figuras que se solapan no se agujereen entre si (regla par-impar).
    Aparte,
}

/// Una figura cerrada o abierta de un trayecto.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Figura {
    pub inicio: (f32, f32),
    pub tramos: Vec<Tramo>,
    pub cerrada: bool,
    /// Si esta figura se rellena (en las geometrias predefinidas hay
    /// trayectos solo de trazo, `fill="none"`).
    pub rellenar: bool,
    /// Si esta figura se traza.
    pub trazar: bool,
    /// Su tinte respecto al relleno (`Sombra::Ninguna` en casi todas).
    pub sombra: Sombra,
}

/// Que hay que escribir en un texto que depende de la paginacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo {
    /// El numero de esta pagina (ya va escrito en `texto`).
    Pagina,
    /// El total de paginas: el pintor lo sustituye, porque al maquetar
    /// perezosamente aun no se sabe.
    TotalPaginas,
}

/// Como se subraya.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subrayado {
    Simple,
    Doble,
    Grueso,
    Puntos,
    Rayas,
    Ondulado,
}

/// Un trozo de texto en un solo renglon, con una sola letra.
#[derive(Debug, Clone, PartialEq)]
pub struct Texto {
    /// Donde empieza el texto (izquierda).
    pub x: f32,
    /// La linea base.
    pub y: f32,
    pub letra: Letra,
    pub color: Color,
    pub texto: String,
    pub subrayado: Option<Subrayado>,
    pub tachado: bool,
    /// Espacio de mas tras cada caracter (`w:spacing` de Word, `spc` de
    /// PowerPoint), en puntos.
    pub espaciado: f32,
    /// Espacio de mas en cada espacio en blanco (justificado), en puntos.
    pub extra_espacio: f32,
    /// El ancho medido, con el espaciado y el justificado ya sumados: lo
    /// que mide el subrayado y la seleccion.
    pub ancho: f32,
    pub campo: Option<Campo>,
    /// Destino de un hipervinculo, si lo es.
    pub enlace: Option<Arc<str>>,
}

/// Un efecto de DrawingML (`a:effectLst`) sobre lo que envuelve un
/// [`Elemento::Efecto`]. Las medidas van en puntos, en las coordenadas de
/// donde esta el `Efecto` (no giran con la forma salvo que el lector ya
/// haya girado el desplazamiento: `rotWithShape`).
#[derive(Debug, Clone, PartialEq)]
pub enum Efecto {
    /// `a:outerShdw` (y `a:prstShdw`, aproximada): la silueta de lo
    /// envuelto, de `color`, desenfocada `desenfoque` (el `blurRad`),
    /// desplazada `dx`, `dy`, escalada (`sx`, `sy`) y sesgada (`kx`, `ky`
    /// en grados) respecto al punto `ancla` (fracciones de la caja: el
    /// `algn`).
    SombraExterior {
        color: Color,
        desenfoque: f32,
        dx: f32,
        dy: f32,
        escala: (f32, f32),
        sesgo: (f32, f32),
        ancla: (f32, f32),
    },
    /// `a:innerShdw`: sombra por dentro del borde, hacia `dx`, `dy`.
    SombraInterior {
        color: Color,
        desenfoque: f32,
        dx: f32,
        dy: f32,
    },
    /// `a:glow`: un halo de `color` de `radio` puntos alrededor.
    Brillo { color: Color, radio: f32 },
    /// `a:softEdge`: el borde se desvanece hacia dentro `radio` puntos.
    BordeSuave { radio: f32 },
}

impl Efecto {
    /// El desplazamiento de una sombra a partir de su `dist` (puntos) y su
    /// `dir` (grados, en el sentido de las agujas del reloj desde el eje x,
    /// con la y hacia abajo: 90 = hacia abajo).
    pub fn desplazamiento(dist: f32, dir: f32) -> (f32, f32) {
        let (s, c) = dir.to_radians().sin_cos();
        (dist * c, dist * s)
    }
}

/// Una cosa que se pinta.
#[derive(Debug, Clone, PartialEq)]
pub enum Elemento {
    Texto(Texto),
    Rect {
        rect: Rect,
        relleno: Option<Relleno>,
        borde: Option<Trazo>,
    },
    Linea {
        desde: (f32, f32),
        hasta: (f32, f32),
        trazo: Trazo,
    },
    /// Una imagen de [`Recursos`]. `recorte` son las fracciones que se
    /// quitan por la izquierda, arriba, derecha y abajo (el `srcRect`).
    Imagen {
        id: Arc<str>,
        rect: Rect,
        recorte: Option<[f32; 4]>,
        opacidad: f32,
    },
    Trayecto {
        figuras: Vec<Figura>,
        relleno: Option<Relleno>,
        borde: Option<Trazo>,
    },
    /// Elementos con una transformada (afin: `[a, b, c, d, e, f]` como en
    /// Direct2D: x' = a*x + c*y + e, y' = b*x + d*y + f) y un recorte
    /// opcional en coordenadas del grupo.
    Grupo {
        transformada: [f32; 6],
        recorte: Option<Rect>,
        elementos: Vec<Elemento>,
    },
    /// Elementos con efectos (sombras, brillo, borde suave): el pintor
    /// los pinta como siempre y ademas sus efectos. Una pagina sin efectos
    /// no tiene ninguno y no paga nada.
    Efecto {
        efectos: Vec<Efecto>,
        elementos: Vec<Elemento>,
    },
}

impl Elemento {
    /// Envuelve `self` en un [`Elemento::Efecto`] si hay efectos; si no, lo
    /// deja como esta.
    pub fn con_efectos(self, efectos: Vec<Efecto>) -> Elemento {
        if efectos.is_empty() {
            self
        } else {
            Elemento::Efecto {
                efectos,
                elementos: vec![self],
            }
        }
    }

    /// Mueve el elemento `dx`, `dy` puntos.
    pub fn trasladar(&mut self, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        match self {
            Elemento::Texto(t) => {
                t.x += dx;
                t.y += dy;
            }
            Elemento::Rect { rect, .. } | Elemento::Imagen { rect, .. } => {
                rect.x += dx;
                rect.y += dy;
            }
            Elemento::Linea { desde, hasta, .. } => {
                *desde = (desde.0 + dx, desde.1 + dy);
                *hasta = (hasta.0 + dx, hasta.1 + dy);
            }
            Elemento::Trayecto { figuras, .. } => {
                let m = |p: &mut (f32, f32)| *p = (p.0 + dx, p.1 + dy);
                for f in figuras {
                    m(&mut f.inicio);
                    for t in &mut f.tramos {
                        match t {
                            Tramo::Linea(x, y) => {
                                *x += dx;
                                *y += dy;
                            }
                            Tramo::Bezier(a, b, c) => {
                                m(a);
                                m(b);
                                m(c);
                            }
                        }
                    }
                }
            }
            Elemento::Grupo { transformada, .. } => {
                transformada[4] += dx;
                transformada[5] += dy;
            }
            Elemento::Efecto { elementos, .. } => {
                for e in elementos {
                    e.trasladar(dx, dy);
                }
            }
        }
    }

    /// La altura de referencia para repartir elementos entre dos trozos
    /// (al partir una fila de tabla): la linea base de un texto, la parte
    /// de arriba de lo demas.
    pub fn y_referencia(&self) -> f32 {
        match self {
            Elemento::Texto(t) => t.y - 0.01,
            Elemento::Rect { rect, .. } | Elemento::Imagen { rect, .. } => rect.y,
            Elemento::Linea { desde, hasta, .. } => desde.1.min(hasta.1),
            Elemento::Trayecto { figuras, .. } => {
                figuras.first().map(|f| f.inicio.1).unwrap_or(0.0)
            }
            Elemento::Grupo { transformada, .. } => transformada[5],
            Elemento::Efecto { elementos, .. } => elementos
                .iter()
                .map(|e| e.y_referencia())
                .reduce(f32::min)
                .unwrap_or(0.0),
        }
    }
}

/// Una pagina o diapositiva maquetada.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pagina {
    /// Ancho y alto de la hoja, en puntos.
    pub ancho: f32,
    pub alto: f32,
    pub fondo: Option<Relleno>,
    pub elementos: Vec<Elemento>,
    /// Numero de esta pagina (desde 1) tal como se escribe en sus campos.
    pub numero: u32,
}

impl Pagina {
    /// Todos los textos de la pagina, en orden de pintado, con su caja en
    /// coordenadas de la pagina (los grupos girados se ignoran para la
    /// caja: se da la de antes de girar trasladada). Sirve para buscar y
    /// seleccionar texto.
    pub fn textos(&self) -> Vec<(&Texto, Rect)> {
        let mut v = Vec::new();
        fn bajar<'a>(els: &'a [Elemento], dx: f32, dy: f32, v: &mut Vec<(&'a Texto, Rect)>) {
            for e in els {
                match e {
                    Elemento::Texto(t) => {
                        let alto = t.letra.tam * 1.2;
                        let r = Rect::nuevo(t.x + dx, t.y + dy - t.letra.tam * 0.9, t.ancho, alto);
                        v.push((t, r));
                    }
                    Elemento::Grupo {
                        transformada,
                        elementos,
                        ..
                    } => bajar(elementos, dx + transformada[4], dy + transformada[5], v),
                    Elemento::Efecto { elementos, .. } => bajar(elementos, dx, dy, v),
                    _ => {}
                }
            }
        }
        bajar(&self.elementos, 0.0, 0.0, &mut v);
        v
    }

    /// El texto que hay en un punto de la pagina (en puntos), si lo hay.
    pub fn texto_en(&self, x: f32, y: f32) -> Option<&Texto> {
        self.textos()
            .into_iter()
            .find(|(_, r)| r.contiene(x, y))
            .map(|(t, _)| t)
    }
}

/// Las imagenes del documento, por id (la ruta de la parte dentro del ZIP).
/// Los bytes se guardan tal como vienen (PNG, JPEG, EMF...): decodificarlos
/// es cosa del pintor, y solo cuando de verdad los pinta.
#[derive(Debug, Default)]
pub struct Recursos {
    imagenes: Mutex<HashMap<Arc<str>, Arc<[u8]>>>,
}

impl Recursos {
    pub fn nuevo() -> Arc<Recursos> {
        Arc::new(Recursos::default())
    }

    pub fn poner(&self, id: Arc<str>, bytes: Arc<[u8]>) {
        if let Ok(mut m) = self.imagenes.lock() {
            m.insert(id, bytes);
        }
    }

    pub fn tiene(&self, id: &str) -> bool {
        self.imagenes
            .lock()
            .map(|m| m.contains_key(id))
            .unwrap_or(false)
    }

    /// Los bytes de una imagen, si ya se leyeron del paquete.
    pub fn bytes(&self, id: &str) -> Option<Arc<[u8]>> {
        self.imagenes.lock().ok()?.get(id).cloned()
    }

    pub fn len(&self) -> usize {
        self.imagenes.lock().map(|m| m.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_color_se_lee_en_hexadecimal() {
        assert_eq!(Color::hex("FF8000"), Some(Color::rgb(255, 128, 0)));
        assert_eq!(Color::hex("auto"), None);
        assert_eq!(Color::hex("ññññ"), None);
    }

    #[test]
    fn el_texto_de_un_punto_se_encuentra() {
        let mut p = Pagina {
            ancho: 100.0,
            alto: 100.0,
            ..Default::default()
        };
        p.elementos.push(Elemento::Texto(Texto {
            x: 10.0,
            y: 20.0,
            letra: Letra::nueva("Arial", 10.0),
            color: Color::NEGRO,
            texto: "hola".into(),
            subrayado: None,
            tachado: false,
            espaciado: 0.0,
            extra_espacio: 0.0,
            ancho: 20.0,
            campo: None,
            enlace: None,
        }));
        assert_eq!(
            p.texto_en(15.0, 18.0).map(|t| t.texto.as_str()),
            Some("hola")
        );
        assert!(p.texto_en(50.0, 18.0).is_none());
    }
}
