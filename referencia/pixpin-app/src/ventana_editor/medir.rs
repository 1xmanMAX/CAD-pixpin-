//! El fotograma del EDITOR medido sin ventana, como ya se medía el del
//! universo (`universo/sesion/medir.rs`).
//!
//! Separa lo que cuesta la CPU en decidir qué se ve (`preparar`: la rejilla
//! y sus candidatos) de lo que cuesta encargarle a Direct2D cada primitiva
//! (`encargar`), y de lo que tarda de verdad con la GPU terminada (`total`).
//! Y cuenta los objetos de Direct2D/DirectWrite creados por fotograma
//! (`MotorRender::contadores`), que es el número que la investigación del
//! 2026-09-19 quería ver en cero durante un paneo.
//!
//! Va con `#[ignore]` porque necesita GPU: se ejecuta a mano, en `--release`,
//!
//! ```text
//! cargo test --release -p pixpin --bin pixpinmax medir_fotograma_del_editor -- --ignored --nocapture --test-threads=1
//! ```

use std::time::Instant;

use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::indice::Rejilla;
use pixpin_motor2d::tinta::OpcionesTinta;
use pixpin_motor2d::{ColorRgba, Elemento, EstiloRelleno, EstiloTrazo, Figura, TipoPunta};
use pixpin_render::fuera_de_pantalla::FueraDePantalla;

use super::*;

/// La pantalla del equipo del usuario al 150 %, igual que el banco del
/// universo: los dos números son comparables entre sí.
const ANCHO: u32 = 3000;
const ALTO: u32 = 2000;
const ESCALA: u32 = 150;
/// Fotogramas por guion.
const VUELTAS: usize = 60;

/// Milisegundos medios por fotograma, el peor total, y cuántos objetos de
/// Direct2D/DirectWrite se crearon de media en cada uno.
#[derive(Debug, Clone, Copy, Default)]
struct Medida {
    preparar: f64,
    encargar: f64,
    total: f64,
    peor: f64,
    objetos: f64,
}

struct Banco {
    motor: MotorRender,
    destino: FueraDePantalla,
    /// B2: la capa de la tinta viva. Fuera de pantalla es otra textura del
    /// tamano de la VENTANA (la de verdad es una `IDCompositionSurface`, que
    /// no se puede crear sin ventana); lo que se mide con ella es lo unico
    /// que cambia entre los dos caminos: sobre que se pinta el trazo y
    /// cuanto hay que mover antes de pintarlo.
    tinta: FueraDePantalla,
    /// El «mapa de delante» de la cadena de intercambio, para medir soltar
    /// el lapiz (`hornear_trazo`): antes de pintar el trazo se copia de el
    /// al de atras lo que difiera.
    delante: FueraDePantalla,
    imagenes: ImagenesLienzo,
    /// El D3D del motor: `devolver_memoria` tiene que recortar ESTE.
    dispositivo: pixpin_capture::Dispositivo,
}

impl Banco {
    fn nuevo() -> Banco {
        let dispositivo = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(dispositivo.d3d()).expect("motor");
        let destino =
            FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ANCHO, ALTO).expect("destino");
        let tinta = FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ANCHO, ALTO).expect("tinta");
        let delante =
            FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ANCHO, ALTO).expect("delante");
        let imagenes = ImagenesLienzo::nuevo(motor.lado_maximo_bitmap());
        Banco {
            motor,
            destino,
            tinta,
            delante,
            imagenes,
            dispositivo,
        }
    }

    /// Un fotograma de la ESCENA: lo mismo que hornea `CapaEstatica::preparar`
    /// y lo mismo que repinta `pintar` cuando la cámara se mueve. No incluye
    /// la barra ni el panel: son unas decenas de primitivas fijas que no
    /// cambian con el tamaño del documento, y meterlas solo añadiría ruido.
    ///
    /// `formas_cacheadas` a `false` reproduce el camino de ANTES de A1: la
    /// tinta con su realizacion (D121) y las formas de rough.js creando su
    /// geometria en cada fotograma. Es lo que hace que el banco imprima el
    /// antes y el despues en la misma vuelta, con la misma GPU, la misma
    /// escena y la misma camara: dos ejecuciones distintas no serian
    /// comparables.
    fn fotograma(
        &self,
        escena: &Escena,
        rejilla: &Rejilla,
        cache: &mut Cache,
        cache_tinta: &mut pixpin_render::CacheTinta,
        camara: &Camara,
        formas_cacheadas: bool,
    ) -> (f64, f64, f64, u32) {
        let efectiva = crate::navegacion::vista_efectiva(camara, ESCALA);
        let vista = efectiva.ventana(ANCHO as f32, ALTO as f32);
        let t0 = Instant::now();
        let candidatos = rejilla.candidatos(vista);
        let t1 = Instant::now();
        self.motor
            .dibujar(&self.destino.destino, |p| {
                p.limpiar(Color::BLANCO);
                let origen = efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
                p.poner_vista((0.0, 0.0), efectiva.zoom, (origen.x, origen.y));
                for id in &candidatos {
                    let Some(e) = elemento_por_id(escena, rejilla, *id) else {
                        continue;
                    };
                    if e.borrado {
                        continue;
                    }
                    let mut indice = 0u32;
                    let grano = pixpin_motor2d::pintado::grano_de(e);
                    por_cada_orden(cache, e, efectiva.zoom, escena.escala.as_ref(), |orden| {
                        let con_cache = formas_cacheadas || matches!(orden, Orden::Tinta { .. });
                        dibujar_orden(
                            p,
                            orden,
                            vista,
                            con_cache.then_some((&mut *cache_tinta, (e.id, e.version, indice))),
                            &self.imagenes,
                            efectiva.zoom,
                            grano,
                        );
                        indice += 1;
                    });
                }
            })
            .expect("fotograma");
        let objetos = self.motor.contadores().total();
        let t2 = Instant::now();
        self.destino.esperar_gpu().expect("GPU");
        let t3 = Instant::now();
        let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1000.0;
        (ms(t0, t1), ms(t1, t2), ms(t0, t3), objetos)
    }

    /// `VUELTAS` fotogramas con las cámaras que diga `camara(i)`.
    ///
    /// Las cachés (la de órdenes y la de realizaciones) se crean aquí y
    /// viven todo el guion: es lo que hace el editor, y medir con cachés
    /// frías mediría el primer fotograma sesenta veces.
    fn medir(
        &self,
        escena: &Escena,
        formas_cacheadas: bool,
        camara: impl Fn(usize) -> Camara,
    ) -> Medida {
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(escena);
        let mut cache = Cache::nueva();
        let mut cache_tinta = pixpin_render::CacheTinta::nueva();
        cache_tinta.fijar_escala(crate::navegacion::vista_efectiva(&camara(0), ESCALA).zoom);
        // Calentar: teselaciones, pinceles, disposiciones de texto. Sin esto
        // el primero paga toda la escena y la media no diría nada del paneo.
        for i in 0..3 {
            self.fotograma(
                escena,
                &rejilla,
                &mut cache,
                &mut cache_tinta,
                &camara(i),
                formas_cacheadas,
            );
        }
        let mut m = Medida::default();
        for i in 0..VUELTAS {
            let (a, b, t, o) = self.fotograma(
                escena,
                &rejilla,
                &mut cache,
                &mut cache_tinta,
                &camara(i),
                formas_cacheadas,
            );
            m.preparar += a;
            m.encargar += b;
            m.total += t;
            m.objetos += o as f64;
            m.peor = m.peor.max(t);
        }
        let n = VUELTAS as f64;
        m.preparar /= n;
        m.encargar /= n;
        m.total /= n;
        m.objetos /= n;
        m
    }
}

impl Banco {
    /// El mismo guion de cámara, pero con A3: se pinta solo cuando
    /// `transformada_de_camara` dice que el colchón ya no da.
    ///
    /// Devuelve la medida REPARTIDA entre los `VUELTAS` fotogramas del
    /// guion (que es lo que cuesta de media un fotograma de paneo) y cuántos
    /// repintados hubo. Los fotogramas compuestos no se miden porque no hay
    /// nada que medir: ni recorrido de escena, ni primitivas, ni present.
    ///
    /// **Lo que este número se deja fuera, dicho claro:** el repintado de
    /// verdad cubre la ventana MÁS el colchón (a 3000x2000 con 256 px de
    /// colchón, un 47 % más de superficie y de candidatos), y aquí se pinta
    /// solo la ventana, porque el destino del banco es del tamaño de la
    /// pantalla. Así que el coste por repintado está subestimado en ese
    /// orden; el número de repintados, que es donde está la ganancia, no.
    fn medir_por_composicion(
        &self,
        escena: &Escena,
        camara: impl Fn(usize) -> Camara,
    ) -> (Medida, usize) {
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(escena);
        let mut cache = Cache::nueva();
        let mut cache_tinta = pixpin_render::CacheTinta::nueva();
        cache_tinta.fijar_escala(crate::navegacion::vista_efectiva(&camara(0), ESCALA).zoom);
        for i in 0..3 {
            self.fotograma(
                escena,
                &rejilla,
                &mut cache,
                &mut cache_tinta,
                &camara(i),
                true,
            );
        }
        let mut m = Medida::default();
        let mut repintados = 0usize;
        let mut pintada = crate::navegacion::vista_efectiva(&camara(0), ESCALA);
        for i in 0..VUELTAS {
            let efectiva = crate::navegacion::vista_efectiva(&camara(i), ESCALA);
            if crate::ventana_editor::transformada_de_camara(
                &pintada,
                &efectiva,
                ANCHO as f32,
                ALTO as f32,
                MARGEN_ESCENA as f32,
            )
            .is_some()
            {
                continue;
            }
            repintados += 1;
            pintada = efectiva;
            let (a, b, t, o) = self.fotograma(
                escena,
                &rejilla,
                &mut cache,
                &mut cache_tinta,
                &camara(i),
                true,
            );
            m.preparar += a;
            m.encargar += b;
            m.total += t;
            m.objetos += o as f64;
            m.peor = m.peor.max(t);
        }
        let n = VUELTAS as f64;
        m.preparar /= n;
        m.encargar /= n;
        m.total /= n;
        m.objetos /= n;
        (m, repintados)
    }
}

