//! **El grafito, pintado**: el mapa de casillas de un trazo, subido a un
//! bitmap y puesto en su sitio del dibujo.
//!
//! La otra mitad de `pixpin_motor2d::tinta::grafito`, que es quien estampa los
//! sellos y pasa el diente del papel —puro, sin GPU—. Aqui solo se sube y se
//! pinta; `pixpin-render` no depende del motor, asi que el mapa llega como
//! bytes y cuatro numeros ([`MapaGrafito`]).
//!
//! ## De cerca, sin suavizar
//!
//! El mapa es la rejilla fija del dibujo: una casilla por unidad. Acercandose
//! se pinta con el **vecino mas cercano**, y cada cuadrito sale de canto vivo
//! —lo que el usuario pedia del movil: «que exista un numero fijo de cuadrados
//! y solo se pinten esos»—. Alejandose, lineal: con el vecino, las casillas
//! que caen entre dos pixeles de pantalla parpadearian al desplazarse.
//!
//! ## Un bitmap por trazo, subido una vez
//!
//! El trazo terminado se sube una vez y se guarda aqui por elemento y huella;
//! lo que cuesta cada fotograma es un `DrawBitmap`. El que se esta dibujando
//! cambia en cada fotograma y crece por la punta: de ese se sube solo el trozo
//! que cambio sobre el bitmap del fotograma anterior (ver `Vivo`).

use std::collections::HashMap;

use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows_numerics::Matrix3x2;

use crate::lienzo::{Interpolacion, Pintor, RectF};

/// Lo que pueden pesar a la vez los bitmaps de grafito en la GPU. El mismo
/// tope que el movil pone a sus mapas (`PESO_DE_LOS_LAPICES`).
const PESO_MAXIMO: usize = 96 * 1024 * 1024;

/// Un mapa de grafito tal como lo entrega el motor.
#[derive(Debug, Clone, Copy)]
pub struct MapaGrafito<'a> {
    /// RGBA sin premultiplicar, fila tras fila.
    pub rgba: &'a [u8],
    pub ancho: u32,
    pub alto: u32,
    /// Donde cae en el dibujo: `(x, y, ancho, alto)`.
    pub caja: (f32, f32, f32, f32),
    /// Cambia cuando cambia el trazo: es lo que dice si el bitmap guardado
    /// sigue valiendo.
    pub huella: u64,
    /// El giro del elemento, en radianes, alrededor de `centro`.
    pub angulo: f32,
    pub centro: (f32, f32),
    /// De que elemento es.
    pub id: u64,
    /// Un numero distinto para cada mapa distinto.
    pub generacion: u64,
    /// Si es el mapa de la generacion `.0` con solo el trozo `.1` cambiado
    /// (`x, y, ancho, alto` en pixeles del mapa): el trazo en curso.
    pub sucio: Option<(u64, (u32, u32, u32, u32))>,
}

/// **El bitmap del trazo que se esta dibujando.**
///
/// El trazo en curso cambia en cada fotograma y crece por la punta: subir su
/// mapa entero cada vez —varios megas en un garabato largo— es lo que hacia
/// ir lento el lienzo. Se guarda el bitmap del ultimo y, si el mapa nuevo es
/// ese mismo con un trozo cambiado, se sube solo el trozo (`CopyFromMemory`).
///
/// Vive en el motor (`MotorRender::grafito_vivo`) porque quien lo pinta no
/// tiene cache que pasar en ese camino, y porque un bitmap solo vale en el
/// dispositivo que lo creo: se va con el. Guardado suelto, por hilo, lo
/// sobrevivia, y un motor nuevo se quedaba esperando a la GPU (la muestra
/// de grafito lo enseno: la segunda prueba del mismo hilo no acababa).
pub(crate) struct Vivo {
    id: u64,
    generacion: u64,
    ancho: u32,
    alto: u32,
    bitmap: ID2D1Bitmap1,
}

