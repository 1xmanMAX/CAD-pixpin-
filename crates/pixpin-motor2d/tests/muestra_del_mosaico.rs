//! **El mosaico, antes y despues, en un PNG que se puede mirar.**
//!
//! Un mosaico no se comprueba con un `assert_eq`: se mira. Lo que esta tanda
//! viene a arreglar es que en el PC se VEIA lo que el usuario habia tapado en
//! el movil, y lo contrario de eso —que ya no se vea— no lo dice ningun
//! numero tan bien como la imagen.
//!
//! Lo que si se comprueba a maquina, y por eso esto es una prueba y no un
//! ejemplo, son las dos cosas que una prueba puede decir de una imagen sin
//! verla: que dentro de la caja **no queda ni un pixel de la tinta de las
//! cifras** —la tinta de la muestra es negro puro, y ese solo sobrevive si no
//! se promedio con nada— y que **dentro de cada cuadro no queda contraste**,
//! que es lo que significa que ya no asoma ninguna forma.
//!
//! Y el caso negativo, en la misma imagen: la banda azul de arriba y la fila
//! de abajo quedan fuera de la caja y tienen que salir **identicas**, pixel a
//! pixel, porque un mosaico que se come lo que no es suyo tapa de mas.
//!
//! Los PNG se escriben en `target/muestras-grupo-d/`. Para rehacerlos:
//! `cargo test -p pixpin-motor2d --test muestra_del_mosaico -- --nocapture`.
//!
//! El codificador PNG de aqui abajo es de juguete a proposito —bloques sin
//! comprimir— para no meterle una dependencia al motor por una muestra.

use std::path::PathBuf;

use pixpin_motor2d::mosaico;

const ANCHO: u32 = 480;
const ALTO: u32 = 150;

/// Una tipografia de tres por cinco para las diez cifras. Da igual que sea
/// fea: lo que tiene que hacer es **leerse** antes de tapar.
const CIFRAS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

fn punto(v: &mut [u8], x: u32, y: u32, c: [u8; 4]) {
    if x >= ANCHO || y >= ALTO {
        return;
    }
    let i = ((y * ANCHO + x) * 4) as usize;
    v[i..i + 4].copy_from_slice(&c);
}

/// Escribe una cifra ampliada `escala` veces con la esquina en `(x, y)`.
fn cifra(v: &mut [u8], n: usize, x: u32, y: u32, escala: u32, c: [u8; 4]) {
    for (fila, bits) in CIFRAS[n].iter().enumerate() {
        for col in 0..3u32 {
            if bits & (1 << (2 - col)) == 0 {
                continue;
            }
            for dy in 0..escala {
                for dx in 0..escala {
                    punto(v, x + col * escala + dx, y + fila as u32 * escala + dy, c);
                }
            }
        }
    }
}

fn texto(v: &mut [u8], s: &str, x: u32, y: u32, escala: u32, c: [u8; 4]) {
    let mut cx = x;
    for ch in s.chars() {
        if let Some(d) = ch.to_digit(10) {
            cifra(v, d as usize, cx, y, escala, c);
        }
        cx += 4 * escala;
    }
}

/// La captura de mentira: una ficha con un numero de tarjeta bien legible.
fn captura() -> Vec<u8> {
    let mut v = vec![255u8; (ANCHO * ALTO * 4) as usize];
    let negro = [20, 20, 24, 255];
    let azul = [30, 90, 200, 255];
    // Una banda de color arriba, para que se vea que el mosaico promedia
    // colores y no solo grises.
    for y in 0..26 {
        for x in 0..ANCHO {
            punto(&mut v, x, y, azul);
        }
    }
    texto(&mut v, "4539148803436467", 24, 56, 6, negro);
    texto(&mut v, "0428", 24, 104, 5, negro);
    texto(&mut v, "915", 300, 104, 5, negro);
    v
}

