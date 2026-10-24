//! **Un lienzo como PDF de vectores**, escrito aqui mismo.
//!
//! Por que no `ID2D1PrintControl` con «Microsoft Print to PDF», que es lo que
//! Windows trae: esa via pasa por la cola de impresion (un servicio del
//! sistema, un trabajo en la cola, un controlador que puede estar quitado en
//! Windows N o en equipos de empresa) y el fichero sale cuando el spooler
//! termina, no cuando vuelve la llamada. Para imprimir en papel si es la via
//! (ver `pixpin_render::imprimir`), pero para **escribir un fichero** es
//! mucha maquinaria para lo que hace falta: un PDF de caminos rellenos y
//! trazados es texto, y el movil tampoco hace otra cosa (`DrawPdf.kt` pinta
//! con el lienzo del sistema sobre `PdfDocument`, que escribe lo mismo).
//!
//! Lo que sale es lo que promete el movil: **trazos y texto de verdad**, no
//! una imagen metida en un PDF. Se amplia sin pixelarse, se imprime a la
//! escala que sea y pesa una fraccion del PNG. Una hoja del lienzo es una
//! pagina A4, apaisada si la hoja lo es, con la hoja encajada y centrada con
//! un dedo de margen (`DrawPdf.aArchivo`).
//!
//! Las ordenes son las mismas que pinta la pantalla (`pintado::ordenes`): no
//! hay un segundo dibujante que pueda discrepar del primero. El texto va en
//! la letra de la pantalla, Segoe UI, incrustada con solo las letras que se
//! usan (`letra`, [`de_hojas_con_letra`]): con ella las lineas se parten
//! donde las parte la pantalla y la cara es la misma. Si no se puede leer,
//! Helvetica sin incrustar, que todo lector trae puesta.
//!
//! El grano de las tintas porosas va como un patron de mosaico con la misma
//! tela de la pantalla, y las imagenes recortadas (`crop`) ensenan solo su
//! trozo.

use crate::letra::Letra;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use pixpin_motor2d::ColorRgba;
use pixpin_motor2d::exportar::{self, Hoja};
use pixpin_motor2d::pintado::{Grano, Orden};
use pixpin_motor2d::vector::Punto2;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;

/// A4 en puntos (1/72 de pulgada), que es la unidad del PDF.
pub const A4: (f32, f32) = (595.28, 841.89);

/// Margen de la pagina, en puntos: un dedo, como el PDF del movil.
pub const MARGEN: f32 = 28.0;

/// Los pixeles de una imagen del lienzo, RGBA sin premultiplicar.
pub struct Pixeles {
    pub ancho: u32,
    pub alto: u32,
    pub rgba: Vec<u8>,
}

/// Un numero corto: dos decimales, sin ceros de sobra. Los caminos de un
/// dibujo a mano son miles de numeros y cada caracter cuenta.
fn n(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let r = (v * 100.0).round() / 100.0;
    if r == r.trunc() {
        return format!("{}", r as i64);
    }
    let s = format!("{r:.2}");
    s.trim_end_matches('0').to_string()
}

fn rgb(c: ColorRgba) -> String {
    format!(
        "{} {} {}",
        n(c.r.clamp(0.0, 1.0)),
        n(c.g.clamp(0.0, 1.0)),
        n(c.b.clamp(0.0, 1.0))
    )
}

/// Una transparencia, en 255 escalones: el nombre de su estado grafico.
fn alfa(a: f32) -> u8 {
    (a.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Lo que va juntando una pagina mientras se escribe: los estados de
/// transparencia y las imagenes que usa, para declararlos en sus recursos.
#[derive(Default)]
struct Recursos {
    alfas: std::collections::BTreeSet<u8>,
    imagenes: std::collections::BTreeSet<u64>,
    /// Los glifos de cada letra incrustada que usa la pagina (por su numero
    /// en el [`Juego`]), con el caracter que dibujan: para que copiar el
    /// texto del PDF de letras y no numeros.
    glifos: BTreeMap<usize, BTreeMap<u16, char>>,
    /// Las telas de grano que usa la pagina, en el orden de su nombre
    /// (`/G0`, `/G1`...), sin repetir.
    telas: Vec<Grano>,
    /// De escena a papel en esta pagina, `[a b c d e f]`. El patron de una
    /// tela se coloca contra el papel y no contra la matriz en curso (asi lo
    /// dice la norma del PDF), asi que la necesita para clavarse al dibujo.
    a_papel: [f32; 6],
}

impl Recursos {
    /// El nombre de la tela de `g` en esta pagina, apuntandola si es nueva.
    fn tela(&mut self, g: Grano) -> usize {
        match self.telas.iter().position(|t| *t == g) {
            Some(i) => i,
            None => {
                self.telas.push(g);
                self.telas.len() - 1
            }
        }
    }
}

impl Recursos {
    /// Pone la transparencia `a` si hace falta. Opaco no escribe nada, que
    /// es el caso de casi todo.
    fn alfa(&mut self, a: f32, c: &mut String) {
        let k = alfa(a);
        if k < 255 {
            self.alfas.insert(k);
            let _ = write!(c, "/A{k} gs ");
        }
    }
}

fn camino_poligonal(puntos: &[Punto2], cerrar: bool, c: &mut String) {
    for (i, p) in puntos.iter().enumerate() {
        let _ = write!(
            c,
            "{} {} {} ",
            n(p.x),
            n(p.y),
            if i == 0 { "m" } else { "l" }
        );
    }
    if cerrar {
        c.push_str("h ");
    }
}

/// El contorno de tinta, con las curvas por los puntos medios de la
/// pantalla. El PDF no tiene cuadraticas: cada una se escribe como la cubica
/// que traza la misma curva (los controles a dos tercios).
fn camino_de_tinta(contorno: &[Punto2], c: &mut String) {
    let Some((inicio, curvas)) = exportar::curvas_de_tinta(contorno) else {
        return;
    };
    let _ = write!(c, "{} {} m ", n(inicio.x), n(inicio.y));
    let mut actual = inicio;
    for (q, f) in curvas {
        let c1 = Punto2::nuevo(
            actual.x + 2.0 / 3.0 * (q.x - actual.x),
            actual.y + 2.0 / 3.0 * (q.y - actual.y),
        );
        let c2 = Punto2::nuevo(f.x + 2.0 / 3.0 * (q.x - f.x), f.y + 2.0 / 3.0 * (q.y - f.y));
        let _ = write!(
            c,
            "{} {} {} {} {} {} c ",
            n(c1.x),
            n(c1.y),
            n(c2.x),
            n(c2.y),
            n(f.x),
            n(f.y)
        );
        actual = f;
    }
    let _ = write!(c, "{} {} l h ", n(inicio.x), n(inicio.y));
}

/// El byte de `ch` en la codificacion de Helvetica (WinAnsi), si lo tiene.
fn winansi(ch: char) -> Option<u8> {
    Some(match ch as u32 {
        0x20..=0x7e => ch as u8,
        0xa0..=0xff => ch as u32 as u8,
        0x20ac => 0x80,
        0x2018 => 0x91,
        0x2019 => 0x92,
        0x201c => 0x93,
        0x201d => 0x94,
        0x2022 => 0x95,
        0x2013 => 0x96,
        0x2014 => 0x97,
        0x2026 => 0x85,
        _ => return None,
    })
}

/// Un byte de WinAnsi escapado para ir entre parentesis.
fn byte_pdf(b: u8, o: &mut String) {
    match b {
        b'(' | b')' | b'\\' => {
            o.push('\\');
            o.push(b as char);
        }
        0x20..=0x7e => o.push(b as char),
        // Lo de fuera de ASCII, en octal: el contenido va como bytes y una
        // cadena de Rust no puede llevar un byte suelto.
        _ => {
            let _ = write!(o, "\\{b:03o}");
        }
    }
}

/// Lo que no se pinta si ninguna letra lo tiene: selectores de variante
/// (el `U+FE0F` que va detras de muchos emoji), uniones de anchura cero y
/// etiquetas. Un cuadrito por cada uno ensuciaria el texto sin decir nada.
fn invisible(ch: char) -> bool {
    matches!(ch as u32,
        0x200b..=0x200f | 0x2060..=0x2064 | 0xfe00..=0xfe0f | 0xfeff
        | 0xe0000..=0xe007f | 0xe0100..=0xe01ef)
        || ch.is_control()
}

/// Con que se escribe un caracter.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pieza {
    /// En Helvetica sin incrustar, con su byte de WinAnsi.
    Helvetica(u8),
    /// En una letra incrustada, por su numero de glifo: la `0` es la del
    /// texto y la `1 + i`, la reserva `i`. El glifo `0` es el hueco
    /// (`.notdef`): lo que no tiene ninguna letra.
    Glifo(usize, u16),
    /// No se escribe ([`invisible`]).
    Nada,
}

/// **Las letras de un documento**: la del texto (Segoe UI, o Helvetica si no
/// se pudo leer) y las de reserva para lo que ella no tiene, como hace
/// DirectWrite al pintar la pantalla. Cada caracter se resuelve una vez.
struct Juego<'a> {
    propia: Option<&'a Letra>,
    reservas: crate::letra::Reservas,
    elegidas: std::collections::HashMap<char, Pieza>,
}

