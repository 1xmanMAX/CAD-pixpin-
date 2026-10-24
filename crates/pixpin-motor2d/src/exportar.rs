//! **Que se exporta de un lienzo: las hojas.**
//!
//! Exportar a PNG, SVG, PDF o pagina web, e imprimir, empiezan todos por la
//! misma pregunta: que trozo del lienzo sale y con que dentro. Si cada
//! formato la contestara por su cuenta, exportar dos veces en formatos
//! distintos daria dos encuadres distintos, que es justo lo que el movil
//! evita (`DrawSvg.aTexto`: «que las tres salidas encuadren igual no es un
//! detalle»). Por eso la respuesta vive aqui, una vez, y es pura: una lista
//! de [`Hoja`]s con su caja y sus ordenes de dibujo ya calculadas.
//!
//! Las reglas son las del movil (`DrawExport.kt`, `DrawPdf.kt`,
//! `Imprimir.kt`):
//!
//! - **Todo**: lo visible, con [`MARGEN`] alrededor. Los marcos no se pintan
//!   (son el encuadre, no un trazo), pero su papel pautado si.
//! - **Cada marco**: una hoja por marco que tenga algo dentro, en el orden en
//!   que se leen, recortada a su caja y sin margen (el margen ya lo decidio
//!   quien puso el marco). Sin marcos, lo mismo que «todo».
//! - **Lo elegido**: solo eso. Si lo elegido es un marco, sale ese marco.
//! - Con **papel de fondo** (la foto o la pagina sobre la que se anota), el
//!   papel manda en el encuadre y no hay margen: exportar la anotacion sin la
//!   hoja dejaria los trazos flotando sobre nada.

use crate::elemento::{ColorRgba, Elemento};
use crate::escena::Escena;
use crate::marco;
use crate::pintado::{self, Orden};
use crate::vector::Punto2;

/// Margen alrededor del contenido, en pixeles de escena (`exportPadding` de
/// Excalidraw, el mismo que usa el movil).
pub const MARGEN: f32 = 10.0;

/// Lado maximo de un PNG exportado, en pixeles.
///
/// El movil topa en 4096 porque un telefono no tiene memoria para mas. Aqui
/// se permite el doble, que sigue muy por debajo de los 16384 que garantiza
/// Direct3D 11 y son 256 MB de RGBA en el peor caso: un lienzo infinito con
/// dos trazos separados por diez mil pixeles a escala 3 no puede tumbar el
/// programa, sale mas pequeno.
pub const LADO_MAXIMO_PNG: u32 = 8192;

/// El `id_objeto` reservado para el papel de fondo (la foto o la pagina del
/// PDF sobre la que se dibuja). Va como una `Orden::Imagen` mas para que
/// ningun formato tenga que saber que existe el papel: quien resuelve
/// imagenes, lo resuelve tambien a el. Nunca coincide con una imagen de
/// verdad porque esas se numeran desde uno hacia arriba.
pub const ID_PAPEL: u64 = u64::MAX;

/// Que trozo del lienzo se exporta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alcance {
    Todo,
    Seleccion,
    Marcos,
}

/// Un marco a la vista, para la pagina web del lienzo entero: el visor lo usa
/// para imprimir marco a marco (`g.marcos` del movil).
#[derive(Debug, Clone, PartialEq)]
pub struct MarcoALaVista {
    /// Su numero de hoja, desde uno.
    pub n: usize,
    pub nombre: String,
    pub caja: (f32, f32, f32, f32),
}

/// Una pagina de lo exportado.
#[derive(Debug, Clone, PartialEq)]
pub struct Hoja {
    /// El nombre del marco, o vacio si no sale de uno.
    pub nombre: String,
    /// Lo que se ve: `(x0, y0, x1, y1)` en coordenadas de escena, ya con su
    /// margen.
    pub caja: (f32, f32, f32, f32),
    /// Lo que hay que pintar, de abajo arriba.
    pub ordenes: Vec<Orden>,
    /// Los marcos del lienzo, solo en la hoja del lienzo entero.
    pub marcos: Vec<MarcoALaVista>,
    /// **El grano de las tintas porosas**: `(i, grano)` dice que la silueta
    /// de `ordenes[i]` (una `Orden::Tinta`) se tine con esa tela, encima de
    /// su cuerpo. En orden de `i`.
    ///
    /// Va aparte y no dentro de las ordenes por lo mismo que en pantalla
    /// (`pintado::Grano`): la silueta ya esta en la orden y repetirla seria
    /// escribir dos veces el camino mas largo del fichero. Sin esto, la tiza,
    /// el rayado y los otros ocho materiales salian lisos en todo lo
    /// exportado, iguales entre si.
    pub granos: Vec<(usize, pintado::Grano)>,
    /// **El grafito, como mapa de casillas** (`tinta::grafito`): cada uno se
    /// pinta justo antes de `ordenes[antes_de]`. Es lo que hace el `DrawSvg`
    /// del movil: lo que hace grafito al grafito vive en su mapa, asi que es
    /// el mapa lo que viaja —como imagen en el SVG y en el PDF, pintado en el
    /// PNG—. Sin esto salia como una raya lisa. En orden de `antes_de`.
    pub grafitos: Vec<crate::tinta::grafito::GrafitoSuelto>,
}

impl Hoja {
    pub fn ancho(&self) -> f32 {
        self.caja.2 - self.caja.0
    }

    pub fn alto(&self) -> f32 {
        self.caja.3 - self.caja.1
    }
}

/// Las hojas que salen de exportar `alcance`.
///
/// `seleccion` son los ids elegidos (solo cuentan con [`Alcance::Seleccion`]).
/// `papel` es el tamano del papel de fondo si lo hay, colocado en (0, 0).
///
/// Devuelve una lista vacia si no hay nada que exportar: quien llama lo dice
/// en vez de sacar un rectangulo en blanco.
pub fn hojas(
    escena: &Escena,
    alcance: Alcance,
    seleccion: &[u64],
    papel: Option<(f32, f32)>,
) -> Vec<Hoja> {
    let papel = papel.filter(|(w, h)| *w > 0.0 && *h > 0.0);
    match alcance {
        Alcance::Todo => entero(escena, papel).into_iter().collect(),
        Alcance::Marcos => {
            let v = por_marcos(escena, papel);
            if v.is_empty() {
                entero(escena, papel).into_iter().collect()
            } else {
                v
            }
        }
        Alcance::Seleccion => elegido(escena, seleccion, papel).into_iter().collect(),
    }
}

