//! **De las ordenes del motor a Direct2D**, comun a todos los anfitriones.
//!
//! El motor entrega `pixpin_motor2d::pintado::Orden` (geometria ya hecha) y
//! `pixpin-render` no conoce ese tipo a proposito (D28): la traduccion vive
//! en la aplicacion. Estaba en `ventana_editor.rs`; se movio aqui tal cual
//! el 2026-09-24 para que el lector y el anotador de pantalla pinten lo
//! dibujado con EXACTAMENTE las mismas llamadas que el lienzo (grano,
//! grafito, realizaciones cacheadas, punta predicha). Dos traducciones
//! distintas eran dos maneras de que un trazo se viera distinto segun donde
//! se abriera.

use crate::imagenes_lienzo::ImagenesLienzo;
use pixpin_motor2d::cache::Cache;
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{ColorRgba, Elemento, Escala, EstiloTrazo, Figura};
use pixpin_render::{Color, RectF};

/// Todas las ordenes de un elemento para un fotograma: las cacheadas (forma,
/// colores...) mas las que dependen de la escala (el numero de una cota, el
/// cuadro de una barra), que `cache.medibles` guarda aparte con la escala y
/// el papel en la clave -asi calibrar o cambiar el papel surte efecto sin
/// tocar la version de nada-.
///
/// La usan `pintar` y el cierre de `capa.preparar` (mas abajo, en `abrir`):
/// las dos veces que se decide que ordenes representan a un elemento en un
/// fotograma. Si un camino llamara solo a `cache.ordenes` y se olvidara de
/// esta funcion, ese elemento perderia sus rotulos de medida alli donde falte
/// -es justo lo que paso con la capa estatica: hornea solo lo que pasa por
/// aqui, asi que si el rotulo no entra, desaparece mientras dura el arrastre-.
///
/// La coma va como separador decimal (D39), no el del sistema: si algun dia
/// hay que respetar el idioma del usuario, sale de los ajustes y se pasa
/// aqui, no se lee dentro del motor.
///
/// Entrega las ordenes de una en una a `dibuja` en vez de devolver un `Vec`,
/// y eso NO es un detalle de estilo: la version que devolvia `Vec` hacia
/// `cache.ordenes(e, zoom).to_vec()`, o sea copiaba la geometria cacheada de
/// cada elemento visible en CADA fotograma -medido: 6,6 MB por fotograma con
/// 2.000 garabatos- dentro del unico camino donde el presupuesto de
/// rendimiento se compromete a no pedir memoria. Prestando la rebanada no hay
/// nada que copiar. El prestamo de `cache` muere al cerrar el primer bucle,
/// asi que el segundo puede volver a mirar el elemento sin pelearse con el.
///
/// El sumidero es un cierre y no el `Pintor` para que la funcion siga siendo
/// probable sin GPU: la prueba de aqui abajo le pasa un cierre que colecciona,
/// y asi el invariante que costo la ronda 1 -que ningun camino se olvide del
/// rotulo- conserva su prueba.
pub(crate) fn por_cada_orden(
    cache: &mut Cache,
    e: &Elemento,
    zoom: f32,
    escala: Option<&Escala>,
    mut dibuja: impl FnMut(&Orden),
) {
    // **El grafito va entero en su mapa** (`tinta::grafito`): en vez de sus
    // ordenes lisas sale un aviso, y quien pinta, al verlo, pinta el mapa que
    // deja la marca. Aqui y no en `pintar` porque este es el unico sitio por
    // el que pasan los cuatro caminos (la escena, la capa congelada, la tinta
    // viva y la seleccion en su capa). Si no se pudo cocer, va liso.
    // Sobre papel de noche se cuece con la tinta adaptada: su color va
    // dentro del mapa y `a_tinta` no llega a verlo (`tema::grafito_adaptado`).
    let adaptado = pixpin_motor2d::tinta::grafito::es_de_grafito(e)
        .then(|| super::tema::grafito_adaptado(e))
        .flatten();
    match pixpin_motor2d::tinta::grafito::cocer(adaptado.as_ref().unwrap_or(e)) {
        Some(cocido) => pixpin_motor2d::tinta::grafito::con_marca(cocido, || {
            dibuja(&pixpin_motor2d::tinta::grafito::orden_de_aviso(e));
        }),
        None => {
            for orden in cache.ordenes(e, zoom) {
                dibuja(orden);
            }
        }
    }
    // La cota entera (raya y numero) sale de aqui con la tinta adaptada al
    // papel de este fotograma, como el movil (`pintado::cota_entera`). De la
    // cache, con la escala y el papel en la clave (`Cache::medibles`): antes
    // se recalculaba cada cota visible en cada pintado, y lo que se traza
    // sube de version en cada aviso, asi que solo se recalcula lo vivo.
    let papel = super::tema::papel_del_fotograma();
    for orden in cache.medibles(e, escala, ',', papel) {
        dibuja(orden);
    }
}