impl<'a> Juego<'a> {
    fn nuevo(propia: Option<&'a Letra>, reservas: crate::letra::Reservas) -> Juego<'a> {
        Juego {
            propia,
            reservas,
            elegidas: Default::default(),
        }
    }

    /// La letra incrustada `i` (ver [`Pieza::Glifo`]).
    fn letra(&self, i: usize) -> Option<&Letra> {
        match i {
            0 => self.propia,
            _ => self.reservas.letra(i - 1),
        }
    }

    fn pieza(&mut self, ch: char) -> Pieza {
        if let Some(p) = self.elegidas.get(&ch) {
            return *p;
        }
        let p = self.elegir(ch);
        self.elegidas.insert(ch, p);
        p
    }

    fn elegir(&mut self, ch: char) -> Pieza {
        match self.propia {
            Some(l) => {
                if let Some(g) = l.glifo(ch) {
                    return Pieza::Glifo(0, g);
                }
            }
            None => {
                if let Some(b) = winansi(ch) {
                    return Pieza::Helvetica(b);
                }
            }
        }
        if let Some((i, g)) = self.reservas.para(ch) {
            return Pieza::Glifo(i + 1, g);
        }
        if invisible(ch) {
            return Pieza::Nada;
        }
        // Lo que no tiene nadie: el hueco de la letra, un cuadrito que dice
        // que ahi habia algo. Helvetica no tiene uno que se vea, asi que sale
        // el de la primera reserva; sin ninguna, el `?` es lo unico que queda.
        if self.propia.is_some() {
            return Pieza::Glifo(0, 0);
        }
        match self.reservas.primera() {
            Some(i) => Pieza::Glifo(i + 1, 0),
            None => Pieza::Helvetica(b'?'),
        }
    }

    /// Lo que avanza `ch`, en milesimas de em, en la letra con que se va a
    /// escribir. Hay que haberlo resuelto antes ([`Juego::pieza`]).
    fn ancho(&self, ch: char) -> f32 {
        match self.elegidas.get(&ch) {
            Some(Pieza::Glifo(i, g)) => self.letra(*i).map_or(0.0, |l| l.avance(*g)),
            Some(Pieza::Nada) => 0.0,
            Some(Pieza::Helvetica(_)) | None => exportar::ancho_de_caracter(ch) as f32,
        }
    }

    /// El texto partido en lineas como lo parte la pantalla.
    fn partir(&mut self, texto: &str, tam: f32, ancho_max: f32) -> Vec<String> {
        for ch in texto.chars() {
            self.pieza(ch);
        }
        match self.propia {
            Some(_) => exportar::partir_texto_con(texto, tam, ancho_max, &|ch| self.ancho(ch)),
            None => exportar::partir_texto(texto, tam, ancho_max),
        }
    }

    /// **Un renglon como ordenes de texto**, cambiando de letra donde hace
    /// falta (`/F1` Helvetica, `/F2` la del texto, `/F3`... las reservas):
    /// cada tramo de una misma letra en un solo `Tj`, que avanza lo suyo y
    /// deja el siguiente justo detras. Apunta en `r` los glifos usados.
    fn renglon(&mut self, linea: &str, tam: f32, r: &mut Recursos) -> String {
        let mut c = String::new();
        // La letra del tramo abierto y si es Helvetica (entre parentesis) o
        // incrustada (en hexadecimal, `Identity-H`).
        let mut abierta: Option<(usize, bool)> = None;
        let cerrar = |c: &mut String, a: Option<(usize, bool)>| {
            if let Some((_, helvetica)) = a {
                c.push_str(if helvetica { ") Tj " } else { "> Tj " });
            }
        };
        for ch in linea.chars() {
            let (fuente, helvetica) = match self.pieza(ch) {
                Pieza::Nada => continue,
                Pieza::Helvetica(_) => (1, true),
                Pieza::Glifo(i, g) => {
                    r.glifos.entry(i).or_default().entry(g).or_insert(ch);
                    (i + 2, false)
                }
            };
            if abierta.map(|(f, _)| f) != Some(fuente) {
                cerrar(&mut c, abierta);
                let _ = write!(
                    c,
                    "/F{fuente} {} Tf {}",
                    n(tam),
                    if helvetica { '(' } else { '<' }
                );
                abierta = Some((fuente, helvetica));
            }
            match self.pieza(ch) {
                Pieza::Helvetica(b) => byte_pdf(b, &mut c),
                Pieza::Glifo(_, g) => {
                    let _ = write!(c, "{g:04X}");
                }
                Pieza::Nada => {}
            }
        }
        cerrar(&mut c, abierta);
        c
    }
}

/// Las ordenes de una hoja como contenido de pagina.
fn contenido(
    hoja: &Hoja,
    pagina: (f32, f32),
    fondo: Option<ColorRgba>,
    hay_imagen: &mut dyn FnMut(u64) -> bool,
    r: &mut Recursos,
    juego: &mut Juego,
    margen: f32,
) -> String {
    let mut c = String::with_capacity(hoja.ordenes.len() * 256);
    let Some((k, dx, dy)) = exportar::encaje(hoja, pagina.0, pagina.1, margen) else {
        return c;
    };
    // De escena a papel: escalar, correr y voltear la Y, porque el papel
    // mide de abajo arriba y la escena de arriba abajo.
    r.a_papel = [k, 0.0, 0.0, -k, dx, pagina.1 - dy];
    let _ = writeln!(
        c,
        "q {:.5} 0 0 {:.5} {} {} cm",
        k,
        -k,
        n(dx),
        n(pagina.1 - dy)
    );
    let (x0, y0, x1, y1) = hoja.caja;
    // Lo que sobresale de la hoja no se ve, como en el PNG.
    let _ = writeln!(
        c,
        "{} {} {} {} re W n",
        n(x0),
        n(y0),
        n(x1 - x0),
        n(y1 - y0)
    );
    if let Some(f) = fondo {
        c.push_str("q ");
        r.alfa(f.a, &mut c);
        let _ = writeln!(
            c,
            "{} rg {} {} {} {} re f Q",
            rgb(f),
            n(x0),
            n(y0),
            n(x1 - x0),
            n(y1 - y0)
        );
    }
    // El grafito, cada mapa justo antes de la orden que le toca; lo que va
    // despues de la ultima, al acabar (`grafitos_al_final`).
    let mut grafitos = hoja.grafitos.iter().peekable();
    for (i, o) in hoja.ordenes.iter().enumerate() {
        while let Some(g) = grafitos.next_if(|g| g.antes_de <= i) {
            grafito(g, hay_imagen, r, &mut c);
        }
        // El grano va encima del cuerpo: se escribe al acabar la orden (ver
        // el final del bucle), y las que no dibujan nada salen con `continue`
        // antes de llegar.
        let grano = exportar::grano_de_la_orden(hoja, i);
        match o {
            Orden::Poligono { puntos, color } | Orden::Relleno { puntos, color } => {
                if puntos.len() < 3 {
                    continue;
                }
                c.push_str("q ");
                r.alfa(color.a, &mut c);
                let _ = write!(c, "{} rg ", rgb(*color));
                camino_poligonal(puntos, true, &mut c);
                c.push_str("f Q\n");
            }
            Orden::Tinta { contorno, color } => {
                if contorno.len() < 3 {
                    continue;
                }
                c.push_str("q ");
                r.alfa(color.a, &mut c);
                let _ = write!(c, "{} rg ", rgb(*color));
                camino_de_tinta(contorno, &mut c);
                c.push_str("f Q\n");
            }
            Orden::Polilinea {
                puntos,
                color,
                grosor,
                estilo,
            } => {
                if puntos.len() < 2 {
                    continue;
                }
                c.push_str("q ");
                r.alfa(color.a, &mut c);
                let _ = write!(c, "{} RG {} w 1 J 1 j ", rgb(*color), n(*grosor));
                if let Some((raya, hueco)) = exportar::rayas(*estilo, *grosor) {
                    let _ = write!(c, "[{} {}] 0 d ", n(raya), n(hueco));
                }
                camino_poligonal(puntos, false, &mut c);
                c.push_str("S Q\n");
            }
            Orden::Velo { hueco, color } => {
                c.push_str("q ");
                r.alfa(color.a, &mut c);
                let _ = write!(
                    c,
                    "{} rg {} {} {} {} re ",
                    rgb(*color),
                    n(x0),
                    n(y0),
                    n(x1 - x0),
                    n(y1 - y0)
                );
                camino_poligonal(hueco, true, &mut c);
                // Par-impar: el hueco agujerea el velo.
                c.push_str("f* Q\n");
            }
            Orden::Texto {
                texto,
                x,
                y,
                tam,
                familia,
                color,
                ancho_max,
                ..
            } => {
                // Los renglones de una letra del lienzo van a su interlineado
                // fijo, como en pantalla (`pixpin_render::letras`); la cara
                // sigue siendo la incrustada (Segoe UI), que el PDF no lleva
                // las del catalogo.
                let (linea_base, interlinea) =
                    match pixpin_motor2d::texto::fuente_por_nombre(familia) {
                        Some(f) => (f.linea_base(), f.interlineado),
                        None => (exportar::LINEA_BASE, exportar::INTERLINEA),
                    };
                c.push_str("q ");
                r.alfa(color.a, &mut c);
                // Con la letra de la pantalla dentro (`/F2`) se parte y se
                // escribe con ella; sin ella, Helvetica (`/F1`). Lo que no
                // tienen, con sus reservas ([`Juego`]).
                let _ = write!(c, "{} rg BT ", rgb(*color));
                let lineas = juego.partir(texto, *tam, *ancho_max);
                for (i, linea) in lineas.iter().enumerate() {
                    let base = y + tam * (linea_base + interlinea * i as f32);
                    // La matriz del texto vuelve a voltear la Y: con la de la
                    // pagina, las letras saldrian boca abajo.
                    let _ = write!(c, "1 0 0 -1 {} {} Tm ", n(*x), n(base));
                    c.push_str(&juego.renglon(linea, *tam, r));
                }
                c.push_str("ET Q\n");
            }
            // El numero de una cota: girado con su raya alrededor de su
            // centro y con halo. El halo es el mismo renglon trazado (modo de
            // texto 1) con el grosor del halo y las juntas redondas, y encima
            // el relleno (modo 0): primero el halo, como el movil, o se
            // comeria los perfiles de las letras.
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
                let linea_base = pixpin_motor2d::texto::fuente_por_nombre(familia)
                    .map_or(exportar::LINEA_BASE, |f| f.linea_base());
                c.push_str("q ");
                r.alfa(color.a, &mut c);
                if *angulo != 0.0 {
                    let (s, co) = angulo.sin_cos();
                    let (cx, cy) = (centro.x, centro.y);
                    let _ = write!(
                        c,
                        "{:.5} {:.5} {:.5} {:.5} {} {} cm ",
                        co,
                        s,
                        -s,
                        co,
                        n(cx - cx * co + cy * s),
                        n(cy - cx * s - cy * co)
                    );
                }
                let renglon = juego.renglon(texto, *tam, r);
                let base = y + tam * linea_base;
                for (modo, tinta) in [
                    (
                        "1 Tr",
                        format!("{} RG {} w 1 j 1 J", rgb(*halo), n(*grosor_halo)),
                    ),
                    ("0 Tr", format!("{} rg", rgb(*color))),
                ] {
                    let _ = write!(
                        c,
                        "{tinta} BT {modo} 1 0 0 -1 {} {} Tm {renglon}ET ",
                        n(*x),
                        n(base)
                    );
                }
                c.push_str("Q\n");
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
                // Sin pixeles, su hueco: una imagen que no se encuentra no
                // rompe la pagina.
                if !hay_imagen(*id_objeto) {
                    continue;
                }
                r.imagenes.insert(*id_objeto);
                c.push_str("q ");
                r.alfa(*opacidad, &mut c);
                // Girada alrededor del centro de su caja, con la misma cuenta
                // que el grafito: una foto girada en el movil sale girada.
                if *angulo != 0.0 {
                    let (s, co) = angulo.sin_cos();
                    let (cx, cy) = (*x + *ancho / 2.0, *y + *alto / 2.0);
                    let _ = write!(
                        c,
                        "{:.5} {:.5} {:.5} {:.5} {} {} cm ",
                        co,
                        s,
                        -s,
                        co,
                        n(cx - cx * co + cy * s),
                        n(cy - cx * s - cy * co)
                    );
                }
                // **Recortada**, la imagen entera estirada y corrida para que
                // su trozo caiga en la caja, y la caja como recorte: el PDF no
                // sabe pintar un trozo de una imagen, pero si recortar. Asi el
                // mismo objeto de imagen sirve aunque dos figuras ensenen dos
                // trozos distintos de la misma foto.
                let caja = (*x, *y, *ancho, *alto);
                let (ix, iy, iw, ih) = match recorte
                    .and_then(|rc| exportar::imagen_entera(caja, &rc))
                {
                    Some(entera) => {
                        let _ = write!(c, "{} {} {} {} re W n ", n(*x), n(*y), n(*ancho), n(*alto));
                        entera
                    }
                    None => caja,
                };
                // La imagen se pinta en el cuadrado unidad con su primera
                // fila arriba; con la Y de la escena hacia abajo, arriba es
                // `y`, asi que el cuadrado se voltea otra vez.
                let _ = writeln!(
                    c,
                    "{} 0 0 {} {} {} cm /I{} Do Q",
                    n(iw),
                    n(-ih),
                    n(ix),
                    n(iy + ih),
                    nombre_imagen(*id_objeto)
                );
            }
        }
        // La silueta otra vez, rellena con la tela como patron: la tela solo
        // se ve dentro del trazo, que es recortarla a el sin un recorte.
        if let (Some(g), Orden::Tinta { contorno, .. }) = (grano, o)
            && contorno.len() >= 3
            && !exportar::tela_vacia(&g)
            && g.paso.is_finite()
            && g.paso > 0.0
        {
            let t = r.tela(g);
            let _ = write!(c, "q /Pattern cs /G{t} scn ");
            camino_de_tinta(contorno, &mut c);
            c.push_str("f Q\n");
        }
    }
    // grafitos_al_final: lo de grafito que va encima de la ultima orden.
    for g in grafitos {
        grafito(g, hay_imagen, r, &mut c);
    }
    c.push_str("Q\n");
    c
}

