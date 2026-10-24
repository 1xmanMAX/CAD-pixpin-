//! **La pasada de las lupas del editor**: donde una lupa deja de ser un
//! marco y pasa a ensenar en grande lo que mira.
//!
//! # Se vuelve a dibujar, no se amplia una foto
//!
//! Es la decision del movil (`Renderer.drawLupa`) y se copia: recortar un
//! mapa de bits de lo de debajo y estirarlo ensenaria pixeles, y una lupa
//! esta para ver mejor el detalle. Asi que se pone una vista agrandada y
//! **se pinta la escena otra vez**, solo lo que cae en la zona mirada: las
//! lineas siguen finas y el texto se lee. El papel (la pagina del PDF, la
//! foto) va debajo, pedido a la resolucion de ese aumento.
//!
//! # Lo de dentro depende de la lupa, no de la camara
//!
//! Lo que ensena una lupa ya puesta es la escena y su aumento, en unidades
//! del mundo: como en el movil, que pinta dentro con la matriz de la lupa y
//! nada mas. Aqui salia distinto segun el zoom (queja del usuario: «cuando
//! hago zoom varia o cambia»): el detalle de las ordenes, el muestreo de las
//! fotos y del papel y hasta las teselas del PDF se elegian por el aumento
//! en pantalla. Ahora el detalle es fijo ([`DETALLE`]), las fotos y el
//! papel se muestrean siempre lineal, y el zoom solo decide **cuantos
//! pixeles** tiene lo pintado: lo mismo, mas o menos nitido.
//!
//! # Pintado una vez, copiado cada fotograma
//!
//! Lo de dentro se pinta en un mapa propio del tamano del cristal en
//! pantalla ([`Clave`]) y se guarda; cada fotograma solo lo copia recortado
//! al contorno. Se rehace cuando cambia algo de lo que se ve dentro (su
//! version), lo mirado, el papel o el zoom. Mover el cristal no es ninguna
//! de esas: arrastrar una lupa ya no vuelve a pintar la escena agrandada en
//! cada aviso del raton (queja: «la lupa se mueve lento»).
//!
//! # Como se engancha
//!
//! Como el mosaico (`tapar.rs`): despues del fotograma de la escena, encima
//! de lo ya pintado. Las ordenes de la lupa (`pintado.rs`) ya pusieron la
//! montura y la guia; aqui se copia lo de dentro y la montura otra vez
//! encima, que el recorte le come la mitad de dentro. Los atajos que pintan
//! solo un trozo de la escena (hornear un trazo, repintar una zona) no
//! saben de lupas: si su trozo pisa una lupa o lo que mira, piden el
//! fotograma entero ([`pisa_una_lupa`]), igual que con un mosaico. Y la
//! lupa que se arrastra en la capa de la tinta lleva su contenido con ella
//! ([`pintar_en_capa`]): la pasada de la escena se la salta.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::dibujo::pintar::{dibujar_orden, por_cada_orden};
use crate::fondo_lienzo::FondoLienzo;
use crate::imagenes_lienzo::ImagenesLienzo;
use pixpin_motor2d::cache::Cache;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::lupa_elemento as lupa;
use pixpin_motor2d::vector::Punto2;
use pixpin_render::{Interpolacion, MotorRender, RectF};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// **El detalle de lo de dentro, por unidad de aumento**: el de una vista a
/// ocho aumentos. Fijo a proposito: `ordenes_a_distancia` aligera la
/// geometria segun el aumento en pantalla, y con el de la camara lo de
/// dentro cambiaba de forma al hacer zoom. Ocho es mas fino de lo que se
/// llega a ver, y solo se paga al rehacer lo de dentro.
const DETALLE: f32 = 8.0;

/// El «zoom» con el que se piden las fotos de dentro: el que da muestreo
/// lineal (`modo_nitidez`), siempre. Es el `imagePaint` del movil.
const MUESTREO_DE_FOTOS: f32 = 2.0;

/// Cuantas pasadas se guarda lo de dentro de una lupa que ya no se ve.
const OLVIDO: u64 = 600;

thread_local! {
    /// Las ordenes que se pintan dentro de las lupas, aparte de las de la
    /// escena: van a otro nivel de detalle, y con la cache de la escena se
    /// echarian una a otra en cada fotograma.
    static CACHE: RefCell<Cache> = RefCell::new(Cache::nueva());
    /// Lo de dentro de cada lupa, ya pintado, por su id.
    static CONTENIDOS: RefCell<HashMap<u64, Contenido>> = RefCell::new(HashMap::new());
    /// Cuantas pasadas van: para soltar lo que lleva mucho sin verse.
    static PASADA: Cell<u64> = const { Cell::new(0) };
    /// Cuantas veces se ha pintado lo de dentro (no copiado): la prueba de
    /// que mover una lupa no lo rehace.
    static REHECHAS: Cell<u64> = const { Cell::new(0) };
    /// De que motor son los mapas guardados: los de otro no se pueden
    /// pintar con este.
    static MOTOR: Cell<usize> = const { Cell::new(0) };
}

/// **Suelta lo de dentro guardado de todas las lupas.** Al cerrar el
/// editor y con el dispositivo perdido: son mapas de la GPU, y guardados
/// en el hilo sobrevivirian al motor que los hizo (soltarlos al morir el
/// hilo, con el dispositivo ya cerrado, dejaba colgado el proceso).
pub fn olvidar() {
    CONTENIDOS.with(|c| c.borrow_mut().clear());
    MOTOR.with(|m| m.set(0));
}

type Caja = (f32, f32, f32, f32);

fn se_cortan(a: Caja, b: Caja) -> bool {
    a.0 <= b.2 && a.2 >= b.0 && a.1 <= b.3 && a.3 >= b.1
}

/// La caja `(x0, y0, x1, y1)` del cristal y la de lo que mira.
fn cajas(e: &Elemento, cr: &lupa::Cristal) -> (Caja, Caja) {
    let c = lupa::caja_de(e);
    let r = lupa::region(cr, c);
    (
        (c.0, c.1, c.0 + c.2, c.1 + c.3),
        (r.0, r.1, r.0 + r.2, r.1 + r.3),
    )
}

/// **El trozo del mundo que cabe en la caja del cristal**: centrado en el
/// foco y del tamano del cristal partido por el aumento. Es lo que de verdad
/// se ve dentro (la vista lleva el foco al centro del cristal, agrandado), y
/// puede ser algo mas que `region` si el cristal se estiro solo de alto.
pub(crate) fn mirado(cr: &lupa::Cristal, caja: lupa::Caja) -> Caja {
    let a = lupa::aumento_de(cr, caja);
    let f = lupa::foco_de(cr, caja);
    let (mw, mh) = (caja.2.abs() / a / 2.0, caja.3.abs() / a / 2.0);
    (f.x - mw, f.y - mh, f.x + mw, f.y + mh)
}