/// PNG de 8 bits RGBA con la compresion apagada (bloques «stored»).
fn png(ancho: u32, alto: u32, rgba: &[u8]) -> Vec<u8> {
    fn crc32(datos: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in datos {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    fn trozo(salida: &mut Vec<u8>, tipo: &[u8; 4], datos: &[u8]) {
        salida.extend_from_slice(&(datos.len() as u32).to_be_bytes());
        let mut con_tipo = tipo.to_vec();
        con_tipo.extend_from_slice(datos);
        salida.extend_from_slice(&con_tipo);
        salida.extend_from_slice(&crc32(&con_tipo).to_be_bytes());
    }

    // Los datos del PNG llevan un byte de filtro por fila (0 = ninguno).
    let mut crudo = Vec::with_capacity((alto * (1 + ancho * 4)) as usize);
    for y in 0..alto {
        crudo.push(0);
        let desde = (y * ancho * 4) as usize;
        crudo.extend_from_slice(&rgba[desde..desde + (ancho * 4) as usize]);
    }
    // zlib con bloques sin comprimir: cabecera 0x78 0x01, bloques de 65535 y
    // Adler-32 al final.
    let mut z = vec![0x78, 0x01];
    for (i, trozo_datos) in crudo.chunks(65_535).enumerate() {
        let ultimo = (i + 1) * 65_535 >= crudo.len();
        z.push(if ultimo { 1 } else { 0 });
        z.extend_from_slice(&(trozo_datos.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(trozo_datos.len() as u16)).to_le_bytes());
        z.extend_from_slice(trozo_datos);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &crudo {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut salida = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&ancho.to_be_bytes());
    ihdr.extend_from_slice(&alto.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    trozo(&mut salida, b"IHDR", &ihdr);
    trozo(&mut salida, b"IDAT", &z);
    trozo(&mut salida, b"IEND", &[]);
    salida
}

fn carpeta() -> PathBuf {
    // `CARGO_TARGET_DIR` puede estar en cualquier parte; se pregunta por el
    // sitio del ejecutable de la prueba, que siempre esta dentro de el.
    let mut p = std::env::current_exe().expect("el ejecutable de la prueba");
    // .../target/<perfil>/deps/<exe>  ->  .../target
    p.pop();
    p.pop();
    p.pop();
    p.push("muestras-grupo-d");
    std::fs::create_dir_all(&p).expect("crear la carpeta de muestras");
    p
}

fn guardar(nombre: &str, pixeles: &[u8]) -> PathBuf {
    let ruta = carpeta().join(nombre);
    std::fs::write(&ruta, png(ANCHO, ALTO, pixeles)).expect("escribir el PNG");
    println!("muestra: {}", ruta.display());
    ruta
}

/// Cuantos pixeles de la caja siguen siendo **tinta pura**.
///
/// Es la medida de «queda algo que leer», y mide la tinta y no el papel
/// porque el papel tiene derecho a quedarse blanco: un cuadro que solo tapaba
/// hueco promedia blanco y sigue siendo blanco, y eso no es un fallo. Lo que
/// no puede quedar es un pixel del negro de la cifra, porque ese solo
/// sobrevive si no se promedio con nada.
fn tinta_pura(v: &[u8], caja: (u32, u32, u32, u32)) -> usize {
    let (x0, y0, x1, y1) = caja;
    let mut n = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * ANCHO + x) * 4) as usize;
            if v[i] <= 30 && v[i + 1] <= 30 {
                n += 1;
            }
        }
    }
    n
}

/// El contraste que queda **dentro** de cada cuadro del mosaico.
///
/// Con bloques tiene que ser cero: un cuadro con dos colores dentro es un
/// cuadro por el que todavia asoma la forma de la cifra.
fn contraste_dentro_de_los_cuadros(v: &[u8], caja: (u32, u32, u32, u32), lado: u32) -> u8 {
    let (x0, y0, x1, y1) = caja;
    let mut peor = 0u8;
    let mut cy = y0;
    while cy < y1 {
        let mut cx = x0;
        while cx < x1 {
            let (mut min, mut max) = (255u8, 0u8);
            for y in cy..(cy + lado).min(y1) {
                for x in cx..(cx + lado).min(x1) {
                    let c = v[((y * ANCHO + x) * 4) as usize];
                    min = min.min(c);
                    max = max.max(c);
                }
            }
            peor = peor.max(max - min);
            cx += lado;
        }
        cy += lado;
    }
    peor
}

