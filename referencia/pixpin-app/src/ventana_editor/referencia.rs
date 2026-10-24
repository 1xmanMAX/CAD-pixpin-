//! **La imagen de referencia**: la foto que se esta copiando, flotando
//! encima del lienzo (`VentanaDeReferencia.kt`).
//!
//! Dibujar mirando una referencia en otra aplicacion es cambiar de ventana
//! cada treinta segundos, y en cada vuelta se pierde el sitio. Teniendola
//! encima, en una ventanita que se aparta a donde no estorbe, no hay que
//! salir del lienzo en toda la faena.
//!
//! # Lo que se copia del movil y lo que pone Windows
//!
//! Del movil: se abre desde el menu del lienzo (alla, «Referencias → Imagen
//! de referencia» en sus ajustes; aqui, el clic derecho, que es el menu del
//! lienzo) y **la misma entrada la quita** si ya esta puesta; nace pequena
//! (180 de lado, la imagen entera dentro), arriba a la izquierda y corrida
//! 180 x 140 para no quedar bajo la barra; y la foto se lee reducida a 1200
//! de lado, que para una ventanita sobra.
//!
//! De Windows: la ventanita es un **pin** (`pixpin_pin::Pin`), que ya sabe
//! moverse arrastrando, estirarse por el borde, acercar con la rueda, girar y
//! dejar pasar el clic. Es lo que en el movil hacen la cabecera, la esquina
//! y los dos dedos, y el pin lo trae hecho: no se escribe otra ventana.
//! Vive en el hilo del lienzo (su bucle ya despacha todos los mensajes del
//! hilo) y se cierra con el.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use pixpin_codec::ImagenRgba;
use pixpin_geom::Rect;
use pixpin_pin::{CambioPin, Contenido, Pin};
use pixpin_render::MotorRender;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

/// Lo mas que se guarda de lado (`LADO_DE_LA_REFERENCIA`).
pub const LADO_DE_LA_FOTO: u32 = 1200;
/// El lado de la ventanita al nacer, en puntos (`LADO_INICIAL`).
pub const LADO_INICIAL: u32 = 180;
/// Donde nace, desde la esquina del lienzo (`SITIO_INICIAL_X/Y`).
pub const SITIO_INICIAL: (u32, u32) = (180, 140);

/// El tamano al que se lee una foto: entera, sin pasar de 1200 de lado.
pub fn tamano_de_la_foto(ancho: u32, alto: u32) -> (u32, u32) {
    let mayor = ancho.max(alto).max(1);
    if mayor <= LADO_DE_LA_FOTO {
        return (ancho.max(1), alto.max(1));
    }
    let f = LADO_DE_LA_FOTO as f32 / mayor as f32;
    (
        ((ancho as f32 * f).round() as u32).max(1),
        ((alto as f32 * f).round() as u32).max(1),
    )
}

/// **Donde nace la ventanita**: la imagen entera dentro de un cuadrado de
/// 180 puntos (`ContentScale.Fit`), corrida 180 x 140 desde la esquina del
/// lienzo, en pixeles fisicos.
pub fn sitio(lienzo: Rect, escala_por_cien: u32, ancho: u32, alto: u32) -> Rect {
    let e = |v: u32| v * escala_por_cien / 100;
    let lado = e(LADO_INICIAL) as f32;
    let f = lado / ancho.max(alto).max(1) as f32;
    Rect {
        x: lienzo.x + e(SITIO_INICIAL.0) as i32,
        y: lienzo.y + e(SITIO_INICIAL.1) as i32,
        ancho: ((ancho as f32 * f).round() as u32).max(1),
        alto: ((alto as f32 * f).round() as u32).max(1),
    }
}

