//! Iconos vectoriales: los de Excalidraw (MIT), pintados nitidos a cualquier
//! escala.
//!
//! Un icono es lo que habia en su `<svg>`: una caja de vista y una lista de
//! `<path>` con relleno, trazo, grosor y extremos. Los datos son estaticos
//! (los genera `herramientas/iconos-excalidraw.mjs`) y aqui solo se pintan:
//! el trazado se lee y se construye como geometria UNA vez por icono y se
//! guarda en el motor; cada fotograma solo pone la transformada y dibuja.

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_BEZIER_SEGMENT, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    D2D1_FIGURE_END_OPEN, D2D1_FILL_MODE_ALTERNATE, D2D1_FILL_MODE_WINDING,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_CAP_STYLE_FLAT, D2D1_CAP_STYLE_ROUND,
    D2D1_LAYER_OPTIONS1_NONE, D2D1_LAYER_PARAMETERS1, D2D1_LINE_JOIN_MITER, D2D1_LINE_JOIN_ROUND,
    D2D1_STROKE_STYLE_PROPERTIES1, ID2D1Geometry, ID2D1PathGeometry1, ID2D1StrokeStyle,
};
use windows::core::Interface;
use windows_numerics::{Matrix3x2, Vector2};

use crate::lienzo::{Pintor, RectF};
use crate::motor::Color;
use crate::trayecto_svg::{Tramo, analizar};

/// Los iconos de Material que usa el chat del movil, copiados tal cual.
pub mod material;

/// El icono de un archivo: la hoja de su color con la esquina doblada
/// (`IconoDeArchivo` del movil, v0.98.4).
pub mod archivo;

/// De que se pinta un relleno o un trazo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pintura {
    Nada,
    /// `currentColor`: el color que pide quien pinta (el del tema).
    Actual,
    /// Blanco fijo: huecos que el icono quiere claros sobre cualquier tema.
    Blanco,
}

/// Un `<path>` del icono.
#[derive(Debug, Clone, Copy)]
pub struct TrazoIcono {
    pub d: &'static str,
    pub relleno: Pintura,
    pub trazo: Pintura,
    /// En unidades de la caja de vista: escala con el icono, como en SVG.
    pub grosor: f32,
    pub extremo_redondo: bool,
    pub union_redonda: bool,
    pub opacidad: f32,
    /// `fill-rule="evenodd"`.
    pub par_impar: bool,
    /// `transform` de SVG como `matrix(a b c d e f)`.
    pub matriz: Option<[f32; 6]>,
    /// El trazado de la mascara o recorte que limita a este, si lo hay.
    pub mascara: Option<&'static str>,
}

/// Un icono entero.
#[derive(Debug, Clone, Copy)]
pub struct Icono {
    /// `viewBox`: x, y, ancho, alto.
    pub vista: (f32, f32, f32, f32),
    pub trazos: &'static [TrazoIcono],
}

/// Producto de transformadas en el orden de Direct2D (vector fila): primero
/// `a`, despues `b`.
fn componer(a: &Matrix3x2, b: &Matrix3x2) -> Matrix3x2 {
    Matrix3x2 {
        M11: a.M11 * b.M11 + a.M12 * b.M21,
        M12: a.M11 * b.M12 + a.M12 * b.M22,
        M21: a.M21 * b.M11 + a.M22 * b.M21,
        M22: a.M21 * b.M12 + a.M22 * b.M22,
        M31: a.M31 * b.M11 + a.M32 * b.M21 + b.M31,
        M32: a.M31 * b.M12 + a.M32 * b.M22 + b.M32,
    }
}