/// Premultiplica un trozo del mapa, fila tras fila, como `premultiplicar`.
fn trozo_premultiplicado(m: &MapaGrafito<'_>, (x, y, w, h): (u32, u32, u32, u32)) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for fila in y..y + h {
        let a = ((fila * m.ancho + x) * 4) as usize;
        v.extend_from_slice(&m.rgba[a..a + (w * 4) as usize]);
    }
    crate::motor::premultiplicar(&v)
}

/// Los bitmaps ya subidos, por elemento.
///
/// Es del dispositivo, como `CacheGrano`, en la que vive: se vacia con ella.
#[derive(Default)]
pub struct CacheGrafito {
    mapas: HashMap<u64, (u64, ID2D1Bitmap1, usize)>,
    peso: usize,
    /// Cuantos bitmaps se han subido (no cuenta los reusados): lo que mide
    /// que lo quieto no se suba en cada fotograma.
    subidas: u64,
}

impl CacheGrafito {
    pub fn nueva() -> Self {
        Self::default()
    }

    /// Dispositivo perdido: los bitmaps son del viejo y no valen.
    pub fn vaciar(&mut self) {
        self.mapas.clear();
        self.peso = 0;
    }

    /// Cuantos hay subidos: para las pruebas.
    pub fn cuantos(&self) -> usize {
        self.mapas.len()
    }

    /// Cuantos bitmaps se han subido a la GPU desde que existe.
    pub fn subidas(&self) -> u64 {
        self.subidas
    }
}

/// `a` y luego `b`, en el orden de Direct2D (vector fila a la izquierda).
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

/// El giro de `angulo` radianes alrededor de `centro`.
fn giro(angulo: f32, centro: (f32, f32)) -> Matrix3x2 {
    let (s, c) = angulo.sin_cos();
    Matrix3x2 {
        M11: c,
        M12: s,
        M21: -s,
        M22: c,
        M31: centro.0 - centro.0 * c + centro.1 * s,
        M32: centro.1 - centro.0 * s - centro.1 * c,
    }
}

/// Como se muestrea el mapa a este aumento: de cerca, cada casilla de canto
/// vivo; de lejos, fundidas.
pub fn interpolacion_a(zoom: f32) -> Interpolacion {
    if zoom >= 1.0 {
        Interpolacion::Vecino
    } else {
        Interpolacion::Lineal
    }
}