/// Un elemento con todo lo que no interesa puesto a lo de siempre.
fn base(id: u64, figura: Figura, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
    Elemento {
        id,
        figura,
        x,
        y,
        ancho,
        alto,
        angulo: 0.0,
        trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
        relleno: None,
        estilo_relleno: EstiloRelleno::Rayado,
        grosor: 2.0,
        estilo: EstiloTrazo::Solido,
        rugosidad: 1.0,
        opacidad: 1.0,
        // La semilla varía con el id: rough.js dibuja cada forma distinta y
        // una escena con todas iguales mediría un caso que no existe.
        semilla: 1 + id as u32 * 2_654_435_761u32.wrapping_rem(1_000_003),
        version: 1,
        borrado: false,
        grupos: Vec::new(),
        bloqueado: false,
        enlace: None,
        redondo: false,
        material: Default::default(),
        extras: Default::default(),
    }
}

/// Un trazo a mano de `puntos` puntos, ondulado, dentro de una caja de
/// 220 x 160 en `(x, y)`.
fn trazo(id: u64, x: f32, y: f32, puntos: usize) -> Elemento {
    let p: Vec<Punto2> = (0..puntos)
        .map(|i| {
            let t = i as f32 / puntos as f32;
            Punto2::nuevo(
                x + t * 220.0,
                y + 80.0 + 60.0 * (t * 9.0 + id as f32).sin() + 12.0 * (t * 41.0).cos(),
            )
        })
        .collect();
    let mut e = base(id, Figura::Rectangulo, x, y, 220.0, 160.0);
    e.figura = Figura::Lapiz {
        presiones: (0..puntos)
            .map(|i| 0.4 + 0.5 * (i as f32 / puntos as f32 * 3.0).sin().abs())
            .collect(),
        puntos: p,
        opciones: Some(OpcionesTinta::default()),
    };
    e
}

/// Una escena con el reparto que pidió la investigación: mitad trazos a
/// mano, una cuarta parte formas de rough.js CON relleno de sombreado (lo
/// caro), y el resto flechas y textos.
pub(crate) fn escena_sintetica(cuantos: usize) -> Escena {
    let mut escena = Escena::nueva();
    // Rejilla ancha: a la cámara del guion se ven unos cuantos cientos, que
    // es el caso real. Todos encima seria medir otra cosa.
    let por_fila = (cuantos as f32).sqrt().ceil() as usize;
    for i in 0..cuantos {
        let id = i as u64 + 1;
        let (fx, fy) = ((i % por_fila) as f32, (i / por_fila) as f32);
        let (x, y) = (fx * 260.0, fy * 200.0);
        let e = match i % 4 {
            0 | 1 => trazo(id, x, y, 90),
            2 => {
                let mut e = if i % 8 == 2 {
                    base(id, Figura::Rectangulo, x, y, 200.0, 140.0)
                } else {
                    base(id, Figura::Elipse, x, y, 200.0, 140.0)
                };
                // El relleno de sombreado es lo que genera decenas de
                // polilíneas por forma: es el peor caso de rough.js.
                e.relleno = Some(ColorRgba::opaco(0.85, 0.9, 1.0));
                e
            }
            _ => {
                if i % 8 == 3 {
                    let mut e = base(id, Figura::Rectangulo, x, y, 240.0, 40.0);
                    e.figura = Figura::Texto {
                        texto: format!("Un rótulo del elemento número {i}"),
                        tam: 20.0,
                        familia: "Segoe UI".into(),
                    };
                    e
                } else {
                    let mut e = base(id, Figura::Rectangulo, x, y, 220.0, 120.0);
                    // Los puntos son del mundo, no relativos a `x, y`: con
                    // (0,0) todas las flechas caian amontonadas en el origen
                    // (medio millar en la escena de 2.000) y la puerta del
                    // arrastre las contaba a todas en la zona de la esquina.
                    e.figura = Figura::Flecha {
                        puntos: vec![Punto2::nuevo(x, y), Punto2::nuevo(x + 220.0, y + 120.0)],
                        punta_inicio: TipoPunta::Ninguna,
                        punta_fin: TipoPunta::Flecha,
                        codos: false,
                    };
                    e
                }
            }
        };
        escena.elementos.push(e);
    }
    escena.siguiente_id = cuantos as u64 + 1;
    escena
}

/// Una cámara que encaja la escena entera y luego se acerca `factor`.
fn camara_de(escena: &Escena, factor: f32) -> Camara {
    let caja = escena.caja().unwrap_or((0.0, 0.0, 100.0, 100.0));
    let e = crate::navegacion::escala_de(ESCALA);
    let (w, h) = (ANCHO as f32 / e, ALTO as f32 / e);
    let c = Camara::encajar(caja, w, h, 1.0);
    let centro = (c.x + w / (2.0 * c.zoom), c.y + h / (2.0 * c.zoom));
    let zoom = c.zoom * factor;
    Camara {
        x: centro.0 - w / (2.0 * zoom),
        y: centro.1 - h / (2.0 * zoom),
        zoom,
    }
}

fn escribir(nombre: &str, m: Medida) {
    println!(
        "{nombre:<38} preparar {:>6.2} ms | encargar {:>7.2} ms | CPU {:>7.2} ms | total {:>7.2} ms (peor {:>7.2}) | objetos D2D/fotograma {:>8.1}",
        m.preparar,
        m.encargar,
        m.preparar + m.encargar,
        m.total,
        m.peor,
        m.objetos
    );
}

/// Los tres guiones: cámara quieta, paneo de 60 fotogramas a 30 px lógicos
/// (un arrastre rápido: 1.800 px/s a 60 Hz) y zoom continuo de ×1 a ×2 sin
/// reposar (mientras el zoom no reposa, las realizaciones se estiran: D122).
fn medir_todo(b: &Banco, escena: &Escena, que: &str) {
    let quieta = camara_de(escena, 4.0);
    let paneo = move |i: usize| Camara {
        x: quieta.x + i as f32 * 30.0 / quieta.zoom,
        y: quieta.y + i as f32 * 12.0 / quieta.zoom,
        ..quieta
    };
    let zoom = move |i: usize| Camara {
        zoom: quieta.zoom * (1.0 + i as f32 / VUELTAS as f32),
        ..quieta
    };
    for (como, cacheadas) in [("antes", false), ("ahora", true)] {
        escribir(
            &format!("[{como}] {que} quieta"),
            b.medir(escena, cacheadas, |_| quieta),
        );
        escribir(
            &format!("[{como}] {que} paneo"),
            b.medir(escena, cacheadas, paneo),
        );
        escribir(
            &format!("[{como}] {que} zoom continuo"),
            b.medir(escena, cacheadas, zoom),
        );
    }
    // A3: el mismo paneo, pero pintando SOLO cuando el colchon se agota.
    // Los fotogramas que no se pintan cuestan una matriz y un `Commit`, que
    // no se miden aquí porque no tocan ni la CPU del recorrido ni la GPU:
    // la media por fotograma es la de los repintados dividida entre los 60
    // fotogramas del guion, que es exactamente lo que siente la mano.
    let m = b.medir_por_composicion(escena, paneo);
    escribir(&format!("[A3] {que} paneo por composicion"), m.0);
    println!(
        "{:<38} {} repintados de {VUELTAS} fotogramas (colchon {} px)",
        format!("[A3] {que} paneo por composicion"),
        m.1,
        MARGEN_ESCENA
    );
}

#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn medir_fotograma_del_editor_a_3000_por_2000() {
    let b = Banco::nuevo();
    for cuantos in [200usize, 2_000, 10_000] {
        let escena = escena_sintetica(cuantos);
        medir_todo(&b, &escena, &format!("mezcla {cuantos}"));
    }
}

/// Cuantos puntos tiene el trazo que se esta dibujando en el banco de B2.
/// Un trazo largo de verdad: es donde el coste por fotograma se nota.
const PUNTOS_DEL_TRAZO: usize = 1_200;
/// Medio lado de la zona sucia de un fotograma de trazo, en pixeles de
/// pantalla: el tramo nuevo mas la holgura de la punta predicha.
const ZONA_TRAZO: i32 = 200;

/// Lo que se prepara una sola vez para medir (y para retratar) un trazo en
/// curso: la camara, el trazo, su punta y la zona sucia por los dos caminos.
struct Trazando {
    camara: Camara,
    efectiva: Camara,
    vista: (f32, f32, f32, f32),
    vivo: Elemento,
    punta: Option<Punto2>,
    /// La zona sucia en pixeles de ventana (capa de tinta) y en pixeles de
    /// la superficie de escena, que va corrida el colchon.
    zona_ventana: (i32, i32, i32, i32),
    zona_escena: (i32, i32, i32, i32),
    estampa: Estampa,
    candidatos: Vec<u64>,
}

fn trazando(escena: &Escena, rejilla: &Rejilla) -> Trazando {
    let camara = camara_de(escena, 4.0);
    let efectiva = crate::navegacion::vista_efectiva(&camara, ESCALA);
    let (w, h) = (ANCHO as f32, ALTO as f32);
    let vista = efectiva.ventana(w, h);
    // El trazo que se esta dibujando: en medio de lo que se ve, para que la
    // zona sucia caiga sobre escena de verdad y no sobre papel en blanco.
    let centro = (
        efectiva.x + w / (2.0 * efectiva.zoom),
        efectiva.y + h / (2.0 * efectiva.zoom),
    );
    let ancho_mundo = 220.0;
    let vivo = trazo(
        u64::from(u32::MAX),
        centro.0 - ancho_mundo / 2.0,
        centro.1 - 80.0,
        PUNTOS_DEL_TRAZO,
    );
    // La punta predicha, como la pone el editor: el ultimo punto adelantado.
    let punta = match &vivo.figura {
        Figura::Lapiz { puntos, .. } => puntos.last().copied(),
        _ => None,
    }
    .map(|p| Punto2::nuevo(p.x + 12.0, p.y + 6.0));
    let q = efectiva.a_pantalla(punta.unwrap_or(Punto2::nuevo(centro.0, centro.1)));
    let zona_ventana = (
        q.x as i32 - ZONA_TRAZO,
        q.y as i32 - ZONA_TRAZO,
        q.x as i32 + ZONA_TRAZO,
        q.y as i32 + ZONA_TRAZO,
    );
    let m = MARGEN_ESCENA as i32;
    Trazando {
        camara,
        efectiva,
        vista,
        punta,
        zona_ventana,
        zona_escena: (
            zona_ventana.0 + m,
            zona_ventana.1 + m,
            zona_ventana.2 + m,
            zona_ventana.3 + m,
        ),
        estampa: Estampa {
            camara: (efectiva.x, efectiva.y, efectiva.zoom),
            tamano: tamano_escena(w, h, MARGEN_ESCENA as f32),
            excluidos: vec![vivo.id],
        },
        candidatos: rejilla.candidatos(vista),
        vivo,
    }
}