#[test]
fn el_mosaico_se_come_el_numero_de_tarjeta_y_deja_el_resto_intacto() {
    // La caja tapa las cifras grandes y NO la banda azul de arriba ni la
    // fila de abajo: asi la misma muestra ensena a la vez que tapa lo suyo y
    // que no toca lo ajeno.
    let caja = (16.0, 44.0, 464.0, 96.0);
    let zona = (16u32, 44u32, 464u32, 96u32);

    let antes = captura();
    guardar("mosaico-antes.png", &antes);
    let legible = tinta_pura(&antes, zona);
    assert!(
        legible > 5_000,
        "la muestra tendria que ser bien legible antes de taparla: {legible}"
    );

    // Grosor 2 en el movil son cuadros de 16 pixeles de escena; a un aumento
    // son 16 de pantalla.
    let lado = mosaico::lado_en_pantalla(mosaico::grano(2.0), 1.0);
    assert_eq!(lado, 16);

    let mut bloques = antes.clone();
    assert!(mosaico::tapar_rgba(
        &mut bloques,
        ANCHO,
        ALTO,
        caja,
        lado,
        false
    ));
    guardar("mosaico-despues-bloques.png", &bloques);

    let mut mancha = antes.clone();
    assert!(mosaico::tapar_rgba(
        &mut mancha,
        ANCHO,
        ALTO,
        caja,
        lado,
        true
    ));
    guardar("mosaico-despues-mancha.png", &mancha);

    // **Lo que importa**: dentro de la caja no queda ni un pixel de la tinta
    // de las cifras, y con bloques ademas no queda contraste dentro de
    // ningun cuadro. Las dos cosas juntas son «ya no hay nada que leer».
    assert_eq!(
        tinta_pura(&bloques, zona),
        0,
        "con bloques quedo tinta sin promediar"
    );
    assert_eq!(
        tinta_pura(&mancha, zona),
        0,
        "con mancha quedo tinta sin promediar"
    );
    assert_eq!(
        contraste_dentro_de_los_cuadros(&bloques, zona, lado),
        0,
        "un cuadro con dos colores dentro deja asomar la cifra"
    );

    // Y fuera de la caja no se movio nada: ni la banda azul ni la fila baja.
    for y in 0..ALTO {
        for x in 0..ANCHO {
            let dentro = (16..464).contains(&x) && (44..96).contains(&y);
            if dentro {
                continue;
            }
            let i = ((y * ANCHO + x) * 4) as usize;
            assert_eq!(
                &bloques[i..i + 4],
                &antes[i..i + 4],
                "el mosaico se comio el pixel ({x},{y}), que no era suyo"
            );
        }
    }
}

#[test]
fn los_cuatro_granos_del_movil_tapan_los_cuatro() {
    // El grano no es un ajuste de gusto: es el unico mando que tiene el
    // mosaico, y con el mas fino —8 pixeles, grosor 1— tiene que tapar
    // igual que con el mas grueso. Un grano que solo tapa en su escalon
    // grande es un mosaico que engana.
    let caja = (16.0, 44.0, 464.0, 96.0);
    let zona = (16u32, 44u32, 464u32, 96u32);
    for grosor in [1.0f32, 2.0, 3.0, 8.0] {
        let mut v = captura();
        let pedido = mosaico::lado_en_pantalla(mosaico::grano(grosor), 1.0);
        // El de 64 no cabe de alto en esta caja de 52: el mosaico lo recorta
        // a una sola fila de cuadros, y la rejilla que hay que mirar es esa.
        let lado = mosaico::lado_efectivo(pedido, mosaico::recortar(caja, ANCHO, ALTO).unwrap());
        assert!(mosaico::tapar_rgba(
            &mut v, ANCHO, ALTO, caja, pedido, false
        ));
        assert_eq!(
            tinta_pura(&v, zona),
            0,
            "con grosor {grosor} (cuadro de {lado}) quedo tinta sin promediar"
        );
        assert_eq!(
            contraste_dentro_de_los_cuadros(&v, zona, lado),
            0,
            "con grosor {grosor} los cuadros no quedaron planos"
        );
    }
}