/// Lo que se recoge para pintar dentro: lo mirado y la region guardada.
fn recogido(e: &Elemento, cr: &lupa::Cristal) -> Caja {
    let m = mirado(cr, lupa::caja_de(e));
    let r = cajas(e, cr).1;
    (m.0.min(r.0), m.1.min(r.1), m.2.max(r.2), m.3.max(r.3))
}

/// **Si un trozo del dibujo (`mundo`, `(x0, y0, x1, y1)`) pisa una lupa o
/// lo que mira.** Quien repinta solo ese trozo tiene que pedir entonces el
/// fotograma entero: su pintado no pasa por aqui, y la lupa se quedaria
/// ensenando lo de antes (o, peor, sin nada dentro).
pub fn pisa_una_lupa(elementos: &[Elemento], mundo: Caja) -> bool {
    elementos.iter().any(|e| {
        !e.borrado
            && lupa::de(e).is_some_and(|cr| {
                let cristal = cajas(e, cr).0;
                se_cortan(cristal, mundo) || se_cortan(recogido(e, cr), mundo)
            })
    })
}

/// **Si alguna lupa mira a este trozo del mundo** (`(x0, y0, x1, y1)`): lo
/// que cambie ahi cambia tambien lo de dentro de esa lupa, este donde este su
/// cristal. Pasar por encima del cristal, en cambio, no le hace nada: la
/// pasada de las lupas pinta siempre la ultima.
pub fn alguna_mira(elementos: &[Elemento], mundo: Caja) -> bool {
    elementos
        .iter()
        .any(|e| !e.borrado && lupa::de(e).is_some_and(|cr| se_cortan(recogido(e, cr), mundo)))
}

/// Lo que se pinta dentro de una lupa: lo que no es otra ventana (lupas,
/// focos y mosaicos, que se pintarian a si mismos o necesitan su pasada)
/// y cae en lo mirado.
fn se_ve_dentro(e: &Elemento, mira: Caja) -> bool {
    if e.borrado
        || matches!(
            e.figura,
            Figura::Lupa { .. } | Figura::Foco { .. } | Figura::Mosaico { .. }
        )
    {
        return false;
    }
    let (x0, y0, x1, y1) = e.caja();
    // Con holgura por el grueso del trazo, que se sale de la caja.
    let h = e.grosor * 2.0;
    se_cortan((x0 - h, y0 - h, x1 + h, y1 + h), mira)
}

/// Las lupas que se ven con esta camara en una superficie de `ancho` x
/// `alto`: su cristal cae dentro.
pub fn a_la_vista<'a>(
    elementos: &'a [Elemento],
    camara: &Camara,
    ancho: u32,
    alto: u32,
) -> impl Iterator<Item = &'a Elemento> {
    let vista = camara.ventana(ancho as f32, alto as f32);
    elementos.iter().filter(move |e| {
        !e.borrado && lupa::de(e).is_some_and(|cr| se_cortan(cajas(e, cr).0, vista))
    })
}

/// **Lo que decide si lo de dentro ya pintado sigue valiendo.** Ni la
/// posicion del cristal ni la de la camara estan: moverlos no cambia lo que
/// se ve dentro, solo donde se copia.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Clave {
    /// Lo mirado que cubre el mapa, `(x0, y0, x1, y1)` del mundo.
    mirado: Caja,
    /// Los pixeles del mapa: la caja del cristal en pantalla.
    pixeles: (u32, u32),
    /// La huella de lo que se ve dentro ([`firma`]).
    firma: u64,
}

impl Clave {
    /// Pixeles del mapa por unidad del mundo mirado.
    fn escala(&self) -> f32 {
        self.pixeles.0 as f32 / (self.mirado.2 - self.mirado.0).max(f32::EPSILON)
    }
}

/// **Los pixeles del mapa de una lupa**: los de su cristal en pantalla, sin
/// pasar del tope de la GPU (`tope`). Con el tope, el mapa sale mas basto y
/// se estira al copiarlo: mejor eso que quedarse vacio, que es lo que le
/// pasaba al movil con la capa demasiado grande.
pub(crate) fn pixeles_para(caja: lupa::Caja, zoom: f32, tope: u32) -> (u32, u32) {
    let tope = tope.max(1) as f32;
    let (w, h) = (caja.2.abs() * zoom, caja.3.abs() * zoom);
    let reduce = (tope / w.max(h).max(1.0)).min(1.0);
    (
        ((w * reduce).ceil() as u32).clamp(1, tope as u32),
        ((h * reduce).ceil() as u32).clamp(1, tope as u32),
    )
}

/// Un `fmt::Write` que va a parar a un `Hasher`: resumir la escala de la
/// hoja por su texto sin reservar una cadena en cada fotograma.
struct AlHasher<'a, H: Hasher>(&'a mut H);
impl<H: Hasher> std::fmt::Write for AlHasher<'_, H> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

/// **La huella de lo que se ve dentro**: el id y la version de cada cosa que
/// cae en lo recogido (la version sube con cualquier cambio: moverla,
/// recolorearla, escribir en ella), el color del papel, lo subido del papel
/// y la escala de la hoja (las cotas la escriben).
fn firma(escena: &Escena, recogido: Caja, fondo: Option<&FondoLienzo>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for c in [
        escena.fondo.r,
        escena.fondo.g,
        escena.fondo.b,
        escena.fondo.a,
    ] {
        c.to_bits().hash(&mut h);
    }
    fondo.map(FondoLienzo::firma).hash(&mut h);
    if let Some(esc) = &escena.escala {
        let _ = std::fmt::Write::write_fmt(&mut AlHasher(&mut h), format_args!("{esc:?}"));
    }
    for e in escena
        .elementos
        .iter()
        .filter(|e| se_ve_dentro(e, recogido))
    {
        (e.id, e.version).hash(&mut h);
    }
    h.finish()
}

/// Lo de dentro de una lupa, ya pintado.
struct Contenido {
    clave: Clave,
    mapa: ID2D1Bitmap1,
    /// Si al pintarlo faltaban teselas del papel: se siguen pidiendo, y al
    /// llegar cambia la firma y se rehace.
    pendiente: Option<(f32, Caja)>,
    /// La ultima pasada en que se uso.
    visto: u64,
}

/// La clave que le toca ahora a la lupa `e` con este zoom.
fn clave_de(
    escena: &Escena,
    e: &Elemento,
    zoom: f32,
    tope: u32,
    fondo: Option<&FondoLienzo>,
) -> Option<Clave> {
    let cr = lupa::de(e)?;
    let caja = lupa::caja_de(e);
    if caja.2.abs() < 1.0 || caja.3.abs() < 1.0 {
        return None;
    }
    let m = mirado(cr, caja);
    let pixeles = pixeles_para(caja, zoom, tope);
    // El mapa cubre lo mirado a lo ancho; a lo alto, lo que dan sus pixeles
    // a esa misma escala (el redondeo hacia arriba se come media unidad).
    let k = pixeles.0 as f32 / (m.2 - m.0).max(f32::EPSILON);
    let mirado = (m.0, m.1, m.2, m.1 + pixeles.1 as f32 / k);
    Some(Clave {
        mirado,
        pixeles,
        firma: firma(escena, recogido(e, cr), fondo),
    })
}