/// **El grafito como su mapa de casillas** (`DrawPdf` del movil, por el
/// mismo `elGrafitoDe` que el SVG): una imagen con su mascara, sin suavizar
/// —sin `Interpolate`, un lector que amplia ensena las casillas, como el
/// lienzo—, girada con la figura alrededor de su centro. Como raya lisa
/// dejaba de ser grafito.
fn grafito(
    g: &pixpin_motor2d::tinta::grafito::GrafitoSuelto,
    hay_imagen: &mut dyn FnMut(u64) -> bool,
    r: &mut Recursos,
    c: &mut String,
) {
    let m = &g.mapa;
    if !hay_imagen(m.id) {
        return;
    }
    r.imagenes.insert(m.id);
    c.push_str("q ");
    r.alfa(g.opacidad, c);
    if m.angulo != 0.0 {
        // El giro de la pantalla (`pixpin_render::grafito`), en la forma
        // `a b c d e f` del PDF: con la Y hacia abajo, gira hacia el mismo
        // lado que la figura.
        let (s, co) = m.angulo.sin_cos();
        let (cx, cy) = (m.centro.x, m.centro.y);
        let _ = write!(
            c,
            "{:.5} {:.5} {:.5} {:.5} {} {} cm ",
            co,
            s,
            -s,
            co,
            n(cx - cx * co + cy * s),
            n(cy - cx * s - cy * co)
        );
    }
    let (x, y, w, h) = m.caja();
    // Como las demas imagenes: el cuadrado unidad, volteado otra vez.
    let _ = writeln!(
        c,
        "{} 0 0 {} {} {} cm /I{} Do Q",
        n(w),
        n(-h),
        n(x),
        n(y + h),
        nombre_imagen(m.id)
    );
}

