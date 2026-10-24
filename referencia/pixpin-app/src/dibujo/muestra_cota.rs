//! **La cota del movil, pintada por el camino de la pantalla** (Direct2D,
//! `pintar_escena`, el mismo del lienzo y del PNG) y guardada para mirarla.
//!
//! La queja (27-sep-2026): la foto medida en el movil decia «5 cm» alli y
//! «37 px» aqui, con el numero tumbado, sin halo y de otro color; y las
//! lineas salian torcidas. El lienzo es inventado con la misma forma que los
//! del movil (escala arriba del todo, cotas `pixpin-measure` con su
//! `fontSize`, una linea redonda larga).
//!
//! `cargo test -p pixpin --bin pixpinmax muestra_de_la_cota -- --ignored
//! --nocapture --test-threads=1`. Deja `target/muestras-de-cota/*.png`.

use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;

const PLANO: &str = r##"{"type":"excalidraw","backgroundColor":"#ffffff",
    "escala":{"decimales":2,"unidad":"cm","unidadesPorPixel":0.016666666666666666},
    "elements":[
      {"type":"rectangle","id":"foto","x":0,"y":0,"width":240,"height":400,
       "strokeColor":"#1e1e1e","backgroundColor":"#1e1e1e","fillStyle":"solid",
       "strokeWidth":1,"roughness":0,"opacity":100,"seed":1},
      {"type":"pixpin-measure","id":"larga","x":60,"y":50,"width":0,"height":300,
       "points":[{"x":0.0,"y":0.0},{"x":0.0,"y":300.0}],"strokeColor":"#0edeff",
       "strokeWidth":2.5,"roughness":1,"opacity":100,"seed":42,"fontFamily":5,"fontSize":20.0},
      {"type":"pixpin-measure","id":"corta","x":170,"y":180,"width":0.6955,"height":36.6305,
       "points":[{"x":0.0,"y":0.0},{"x":-0.6955,"y":36.6305}],"strokeColor":"#0edeff",
       "strokeWidth":2.5,"roughness":1,"opacity":100,"seed":7,"fontFamily":5,"fontSize":9.0},
      {"type":"pixpin-measure","id":"blanca","x":260,"y":120,"width":200,"height":60,
       "points":[{"x":0.0,"y":0.0},{"x":200.0,"y":60.0}],"strokeColor":"#0edeff",
       "strokeWidth":2.5,"roughness":1,"opacity":100,"seed":9,"fontFamily":5,"fontSize":20.0},
      {"type":"line","id":"linea","x":120,"y":10,"width":3,"height":380,"roundness":{"type":3},
       "points":[{"x":0.0,"y":0.0},{"x":3.0,"y":380.0}],"strokeColor":"#ff6600",
       "strokeWidth":2.5,"roughness":1,"opacity":100,"seed":950731993},
      {"type":"line","id":"recta","x":260,"y":300,"width":200,"height":0,
       "points":[{"x":0.0,"y":0.0},{"x":200.0,"y":0.0}],"strokeColor":"#8633ff",
       "strokeWidth":2.5,"roughness":1,"opacity":100,"seed":1463832659}
    ]}"##;

const ANCHO: u32 = 480;
const ALTO: u32 = 400;

/// Pinta el plano sobre `papel` DOS fotogramas seguidos, como el lienzo, y
/// devuelve los pixeles de los dos; guarda el PNG del segundo, que es lo que
/// se ve desde entonces.
///
/// Dos porque el halo del numero se pinta distinto la primera vez que se ve
/// un renglon (sus 24 copias, las de siempre) y las siguientes (desde un
/// mapa hecho una vez, `pixpin-render/src/halo.rs`): la prueba de abajo
/// compara uno con otro para que el mapa no cambie lo que se ve.
fn pintar(nombre: &str, papel: pixpin_motor2d::ColorRgba) -> (Vec<u8>, Vec<u8>) {
    let mut escena =
        pixpin_motor2d::excalidraw::a_escena(&pixpin_motor2d::excalidraw::leer(PLANO).unwrap());
    escena.fondo = papel;
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ANCHO, ALTO).expect("superficie");
    let imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(16);
    let mut cache = pixpin_motor2d::cache::Cache::nueva();
    let mut tinta = pixpin_render::CacheTinta::nueva();
    let mut fotogramas = Vec::new();
    for _ in 0..2 {
        // Como el lienzo: el medidor de DirectWrite para las cajas y el papel
        // del fotograma fijado.
        pixpin_motor2d::texto::con_medidor(super::pintar::medir_para_el_motor, || {
            super::tema::fijar_papel(Some(papel));
            motor
                .dibujar(&fuera.destino, |p| {
                    p.limpiar(super::pintar::a_color(papel));
                    super::pintar::pintar_escena(
                        p,
                        &escena,
                        &mut cache,
                        &mut tinta,
                        &imagenes,
                        (0.0, 0.0, ANCHO as f32, ALTO as f32),
                        1.0,
                        |_| false,
                    );
                })
                .expect("pintar");
            super::tema::fijar_papel(None);
        });
        fuera.esperar_gpu().expect("esperar");
        fotogramas.push(fuera.leer_rgba().expect("leer").2);
    }
    let segundo = fotogramas.pop().expect("dos");
    let primero = fotogramas.pop().expect("dos");
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho: ANCHO,
        alto: ALTO,
        pixeles: segundo.clone(),
    })
    .expect("codificar");
    let carpeta =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-de-cota");
    std::fs::create_dir_all(&carpeta).expect("carpeta");
    std::fs::write(carpeta.join(format!("{nombre}.png")), png).expect("guardar");
    (primero, segundo)
}