#[test]
fn el_cuadro_se_encoge_con_la_camara_pero_nunca_desaparece() {
    // El caso negativo del zoom. Lo que se ve en pantalla se encoge con la
    // camara **y el cuadro tambien**, asi que la proporcion entre cuadro y
    // dibujo se mantiene: eso es lo correcto y es lo que hace que un mosaico
    // se vea igual de cerca que de lejos.
    //
    // Lo unico que no puede pasar es que el cuadro llegue a redondearse a
    // cero o a uno, porque un mosaico de cuadro uno **es la imagen
    // original**: seguiria estando ahi el elemento y ya no taparia nada.
    let g = mosaico::grano(1.0);
    assert_eq!(mosaico::lado_en_pantalla(g, 1.0), 8);
    assert_eq!(mosaico::lado_en_pantalla(g, 0.5), 4);
    for zoom in [0.0f32, 0.001, 0.01, 0.1, 0.2] {
        let lado = mosaico::lado_en_pantalla(g, zoom);
        assert!(
            lado >= mosaico::LADO_MINIMO,
            "a {zoom} aumentos el cuadro se quedo en {lado}"
        );
    }
    // Y con la camara metida crece con el dibujo, sin tope.
    assert_eq!(mosaico::lado_en_pantalla(g, 40.0), 320);
}

/// Amplia `fuente` de `origen` sobre `destino`, como hace `p.bitmap(..)` con
/// un bitmap de origen y un rectangulo de destino: vecino mas cercano, que es
/// lo peor que puede hacer el interpolado para la privacidad —y por eso es lo
/// que hay que probar—.
fn ampliar_encima(
    lienzo: &mut [u8],
    origen: &[u8],
    fuente: (f32, f32, f32, f32),
    destino: (f32, f32, f32, f32),
) {
    let (fx, fy, fa, fal) = fuente;
    let (dx, dy, da, dal) = destino;
    for j in 0..dal as u32 {
        for i in 0..da as u32 {
            let sx = (fx + (i as f32 + 0.5) * fa / da) as u32;
            let sy = (fy + (j as f32 + 0.5) * fal / dal) as u32;
            if sx >= ANCHO || sy >= ALTO {
                continue;
            }
            let s = ((sy * ANCHO + sx) * 4) as usize;
            let c = [origen[s], origen[s + 1], origen[s + 2], origen[s + 3]];
            punto(lienzo, dx as u32 + i, dy as u32 + j, c);
        }
    }
}

/// Rellena una caja en opaco, como el gris macizo con el que quien pinta
/// vuelve a tapar dentro del cristal.
fn rellenar(lienzo: &mut [u8], caja: (f32, f32, f32, f32)) {
    let (x0, y0, x1, y1) = caja;
    for y in y0 as u32..y1 as u32 {
        for x in x0 as u32..x1 as u32 {
            punto(lienzo, x, y, [107, 107, 115, 255]);
        }
    }
}

