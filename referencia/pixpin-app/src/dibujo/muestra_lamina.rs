//! **La figura central de la lamina de geometria, dibujada con las
//! herramientas del lienzo** y pintada por el camino de la pantalla (Direct2D,
//! `pintar_escena`), junto al original, para mirarla.
//!
//! La lamina (`circunferencia-geometria.png`, 15-oct-2026) es la que el
//! usuario puso delante al pedir «mejora las otras herramientas para que
//! puedan hacer todo eso». Aqui se dibuja **a mano, por coordenadas**, con lo
//! mismo que tiene el usuario en la barra: el Arco por centro y dos puntos
//! (con el gesto, clic a clic), la marca de Angulo (un clic dentro del
//! angulo), el cuadrante con su degradado radial, las rectas, la recta
//! punteada con flecha, los puntos con su letra y el recuadro.
//!
//! `cargo test -p pixpin --bin pixpinmax muestra_de_la_lamina -- --ignored
//! --nocapture --test-threads=1`. Deja `figura-central.png` (lo dibujado) y
//! `figura-central-junto.png` (el original a la izquierda) en la carpeta de
//! `PIXPIN_MUESTRAS`, o en `target/muestras-de-lamina`.

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::formas::TipoPunta;
use pixpin_motor2d::geometria_exacta as ge;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::vector::Punto2;
use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;

/// El trozo de la lamina original que es la figura central.
const RECORTE: (u32, u32, u32, u32) = (296, 128, 484, 424);

fn p(x: f32, y: f32) -> Punto2 {
    // Las coordenadas son las de la lamina original: se llevan al recorte.
    Punto2::nuevo(x - RECORTE.0 as f32, y - RECORTE.1 as f32)
}

fn hex(c: u32) -> ColorRgba {
    ColorRgba::opaco(
        ((c >> 16) & 0xff) as f32 / 255.0,
        ((c >> 8) & 0xff) as f32 / 255.0,
        (c & 0xff) as f32 / 255.0,
    )
}

const NEGRO: u32 = 0x1e1e1e;
const AZUL: u32 = 0x2a2ad6;
const ROJO: u32 = 0xe8262b;
const VERDE: u32 = 0x2ea84a;
const GRIS: u32 = 0x8a8a8a;

fn con(mut e: Elemento, color: u32, grosor: f32) -> Elemento {
    e.trazo = hex(color);
    e.grosor = grosor;
    e.rugosidad = 0.0;
    e
}

fn texto(t: &str, x: f32, y: f32, tam: f32, color: u32, cursiva: bool) -> Elemento {
    let q = p(x, y);
    let mut e = Elemento {
        figura: Figura::Texto {
            texto: t.into(),
            tam,
            familia: "Segoe UI".into(),
        },
        x: q.x,
        y: q.y,
        ancho: tam * 0.6 * t.chars().count() as f32,
        alto: tam * 1.25,
        trazo: hex(color),
        rugosidad: 0.0,
        ..Default::default()
    };
    e.extras.cursiva = cursiva;
    e
}

fn clic(g: &mut Gesto, escena: &mut Escena, q: Punto2) {
    let ev = |p| EventoGesto::Pulsar {
        p,
        shift: false,
        alt: false,
        presion: None,
    };
    g.evento(ev(q), escena, 1.0);
    g.evento(EventoGesto::Soltar { p: q }, escena, 1.0);
}

fn hover(g: &mut Gesto, escena: &mut Escena, q: Punto2) {
    g.evento(
        EventoGesto::Mover {
            p: q,
            shift: false,
            alt: false,
            presion: None,
        },
        escena,
        1.0,
    );
}