/// Traduce una `Orden` ya calculada por el motor a la llamada de `Pintor`
/// que le toca. Pura traduccion: la geometria ya viene hecha, aqui solo se
/// decide con que primitiva de Direct2D se pinta.
///
/// `vista` es la caja del mundo que se ve (en las mismas coordenadas que
/// `Orden`), y hace falta para `Orden::Velo`: el motor no sabe cuanto mide
/// el lienzo (lo dice `pintado.rs`), asi que quien pinta pone el marco.
///
/// `imagenes` y `zoom` solo los usa `Orden::Imagen`: el almacen resuelve el
/// `id_objeto` y el zoom efectivo elige el muestreo (D141).
/// Estampa la tela de un material dentro de una silueta ya calculada.
///
/// Es la misma llamada que hace `apps/pixpin/tests/muestra_de_tintas.rs`,
/// puesta en una funcion para que no haya dos formas de pedir lo mismo: si
/// el editor y la muestra se pidieran por su cuenta, la muestra dejaria de
/// ser prueba de lo que se ve en pantalla.
pub(crate) fn estampar_grano(
    p: &pixpin_render::Pintor<'_>,
    cache: &mut pixpin_render::CacheGrano,
    clave: (u64, u32),
    contorno: &[(f32, f32)],
    g: &pixpin_motor2d::pintado::Grano,
) {
    let tela = pixpin_motor2d::tinta::tejido(g.material);
    p.grano(
        cache,
        clave,
        contorno,
        &tela,
        pixpin_motor2d::tinta::material::LADO_DEL_MOSAICO,
        g.material as u32,
        a_tinta(g.color),
        g.paso,
        g.inclinada,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn dibujar_orden(
    p: &pixpin_render::Pintor<'_>,
    orden: &Orden,
    vista: (f32, f32, f32, f32),
    tinta: Option<(&mut pixpin_render::CacheTinta, (u64, u32, u32))>,
    imagenes: &ImagenesLienzo,
    zoom: f32,
    grano: Option<pixpin_motor2d::pintado::Grano>,
) {
    match orden {
        // Acierto de cache: se pinta con la realizacion ya teselada sin
        // volver a convertir los puntos en `Vec<(f32, f32)>` -esa reserva es
        // la que se pagaba cada fotograma sin usarla para nada en cuanto
        // habia acierto-. Solo si `pintar_realizada` no encuentra nada (o el
        // contexto no da D2D 1.1) se paga la conversion y se rehace.
        //
        // Antes esto solo lo hacia la tinta (D121) y las formas de rough.js
        // creaban una geometria por pasada y por fotograma: es lo que hacia
        // que un paneo con formas rellenas costara decenas de veces mas que
        // uno con trazos a mano.
        Orden::Poligono { puntos, color } | Orden::Relleno { puntos, color } => match tinta {
            Some((c, clave)) => {
                if !p.pintar_realizada(c, clave, a_tinta(*color)) {
                    p.poligono_cacheado(c, clave, &a_tuplas(puntos), a_tinta(*color));
                }
            }
            None => p.poligono(&a_tuplas(puntos), a_tinta(*color)),
        },
        // **El cuerpo del trazo y, detras, su grano.** Hasta ahora el grano
        // solo se veia en la prueba que saca los PNG de muestra: el editor
        // pintaba la silueta y se saltaba la tela, asi que un trazo de tiza
        // del movil se veia aqui macizo y los diez materiales se veian
        // iguales. El motor ya decia con que tenirlo (`pintado::grano_de`);
        // lo que faltaba era pedirlo.
        //
        // **El aviso del grafito**: una tinta sin contorno. Lo pone
        // `por_cada_orden` con el mapa ya cocido en la marca; aqui se pinta
        // ese mapa con la opacidad del elemento, guardando el bitmap si hay
        // cache (lo quieto) y subiendolo sin guardar si no (el trazo en curso
        // y lo que se arrastra). Sin marca —la punta predicha de un grafito—
        // no hay nada que pintar.
        Orden::Tinta { contorno, color } if contorno.is_empty() => {
            if let Some(g) = pixpin_motor2d::tinta::grafito::marca() {
                let mapa = pixpin_render::MapaGrafito {
                    rgba: &g.rgba,
                    ancho: g.ancho,
                    alto: g.alto,
                    caja: g.caja(),
                    huella: g.huella,
                    angulo: g.angulo,
                    centro: (g.centro.x, g.centro.y),
                    id: g.id,
                    generacion: g.generacion,
                    sucio: g.sucio.map(|(desde, r)| {
                        (
                            desde,
                            (r.x as u32, r.y as u32, r.ancho as u32, r.alto as u32),
                        )
                    }),
                };
                let cache = tinta.map(|(c, _)| &mut c.grano.grafito);
                p.grafito(cache, &mapa, color.a, zoom);
            }
        }
        Orden::Tinta { contorno, color } => match tinta {
            Some((c, clave)) => {
                let puntos = if p.pintar_realizada(c, clave, a_tinta(*color)) {
                    // Acierto de cache: la silueta ya esta teselada y no
                    // hace falta convertir los puntos... salvo que haya
                    // grano, que necesita la geometria de verdad porque se
                    // pinta con un pincel de mosaico y no de color.
                    grano.is_some().then(|| a_tuplas(contorno))
                } else {
                    let v = a_tuplas(contorno);
                    p.tinta_cacheada(c, clave, &v, a_tinta(*color));
                    Some(v)
                };
                if let (Some(g), Some(v)) = (grano, puntos) {
                    estampar_grano(p, &mut c.grano, (clave.0, clave.1), &v, &g);
                }
            }
            // Sin cache es lo que cambia en CADA fotograma: el trazo en
            // curso y lo que se arrastra. Ahi el grano se salta a
            // proposito, porque su brocha se ancla al documento y volver a
            // tejerla sesenta veces por segundo cuesta mas de lo que se ve;
            // en cuanto se suelta, el trazo entra por el camino de arriba y
            // aparece con su tela.
            None => p.tinta(&a_tuplas(contorno), a_tinta(*color)),
        },
        Orden::Polilinea {
            puntos,
            color,
            grosor,
            estilo,
        } => {
            // `Pintor` todavia no distingue rayas de puntos (nadie mas en el
            // proyecto dibuja punteado con Direct2D); a rayas es la
            // aproximacion mas cercana a lo discontinuo.
            let discontinua = !matches!(estilo, EstiloTrazo::Solido);
            match tinta {
                Some((c, clave)) => {
                    if !p.pintar_realizada(c, clave, a_tinta(*color)) {
                        p.polilinea_cacheada(
                            c,
                            clave,
                            &a_tuplas(puntos),
                            *grosor,
                            discontinua,
                            a_tinta(*color),
                        );
                    }
                }
                None => {
                    let v = a_tuplas(puntos);
                    if discontinua {
                        p.polilinea_discontinua(&v, *grosor, a_tinta(*color))
                    } else {
                        p.polilinea(&v, *grosor, a_tinta(*color))
                    }
                }
            }
        }
        Orden::Velo { hueco, color } => {
            let (x0, y0, x1, y1) = vista;
            p.velo(
                RectF {
                    x: x0,
                    y: y0,
                    ancho: x1 - x0,
                    alto: y1 - y0,
                },
                &a_tuplas(hueco),
                a_tinta(*color),
            );
        }
        Orden::Texto {
            texto,
            x,
            y,
            tam,
            familia,
            color,
            ancho_max,
            negrita,
            cursiva,
        } => {
            // Con su letra: la familia, la cara y el interlineado de
            // Excalidraw. Antes todo salia en Segoe UI dijera lo que dijera el
            // texto, y la caja (medida con otra cuenta) no casaba con lo
            // pintado.
            let letra = letra_de(familia, *negrita, *cursiva);
            p.texto_con_letra(texto, *x, *y, *tam, *ancho_max, &letra, a_tinta(*color));
        }
        // El numero de una cota, girado con su raya y con halo. La tinta ya
        // viene adaptada al papel desde el motor, y el halo va tal cual: es
        // el contrario de esa tinta (`contrastingTextColor`), y pasarlo por
        // `a_tinta` lo volveria a adaptar y sobre papel de noche el halo negro
        // saldria gris claro, pegado a la letra.
        Orden::Rotulo {
            texto,
            x,
            y,
            tam,
            familia,
            color,
            halo,
            grosor_halo,
            centro,
            angulo,
        } => {
            let letra = letra_de(familia, false, false);
            p.girado((centro.x, centro.y), *angulo, |p| {
                p.texto_con_halo(
                    texto,
                    *x,
                    *y,
                    *tam,
                    &letra,
                    a_tinta(*color),
                    a_color(*halo),
                    *grosor_halo,
                );
            });
        }
        Orden::Imagen {
            id_objeto,
            x,
            y,
            ancho,
            alto,
            opacidad,
            recorte,
            angulo,
        } => {
            // El motor no sabe de bitmaps: solo dice «aqui va la imagen
            // numero N». Quien la tiene es el almacen del lienzo. Con su
            // recorte (`crop`): `pintar_recortada` ya lo sabia pintar, pero
            // nadie se lo pasaba y la foto salia entera.
            let recorte = recorte.map(|r| crate::imagenes_lienzo::Recorte {
                x: r.x,
                y: r.y,
                ancho: r.ancho,
                alto: r.alto,
                ancho_natural: r.ancho_natural,
                alto_natural: r.alto_natural,
            });
            // Y con su giro, alrededor del centro de su caja: una foto girada
            // en el movil se veia aqui derecha.
            let centro = (*x + *ancho / 2.0, *y + *alto / 2.0);
            p.girado(centro, *angulo, |p| {
                imagenes.pintar_recortada(
                    p,
                    *id_objeto,
                    RectF {
                        x: *x,
                        y: *y,
                        ancho: *ancho,
                        alto: *alto,
                    },
                    zoom,
                    *opacidad,
                    recorte.as_ref(),
                );
            });
        }
    }
}

/// **La figura en curso con la punta en el punto predicho**: la copia que
/// hace `prediccion::con_punta`, pintada en lugar de la real.
///
/// De grafito, de grafito: antes esta copia iba siempre por
/// `ordenes_a_distancia`, que la da lisa, y el rectangulo o la flecha de
/// grafito se veian lisos todo el arrastre y solo al soltar salian de
/// grafito. Se cuece como la real (`tinta::grafito::cocer`, con el mismo id:
/// la copia ocupa el sitio de la real en el horno, que mientras se arrastra
/// no se pinta) y se pinta su mapa por el aviso, igual que `por_cada_orden`.
/// Cuesta lo mismo que la real, que es lo que se deja de cocer.
pub(crate) fn pintar_copia_predicha(
    p: &pixpin_render::Pintor<'_>,
    copia: &Elemento,
    vista: (f32, f32, f32, f32),
    imagenes: &ImagenesLienzo,
    zoom: f32,
    escala: Option<&Escala>,
) {
    use pixpin_motor2d::tinta::grafito;
    let adaptado = grafito::es_de_grafito(copia)
        .then(|| super::tema::grafito_adaptado(copia))
        .flatten();
    if let Some(cocido) = grafito::cocer(adaptado.as_ref().unwrap_or(copia)) {
        grafito::con_marca(cocido, || {
            dibujar_orden(
                p,
                &grafito::orden_de_aviso(copia),
                vista,
                None,
                imagenes,
                zoom,
                None,
            );
        });
        return;
    }
    for orden in pixpin_motor2d::pintado::ordenes_a_distancia(copia, zoom) {
        dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
    }
    // Una cota que se arrastra no tiene nada en las ordenes cacheables: su
    // raya y su numero salen de las medibles, con la escala de la escena.
    let papel = super::tema::papel_del_fotograma();
    for orden in pixpin_motor2d::pintado::ordenes_medibles(copia, escala, ',', papel) {
        dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
    }
}

/// La punta predicha de un trazo a mano, como una mancha de tinta APARTE.
///
/// Antes se clonaba el elemento entero y se recalculaba su contorno completo
/// (`prediccion::con_punta` + `ordenes_a_distancia`): con la prediccion
/// encendida, un trazo de n puntos costaba 2 x O(n) por fotograma y dos
/// reservas de n, y se notaba justo al final de un trazo largo. La punta solo
/// necesita el final del trazo, asi que se hace con las ultimas
/// `PUNTOS_DE_PUNTA` muestras mas el punto predicho: coste fijo, no importa
/// lo largo que sea el trazo. Lo que sobra por detras cae dentro de la mancha
/// del trazo real, que ya esta pintada debajo.
///
/// Los puntos del lapiz estan en coordenadas del mundo (`pintado::ordenes`
/// no los desplaza; solo los gira, y un trazo en curso no esta girado), asi
/// que el contorno de la cola cae exactamente donde tiene que caer.
///
/// `None` para lo que no es tinta: la punta de una linea o de una caja es
/// barata y se sigue haciendo con la copia entera.
pub(crate) fn punta_de_tinta(e: &Elemento, q: Punto2) -> Option<Orden> {
    // El trazo a mano de grafito no lleva punta predicha: una punta de tinta
    // lisa pegada a un trazo de sellos se veria como una gota. Se devuelve un
    // aviso sin marca, que no pinta nada, para que nadie tire por la copia
    // entera y lisa. Las figuras de grafito si van por la copia: la pinta
    // `pintar_copia_predicha`, de grafito.
    if pixpin_motor2d::tinta::grafito::es_de_grafito(e) && matches!(e.figura, Figura::Lapiz { .. })
    {
        return Some(Orden::Tinta {
            contorno: Vec::new(),
            color: e.trazo,
        });
    }
    // Con dos puntos o menos, perfect-freehand no tiene trazo del que sacar
    // una cola: se deja la copia entera, que ahi tampoco cuesta nada.
    let cola = |v: &[Punto2]| -> Option<(Vec<Punto2>, usize)> {
        if v.len() <= 2 {
            return None;
        }
        let desde = v.len() - pixpin_tinta::PUNTOS_DE_PUNTA.min(v.len());
        let mut c = v[desde..].to_vec();
        c.push(q);
        Some((c, desde))
    };
    let opacidad = e.opacidad.clamp(0.0, 1.0);
    match &e.figura {
        Figura::Lapiz {
            puntos,
            presiones,
            opciones,
        } => {
            let (c, desde) = cola(puntos)?;
            // Las presiones solo valen si son de verdad (una lista de otra
            // longitud es «simuladas», y entonces no se pasa ninguna: es la
            // misma regla que sigue `contorno_de_lapiz`).
            let mut pr: Vec<f32> = if presiones.len() == puntos.len() {
                presiones[desde..].to_vec()
            } else {
                Vec::new()
            };
            if let Some(&u) = pr.last() {
                // El punto predicho hereda la presion del ultimo real: naciendo
                // a cero se veria como un pico afilado.
                pr.push(u);
            }
            let contorno = pixpin_motor2d::tinta::contorno_de_lapiz(&c, &pr, e.grosor, *opciones);
            (!contorno.is_empty()).then_some(Orden::Tinta {
                contorno,
                color: ColorRgba {
                    a: e.trazo.a * opacidad,
                    ..e.trazo
                },
            })
        }
        Figura::Resaltador { puntos } => {
            let (c, _) = cola(puntos)?;
            let contorno = pixpin_motor2d::tinta::contorno_de_resaltador(&c, e.grosor);
            (!contorno.is_empty()).then_some(Orden::Tinta {
                contorno,
                color: ColorRgba {
                    a: pixpin_motor2d::tinta::OPACIDAD_DEL_RESALTADOR * opacidad,
                    ..e.trazo
                },
            })
        }
        _ => None,
    }
}

/// **La letra con que se pinta un texto del dibujo**: su familia, su cara y,
/// si es de las del catalogo (`texto::FUENTES`), el interlineado fijo de
/// Excalidraw. Las de fuera del catalogo (Segoe UI de los rotulos) siguen
/// con el de su fuente, como siempre.
pub(crate) fn letra_de(
    familia: &str,
    negrita: bool,
    cursiva: bool,
) -> pixpin_render::letras::Letra<'_> {
    pixpin_render::letras::Letra {
        familia,
        negrita,
        cursiva,
        interlineado: pixpin_motor2d::texto::interlineado_de(familia),
    }
}

/// **El medidor del motor** (`texto::Medidor`): lo que mide un texto con
/// DirectWrite y la MISMA letra con que lo pinta `dibujar_orden`. Lo instala
/// `main` al arrancar; asi la caja de un texto es la de lo escrito en todos
/// los anfitriones del nucleo (lienzo, lector, pines y anotador), que
/// comparten el motor.
pub(crate) fn medir_para_el_motor(
    texto: &str,
    tam: f32,
    familia: &str,
    estilo: pixpin_motor2d::texto::EstiloDeTexto,
) -> Option<(f32, f32)> {
    pixpin_render::letras::medir(
        texto,
        tam,
        &letra_de(familia, estilo.negrita, estilo.cursiva),
    )
}

pub(crate) fn a_tuplas(puntos: &[Punto2]) -> Vec<(f32, f32)> {
    puntos.iter().map(|p| (p.x, p.y)).collect()
}

pub(crate) fn a_color(c: ColorRgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// Un color de TINTA (trazo, relleno, texto): el del elemento, adaptado al
/// papel si el fotograma fijo uno de noche (`tema::con_papel`). El papel
/// mismo no pasa por aqui: se pinta con `a_color`, tal cual.
pub(crate) fn a_tinta(c: ColorRgba) -> Color {
    a_color(super::tema::tinta(c))
}

/// **Lo dibujado de una escena, entero**, para los anfitriones que pintan
/// todo en una pasada (el lector: la tinta va con el texto, en la misma
/// transformada). Quien llama ya puso la vista del documento; `vista` es lo
/// que se ve, en unidades del documento, y lo que cae fuera ni se calcula.
///
/// `sin_cache(id)` dice que elementos cambian en cada fotograma (el trazo en
/// curso): cachear su realizacion seria teselar de nuevo cada vez sin
/// acertar nunca, igual que en el lienzo.
///
/// El mosaico no se pinta: tapa leyendo lo pintado debajo, y eso necesita
/// la pasada de tapado del lienzo, que estos anfitriones no tienen.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pintar_escena(
    p: &pixpin_render::Pintor<'_>,
    escena: &pixpin_motor2d::Escena,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    imagenes: &ImagenesLienzo,
    vista: (f32, f32, f32, f32),
    zoom: f32,
    sin_cache: impl Fn(u64) -> bool,
) {
    pintar_escena_si(
        p,
        escena,
        cache,
        cache_tinta,
        imagenes,
        vista,
        zoom,
        sin_cache,
        |_| true,
    );
}

/// Como [`pintar_escena`], pero solo lo que `pintar_este` diga de su caja
/// (con la holgura del trazo, en unidades del documento): el visor de planos
/// pinta asi, encima de la foto de la capa mientras se mueve, solo lo que la
/// foto no tiene entero (`pixpin_render::FotoDeCapa`).
///
/// **Con el presupuesto de la cache de tinta** (`CacheTinta::presupuesto`):
/// lo que se gasta en calcular y teselar cada elemento se apunta, y cuando
/// ya no queda tiempo, lo que tiene su geometria de otra octava de zoom se
/// pinta con ella (y queda pendiente) en vez de recalcularla. Sin
/// presupuesto es lo de siempre.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pintar_escena_si(
    p: &pixpin_render::Pintor<'_>,
    escena: &pixpin_motor2d::Escena,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    imagenes: &ImagenesLienzo,
    vista: (f32, f32, f32, f32),
    zoom: f32,
    sin_cache: impl Fn(u64) -> bool,
    pintar_este: impl Fn((f32, f32, f32, f32)) -> bool,
) {
    let (vx0, vy0, vx1, vy1) = vista;
    let nivel = pixpin_motor2d::cache::nivel_de_detalle(zoom);
    // Lo que se ve: sin las guias escondidas.
    for e in escena.a_la_vista() {
        if matches!(e.figura, Figura::Mosaico { .. }) {
            continue;
        }
        let (x0, y0, x1, y1) = e.caja();
        let holgura = e.grosor * 2.0 + 2.0 / zoom.max(1e-3);
        if x1 + holgura < vx0 || x0 - holgura > vx1 || y1 + holgura < vy0 || y0 - holgura > vy1 {
            continue;
        }
        if !pintar_este((x0 - holgura, y0 - holgura, x1 + holgura, y1 + holgura)) {
            continue;
        }
        let cacheable = !sin_cache(e.id);
        let grano = pixpin_motor2d::pintado::grano_de(e);
        // Sin tiempo, las ordenes de la octava que ya estan (si las hay).
        let zoom_ordenes = match cache.nivel_guardado(e) {
            Some(n) if cacheable && n != nivel && cache_tinta.sin_tiempo() => {
                cache_tinta.apuntar_pendiente();
                2f32.powi(n as i32)
            }
            _ => zoom,
        };
        let (t, realizadas, fallos) = (
            std::time::Instant::now(),
            cache_tinta.realizadas(),
            cache.fallos(),
        );
        let mut indice = 0u32;
        por_cada_orden(cache, e, zoom_ordenes, escena.escala.as_ref(), |orden| {
            let tinta = cacheable.then_some((&mut *cache_tinta, (e.id, e.version, indice)));
            dibujar_orden(p, orden, vista, tinta, imagenes, zoom, grano);
            indice += 1;
        });
        if cache_tinta.realizadas() != realizadas || cache.fallos() != fallos {
            cache_tinta.gastar(t.elapsed());
        }
    }
}