/// **La letra incrustada como objetos del PDF**: una letra compuesta
/// (`Type0`) de identidad —el texto va por numero de glifo— con su subconjunto
/// TrueType dentro, los anchos de los glifos usados y la tabla que devuelve
/// cada glifo a su caracter (`ToUnicode`), sin la cual buscar o copiar en el
/// PDF daria basura. Devuelve el numero del objeto de la letra.
fn objetos_de_letra(f: &mut Fichero, l: &Letra, caracteres: &BTreeMap<u16, char>) -> usize {
    let usados: std::collections::BTreeSet<u16> = caracteres.keys().copied().collect();
    let sub = l.subconjunto(&usados, caracteres);
    // Seis mayusculas delante del nombre: la norma marca asi que la letra es
    // un subconjunto. Salen de los glifos, asi que dos PDF con las mismas
    // letras llevan el mismo nombre.
    let mut h: u32 = 2166136261;
    for g in &usados {
        for b in g.to_be_bytes() {
            h = (h ^ b as u32).wrapping_mul(16777619);
        }
    }
    let marca: String = (0..6)
        .map(|i| (b'A' + ((h >> (i * 5)) % 26) as u8) as char)
        .collect();
    let nombre = format!("{marca}+{}", l.nombre);

    let fichero = f.reservar();
    let comprimido = comprimir(&sub);
    let mut cuerpo = Vec::with_capacity(comprimido.len() + 64);
    let _ = write!(
        cuerpo,
        "<< /Length1 {} /Filter /FlateDecode /Length {} >>\nstream\n",
        sub.len(),
        comprimido.len()
    );
    cuerpo.extend_from_slice(&comprimido);
    cuerpo.extend_from_slice(b"\nendstream");
    f.poner(fichero, cuerpo);

    let (ascenso, descenso, mayusculas) = l.alturas;
    let [x0, y0, x1, y1] = l.caja;
    let descriptor = f.reservar();
    f.poner(
        descriptor,
        format!(
            "<< /Type /FontDescriptor /FontName /{nombre} /Flags 32 /FontBBox [{x0} {y0} {x1} {y1}] /ItalicAngle 0 /Ascent {ascenso} /Descent {descenso} /CapHeight {mayusculas} /StemV 80 /FontFile2 {fichero} 0 R >>"
        )
        .into_bytes(),
    );
    let mut anchos = String::new();
    for g in &usados {
        let _ = write!(anchos, "{g} [{}] ", l.avance(*g).round() as i32);
    }
    let cid = f.reservar();
    f.poner(
        cid,
        format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{nombre} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {descriptor} 0 R /CIDToGIDMap /Identity /DW {} /W [{anchos}] >>",
            l.avance(0).round() as i32
        )
        .into_bytes(),
    );
    // El glifo 0 es el hueco, lo que no tiene ninguna letra: se devuelve como
    // el caracter de sustitucion (U+FFFD), que dice al copiar que ahi habia
    // algo que no se pudo pintar.
    let mut mapa = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let pares: Vec<(u16, char)> = caracteres
        .iter()
        .map(|(g, c)| (*g, if *g == 0 { '\u{fffd}' } else { *c }))
        .collect();
    // De cien en cien, que es lo mas que la norma deja en un bloque.
    for trozo in pares.chunks(100) {
        let _ = writeln!(mapa, "{} beginbfchar", trozo.len());
        for (g, c) in trozo {
            let mut u = [0u16; 2];
            let hex: String = c
                .encode_utf16(&mut u)
                .iter()
                .map(|x| format!("{x:04X}"))
                .collect();
            let _ = writeln!(mapa, "<{g:04X}> <{hex}>");
        }
        mapa.push_str("endbfchar\n");
    }
    mapa.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    let unicode = f.reservar();
    f.poner(unicode, Fichero::flujo("", &comprimir(mapa.as_bytes())));
    let letra = f.reservar();
    f.poner(
        letra,
        format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /{nombre} /Encoding /Identity-H /DescendantFonts [{cid} 0 R] /ToUnicode {unicode} 0 R >>"
        )
        .into_bytes(),
    );
    letra
}