fn rect_de(z: (i32, i32, i32, i32)) -> RectF {
    RectF {
        x: z.0 as f32,
        y: z.1 as f32,
        ancho: (z.2 - z.0) as f32,
        alto: (z.3 - z.1) as f32,
    }
}

/// **B2 medido.** Lo que cuesta APOYAR el lapiz y lo que cuesta cada uno de
/// los fotogramas siguientes, por los dos caminos, en la misma vuelta y con
/// la misma GPU.
///
/// - «antes»: al pulsar se hornea la capa congelada -un `ID2D1Bitmap1` del
///   tamano de la superficie de escena, 3512 x 2512 a 3000 x 2000 con
///   colchon, y toda la escena pintada dentro-, y cada fotograma copia de
///   ella la zona sucia (D148) antes de pintar el trazo encima.
/// - «B2»: al pulsar no se hornea nada -la escena ya esta en su visual y no
///   se toca- y cada fotograma limpia la zona sucia de la capa de tinta y
///   pinta el trazo ahi.
///
/// **Lo que este banco NO ve, dicho claro:** no ve lo que se ahorra al NO
/// presentar la cadena de intercambio de la escena en cada fotograma del
/// trazo, ni la recomposicion que DWM se ahorra. Eso solo se mide con la
/// ventana abierta. O sea, la diferencia de los fotogramas «siguientes» que
/// sale aqui es el SUELO de la real.
fn medir_trazo(b: &mut Banco, escena: &Escena, que: &str) {
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(escena);
    let t = trazando(escena, &rejilla);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(t.efectiva.zoom);
    let ms = |i: Instant| i.elapsed().as_secs_f64() * 1000.0;
    let vueltas_primer = 5;

    // --- Antes: apoyar el lapiz hornea la escena entera ---
    let mut capa = CapaEstatica::nueva();
    let mut hornear = 0.0;
    for _ in 0..vueltas_primer {
        capa.soltar();
        let i = Instant::now();
        hornear_capa(b, escena, &t, &mut cache, &mut cache_tinta, &mut capa);
        b.destino.esperar_gpu().expect("GPU");
        hornear += ms(i);
    }
    println!(
        "{:<44} primer fotograma {:>7.2} ms | {:>5.1} MB de video ({} x {})",
        format!("[antes] {que} apoyar el lapiz"),
        hornear / vueltas_primer as f64,
        capa.bytes() as f64 / (1024.0 * 1024.0),
        t.estampa.tamano.0,
        t.estampa.tamano.1
    );

    // --- Antes: cada fotograma copia su zona de la capa y pinta encima ---
    let mut seguir = 0.0;
    for _ in 0..VUELTAS {
        let i = Instant::now();
        capa.volcar_zona(
            &mut b.motor,
            &b.destino.destino,
            &t.estampa,
            Some(t.zona_escena),
        );
        fotograma_de_trazo(b, &t, true, false);
        b.destino.esperar_gpu().expect("GPU");
        seguir += ms(i);
    }
    println!(
        "{:<44} cada fotograma  {:>7.2} ms",
        format!("[antes] {que} trazando"),
        seguir / VUELTAS as f64
    );
    capa.soltar();

    // --- B2: apoyar el lapiz solo vacia la capa de tinta ---
    let mut encender = 0.0;
    for _ in 0..vueltas_primer {
        let i = Instant::now();
        b.motor
            .dibujar(&b.tinta.destino, |p| p.limpiar_transparente())
            .expect("vaciar la capa de tinta");
        b.tinta.esperar_gpu().expect("GPU");
        encender += ms(i);
    }
    println!(
        "{:<44} primer fotograma {:>7.2} ms | {:>5.1} MB de video ({ANCHO} x {ALTO})",
        format!("[B2] {que} apoyar el lapiz"),
        encender / vueltas_primer as f64,
        (ANCHO as f64 * ALTO as f64 * 4.0) / (1024.0 * 1024.0)
    );

    // --- B2: cada fotograma limpia su zona de la capa y pinta el trazo ---
    let mut seguir_b2 = 0.0;
    for _ in 0..VUELTAS {
        let i = Instant::now();
        fotograma_de_trazo(b, &t, false, true);
        b.tinta.esperar_gpu().expect("GPU");
        seguir_b2 += ms(i);
    }
    println!(
        "{:<44} cada fotograma  {:>7.2} ms",
        format!("[B2] {que} trazando"),
        seguir_b2 / VUELTAS as f64
    );
}

/// Hornea la capa congelada como lo hace el editor al apoyar el lapiz.
fn hornear_capa(
    b: &mut Banco,
    escena: &Escena,
    t: &Trazando,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    capa: &mut CapaEstatica,
) {
    let m = MARGEN_ESCENA as f32;
    let imagenes = &b.imagenes;
    capa.preparar(&mut b.motor, t.estampa.clone(), |p| {
        p.limpiar(Color::BLANCO);
        let origen = t.efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
        p.poner_vista((0.0, 0.0), t.efectiva.zoom, (origen.x + m, origen.y + m));
        for id in &t.candidatos {
            let Some(e) = escena.buscar(*id) else {
                continue;
            };
            if e.borrado {
                continue;
            }
            let mut indice = 0u32;
            let grano = pixpin_motor2d::pintado::grano_de(e);
            por_cada_orden(cache, e, t.efectiva.zoom, escena.escala.as_ref(), |orden| {
                dibujar_orden(
                    p,
                    orden,
                    t.vista,
                    Some((&mut *cache_tinta, (e.id, e.version, indice))),
                    imagenes,
                    t.efectiva.zoom,
                    grano,
                );
                indice += 1;
            });
        }
    })
    .expect("hornear la capa");
}

/// Un fotograma del trazo: el contorno entero y su punta, recortados a la
/// zona sucia. `en_escena` lo pinta sobre la superficie de escena (camino de
/// antes, con el colchon); si no, sobre la capa de tinta, que ademas se
/// limpia primero porque ahi no hay escena debajo que tape lo anterior.
fn fotograma_de_trazo(b: &Banco, t: &Trazando, en_escena: bool, limpiar: bool) {
    let m = if en_escena { MARGEN_ESCENA as f32 } else { 0.0 };
    let zona = if en_escena {
        t.zona_escena
    } else {
        t.zona_ventana
    };
    let destino = if en_escena {
        &b.destino.destino
    } else {
        &b.tinta.destino
    };
    b.motor
        .dibujar(destino, |p| {
            p.empujar_recorte(rect_de(zona));
            if limpiar {
                p.limpiar_transparente();
            }
            let origen = t.efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
            p.poner_vista((0.0, 0.0), t.efectiva.zoom, (origen.x + m, origen.y + m));
            for orden in pixpin_motor2d::pintado::ordenes_a_distancia(&t.vivo, t.efectiva.zoom) {
                dibujar_orden(p, &orden, t.vista, None, &b.imagenes, t.efectiva.zoom, None);
            }
            if let Some(o) = t.punta.and_then(|q| punta_de_tinta(&t.vivo, q)) {
                dibujar_orden(p, &o, t.vista, None, &b.imagenes, t.efectiva.zoom, None);
            }
            p.soltar_recorte();
        })
        .expect("fotograma de trazo");
}

#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn medir_apoyar_el_lapiz_y_trazar_a_3000_por_2000() {
    let mut b = Banco::nuevo();
    for cuantos in [200usize, 2_000, 10_000] {
        let escena = escena_sintetica(cuantos);
        medir_trazo(&mut b, &escena, &format!("mezcla {cuantos}"));
    }
}

/// **El fotograma del trazo vivo segun lo largo que sea el trazo.**
///
/// En la capa de la tinta (B2) cada fotograma recalcula el contorno ENTERO
/// del trazo (la version sube con cada punto, asi que la cache de ordenes
/// no acierta nunca) y Direct2D lo tesela entero aunque se recorte a un
/// rectangulo pequeno. Este banco separa las dos cosas -`contorno` (CPU,
/// perfect-freehand) y `pintar` (encargar mas GPU)- para 200 a 6.000
/// puntos, y dice cuanto crece cada una con el largo.
///
/// ```text
/// cargo test --release -p pixpin --bin pixpinmax medir_el_trazo_vivo -- --ignored --nocapture --test-threads=1
/// ```
#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn medir_el_trazo_vivo_segun_su_largo() {
    let b = Banco::nuevo();
    let camara = Camara::nueva();
    let efectiva = crate::navegacion::vista_efectiva(&camara, ESCALA);
    let vista = efectiva.ventana(ANCHO as f32, ALTO as f32);
    for n in [200usize, 1_000, 3_000, 6_000] {
        let mut e = trazo(1, 100.0, 100.0, n);
        let mut cache = Cache::nueva();
        let (mut contorno, mut pintar) = (0.0, 0.0);
        // La zona de la punta, como la que da el gesto: el tramo nuevo.
        let fin = match &e.figura {
            Figura::Lapiz { puntos, .. } => *puntos.last().expect("puntos"),
            _ => unreachable!(),
        };
        let q = efectiva.a_pantalla(fin);
        let zona = (
            q.x as i32 - 60,
            q.y as i32 - 60,
            q.x as i32 + 60,
            q.y as i32 + 60,
        );
        for _ in 0..VUELTAS {
            e.tocar();
            let t = Instant::now();
            let _ = cache.ordenes(&e, efectiva.zoom);
            contorno += t.elapsed().as_secs_f64() * 1000.0;
            let t = Instant::now();
            b.motor
                .dibujar(&b.tinta.destino, |p| {
                    p.empujar_recorte(rect_de(zona));
                    p.limpiar_transparente();
                    let origen = efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
                    p.poner_vista((0.0, 0.0), efectiva.zoom, (origen.x, origen.y));
                    for orden in cache.ordenes(&e, efectiva.zoom) {
                        dibujar_orden(p, orden, vista, None, &b.imagenes, efectiva.zoom, None);
                    }
                    p.soltar_recorte();
                })
                .expect("fotograma vivo");
            b.tinta.esperar_gpu().expect("GPU");
            pintar += t.elapsed().as_secs_f64() * 1000.0;
        }
        let v = VUELTAS as f64;
        println!(
            "trazo vivo de {n:>5} puntos: contorno {:>6.3} ms | pintar {:>6.3} ms | total {:>6.3} ms",
            contorno / v,
            pintar / v,
            (contorno + pintar) / v
        );
    }
}