/// **Lo que va encima de lo dibujado**: el marco de lo elegido y sus
/// tiradores, la marquesina, el lazo, la pista del iman, la figura a la que
/// se atara la flecha, los puntos de una flecha elegida y los angulos que se
/// mueven. Es el mismo orden y las mismas llamadas que siempre pinto el
/// lienzo; aqui para que todos los anfitriones lo pinten igual.
///
/// `sin_seleccion` y `sin_marquesina`: lo que el lienzo esta pintando en
/// otra capa mientras se arrastra (se veria dos veces).
#[allow(clippy::too_many_arguments)]
/// **El icono de enlace** de cada elemento que lleva a otra hoja: un
/// redondel azul en la esquina de arriba a la derecha con dos eslabones
/// blancos en diagonal (`Renderer.pintarIconoDeEnlace` del movil). Mide lo
/// mismo a cualquier aumento: el radio va en pixeles de pantalla.
pub(crate) fn pintar_iconos_de_enlace(
    p: &pixpin_render::Pintor<'_>,
    escena: &pixpin_motor2d::Escena,
    zoom: f32,
) {
    let r = pixpin_motor2d::zona::RADIO_DEL_ICONO_DE_ENLACE / zoom.max(0.0001);
    let azul = a_color(pixpin_motor2d::zona::COLOR_DE_LA_ZONA);
    for c in escena
        .elementos
        .iter()
        .filter_map(pixpin_motor2d::zona::icono_de_enlace)
    {
        p.circulo((c.x, c.y), r, azul);
        let (w, h) = (r * 0.62, r * 0.42);
        for (desde, hasta) in [(-0.95 * w, 0.15 * w), (-0.15 * w, 0.95 * w)] {
            let v = capsula(desde, hasta, h / 2.0)
                .into_iter()
                // Girada 45 grados hacia arriba, como `canvas.rotate(-45f)`.
                .map(|(x, y)| {
                    let (s, k) = (-std::f32::consts::FRAC_PI_4).sin_cos();
                    (c.x + x * k - y * s, c.y + x * s + y * k)
                })
                .collect::<Vec<_>>();
            p.polilinea(&v, r * 0.16, Color::BLANCO);
        }
    }
}