/// Lo que se pinta de un elemento al exportarlo.
///
/// Un marco no se pinta a si mismo —ni su recuadro ni su rotulo, que son de
/// la pantalla—, solo su papel si lo tiene: uno eligio papel rayado para que
/// salga rayado. Lo demas, igual que `ordenes_de_escena`, con sus cotas.
fn ordenes_al_exportar(escena: &Escena, e: &Elemento) -> Vec<Orden> {
    if marco::es_marco(e) {
        return marco::ordenes_del_papel(e);
    }
    let mut v = pintado::ordenes(e);
    v.extend(pintado::ordenes_medibles(
        e,
        escena.escala.as_ref(),
        ',',
        escena.fondo,
    ));
    v
}

/// Mete lo que se pinta de `e` y apunta que siluetas llevan grano: las
/// mismas que en pantalla, cada `Orden::Tinta` de un elemento con tela
/// (`dibujar_orden` del editor). La tinta sin contorno es el aviso del
/// grafito, que no es una silueta y se pinta por su cuenta.
fn meter(
    escena: &Escena,
    e: &Elemento,
    ordenes: &mut Vec<Orden>,
    granos: &mut Vec<(usize, pintado::Grano)>,
    grafitos: &mut Vec<crate::tinta::grafito::GrafitoSuelto>,
) {
    // El grafito va en su mapa y, en las ordenes, solo lo medible (una cota
    // de grafito lleva su numero). Si no se puede cocer, va liso.
    if !marco::es_marco(e)
        && let Some(g) = crate::tinta::grafito::suelto(e, ordenes.len())
    {
        grafitos.push(g);
        ordenes.extend(pintado::ordenes_medibles(
            e,
            escena.escala.as_ref(),
            ',',
            escena.fondo,
        ));
        return;
    }
    let grano = pintado::grano_de(e);
    for o in ordenes_al_exportar(escena, e) {
        if let (Some(g), Orden::Tinta { contorno, .. }) = (grano, &o)
            && contorno.len() >= 3
        {
            granos.push((ordenes.len(), g));
        }
        ordenes.push(o);
    }
}

/// El grano de la orden `i` de una hoja, si lo lleva.
pub fn grano_de_la_orden(hoja: &Hoja, i: usize) -> Option<pintado::Grano> {
    hoja.granos
        .binary_search_by_key(&i, |(j, _)| *j)
        .ok()
        .map(|k| hoja.granos[k].1)
}

/// **La tela de un grano como pixeles**: `lado` x `lado` RGBA sin
/// premultiplicar, el color de la tinta con la cobertura del tejido de
/// alfa. Es la misma tela que sube la pantalla (`Pintor::grano`), para los
/// formatos que la meten como imagen (el SVG y el PDF).
pub fn tela_rgba(g: &pintado::Grano) -> (u32, Vec<u8>) {
    let lado = crate::tinta::material::LADO_DEL_MOSAICO;
    let tejido = crate::tinta::tejido(g.material);
    let canal = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, gg, b) = (canal(g.color.r), canal(g.color.g), canal(g.color.b));
    let alfa = g.color.a.clamp(0.0, 1.0);
    let mut rgba = Vec::with_capacity(tejido.len() * 4);
    for tapa in &tejido {
        rgba.extend_from_slice(&[r, gg, b, (*tapa as f32 * alfa).round() as u8]);
    }
    (lado, rgba)
}

/// **Como se coloca la tela**: la matriz `[a b c d]` (la de SVG y PDF, sin
/// traslacion) que lleva un pixel de la tela al documento. Clavada al origen
/// del documento, no a la figura, igual que la brocha de la pantalla: dos
/// secciones pegadas casan su rayado. Mismos numeros que `Pintor::grano`.
pub fn matriz_de_la_tela(g: &pintado::Grano) -> [f32; 4] {
    let cuanto = g.paso / crate::tinta::material::LADO_DEL_MOSAICO as f32;
    if g.inclinada {
        let (sen, cos) = crate::tinta::material::GRADOS_DEL_GRANO
            .to_radians()
            .sin_cos();
        [cuanto * cos, cuanto * sen, -cuanto * sen, cuanto * cos]
    } else {
        [cuanto, 0.0, 0.0, cuanto]
    }
}