/// Pinta lo de dentro de `e` en `mapa`, segun `clave`. Devuelve si faltaban
/// teselas del papel.
fn pintar_contenido(
    motor: &MotorRender,
    mapa: &ID2D1Bitmap1,
    escena: &Escena,
    e: &Elemento,
    clave: &Clave,
    fondo: Option<&FondoLienzo>,
    imagenes: &ImagenesLienzo,
) -> Option<(f32, Caja)> {
    let cr = lupa::de(e)?;
    let aumento = lupa::aumento_de(cr, lupa::caja_de(e));
    let recogido = recogido(e, cr);
    let k = clave.escala();
    let m = clave.mirado;
    let papel = crate::dibujo::pintar::a_color(escena.fondo);
    let mut faltan = 0;
    let r = motor.dibujar(mapa, |p| {
        // El papel debajo de todo: sin el, el cristal dejaria ver lo que
        // hay detras de la lupa mezclado con lo que ensena.
        p.limpiar(papel);
        p.poner_vista((0.0, 0.0), k, (-m.0 * k, -m.1 * k));
        if let Some(f) = fondo {
            faltan = f.pintar_en_lupa(p, recogido, k);
        }
        CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            for otro in escena
                .elementos
                .iter()
                .filter(|o| se_ve_dentro(o, recogido))
            {
                let grano = pixpin_motor2d::pintado::grano_de(otro);
                por_cada_orden(
                    &mut cache,
                    otro,
                    aumento * DETALLE,
                    escena.escala.as_ref(),
                    |orden| {
                        dibujar_orden(p, orden, recogido, None, imagenes, MUESTREO_DE_FOTOS, grano);
                    },
                );
            }
        });
    });
    REHECHAS.with(|n| n.set(n.get() + 1));
    if let Err(e) = r {
        tracing::warn!(?e, "no se pudo pintar dentro de una lupa");
    }
    (faltan > 0).then_some((k, recogido))
}

/// **Pone al dia lo de dentro de las lupas que se ven** y que `excluir` no
/// quita: rehace solo las que cambiaron. Devuelve cuantas se rehicieron.
/// Va antes de abrir el fotograma: pintar en su mapa es otro `dibujar`.
#[allow(clippy::too_many_arguments)]
pub fn preparar(
    motor: &MotorRender,
    escena: &Escena,
    camara: &Camara,
    fondo: Option<&FondoLienzo>,
    imagenes: &ImagenesLienzo,
    ancho: u32,
    alto: u32,
    excluir: &dyn Fn(u64) -> bool,
) -> usize {
    let pasada = PASADA.with(|p| {
        p.set(p.get() + 1);
        p.get()
    });
    let tope = motor.lado_maximo_bitmap();
    // Mapas de otro motor (otro lienzo en este hilo): no valen aqui.
    let este = motor as *const MotorRender as usize;
    if MOTOR.with(|m| m.replace(este)) != este {
        CONTENIDOS.with(|c| c.borrow_mut().clear());
    }
    let mut rehechas = 0;
    CONTENIDOS.with(|c| {
        let mut c = c.borrow_mut();
        for e in a_la_vista(&escena.elementos, camara, ancho, alto) {
            if excluir(e.id) {
                continue;
            }
            let Some(clave) = clave_de(escena, e, camara.zoom, tope, fondo) else {
                continue;
            };
            if let Some(x) = c.get_mut(&e.id).filter(|x| x.clave == clave) {
                x.visto = pasada;
                // Las teselas que faltaban se siguen pidiendo: el pedido de
                // la pantalla pudo quitarlas.
                if let (Some((k, v)), Some(f)) = (x.pendiente, fondo)
                    && f.pedir_tambien(k, v) == 0
                {
                    x.pendiente = None;
                }
                continue;
            }
            // El mapa de antes vale si mide lo mismo; si no, uno nuevo.
            let viejo = c
                .remove(&e.id)
                .filter(|x| x.clave.pixeles == clave.pixeles)
                .map(|x| x.mapa);
            let mapa = match viejo {
                Some(m) => m,
                None => match motor.mapa_de_dibujo(clave.pixeles.0, clave.pixeles.1) {
                    Ok(m) => m,
                    Err(err) => {
                        tracing::warn!(?err, "sin mapa para lo de dentro de una lupa");
                        continue;
                    }
                },
            };
            let pendiente = pintar_contenido(motor, &mapa, escena, e, &clave, fondo, imagenes);
            rehechas += 1;
            c.insert(
                e.id,
                Contenido {
                    clave,
                    mapa,
                    pendiente,
                    visto: pasada,
                },
            );
        }
        // Lo de las lupas que ya no estan, o que llevan mucho sin verse,
        // fuera: cada mapa es un cristal de pantalla en memoria de video.
        c.retain(|id, x| {
            pasada.saturating_sub(x.visto) < OLVIDO
                && escena.buscar(*id).is_some_and(|e| !e.borrado)
        });
    });
    rehechas
}