/// El brazo de la cruz del centro de un ovalo guia, en pixeles de pantalla
/// (`CENTRO_BRAZO` del movil), y su trazo (`CENTRO_TRAZO`).
const CENTRO_BRAZO: f32 = 7.0;
const CENTRO_TRAZO: f32 = 1.4;

/// **La marca del centro de los ovalos guia** (`marcarCentro` del movil):
/// una cruz pequena en el centro de cada elipse y cada arco de referencia, del
/// color de su trazo y del mismo tamano a cualquier aumento. Un ovalo de
/// referencia esta para clavar el compas en su centro, y sin marca hay que
/// adivinarlo. Solo en pantalla: exportar no pasa por aqui (`paraExportar`).
pub(crate) fn pintar_centros_de_guias(
    p: &pixpin_render::Pintor<'_>,
    escena: &pixpin_motor2d::Escena,
    zoom: f32,
) {
    let brazo = CENTRO_BRAZO / zoom.max(0.0001);
    let trazo = CENTRO_TRAZO / zoom.max(0.0001);
    for e in escena.a_la_vista().filter(|e| es_centro_de_guia(e)) {
        let (x0, y0, x1, y1) = e.caja();
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let color = a_color(ColorRgba { a: 1.0, ..e.trazo });
        p.polilinea(&[(cx - brazo, cy), (cx + brazo, cy)], trazo, color);
        p.polilinea(&[(cx, cy - brazo), (cx, cy + brazo)], trazo, color);
    }
}