/// Lo que distingue una imagen de tela de otra: el material y el color. El
/// paso y el giro no, que van en el patron.
fn llave_de_tela(g: &Grano) -> (pixpin_motor2d::tinta::MaterialTinta, [u8; 4]) {
    let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    (
        g.material,
        [c(g.color.r), c(g.color.g), c(g.color.b), c(g.color.a)],
    )
}

/// La imagen de una tela (la de `exportar::tela_rgba`, la misma que sube la
/// pantalla) con su mascara, que es la que dice donde tapa.
fn objeto_de_tela(f: &mut Fichero, g: &Grano) -> usize {
    let (lado, rgba) = exportar::tela_rgba(g);
    let (color, mascara) = partes_de(&Pixeles {
        ancho: lado,
        alto: lado,
        rgba,
    })
    .unwrap_or_default();
    // `Interpolate`: la pantalla estira la tela con filtro lineal; sin el, un
    // lector la amplia a bloques y las rayas salen en escalera.
    let comun = format!(
        "/Type /XObject /Subtype /Image /Width {lado} /Height {lado} /BitsPerComponent 8 /Interpolate true"
    );
    let smask = mascara.map(|m| {
        let o = f.reservar();
        f.poner(
            o,
            Fichero::flujo(&format!("{comun} /ColorSpace /DeviceGray"), &comprimir(&m)),
        );
        o
    });
    let o = f.reservar();
    let extra = smask.map_or(String::new(), |s| format!(" /SMask {s} 0 R"));
    f.poner(
        o,
        Fichero::flujo(
            &format!("{comun} /ColorSpace /DeviceRGB{extra}"),
            &comprimir(&color),
        ),
    );
    o
}

/// El patron de mosaico de una tela en una pagina: la imagen `imagen`
/// repetida cada `lado` pixeles de tela, colocada con su matriz.
fn patron(g: &Grano, a_papel: [f32; 6], imagen: usize) -> Vec<u8> {
    let lado = pixpin_motor2d::tinta::material::LADO_DEL_MOSAICO;
    let m = matriz_del_patron(g, a_papel);
    // La imagen llena el cuadrado unidad con su primera fila arriba; el
    // patron mide con la Y de la escena (hacia abajo), asi que se voltea
    // igual que las imagenes del dibujo.
    let dibujo = format!("q {lado} 0 0 -{lado} 0 {lado} cm /T Do Q");
    Fichero::flujo(
        &format!(
            "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 {lado} {lado}] /XStep {lado} /YStep {lado} /Matrix [{:.5} {:.5} {:.5} {:.5} {:.3} {:.3}] /Resources << /XObject << /T {imagen} 0 R >> >>",
            m[0], m[1], m[2], m[3], m[4], m[5]
        ),
        &comprimir(dibujo.as_bytes()),
    )
}

/// El patron de una tela en una pagina: su matriz lleva un pixel de la tela
/// al papel pasando por el documento, `tela -> escena -> papel`. Con la
/// matriz de la tela sola, el rayado saldria del tamano de un punto de papel
/// y clavado a la esquina de la pagina, no al dibujo.
fn matriz_del_patron(g: &Grano, a_papel: [f32; 6]) -> [f32; 6] {
    let [a, b, c, d] = exportar::matriz_de_la_tela(g);
    let [p, q, r, s, e, f] = a_papel;
    // Vectores fila, como Direct2D y como el PDF: primero la tela, luego el
    // papel.
    [
        a * p + b * r,
        a * q + b * s,
        c * p + d * r,
        c * q + d * s,
        e,
        f,
    ]
}

/// El nombre de una imagen en los recursos. El papel lleva `u64::MAX`, que
/// escrito entero seria largo pero valido; se deja tal cual.
fn nombre_imagen(id: u64) -> String {
    id.to_string()
}

fn comprimir(datos: &[u8]) -> Vec<u8> {
    let mut z = ZlibEncoder::new(Vec::with_capacity(datos.len() / 3), Compression::default());
    // Escribir en un Vec no falla.
    let _ = z.write_all(datos);
    z.finish().unwrap_or_default()
}

/// Los objetos del fichero, en orden, y quien sabe numerarlos.
struct Fichero {
    objetos: Vec<Vec<u8>>,
}

impl Fichero {
    /// Aparta un numero de objeto para rellenarlo despues.
    fn reservar(&mut self) -> usize {
        self.objetos.push(Vec::new());
        self.objetos.len()
    }

    fn poner(&mut self, numero: usize, cuerpo: Vec<u8>) {
        self.objetos[numero - 1] = cuerpo;
    }

    fn flujo(diccionario: &str, datos: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(datos.len() + 64);
        let _ = write!(
            v,
            "<< {diccionario} /Filter /FlateDecode /Length {} >>\nstream\n",
            datos.len()
        );
        v.extend_from_slice(datos);
        v.extend_from_slice(b"\nendstream");
        v
    }

    fn escribir(self, catalogo: usize) -> Vec<u8> {
        let mut salida =
            Vec::with_capacity(self.objetos.iter().map(Vec::len).sum::<usize>() + 1024);
        // El comentario binario de la segunda linea es lo que la norma pide
        // para que un programa de transferencia no lo trate como texto.
        salida.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
        let mut posiciones = Vec::with_capacity(self.objetos.len());
        for (i, o) in self.objetos.iter().enumerate() {
            posiciones.push(salida.len());
            let _ = writeln!(salida, "{} 0 obj", i + 1);
            salida.extend_from_slice(o);
            salida.extend_from_slice(b"\nendobj\n");
        }
        let xref = salida.len();
        let _ = write!(
            salida,
            "xref\n0 {}\n0000000000 65535 f \n",
            self.objetos.len() + 1
        );
        for p in posiciones {
            let _ = writeln!(salida, "{p:010} 00000 n ");
        }
        let _ = write!(
            salida,
            "trailer\n<< /Size {} /Root {catalogo} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            self.objetos.len() + 1
        );
        salida
    }
}

/// Las dos partes de una imagen para el PDF: el color, y la transparencia si
/// hace falta (una foto opaca no la necesita y pesaria de mas).
fn partes_de(p: &Pixeles) -> Option<(Vec<u8>, Option<Vec<u8>>)> {
    let total = p.ancho as usize * p.alto as usize;
    if total == 0 || p.rgba.len() != total * 4 {
        return None;
    }
    let mut color = Vec::with_capacity(total * 3);
    let mut mascara = Vec::with_capacity(total);
    for px in p.rgba.chunks_exact(4) {
        color.extend_from_slice(&px[..3]);
        mascara.push(px[3]);
    }
    let opaca = mascara.iter().all(|&a| a == 255);
    Some((color, (!opaca).then_some(mascara)))
}

/// **Las hojas como PDF**, una por pagina. `fondo` es el papel (`None`,
/// transparente: queda el blanco de la hoja). `imagenes` da los pixeles de
/// cada `Orden::Imagen`, el papel de fondo incluido.
///
/// `None` si no hay hojas: un PDF de cero paginas es valido para la norma y
/// no sirve para nada.
pub fn de_hojas(
    hojas: &[Hoja],
    fondo: Option<ColorRgba>,
    imagenes: &dyn Fn(u64) -> Option<Pixeles>,
) -> Option<Vec<u8>> {
    de_hojas_con_letra(hojas, fondo, imagenes, None)
}