#[test]
fn la_lupa_no_deja_leer_por_dentro_lo_que_el_mosaico_tapa() {
    // **El fallo de privacidad que esta prueba guarda.** La lupa amplia el
    // bitmap de ORIGEN —la foto congelada, el bitmap del pin—, y ese bitmap
    // no lleva las anotaciones: el mosaico se pinta encima de el, no dentro.
    // Asi que bastaba pulsar la lupa y pasar el cursor por encima de un
    // mosaico para leer el numero tapado. No se guardaba ni se capturaba,
    // pero en pantalla basta.
    //
    // Aqui se reproduce el fotograma entero a mano en CPU: se tapa, se
    // amplia el original encima, y se mira si el numero vuelve.
    let original = captura();
    let caja_mosaico = (16.0f32, 44.0f32, 464.0f32, 96.0f32);
    let lado = mosaico::lado_en_pantalla(mosaico::grano(2.0), 1.0);

    let mut compuesto = original.clone();
    assert!(mosaico::tapar_rgba(
        &mut compuesto,
        ANCHO,
        ALTO,
        caja_mosaico,
        lado,
        false
    ));

    // El cristal: dos aumentos sobre un trozo que cae de lleno en el
    // mosaico, colocado en una esquina libre de la muestra.
    let fuente = (100.0f32, 50.0f32, 60.0f32, 20.0f32);
    let destino = (300.0f32, 4.0f32, 120.0f32, 40.0f32);
    let zona_cristal = (
        destino.0 as u32,
        destino.1 as u32,
        (destino.0 + destino.2) as u32,
        (destino.1 + destino.3) as u32,
    );

    // **El caso negativo primero**: sin volver a tapar, el cristal ensena el
    // numero. Si esto dejara de ser cierto, la prueba de abajo no probaria
    // nada y habria que rehacer la muestra.
    let mut sin_arreglo = compuesto.clone();
    ampliar_encima(&mut sin_arreglo, &original, fuente, destino);
    guardar("lupa-sin-arreglo.png", &sin_arreglo);
    let filtrado = tinta_pura(&sin_arreglo, zona_cristal);
    assert!(
        filtrado > 200,
        "el montaje de la prueba no reproduce la fuga: solo {filtrado} pixeles de tinta"
    );

    // Y ahora con el arreglo: las zonas que dice `zonas_en_la_lupa` se
    // vuelven a tapar despues de ampliar.
    let mut con_arreglo = compuesto.clone();
    ampliar_encima(&mut con_arreglo, &original, fuente, destino);
    let zonas = mosaico::zonas_en_la_lupa(&[caja_mosaico], fuente, destino);
    assert_eq!(zonas.len(), 1, "el mosaico cae dentro del cristal");
    for z in &zonas {
        rellenar(&mut con_arreglo, *z);
    }
    guardar("lupa-con-arreglo.png", &con_arreglo);
    assert_eq!(
        tinta_pura(&con_arreglo, zona_cristal),
        0,
        "por el cristal de la lupa se sigue leyendo lo tapado"
    );

    // Y no se ha tapado de mas: fuera del cristal, el fotograma es el que
    // ya estaba compuesto y tapado.
    for y in 0..ALTO {
        for x in 0..ANCHO {
            if (zona_cristal.0..zona_cristal.2).contains(&x)
                && (zona_cristal.1..zona_cristal.3).contains(&y)
            {
                continue;
            }
            let i = ((y * ANCHO + x) * 4) as usize;
            assert_eq!(
                &con_arreglo[i..i + 4],
                &compuesto[i..i + 4],
                "la lupa toco el pixel ({x},{y}), que esta fuera de su cristal"
            );
        }
    }
}

#[test]
fn una_lupa_lejos_del_mosaico_no_tapa_nada_y_una_esquina_si() {
    // El caso negativo de `zonas_en_la_lupa`: sin mosaico a la vista, el
    // cristal se queda limpio. Si tapara siempre, la lupa seria inutil.
    let caja = (200.0f32, 200.0f32, 260.0f32, 240.0f32);
    let lejos =
        mosaico::zonas_en_la_lupa(&[caja], (0.0, 0.0, 50.0, 50.0), (0.0, 0.0, 100.0, 100.0));
    assert!(lejos.is_empty(), "tapo sin tener nada que tapar");

    // Y un mosaico que entra solo por una esquina si da zona, recortada al
    // cristal: media esquina sin tapar es media cifra legible.
    let esquina =
        mosaico::zonas_en_la_lupa(&[caja], (180.0, 180.0, 40.0, 40.0), (0.0, 0.0, 80.0, 80.0));
    assert_eq!(esquina.len(), 1);
    let (x0, y0, x1, y1) = esquina[0];
    assert!(
        x0 >= 0.0 && y0 >= 0.0 && x1 <= 80.0 && y1 <= 80.0,
        "sin recortar al cristal"
    );
    assert!(x1 > x0 && y1 > y0);
    // Con dos aumentos, el trozo de mosaico que entra (20x20 de la fuente)
    // ocupa la esquina baja-derecha del cristal.
    assert!(
        (x0 - 40.0).abs() < 1.5 && (y0 - 40.0).abs() < 1.5,
        "la esquina cayo en ({x0},{y0})"
    );

    // Una fuente o un cristal sin area no dan nada en vez de dividir por cero.
    assert!(
        mosaico::zonas_en_la_lupa(&[caja], (0.0, 0.0, 0.0, 10.0), (0.0, 0.0, 10.0, 10.0))
            .is_empty()
    );
    assert!(
        mosaico::zonas_en_la_lupa(&[caja], (0.0, 0.0, 10.0, 10.0), (0.0, 0.0, 10.0, 0.0))
            .is_empty()
    );
    assert!(
        mosaico::zonas_en_la_lupa(&[caja], (f32::NAN, 0.0, 10.0, 10.0), (0.0, 0.0, 10.0, 10.0))
            .is_empty()
    );
}