/// La transformada que lleva la caja de vista a `caja`, centrada y sin
/// deformar (`preserveAspectRatio` por defecto de SVG): (escala, dx, dy).
pub fn encaje(vista: (f32, f32, f32, f32), caja: RectF) -> (f32, f32, f32) {
    let escala = (caja.ancho / vista.2.max(1e-6)).min(caja.alto / vista.3.max(1e-6));
    let dx = caja.x + (caja.ancho - vista.2 * escala) / 2.0 - vista.0 * escala;
    let dy = caja.y + (caja.alto - vista.3 * escala) / 2.0 - vista.1 * escala;
    (escala, dx, dy)
}

impl Pintor<'_> {
    /// Pinta `icono` encajado en `caja` con `color` como `currentColor`.
    pub fn icono(&self, icono: &Icono, caja: RectF, color: Color) {
        self.icono_con(icono, caja, color, false);
    }

    /// Como [`Pintor::icono`] pero reflejado de izquierda a derecha dentro
    /// de su caja. Es como Excalidraw ensena las puntas del PRINCIPIO de una
    /// flecha: el mismo dibujo que las del final, mirando al otro lado. Se
    /// hace con la transformada y no con otro juego de iconos para que las
    /// dos puntas no puedan dejar de ser la misma.
    pub fn icono_volteado(&self, icono: &Icono, caja: RectF, color: Color) {
        self.icono_con(icono, caja, color, true);
    }

    fn icono_con(&self, icono: &Icono, caja: RectF, color: Color, volteado: bool) {
        let c = self.motor.contexto();
        let mut previa = Matrix3x2::default();
        // SAFETY: dentro del fotograma; lectura de la transformada actual.
        unsafe { c.GetTransform(&mut previa) };
        let (escala, dx, dy) = encaje(icono.vista, caja);
        // El espejo va entre el encaje y lo que ya hubiera: x -> 2c - x,
        // con c el centro de la caja.
        let espejo = Matrix3x2 {
            M11: -1.0,
            M12: 0.0,
            M21: 0.0,
            M22: 1.0,
            M31: 2.0 * caja.x + caja.ancho,
            M32: 0.0,
        };
        let base = if volteado {
            componer(&espejo, &previa)
        } else {
            previa
        };
        let a_caja = componer(
            &Matrix3x2 {
                M11: escala,
                M12: 0.0,
                M21: 0.0,
                M22: escala,
                M31: dx,
                M32: dy,
            },
            &base,
        );

        for t in icono.trazos {
            let Some(geometria) = self.geometria_icono(t.d, t.par_impar) else {
                continue;
            };
            let transformada = match t.matriz {
                Some([a, b, cc, d, e, f]) => componer(
                    &Matrix3x2 {
                        M11: a,
                        M12: b,
                        M21: cc,
                        M22: d,
                        M31: e,
                        M32: f,
                    },
                    &a_caja,
                ),
                None => a_caja,
            };
            let mascara: Option<ID2D1Geometry> = t
                .mascara
                .and_then(|m| self.geometria_icono(m, false))
                .and_then(|g| g.cast().ok());
            // SAFETY: dentro del fotograma; la transformada es un valor.
            unsafe { c.SetTransform(&transformada) };
            let con_capa = mascara.is_some();
            if let Some(m) = mascara {
                // La mascara esta en las unidades del icono, como el trazo:
                // se empuja con la misma transformada puesta.
                let parametros = D2D1_LAYER_PARAMETERS1 {
                    contentBounds: D2D_RECT_F {
                        left: -1.0e7,
                        top: -1.0e7,
                        right: 1.0e7,
                        bottom: 1.0e7,
                    },
                    geometricMask: std::mem::ManuallyDrop::new(Some(m)),
                    maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                    maskTransform: Matrix3x2::identity(),
                    opacity: 1.0,
                    opacityBrush: std::mem::ManuallyDrop::new(None),
                    layerOptions: D2D1_LAYER_OPTIONS1_NONE,
                };
                // SAFETY: capa emparejada con el PopLayer de abajo, en el
                // mismo fotograma.
                unsafe { c.PushLayer(&parametros, None) };
                // La geometria viajaba en un ManuallyDrop: se suelta aqui
                // para no fugar su cuenta de referencias.
                drop(std::mem::ManuallyDrop::into_inner(parametros.geometricMask));
            }

            let tinte = |p: Pintura| -> Option<Color> {
                let base = match p {
                    Pintura::Nada => return None,
                    Pintura::Actual => color,
                    Pintura::Blanco => Color::BLANCO,
                };
                Some(Color {
                    a: base.a * t.opacidad,
                    ..base
                })
            };
            if let Some(pincel) = tinte(t.relleno).and_then(|col| self.motor.pincel(col)) {
                // SAFETY: dentro del fotograma; geometria y pincel vivos.
                unsafe { c.FillGeometry(&geometria, &pincel, None) };
            }
            if t.grosor > 0.0 {
                if let Some(pincel) = tinte(t.trazo).and_then(|col| self.motor.pincel(col)) {
                    let estilo = self.estilo_icono(t.extremo_redondo, t.union_redonda);
                    // SAFETY: dentro del fotograma; objetos vivos. El grosor
                    // escala con la transformada, igual que en SVG.
                    unsafe { c.DrawGeometry(&geometria, &pincel, t.grosor, estilo.as_ref()) };
                }
            }

            if con_capa {
                // SAFETY: cierra el PushLayer de arriba.
                unsafe { c.PopLayer() };
            }
        }
        // SAFETY: devolver la transformada de quien llamo.
        unsafe { c.SetTransform(&previa) };
    }

    /// Recorta lo que se pinte despues a un rectangulo de esquinas redondas,
    /// hasta [`Pintor::soltar_recorte_redondeado`].
    ///
    /// Es la misma capa con mascara que usan los iconos, con otra figura: una
    /// foto que llena su burbuja tiene que redondearse con ella, como en el
    /// movil (`clip(RoundedCornerShape)`). Sin esto las esquinas de la foto
    /// asoman por fuera de la burbuja. Si la figura no se puede crear no se
    /// recorta nada, que es mejor que no pintar la foto.
    pub fn empujar_recorte_redondeado(&self, caja: RectF, radio: f32) -> bool {
        use windows::Win32::Graphics::Direct2D::D2D1_ROUNDED_RECT;
        let figura = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: caja.x,
                top: caja.y,
                right: caja.x + caja.ancho,
                bottom: caja.y + caja.alto,
            },
            radiusX: radio,
            radiusY: radio,
        };
        // SAFETY: crear una figura sobre la factoria viva no tiene
        // precondiciones; el cast es el upcast que pide la capa.
        let mascara: Option<ID2D1Geometry> = unsafe {
            self.motor
                .fabrica()
                .CreateRoundedRectangleGeometry(&figura)
                .ok()
                .and_then(|g| g.cast().ok())
        };
        let Some(m) = mascara else {
            return false;
        };
        let c = self.motor.contexto();
        let parametros = D2D1_LAYER_PARAMETERS1 {
            contentBounds: D2D_RECT_F {
                left: -1.0e7,
                top: -1.0e7,
                right: 1.0e7,
                bottom: 1.0e7,
            },
            geometricMask: std::mem::ManuallyDrop::new(Some(m)),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            // La figura va en las coordenadas de quien pinta: Direct2D ya le
            // aplica la transformada que este puesta, como a la de un icono.
            maskTransform: Matrix3x2::identity(),
            opacity: 1.0,
            opacityBrush: std::mem::ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        };
        // SAFETY: capa emparejada con `soltar_recorte_redondeado`, que quien
        // llama pone en el mismo fotograma.
        unsafe { c.PushLayer(&parametros, None) };
        drop(std::mem::ManuallyDrop::into_inner(parametros.geometricMask));
        true
    }

    /// **Recorta lo que se pinte despues a un poligono** (en las coordenadas
    /// de quien pinta), hasta [`Pintor::soltar_recorte_redondeado`], que
    /// cierra la misma capa. Es el `clipPath(contorno)` del cristal de una
    /// lupa del movil: el cristal puede ser un ovalo, un rombo o un garabato.
    /// `false` si el poligono no vale o no se pudo crear, y entonces no se
    /// recorta nada ni hay que cerrar.
    pub fn empujar_recorte_poligono(&self, vertices: &[(f32, f32)]) -> bool {
        if vertices.len() < 3 {
            return false;
        }
        let Some(figura) = self.geometria(vertices, true) else {
            return false;
        };
        let Ok(m) = figura.cast::<ID2D1Geometry>() else {
            return false;
        };
        let c = self.motor.contexto();
        let parametros = D2D1_LAYER_PARAMETERS1 {
            contentBounds: D2D_RECT_F {
                left: -1.0e7,
                top: -1.0e7,
                right: 1.0e7,
                bottom: 1.0e7,
            },
            geometricMask: std::mem::ManuallyDrop::new(Some(m)),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: 1.0,
            opacityBrush: std::mem::ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        };
        // SAFETY: capa emparejada con `soltar_recorte_redondeado`, que quien
        // llama pone en el mismo fotograma.
        unsafe { c.PushLayer(&parametros, None) };
        drop(std::mem::ManuallyDrop::into_inner(parametros.geometricMask));
        true
    }

    /// Cierra el recorte de [`Pintor::empujar_recorte_redondeado`]. Solo se
    /// llama si aquel devolvio `true`.
    pub fn soltar_recorte_redondeado(&self) {
        // SAFETY: cierra la capa abierta por `empujar_recorte_redondeado`.
        unsafe { self.motor.contexto().PopLayer() };
    }

    fn geometria_icono(&self, d: &'static str, par_impar: bool) -> Option<ID2D1PathGeometry1> {
        // La direccion del texto estatico identifica el trazado; el bit bajo
        // separa la regla de relleno (las cadenas nunca empiezan en impar
        // por alineacion no se garantiza, asi que se desplaza antes).
        let clave = ((d.as_ptr() as usize) << 1) | usize::from(par_impar);
        if let Some(g) = self.motor.iconos.borrow().get(&clave) {
            return g.clone();
        }
        let hecha = analizar(d)
            .ok()
            .and_then(|tramos| self.construir(&tramos, par_impar));
        self.motor.iconos.borrow_mut().insert(clave, hecha.clone());
        hecha
    }

    fn construir(&self, tramos: &[Tramo], par_impar: bool) -> Option<ID2D1PathGeometry1> {
        let v = |p: (f32, f32)| Vector2 { X: p.0, Y: p.1 };
        // SAFETY: la geometria se abre, se rellena y se cierra aqui mismo; si
        // algo falla a mitad se descarta sin usarla.
        unsafe {
            let geometria = self.motor.fabrica().CreatePathGeometry().ok()?;
            let sumidero = geometria.Open().ok()?;
            sumidero.SetFillMode(if par_impar {
                D2D1_FILL_MODE_ALTERNATE
            } else {
                D2D1_FILL_MODE_WINDING
            });
            let mut abierta = false;
            let mut inicio = (0.0, 0.0);
            for tramo in tramos {
                match *tramo {
                    Tramo::Mover(p) => {
                        if abierta {
                            sumidero.EndFigure(D2D1_FIGURE_END_OPEN);
                        }
                        // Toda figura se abre rellenable: si el trazo no
                        // lleva relleno, simplemente no se rellena.
                        sumidero.BeginFigure(v(p), D2D1_FIGURE_BEGIN_FILLED);
                        abierta = true;
                        inicio = p;
                    }
                    Tramo::Linea(p) => {
                        if !abierta {
                            sumidero.BeginFigure(v(inicio), D2D1_FIGURE_BEGIN_FILLED);
                            abierta = true;
                        }
                        sumidero.AddLine(v(p));
                    }
                    Tramo::Cubica { c1, c2, fin } => {
                        if !abierta {
                            sumidero.BeginFigure(v(inicio), D2D1_FIGURE_BEGIN_FILLED);
                            abierta = true;
                        }
                        sumidero.AddBezier(&D2D1_BEZIER_SEGMENT {
                            point1: v(c1),
                            point2: v(c2),
                            point3: v(fin),
                        });
                    }
                    Tramo::Cerrar => {
                        if abierta {
                            sumidero.EndFigure(D2D1_FIGURE_END_CLOSED);
                            abierta = false;
                        }
                    }
                }
            }
            if abierta {
                sumidero.EndFigure(D2D1_FIGURE_END_OPEN);
            }
            sumidero.Close().ok()?;
            Some(geometria)
        }
    }

    /// Tambien lo usan las polilineas del lienzo (`lienzo.rs`): el mismo
    /// extremo y union redondos que pide Excalidraw a su canvas.
    pub(crate) fn estilo_icono(
        &self,
        extremo_redondo: bool,
        union_redonda: bool,
    ) -> Option<ID2D1StrokeStyle> {
        let i = usize::from(extremo_redondo) * 2 + usize::from(union_redonda);
        if let Some(e) = &self.motor.estilos_icono.borrow()[i] {
            return Some(e.clone());
        }
        let extremo = if extremo_redondo {
            D2D1_CAP_STYLE_ROUND
        } else {
            D2D1_CAP_STYLE_FLAT
        };
        let propiedades = D2D1_STROKE_STYLE_PROPERTIES1 {
            startCap: extremo,
            endCap: extremo,
            dashCap: extremo,
            lineJoin: if union_redonda {
                D2D1_LINE_JOIN_ROUND
            } else {
                D2D1_LINE_JOIN_MITER
            },
            miterLimit: 4.0,
            ..Default::default()
        };
        // SAFETY: crear un estilo de trazo sobre la factoria viva no tiene
        // precondiciones; el cast es el upcast que DrawGeometry espera.
        let estilo = unsafe {
            self.motor
                .fabrica()
                .CreateStrokeStyle(&propiedades, None)
                .ok()
                .and_then(|e| e.cast::<ID2D1StrokeStyle>().ok())
        };
        self.motor.estilos_icono.borrow_mut()[i] = estilo.clone();
        estilo
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_icono_se_encaja_centrado_sin_deformarse() {
        // Una vista cuadrada de 24 en una caja apaisada de 40x20: escala
        // 20/24 y centrada a lo ancho.
        let (e, dx, dy) = encaje(
            (0.0, 0.0, 24.0, 24.0),
            RectF {
                x: 100.0,
                y: 50.0,
                ancho: 40.0,
                alto: 20.0,
            },
        );
        assert!((e - 20.0 / 24.0).abs() < 1e-6);
        assert!((dx - 110.0).abs() < 1e-4, "{dx}");
        assert!((dy - 50.0).abs() < 1e-4, "{dy}");
    }

    #[test]
    fn componer_aplica_primero_la_de_la_izquierda() {
        let escalar = Matrix3x2 {
            M11: 2.0,
            M12: 0.0,
            M21: 0.0,
            M22: 2.0,
            M31: 0.0,
            M32: 0.0,
        };
        let mover = Matrix3x2 {
            M11: 1.0,
            M12: 0.0,
            M21: 0.0,
            M22: 1.0,
            M31: 10.0,
            M32: 0.0,
        };
        // (1,0) escalado da (2,0) y movido (12,0).
        let m = componer(&escalar, &mover);
        assert_eq!((m.M11 + m.M31, m.M12 + m.M32), (12.0, 0.0));
        // Caso negativo: al reves, (1,0) movido da (11,0) y escalado (22,0).
        let r = componer(&mover, &escalar);
        assert_eq!((r.M11 + r.M31, r.M12 + r.M32), (22.0, 0.0));
    }
}