/// Cuantos pixeles cambian de un fotograma a otro y cuanto como mucho (el
/// canal que mas, de 0 a 255).
fn diferencia(a: &[u8], b: &[u8]) -> (usize, u8) {
    a.chunks(4).zip(b.chunks(4)).fold((0, 0), |(n, m), (p, q)| {
        let d = p
            .iter()
            .zip(q)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        (n + (d > 0) as usize, m.max(d))
    })
}

/// La caja de los pixeles que cumplen `f` dentro de `(x0, y0, x1, y1)`.
fn caja(
    v: &[u8],
    (x0, y0, x1, y1): (u32, u32, u32, u32),
    f: impl Fn(&[u8]) -> bool,
) -> Option<(u32, u32, u32, u32)> {
    let mut c: Option<(u32, u32, u32, u32)> = None;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * ANCHO + x) * 4) as usize;
            if f(&v[i..i + 4]) {
                c = Some(c.map_or((x, y, x, y), |(a, b, cc, d)| {
                    (a.min(x), b.min(y), cc.max(x), d.max(y))
                }));
            }
        }
    }
    c
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestra_de_la_cota_sobre_papel_blanco_y_de_noche() {
    let (blanco_antes, blanco) = pintar(
        "cota_pantalla_papel_blanco",
        pixpin_motor2d::ColorRgba::opaco(1.0, 1.0, 1.0),
    );
    // El mapa del halo no cambia lo que se ve: solo el remuestreo del borde
    // del halo (medido: 1.087 de 192.000 pixeles, todos en su borde).
    let (n, _) = diferencia(&blanco_antes, &blanco);
    assert!(
        n < (ANCHO * ALTO / 100) as usize,
        "el halo de mapa cambia {n} pixeles"
    );
    // Sobre la foto oscura (x 0..240) solo es blanco el halo de un numero, y
    // el de la cota larga (x 0..100) esta de pie: va a lo largo de la raya.
    let es_blanco = |p: &[u8]| p[0] > 230 && p[1] > 230 && p[2] > 230;
    let (hx0, hy0, hx1, hy1) =
        caja(&blanco, (5, 5, 100, 395), es_blanco).expect("hay halo sobre la foto");
    assert!(
        hy1 - hy0 > (hx1 - hx0) * 2,
        "de pie: {}x{}",
        hx1 - hx0,
        hy1 - hy0
    );
    // La tinta de la cota: el cian adaptado, oscuro (azul > verde > rojo, y
    // ningun pixel del cian claro original #0edeff).
    let cian_claro = |p: &[u8]| p[0] < 40 && p[1] > 200 && p[2] > 240;
    assert!(
        caja(&blanco, (5, 5, 100, 395), cian_claro).is_none(),
        "no el cian claro tal cual"
    );

    // Sobre papel de noche el cian se lee tal cual y su halo es negro.
    let noche = pixpin_motor2d::ColorRgba::opaco(
        0x12 as f32 / 255.0,
        0x12 as f32 / 255.0,
        0x12 as f32 / 255.0,
    );
    let (oscuro_antes, oscuro) = pintar("cota_pantalla_papel_de_noche", noche);
    let (n, _) = diferencia(&oscuro_antes, &oscuro);
    assert!(
        n < (ANCHO * ALTO / 100) as usize,
        "el halo de mapa cambia {n} pixeles de noche"
    );
    assert!(
        caja(&oscuro, (250, 100, 470, 200), cian_claro).is_some(),
        "el cian de siempre sobre la pizarra"
    );
}