/// **Copia lo de dentro de la lupa `e`** (ya preparado) recortado a su
/// cristal, y la montura encima. `p` puede tener cualquier vista puesta: se
/// pinta en pixeles de la superficie, con la camara `camara` corrida `base`.
/// `false` si no habia nada preparado para ella.
fn copiar_dentro(
    p: &pixpin_render::Pintor<'_>,
    e: &Elemento,
    camara: &Camara,
    base: (f32, f32),
    imagenes: &ImagenesLienzo,
    ancho: u32,
    alto: u32,
) -> bool {
    let Some(cr) = lupa::de(e) else { return false };
    let caja = lupa::caja_de(e);
    let a_sup = |q: Punto2| {
        let s = camara.a_pantalla(q);
        (s.x + base.0, s.y + base.1)
    };
    let copiado = CONTENIDOS.with(|c| {
        let c = c.borrow();
        let Some(x) = c.get(&e.id) else { return false };
        let contorno: Vec<(f32, f32)> = lupa::puntos_del_cristal(cr, caja)
            .into_iter()
            .map(a_sup)
            .collect();
        p.desplazar(0.0, 0.0);
        if !p.empujar_recorte_poligono(&contorno) {
            return false;
        }
        // Lo mirado va al cristal agrandado: su esquina cae en la del
        // cristal y cada unidad mirada mide `aumento` unidades del mundo.
        let aumento = lupa::aumento_de(cr, caja);
        let foco = lupa::foco_de(cr, caja);
        let centro = Punto2::nuevo(caja.0 + caja.2 / 2.0, caja.1 + caja.3 / 2.0);
        let m = x.clave.mirado;
        let esquina = Punto2::nuevo(
            centro.x + (m.0 - foco.x) * aumento,
            centro.y + (m.1 - foco.y) * aumento,
        );
        let (x0, y0) = a_sup(esquina);
        let ancho_px = (m.2 - m.0) * aumento * camara.zoom;
        let alto_px = (m.3 - m.1) * aumento * camara.zoom;
        // A su tamano (lo normal: se pinto para este zoom), pixel a pixel;
        // estirado (el tope de la GPU), liso.
        let modo = if (ancho_px - x.clave.pixeles.0 as f32).abs() <= 1.0 {
            Interpolacion::Vecino
        } else {
            Interpolacion::Lineal
        };
        p.bitmap_con(
            &x.mapa,
            RectF {
                x: x0,
                y: y0,
                ancho: ancho_px,
                alto: alto_px,
            },
            None,
            modo,
        );
        p.soltar_recorte_redondeado();
        true
    });
    // La montura otra vez, entera y encima: el recorte se comio su mitad
    // de dentro.
    let origen = a_sup(Punto2::nuevo(0.0, 0.0));
    p.poner_vista((0.0, 0.0), camara.zoom, origen);
    if let Some(montura) = pixpin_motor2d::pintado::ordenes(e).into_iter().next() {
        let vista = camara.ventana(ancho as f32, alto as f32);
        dibujar_orden(p, &montura, vista, None, imagenes, camara.zoom, None);
    }
    copiado
}

/// **Pinta dentro de cada lupa que se ve**, salvo las que `excluir` quita
/// (la que se arrastra en la capa de la tinta, que lleva su contenido con
/// ella). `camara` es la de la superficie (ya corrida el colchon, como en
/// `tapar::pasar`). Devuelve cuantas pinto, que es lo que una prueba sin
/// GPU puede mirar.
#[allow(clippy::too_many_arguments)]
pub fn pasar(
    motor: &mut MotorRender,
    destino: &ID2D1Bitmap1,
    escena: &Escena,
    camara: &Camara,
    fondo: Option<&FondoLienzo>,
    imagenes: &ImagenesLienzo,
    ancho: u32,
    alto: u32,
    excluir: &dyn Fn(u64) -> bool,
) -> usize {
    let lupas: Vec<&Elemento> = a_la_vista(&escena.elementos, camara, ancho, alto)
        .filter(|e| !excluir(e.id))
        .collect();
    if lupas.is_empty() {
        return 0;
    }
    preparar(motor, escena, camara, fondo, imagenes, ancho, alto, excluir);
    let mut pintadas = 0;
    let r = motor.dibujar(destino, |p| {
        for e in &lupas {
            if copiar_dentro(p, e, camara, (0.0, 0.0), imagenes, ancho, alto) {
                pintadas += 1;
            }
        }
    });
    if let Err(e) = r {
        tracing::warn!(?e, "no se pudo pintar dentro de las lupas");
    }
    pintadas
}

/// **La lupa que se arrastra en la capa de la tinta, con lo de dentro.** La
/// capa se pinta una vez y la composicion la corre: sin esto el cristal
/// viajaba vacio y lo de dentro se quedaba en la escena, donde estaba. Lo
/// de dentro no cambia mientras solo se mueve el cristal (el foco no va con
/// el), asi que vale lo ya preparado por el ultimo fotograma. `p` llega con
/// la vista de la capa: `camara` corrida `base`.
pub(super) fn pintar_en_capa(
    p: &pixpin_render::Pintor<'_>,
    e: &Elemento,
    camara: &Camara,
    base: (f32, f32),
    imagenes: &ImagenesLienzo,
    ancho: u32,
    alto: u32,
) {
    if lupa::de(e).is_none() {
        return;
    }
    copiar_dentro(p, e, camara, base, imagenes, ancho, alto);
    // Lo que venga despues en la capa espera la vista del mundo.
    let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
    p.poner_vista(
        (0.0, 0.0),
        camara.zoom,
        (origen.x + base.0, origen.y + base.1),
    );
}

/// **Lo que puede manchar en pantalla la lupa elegida**: su cristal y lo que
/// mira (la guia va de uno a otro, asi que cae entre los dos), con holgura
/// por la montura y la punta de la flecha. Pixeles de ventana con `camara`.
/// Es la zona que se rehace al apuntar la lupa a otro sitio, sobre la capa
/// congelada, en vez de la escena entera. `None` si lo elegido no es una
/// lupa sola.
pub(super) fn huella_elegida(
    escena: &Escena,
    seleccion: &[u64],
    camara: &Camara,
) -> Option<(f32, f32, f32, f32)> {
    let [id] = seleccion else { return None };
    let e = escena.buscar(*id).filter(|e| !e.borrado)?;
    let cr = lupa::de(e)?;
    let (c, _) = cajas(e, cr);
    let r = recogido(e, cr);
    let h = lupa::grueso_de_montura(e.grosor) * 2.0 + (e.grosor * 4.0).max(12.0);
    let a = camara.a_pantalla(Punto2::nuevo(c.0.min(r.0) - h, c.1.min(r.1) - h));
    let b = camara.a_pantalla(Punto2::nuevo(c.2.max(r.2) + h, c.3.max(r.3) + h));
    Some((a.x - 2.0, a.y - 2.0, b.x + 2.0, b.y + 2.0))
}

/// Cuantas veces se ha pintado lo de dentro de alguna lupa en este hilo.
#[cfg(test)]
pub(crate) fn rehechas() -> u64 {
    REHECHAS.with(Cell::get)
}