/// Si `e` lleva marca de centro: un ovalo o un arco que es guia (`esRedonda`).
pub(crate) fn es_centro_de_guia(e: &pixpin_motor2d::Elemento) -> bool {
    e.extras.referencia && matches!(e.figura, Figura::Elipse | Figura::Arco { .. })
}

/// El contorno cerrado de una capsula horizontal de `desde` a `hasta` y
/// radio `radio`, centrada en y = 0.
fn capsula(desde: f32, hasta: f32, radio: f32) -> Vec<(f32, f32)> {
    const PASOS: usize = 8;
    let mut v = Vec::with_capacity(2 * PASOS + 3);
    let (a, b) = (desde + radio, hasta - radio);
    for i in 0..=PASOS {
        let t = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / PASOS as f32;
        v.push((b + radio * t.cos(), radio * t.sin()));
    }
    for i in 0..=PASOS {
        let t = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / PASOS as f32;
        v.push((a + radio * t.cos(), radio * t.sin()));
    }
    v.push(v[0]);
    v
}

// Lo que hace falta para pintar encima del lienzo; agruparlo no aclara nada.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pintar_encima(
    p: &pixpin_render::Pintor<'_>,
    gesto: &pixpin_motor2d::gesto::Gesto,
    escena: &pixpin_motor2d::Escena,
    vista: (f32, f32, f32, f32),
    zoom: f32,
    imagenes: &ImagenesLienzo,
    sin_seleccion: bool,
    sin_marquesina: bool,
) {
    let escala = 1.0 / zoom;
    // Los puntos tecleados y su eje, encima de lo dibujado y debajo de la
    // seleccion: son la referencia contra la que se dibuja.
    for orden in ordenes_de_las_tablas(escena, zoom, Some(vista)) {
        dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
    }
    // `Gesto::tiradores` es la MISMA llamada que usa el gesto para decidir
    // que agarra el clic: pintar con una copia propia del angulo es como se
    // desincronizaron una vez -tiradores rectos que se picaban girados-.
    // Mientras se escribe no hay marco (`Gesto::marco_visible`): solo el
    // cursor, como en Excalidraw.
    if let Some(caja) = gesto
        .seleccion
        .caja(escena)
        .filter(|_| !sin_seleccion && gesto.marco_visible())
    {
        let tiradores = gesto.tiradores(escena, escala);
        let angulo = tiradores.as_ref().map_or(0.0, |t| t.angulo);
        p.marco(caja, angulo, escala);
        if let Some(tiradores) = tiradores {
            for orden in tiradores.ordenes(escala) {
                dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
            }
        }
    }
    // Lo elegido del texto que se escribe (Mayus+flechas), debajo del cursor.
    for orden in gesto.seleccion_de_texto(escena) {
        dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
    }
    if let Some(cursor) = gesto.cursor_de_texto(escena, zoom) {
        dibujar_orden(p, &cursor, vista, None, imagenes, zoom, None);
    }
    if let Some(m) = gesto.marquesina().filter(|_| !sin_marquesina) {
        p.marquesina(m, escala);
    }
    // El rastro del lazo, con la marquesina: es lo mismo —decir que se esta
    // encerrando— y va igual de encima de todo.
    if let Some(l) = &gesto.lazo {
        if let Some(relleno) = l.relleno() {
            dibujar_orden(p, &relleno, vista, None, imagenes, zoom, None);
        }
        dibujar_orden(p, &l.orden(escala), vista, None, imagenes, zoom, None);
    }
    // El rectangulo de la zona, como la marquesina (el movil lo pinta con
    // su `selectionBox`), y la estela del laser.
    if let Some((a, b)) = gesto.zona {
        p.marquesina(pixpin_motor2d::zona::caja(a, b), escala);
    }
    // El icono de enlace de las marcas de zona (F8), que dice «esto lleva a
    // otra hoja». Solo en pantalla: exportar no pasa por aqui, como el
    // `paraExportar` del movil.
    pintar_iconos_de_enlace(p, escena, zoom);
    pintar_centros_de_guias(p, escena, zoom);
    for orden in gesto.laser.ordenes(zoom) {
        dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
    }
    // La bolita de elegir, mientras se pasa (`bolita.rs`).
    if let Some(orden) = gesto.bolita.orden(zoom) {
        dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
    }
    // Los clics ya puestos del arco exacto (centro, inicio...).
    for orden in gesto.ordenes_de_arco_en_curso(zoom) {
        dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
    }
    // La pista del iman: encima de todo, porque es lo que dice donde va a
    // caer el punto.
    if let Some(a) = gesto.anclaje_activo {
        dibujar_orden(
            p,
            &pixpin_motor2d::enganche::pista(&a, zoom),
            vista,
            None,
            imagenes,
            zoom,
            None,
        );
    }
    // La figura a la que se va a atar la flecha, y los puntos que se pueden
    // coger de una flecha elegida: la respuesta a «que pasa si suelto aqui».
    for orden in gesto.resaltado_de_union(escena, zoom) {
        dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
    }
    if let Some(puntas) = gesto.tiradores_de_punta(escena, escala) {
        for orden in puntas.ordenes(escala) {
            dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
        }
    }
    // Los angulos de las esquinas que se estan moviendo. La pregunta barata
    // va delante para no montar los angulos de un plano entero por aviso.
    if super::construir::hay_angulos_que_ensenar(gesto) {
        for orden in super::construir::angulos_en_vivo(escena, gesto, zoom) {
            dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
        }
    }
    // Las cabezas de los clavos de soldar vertices, siempre a la vista como
    // en el movil (`nudosVisibles`): un clavo que no se ve no se puede quitar.
    for orden in pixpin_motor2d::nudos::ordenes_de_clavos(escena, zoom) {
        dibujar_orden(p, &orden, vista, None, imagenes, zoom, None);
    }
}