impl Pintor<'_> {
    /// **Pinta girado `angulo` radianes alrededor de `centro`**, encima de la
    /// transformada que ya hubiera (el zoom y el desplazamiento del lienzo), y
    /// la deja como estaba al salir. Es lo que hace `canvas.rotate(angulo, cx,
    /// cy)` en `Renderer.kt` antes de pintar una foto girada. Sin giro no
    /// toca la transformada.
    pub fn girado(&self, centro: (f32, f32), angulo: f32, pintar: impl FnOnce(&Pintor)) {
        if angulo == 0.0 {
            pintar(self);
            return;
        }
        let c = self.motor.contexto();
        let mut previa = Matrix3x2::default();
        // SAFETY: dentro del fotograma; se lee la transformada y se restaura
        // justo despues de pintar.
        unsafe {
            c.GetTransform(&mut previa);
            c.SetTransform(&componer(&giro(angulo, centro), &previa));
        }
        pintar(self);
        // SAFETY: la misma transformada que habia.
        unsafe { c.SetTransform(&previa) };
    }

    /// **Pinta un mapa de grafito** en su caja del dibujo, con `opacidad` y
    /// muestreado segun `zoom`. Con `cache` (lo quieto), el bitmap se guarda
    /// por elemento y se reusa mientras la huella no cambie; sin ella (el
    /// trazo en curso), se sigue el bitmap del ultimo y se sube solo el trozo
    /// que cambio ([`Vivo`]).
    ///
    /// Devuelve `false` si no se pudo subir (sin memoria de video, un mapa
    /// mal formado): quien llama puede entonces pintarlo liso.
    pub fn grafito(
        &self,
        cache: Option<&mut CacheGrafito>,
        m: &MapaGrafito<'_>,
        opacidad: f32,
        zoom: f32,
    ) -> bool {
        if m.ancho == 0 || m.alto == 0 || m.rgba.len() != (m.ancho * m.alto * 4) as usize {
            return false;
        }
        let bitmap = match cache {
            Some(c) => self.bitmap_guardado(c, m),
            None => self.bitmap_vivo(m),
        };
        let Some(bitmap) = bitmap else {
            return false;
        };
        let destino = RectF {
            x: m.caja.0,
            y: m.caja.1,
            ancho: m.caja.2,
            alto: m.caja.3,
        };
        let modo = interpolacion_a(zoom);
        if m.angulo == 0.0 {
            self.bitmap_translucido(&bitmap, destino, None, modo, opacidad);
            return true;
        }
        let c = self.motor.contexto();
        let mut previa = Matrix3x2::default();
        // SAFETY: dentro del fotograma; se lee la transformada y se restaura
        // justo despues de pintar.
        unsafe {
            c.GetTransform(&mut previa);
            c.SetTransform(&componer(&giro(m.angulo, m.centro), &previa));
        }
        self.bitmap_translucido(&bitmap, destino, None, modo, opacidad);
        // SAFETY: la misma transformada que habia.
        unsafe { c.SetTransform(&previa) };
        true
    }

    /// El bitmap guardado de lo quieto, subiendolo si no esta o cambio.
    fn bitmap_guardado(&self, c: &mut CacheGrafito, m: &MapaGrafito<'_>) -> Option<ID2D1Bitmap1> {
        if let Some((_, b, _)) = c.mapas.get(&m.id).filter(|(h, _, _)| *h == m.huella) {
            return Some(b.clone());
        }
        let b = self
            .motor
            .bitmap_desde_pixeles_premultiplicado(m.ancho, m.alto, m.rgba)
            .ok()?;
        let peso = m.rgba.len();
        c.subidas += 1;
        if let Some((_, _, viejo)) = c.mapas.remove(&m.id) {
            c.peso -= viejo;
        }
        // Pasado el tope se vacia entero, como las siluetas del grano: lo que
        // siga en pantalla vuelve en un fotograma.
        if c.peso + peso > PESO_MAXIMO {
            c.vaciar();
        }
        c.peso += peso;
        c.mapas.insert(m.id, (m.huella, b.clone(), peso));
        Some(b)
    }

    /// El bitmap del trazo en curso: el mismo de antes con el trozo nuevo
    /// copiado encima si se puede; si no, uno nuevo entero.
    fn bitmap_vivo(&self, m: &MapaGrafito<'_>) -> Option<ID2D1Bitmap1> {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_U;
        let mut vivo = self.motor.grafito_vivo.borrow_mut();
        if let Some(v) = vivo
            .as_mut()
            .filter(|v| v.id == m.id && v.ancho == m.ancho && v.alto == m.alto)
        {
            if v.generacion == m.generacion {
                return Some(v.bitmap.clone());
            }
            if let Some((_, (x, y, w, h))) = m.sucio.filter(|(desde, _)| *desde == v.generacion) {
                let bien = w == 0 || h == 0 || {
                    let datos = trozo_premultiplicado(m, (x, y, w, h));
                    let rect = D2D_RECT_U {
                        left: x,
                        top: y,
                        right: x + w,
                        bottom: y + h,
                    };
                    // SAFETY: `datos` son `w * h` pixeles de cuatro bytes con
                    // paso `w * 4`, vivos durante la llamada; el rectangulo
                    // cae dentro del bitmap porque es del mismo tamano que
                    // el mapa del que sale el trozo.
                    unsafe {
                        v.bitmap
                            .CopyFromMemory(Some(&rect), datos.as_ptr().cast(), w * 4)
                            .is_ok()
                    }
                };
                if bien {
                    v.generacion = m.generacion;
                    return Some(v.bitmap.clone());
                }
            }
        }
        let bitmap = self
            .motor
            .bitmap_desde_pixeles_premultiplicado(m.ancho, m.alto, m.rgba)
            .ok()?;
        *vivo = Some(Vivo {
            id: m.id,
            generacion: m.generacion,
            ancho: m.ancho,
            alto: m.alto,
            bitmap: bitmap.clone(),
        });
        Some(bitmap)
    }
}