/// Guarda en PNG lo que ve el usuario con B2: la escena en su visual, la
/// tinta en el suyo, y las dos compuestas como las compone DWM.
///
/// Es la unica forma, sin abrir la aplicacion, de comprobar que el trazo cae
/// donde tiene que caer: la capa de tinta pinta en pixeles de VENTANA y la
/// escena en pixeles de superficie (corridos el colchon), y equivocarse en
/// ese colchon se ve como un trazo 256 px desplazado. La carpeta la da
/// `PIXPIN_TRAZO_RETRATO`; sin ella no hace nada.
#[test]
#[ignore = "necesita GPU real; ejecutar con --ignored"]
fn retratar_el_trazo_en_su_capa() {
    let Some(dir) = std::env::var_os("PIXPIN_TRAZO_RETRATO").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("carpeta");
    let b = Banco::nuevo();
    let escena = escena_sintetica(200);
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(&escena);
    let t = trazando(&escena, &rejilla);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(t.efectiva.zoom);

    // La escena tal como se ve POR LA VENTANA, que es lo que ensena su
    // visual: el colchon se lo come la transformada del visual, asi que en
    // pantalla la escena y la tinta comparten el mismo origen. Si no fuera
    // asi -si `pintar_tinta_viva` le sumara el colchon-, el trazo saldria
    // 256 px corrido y este retrato lo ensenaria.
    b.fotograma(
        &escena,
        &rejilla,
        &mut cache,
        &mut cache_tinta,
        &t.camara,
        true,
    );
    b.destino.esperar_gpu().expect("GPU");
    let (w, h, escena_px) = b.destino.leer_rgba().expect("leer escena");

    // Y la capa de tinta: vacia, y con el fotograma del trazo dentro.
    b.motor
        .dibujar(&b.tinta.destino, |p| p.limpiar_transparente())
        .expect("vaciar");
    fotograma_de_trazo(&b, &t, false, true);
    b.tinta.esperar_gpu().expect("GPU");
    let (_, _, tinta_px) = b.tinta.leer_rgba().expect("leer tinta");

    // Compuestas: la tinta encima, con su alfa. Es lo que hace DWM con los
    // dos visuales, hecho aqui en CPU para poder mirarlo.
    let mut juntas = escena_px.clone();
    for (d, s) in juntas.chunks_exact_mut(4).zip(tinta_px.chunks_exact(4)) {
        let a = s[3] as f32 / 255.0;
        for k in 0..3 {
            d[k] = (s[k] as f32 * a + d[k] as f32 * (1.0 - a)).round() as u8;
        }
    }
    for (nombre, px) in [
        ("escena", escena_px),
        ("tinta", tinta_px),
        ("compuesto", juntas),
    ] {
        let png = pixpin_codec::codificar_png(&pixpin_codec::ImagenRgba {
            ancho: w,
            alto: h,
            pixeles: px,
        })
        .expect("png");
        std::fs::write(dir.join(format!("{nombre}.png")), png).expect("escribir");
    }
}

#[test]
fn la_escena_sintetica_trae_de_todo_y_la_mitad_son_trazos() {
    let e = escena_sintetica(200);
    assert_eq!(e.elementos.len(), 200);
    let cuenta = |f: fn(&Figura) -> bool| e.elementos.iter().filter(|x| f(&x.figura)).count();
    assert_eq!(cuenta(|f| matches!(f, Figura::Lapiz { .. })), 100);
    assert!(cuenta(|f| matches!(f, Figura::Texto { .. })) > 0, "textos");
    assert!(
        cuenta(|f| matches!(f, Figura::Flecha { .. })) > 0,
        "flechas"
    );
    // Caso negativo: las formas cerradas van CON relleno, que es lo caro;
    // sin esto el banco mediría rough.js sin sombreado y no diría nada.
    assert!(
        e.elementos
            .iter()
            .filter(|x| matches!(x.figura, Figura::Elipse | Figura::Rectangulo))
            .all(|x| x.relleno.is_some()),
        "las formas cerradas del banco tienen que llevar relleno"
    );
}

// ---------------------------------------------------------------------------
// **El editor al cabo de varios minutos.**
//
// El usuario lo dijo asi: «al principio todo funciona rapido; luego de un
// minuto empieza a ser lento lo de dibujar». Los bancos de arriba miden un
// fotograma con la escena ya hecha y las caches recien creadas; ninguno ve
// lo que pasa cuando la MISMA sesion lleva minutos dibujando, moviendo y
// deshaciendo. Este guion lo hace: cada «minuto» son los gestos de un minuto
// de uso -trazos a mano, rectangulos, arrastres, redimensionados, deshacer y
// rehacer- pasados por el `Gesto` de verdad, con las caches, la rejilla y el
// motor vivos toda la sesion como en `abrir_en_modo`, y al acabar cada
// minuto se apunta cuanto cuesta y cuanto ocupa todo.
//
// ```text
// cargo test --release -p pixpin --bin pixpinmax envejecer_el_editor -- --ignored --nocapture --test-threads=1
// ```
//
// `PIXPIN_MINUTOS` cambia cuantos (8 por defecto).
// ---------------------------------------------------------------------------

/// Lo que el sistema dice que ocupa este proceso: memoria privada y
/// conjunto de trabajo, en megas. Se pregunta a PowerShell porque la app no
/// escribe `unsafe` y no hace falta para una medicion.
fn memoria_del_proceso() -> (f64, f64) {
    let pid = std::process::id();
    let orden =
        format!("$p=Get-Process -Id {pid}; \"$($p.PrivateMemorySize64) $($p.WorkingSet64)\"");
    let salida = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &orden])
        .output();
    let Ok(s) = salida else { return (0.0, 0.0) };
    let texto = String::from_utf8_lossy(&s.stdout);
    let mut n = texto
        .split_whitespace()
        .map(|x| x.parse::<f64>().unwrap_or(0.0) / (1024.0 * 1024.0));
    (n.next().unwrap_or(0.0), n.next().unwrap_or(0.0))
}

/// Un azar sin dependencias: el mismo guion en cada ejecucion.
struct Dado(u64);
impl Dado {
    fn siguiente(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        ((self.0 >> 33) as f32) / (u32::MAX >> 1) as f32
    }
}

/// Lo acumulado en un minuto.
#[derive(Default)]
struct Minuto {
    movs: usize,
    gesto_ms: f64,
    gesto_peor: f64,
    vivos: usize,
    vivo_ms: f64,
    vivo_peor: f64,
    fotos: usize,
    foto_ms: f64,
    foto_peor: f64,
    /// De `foto_ms`, lo que es CPU encargando primitivas a Direct2D.
    encargar_ms: f64,
    /// Objetos de Direct2D creados en los fotogramas nitidos del minuto.
    objetos: u64,
    no_agarro: usize,
    /// Lo mas que se recorrio en un fotograma nitido: los candidatos de la
    /// rejilla, que es de lo que depende su coste.
    visibles: usize,
    /// Soltar el lapiz como ahora (`hornear_trazo`): copiar lo que difiere
    /// del mapa de delante y pintar SOLO el trazo nuevo. El «nitido» de
    /// arriba es como se soltaba antes: la escena entera.
    sueltas: usize,
    soltar_ms: f64,
    soltar_peor: f64,
    /// Empezar y acabar de arrastrar como ahora (`repintar_zona`): solo el
    /// trozo de lo elegido. El «nitido» tras mover es como se hacia antes.
    zonas: usize,
    zona_ms: f64,
    zona_peor: f64,
    /// Lo mas que se recorrio en una zona (candidatos de la rejilla en ella).
    zona_vis: usize,
    /// Zonas que no se supieron acotar (irian por la escena entera).
    zona_no: usize,
}

struct Sesion {
    dispositivo: pixpin_capture::Dispositivo,
    b: Banco,
    escena: Escena,
    gesto: Gesto,
    rejilla: Rejilla,
    cache: Cache,
    cache_tinta: pixpin_render::CacheTinta,
    camara: Camara,
    efectiva: Camara,
    dado: Dado,
    /// Si cada minuto se dibuja en otro sitio del papel, como quien recorre
    /// un plano grande: lo de antes queda fuera de la pantalla.
    panear: bool,
    /// Lo que el mapa de atras tiene distinto del de delante, como lo lleva
    /// el editor (`trasero_distinto`): `None` tras un fotograma entero.
    trasero: Option<(i32, i32, i32, i32)>,
}

impl Sesion {
    fn escala(&self) -> f32 {
        1.0 / self.efectiva.zoom
    }