/// **Lo de dentro de las lupas en lo que se exporta** (PNG, JPG, la foto de
/// una zona, las imagenes de la hoja de compartir): el `contenidoDeLaLupa`
/// del movil. Se pinta con el mismo `pintar` de la hoja —la zona mirada
/// sale como una hoja propia (`exportar::de_una_zona`), con su papel y sus
/// fotos—, recortado al cristal y agrandado. `p` llega con la vista de la
/// hoja puesta: `k` pixeles por unidad y la esquina `hoja.caja` en el
/// origen. Sin esto, una lupa compartida salia como un circulo vacio.
pub(crate) fn en_la_hoja(
    p: &pixpin_render::Pintor<'_>,
    escena: &Escena,
    hoja: &pixpin_motor2d::exportar::Hoja,
    k: f32,
    papel: Option<(f32, f32)>,
    fondo: pixpin_render::Color,
    pintar: impl Fn(&pixpin_render::Pintor<'_>, &pixpin_motor2d::exportar::Hoja),
) -> usize {
    let lupas: Vec<&Elemento> = escena
        .visibles()
        .filter(|e| lupa::de(e).is_some_and(|cr| se_cortan(cajas(e, cr).0, hoja.caja)))
        .collect();
    if lupas.is_empty() {
        return 0;
    }
    // Lo mirado, sin lupas: una lupa mirando a otra no acabaria nunca.
    // Una escena de usar y tirar con solo los elementos: clonar la entera
    // se llevaria tambien su historial de deshacer.
    let mut sin_lupas = Escena::nueva();
    sin_lupas.fondo = escena.fondo;
    sin_lupas.escala = escena.escala.clone();
    sin_lupas.elementos = escena
        .elementos
        .iter()
        .filter(|e| lupa::de(e).is_none())
        .cloned()
        .collect();
    let (ox, oy) = (hoja.caja.0, hoja.caja.1);
    let a_imagen = |q: Punto2| ((q.x - ox) * k, (q.y - oy) * k);
    let mut pintadas = 0;
    for e in lupas {
        let Some(cr) = lupa::de(e) else { continue };
        let caja = lupa::caja_de(e);
        let (cristal, mira) = cajas(e, cr);
        let Some(dentro) = pixpin_motor2d::exportar::de_una_zona(&sin_lupas, mira, papel) else {
            continue;
        };
        let contorno: Vec<(f32, f32)> = lupa::puntos_del_cristal(cr, caja)
            .into_iter()
            .map(a_imagen)
            .collect();
        p.desplazar(0.0, 0.0);
        if !p.empujar_recorte_poligono(&contorno) {
            continue;
        }
        let (a, b) = (
            a_imagen(Punto2::nuevo(cristal.0, cristal.1)),
            a_imagen(Punto2::nuevo(cristal.2, cristal.3)),
        );
        p.rellenar(
            pixpin_render::RectF {
                x: a.0,
                y: a.1,
                ancho: b.0 - a.0,
                alto: b.1 - a.1,
            },
            fondo,
        );
        let s = k * lupa::aumento_de(cr, caja);
        let foco = lupa::foco_de(cr, caja);
        let c = a_imagen(Punto2::nuevo(
            (cristal.0 + cristal.2) / 2.0,
            (cristal.1 + cristal.3) / 2.0,
        ));
        p.poner_vista((0.0, 0.0), s, (c.0 - foco.x * s, c.1 - foco.y * s));
        pintar(p, &dentro);
        p.desplazar(0.0, 0.0);
        p.soltar_recorte_redondeado();
        // La montura encima, con la vista de la hoja otra vez.
        p.poner_vista((0.0, 0.0), k, (-ox * k, -oy * k));
        if let Some(montura) = pixpin_motor2d::pintado::ordenes(e).into_iter().next() {
            pintar(
                p,
                &pixpin_motor2d::exportar::Hoja {
                    nombre: String::new(),
                    caja: hoja.caja,
                    ordenes: vec![montura],
                    marcos: Vec::new(),
                    granos: Vec::new(),
                    grafitos: Vec::new(),
                },
            );
        }
        pintadas += 1;
    }
    pintadas
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::lupa_elemento::{Cristal, GuiaDeLupa};

    fn una_lupa() -> Elemento {
        // Cristal en (300, 0)-(400, 100), mirando un recuadro de 50 en
        // (0, 0)-(50, 50).
        Elemento {
            id: 3,
            figura: Figura::Lupa {
                cristal: Cristal {
                    foco: Some(Punto2::nuevo(25.0, 25.0)),
                    aumento: Some(2.0),
                    foco_ancho: Some(50.0),
                    foco_alto: Some(50.0),
                    guia: GuiaDeLupa::Flecha,
                    ..Default::default()
                },
            },
            x: 300.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            grosor: 2.0,
            ..Default::default()
        }
    }

    #[test]
    fn un_trazo_en_lo_mirado_o_en_el_cristal_pide_el_fotograma_entero() {
        let v = vec![una_lupa()];
        assert!(pisa_una_lupa(&v, (10.0, 10.0, 20.0, 20.0)), "en lo mirado");
        assert!(
            pisa_una_lupa(&v, (320.0, 20.0, 330.0, 30.0)),
            "en el cristal"
        );
        // Caso negativo: lejos de las dos cajas, el atajo sigue valiendo.
        assert!(!pisa_una_lupa(&v, (150.0, 150.0, 200.0, 200.0)));
        assert!(
            !pisa_una_lupa(&[], (10.0, 10.0, 20.0, 20.0)),
            "sin lupas nunca"
        );
    }

    #[test]
    fn dentro_de_una_lupa_no_se_pinta_otra_lupa_ni_lo_que_queda_fuera_de_lo_mirado() {
        let mira = (0.0, 0.0, 50.0, 50.0);
        assert!(
            !se_ve_dentro(&una_lupa(), mira),
            "una lupa mirando a otra no acaba"
        );
        let dentro = Elemento {
            figura: Figura::Rectangulo,
            x: 10.0,
            y: 10.0,
            ancho: 10.0,
            alto: 10.0,
            ..Default::default()
        };
        assert!(se_ve_dentro(&dentro, mira));
        let fuera = Elemento {
            x: 500.0,
            ..dentro.clone()
        };
        assert!(!se_ve_dentro(&fuera, mira));
    }

    /// **La muestra**: una lamina con un texto pequeno, un circulo y una
    /// cuadricula; se toca el circulo con la varita, se aparta la lupa y se
    /// pinta fuera de pantalla con la pasada de verdad. Dentro del cristal
    /// tiene que salir lo mirado agrandado (el texto, nitido) y fuera, el
    /// papel. Se deja `lupa-lienzo.png` para mirarla.
    fn lamina_con_lupa() -> (Escena, u64) {
        use crate::dibujo::lupa::convertir;
        use pixpin_motor2d::gesto::Gesto;
        let mut escena = Escena::nueva();
        let negro = pixpin_motor2d::ColorRgba::opaco(0.1, 0.1, 0.1);
        for i in 0..6 {
            let x = 40.0 + i as f32 * 12.0;
            escena.anadir(Elemento {
                figura: Figura::Linea {
                    puntos: vec![Punto2::nuevo(x, 40.0), Punto2::nuevo(x, 120.0)],
                },
                x,
                y: 40.0,
                ancho: 0.0,
                alto: 80.0,
                trazo: pixpin_motor2d::ColorRgba::opaco(0.2, 0.4, 0.9),
                grosor: 1.0,
                ..Default::default()
            });
        }
        escena.anadir(Elemento {
            figura: Figura::Texto {
                texto: "cota 3,25".into(),
                tam: 10.0,
                familia: "Segoe UI".into(),
            },
            x: 48.0,
            y: 72.0,
            ancho: 50.0,
            alto: 14.0,
            trazo: negro,
            ..Default::default()
        });
        let circulo = escena.anadir(Elemento {
            figura: Figura::Elipse,
            x: 30.0,
            y: 40.0,
            ancho: 90.0,
            alto: 80.0,
            trazo: pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19),
            grosor: 2.0,
            ..Default::default()
        });
        let mut gesto = Gesto::nuevo();
        let id = convertir(&mut escena, &mut gesto, circulo).expect("la varita convierte");
        // Apartada a la derecha: la guia (flecha) sale del circulo mirado.
        escena.mover(id, 260.0, 0.0);
        assert_eq!(
            escena.cuantos_visibles(),
            8,
            "el circulo se fue con la lupa"
        );
        (escena, id)
    }

    fn guardar_muestra(nombre: &str, ancho: u32, alto: u32, px: Vec<u8>) {
        let dir = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&dir).expect("carpeta");
        let png = pixpin_codec::codificar_png(&pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles: px,
        })
        .expect("png");
        std::fs::write(dir.join(nombre), png).expect("escribe");
    }

    #[test]
    fn la_lupa_ensena_en_grande_lo_que_mira_y_la_muestra_se_deja_en_png() {
        let (escena, id) = lamina_con_lupa();

        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let mut motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let (w, h) = (560u32, 260u32);
        let destino =
            pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h)
                .expect("textura");
        let imagenes = ImagenesLienzo::nuevo(4096);
        let camara = Camara::nueva();
        let vista = camara.ventana(w as f32, h as f32);
        motor
            .dibujar(&destino.destino, |p| {
                p.limpiar(crate::dibujo::pintar::a_color(escena.fondo));
                p.poner_vista((0.0, 0.0), 1.0, (0.0, 0.0));
                let mut cache = Cache::nueva();
                for e in escena.visibles() {
                    por_cada_orden(&mut cache, e, 1.0, None, |o| {
                        dibujar_orden(p, o, vista, None, &imagenes, 1.0, None);
                    });
                }
            })
            .expect("pinta la escena");
        let pintadas = pasar(
            &mut motor,
            &destino.destino,
            &escena,
            &camara,
            None,
            &imagenes,
            w,
            h,
            &|_| false,
        );
        assert_eq!(pintadas, 1);
        let (ancho, alto, px) = destino.leer_rgba().expect("lee");
        // En el centro del cristal (lo mirado es el centro del circulo, con
        // las rayas azules y el texto) hay tinta; antes de la pasada ahi
        // solo habia papel blanco.
        let e = escena.buscar(id).unwrap();
        let (cx, cy) = ((e.x + e.ancho / 2.0) as u32, (e.y + e.alto / 2.0) as u32);
        let mut tinta = 0;
        for y in cy - 30..cy + 30 {
            for x in cx - 60..cx + 60 {
                let k = ((y * ancho + x) * 4) as usize;
                if px[k] < 200 || px[k + 1] < 200 {
                    tinta += 1;
                }
            }
        }
        assert!(tinta > 300, "el cristal salio vacio: {tinta}");
        guardar_muestra("lupa-lienzo.png", ancho, alto, px);
        olvidar();
    }

    /// **Exportada tambien se ve por dentro** (el `contenidoDeLaLupa` del
    /// movil): el PNG del lienzo, la foto de una zona y las imagenes de la
    /// hoja de compartir pasan por `a_imagen`. Se deja `lupa-exportada.png`.
    #[test]
    fn exportada_a_png_la_lupa_sale_con_lo_que_mira_y_no_vacia() {
        let (escena, id) = lamina_con_lupa();
        let fotos = |_: u64| None;
        let lienzo = super::super::exportar::Lienzo {
            escena: &escena,
            seleccion: &[],
            papel: None,
            fotos: &fotos,
            nombre: "lupa".into(),
        };
        let hoja = pixpin_motor2d::exportar::hojas(
            &escena,
            pixpin_motor2d::exportar::Alcance::Todo,
            &[],
            None,
        )
        .remove(0);
        let img = super::super::exportar::a_imagen(&hoja, 2.0, Some(escena.fondo), &lienzo)
            .expect("sale el png");
        // El centro del cristal, en pixeles de la imagen (a 2 por unidad).
        let e = escena.buscar(id).unwrap();
        let k = img.ancho as f32 / (hoja.caja.2 - hoja.caja.0);
        let cx = ((e.x + e.ancho / 2.0 - hoja.caja.0) * k) as u32;
        let cy = ((e.y + e.alto / 2.0 - hoja.caja.1) * k) as u32;
        let mut tinta = 0;
        for y in cy - 40..cy + 40 {
            for x in cx - 80..cx + 80 {
                let i = ((y * img.ancho + x) * 4) as usize;
                if img.pixeles[i] < 200 || img.pixeles[i + 1] < 200 {
                    tinta += 1;
                }
            }
        }
        assert!(tinta > 500, "la lupa exportada salio vacia: {tinta}");
        guardar_muestra("lupa-exportada.png", img.ancho, img.alto, img.pixeles);
    }

    /// **El panel de una lupa y el de un mosaico** (`Propiedad.LUPA` y
    /// `Propiedad.MOSAICO` del movil): salen sus filas, pulsarlas cambia lo
    /// elegido y se dejan `panel-lupa.png` y `panel-mosaico.png`.
    #[test]
    fn el_panel_de_una_lupa_y_de_un_mosaico_ofrece_sus_mandos_y_los_aplica() {
        use pixpin_motor2d::estilo::CambioForma;
        use pixpin_motor2d::gesto::Gesto;
        use pixpin_ui::panel_lateral::{AccionPanel, Control, Seccion};
        let area = pixpin_geom::Rect {
            x: 0,
            y: 0,
            ancho: 260,
            alto: 620,
        };
        let secciones = |p: &pixpin_ui::panel_lateral::PanelLateral| -> Vec<Seccion> {
            p.controles
                .iter()
                .filter_map(|c| match c {
                    Control::Titulo { seccion, .. } => Some(*seccion),
                    _ => None,
                })
                .collect()
        };
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let fuera = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(
            &motor,
            d.d3d(),
            area.ancho,
            area.alto,
        )
        .expect("textura");
        let foto = |panel: &pixpin_ui::panel_lateral::PanelLateral, nombre: &str| {
            motor
                .dibujar(&fuera.destino, |p| {
                    p.limpiar(pixpin_render::Color::BLANCO);
                    crate::panel_dibujo::pintar(p, panel, 100);
                })
                .expect("pinta");
            let (w, h, px) = fuera.leer_rgba().expect("lee");
            guardar_muestra(nombre, w, h, px);
        };

        // La lupa: Aumento y Guia, con x2 y la flecha marcados.
        let (mut escena, id) = lamina_con_lupa();
        let mut gesto = Gesto::nuevo();
        gesto.seleccion.poner(id);
        let panel = crate::panel_dibujo::panel_para(&gesto, &escena, area, 100).expect("panel");
        let s = secciones(&panel);
        assert!(
            s.contains(&Seccion::AumentoLupa) && s.contains(&Seccion::GuiaLupa),
            "{s:?}"
        );
        assert!(
            !s.contains(&Seccion::Mosaico),
            "caso negativo: una lupa no se pixela"
        );
        foto(&panel, "panel-lupa.png");
        let antes = escena.buscar(id).unwrap().clone();
        assert!(crate::panel_dibujo::aplicar(
            AccionPanel::Forma(CambioForma::AumentoLupa(4.0)),
            &mut gesto,
            &mut escena,
        ));
        let e = escena.buscar(id).unwrap();
        let cr = lupa::de(e).unwrap();
        assert!((lupa::aumento_de(cr, lupa::caja_de(e)) - 4.0).abs() < 0.05);
        assert_eq!(
            lupa::foco_de(cr, lupa::caja_de(e)),
            lupa::foco_de(lupa::de(&antes).unwrap(), lupa::caja_de(&antes)),
            "lo mirado no se mueve"
        );
        assert!(e.ancho > antes.ancho * 1.9, "crece el cristal");

        // El mosaico: pixelar o desenfocar, y el grano con el grosor.
        let mut escena = Escena::nueva();
        let m = escena.anadir(Elemento {
            figura: Figura::Mosaico { desenfoque: false },
            ancho: 100.0,
            alto: 40.0,
            grosor: 2.0,
            ..Default::default()
        });
        let mut gesto = Gesto::nuevo();
        gesto.seleccion.poner(m);
        let panel = crate::panel_dibujo::panel_para(&gesto, &escena, area, 100).expect("panel");
        let s = secciones(&panel);
        assert!(
            s.contains(&Seccion::Mosaico) && s.contains(&Seccion::Grosor),
            "{s:?}"
        );
        assert!(!s.contains(&Seccion::AumentoLupa));
        foto(&panel, "panel-mosaico.png");
        assert!(crate::panel_dibujo::aplicar(
            AccionPanel::Forma(CambioForma::Desenfoque(true)),
            &mut gesto,
            &mut escena,
        ));
        assert_eq!(
            escena.buscar(m).unwrap().figura,
            Figura::Mosaico { desenfoque: true }
        );
        // Y queda para el proximo: el gesto lo recuerda.
        assert!(gesto.estilo.desenfoque);
    }

    /// Una foto de papel con cuadros de 3 unidades y una raya diagonal:
    /// lo que mas delata un cambio de muestreo o de resolucion.
    fn papel_a_cuadros() -> FondoLienzo {
        let (w, h) = (600u32, 300u32);
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let c = if ((x / 3) + (y / 3)) % 2 == 0 {
                    235u8
                } else {
                    190u8
                };
                let d = if (x as i32 - y as i32 * 2).abs() < 2 {
                    60u8
                } else {
                    c
                };
                px.extend_from_slice(&[d, c, 255, 255]);
            }
        }
        FondoLienzo::nuevo(
            pixpin_codec::ImagenRgba {
                ancho: w,
                alto: h,
                pixeles: px,
            },
            4096,
        )
    }

    /// La lamina de siempre con un trazo a mano bajo lo mirado: su contorno
    /// es el que cambia de detalle con el aumento (`ordenes_a_distancia`).
    fn lamina_con_papel_y_tinta() -> (Escena, u64) {
        let (mut escena, id) = lamina_con_lupa();
        let puntos: Vec<Punto2> = (0..60)
            .map(|i| {
                let t = i as f32 / 59.0;
                Punto2::nuevo(35.0 + 80.0 * t, 60.0 + 25.0 * (t * 12.0).sin())
            })
            .collect();
        escena.anadir(Elemento {
            figura: Figura::Lapiz {
                presiones: vec![0.6; puntos.len()],
                puntos,
                opciones: Some(pixpin_motor2d::tinta::OpcionesTinta::default()),
            },
            x: 35.0,
            y: 35.0,
            ancho: 80.0,
            alto: 50.0,
            trazo: pixpin_motor2d::ColorRgba::opaco(0.1, 0.6, 0.2),
            grosor: 1.0,
            ..Default::default()
        });
        (escena, id)
    }

    /// **El fotograma del editor fuera de pantalla** a `zoom`: el papel, la
    /// escena y la pasada de las lupas, con la camara en el origen.
    fn retratar(
        motor: &mut MotorRender,
        d3d: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
        escena: &Escena,
        fondo: &mut FondoLienzo,
        zoom: f32,
        ancho: u32,
        alto: u32,
    ) -> Vec<u8> {
        let destino =
            pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(motor, d3d, ancho, alto)
                .expect("textura");
        let imagenes = ImagenesLienzo::nuevo(4096);
        fondo.asegurar(motor);
        let camara = Camara {
            x: 0.0,
            y: 0.0,
            zoom,
        };
        let vista = camara.ventana(ancho as f32, alto as f32);
        motor
            .dibujar(&destino.destino, |p| {
                p.limpiar(crate::dibujo::pintar::a_color(escena.fondo));
                p.poner_vista((0.0, 0.0), zoom, (0.0, 0.0));
                fondo.pintar(p, vista, zoom);
                let mut cache = Cache::nueva();
                for e in escena.visibles() {
                    por_cada_orden(&mut cache, e, zoom, None, |o| {
                        dibujar_orden(p, o, vista, None, &imagenes, zoom, None);
                    });
                }
            })
            .expect("pinta la escena");
        assert_eq!(
            pasar(
                motor,
                &destino.destino,
                escena,
                &camara,
                Some(fondo),
                &imagenes,
                ancho,
                alto,
                &|_| false
            ),
            1
        );
        destino.leer_rgba().expect("lee").2
    }

    /// **Una lupa puesta ensena lo mismo a cualquier zoom** (queja del
    /// usuario: «cuando hago zoom varia o cambia; una vez puesta no deberia
    /// cambiar nada»). Lo de dentro es la escena y el aumento de la lupa,
    /// en unidades del mundo, como en `Renderer.drawLupa`: se pinta el
    /// lienzo a zoom 1 y a zoom 2, se reduce el de zoom 2 a la mitad y el
    /// cristal tiene que salir igual pixel a pixel (con el margen del
    /// suavizado de bordes). Deja `lupa-zoom-1.png` y `lupa-zoom-2.png`.
    #[test]
    fn la_lupa_ensena_lo_mismo_a_zoom_uno_que_a_zoom_dos() {
        let (escena, id) = lamina_con_papel_y_tinta();
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let mut motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let mut fondo = papel_a_cuadros();
        let (w, h) = (560u32, 260u32);
        let uno = retratar(&mut motor, d.d3d(), &escena, &mut fondo, 1.0, w, h);
        let dos = retratar(&mut motor, d.d3d(), &escena, &mut fondo, 2.0, w * 2, h * 2);
        olvidar();
        guardar_muestra("lupa-zoom-1.png", w, h, uno.clone());
        guardar_muestra("lupa-zoom-2.png", w * 2, h * 2, dos.clone());
        // El cristal, sin la montura: un recuadro dentro del ovalo.
        let e = escena.buscar(id).unwrap();
        let (cx, cy) = (e.x + e.ancho / 2.0, e.y + e.alto / 2.0);
        let (rx, ry) = (e.ancho * 0.3, e.alto * 0.3);
        let (mut distintos, mut total, mut suma) = (0u32, 0u32, 0u64);
        for y in (cy - ry) as u32..(cy + ry) as u32 {
            for x in (cx - rx) as u32..(cx + rx) as u32 {
                let a = ((y * w + x) * 4) as usize;
                for c in 0..3 {
                    // El de zoom 2, reducido: la media de sus cuatro pixeles.
                    let m: u32 = [(0, 0), (1, 0), (0, 1), (1, 1)]
                        .iter()
                        .map(|(dx, dy)| {
                            dos[(((2 * y + dy) * w * 2 + 2 * x + dx) * 4) as usize + c] as u32
                        })
                        .sum::<u32>()
                        / 4;
                    let diferencia = (uno[a + c] as i32 - m as i32).unsigned_abs();
                    suma += diferencia as u64;
                    if diferencia > 60 {
                        distintos += 1;
                    }
                    total += 1;
                }
            }
        }
        let media = suma as f64 / total as f64;
        println!("lupa a zoom 1 y 2: media {media:.2}, muy distintos {distintos} de {total}");
        // Medido el 27-sep: la pasada de antes (detalle, muestreo y teselas
        // segun el zoom de la camara) daba 4,18 de media; esta, 1,87. Lo
        // que queda es el suavizado de los bordes, que a otra resolucion
        // cae en otros pixeles (un 1 % de canales, las rayas de un pixel).
        assert!(
            media < 3.0,
            "lo de dentro cambia con el zoom: diferencia media {media:.2}"
        );
        assert!(
            (distintos as f64) < total as f64 * 0.02,
            "lo de dentro cambia con el zoom: {distintos} de {total} canales muy distintos"
        );
    }

    /// **Mover el cristal no vuelve a pintar lo de dentro** (queja: «la lupa
    /// se mueve lento»): se copia lo ya pintado. Si se rehace al cambiar lo
    /// mirado, al tocar algo que se ve dentro o al cambiar el zoom; y no al
    /// tocar algo que queda lejos.
    #[test]
    fn mover_la_lupa_copia_lo_de_dentro_y_solo_se_rehace_si_cambia_lo_que_se_ve() {
        let (mut escena, id) = lamina_con_lupa();
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let mut motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let (w, h) = (900u32, 400u32);
        let destino =
            pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h)
                .expect("textura");
        let imagenes = ImagenesLienzo::nuevo(4096);
        let camara = Camara::nueva();
        let mut pasada = |escena: &Escena, camara: &Camara| {
            let antes = rehechas();
            assert_eq!(
                pasar(
                    &mut motor,
                    &destino.destino,
                    escena,
                    camara,
                    None,
                    &imagenes,
                    w,
                    h,
                    &|_| false
                ),
                1
            );
            rehechas() - antes
        };
        assert_eq!(pasada(&escena, &camara), 1, "la primera vez se pinta");
        assert_eq!(pasada(&escena, &camara), 0, "sin cambios se copia");
        escena.mover(id, 37.0, 11.0);
        assert_eq!(
            pasada(&escena, &camara),
            0,
            "mover el cristal no cambia lo de dentro"
        );
        let lejos = Camara {
            x: -20.0,
            y: -10.0,
            zoom: 1.0,
        };
        assert_eq!(pasada(&escena, &lejos), 0, "desplazar la camara tampoco");
        // Caso negativo del lado contrario: algo que no se ve dentro.
        let fuera = escena.anadir(Elemento {
            figura: Figura::Rectangulo,
            x: 800.0,
            y: 300.0,
            ancho: 20.0,
            alto: 20.0,
            ..Default::default()
        });
        escena.mover(fuera, 5.0, 0.0);
        assert_eq!(
            pasada(&escena, &camara),
            0,
            "lo de fuera de lo mirado no cuenta"
        );
        // Lo que si cambia lo de dentro.
        let texto = escena
            .visibles()
            .find(|e| matches!(e.figura, Figura::Texto { .. }))
            .map(|e| e.id)
            .unwrap();
        escena.mover(texto, 3.0, 0.0);
        assert_eq!(pasada(&escena, &camara), 1, "se movio algo de lo mirado");
        if let Some(e) = escena.buscar_mut(id)
            && let Figura::Lupa { cristal } = &mut e.figura
        {
            cristal.foco = cristal.foco.map(|f| Punto2::nuevo(f.x + 10.0, f.y));
            e.tocar();
        }
        assert_eq!(pasada(&escena, &camara), 1, "se apunto a otro sitio");
        let cerca = Camara {
            zoom: 2.0,
            ..camara
        };
        assert_eq!(pasada(&escena, &cerca), 1, "otro zoom, otros pixeles");
        olvidar();
    }

    #[test]
    fn lo_mirado_no_depende_de_donde_este_el_cristal_y_los_pixeles_no_pasan_del_tope() {
        let cr = Cristal {
            foco: Some(Punto2::nuevo(50.0, 50.0)),
            foco_ancho: Some(40.0),
            foco_alto: Some(20.0),
            ..Default::default()
        };
        let a = mirado(&cr, (300.0, 0.0, 80.0, 40.0));
        let b = mirado(&cr, (-900.0, 77.0, 80.0, 40.0));
        assert_eq!(a, b);
        assert_eq!(
            a,
            (30.0, 40.0, 70.0, 60.0),
            "a x2, el cristal partido por dos"
        );
        assert_eq!(pixeles_para((0.0, 0.0, 80.0, 40.0), 1.5, 4096), (120, 60));
        // Caso negativo: a mucho zoom no pasa del tope, y guarda la forma.
        assert_eq!(
            pixeles_para((0.0, 0.0, 800.0, 400.0), 100.0, 4096),
            (4096, 2048)
        );
    }

    #[test]
    fn solo_cuentan_las_lupas_cuyo_cristal_se_ve() {
        let v = vec![una_lupa()];
        let c = Camara::nueva();
        assert_eq!(a_la_vista(&v, &c, 800, 600).count(), 1);
        // Con la camara lejos, ninguna: la pasada no hace nada.
        let lejos = Camara {
            x: 5000.0,
            y: 5000.0,
            zoom: 1.0,
        };
        assert_eq!(a_la_vista(&v, &lejos, 800, 600).count(), 0);
    }
}