/// Lo mismo que [`de_hojas`], con el texto en `letra_propia` —la letra de la
/// pantalla, `letra::del_sistema("Segoe UI")`— incrustada con solo las letras
/// que se usan. Asi el PDF parte las lineas donde la pantalla y se ve con la
/// misma cara. Sin ella, Helvetica sin incrustar.
pub fn de_hojas_con_letra(
    hojas: &[Hoja],
    fondo: Option<ColorRgba>,
    imagenes: &dyn Fn(u64) -> Option<Pixeles>,
    letra_propia: Option<&Letra>,
) -> Option<Vec<u8>> {
    de_hojas_con_indice(hojas, fondo, imagenes, letra_propia, &[])
}

/// **Un marcador del indice del PDF** (su «outline»): lo que se lee en el
/// panel de marcadores de cualquier lector y adonde lleva al pulsarlo. Asi
/// los marcadores con emoticono del lector de PixPin viajan dentro del PDF
/// anotado y no se quedan en el ordenador donde se pusieron.
#[derive(Debug, Clone, PartialEq)]
pub struct Marcador {
    pub titulo: String,
    /// La hoja, desde 0.
    pub hoja: usize,
    /// La altura en la hoja, en sus unidades (las de `Hoja::caja`).
    pub y: f32,
}

/// Un texto del PDF que no es contenido (titulos del indice): UTF-16 con su
/// marca delante, en hexadecimal. Es la unica codificacion que admite
/// acentos y emoticonos en cualquier lector.
fn texto_unicode(s: &str) -> String {
    let mut o = String::from("<FEFF");
    for u in s.encode_utf16() {
        let _ = write!(o, "{u:04X}");
    }
    o.push('>');
    o
}

/// Lo mismo que [`de_hojas_con_letra`], con **un indice de marcadores**:
/// el lector de PDF que lo abra los ensena en su panel y cada uno lleva a
/// su hoja y a su altura. Los que apuntan a una hoja que no hay se saltan.
pub fn de_hojas_con_indice(
    hojas: &[Hoja],
    fondo: Option<ColorRgba>,
    imagenes: &dyn Fn(u64) -> Option<Pixeles>,
    letra_propia: Option<&Letra>,
    indice: &[Marcador],
) -> Option<Vec<u8>> {
    let a4 = |_: usize, h: &Hoja| {
        if exportar::apaisada(h) {
            (A4.1, A4.0)
        } else {
            A4
        }
    };
    escribir_hojas(hojas, fondo, imagenes, letra_propia, indice, &a4, MARGEN)
}

/// **Cada hoja en una pagina de su medida exacta y sin margen**: la unidad
/// de la hoja cae en `medidas[i].0 / hoja.ancho()` puntos y la esquina de
/// arriba a la izquierda de su caja, en la de arriba a la izquierda del
/// papel. Es lo que necesita la tinta que se pega encima de un PDF ajeno
/// (`con_anotaciones`): alli no se encaja en un A4, se calca sobre la hoja.
/// `None` si no hay una medida por hoja.
pub fn de_hojas_a_medida(
    hojas: &[Hoja],
    medidas: &[(f32, f32)],
    imagenes: &dyn Fn(u64) -> Option<Pixeles>,
    letra_propia: Option<&Letra>,
) -> Option<Vec<u8>> {
    if medidas.len() != hojas.len() {
        return None;
    }
    escribir_hojas(
        hojas,
        None,
        imagenes,
        letra_propia,
        &[],
        &|i, _| medidas[i],
        0.0,
    )
}