    /// Un aviso del raton con lo que hace el bucle del editor detras: el
    /// gesto, y si hay trazo en curso, su fotograma en la capa de la tinta
    /// (`pintar_tinta_viva`) recortado a la zona que dice el gesto.
    fn aviso(&mut self, ev: EventoGesto, m: &mut Minuto) {
        let es_mover = matches!(ev, EventoGesto::Mover { .. });
        let escala = self.escala();
        let t = Instant::now();
        let r = self.gesto.evento(ev, &mut self.escena, escala);
        let g = t.elapsed().as_secs_f64() * 1000.0;
        if !es_mover {
            return;
        }
        m.movs += 1;
        m.gesto_ms += g;
        m.gesto_peor = m.gesto_peor.max(g);
        let Some((id, _)) = self.gesto.elemento_en_curso() else {
            return;
        };
        let Some(e) = self.escena.buscar(id) else {
            return;
        };
        let zona = match r.region {
            Region::Caja(x0, y0, x1, y1) => {
                let a = self.efectiva.a_pantalla(Punto2::nuevo(x0, y0));
                let c = self.efectiva.a_pantalla(Punto2::nuevo(x1, y1));
                (
                    a.x.min(c.x) as i32 - 8,
                    a.y.min(c.y) as i32 - 8,
                    a.x.max(c.x) as i32 + 8,
                    a.y.max(c.y) as i32 + 8,
                )
            }
            _ => (0, 0, ANCHO as i32, ALTO as i32),
        };
        let efectiva = self.efectiva;
        let vista = efectiva.ventana(ANCHO as f32, ALTO as f32);
        let t = Instant::now();
        let cache = &mut self.cache;
        let imagenes = &self.b.imagenes;
        let escala_doc = self.escena.escala.as_ref();
        self.b
            .motor
            .dibujar(&self.b.tinta.destino, |p| {
                p.empujar_recorte(rect_de(zona));
                p.limpiar_transparente();
                let origen = efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
                p.poner_vista((0.0, 0.0), efectiva.zoom, (origen.x, origen.y));
                por_cada_orden(cache, e, efectiva.zoom, escala_doc, |orden| {
                    dibujar_orden(p, orden, vista, None, imagenes, efectiva.zoom, None);
                });
                p.soltar_recorte();
            })
            .expect("fotograma vivo");
        self.b.tinta.esperar_gpu().expect("GPU");
        let v = t.elapsed().as_secs_f64() * 1000.0;
        m.vivos += 1;
        m.vivo_ms += v;
        m.vivo_peor = m.vivo_peor.max(v);
    }

    /// Soltar el lapiz como lo hace ahora el editor: `hornear_trazo` sin
    /// ventana. Copia del «mapa de delante» lo que difiere (la zona del
    /// trazo anterior, o todo tras un fotograma entero) y pinta solo el
    /// ultimo elemento en su zona. Devuelve si habia trazo que hornear.
    fn soltar(&mut self, m: &mut Minuto) -> bool {
        let Some(e) = self.escena.elementos.last() else {
            return false;
        };
        if !matches!(e.figura, Figura::Lapiz { .. }) {
            return false;
        }
        let Some(zona) = zona_del_horneado(e, &self.efectiva, 0.0, ANCHO, ALTO) else {
            return false;
        };
        let t = Instant::now();
        self.b
            .destino
            .copiar_desde(&self.b.delante, self.trasero)
            .expect("igualar el trasero");
        let (cache, cache_tinta, imagenes, escena) = (
            &mut self.cache,
            &mut self.cache_tinta,
            &self.b.imagenes,
            &self.escena,
        );
        let efectiva = self.efectiva;
        self.b
            .motor
            .dibujar(&self.b.destino.destino, |p| {
                pintar_horneado(
                    p,
                    e,
                    &efectiva,
                    0.0,
                    zona,
                    escena,
                    cache,
                    cache_tinta,
                    imagenes,
                    ANCHO as f32,
                    ALTO as f32,
                );
            })
            .expect("horneado");
        self.b.destino.esperar_gpu().expect("GPU");
        let s = t.elapsed().as_secs_f64() * 1000.0;
        m.sueltas += 1;
        m.soltar_ms += s;
        m.soltar_peor = m.soltar_peor.max(s);
        self.trasero = Some(zona);
        true
    }

    /// Empezar y acabar de arrastrar como lo hace ahora el editor
    /// (`repintar_zona` sin ventana): al empezar, rehacer el trozo de donde
    /// estaba lo elegido sin el (va en la capa); al acabar, el trozo de donde
    /// quedo, con el y su marco. Se mide tras soltar, con lo elegido ya en su
    /// sitio nuevo: `atras` es lo que se movio, al reves.
    fn arrastre_por_zona(&mut self, m: &mut Minuto, atras: (f32, f32)) {
        self.rejilla.sincronizar(&self.escena);
        for (fuera, atras) in [(true, atras), (false, (0.0, 0.0))] {
            let Some(zv) = zona_de_seleccion(
                &self.escena,
                &self.gesto,
                &self.efectiva,
                &mut self.cache,
                atras,
            ) else {
                m.zona_no += 1;
                continue;
            };
            let Some(zona) = zona_en_superficie(zv, 0.0, ANCHO, ALTO) else {
                continue;
            };
            let t = Instant::now();
            self.b
                .destino
                .copiar_desde(&self.b.delante, self.trasero)
                .expect("igualar el trasero");
            let (cache, cache_tinta, imagenes, escena, gesto, rejilla) = (
                &mut self.cache,
                &mut self.cache_tinta,
                &self.b.imagenes,
                &self.escena,
                &self.gesto,
                &self.rejilla,
            );
            let efectiva = self.efectiva;
            let mut visibles = 0;
            self.b
                .motor
                .dibujar(&self.b.destino.destino, |p| {
                    visibles = pintar_zona(
                        p,
                        escena,
                        &efectiva,
                        gesto,
                        rejilla,
                        cache,
                        cache_tinta,
                        imagenes,
                        None,
                        None,
                        ESCALA,
                        0.0,
                        zona,
                        fuera,
                        ANCHO as f32,
                        ALTO as f32,
                    );
                })
                .expect("zona");
            self.b.destino.esperar_gpu().expect("GPU");
            let z = t.elapsed().as_secs_f64() * 1000.0;
            m.zonas += 1;
            m.zona_ms += z;
            m.zona_peor = m.zona_peor.max(z);
            m.zona_vis = m.zona_vis.max(visibles as usize);
            self.trasero = Some(zona);
        }
    }

    /// El fotograma nitido de despues de un gesto: rejilla al dia y la
    /// escena entera, con las caches de toda la sesion.
    ///
    /// Tras un trazo mide como se soltaba ANTES; el editor ya no lo pinta,
    /// asi que no olvida lo que el mapa de atras tenia distinto
    /// (`olvidar_trasero` a `false`). Tras cualquier otro gesto si lo pinta.
    fn fotograma_nitido(&mut self, m: &mut Minuto, olvidar_trasero: bool) {
        if olvidar_trasero {
            self.trasero = None;
        }
        let t = Instant::now();
        self.rejilla.sincronizar(&self.escena);
        let (_, encargar, _, objetos) = self.b.fotograma(
            &self.escena,
            &self.rejilla,
            &mut self.cache,
            &mut self.cache_tinta,
            &self.camara,
            true,
        );
        let f = t.elapsed().as_secs_f64() * 1000.0;
        let vista = self.efectiva.ventana(ANCHO as f32, ALTO as f32);
        m.visibles = m.visibles.max(self.rejilla.candidatos(vista).len());
        m.encargar_ms += encargar;
        m.objetos += objetos as u64;
        m.fotos += 1;
        m.foto_ms += f;
        m.foto_peor = m.foto_peor.max(f);
    }

    fn punto_al_azar(&mut self) -> Punto2 {
        let v = self.efectiva.ventana(ANCHO as f32, ALTO as f32);
        let (w, h) = (v.2 - v.0, v.3 - v.1);
        Punto2::nuevo(
            v.0 + w * (0.05 + 0.8 * self.dado.siguiente()),
            v.1 + h * (0.05 + 0.8 * self.dado.siguiente()),
        )
    }

    fn arrastre(&mut self, desde: Punto2, pasos: usize, d: (f32, f32), m: &mut Minuto) {
        self.aviso(
            EventoGesto::Pulsar {
                p: desde,
                shift: false,
                alt: false,
                presion: None,
            },
            m,
        );
        for i in 1..=pasos {
            let t = i as f32 / pasos as f32;
            // Un temblor para que el trazo no sea una recta perfecta.
            let q = Punto2::nuevo(
                desde.x + d.0 * t + 15.0 * (t * 17.0).sin(),
                desde.y + d.1 * t + 15.0 * (t * 11.0).cos(),
            );
            self.aviso(
                EventoGesto::Mover {
                    p: q,
                    shift: false,
                    alt: false,
                    presion: Some(0.5),
                },
                m,
            );
        }
        let fin = Punto2::nuevo(desde.x + d.0, desde.y + d.1);
        self.aviso(EventoGesto::Soltar { p: fin }, m);
        let era_trazo = self.soltar(m);
        // Con `PIXPIN_SIN_ANTES` no se mide como se soltaba antes (la escena
        // entera tras cada trazo), que es lo que hace el editor ahora: asi la
        // memoria del informe es la de una sesion de verdad y no la del
        // banco midiendo el camino viejo.
        if !(era_trazo && std::env::var_os("PIXPIN_SIN_ANTES").is_some()) {
            self.fotograma_nitido(m, !era_trazo);
        }
    }