/// **Un PNG sin comprimir**, para la tela del grano dentro de un SVG.
///
/// Sin comprimir y escrito aqui porque la tela son 16x16 pixeles —un
/// kilobyte— y el motor es puro: traer un codificador para esto seria
/// atarlo a una biblioteca de imagen para ahorrar ochocientos bytes. El
/// deflate va en bloques «guardados», que todo lector de PNG entiende.
pub fn png_rgba(ancho: u32, alto: u32, rgba: &[u8]) -> Vec<u8> {
    fn crc(datos: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for b in datos {
            c ^= *b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
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
        salida.extend_from_slice(&crc(&con_tipo).to_be_bytes());
    }
    // Cada fila con su filtro «ninguno» delante.
    let fila = ancho as usize * 4;
    let mut crudo = Vec::with_capacity((fila + 1) * alto as usize);
    for y in 0..alto as usize {
        crudo.push(0);
        let desde = (y * fila).min(rgba.len());
        let hasta = (desde + fila).min(rgba.len());
        crudo.extend_from_slice(&rgba[desde..hasta]);
        // Una imagen mal contada se rellena de transparente, no se sale.
        crudo.resize(crudo.len() + fila - (hasta - desde), 0);
    }
    let mut zlib = vec![0x78, 0x01];
    let mut bloques = crudo.chunks(65_535).peekable();
    if bloques.peek().is_none() {
        zlib.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    while let Some(b) = bloques.next() {
        zlib.push(u8::from(bloques.peek().is_none()));
        let n = b.len() as u16;
        zlib.extend_from_slice(&n.to_le_bytes());
        zlib.extend_from_slice(&(!n).to_le_bytes());
        zlib.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for x in &crudo {
        a = (a + *x as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut salida = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut cabecera = Vec::with_capacity(13);
    cabecera.extend_from_slice(&ancho.to_be_bytes());
    cabecera.extend_from_slice(&alto.to_be_bytes());
    // 8 bits, RGBA, sin entrelazar.
    cabecera.extend_from_slice(&[8, 6, 0, 0, 0]);
    trozo(&mut salida, b"IHDR", &cabecera);
    trozo(&mut salida, b"IDAT", &zlib);
    trozo(&mut salida, b"IEND", &[]);
    salida
}

/// Si una tela no tapa nada: la lisa y las encendidas no tienen tela, y
/// escribir un relleno de nada solo engorda el fichero.
pub fn tela_vacia(g: &pintado::Grano) -> bool {
    crate::tinta::tejido(g.material).iter().all(|b| *b == 0)
}

fn orden_del_papel(papel: (f32, f32)) -> Orden {
    Orden::Imagen {
        id_objeto: ID_PAPEL,
        x: 0.0,
        y: 0.0,
        ancho: papel.0,
        alto: papel.1,
        opacidad: 1.0,
        recorte: None,
        angulo: 0.0,
    }
}

/// **Donde cae la imagen ENTERA** para que su trozo recortado llene la caja
/// `(x, y, ancho, alto)`: `(x, y, ancho, alto)` de la imagen completa, que se
/// sale de la caja por donde el recorte quito.
///
/// Es la cuenta de los formatos que no saben pedir «un trozo de la fuente»
/// (el PDF): pintan la imagen entera, estirada y corrida, y la recortan a la
/// caja. Se hace en pixeles del original (`ancho_natural`), asi que vale para
/// la copia que sea. `None` si el recorte no dice nada util: entonces se
/// pinta la imagen entera en la caja, como en pantalla.
pub fn imagen_entera(
    caja: (f32, f32, f32, f32),
    recorte: &crate::elemento::RecorteImagen,
) -> Option<(f32, f32, f32, f32)> {
    let (x, y, ancho, alto) = caja;
    let (tx0, ty0, tx1, ty1) = recorte.trozo_en(recorte.ancho_natural, recorte.alto_natural)?;
    let (kx, ky) = (ancho / (tx1 - tx0), alto / (ty1 - ty0));
    Some((
        x - tx0 * kx,
        y - ty0 * ky,
        recorte.ancho_natural * kx,
        recorte.alto_natural * ky,
    ))
}

/// **Los puntos de las tablas de coordenadas**, encima de todo y sin el eje,
/// que es andamio de la pantalla (`paraExportar` del `Renderer` del movil):
/// lo tecleado si sale en el PNG, el SVG, el PDF y la web.
fn ordenes_de_las_tablas(escena: &Escena) -> Vec<Orden> {
    crate::instrumento::tabla::ordenes(
        &escena.tablas,
        escena.origen_coordenadas,
        escena.escala.as_ref(),
        None,
        None,
    )
}

/// Lo que ocupan esos puntos, para que el encuadre de «todo» no los corte.
fn caja_de_las_tablas(escena: &Escena) -> Option<(f32, f32, f32, f32)> {
    let r = crate::instrumento::tabla::TABLA_PUNTO * 2.0;
    crate::instrumento::tabla::anclajes(
        &escena.tablas,
        escena.origen_coordenadas,
        escena.escala.as_ref(),
    )
    .into_iter()
    .map(|p| (p.x - r, p.y - r, p.x + r, p.y + r))
    .reduce(union)
}

fn union(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
}

fn con_margen(c: (f32, f32, f32, f32), m: f32) -> (f32, f32, f32, f32) {
    (c.0 - m, c.1 - m, c.2 + m, c.3 + m)
}

fn se_tocan(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// La caja de un elemento tal como se pinta. Una marca de medida (el rotulo
/// de una cota) puede salirse un poco, pero no se persigue: medir el texto
/// exigiria la fuente, y el margen ya da aire.
fn caja_de(e: &Elemento) -> (f32, f32, f32, f32) {
    e.caja()
}

fn es_valida(c: (f32, f32, f32, f32)) -> bool {
    c.2 - c.0 > 0.0 && c.3 - c.1 > 0.0 && [c.0, c.1, c.2, c.3].iter().all(|v| v.is_finite())
}

fn marcos_a_la_vista(escena: &Escena) -> Vec<MarcoALaVista> {
    marco::hojas_en_orden(&escena.elementos)
        .into_iter()
        .filter(|m| es_valida(m.caja()))
        .enumerate()
        .map(|(i, m)| MarcoALaVista {
            n: i + 1,
            nombre: nombre_del_marco(m),
            caja: m.caja(),
        })
        .collect()
}

fn nombre_del_marco(m: &Elemento) -> String {
    match &m.figura {
        crate::elemento::Figura::Marco { nombre } => nombre.clone(),
        _ => String::new(),
    }
}

fn entero(escena: &Escena, papel: Option<(f32, f32)>) -> Option<Hoja> {
    let visibles: Vec<&Elemento> = escena.a_la_vista().collect();
    let contenido = visibles
        .iter()
        .map(|e| caja_de(e))
        .chain(caja_de_las_tablas(escena))
        .reduce(union);
    let caja = match (papel, contenido) {
        // Con papel manda el papel (y lo que se salga de el), sin margen.
        (Some((w, h)), Some(c)) => union((0.0, 0.0, w, h), c),
        (Some((w, h)), None) => (0.0, 0.0, w, h),
        (None, Some(c)) => con_margen(c, MARGEN),
        (None, None) => return None,
    };
    if !es_valida(caja) {
        return None;
    }
    let mut granos = Vec::new();
    let mut grafitos = Vec::new();
    let mut ordenes: Vec<Orden> = papel.map(orden_del_papel).into_iter().collect();
    for e in &visibles {
        meter(escena, e, &mut ordenes, &mut granos, &mut grafitos);
    }
    ordenes.extend(ordenes_de_las_tablas(escena));
    if ordenes.is_empty() && grafitos.is_empty() {
        return None;
    }
    Some(Hoja {
        nombre: String::new(),
        caja,
        ordenes,
        marcos: marcos_a_la_vista(escena),
        granos,
        grafitos,
    })
}

/// **La hoja de una zona** (la herramienta Zona, F8): lo que se ve dentro de
/// `caja` —el papel de fondo y lo dibujado que la toca— recortado a ella.
///
/// Como `DrawEditorActivity.fotoDeLaZona` del movil, sin lo que lleva enlace:
/// la marca de una zona mandada al chat es un boton del lienzo, no dibujo, y
/// no tiene que salir en la foto de al lado. Los marcos tampoco se pintan
/// (solo su papel), igual que al exportar. `None` si la caja no vale o no
/// hay nada que pintar.
pub fn de_una_zona(
    escena: &Escena,
    caja: (f32, f32, f32, f32),
    papel: Option<(f32, f32)>,
) -> Option<Hoja> {
    if !es_valida(caja) {
        return None;
    }
    let papel = papel.filter(|(w, h)| *w > 0.0 && *h > 0.0);
    let mut granos = Vec::new();
    let mut grafitos = Vec::new();
    let mut ordenes: Vec<Orden> = papel
        .filter(|(w, h)| se_tocan((0.0, 0.0, *w, *h), caja))
        .map(orden_del_papel)
        .into_iter()
        .collect();
    for e in escena.a_la_vista() {
        if e.enlace.is_some() || !se_tocan(caja_de(e), caja) {
            continue;
        }
        meter(escena, e, &mut ordenes, &mut granos, &mut grafitos);
    }
    Some(Hoja {
        nombre: String::new(),
        caja,
        ordenes,
        marcos: Vec::new(),
        granos,
        grafitos,
    })
}

/// La hoja de un marco: su papel, y lo que cae entero dentro de el.
/// **Lo que se ve dentro de un marco**: lo suyo (`marco::contenidos`, lo que
/// cabe entero) y ademas todo lo que lo toca. Una raya que entra por un lado
/// y sale por el otro no es del marco, pero su tramo de en medio se ve en el
/// y el usuario espera verlo en el papel; la hoja se recorta a la caja del
/// marco, asi que lo que sobresale no sale. Los otros marcos no cuentan: no
/// se pintan.
fn a_la_vista_en(escena: &Escena, m: &Elemento) -> Vec<u64> {
    let suyos = marco::contenidos(&escena.elementos, m);
    let caja = m.caja();
    escena
        .a_la_vista()
        .filter(|e| e.id != m.id && !marco::es_marco(e))
        .filter(|e| suyos.contains(&e.id) || se_tocan(caja_de(e), caja))
        .map(|e| e.id)
        .collect()
}

fn de_un_marco(escena: &Escena, m: &Elemento, papel: Option<(f32, f32)>) -> Option<Hoja> {
    let caja = m.caja();
    if !es_valida(caja) {
        return None;
    }
    let dentro = a_la_vista_en(escena, m);
    let mut granos = Vec::new();
    let mut grafitos = Vec::new();
    let mut ordenes: Vec<Orden> = papel
        .filter(|(w, h)| se_tocan((0.0, 0.0, *w, *h), caja))
        .map(orden_del_papel)
        .into_iter()
        .collect();
    ordenes.extend(marco::ordenes_del_papel(m));
    // En el orden de pintado de la escena, no en el de `contenidos`: lo que
    // estaba encima en pantalla tiene que seguir encima en el papel.
    for e in escena.a_la_vista().filter(|e| dentro.contains(&e.id)) {
        meter(escena, e, &mut ordenes, &mut granos, &mut grafitos);
    }
    ordenes.extend(ordenes_de_las_tablas(escena));
    Some(Hoja {
        nombre: nombre_del_marco(m),
        caja,
        ordenes,
        marcos: Vec::new(),
        granos,
        grafitos,
    })
}

fn por_marcos(escena: &Escena, papel: Option<(f32, f32)>) -> Vec<Hoja> {
    marco::hojas_en_orden(&escena.elementos)
        .into_iter()
        // Como el movil: un marco vacio no es una pagina, es un hueco. Vacio
        // de verdad: lo que solo lo cruza tambien se ve en el.
        .filter(|m| !a_la_vista_en(escena, m).is_empty())
        .filter_map(|m| de_un_marco(escena, m, papel))
        .collect()
}

fn elegido(escena: &Escena, seleccion: &[u64], papel: Option<(f32, f32)>) -> Option<Hoja> {
    let elegidos: Vec<&Elemento> = escena
        .a_la_vista()
        .filter(|e| seleccion.contains(&e.id))
        .collect();
    // Un marco solo elegido es «esta hoja»: sale recortado a el, como con
    // «cada marco».
    if let [solo] = elegidos.as_slice()
        && marco::es_marco(solo)
    {
        return de_un_marco(escena, solo, papel);
    }
    let ids = marco::con_contenidos(&escena.elementos, seleccion);
    let dentro: Vec<&Elemento> = escena
        .a_la_vista()
        .filter(|e| ids.contains(&e.id))
        .collect();
    let caja = con_margen(dentro.iter().map(|e| caja_de(e)).reduce(union)?, MARGEN);
    if !es_valida(caja) {
        return None;
    }
    let mut granos = Vec::new();
    let mut grafitos = Vec::new();
    let mut ordenes: Vec<Orden> = papel
        .filter(|(w, h)| se_tocan((0.0, 0.0, *w, *h), caja))
        .map(orden_del_papel)
        .into_iter()
        .collect();
    for e in &dentro {
        meter(escena, e, &mut ordenes, &mut granos, &mut grafitos);
    }
    Some(Hoja {
        nombre: String::new(),
        caja,
        ordenes,
        marcos: Vec::new(),
        granos,
        grafitos,
    })
}

/// El tamano en pixeles de una hoja exportada a `escala`, y la escala que de
/// verdad se usa: si no cabe en [`LADO_MAXIMO_PNG`] se reduce, en vez de
/// fallar.
pub fn tamano_png(hoja: &Hoja, escala: f32) -> (u32, u32, f32) {
    let escala = if escala.is_finite() && escala > 0.0 {
        escala
    } else {
        1.0
    };
    let (w, h) = (hoja.ancho().max(1.0), hoja.alto().max(1.0));
    let tope = LADO_MAXIMO_PNG as f32;
    let efectiva = escala.min(tope / w).min(tope / h);
    let px = |v: f32| ((v * efectiva).round() as u32).clamp(1, LADO_MAXIMO_PNG);
    (px(w), px(h), efectiva)
}

/// **El encaje en el papel**, para el PDF y la impresion: la escala y el
/// desplazamiento que meten la hoja centrada en una pagina de `ancho` x `alto`
/// con `margen` a cada lado. Mismas cuentas que `encuadreEnPagina` del movil.
///
/// Devuelve `(escala, dx, dy)` tal que un punto de escena `p` cae en
/// `p * escala + (dx, dy)` de la pagina. `None` si la pagina no deja sitio.
pub fn encaje(hoja: &Hoja, ancho: f32, alto: f32, margen: f32) -> Option<(f32, f32, f32)> {
    let (w, h) = (hoja.ancho(), hoja.alto());
    let (uw, uh) = (ancho - 2.0 * margen, alto - 2.0 * margen);
    if w <= 0.0 || h <= 0.0 || uw <= 0.0 || uh <= 0.0 {
        return None;
    }
    let k = (uw / w).min(uh / h);
    let dx = (ancho - w * k) / 2.0 - hoja.caja.0 * k;
    let dy = (alto - h * k) / 2.0 - hoja.caja.1 * k;
    Some((k, dx, dy))
}

/// Si una hoja es apaisada: decide si su pagina va tumbada.
pub fn apaisada(hoja: &Hoja) -> bool {
    hoja.ancho() > hoja.alto()
}

// ---------------------------------------------------------------------------
// El texto, para los formatos que no tienen DirectWrite a mano
// ---------------------------------------------------------------------------

/// Cuanto separa una linea de la siguiente, en veces el tamano de letra. Es
/// lo que da DirectWrite con Segoe UI, que es con lo que se pinta en
/// pantalla: asi un texto de tres lineas ocupa lo mismo en el PNG, el SVG y
/// el PDF.
pub const INTERLINEA: f32 = 1.33;

/// Donde cae la linea base de la primera linea, desde arriba de la caja.
pub const LINEA_BASE: f32 = 1.08;

/// Anchos de Segoe UI (milesimas de em) de los caracteres 32 a 126.
///
/// Segoe UI es la letra con la que la pantalla pinta todo texto del lienzo
/// (`CreateTextFormat` de `pixpin_render::lienzo`), asi que partir las lineas
/// con sus anchos es partirlas donde las parte la pantalla. Antes eran los
/// de Helvetica —la letra que el PDF usaba sin incrustar— y una nota que en
/// pantalla ocupaba tres lineas salia en el SVG y en el PDF con otra
/// particion: Segoe UI es un siete por ciento mas estrecha.
///
/// Una tabla fija y no medir con la letra porque esto es puro, se prueba sin
/// Windows y no necesita un dispositivo de dibujo para exportar un fichero.
/// Sale de la propia letra (`pixpin_pdf::letra`, la prueba
/// `imprimir_la_tabla_de_anchos`), no de memoria.
const ANCHOS_SEGOE_UI: [u16; 95] = [
    274, 284, 392, 591, 539, 818, 800, 230, 302, 302, 417, 684, 217, 400, 217, 390, 539, 539, 539,
    539, 539, 539, 539, 539, 539, 539, 217, 217, 684, 684, 684, 448, 955, 645, 573, 619, 701, 506,
    488, 686, 710, 266, 357, 580, 471, 898, 748, 754, 560, 754, 598, 531, 524, 687, 621, 934, 590,
    553, 570, 302, 379, 302, 684, 415, 268, 509, 588, 462, 589, 523, 313, 589, 566, 242, 242, 497,
    242, 861, 566, 586, 588, 589, 348, 424, 339, 566, 479, 723, 459, 484, 452, 302, 239, 302, 684,
];

/// Los de 160 a 255: las vocales con tilde, la «n» con virgulilla, los
/// signos de abrir. Sin ellos, un rotulo en castellano media como si cada
/// «a» fuera una «n».
const ANCHOS_SEGOE_UI_LATIN1: [u16; 96] = [
    274, 284, 539, 539, 556, 539, 239, 448, 414, 890, 392, 506, 684, 400, 890, 415, 377, 684, 366,
    366, 282, 577, 458, 217, 205, 351, 431, 506, 906, 931, 952, 448, 645, 645, 645, 645, 645, 645,
    860, 619, 506, 506, 506, 506, 266, 266, 266, 266, 701, 748, 754, 754, 754, 754, 754, 684, 754,
    687, 687, 687, 687, 553, 560, 544, 509, 509, 509, 509, 509, 509, 832, 462, 523, 523, 523, 523,
    242, 242, 242, 242, 559, 566, 586, 586, 586, 586, 586, 684, 586, 566, 566, 566, 566, 484, 588,
    484,
];

/// Lo que mide un caracter en Segoe UI, en milesimas de em. Lo que no esta
/// en las tablas toma el ancho de una letra corriente: una «n», que es lo mas
/// parecido a la media.
pub fn ancho_de_caracter(c: char) -> u16 {
    let n = c as u32;
    match n {
        32..=126 => ANCHOS_SEGOE_UI[(n - 32) as usize],
        160..=255 => ANCHOS_SEGOE_UI_LATIN1[(n - 160) as usize],
        _ => 566,
    }
}

/// Lo que mide `texto` a tamano `tam`.
pub fn ancho_de_texto(texto: &str, tam: f32) -> f32 {
    texto
        .chars()
        .map(|c| ancho_de_caracter(c) as f32)
        .sum::<f32>()
        * tam
        / 1000.0
}

/// El texto partido en lineas que caben en `ancho_max`, midiendo con la
/// tabla de Segoe UI. Ver [`partir_texto_con`].
pub fn partir_texto(texto: &str, tam: f32, ancho_max: f32) -> Vec<String> {
    partir_texto_con(texto, tam, ancho_max, &|c| ancho_de_caracter(c) as f32)
}

/// El texto partido en lineas que caben en `ancho_max`: primero por los
/// saltos que escribio el usuario y despues por palabras. Una palabra que no
/// cabe sola se deja entera en su linea antes que cortarla por la mitad.
///
/// `ancho` da lo que mide cada caracter en milesimas de em: el PDF, que
/// lleva la letra de verdad dentro, mide con ella (tambien lo que no esta en
/// la tabla); lo demas, con la tabla.
pub fn partir_texto_con(
    texto: &str,
    tam: f32,
    ancho_max: f32,
    ancho: &dyn Fn(char) -> f32,
) -> Vec<String> {
    let tope = if ancho_max.is_finite() && ancho_max > 0.0 {
        ancho_max
    } else {
        f32::MAX
    };
    let mide = |s: &str| s.chars().map(ancho).sum::<f32>() * tam / 1000.0;
    let mut salida = Vec::new();
    for parrafo in texto.split('\n') {
        let parrafo = parrafo.trim_end_matches('\r');
        let mut linea = String::new();
        for palabra in parrafo.split(' ') {
            let candidata = if linea.is_empty() {
                palabra.to_string()
            } else {
                format!("{linea} {palabra}")
            };
            if !linea.is_empty() && mide(&candidata) > tope {
                salida.push(std::mem::take(&mut linea));
                linea = palabra.to_string();
            } else {
                linea = candidata;
            }
        }
        salida.push(linea);
    }
    salida
}

/// Un color como `#rrggbb`.
pub fn hex(c: ColorRgba) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b))
}

/// Los puntos del contorno de tinta como tramos: el mismo camino que pinta
/// la pantalla (`pixpin_render::tinta::pasos_de_tinta`), curvas cuadraticas
/// por los puntos medios. Devuelve `(inicio, [(control, fin)], cierre)`.
pub fn curvas_de_tinta(contorno: &[Punto2]) -> Option<(Punto2, Vec<(Punto2, Punto2)>)> {
    let primero = *contorno.first()?;
    let max = contorno.len() - 1;
    let medio = |a: Punto2, b: Punto2| Punto2::nuevo((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let curvas = contorno
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let siguiente = if i == max { primero } else { contorno[i + 1] };
            (p, medio(p, siguiente))
        })
        .collect();
    Some((primero, curvas))
}

/// El patron de rayas de un trazo discontinuo o punteado, en pixeles de
/// escena. Los de Excalidraw (`strokeDasharray`): crecen con el grosor para
/// que un trazo gordo no se vea como una linea continua con muescas.
pub fn rayas(estilo: crate::elemento::EstiloTrazo, grosor: f32) -> Option<(f32, f32)> {
    use crate::elemento::EstiloTrazo;
    match estilo {
        EstiloTrazo::Solido => None,
        EstiloTrazo::Discontinuo => Some((8.0, 8.0 + grosor)),
        EstiloTrazo::Punteado => Some((1.5, 6.0 + grosor)),
        // El motor lo entrega ya partido en tramos lisos
        // (`geometria_exacta::desplegar_raya_punto`); si alguna orden llegara
        // entera, a rayas largas, que es lo mas parecido.
        EstiloTrazo::RayaPunto => Some((5.0 * grosor + 4.0, 2.0 * grosor + 2.0)),
    }
}

/// Base64 estandar, con relleno. Para meter imagenes en un SVG o en una
/// pagina web sin traer una biblioteca entera por veinte lineas.
pub fn base64(datos: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(datos.len().div_ceil(3) * 4);
    for trozo in datos.chunks(3) {
        let n = (trozo[0] as u32) << 16
            | (*trozo.get(1).unwrap_or(&0) as u32) << 8
            | *trozo.get(2).unwrap_or(&0) as u32;
        s.push(ABC[(n >> 18) as usize & 63] as char);
        s.push(ABC[(n >> 12) as usize & 63] as char);
        s.push(if trozo.len() > 1 {
            ABC[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if trozo.len() > 2 {
            ABC[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::Figura;

    fn rect(id: u64, x: f32, y: f32, w: f32, h: f32) -> Elemento {
        Elemento {
            id,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho: w,
            alto: h,
            ..Elemento::default()
        }
    }

    fn marco_en(id: u64, x: f32, y: f32, w: f32, h: f32, nombre: &str) -> Elemento {
        Elemento {
            figura: Figura::Marco {
                nombre: nombre.into(),
            },
            ..rect(id, x, y, w, h)
        }
    }

    fn escena_con(elementos: Vec<Elemento>) -> Escena {
        let mut e = Escena::nueva();
        for x in elementos {
            e.anadir(x);
        }
        e
    }

    #[test]
    fn todo_encuadra_lo_visible_con_su_margen() {
        let e = escena_con(vec![rect(0, 10.0, 20.0, 100.0, 50.0)]);
        let h = hojas(&e, Alcance::Todo, &[], None);
        assert_eq!(h.len(), 1);
        let (x0, y0, x1, y1) = h[0].caja;
        let (cx0, cy0, cx1, cy1) = e.elementos[0].caja();
        assert_eq!(
            (x0, y0, x1, y1),
            (cx0 - MARGEN, cy0 - MARGEN, cx1 + MARGEN, cy1 + MARGEN)
        );
        assert!(!h[0].ordenes.is_empty());
    }

    #[test]
    fn un_lienzo_vacio_no_da_ninguna_hoja() {
        // Caso negativo: nada que exportar es una lista vacia, no una hoja en
        // blanco que se guardaria como un PNG de un pixel.
        assert!(hojas(&Escena::nueva(), Alcance::Todo, &[], None).is_empty());
        assert!(hojas(&Escena::nueva(), Alcance::Marcos, &[], None).is_empty());
        assert!(hojas(&Escena::nueva(), Alcance::Seleccion, &[1], None).is_empty());
    }

    #[test]
    fn cada_marco_con_algo_dentro_es_una_hoja_en_su_orden_y_los_vacios_no() {
        let e = escena_con(vec![
            marco_en(0, 0.0, 500.0, 300.0, 200.0, "Alzado"),
            marco_en(0, 0.0, 0.0, 300.0, 200.0, "Planta"),
            marco_en(0, 0.0, 1000.0, 300.0, 200.0, "Vacio"),
            rect(0, 20.0, 20.0, 50.0, 50.0),
            rect(0, 20.0, 520.0, 50.0, 50.0),
        ]);
        let h = hojas(&e, Alcance::Marcos, &[], None);
        let nombres: Vec<&str> = h.iter().map(|x| x.nombre.as_str()).collect();
        assert_eq!(nombres, ["Planta", "Alzado"]);
        // Recortada al marco, sin margen.
        assert_eq!(h[0].caja, (0.0, 0.0, 300.0, 200.0));
    }

    #[test]
    fn lo_que_cruza_el_marco_sale_en_su_hoja_y_lo_de_fuera_no() {
        // Una raya larga que entra por un lado y sale por el otro: no es del
        // marco (no cabe entera), pero su tramo de en medio se ve en el y
        // tiene que salir en el papel, recortado a la hoja.
        let e = escena_con(vec![
            marco_en(0, 0.0, 0.0, 300.0, 200.0, "Planta"),
            rect(0, -100.0, 90.0, 500.0, 20.0),
            // Lejos del marco: no entra en su hoja.
            rect(0, 900.0, 900.0, 40.0, 40.0),
            // Otro marco que solo tiene algo que lo cruza tambien es hoja.
            marco_en(0, 0.0, 500.0, 300.0, 200.0, "Alzado"),
            rect(0, 250.0, 450.0, 200.0, 100.0),
        ]);
        let h = hojas(&e, Alcance::Marcos, &[], None);
        let nombres: Vec<&str> = h.iter().map(|x| x.nombre.as_str()).collect();
        assert_eq!(nombres, ["Planta", "Alzado"]);
        let solo_planta = escena_con(vec![
            marco_en(0, 0.0, 0.0, 300.0, 200.0, "Planta"),
            rect(0, -100.0, 90.0, 500.0, 20.0),
        ]);
        let con_lejano = escena_con(vec![
            marco_en(0, 0.0, 0.0, 300.0, 200.0, "Planta"),
            rect(0, -100.0, 90.0, 500.0, 20.0),
            rect(0, 900.0, 900.0, 40.0, 40.0),
        ]);
        let a = &hojas(&solo_planta, Alcance::Marcos, &[], None)[0];
        let b = &hojas(&con_lejano, Alcance::Marcos, &[], None)[0];
        assert!(
            !a.ordenes.is_empty(),
            "la raya que cruza tiene que pintarse"
        );
        assert_eq!(a.ordenes, b.ordenes, "lo que no toca el marco no entra");
        assert_eq!(a.caja, (0.0, 0.0, 300.0, 200.0), "recortada al marco");
    }

    #[test]
    fn sin_marcos_cada_marco_es_lo_mismo_que_todo() {
        let e = escena_con(vec![rect(0, 0.0, 0.0, 10.0, 10.0)]);
        assert_eq!(
            hojas(&e, Alcance::Marcos, &[], None),
            hojas(&e, Alcance::Todo, &[], None)
        );
    }

    #[test]
    fn el_marco_no_se_pinta_a_si_mismo_al_exportar() {
        // El recuadro y el rotulo del marco son de la pantalla: en el papel
        // no sale ni una raya suya si no tiene papel pautado.
        let m = marco_en(0, 0.0, 0.0, 300.0, 200.0, "Planta");
        let e = escena_con(vec![m]);
        let solo = e.elementos[0].clone();
        assert!(ordenes_al_exportar(&e, &solo).is_empty());
        let todo = hojas(&e, Alcance::Todo, &[], None);
        assert!(
            todo.is_empty(),
            "un lienzo con solo un marco vacio no exporta nada"
        );
    }

    #[test]
    fn la_hoja_entera_lleva_la_lista_de_marcos_para_la_web() {
        let e = escena_con(vec![
            marco_en(0, 0.0, 0.0, 300.0, 200.0, "Planta"),
            rect(0, 20.0, 20.0, 50.0, 50.0),
        ]);
        let h = hojas(&e, Alcance::Todo, &[], None);
        assert_eq!(h[0].marcos.len(), 1);
        assert_eq!(h[0].marcos[0].n, 1);
        assert_eq!(h[0].marcos[0].nombre, "Planta");
    }

    #[test]
    fn lo_elegido_sale_solo_y_un_marco_elegido_sale_recortado() {
        let e = escena_con(vec![
            marco_en(0, 0.0, 0.0, 300.0, 200.0, "Planta"),
            rect(0, 20.0, 20.0, 50.0, 50.0),
            rect(0, 1000.0, 1000.0, 50.0, 50.0),
        ]);
        let ids: Vec<u64> = e.elementos.iter().map(|x| x.id).collect();
        let lejos = hojas(&e, Alcance::Seleccion, &[ids[2]], None);
        assert_eq!(lejos.len(), 1);
        assert!(lejos[0].caja.0 > 900.0, "no se lleva lo que no se eligio");
        let marco = hojas(&e, Alcance::Seleccion, &[ids[0]], None);
        assert_eq!(marco[0].caja, (0.0, 0.0, 300.0, 200.0));
        // Caso negativo: nada elegido, nada que exportar.
        assert!(hojas(&e, Alcance::Seleccion, &[], None).is_empty());
    }

    #[test]
    fn con_papel_manda_el_papel_y_va_debajo_de_todo() {
        let e = escena_con(vec![rect(0, 10.0, 10.0, 20.0, 20.0)]);
        let h = hojas(&e, Alcance::Todo, &[], Some((800.0, 600.0)));
        assert_eq!(h[0].caja, (0.0, 0.0, 800.0, 600.0));
        assert!(matches!(
            h[0].ordenes.first(),
            Some(Orden::Imagen {
                id_objeto: ID_PAPEL,
                ..
            })
        ));
        // Y sin trazos, el papel solo sigue siendo algo que exportar.
        let solo = hojas(&Escena::nueva(), Alcance::Todo, &[], Some((800.0, 600.0)));
        assert_eq!(solo.len(), 1);
    }

    #[test]
    fn una_escala_enorme_se_reduce_en_vez_de_pedir_un_png_imposible() {
        let e = escena_con(vec![rect(0, 0.0, 0.0, 6000.0, 100.0)]);
        let h = &hojas(&e, Alcance::Todo, &[], None)[0];
        let (w, _, k) = tamano_png(h, 3.0);
        assert!(w <= LADO_MAXIMO_PNG);
        assert!(k < 3.0);
        let (w1, _, k1) = tamano_png(h, 1.0);
        assert_eq!(k1, 1.0);
        assert_eq!(w1, h.ancho().round() as u32);
        // Caso negativo: una escala absurda no da un tamano de cero.
        assert_eq!(tamano_png(h, f32::NAN).2, 1.0);
    }

    #[test]
    fn el_encaje_centra_la_hoja_en_la_pagina_con_su_margen() {
        let h = Hoja {
            nombre: String::new(),
            caja: (100.0, 100.0, 300.0, 200.0),
            ordenes: Vec::new(),
            marcos: Vec::new(),
            granos: Vec::new(),
            grafitos: Vec::new(),
        };
        let (k, dx, dy) = encaje(&h, 842.0, 595.0, 28.0).expect("cabe");
        // Manda el ancho: (842 - 56) / 200.
        assert!((k - 3.93).abs() < 0.01);
        let izquierda = 100.0 * k + dx;
        let derecha = 300.0 * k + dx;
        assert!((izquierda - 28.0).abs() < 0.01 && (derecha - 814.0).abs() < 0.01);
        let arriba = 100.0 * k + dy;
        let abajo = 200.0 * k + dy;
        assert!(
            ((arriba + abajo) / 2.0 - 297.5).abs() < 0.01,
            "centrada en alto"
        );
        assert!(
            encaje(&h, 40.0, 40.0, 28.0).is_none(),
            "sin sitio no hay encaje"
        );
    }

    #[test]
    fn el_texto_se_parte_por_saltos_y_por_palabras() {
        assert_eq!(partir_texto("uno\ndos", 20.0, 1000.0), ["uno", "dos"]);
        let v = partir_texto("una frase bastante larga", 20.0, 100.0);
        assert!(v.len() > 1);
        assert!(v.iter().all(|l| !l.starts_with(' ')));
        // Caso negativo: una palabra que no cabe sola no se corta.
        assert_eq!(partir_texto("supercalifragilistico", 20.0, 10.0).len(), 1);
    }

    #[test]
    fn base64_da_lo_de_la_norma() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    fn recorte(x: f32, y: f32, w: f32, h: f32, nw: f32, nh: f32) -> crate::RecorteImagen {
        crate::RecorteImagen {
            x,
            y,
            ancho: w,
            alto: h,
            ancho_natural: nw,
            alto_natural: nh,
        }
    }

    #[test]
    fn el_recorte_se_lleva_a_una_copia_de_otro_tamano_y_uno_roto_no_recorta() {
        // La mitad izquierda de un original de 200x100, sobre una copia de
        // 100x50: la mitad izquierda de la copia.
        let r = recorte(0.0, 0.0, 100.0, 100.0, 200.0, 100.0);
        assert_eq!(r.trozo_en(100.0, 50.0), Some((0.0, 0.0, 50.0, 50.0)));
        // Lo que se sale del original se queda en el borde.
        let fuera = recorte(150.0, 0.0, 100.0, 100.0, 200.0, 100.0);
        assert_eq!(
            fuera.trozo_en(200.0, 100.0),
            Some((150.0, 0.0, 200.0, 100.0))
        );
        // Casos negativos: sin tamano natural, sin area o fuera del todo no
        // hay trozo, y se pinta la foto entera en vez de nada.
        assert_eq!(
            recorte(0.0, 0.0, 10.0, 10.0, 0.0, 0.0).trozo_en(100.0, 100.0),
            None
        );
        assert_eq!(
            recorte(0.0, 0.0, 0.0, 10.0, 20.0, 20.0).trozo_en(100.0, 100.0),
            None
        );
        assert_eq!(
            recorte(500.0, 0.0, 10.0, 10.0, 20.0, 20.0).trozo_en(100.0, 100.0),
            None
        );
        assert_eq!(
            recorte(f32::NAN, 0.0, 10.0, 10.0, 20.0, 20.0).trozo_en(100.0, 100.0),
            None
        );
    }

    #[test]
    fn la_imagen_entera_se_estira_para_que_su_trozo_llene_la_caja() {
        // El cuarto de abajo a la derecha de un original de 100x100, en una
        // caja de 50x50 en (10, 20): la imagen entera mide 100x100 y empieza
        // 50 mas arriba y a la izquierda.
        let r = recorte(50.0, 50.0, 50.0, 50.0, 100.0, 100.0);
        assert_eq!(
            imagen_entera((10.0, 20.0, 50.0, 50.0), &r),
            Some((-40.0, -30.0, 100.0, 100.0))
        );
        // Caso negativo: un recorte roto no mueve la imagen.
        assert_eq!(
            imagen_entera((0.0, 0.0, 5.0, 5.0), &recorte(0.0, 0.0, 0.0, 0.0, 1.0, 1.0)),
            None
        );
    }

    #[test]
    fn el_crop_de_un_fichero_llega_a_la_orden_de_la_imagen_y_vuelve_intacto() {
        let json = r#"{"type":"excalidraw","version":2,"elements":[
            {"id":"f","type":"image","x":0,"y":0,"width":50,"height":50,"fileId":"a",
             "crop":{"x":10,"y":0,"width":20,"height":20,"naturalWidth":40,"naturalHeight":40}}],
            "appState":{},"files":{}}"#;
        let lienzo = crate::excalidraw::leer(json).expect("se lee");
        let escena = crate::excalidraw::a_escena(&lienzo);
        let h = hojas(&escena, Alcance::Todo, &[], None);
        let Some(Orden::Imagen {
            recorte: Some(r), ..
        }) = h[0].ordenes.first()
        else {
            panic!("la imagen sale con su recorte: {:?}", h[0].ordenes);
        };
        assert_eq!((r.x, r.ancho, r.ancho_natural), (10.0, 20.0, 40.0));
        // Y al guardar, el `crop` es el del fichero y las diez claves del
        // movil no aparecen por haberlo leido.
        let salida = crate::excalidraw::escribir(&crate::excalidraw::con_escena(&lienzo, &escena));
        assert_eq!(salida.matches("\"crop\"").count(), 1);
        assert!(
            salida.contains("\"naturalWidth\": 40") && !salida.contains("40.0"),
            "sin reescribir: {salida}"
        );
        assert!(
            !salida.contains("presionFirme"),
            "leer el recorte no ensucia: {salida}"
        );
        // Caso negativo: una imagen sin `crop` no inventa ninguno.
        let sin = json.replace(
            r#""crop":{"x":10,"y":0,"width":20,"height":20,"naturalWidth":40,"naturalHeight":40}"#,
            r#""otra":{}"#,
        );
        assert_ne!(sin, json, "el crop se quito de verdad");
        let e2 = crate::excalidraw::a_escena(&crate::excalidraw::leer(&sin).expect("se lee"));
        assert!(e2.elementos[0].extras.recorte.is_none());
    }

    fn trazo_de(material: crate::tinta::MaterialTinta, x: f32) -> Elemento {
        Elemento {
            figura: Figura::Lapiz {
                puntos: (0..20)
                    .map(|i| Punto2::nuevo(x + i as f32 * 5.0, 10.0 + (i % 3) as f32))
                    .collect(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            grosor: 6.0,
            material,
            ..Elemento::default()
        }
    }

    #[test]
    fn las_tintas_porosas_apuntan_su_grano_y_la_lisa_no() {
        use crate::tinta::MaterialTinta;
        let e = escena_con(vec![
            trazo_de(MaterialTinta::Lisa, 0.0),
            trazo_de(MaterialTinta::Rayado, 200.0),
        ]);
        let h = &hojas(&e, Alcance::Todo, &[], None)[0];
        assert_eq!(h.granos.len(), 1, "solo el rayado: {:?}", h.granos);
        let (i, g) = h.granos[0];
        assert!(
            matches!(h.ordenes[i], Orden::Tinta { .. }),
            "el grano va con la silueta"
        );
        assert_eq!(g.material, MaterialTinta::Rayado);
        assert_eq!(grano_de_la_orden(h, i), Some(g));
        // Caso negativo: la orden de al lado no tiene grano.
        assert_eq!(grano_de_la_orden(h, i + 1), None);
        // Y con lo elegido o por marcos, el indice sigue siendo el de su hoja.
        let ids: Vec<u64> = e.elementos.iter().map(|x| x.id).collect();
        let solo = &hojas(&e, Alcance::Seleccion, &[ids[1]], None)[0];
        assert!(matches!(
            solo.ordenes[solo.granos[0].0],
            Orden::Tinta { .. }
        ));
    }

    #[test]
    fn la_tela_lleva_la_cobertura_en_el_alfa_y_gira_con_el_rayado() {
        use crate::tinta::MaterialTinta;
        let e = trazo_de(MaterialTinta::Rayado, 0.0);
        let g = pintado::grano_de(&e).expect("tiene grano");
        let (lado, rgba) = tela_rgba(&g);
        assert_eq!(rgba.len(), (lado * lado * 4) as usize);
        assert!(
            rgba.chunks_exact(4).any(|p| p[3] == 0) && rgba.chunks_exact(4).any(|p| p[3] > 100)
        );
        let [a, b, c, d] = matriz_de_la_tela(&g);
        assert!(
            (a - d).abs() < 1e-6 && (b + c).abs() < 1e-6 && b > 0.0,
            "girada 45 grados"
        );
        let puntos = pintado::grano_de(&trazo_de(MaterialTinta::Puntos, 0.0)).expect("puntos");
        assert_eq!(matriz_de_la_tela(&puntos)[1], 0.0, "los puntos no se giran");
        // Caso negativo: la lisa no tiene grano que estampar.
        assert!(pintado::grano_de(&trazo_de(MaterialTinta::Lisa, 0.0)).is_none());
    }

    #[test]
    fn el_png_de_la_tela_es_un_png_bien_formado() {
        let png = png_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128]);
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(png.ends_with(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82]));
        // El IHDR con su CRC de la norma (2x1, RGBA de 8 bits).
        assert_eq!(&png[8..16], &[0, 0, 0, 13, b'I', b'H', b'D', b'R']);
        // Caso negativo: pixeles de menos no se salen de la imagen.
        let corto = png_rgba(4, 4, &[1, 2, 3]);
        assert!(corto.len() > 60);
    }
}