/// **El arco de color sobre la circunferencia**, con la herramienta Arco en
/// «centro y dos puntos»: clic en el centro, clic donde empieza y el cursor
/// dando la vuelta hasta donde acaba.
fn arco_con_la_herramienta(
    escena: &mut Escena,
    c: Punto2,
    desde: Punto2,
    hasta: Punto2,
    color: u32,
    grosor: f32,
) {
    let mut g = Gesto::nuevo();
    g.enganche.activo = false;
    g.tomar_herramienta(Herramienta::Arco);
    g.estilo.modo_arco = ge::ModoArco::CentroYPuntos;
    g.estilo.trazo = hex(color);
    g.estilo.rugosidad = 0.0;
    clic(&mut g, escena, c);
    clic(&mut g, escena, desde);
    // El cursor va de `desde` a `hasta` por el camino corto, en pasos.
    let (a0, d) = ge::angulo_menor(c, desde, hasta);
    let r = c.distancia(desde);
    for k in 1..=12 {
        hover(
            &mut g,
            escena,
            ge::en_la_circunferencia(c, r, a0 + d * k as f32 / 12.0),
        );
    }
    clic(&mut g, escena, hasta);
    if let Some(e) = escena.elementos.last_mut() {
        e.grosor = grosor;
    }
}

/// **La marca de un angulo** con la herramienta Angulo: un clic dentro.
fn marca_con_la_herramienta(
    escena: &mut Escena,
    dentro: Punto2,
    color: u32,
    relleno: ColorRgba,
    valor: Option<&str>,
) {
    let mut g = Gesto::nuevo();
    g.tomar_herramienta(Herramienta::Angulo);
    g.estilo.trazo = hex(color);
    g.estilo.relleno = Some(relleno);
    let antes = escena.elementos.len();
    clic(&mut g, escena, dentro);
    assert_eq!(escena.elementos.len(), antes + 1, "la marca en {dentro:?}");
    let e = escena.elementos.last_mut().unwrap();
    e.grosor = 1.2;
    if let Some(m) = e.extras.marca_angulo.as_mut() {
        m.valor = valor.map(str::to_string);
    }
    e.extras.tam_letra = Some(15.0);
}