    /// Un minuto de uso: lo que hace alguien que dibuja un plano.
    ///
    /// Con `dibujar` a `false` no nace nada: solo se mueve, se redimensiona y
    /// se deshace lo que ya hay. Es lo que separa «crece porque hay mas
    /// dibujo» de «crece porque pasa el tiempo».
    fn un_minuto(&mut self, dibujar: bool) -> Minuto {
        let mut m = Minuto::default();
        if self.panear && dibujar {
            let v = self.efectiva.ventana(ANCHO as f32, ALTO as f32);
            self.camara.x += v.2 - v.0;
            self.efectiva = crate::navegacion::vista_efectiva(&self.camara, ESCALA);
        }
        let (trazos, cajas) = if dibujar { (20, 4) } else { (0, 0) };
        elegir_herramienta(&mut self.gesto, Herramienta::Lapiz);
        for _ in 0..trazos {
            let p = self.punto_al_azar();
            let d = (
                100.0 + 200.0 * self.dado.siguiente(),
                60.0 * self.dado.siguiente(),
            );
            self.arrastre(p, 150, d, &mut m);
        }
        elegir_herramienta(&mut self.gesto, Herramienta::Rectangulo);
        for _ in 0..cajas {
            let p = self.punto_al_azar();
            self.arrastre(p, 30, (180.0, 120.0), &mut m);
        }
        // Arrastrar lo ya dibujado: se agarra por un punto de su trazo.
        elegir_herramienta(&mut self.gesto, Herramienta::Mano);
        for k in 0..8 {
            let vivos: Vec<Punto2> = self
                .escena
                .visibles()
                .filter_map(|e| match &e.figura {
                    Figura::Lapiz { puntos, .. } => puntos.get(puntos.len() / 2).copied(),
                    _ => None,
                })
                .collect();
            if vivos.is_empty() {
                break;
            }
            let p = vivos[(k * 7919 + vivos.len() / 2) % vivos.len()];
            self.gesto.seleccion.limpiar();
            self.aviso(
                EventoGesto::Pulsar {
                    p,
                    shift: false,
                    alt: false,
                    presion: None,
                },
                &mut m,
            );
            if self.gesto.seleccion.ids().is_empty() {
                m.no_agarro += 1;
            }
            for i in 1..=40 {
                let q = Punto2::nuevo(p.x + i as f32 * 2.0, p.y + i as f32);
                self.aviso(
                    EventoGesto::Mover {
                        p: q,
                        shift: false,
                        alt: false,
                        presion: None,
                    },
                    &mut m,
                );
            }
            self.aviso(
                EventoGesto::Soltar {
                    p: Punto2::nuevo(p.x + 80.0, p.y + 40.0),
                },
                &mut m,
            );
            // Ahora: dos trozos (empezar y acabar). Antes: dos fotogramas
            // enteros como el nitido que sigue.
            self.arrastre_por_zona(&mut m, (-80.0, -40.0));
            self.fotograma_nitido(&mut m, true);
            // Y redimensionar lo recien movido por su esquina sureste.
            if k % 3 == 0
                && let Some(t) = self.gesto.tiradores(&self.escena, self.escala())
            {
                let q = t.tamano[4].1;
                self.arrastre(q, 30, (40.0, 30.0), &mut m);
            }
        }
        for _ in 0..3 {
            self.aviso(EventoGesto::Deshacer, &mut m);
            self.fotograma_nitido(&mut m, true);
        }
        for _ in 0..2 {
            self.aviso(EventoGesto::Rehacer, &mut m);
            self.fotograma_nitido(&mut m, true);
        }
        m
    }
}

#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn envejecer_el_editor_varios_minutos() {
    let minutos: usize = std::env::var("PIXPIN_MINUTOS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    // A partir de aqui ya no nace nada: solo se mueve y se deshace. Por
    // defecto, la mitad de los minutos.
    let dibujar_hasta: usize = std::env::var("PIXPIN_DIBUJAR_HASTA")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(minutos / 2);
    let dispositivo = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let camara = Camara::nueva();
    let efectiva = crate::navegacion::vista_efectiva(&camara, ESCALA);
    let mut s = Sesion {
        dispositivo,
        b: Banco::nuevo(),
        escena: Escena::nueva(),
        gesto: gesto_inicial(pixpin_motor2d::enganche::Ajustes::default()),
        rejilla: Rejilla::nueva(),
        cache: Cache::nueva(),
        cache_tinta: pixpin_render::CacheTinta::nueva(),
        camara,
        efectiva,
        dado: Dado(12345),
        panear: std::env::var_os("PIXPIN_QUIETO").is_none(),
        trasero: None,
    };
    s.cache_tinta.fijar_escala(efectiva.zoom);
    // «nitido» es lo que costaba SOLTAR el lapiz antes (la escena entera,
    // `vis` candidatos) y lo que sigue costando tras mover o deshacer;
    // «soltar» es lo que cuesta ahora (`hornear_trazo`).
    println!(
        "min | elem |  vis | gesto ms (peor) | vivo ms (peor) | nitido ms (peor) cpu obj | soltar ms (peor) | arrastre zona ms (peor) vis no | cache | tinta (peso) | silu | hist KB | celdas | priv MB | ws MB | video MB | no agarro"
    );
    for minuto in 1..=minutos {
        let m = s.un_minuto(minuto <= dibujar_hasta);
        let (privada, ws) = memoria_del_proceso();
        let video = pixpin_render::fuera_de_pantalla::memoria_de_video(s.dispositivo.d3d())
            .map_or(0.0, |(l, n)| (l + n) as f64 / (1024.0 * 1024.0));
        println!(
            "{minuto:>3} | {:>4} | {:>4} | {:>6.3} ({:>6.2}) | {:>6.3} ({:>6.2}) | {:>7.2} ({:>7.2}) {:>6.2} {:>5.0} | {:>6.3} ({:>6.2}) | {:>6.3} ({:>6.2}) {:>4} {:>2} | {:>5} | {:>5} ({:>7}) | {:>4} | {:>7} | {:>6} | {:>7.1} | {:>5.1} | {:>8.1} | {}",
            s.escena.elementos.len(),
            m.visibles,
            m.gesto_ms / m.movs.max(1) as f64,
            m.gesto_peor,
            m.vivo_ms / m.vivos.max(1) as f64,
            m.vivo_peor,
            m.foto_ms / m.fotos.max(1) as f64,
            m.foto_peor,
            m.encargar_ms / m.fotos.max(1) as f64,
            m.objetos as f64 / m.fotos.max(1) as f64,
            m.soltar_ms / m.sueltas.max(1) as f64,
            m.soltar_peor,
            m.zona_ms / m.zonas.max(1) as f64,
            m.zona_peor,
            m.zona_vis,
            m.zona_no,
            s.cache.cuantos(),
            s.cache_tinta.cuantas(),
            peso_de(&s.cache_tinta),
            s.cache_tinta.grano.cuantas_siluetas(),
            s.escena.bytes_de_historial() / 1024,
            s.rejilla.cuantas_celdas(),
            privada,
            ws,
            video,
            m.no_agarro,
        );
    }
    // De quien es lo que ocupa: se vacia cada cache por separado y se mira
    // cuanto baja.
    let memoria = |s: &Sesion| {
        let (p, _) = memoria_del_proceso();
        let v = pixpin_render::fuera_de_pantalla::memoria_de_video(s.dispositivo.d3d())
            .map_or(0.0, |(l, n)| (l + n) as f64 / (1024.0 * 1024.0));
        (p, v)
    };
    let vaciar_gpu = |s: &Sesion| {
        s.b.motor
            .dibujar(&s.b.destino.destino, |p| p.limpiar(Color::BLANCO))
            .expect("vacio");
        s.b.destino.esperar_gpu().expect("GPU");
    };
    let (p0, v0) = memoria(&s);
    s.cache_tinta.vaciar();
    vaciar_gpu(&s);
    let (p1, v1) = memoria(&s);
    s.cache.vaciar();
    vaciar_gpu(&s);
    let (p2, v2) = memoria(&s);
    println!(
        "vaciar realizaciones: priv {p0:.1} -> {p1:.1} MB, video {v0:.1} -> {v1:.1} MB | vaciar ordenes: priv -> {p2:.1} MB, video -> {v2:.1} MB"
    );
    // Lo que hace el editor al perder el primer plano: devolver la reserva
    // de Direct2D y del controlador (`MotorRender::devolver_memoria`).
    s.b.motor.devolver_memoria(s.b.dispositivo.d3d());
    let (p3, v3) = memoria(&s);
    println!("devolver la reserva (Trim): priv -> {p3:.1} MB, video -> {v3:.1} MB");
}

/// Lo que pesa lo realizado, en segmentos (`CacheTinta::peso`).
fn peso_de(c: &pixpin_render::CacheTinta) -> u64 {
    c.peso()
}

// ---------------------------------------------------------------------------
// **Puertas de «al rato va lento»**: soltar el lapiz y empezar/acabar de
// arrastrar tienen que costar lo que mide lo que cambia, no lo que hay a la
// vista. Necesitan GPU; son puertas, se ejecutan en `--release`:
//
// ```text
// cargo test --release -p pixpin --bin pixpinmax cuesta_lo_mismo -- --ignored --nocapture --test-threads=1
// ```
//
// Cada una mide el mismo gesto sobre una escena de 50 y otra de 2.000
// elementos con la MISMA camara (todo a la vista en la grande) y la misma
// densidad, y pide que el grande no cueste mas del triple que el chico. Si
// volviera a pintarse la escena entera, el grande costaria como el
// fotograma entero, que se mide al lado para que se vea la diferencia.
// ---------------------------------------------------------------------------

/// Lo mas rapido de `n` vueltas: la puerta compara costes, no ruido.
fn lo_mejor_de(n: usize, mut f: impl FnMut() -> f64) -> f64 {
    (0..n).map(|_| f()).fold(f64::MAX, f64::min)
}

/// Una escena de `cuantos` con la camara que encaja la de 2.000 (la misma
/// para las dos), y ya pintada una vez: las caches tibias, como en el
/// editor al cabo de un rato.
fn escena_para_puerta(
    b: &Banco,
    cuantos: usize,
) -> (
    Escena,
    Rejilla,
    Cache,
    pixpin_render::CacheTinta,
    Camara,
    Camara,
) {
    let camara = camara_de(&escena_sintetica(2_000), 1.0);
    let efectiva = crate::navegacion::vista_efectiva(&camara, ESCALA);
    let escena = escena_sintetica(cuantos);
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(&escena);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(efectiva.zoom);
    b.fotograma(
        &escena,
        &rejilla,
        &mut cache,
        &mut cache_tinta,
        &camara,
        true,
    );
    (escena, rejilla, cache, cache_tinta, camara, efectiva)
}

/// La escena entera, lo mejor de cinco: lo que costaba antes cada gesto.
fn escena_entera(
    b: &Banco,
    escena: &Escena,
    rejilla: &Rejilla,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    camara: &Camara,
) -> f64 {
    lo_mejor_de(5, || {
        b.fotograma(escena, rejilla, cache, cache_tinta, camara, true)
            .2
    })
}