/// **El icono del grafito** para la barra del editor.
///
/// El movil usa el lapiz relleno de Material (`Icons.Filled.Create`), y
/// Excalidraw no tiene herramienta de grafito. Aqui va el lapiz de trazo de la
/// barra —para que se lea como un lapiz mas, que es lo que es— con tres
/// cuadritos en la raya que deja: es lo que lo distingue del lapiz de tinta a
/// primera vista, y es literalmente lo que pinta de cerca.
pub const ICONO: crate::icono::Icono = crate::icono::Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[
        crate::icono::TrazoIcono {
            d: "M4 17h3l9.5 -9.5a2.121 2.121 0 1 0 -3 -3l-9.5 9.5v3M12 5.5l3 3",
            relleno: crate::icono::Pintura::Nada,
            trazo: crate::icono::Pintura::Actual,
            grosor: 1.5,
            extremo_redondo: true,
            union_redonda: true,
            opacidad: 1.0,
            par_impar: false,
            matriz: None,
            mascara: None,
        },
        crate::icono::TrazoIcono {
            d: "M9 19.5h3v3h-3zM14 19.5h3v3h-3zM19 19.5h3v3h-3z",
            relleno: crate::icono::Pintura::Actual,
            trazo: crate::icono::Pintura::Nada,
            grosor: 0.0,
            extremo_redondo: false,
            union_redonda: false,
            opacidad: 1.0,
            par_impar: false,
            matriz: None,
            mascara: None,
        },
    ],
};

#[cfg(test)]
mod pruebas {
    use super::*;

    fn aplicar(m: &Matrix3x2, p: (f32, f32)) -> (f32, f32) {
        (
            p.0 * m.M11 + p.1 * m.M21 + m.M31,
            p.0 * m.M12 + p.1 * m.M22 + m.M32,
        )
    }

    #[test]
    fn de_cerca_cada_casilla_de_canto_vivo_y_de_lejos_fundidas() {
        assert_eq!(interpolacion_a(1.0), Interpolacion::Vecino);
        assert_eq!(interpolacion_a(8.0), Interpolacion::Vecino);
        // Caso negativo: alejado, el vecino haria parpadear las casillas.
        assert_eq!(interpolacion_a(0.5), Interpolacion::Lineal);
    }

    #[test]
    fn el_giro_deja_quieto_el_centro_y_gira_hacia_el_mismo_lado_que_el_motor() {
        let m = giro(std::f32::consts::FRAC_PI_2, (10.0, 10.0));
        let c = aplicar(&m, (10.0, 10.0));
        assert!((c.0 - 10.0).abs() < 1e-4 && (c.1 - 10.0).abs() < 1e-4);
        // `Punto2::girar` del motor lleva (20, 10) a (10, 20) con +90 grados.
        let p = aplicar(&m, (20.0, 10.0));
        assert!(
            (p.0 - 10.0).abs() < 1e-4 && (p.1 - 20.0).abs() < 1e-4,
            "{p:?}"
        );
    }

    #[test]
    fn componer_aplica_primero_el_de_la_izquierda() {
        let mover = Matrix3x2 {
            M11: 1.0,
            M12: 0.0,
            M21: 0.0,
            M22: 1.0,
            M31: 5.0,
            M32: 0.0,
        };
        let doblar = Matrix3x2 {
            M11: 2.0,
            M12: 0.0,
            M21: 0.0,
            M22: 2.0,
            M31: 0.0,
            M32: 0.0,
        };
        // Mover y luego doblar: (1 + 5) * 2.
        assert_eq!(aplicar(&componer(&mover, &doblar), (1.0, 0.0)).0, 12.0);
    }

    #[test]
    fn una_cache_nueva_no_tiene_nada_y_vaciar_la_deja_a_cero() {
        let mut c = CacheGrafito::nueva();
        assert_eq!(c.cuantos(), 0);
        c.vaciar();
        assert_eq!(c.cuantos(), 0);
        assert_eq!(c.peso, 0);
    }
}