/// La figura central, pieza a pieza.
fn lamina() -> Escena {
    let mut s = Escena::nueva();
    let o = p(567.0, 352.0); // centro de la circunferencia
    let r = 186.0;
    let q = p(437.0, 482.0); // el vertice del cuadrante
    let a = p(437.0, 221.0);
    let b = p(698.0, 482.0);
    let c = ge::en_la_circunferencia(o, r, ge::angulo_de(o, p(560.0, 166.0)));
    let d = Punto2::nuevo(o.x + r, o.y);
    // P: donde se cortan AD y CB.
    let pp = ge::cruce_de_segmentos(a, d, c, b).expect("AD y CB se cortan");

    // 1. El cuadrante, relleno con el degradado radial (debajo de todo).
    let mut cuad = con(
        ge::sector(
            q,
            q.distancia(a),
            -std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_2,
        ),
        NEGRO,
        3.2,
    );
    cuad.trazo.a = 0.0; // el borde lo ponen sus rectas y su arco
    cuad.extras.degradado = Some(ge::Degradado {
        origen: (0.5, 0.5),
        destino: (1.05, 1.05),
        // Naranja amarillento en el centro y casi blanco en las puntas.
        ..ge::Degradado::radial(hex(0xffd47e), hex(0xfff6e6))
    });
    s.anadir(cuad);
    // Su arco AB, grueso y negro, y la prolongacion hacia la izquierda.
    s.anadir(con(
        ge::arco(
            q,
            q.distancia(a),
            -std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_2,
        ),
        NEGRO,
        3.2,
    ));
    let izq = p(313.0, 253.0);
    s.anadir(con(
        ge::arco(
            q,
            q.distancia(a),
            ge::angulo_de(q, izq),
            ge::angulo_menor(q, izq, a).1,
        ),
        NEGRO,
        1.6,
    ));

    // 2. La circunferencia.
    s.anadir(con(ge::circunferencia(o, r), NEGRO, 1.6));

    // 3. Los lados rojos del cuadrante y el recuadro rojo.
    s.anadir(con(ge::recta(q, a), ROJO, 2.0));
    s.anadir(con(ge::recta(q, b), ROJO, 2.0));
    let mut recuadro = con(
        Elemento {
            figura: Figura::Rectangulo,
            ..Default::default()
        },
        ROJO,
        1.6,
    );
    let esq = p(437.0, 407.0);
    (recuadro.x, recuadro.y, recuadro.ancho, recuadro.alto) = (esq.x, esq.y, 76.0, 75.0);
    s.anadir(recuadro);

    // 4. Las rectas: AD y CB negras, OC y OD azules (desde el vertice).
    s.anadir(con(ge::recta(a, d), NEGRO, 1.8));
    s.anadir(con(ge::recta(c, b), NEGRO, 1.8));
    s.anadir(con(ge::recta(q, c), AZUL, 1.8));
    s.anadir(con(ge::recta(q, d), AZUL, 1.8));
    // AB, el diametro, punteada.
    let mut ab = con(ge::recta(a, b), NEGRO, 1.4);
    ab.estilo = EstiloTrazo::Discontinuo;
    s.anadir(ab);

    // 5. Las rectas punteadas con flecha: el radio de la circunferencia y el
    //    del cuadrante.
    for (desde, hasta) in [(o, p(393.0, 296.0)), (p(399.0, 400.0), p(352.0, 244.0))] {
        let mut f = con(
            ge::flecha(desde, hasta, TipoPunta::Ninguna, TipoPunta::Triangulo),
            NEGRO,
            1.3,
        );
        f.estilo = EstiloTrazo::Discontinuo;
        s.anadir(f);
    }

    // 6. Los arcos verdes sobre la circunferencia, con la herramienta Arco.
    arco_con_la_herramienta(&mut s, o, a, c, VERDE, 4.5);
    arco_con_la_herramienta(&mut s, o, d, b, VERDE, 4.5);

    // 7. Las marcas de angulo, con la herramienta Angulo: X entre las azules
    //    (gris), a y b entre las azules y los lados rojos, y 45° en P (verde).
    let gris = ColorRgba {
        a: 0.75,
        ..hex(0xb5b5b5)
    };
    marca_con_la_herramienta(
        &mut s,
        q.sumar(Punto2::nuevo(24.0, -20.0)),
        GRIS,
        gris,
        None,
    );
    let rosa = ColorRgba {
        a: 0.3,
        ..hex(ROJO)
    };
    marca_con_la_herramienta(&mut s, q.sumar(Punto2::nuevo(8.0, -30.0)), ROJO, rosa, None);
    marca_con_la_herramienta(&mut s, q.sumar(Punto2::nuevo(34.0, -6.0)), ROJO, rosa, None);
    let verde = ColorRgba {
        a: 0.35,
        ..hex(VERDE)
    };
    let bis = ge::angulo_de(pp, d) + ge::angulo_menor(pp, d, b).1 / 2.0;
    marca_con_la_herramienta(
        &mut s,
        ge::en_la_circunferencia(pp, 26.0, bis),
        VERDE,
        verde,
        Some("45°"),
    );

    // 8. Los puntos con su letra.
    let punto = |s: &mut Escena, en: Punto2, letra: &str, hacia: f32, color: u32| {
        let mut e = con(
            ge::punto_con_letra(en, letra, hacia.to_radians()),
            color,
            2.6,
        );
        e.extras.tam_letra = Some(17.0);
        if let Figura::Punto { radio, .. } = &mut e.figura {
            *radio = 12.0;
        }
        e.trazo = hex(color);
        s.anadir(e);
    };
    punto(&mut s, a, "A", 215.0, VERDE);
    punto(&mut s, c, "C", 285.0, VERDE);
    punto(&mut s, d, "D", 350.0, VERDE);
    punto(&mut s, b, "B", 60.0, VERDE);
    punto(&mut s, pp, "P", 325.0, NEGRO);
    punto(&mut s, o, "", 0.0, NEGRO);
    punto(&mut s, q, "", 0.0, ROJO);
    punto(&mut s, izq, "", 0.0, NEGRO);

    // 9. Los rotulos: los arcos, la O, los angulos a y b.
    s.anadir(texto("O", 556.0, 357.0, 17.0, NEGRO, true));
    s.anadir(texto("O", 420.0, 492.0, 17.0, NEGRO, true));
    s.anadir(texto("2a", 476.0, 160.0, 17.0, VERDE, true));
    s.anadir(texto("2b", 740.0, 420.0, 17.0, VERDE, true));
    s.anadir(texto("a", 443.0, 414.0, 14.0, ROJO, true));
    s.anadir(texto("b", 487.0, 459.0, 14.0, ROJO, true));
    s.anadir(texto("X", 469.0, 430.0, 16.0, NEGRO, false));
    s
}