#[test]
#[ignore = "necesita GPU real; puerta: ejecutar en --release con --ignored"]
fn soltar_el_lapiz_cuesta_lo_mismo_con_dos_mil_elementos_a_la_vista_que_con_cincuenta() {
    let b = Banco::nuevo();
    let medir = |cuantos: usize| -> (f64, f64) {
        let (mut escena, mut rejilla, mut cache, mut cache_tinta, camara, efectiva) =
            escena_para_puerta(&b, cuantos);
        // El trazo nuevo, en el mismo sitio del mundo en las dos escenas.
        let id = escena.siguiente_id;
        escena.elementos.push(trazo(id, 300.0, 300.0, 150));
        escena.siguiente_id += 1;
        rejilla.sincronizar(&escena);
        let soltar = lo_mejor_de(20, || {
            // Cada suelta es un trazo nuevo: version nueva, sin teselar.
            escena.elementos.last_mut().expect("trazo").tocar();
            let e = escena.elementos.last().expect("trazo");
            let zona = zona_del_horneado(e, &efectiva, 0.0, ANCHO, ALTO).expect("se ve");
            let t = Instant::now();
            b.motor
                .dibujar(&b.destino.destino, |p| {
                    pintar_horneado(
                        p,
                        e,
                        &efectiva,
                        0.0,
                        zona,
                        &escena,
                        &mut cache,
                        &mut cache_tinta,
                        &b.imagenes,
                        ANCHO as f32,
                        ALTO as f32,
                    );
                })
                .expect("horneado");
            b.destino.esperar_gpu().expect("GPU");
            t.elapsed().as_secs_f64() * 1000.0
        });
        let entera = escena_entera(&b, &escena, &rejilla, &mut cache, &mut cache_tinta, &camara);
        (soltar, entera)
    };
    let (chico, entera_chica) = medir(50);
    let (grande, entera_grande) = medir(2_000);
    println!(
        "soltar el lapiz: {chico:.3} ms con 50, {grande:.3} ms con 2000 | la escena entera: {entera_chica:.3} y {entera_grande:.3} ms"
    );
    assert!(
        grande <= chico * 3.0 + 0.5,
        "soltar el lapiz crece con lo que hay a la vista: {chico:.3} ms con 50 y {grande:.3} ms con 2000"
    );
    // Que la puerta de arriba sabe fallar: la escena entera si crece.
    assert!(
        entera_grande > entera_chica * 3.0,
        "la escena entera no crece: la puerta no distinguiria ({entera_chica:.3} / {entera_grande:.3} ms)"
    );
}

#[test]
#[ignore = "necesita GPU real; puerta: ejecutar en --release con --ignored"]
fn empezar_y_acabar_de_arrastrar_cuesta_lo_mismo_con_dos_mil_elementos_a_la_vista_que_con_cincuenta()
 {
    let b = Banco::nuevo();
    let medir = |cuantos: usize| -> (f64, usize, f64) {
        let (escena, rejilla, mut cache, mut cache_tinta, camara, efectiva) =
            escena_para_puerta(&b, cuantos);
        // Un trazo de la esquina, que existe en las dos escenas.
        let mut gesto = gesto_inicial(pixpin_motor2d::enganche::Ajustes::default());
        gesto.seleccion.poner(escena.elementos[0].id);
        let mut visibles = 0;
        let mut zonas = 0;
        let mut costo = 0.0;
        for (fuera, atras) in [(true, (-80.0, -40.0)), (false, (0.0, 0.0))] {
            let zv = zona_de_seleccion(&escena, &gesto, &efectiva, &mut cache, atras)
                .expect("un trazo se sabe acotar");
            let zona = zona_en_superficie(zv, 0.0, ANCHO, ALTO).expect("se ve");
            costo += lo_mejor_de(20, || {
                let t = Instant::now();
                b.motor
                    .dibujar(&b.destino.destino, |p| {
                        visibles = pintar_zona(
                            p,
                            &escena,
                            &efectiva,
                            &gesto,
                            &rejilla,
                            &mut cache,
                            &mut cache_tinta,
                            &b.imagenes,
                            None,
                            None,
                            ESCALA,
                            0.0,
                            zona,
                            fuera,
                            ANCHO as f32,
                            ALTO as f32,
                        ) as usize;
                    })
                    .expect("zona");
                b.destino.esperar_gpu().expect("GPU");
                t.elapsed().as_secs_f64() * 1000.0
            });
            zonas = zonas.max(visibles);
        }
        let entera = escena_entera(&b, &escena, &rejilla, &mut cache, &mut cache_tinta, &camara);
        (costo, zonas, 2.0 * entera)
    };
    let (chico, vis_chico, antes_chico) = medir(50);
    let (grande, vis_grande, antes_grande) = medir(2_000);
    println!(
        "empezar + acabar de arrastrar: {chico:.3} ms ({vis_chico} en la zona) con 50, {grande:.3} ms ({vis_grande}) con 2000 | antes (dos escenas enteras): {antes_chico:.3} y {antes_grande:.3} ms"
    );
    assert!(
        vis_grande <= vis_chico * 3 + 4,
        "la zona recorre lo que hay a la vista: {vis_chico} con 50 y {vis_grande} con 2000"
    );
    assert!(
        grande <= chico * 3.0 + 0.5,
        "arrastrar crece con lo que hay a la vista: {chico:.3} ms con 50 y {grande:.3} ms con 2000"
    );
}

/// **De donde sale la memoria que crece al dibujar** (`envejecer_...`: casi
/// un mega por trazo, que ni vaciar las caches ni `Trim` devuelven). Cada
/// fase repite MUCHAS veces una sola cosa del editor y dice cuanto crecio el
/// proceso y la memoria de video: la que crezca es la que retiene.
///
/// ```text
/// cargo test --release -p pixpin --bin pixpinmax de_donde_sale_la_memoria -- --ignored --nocapture --test-threads=1
/// ```
#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn de_donde_sale_la_memoria_que_crece_al_dibujar() {
    let b = Banco::nuevo();
    let memoria = |b: &Banco| {
        let (p, _) = memoria_del_proceso();
        let v = pixpin_render::fuera_de_pantalla::memoria_de_video(b.dispositivo.d3d())
            .map_or(0.0, |(l, n)| (l + n) as f64 / (1024.0 * 1024.0));
        (p, v)
    };
    let fase = |nombre: &str, antes: (f64, f64), despues: (f64, f64)| {
        println!(
            "{nombre:<46} priv {:>+7.1} MB | video {:>+7.1} MB",
            despues.0 - antes.0,
            despues.1 - antes.1
        );
    };
    let camara = Camara::nueva();
    let efectiva = crate::navegacion::vista_efectiva(&camara, ESCALA);
    let vista = efectiva.ventana(ANCHO as f32, ALTO as f32);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(efectiva.zoom);
    let m0 = memoria(&b);

    // 1. El trazo vivo sin cache (`pintar_tinta_viva`): 3.000 avisos.
    let mut e = trazo(1, 100.0, 100.0, 150);
    for _ in 0..3_000 {
        e.tocar();
        b.motor
            .dibujar(&b.tinta.destino, |p| {
                p.limpiar_transparente();
                let origen = efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
                p.poner_vista((0.0, 0.0), efectiva.zoom, (origen.x, origen.y));
                por_cada_orden(&mut cache, &e, efectiva.zoom, None, |orden| {
                    dibujar_orden(p, orden, vista, None, &b.imagenes, efectiva.zoom, None);
                });
            })
            .expect("vivo");
    }
    b.tinta.esperar_gpu().expect("GPU");
    let m1 = memoria(&b);
    fase("3000 fotogramas del trazo vivo (sin cache)", m0, m1);

    // 2. Hornear trazos nuevos con cache (`pintar_horneado`): 400 trazos
    // distintos, como 400 sueltas del lapiz.
    let mut escena = Escena::nueva();
    for i in 0..400u64 {
        let t = trazo(
            i + 10,
            50.0 + (i % 20) as f32 * 90.0,
            50.0 + (i / 20) as f32 * 60.0,
            150,
        );
        escena.elementos.push(t);
        let e = escena.elementos.last().expect("trazo");
        let Some(zona) = zona_del_horneado(e, &efectiva, 0.0, ANCHO, ALTO) else {
            continue;
        };
        b.motor
            .dibujar(&b.destino.destino, |p| {
                pintar_horneado(
                    p,
                    e,
                    &efectiva,
                    0.0,
                    zona,
                    &escena,
                    &mut cache,
                    &mut cache_tinta,
                    &b.imagenes,
                    ANCHO as f32,
                    ALTO as f32,
                );
            })
            .expect("horneado");
    }
    b.destino.esperar_gpu().expect("GPU");
    let m2 = memoria(&b);
    fase("400 trazos horneados (con cache)", m1, m2);

    // 3. Vaciar las dos caches y devolver la reserva.
    cache_tinta.vaciar();
    cache.vaciar();
    b.motor
        .dibujar(&b.destino.destino, |p| p.limpiar(Color::BLANCO))
        .expect("vacio");
    b.destino.esperar_gpu().expect("GPU");
    let m3 = memoria(&b);
    fase("vaciar caches", m2, m3);
    b.motor.devolver_memoria(b.dispositivo.d3d());
    let m4 = memoria(&b);
    fase("devolver la reserva (Trim)", m3, m4);

    // 4. Fotogramas enteros de esos 400, moviendo uno cada vez (version
    // nueva: realizacion nueva), como arrastrar y soltar muchas veces.
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(&escena);
    for _ in 0..300usize {
        b.fotograma(
            &escena,
            &rejilla,
            &mut cache,
            &mut cache_tinta,
            &camara,
            true,
        );
    }
    let m4a = memoria(&b);
    fase("300 escenas enteras sin mover nada", m4, m4a);
    for k in 0..300usize {
        let i = k % escena.elementos.len();
        escena.elementos[i].mover(1.0, 0.0);
        rejilla.sincronizar(&escena);
        b.fotograma(
            &escena,
            &rejilla,
            &mut cache,
            &mut cache_tinta,
            &camara,
            true,
        );
    }
    let m5 = memoria(&b);
    fase("300 escenas enteras moviendo uno cada vez", m4a, m5);
    // La misma escena entera sin cache de realizaciones: si crece igual, no
    // es la cache; es el pintado de la tinta.
    let mut sin = pixpin_render::CacheTinta::nueva();
    sin.fijar_escala(efectiva.zoom);
    for _ in 0..100usize {
        sin.vaciar();
        b.fotograma(&escena, &rejilla, &mut cache, &mut sin, &camara, true);
    }
    drop(sin);
    let m5b = memoria(&b);
    fase("100 escenas enteras teselando todo de nuevo", m5, m5b);
    let m5 = m5b;
    cache_tinta.vaciar();
    cache.vaciar();
    b.motor.devolver_memoria(b.dispositivo.d3d());
    let m6 = memoria(&b);
    fase("vaciar y devolver otra vez", m5, m6);
    println!(
        "total: priv {:.1} -> {:.1} MB, video {:.1} -> {:.1} MB",
        m0.0, m6.0, m0.1, m6.1
    );
}