/// **Lo mismo que `pintar_encima`, como ordenes del motor**, para quien no
/// pinta con un `Pintor` propio sino que entrega ordenes a otro (el pin las
/// pinta en su ventana). El marco de cada elegido y la marquesina van como
/// rayas: una orden no sabe de rellenos translucidos de interfaz.
pub(crate) fn ordenes_encima(
    gesto: &pixpin_motor2d::gesto::Gesto,
    escena: &pixpin_motor2d::Escena,
    zoom: f32,
) -> Vec<Orden> {
    let escala = 1.0 / zoom.max(1e-6);
    let mut v = Vec::new();
    if gesto.marco_visible() {
        for id in gesto.seleccion.ids() {
            if let Some(marco) = pixpin_motor2d::pintado::marco_de_seleccion(escena, *id, escala) {
                v.push(marco);
            }
        }
    }
    if let Some(tiradores) = gesto.tiradores(escena, escala) {
        v.extend(tiradores.ordenes(escala));
    }
    v.extend(gesto.seleccion_de_texto(escena));
    v.extend(gesto.cursor_de_texto(escena, zoom));
    if let Some((x0, y0, x1, y1)) = gesto.marquesina() {
        v.push(Orden::Polilinea {
            puntos: vec![
                Punto2::nuevo(x0, y0),
                Punto2::nuevo(x1, y0),
                Punto2::nuevo(x1, y1),
                Punto2::nuevo(x0, y1),
                Punto2::nuevo(x0, y0),
            ],
            color: ColorRgba {
                r: 0.41,
                g: 0.40,
                b: 0.84,
                a: 0.9,
            },
            grosor: escala,
            estilo: EstiloTrazo::Discontinuo,
        });
    }
    if let Some(l) = &gesto.lazo {
        if let Some(relleno) = l.relleno() {
            v.push(relleno);
        }
        v.push(l.orden(escala));
    }
    v.extend(gesto.laser.ordenes(zoom));
    v.extend(gesto.bolita.orden(zoom));
    v.extend(gesto.ordenes_de_arco_en_curso(zoom));
    if let Some(a) = gesto.anclaje_activo {
        v.push(pixpin_motor2d::enganche::pista(&a, zoom));
    }
    v.extend(gesto.resaltado_de_union(escena, zoom));
    if let Some(puntas) = gesto.tiradores_de_punta(escena, escala) {
        v.extend(puntas.ordenes(escala));
    }
    if super::construir::hay_angulos_que_ensenar(gesto) {
        v.extend(super::construir::angulos_en_vivo(escena, gesto, zoom));
    }
    v.extend(pixpin_motor2d::nudos::ordenes_de_clavos(escena, zoom));
    // Las tablas de coordenadas con su eje (sin la vista no se tiran las
    // rectas del eje de lado a lado; queda la cruz del cero).
    v.splice(0..0, ordenes_de_las_tablas(escena, zoom, None));
    v
}

/// **Los puntos de las tablas de coordenadas y su eje**, en pantalla: del
/// mismo tamano a cualquier aumento y encima de lo dibujado, como en el
/// movil (`Renderer.drawTabla`, `drawEjeDeCoordenadas`). Exportar no pasa
/// por aqui: alli salen los puntos y no el eje (`exportar`).
pub(crate) fn ordenes_de_las_tablas(
    escena: &pixpin_motor2d::Escena,
    zoom: f32,
    vista: Option<(f32, f32, f32, f32)>,
) -> Vec<Orden> {
    pixpin_motor2d::instrumento::tabla::ordenes(
        &escena.tablas,
        escena.origen_coordenadas,
        escena.escala.as_ref(),
        Some(zoom),
        vista,
    )
}