fn carpeta() -> std::path::PathBuf {
    let c = std::env::var_os("PIXPIN_MUESTRAS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-de-lamina")
        });
    std::fs::create_dir_all(&c).expect("carpeta");
    c
}

fn pintar(escena: &Escena, ancho: u32, alto: u32) -> Vec<u8> {
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
    let imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(16);
    let mut cache = pixpin_motor2d::cache::Cache::nueva();
    let mut tinta = pixpin_render::CacheTinta::nueva();
    pixpin_motor2d::texto::con_medidor(super::pintar::medir_para_el_motor, || {
        motor
            .dibujar(&fuera.destino, |pintor| {
                pintor.limpiar(super::pintar::a_color(ColorRgba::opaco(1.0, 1.0, 1.0)));
                super::pintar::pintar_escena(
                    pintor,
                    escena,
                    &mut cache,
                    &mut tinta,
                    &imagenes,
                    (0.0, 0.0, ancho as f32, alto as f32),
                    1.0,
                    |_| false,
                );
            })
            .expect("pintar");
    });
    fuera.esperar_gpu().expect("esperar");
    fuera.leer_rgba().expect("leer").2
}

#[test]
#[ignore = "necesita GPU; deja el PNG de la figura central para mirarlo"]
fn muestra_de_la_lamina() {
    let escena = lamina();
    let (_, _, ancho, alto) = RECORTE;
    let mio = pintar(&escena, ancho, alto);
    let c = carpeta();
    let png = |pixeles: Vec<u8>, ancho: u32| {
        pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho,
            alto,
            pixeles,
        })
        .expect("png")
    };
    std::fs::write(c.join("figura-central.png"), png(mio.clone(), ancho)).expect("guardar");
    // Junto al original, si esta: a la izquierda el recorte de la lamina.
    let original = std::env::var_os("PIXPIN_LAMINA").map(std::path::PathBuf::from);
    if let Some(o) = original.and_then(|r| pixpin_codec::imagen::cargar(&r).ok()) {
        let mut junto = vec![255u8; (2 * ancho * alto * 4) as usize];
        for y in 0..alto {
            for x in 0..ancho {
                let (ox, oy) = (x + RECORTE.0, y + RECORTE.1);
                let destino = ((y * 2 * ancho + x) * 4) as usize;
                if ox < o.ancho && oy < o.alto {
                    let i = ((oy * o.ancho + ox) * 4) as usize;
                    junto[destino..destino + 4].copy_from_slice(&o.pixeles[i..i + 4]);
                }
                let j = ((y * ancho + x) * 4) as usize;
                let d2 = ((y * 2 * ancho + ancho + x) * 4) as usize;
                junto[d2..d2 + 4].copy_from_slice(&mio[j..j + 4]);
            }
        }
        std::fs::write(c.join("figura-central-junto.png"), png(junto, 2 * ancho)).expect("guardar");
    }
    // Lo que se puede decir sin mirar: hay tinta naranja del degradado y
    // verde de los arcos.
    let naranja = mio
        .chunks(4)
        .filter(|v| v[0] > 230 && v[1] > 150 && v[1] < 215 && v[2] < 150)
        .count();
    let verde = mio
        .chunks(4)
        .filter(|v| v[1] > 140 && v[0] < 90 && v[2] < 110)
        .count();
    println!("naranja {naranja} px, verde {verde} px: {}", c.display());
    assert!(
        naranja > 500 && verde > 300,
        "naranja {naranja}, verde {verde}"
    );
}