/// Lo pidio el menu del clic derecho, que no tiene el lienzo a mano: el
/// bucle lo recoge con [`tomar_pedido`], como el «Fondo del lienzo».
static PEDIDA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn pedir() {
    PEDIDA.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub fn tomar_pedido() -> bool {
    PEDIDA.swap(false, std::sync::atomic::Ordering::SeqCst)
}

/// La ventanita puesta.
struct Puesta {
    pin: Pin,
    foto: ImagenRgba,
    cerrada: Rc<Cell<bool>>,
    pedidos: Rc<RefCell<Vec<CambioPin>>>,
}

/// La imagen de referencia del lienzo: ninguna o una, como en el movil.
#[derive(Default)]
pub struct Referencia {
    puesta: Option<Puesta>,
    /// El motor del pin, del mismo dispositivo que el lienzo. Se crea la
    /// primera vez que hace falta: la mayoria de los lienzos no lo usan.
    motor: Option<Rc<MotorRender>>,
}

impl Referencia {
    #[cfg(test)]
    pub fn puesta(&self) -> bool {
        self.puesta.is_some()
    }

    /// Quita la que haya. Devuelve si habia una.
    pub fn quitar(&mut self) -> bool {
        self.puesta.take().is_some()
    }

    /// **Pone esta foto** encima del lienzo `lienzo` (pixeles del
    /// escritorio). Una que ya estuviera se cambia por esta («Poner otra
    /// imagen» del movil).
    pub fn poner(
        &mut self,
        d3d: &ID3D11Device,
        foto: ImagenRgba,
        lienzo: Rect,
        escala_por_cien: u32,
        textos: pixpin_pin::TextosPin,
    ) -> anyhow::Result<()> {
        use anyhow::Context;
        let (w, h) = tamano_de_la_foto(foto.ancho, foto.alto);
        let foto = if (w, h) != (foto.ancho, foto.alto) {
            pixpin_codec::redimensionar(foto, w, h).context("no se pudo reducir la referencia")?
        } else {
            foto
        };
        let motor = match &self.motor {
            Some(m) => Rc::clone(m),
            None => {
                let m = Rc::new(MotorRender::nuevo(d3d).context("sin motor para la referencia")?);
                self.motor = Some(Rc::clone(&m));
                m
            }
        };
        let cerrada = Rc::new(Cell::new(false));
        let pedidos = Rc::new(RefCell::new(Vec::new()));
        let (c, p) = (Rc::clone(&cerrada), Rc::clone(&pedidos));
        let rect = sitio(lienzo, escala_por_cien, foto.ancho, foto.alto);
        let pin = Pin::nuevo(
            d3d,
            motor,
            Contenido::Imagen(foto.clone()),
            rect,
            escala_por_cien,
            true,
            16,
            Box::new(move |cambio| match cambio {
                CambioPin::Cerrado | CambioPin::EliminarPedido => c.set(true),
                // Moverla o estirarla no se guarda: la referencia vive lo
                // que vive el lienzo, como en el movil.
                CambioPin::Movido(_) | CambioPin::Redimensionado(_) => {}
                otro => p.borrow_mut().push(otro),
            }),
        )
        .map_err(|e| anyhow::anyhow!("no se pudo abrir la ventana de la referencia: {e:?}"))?;
        pin.poner_textos(textos);
        self.puesta = Some(Puesta {
            pin,
            foto,
            cerrada,
            pedidos,
        });
        tracing::info!(?rect, "imagen de referencia puesta");
        Ok(())
    }

    /// Lo que pidio la ventanita desde su menu. Se cierra si se pidio;
    /// copiar deja la foto en el portapapeles. Lo demas del menu del pin
    /// (grupos, abrir en el lienzo) no tiene sentido para una referencia
    /// que no esta en el almacen, y no hace nada.
    pub fn al_dia(&mut self) {
        let Some(p) = &self.puesta else { return };
        if p.cerrada.get() {
            self.puesta = None;
            tracing::info!("imagen de referencia cerrada");
            return;
        }
        let pedidos: Vec<CambioPin> = p.pedidos.borrow_mut().drain(..).collect();
        for c in pedidos {
            if let CambioPin::CopiarPedido = c
                && let Err(e) = pixpin_codec::copiar_imagen(&p.foto)
            {
                tracing::warn!(?e, "no se pudo copiar la referencia");
            }
        }
        let _ = &p.pin;
    }
}

/// **La entrada del menu**: con una puesta la quita (el movil no tiene otro
/// boton de cerrar); sin ninguna, pide una imagen con el selector de Windows
/// y la pone. `true` si cambio algo.
pub fn alternar(
    r: &mut Referencia,
    propietaria: windows::Win32::Foundation::HWND,
    d3d: &ID3D11Device,
    lienzo: Rect,
    escala_por_cien: u32,
) -> bool {
    if r.quitar() {
        return true;
    }
    let Some(ruta) = pixpin_shell::elegir::pedir_imagenes(propietaria)
        .into_iter()
        .next()
    else {
        return false;
    };
    let foto = match pixpin_codec::cargar(&ruta) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(?e, ruta = %ruta.display(), "referencia que no se pudo abrir");
            return false;
        }
    };
    let textos = crate::textos_del_pin(super::exportar::textos());
    match r.poner(d3d, foto, lienzo, escala_por_cien, textos) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(?e, "no se pudo poner la imagen de referencia");
            false
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_foto_grande_se_lee_a_mil_doscientos_de_lado_y_una_pequena_tal_cual() {
        assert_eq!(tamano_de_la_foto(4000, 3000), (1200, 900));
        assert_eq!(tamano_de_la_foto(3000, 4000), (900, 1200));
        // Caso negativo: una pequena no se agranda.
        assert_eq!(tamano_de_la_foto(640, 480), (640, 480));
        assert_eq!(
            tamano_de_la_foto(0, 0),
            (1, 1),
            "sin tamano no se divide por cero"
        );
    }

    #[test]
    fn nace_pequena_entera_y_corrida_desde_la_esquina_del_lienzo() {
        let lienzo = Rect {
            x: 100,
            y: 50,
            ancho: 1920,
            alto: 1080,
        };
        // Apaisada: 180 de ancho y la altura que le toca.
        let r = sitio(lienzo, 100, 1200, 600);
        assert_eq!((r.x, r.y, r.ancho, r.alto), (280, 190, 180, 90));
        // A 150 % todo crece con la escala, y una vertical manda por su alto.
        let r = sitio(lienzo, 150, 600, 1200);
        assert_eq!((r.x, r.y, r.ancho, r.alto), (370, 260, 135, 270));
    }

    #[test]
    fn sin_referencia_puesta_quitar_no_hace_nada() {
        let mut r = Referencia::default();
        assert!(!r.puesta());
        assert!(!r.quitar());
        r.al_dia();
        assert!(!r.puesta());
    }
}