// La cache guarda lo que da `partes_de` con sus medidas; un alias no aclara nada.
#[allow(clippy::type_complexity)]
fn escribir_hojas(
    hojas: &[Hoja],
    fondo: Option<ColorRgba>,
    imagenes: &dyn Fn(u64) -> Option<Pixeles>,
    letra_propia: Option<&Letra>,
    indice: &[Marcador],
    medida: &dyn Fn(usize, &Hoja) -> (f32, f32),
    margen: f32,
) -> Option<Vec<u8>> {
    if hojas.is_empty() {
        return None;
    }
    // Las imagenes se leen una vez aunque salgan en varias paginas (la foto
    // de fondo sale en todas las de sus marcos).
    let mut cache: BTreeMap<u64, Option<(u32, u32, Vec<u8>, Option<Vec<u8>>)>> = BTreeMap::new();
    // Los mapas del grafito son imagenes mas: sus ids no chocan con los de
    // las imagenes del lienzo (`GrafitoSuelto`), asi que van por el mismo
    // camino, con su mascara, y cada uno se escribe una sola vez.
    let grafitos: BTreeMap<u64, &pixpin_motor2d::tinta::grafito::GrafitoSuelto> = hojas
        .iter()
        .flat_map(|h| &h.grafitos)
        .map(|g| (g.mapa.id, g))
        .collect();
    let mut hay = |id: u64| {
        cache
            .entry(id)
            .or_insert_with(|| {
                let p = match grafitos.get(&id) {
                    Some(g) => Some(Pixeles {
                        ancho: g.mapa.ancho,
                        alto: g.mapa.alto,
                        rgba: g.mapa.rgba.clone(),
                    }),
                    None => imagenes(id),
                };
                p.and_then(|p| partes_de(&p).map(|(c, m)| (p.ancho, p.alto, c, m)))
            })
            .is_some()
    };
    // La letra del texto solo si su licencia deja meterla en el fichero; si
    // no, Helvetica. Las reservas se abren solas segun hacen falta.
    let propia = letra_propia.filter(|l| l.se_puede_incrustar());
    let reservas = crate::letra::Reservas::del_sistema(propia.map(|l| l.nombre.as_str()));
    let mut juego = Juego::nuevo(propia, reservas);
    // Primero el contenido, que es lo que dice que recursos hacen falta.
    let mut paginas = Vec::with_capacity(hojas.len());
    for (i, hoja) in hojas.iter().enumerate() {
        let tam = medida(i, hoja);
        let mut r = Recursos::default();
        let c = contenido(hoja, tam, fondo, &mut hay, &mut r, &mut juego, margen);
        paginas.push((tam, c, r));
    }

    let mut f = Fichero {
        objetos: Vec::new(),
    };
    let catalogo = f.reservar();
    let arbol = f.reservar();
    let letra = f.reservar();
    f.poner(
        letra,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    // Cada letra incrustada que escribio alguna pagina: un solo subconjunto
    // con los glifos de todas, no uno por pagina.
    let mut por_letra: BTreeMap<usize, BTreeMap<u16, char>> = BTreeMap::new();
    for (_, _, r) in &paginas {
        for (i, glifos) in &r.glifos {
            let todos = por_letra.entry(*i).or_default();
            for (g, c) in glifos {
                todos.entry(*g).or_insert(*c);
            }
        }
    }
    let incrustadas: BTreeMap<usize, usize> = por_letra
        .iter()
        .filter_map(|(i, caracteres)| {
            let l = juego.letra(*i)?;
            Some((*i, objetos_de_letra(&mut f, l, caracteres)))
        })
        .collect();
    // Un estado grafico por cada transparencia usada en todo el documento.
    let mut estados: BTreeMap<u8, usize> = BTreeMap::new();
    for (_, _, r) in &paginas {
        for &a in &r.alfas {
            estados.entry(a).or_insert_with(|| {
                let o = f.reservar();
                let v = a as f32 / 255.0;
                f.poner(
                    o,
                    format!("<< /Type /ExtGState /ca {} /CA {} >>", n(v), n(v)).into_bytes(),
                );
                o
            });
        }
    }
    // Y un objeto por imagen, con su mascara si la lleva.
    let mut objetos_imagen: BTreeMap<u64, usize> = BTreeMap::new();
    for (id, datos) in &cache {
        let Some((w, h, color, mascara)) = datos else {
            continue;
        };
        let smask = mascara.as_ref().map(|m| {
            let o = f.reservar();
            f.poner(
                o,
                Fichero::flujo(
                    &format!(
                        "/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8"
                    ),
                    &comprimir(m),
                ),
            );
            o
        });
        let o = f.reservar();
        let extra = smask.map_or(String::new(), |s| format!(" /SMask {s} 0 R"));
        f.poner(
            o,
            Fichero::flujo(
                &format!(
                    "/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8{extra}"
                ),
                &comprimir(color),
            ),
        );
        objetos_imagen.insert(*id, o);
    }

    let mut hijos = Vec::with_capacity(paginas.len());
    // A que altura del papel cae cada `y` de su hoja: el mismo encaje que el
    // contenido, para que el marcador lleve justo a lo marcado.
    let a_papel: Vec<AlPapel> = hojas
        .iter()
        .zip(&paginas)
        .map(|(h, (tam, _, _))| (tam.1, exportar::encaje(h, tam.0, tam.1, margen)))
        .collect();
    // La imagen de cada tela, una por material y color en todo el documento:
    // lo que cambia de pagina a pagina es solo su patron (su matriz).
    let mut telas_hechas: std::collections::HashMap<_, usize> = std::collections::HashMap::new();
    for (tam, c, r) in paginas {
        let flujo = f.reservar();
        f.poner(flujo, Fichero::flujo("", &comprimir(c.as_bytes())));
        let mut recursos = format!("/Font << /F1 {letra} 0 R");
        for i in r.glifos.keys() {
            if let Some(o) = incrustadas.get(i) {
                let _ = write!(recursos, " /F{} {o} 0 R", i + 2);
            }
        }
        recursos.push_str(" >>");
        if !r.alfas.is_empty() {
            recursos.push_str(" /ExtGState <<");
            for a in &r.alfas {
                let _ = write!(recursos, " /A{a} {} 0 R", estados[a]);
            }
            recursos.push_str(" >>");
        }
        let usadas: Vec<(u64, usize)> = r
            .imagenes
            .iter()
            .filter_map(|id| objetos_imagen.get(id).map(|o| (*id, *o)))
            .collect();
        if !usadas.is_empty() {
            recursos.push_str(" /XObject <<");
            for (id, o) in usadas {
                let _ = write!(recursos, " /I{} {o} 0 R", nombre_imagen(id));
            }
            recursos.push_str(" >>");
        }
        if !r.telas.is_empty() {
            recursos.push_str(" /Pattern <<");
            for (i, g) in r.telas.iter().enumerate() {
                let imagen = *telas_hechas
                    .entry(llave_de_tela(g))
                    .or_insert_with(|| objeto_de_tela(&mut f, g));
                let o = f.reservar();
                f.poner(o, patron(g, r.a_papel, imagen));
                let _ = write!(recursos, " /G{i} {o} 0 R");
            }
            recursos.push_str(" >>");
        }
        let pagina = f.reservar();
        f.poner(
            pagina,
            format!(
                "<< /Type /Page /Parent {arbol} 0 R /MediaBox [0 0 {} {}] /Resources << {recursos} >> /Contents {flujo} 0 R >>",
                n(tam.0),
                n(tam.1)
            )
            .into_bytes(),
        );
        hijos.push(pagina);
    }
    let kids: Vec<String> = hijos.iter().map(|h| format!("{h} 0 R")).collect();
    f.poner(
        arbol,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            hijos.len()
        )
        .into_bytes(),
    );
    let con_indice = indice_pdf(&mut f, &hijos, &a_papel, indice);
    f.poner(
        catalogo,
        format!("<< /Type /Catalog /Pages {arbol} 0 R{con_indice} >>").into_bytes(),
    );
    Some(f.escribir(catalogo))
}

/// El alto de una pagina y el encaje de su hoja en ella (`exportar::encaje`).
type AlPapel = (f32, Option<(f32, f32, f32)>);