/// **Las muestras del texto, las letras y el resaltador**, pintadas con las
/// MISMAS funciones que el lienzo (`pintar_escena` y `pintar_encima`) y el
/// medidor de verdad, en PNG para mirarlas. Necesita GPU: `cargo test -p
/// pixpin --bin pixpinmax muestra_de_letras -- --ignored --nocapture`. Deja
/// los PNG en `PIXPIN_MUESTRAS` o en la carpeta temporal.
#[cfg(test)]
mod muestras {
    use super::*;
    use pixpin_motor2d::elemento::Elemento;
    use pixpin_motor2d::escena::Escena;
    use pixpin_motor2d::estilo::NivelGrosor;
    use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};

    #[test]
    fn solo_los_ovalos_y_arcos_guia_llevan_marca_de_centro() {
        let guia = |figura: Figura, referencia: bool| {
            let mut e = Elemento {
                figura,
                ancho: 40.0,
                alto: 40.0,
                ..Default::default()
            };
            e.extras.referencia = referencia;
            e
        };
        assert!(es_centro_de_guia(&guia(Figura::Elipse, true)));
        assert!(es_centro_de_guia(&guia(
            Figura::Arco {
                inicio: 0.0,
                barrido: None
            },
            true
        )));
        // Casos negativos: el ovalo de dibujo y el rectangulo guia.
        assert!(!es_centro_de_guia(&guia(Figura::Elipse, false)));
        assert!(!es_centro_de_guia(&guia(Figura::Rectangulo, true)));
    }

    const BLANCO: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    fn pulsar(g: &mut Gesto, e: &mut Escena, x: f32, y: f32) {
        g.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(x, y),
                shift: false,
                alt: false,
                presion: None,
            },
            e,
            1.0,
        );
    }

    fn soltar(g: &mut Gesto, e: &mut Escena, x: f32, y: f32) {
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(x, y),
            },
            e,
            1.0,
        );
    }

    /// Pinta la escena (y lo de encima del gesto) a `zoom` y guarda el PNG.
    fn guardar(
        nombre: &str,
        escena: &Escena,
        gesto: &Gesto,
        zoom: f32,
        (ancho, alto): (u32, u32),
        cajas: bool,
    ) {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
        let mut cache = Cache::nueva();
        let mut cache_tinta = pixpin_render::CacheTinta::nueva();
        let imagenes = ImagenesLienzo::nuevo(4096);
        let vista = (0.0, 0.0, ancho as f32 / zoom, alto as f32 / zoom);
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(BLANCO);
                p.poner_vista((0.0, 0.0), zoom, (0.0, 0.0));
                pintar_escena(
                    p,
                    escena,
                    &mut cache,
                    &mut cache_tinta,
                    &imagenes,
                    vista,
                    zoom,
                    |_| false,
                );
                // La caja de cada texto, en rojo fino: para ver a ojo que la
                // caja es lo escrito.
                if cajas {
                    for e in escena.visibles() {
                        if matches!(e.figura, Figura::Texto { .. }) {
                            let (x0, y0, x1, y1) = e.caja();
                            p.polilinea(
                                &[(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)],
                                0.6 / zoom,
                                Color {
                                    r: 0.9,
                                    g: 0.1,
                                    b: 0.1,
                                    a: 0.8,
                                },
                            );
                        }
                    }
                }
                pintar_encima(p, gesto, escena, vista, zoom, &imagenes, false, false);
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho,
            alto,
            pixeles,
        })
        .expect("codificar");
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let ruta = carpeta.join(format!("{nombre}.png"));
        std::fs::write(&ruta, png).expect("guardar");
        println!("{nombre}: {}", ruta.display());
    }

    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_letras() {
        pixpin_motor2d::texto::con_medidor(medir_para_el_motor, || {
            // 1. Escribiendo: sin caja, con cursor.
            let mut escena = Escena::nueva();
            let mut g = Gesto::nuevo();
            g.tomar_herramienta(Herramienta::Texto);
            pulsar(&mut g, &mut escena, 20.0, 20.0);
            soltar(&mut g, &mut escena, 20.0, 20.0);
            for c in "Hola, esto se escribe".chars() {
                g.escribir(c, &mut escena);
            }
            g.tecla_de_texto(pixpin_motor2d::texto::TeclaTexto::Entrar, &mut escena);
            for c in "sin caja".chars() {
                g.escribir(c, &mut escena);
            }
            guardar("texto-escribiendo", &escena, &g, 2.0, (560, 180), false);
            // 2. Cerrado con Escape: elegido, con la caja exacta.
            g.cerrar_texto_con_teclado(&mut escena);
            guardar("texto-elegido", &escena, &g, 2.0, (560, 180), false);

            // 3. Las ocho letras, normal y negrita, con su caja en rojo.
            let mut escena = Escena::nueva();
            let mut y = 10.0;
            for f in pixpin_motor2d::texto::FUENTES {
                for negrita in [false, true] {
                    let mut e = Elemento {
                        figura: Figura::Texto {
                            texto: format!(
                                "{} ({}): Hola, \u{bf}qu\u{e9} tal? \u{f1}\u{d1} 123",
                                f.nombre, f.etiqueta
                            ),
                            tam: 20.0,
                            familia: f.nombre.to_string(),
                        },
                        x: 10.0,
                        y,
                        ..Default::default()
                    };
                    e.extras.negrita = negrita;
                    let id = escena.anadir(e);
                    Gesto::ajustar_caja_de_texto(&mut escena, id);
                    y += escena.buscar(id).map_or(30.0, |e| e.alto) + 6.0;
                }
            }
            let alto = (y * 1.5) as u32 + 10;
            guardar("letras", &escena, &Gesto::nuevo(), 1.5, (900, alto), true);

            // 4. El resaltador fino, medio y grueso, sobre un renglon.
            let mut escena = Escena::nueva();
            let mut y = 20.0;
            for nivel in [NivelGrosor::Fino, NivelGrosor::Medio, NivelGrosor::Grueso] {
                let id = escena.anadir(Elemento {
                    figura: Figura::Texto {
                        texto: format!("Resaltador {nivel:?}: esto se subraya"),
                        tam: 20.0,
                        familia: "Excalifont".into(),
                    },
                    x: 20.0,
                    y,
                    ..Default::default()
                });
                Gesto::ajustar_caja_de_texto(&mut escena, id);
                let mut g = Gesto::nuevo();
                g.estilo.grosor = nivel;
                g.estilo.trazo = pixpin_motor2d::ColorRgba::opaco(1.0, 0.85, 0.0);
                g.tomar_herramienta(Herramienta::Resaltador);
                pulsar(&mut g, &mut escena, 20.0, y + 14.0);
                for i in 1..60 {
                    g.evento(
                        EventoGesto::Mover {
                            p: Punto2::nuevo(20.0 + i as f32 * 5.0, y + 14.0),
                            shift: false,
                            alt: false,
                            presion: None,
                        },
                        &mut escena,
                        1.0,
                    );
                }
                soltar(&mut g, &mut escena, 315.0, y + 14.0);
                y += 45.0;
            }
            guardar(
                "resaltador",
                &escena,
                &Gesto::nuevo(),
                2.0,
                (760, 300),
                false,
            );
        });
    }

    /// **La raya de resaltador**: a mano (izquierda) y la misma parada hasta
    /// volverse recta (derecha), fino, medio y grueso. Tienen que medir lo
    /// mismo. `cargo test -p pixpin --bin pixpinmax muestra_del_resaltador_recto
    /// -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_resaltador_recto() {
        let mut escena = Escena::nueva();
        let mut y = 30.0;
        for nivel in [NivelGrosor::Fino, NivelGrosor::Medio, NivelGrosor::Grueso] {
            for (x0, pararse) in [(20.0, false), (260.0, true)] {
                let mut g = Gesto::nuevo();
                g.enganche.activo = false;
                g.estilo.grosor = nivel;
                g.estilo.trazo = pixpin_motor2d::ColorRgba::opaco(1.0, 0.85, 0.0);
                g.tomar_herramienta(Herramienta::Resaltador);
                pulsar(&mut g, &mut escena, x0, y);
                for i in 1..=40 {
                    g.evento(
                        EventoGesto::Mover {
                            p: Punto2::nuevo(x0 + i as f32 * 5.0, y),
                            shift: false,
                            alt: false,
                            presion: None,
                        },
                        &mut escena,
                        1.0,
                    );
                }
                if pararse {
                    g.convertir_en_forma(&mut escena, Punto2::nuevo(x0 + 200.0, y), 1.0);
                }
                soltar(&mut g, &mut escena, x0 + 200.0, y);
            }
            y += 80.0;
        }
        // Abajo, la raya de ANTES (D45: grosor 3, tinta constante de 9 x
        // sen 45 = 12,7 de ancho, al 35 %), para comparar con el medio.
        escena.anadir(Elemento {
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(20.0, y), Punto2::nuevo(220.0, y)],
            },
            grosor: 12.7,
            rugosidad: 0.0,
            trazo: pixpin_motor2d::ColorRgba::opaco(1.0, 0.85, 0.0),
            opacidad: 0.35,
            ..Default::default()
        });
        guardar(
            "resaltador-recto",
            &escena,
            &Gesto::nuevo(),
            2.0,
            (980, 560),
            false,
        );
    }

    /// **Los pasos numerados y el foco**, con el gesto de verdad: cuatro
    /// clics con Pasos (colores y letras distintas: el numero va en blanco o
    /// negro segun el disco) y un circulo tocado con el Foco, que oscurece
    /// solo el anillo de su marco. `cargo test -p pixpin --bin pixpinmax
    /// muestra_de_pasos_y_foco -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_pasos_y_foco() {
        use pixpin_motor2d::estilo::CambioForma;
        pixpin_motor2d::texto::con_medidor(medir_para_el_motor, || {
            let mut escena = Escena::nueva();
            for (i, t) in ["Paso uno: abrir", "Paso dos: elegir", "Paso tres: guardar"]
                .iter()
                .enumerate()
            {
                let id = escena.anadir(Elemento {
                    figura: Figura::Texto {
                        texto: t.to_string(),
                        tam: 20.0,
                        familia: "Excalifont".into(),
                    },
                    x: 60.0,
                    y: 20.0 + i as f32 * 50.0,
                    ..Default::default()
                });
                Gesto::ajustar_caja_de_texto(&mut escena, id);
            }
            let mut g = Gesto::nuevo();
            g.enganche.activo = false;
            g.tomar_herramienta(Herramienta::Serie);
            let colores = [
                (
                    pixpin_motor2d::ColorRgba::opaco(0.12, 0.12, 0.12),
                    None,
                    20.0,
                ),
                (
                    pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19),
                    Some(pixpin_motor2d::texto::FUENTE_CAVEAT),
                    20.0,
                ),
                (
                    pixpin_motor2d::ColorRgba::opaco(1.0, 0.85, 0.0),
                    Some(pixpin_motor2d::texto::FUENTE_NUNITO),
                    20.0,
                ),
                (
                    pixpin_motor2d::ColorRgba::opaco(0.1, 0.44, 0.76),
                    Some(pixpin_motor2d::texto::FUENTE_EXCALIFONT),
                    28.0,
                ),
            ];
            for (i, (color, familia, tam)) in colores.iter().enumerate() {
                g.estilo.trazo = *color;
                if let Some(n) = familia {
                    g.estilo.aplicar_forma(CambioForma::Familia(*n));
                }
                g.estilo.aplicar_forma(CambioForma::TamanoLetra(*tam));
                let (x, y) = if i < 3 {
                    (30.0, 32.0 + i as f32 * 50.0)
                } else {
                    (300.0, 32.0)
                };
                pulsar(&mut g, &mut escena, x, y);
                soltar(&mut g, &mut escena, x, y);
            }
            // El foco: un circulo alrededor del paso dos, tocado con el Foco.
            let circulo = escena.anadir(Elemento {
                figura: Figura::Elipse,
                x: 50.0,
                y: 58.0,
                ancho: 200.0,
                alto: 44.0,
                trazo: pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19),
                grosor: 2.0,
                rugosidad: 0.0,
                ..Default::default()
            });
            let mut gf = Gesto::nuevo();
            crate::dibujo::lupa::convertir_en_foco(&mut escena, &mut gf, circulo).expect("foco");
            guardar(
                "pasos-y-foco",
                &escena,
                &Gesto::nuevo(),
                2.0,
                (760, 400),
                false,
            );
        });
    }

    /// La marca de una zona mandada al chat (F8): el recuadro discontinuo
    /// azul con su icono de enlace, sobre un poco de dibujo, a dos aumentos
    /// (el icono mide lo mismo en pantalla).
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_la_marca_de_zona() {
        let mut escena = Escena::nueva();
        for (x, y, w, h) in [(40.0, 40.0, 180.0, 110.0), (260.0, 70.0, 140.0, 140.0)] {
            escena.anadir(Elemento {
                x,
                y,
                ancho: w,
                alto: h,
                trazo: ColorRgba::opaco(0.12, 0.12, 0.12),
                ..Default::default()
            });
        }
        pixpin_motor2d::zona::marcar(&mut escena, (20.0, 25.0, 250.0, 170.0), "foto-1");
        guardar(
            "zona-marca-con-enlace",
            &escena,
            &Gesto::nuevo(),
            1.0,
            (460, 260),
            false,
        );
        guardar(
            "zona-marca-con-enlace-x2",
            &escena,
            &Gesto::nuevo(),
            2.0,
            (920, 520),
            false,
        );
    }
}