// ---------------------------------------------------------------------------
// El anotador de pantalla en dos monitores
//
// ```text
// cargo test --release -p pixpin --bin pixpinmax anotador_de_pantalla -- --ignored --nocapture --test-threads=1
// ```
// ---------------------------------------------------------------------------

/// Lo que se mide de un anotador de `w` x `h`.
struct MedidaAnotador {
    /// Cada fotograma del trazo en la capa de la tinta: media, p95, peor.
    tinta: (f64, f64, f64),
    /// Un fotograma entero de la escena con su present: media, p95, peor.
    entero: (f64, f64, f64),
    entero_congelado: (f64, f64, f64),
    /// Memoria privada y de video, en MB, por encima de la de antes de abrir:
    /// con la superficie, con la tinta, con el primer fotograma y con la
    /// foto de la congelada.
    memoria: Vec<(&'static str, f64, f64)>,
}

fn resumen(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.total_cmp(b));
    let media = v.iter().sum::<f64>() / v.len() as f64;
    (media, v[v.len() * 95 / 100], *v.last().unwrap())
}

/// El anotador abierto a `w` x `h` con sus superficies DE VERDAD, la ventana
/// creada pero SIN ensenar. Los fotogramas van al ritmo de la pantalla (uno
/// cada 16 ms), como en el bucle del editor: sin ritmo, DirectComposition
/// frena la capa de la tinta cuando se le pide mas de un `Commit` por
/// composicion, y lo que se mediria seria esa espera y no el pintado.
fn medir_anotador(w: u32, h: u32) -> MedidaAnotador {
    use crate::dibujo::mano::Mano;
    use crate::dibujo::permitidas::{self, Anfitrion};
    use pixpin_motor2d::gesto::EventoGesto;
    let ritmo = std::time::Duration::from_millis(16);
    let mb = |v: Option<(u64, u64)>| v.map_or(0.0, |(l, n)| (l + n) as f64 / (1024.0 * 1024.0));

    let dispositivo = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let mut motor = MotorRender::nuevo(dispositivo.d3d()).expect("motor");
    let (priv0, _) = memoria_del_proceso();
    let video0 = mb(pixpin_render::fuera_de_pantalla::memoria_de_video(
        dispositivo.d3d(),
    ));
    let mut memoria = Vec::new();
    let mut apuntar = |que: &'static str, d: &pixpin_capture::Dispositivo| {
        let (p, _) = memoria_del_proceso();
        let v = mb(pixpin_render::fuera_de_pantalla::memoria_de_video(d.d3d()));
        memoria.push((que, p - priv0, v - video0));
    };

    let area = pixpin_geom::Rect {
        x: 0,
        y: 0,
        ancho: w,
        alto: h,
    };
    // La ventana existe (sin ella no hay composicion) pero no se ensena.
    let ventana = VentanaOverlay::nueva(area).expect("ventana sin ensenar");
    let superficie =
        Superficie::nueva_con_capas(&motor, dispositivo.d3d(), ventana.handle(), w, h, 1)
            .expect("superficie con capas");
    apuntar("superficie", &dispositivo);
    superficie.encender_tinta(&motor).expect("capa de tinta");
    apuntar("+ capa de tinta", &dispositivo);

    // Lo anotado: sesenta trazos repartidos por la pantalla.
    let mut escena = Escena::nueva();
    escena.fondo = pantalla::papel(pantalla::Modo::Viva);
    for i in 0..60u64 {
        let x = (i % 15) as f32 * (w as f32 / 15.0);
        escena.anadir(trazo(i + 1, x, (i / 15) as f32 * 250.0, 120));
    }
    let camara = pantalla::camara_quieta(100);
    let efectiva = crate::navegacion::vista_efectiva(&camara, 100);
    let mut gesto = Gesto::nuevo();
    let mut mano = Mano::con_tinta(Anfitrion::PantallaViva, Default::default());
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    let mut rejilla = Rejilla::nueva();
    let capa = CapaEstatica::nueva();
    let mut imagenes = ImagenesLienzo::nuevo(motor.lado_maximo_bitmap());
    let caja =
        CajaHerramientas::barra_superior(area, 100, permitidas::botones(Anfitrion::PantallaViva));

    let entero = |motor: &mut MotorRender,
                  fondo: &mut Option<FondoLienzo>,
                  gesto: &Gesto,
                  escena: &Escena,
                  rejilla: &mut Rejilla,
                  cache: &mut Cache,
                  cache_tinta: &mut pixpin_render::CacheTinta,
                  imagenes: &mut ImagenesLienzo| {
        rejilla.sincronizar(escena);
        let t = Instant::now();
        pintar(
            motor,
            &superficie,
            escena,
            &efectiva,
            gesto,
            cache,
            cache_tinta,
            rejilla,
            &capa,
            fondo,
            imagenes,
            &caja,
            None,
            (0.0, 0.0),
            true,
            100,
            w as f32,
            h as f32,
            None,
            None,
            None,
            true,
            FueraDeLaEscena::default(),
            None,
            |_, _| {},
        )
        .expect("fotograma");
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        std::thread::sleep(ritmo);
        ms
    };
    let mut sin_fondo: Option<FondoLienzo> = None;
    let mut enteros: Vec<f64> = (0..20)
        .map(|_| {
            entero(
                &mut motor,
                &mut sin_fondo,
                &gesto,
                &escena,
                &mut rejilla,
                &mut cache,
                &mut cache_tinta,
                &mut imagenes,
            )
        })
        .collect();
    apuntar("+ escena e interfaz pintadas (viva)", &dispositivo);
    // Lo que Direct2D y el controlador guardan de reserva y el editor ya
    // devuelve al perder el primer plano (`devolver_memoria`).
    motor.devolver_memoria(dispositivo.d3d());
    apuntar("  tras devolver la reserva", &dispositivo);

    // La tinta viva: un trazo que cruza la pantalla, tres puntos por
    // fotograma, cada fotograma en la capa de la tinta (B2).
    let mut p = Punto2::nuevo(w as f32 / 2.0 - 200.0, 500.0);
    mano.al_motor(
        EventoGesto::Pulsar {
            p,
            shift: false,
            alt: false,
            presion: None,
        },
        &mut gesto,
        &mut escena,
        &efectiva,
    );
    let mut tinta = Vec::new();
    for k in 0..240 {
        for _ in 0..3 {
            p = Punto2::nuevo(p.x + 2.5, 500.0 + (k as f32 * 0.15).sin() * 120.0);
            mano.al_motor(
                EventoGesto::Mover {
                    p,
                    shift: false,
                    alt: false,
                    presion: None,
                },
                &mut gesto,
                &mut escena,
                &efectiva,
            );
        }
        let q = efectiva.a_pantalla(p);
        let zona = Some((
            q.x as i32 - 120,
            q.y as i32 - 160,
            q.x as i32 + 120,
            q.y as i32 + 160,
        ));
        let prediccion = prediccion_de(&gesto, &mut mano.predictor, false, efectiva.zoom);
        let t = Instant::now();
        assert!(pintar_tinta_viva(
            &motor,
            &superficie,
            &escena,
            &efectiva,
            &gesto,
            &mut cache,
            &imagenes,
            zona,
            prediccion,
            w as f32,
            h as f32,
        ));
        tinta.push(t.elapsed().as_secs_f64() * 1000.0);
        std::thread::sleep(ritmo);
    }

    // Congelada: la foto del escritorio de fondo.
    let foto = pixpin_codec::ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: vec![128u8; (w * h * 4) as usize],
    };
    let mut con_fondo = Some(FondoLienzo::nuevo(
        crate::fondo_lienzo::Fuente::from(foto),
        motor.lado_maximo_bitmap(),
    ));
    let mut congelados: Vec<f64> = (0..20)
        .map(|_| {
            entero(
                &mut motor,
                &mut con_fondo,
                &gesto,
                &escena,
                &mut rejilla,
                &mut cache,
                &mut cache_tinta,
                &mut imagenes,
            )
        })
        .collect();
    apuntar("+ foto de la congelada", &dispositivo);
    drop(ventana);
    MedidaAnotador {
        tinta: resumen(&mut tinta),
        entero: resumen(&mut enteros),
        entero_congelado: resumen(&mut congelados),
        memoria,
    }
}

/// **El anotador de pantalla medido**: un monitor de 1920 x 1080 (lo mismo
/// que el lienzo a pantalla completa) y dos (3840 x 1080), lo que cuesta
/// cada fotograma del trazo (la latencia de la tinta: el mismo camino B2
/// que el lienzo), el fotograma entero (el que se paga al soltar si no se
/// puede hornear) y la memoria del anotador abierto, viva y congelada.
#[test]
#[ignore = "necesita GPU real y sesion de escritorio; ejecutar en --release con --ignored --nocapture"]
fn medir_el_anotador_de_pantalla_en_uno_y_dos_monitores() {
    for (que, w, h) in [("1 monitor", 1920u32, 1080u32), ("2 monitores", 3840, 1080)] {
        let m = medir_anotador(w, h);
        println!("anotador de pantalla, {que} ({w} x {h}), 60 trazos:");
        let (a, b, c) = m.tinta;
        println!("  tinta viva (B2), por fotograma: media {a:.2} ms | p95 {b:.2} | peor {c:.2}");
        let (a, b, c) = m.entero;
        println!("  escena entera, viva:            media {a:.2} ms | p95 {b:.2} | peor {c:.2}");
        let (a, b, c) = m.entero_congelado;
        println!("  escena entera, congelada:       media {a:.2} ms | p95 {b:.2} | peor {c:.2}");
        for (paso, p, v) in &m.memoria {
            println!("  memoria {paso:<38} privada {p:>+7.1} MB | video {v:>+7.1} MB");
        }
    }
}