/// **El indice de marcadores** (`/Outlines`): una lista enlazada de
/// entradas, cada una con su titulo y su destino (la pagina y la altura).
/// Devuelve lo que hay que anadir al catalogo: nada si no hay marcadores, y
/// si los hay, que el lector abra con el panel de marcadores a la vista.
fn indice_pdf(
    f: &mut Fichero,
    paginas: &[usize],
    a_papel: &[AlPapel],
    indice: &[Marcador],
) -> String {
    let validos: Vec<(&Marcador, usize, f32)> = indice
        .iter()
        .filter_map(|m| {
            let pagina = *paginas.get(m.hoja)?;
            let (alto, encaje) = *a_papel.get(m.hoja)?;
            // Y del papel: de abajo arriba, como mide el PDF.
            let y = match encaje {
                Some((k, _, dy)) => alto - (m.y * k + dy),
                None => alto,
            };
            Some((m, pagina, y.clamp(0.0, alto)))
        })
        .collect();
    if validos.is_empty() {
        return String::new();
    }
    let raiz = f.reservar();
    let numeros: Vec<usize> = validos.iter().map(|_| f.reservar()).collect();
    for (j, (m, pagina, y)) in validos.iter().enumerate() {
        let mut d = format!(
            "<< /Title {} /Parent {raiz} 0 R /Dest [{pagina} 0 R /XYZ null {} null]",
            texto_unicode(&m.titulo),
            n(*y)
        );
        if j > 0 {
            let _ = write!(d, " /Prev {} 0 R", numeros[j - 1]);
        }
        if let Some(sig) = numeros.get(j + 1) {
            let _ = write!(d, " /Next {sig} 0 R");
        }
        d.push_str(" >>");
        f.poner(numeros[j], d.into_bytes());
    }
    f.poner(
        raiz,
        format!(
            "<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {} >>",
            numeros[0],
            numeros[numeros.len() - 1],
            numeros.len()
        )
        .into_bytes(),
    );
    format!(" /Outlines {raiz} 0 R /PageMode /UseOutlines")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_numeros_van_cortos() {
        assert_eq!(n(2.0), "2");
        assert_eq!(n(0.5), "0.5");
        assert_eq!(n(-1.234), "-1.23");
        assert_eq!(n(f32::INFINITY), "0");
    }

    #[test]
    fn el_texto_se_escapa_y_lo_que_no_cabe_en_winansi_no_es_de_helvetica() {
        let escapa = |s: &str| {
            let mut o = String::new();
            for ch in s.chars() {
                byte_pdf(winansi(ch).expect("en WinAnsi"), &mut o);
            }
            o
        };
        assert_eq!(escapa("a(b)c\\"), "a\\(b\\)c\\\\");
        assert_eq!(escapa("año"), "a\\361o");
        assert_eq!(escapa("€"), "\\200");
        // Caso negativo: ni el ideograma ni el menos de verdad estan.
        assert_eq!(winansi('漢'), None);
        assert_eq!(winansi('\u{2212}'), None);
    }

    fn segoe_y(familia: &str) -> Option<(Letra, Letra)> {
        Some((
            crate::letra::del_sistema("Segoe UI")?,
            crate::letra::del_sistema(familia)?,
        ))
    }

    #[test]
    fn lo_que_no_tiene_la_letra_va_en_su_reserva_y_lo_de_nadie_en_el_hueco() {
        let Some((segoe, simbolos)) = segoe_y("Segoe UI Symbol") else {
            return;
        };
        let reservas = crate::letra::Reservas::de_letras(vec![simbolos.clone()]);
        let mut j = Juego::nuevo(Some(&segoe), reservas);
        assert!(matches!(j.pieza('a'), Pieza::Glifo(0, g) if g != 0));
        // Un simbolo que Segoe UI no trae y Segoe UI Symbol si.
        let c = '\u{2654}'; // ♔
        assert_eq!(segoe.glifo(c), None);
        assert_eq!(j.pieza(c), Pieza::Glifo(1, simbolos.glifo(c).unwrap()));
        // Caso negativo: lo de nadie, el hueco de la letra, nunca un `?`.
        assert_eq!(j.pieza('\u{10ffff}'), Pieza::Glifo(0, 0));
        // Y el selector de variante de los emoji no deja cuadrito.
        assert_eq!(j.pieza('\u{fe0f}'), Pieza::Nada);
        // El renglon cambia de letra en mitad y vuelve.
        let mut r = Recursos::default();
        let s = j.renglon("a\u{2654}a", 12.0, &mut r);
        assert_eq!(s.matches(" Tf ").count(), 3, "{s}");
        assert!(
            s.starts_with("/F2 12 Tf <") && s.contains("/F3 12 Tf <"),
            "{s}"
        );
        assert_eq!(r.glifos.len(), 2);

        // Sin letra propia, Helvetica para lo latino y la reserva para lo
        // demas; el hueco, de la primera reserva que se deja leer.
        let mut h = Juego::nuevo(None, crate::letra::Reservas::de_letras(vec![simbolos]));
        assert_eq!(h.pieza('a'), Pieza::Helvetica(b'a'));
        assert!(matches!(h.pieza(c), Pieza::Glifo(1, _)));
        assert_eq!(h.pieza('\u{10ffff}'), Pieza::Glifo(1, 0));
        // Sin ninguna reserva no queda otra que el `?`.
        let mut nada = Juego::nuevo(None, crate::letra::Reservas::ninguna());
        assert_eq!(nada.pieza('\u{10ffff}'), Pieza::Helvetica(b'?'));
    }

    #[test]
    fn sin_hojas_no_hay_pdf() {
        assert!(de_hojas(&[], None, &|_| None).is_none());
    }

    fn hoja_blanca() -> Hoja {
        Hoja {
            nombre: String::new(),
            caja: (0.0, 0.0, 1400.0, 1980.0),
            ordenes: Vec::new(),
            marcos: Vec::new(),
            granos: Vec::new(),
            grafitos: Vec::new(),
        }
    }

    #[test]
    fn los_marcadores_van_al_indice_del_pdf_con_su_emoticono_y_su_hoja() {
        let hojas = [hoja_blanca(), hoja_blanca()];
        let indice = [
            Marcador {
                titulo: "⭐ Hoja 2".into(),
                hoja: 1,
                y: 990.0,
            },
            Marcador {
                titulo: "🔖 Hoja 1".into(),
                hoja: 0,
                y: 0.0,
            },
            // Caso negativo: una hoja que no hay no entra.
            Marcador {
                titulo: "x".into(),
                hoja: 7,
                y: 0.0,
            },
        ];
        let bytes = de_hojas_con_indice(&hojas, None, &|_| None, None, &indice).unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("/Outlines "), "el catalogo lo nombra");
        assert!(s.contains("/Type /Outlines"));
        assert!(s.contains("/Count 2 >>"), "solo los dos validos");
        // El emoticono, en UTF-16: la estrella es U+2B50.
        assert!(s.contains("/Title <FEFF2B50002000480"), "{s}");
        assert_eq!(s.matches("/XYZ null").count(), 2);
        // Y el PDF se sigue abriendo, con sus dos paginas.
        let ruta = std::env::temp_dir().join(format!("pixpin-indice-{}.pdf", std::process::id()));
        std::fs::write(&ruta, &bytes).unwrap();
        let doc = crate::Documento::abrir(&ruta).expect("abre");
        assert_eq!(doc.paginas(), 2);
        let _ = std::fs::remove_file(&ruta);
        // Sin marcadores, ni indice ni panel.
        let sin = de_hojas(&hojas, None, &|_| None).unwrap();
        assert!(!String::from_utf8_lossy(&sin).contains("/Outlines"));
    }

    #[test]
    fn una_imagen_con_los_pixeles_mal_contados_no_entra() {
        // Caso negativo: una imagen incoherente no rompe el fichero, se
        // queda fuera.
        assert!(
            partes_de(&Pixeles {
                ancho: 2,
                alto: 2,
                rgba: vec![0; 3],
            })
            .is_none()
        );
        let (_, mascara) = partes_de(&Pixeles {
            ancho: 1,
            alto: 1,
            rgba: vec![1, 2, 3, 255],
        })
        .expect("bien contada");
        assert!(mascara.is_none(), "opaca no lleva mascara");
    }
}

#[cfg(test)]
mod pruebas_del_grafito {
    use super::*;
    use pixpin_motor2d::elemento::{Elemento, Figura};
    use pixpin_motor2d::tinta::MaterialTinta;

    fn hojas_con(material: MaterialTinta) -> Vec<Hoja> {
        let mut escena = pixpin_motor2d::Escena::nueva();
        escena.anadir(Elemento {
            figura: Figura::Lapiz {
                puntos: (0..60)
                    .map(|i| Punto2::nuevo(i as f32 * 3.0, 40.0 + (i % 7) as f32))
                    .collect(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            grosor: 2.0,
            trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
            material,
            ..Default::default()
        });
        exportar::hojas(&escena, exportar::Alcance::Todo, &[], None)
    }

    #[test]
    fn el_grafito_va_al_pdf_como_imagen_con_su_mascara_y_sin_suavizar() {
        let hojas = hojas_con(MaterialTinta::Cuadritos);
        let g = &hojas[0].grafitos[0];
        let pdf = de_hojas(&hojas, None, &|_| None).expect("hay pdf");
        let texto = String::from_utf8_lossy(&pdf);
        // El mapa, con su tamano de casillas y su transparencia aparte.
        assert!(
            texto.contains(&format!("/Width {} /Height {}", g.mapa.ancho, g.mapa.alto)),
            "no esta el mapa"
        );
        assert!(texto.contains("/SMask"));
        // Caso negativo: la tinta lisa no mete ninguna imagen.
        let lisa = de_hojas(&hojas_con(MaterialTinta::Lisa), None, &|_| None).unwrap();
        assert!(!String::from_utf8_lossy(&lisa).contains("/Subtype /Image"));
    }
}
