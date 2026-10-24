//! **El grafito**: un lapiz de verdad, hecho de sellos sobre una rejilla fija
//! de cuadritos del dibujo.
//!
//! Puerto de la herramienta «Grafito» del movil (v0.73-v0.80.2:
//! `Renderer.kt` `cocerElLapiz`, `estamparElLapiz`, `elDienteDelPapel`,
//! `elSelloDelLapiz`, `cocerLaFigura`, `estamparElCamino`). En el fichero es un
//! `freedraw` (o una figura) con `material:"cuadritos"`.
//!
//! ## No es un trazo con grano encima: es un sello
//!
//! El movil probo tres veces a imitar el *aspecto* —cuadritos, tramado— y al
//! usuario no le parecio un lapiz. La cuarta se hizo como lo hacen las
//! aplicaciones de pintar (la ficha del pincel «2B» que mando el usuario): un
//! sello pequeno y rascado se estampa a lo largo del trazo cada **10 % de su
//! tamano**, casi transparente, con tamano y carga atados a la presion. Asi sale
//! solo todo lo que pedia: donde los sellos se pisan el trazo se carga y en los
//! cantos escasea, **repasar oscurece**, apretar engorda, y las puntas se
//! afinan.
//!
//! ## Una rejilla fija, la misma para todo el dibujo
//!
//! Un cuadrito es **una unidad del dibujo** (un pixel al 100 %), siempre en el
//! mismo sitio: el mapa de cada trazo empieza en un multiplo de cuatro, asi que
//! los cuadritos de dos trazos vecinos caen en las mismas casillas. Acercarse
//! los agranda sin suavizarlos —quien pinta usa el vecino mas cercano— y
//! alejarse los funde. Cocerlo a la resolucion del aumento, que fue la primera
//! idea, hacia que de cerca **nunca se vieran los cuadritos**: «un lapiz muy
//! falso», dijo el usuario.
//!
//! ## El diente del papel, casilla a casilla
//!
//! Cada casilla coge el grafito a su manera y siempre igual, porque sale de su
//! sitio en el dibujo y no del trazo: es el mismo papel para todos. Es una
//! **resistencia**, no un agujero: la casilla dura coge poquisimo de cada
//! pasada, pero coge, y a fuerza de repasar todo se iguala («un grafito de
//! verdad pinta»). Y el tono varia un pelo en cada casilla («no es un material
//! perfecto»). Esta cuenta es **la del movil, bit a bit**: con la misma casilla
//! sale lo mismo en los dos aparatos.
//!
//! ## Escritura libre y siete durezas (v0.91-v0.92 del movil)
//!
//! El trazo **no se corrige**: van las muestras tal cual y entre una y otra la
//! curva que pasa por todas (`curva_del_trazo`, la `CurvaDelTrazo` del movil),
//! no rectas que se notan en las esquinas ni el `asentar_lo_rapido` de antes,
//! que aplanaba lo rapido. Y la mina tiene dureza (`Dureza`, 4H ... 8B, la
//! clave `dureza` del elemento): engorda el sello, carga cada estampa y cambia
//! lo que coge el papel con los mismos tres numeros que el movil, asi que lo
//! que alli es 8B aqui tambien lo es, en pantalla y en todo lo que se exporta
//! (todo sale de este mapa).
//!
//! ## Se cuece una vez
//!
//! Un trazo son cientos o miles de sellos. Se estampan una vez en un mapa propio
//! y lo que se pinta cada fotograma es ese mapa; se rehace solo si el trazo
//! cambia. Los mapas viven en un horno por hilo (`cocer`) con tope de memoria,
//! como `PESO_DE_LOS_LAPICES` del movil.
//!
//! Este modulo es **puro**: devuelve pixeles RGBA y no sabe de GPU. Quien los
//! sube a un bitmap y los pinta es `pixpin_render::grafito`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use super::material::AzarJava;
use crate::elemento::{ColorRgba, Elemento, Figura};
use crate::pintado::Orden;
use crate::relleno::EstiloRelleno;
use crate::vector::Punto2;

// ---- Los numeros, los del movil (`Renderer.kt:3796-3814`). ----

/// Lo que mide el sello respecto del grueso del trazo: un lapiz deja una
/// huella mas ancha que su raya.
pub const GORDO_DEL_LAPIZ: f32 = 2.4;
/// *Spacing* 10 %: cada cuanto se estampa, en tamanos de sello.
pub const ESPACIADO_DEL_LAPIZ: f32 = 0.10;
/// *Opacity* 79 %.
pub const OPACIDAD_DEL_LAPIZ: f32 = 0.79;
/// El lado del sello tejido, en pixeles. Se estira a lo que toque al estampar.
pub const LADO_DEL_SELLO: usize = 48;
/// Tope de sellos por trazo: un trazo larguisimo no pide decenas de miles.
pub const SELLOS_POR_TRAZO: f64 = 9000.0;
/// Tope del lado del mapa, en casillas: pasado, se cuece a menos finura.
pub const LADO_MAXIMO_DEL_LAPIZ: f32 = 2048.0;
/// Cuanto se aclara u oscurece una casilla respecto a su color.
pub const TONO_DEL_GRAFITO: f32 = 0.10;
/// Lo cargado que queda un fondo liso de grafito antes del diente del papel.
pub const FONDO_DE_GRAFITO: f32 = 0.55;
/// El grosor en la misma punta, como parte del cuerpo...
pub const PUNTA_DEL_GRAFITO: f32 = 0.22;
/// ...y en cuantos gruesos se llega al cuerpo.
pub const LARGO_DE_LA_PUNTA: f32 = 3.5;
/// Lo que pueden pesar todos los mapas cocidos a la vez.
pub const PESO_DE_LOS_LAPICES: usize = 96 * 1024 * 1024;
/// La rejilla encaja en multiplos de esto: asi dos trazos vecinos comparten
/// casillas y el diente del papel es el mismo debajo de los dos.
pub const ENCAJE_DE_LA_REJILLA: f32 = 4.0;

/// **La dureza de la mina** (v0.91 del movil, `DurezaDeGrafito` de
/// `Element.kt`): la escala de siempre, de la 4H —mina fina, trazo claro, el
/// grano del papel se nota— a la 8B —mina gorda, trazo oscuro que tapa el
/// grano y se carga enseguida al repasar—. HB es el punto medio y es como
/// pintaba el grafito antes de haberla.
///
/// Tres numeros por dureza, todos respecto a HB y **los del movil, tal
/// cual**: `oscuro` multiplica la carga de cada sello, `gordo` el ancho de la
/// mina y `agarre` lo que coge cada casilla del papel (menos = mas grano).
/// Con los mismos numeros, lo que el telefono pinta 8B se ve 8B aqui.
///
/// En el fichero va en la clave `dureza` del elemento («4H», «2B»...). HB no
/// se escribe: el movil lee la clave ausente como HB, y asi un trazo de
/// fabrica no estrena una clave que no dice nada.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum Dureza {
    H4,
    H2,
    #[default]
    Hb,
    B2,
    B4,
    B6,
    B8,
}

impl Dureza {
    /// Las siete, de la mas dura a la mas blanda: el orden del panel.
    pub const TODAS: [Dureza; 7] = [
        Dureza::H4,
        Dureza::H2,
        Dureza::Hb,
        Dureza::B2,
        Dureza::B4,
        Dureza::B6,
        Dureza::B8,
    ];

    /// Su nombre, que es tambien la palabra del fichero.
    pub fn rotulo(self) -> &'static str {
        match self {
            Dureza::H4 => "4H",
            Dureza::H2 => "2H",
            Dureza::Hb => "HB",
            Dureza::B2 => "2B",
            Dureza::B4 => "4B",
            Dureza::B6 => "6B",
            Dureza::B8 => "8B",
        }
    }

    /// La dureza de una palabra del fichero; `None` si no es ninguna (una de
    /// una version futura del movil, que entonces vuelve intacta).
    pub fn desde_rotulo(s: &str) -> Option<Dureza> {
        Dureza::TODAS.into_iter().find(|d| d.rotulo() == s)
    }

    /// `(oscuro, gordo, agarre)`, los de `Element.kt`.
    fn numeros(self) -> (f32, f32, f32) {
        match self {
            Dureza::H4 => (0.42, 0.70, 0.55),
            Dureza::H2 => (0.60, 0.80, 0.70),
            Dureza::Hb => (1.00, 1.00, 1.00),
            Dureza::B2 => (1.25, 1.10, 1.15),
            Dureza::B4 => (1.55, 1.25, 1.30),
            Dureza::B6 => (1.85, 1.40, 1.45),
            Dureza::B8 => (2.20, 1.55, 1.60),
        }
    }

    /// Lo que multiplica la carga de cada sello.
    pub fn oscuro(self) -> f32 {
        self.numeros().0
    }

    /// Lo que multiplica el ancho de la mina.
    pub fn gordo(self) -> f32 {
        self.numeros().1
    }

    /// Lo que multiplica lo que coge cada casilla del papel.
    pub fn agarre(self) -> f32 {
        self.numeros().2
    }

    /// La de un elemento: la suya, o HB si no dice ninguna.
    pub fn de(e: &Elemento) -> Dureza {
        e.extras.dureza.unwrap_or_default()
    }

    /// Lo que se guarda en el elemento: HB no se apunta (ver la cabecera).
    pub fn para_guardar(self) -> Option<Dureza> {
        (self != Dureza::Hb).then_some(self)
    }
}

/// Como mucho, tantos trocitos entre dos muestras (`TOPE_POR_TRAMO` de
/// `CurvaDelTrazo.kt`): una muestra suelta muy lejos no dispara la cuenta.
const TOPE_POR_TRAMO: usize = 48;

/// **Entre muestra y muestra, curva y no recta** (`CurvaDelTrazo` del movil,
/// v0.91: «dibujo una zona curva y sale como un poligono, se nota en las
/// esquinas»).
///
/// Una **Catmull-Rom centripeta** que **pasa por todas y cada una de las
/// muestras**: no mueve ninguna —no es alisar ni corregir, lo trazado sigue
/// ahi tal cual («el grafito es escritura libre»)— y solo decide como se va
/// de una a la siguiente. La centripeta (alfa = 1/2) es la que no hace lazos
/// ni se pasa de largo cuando dos muestras caen muy juntas y la siguiente
/// lejos, que es justo lo que pasa al frenar.
///
/// Devuelve la curva cada `paso` unidades como mucho, sus presiones
/// repartidas igual, y **en que punto de la curva cae cada muestra** (sin las
/// repetidas): quien estampa el trazo en curso lo necesita para saber hasta
/// donde no va a cambiar nada al llegar la muestra siguiente.
pub fn curva_del_trazo(
    puntos: &[[f64; 2]],
    presiones: Option<&[f32]>,
    paso: f64,
) -> (Vec<[f64; 2]>, Option<Vec<f32>>, Vec<usize>) {
    // Fuera las muestras repetidas: dos iguales seguidas no dicen hacia
    // donde se va.
    let presiones = presiones.filter(|p| p.len() == puntos.len());
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(puntos.len());
    let mut prs: Option<Vec<f32>> = presiones.map(|_| Vec::with_capacity(puntos.len()));
    for (i, p) in puntos.iter().enumerate() {
        if let Some(u) = pts.last()
            && (p[0] - u[0]).hypot(p[1] - u[1]) < 1e-6
        {
            continue;
        }
        pts.push(*p);
        if let (Some(v), Some(pr)) = (prs.as_mut(), presiones) {
            v.push(pr[i]);
        }
    }
    if pts.len() < 3 || paso <= 0.0 || paso.is_nan() {
        let indices = (0..pts.len()).collect();
        return (pts, prs, indices);
    }
    let mut sale = Vec::with_capacity(pts.len() * 4);
    let mut sale_pr = prs.as_ref().map(|_| Vec::with_capacity(pts.len() * 4));
    let mut indices = Vec::with_capacity(pts.len());
    sale.push(pts[0]);
    indices.push(0);
    if let (Some(v), Some(pr)) = (sale_pr.as_mut(), prs.as_ref()) {
        v.push(pr[0]);
    }
    for i in 0..pts.len() - 1 {
        let (p1, p2) = (pts[i], pts[i + 1]);
        // En las puntas no hay vecino: se refleja el que hay, asi la curva
        // sale derecha hacia fuera.
        let p0 = if i > 0 {
            pts[i - 1]
        } else {
            [2.0 * p1[0] - p2[0], 2.0 * p1[1] - p2[1]]
        };
        let p3 = if i + 2 < pts.len() {
            pts[i + 2]
        } else {
            [2.0 * p2[0] - p1[0], 2.0 * p2[1] - p1[1]]
        };
        let largo = (p2[0] - p1[0]).hypot(p2[1] - p1[1]);
        let n = ((largo / paso).ceil() as usize).clamp(1, TOPE_POR_TRAMO);
        for k in 1..=n {
            let u = k as f64 / n as f64;
            // La ultima de cada tramo es la muestra misma, sin cuentas que la
            // muevan un pelo.
            sale.push(if k == n {
                p2
            } else {
                centripeta(p0, p1, p2, p3, u)
            });
            if let (Some(v), Some(pr)) = (sale_pr.as_mut(), prs.as_ref()) {
                v.push(pr[i] + (pr[i + 1] - pr[i]) * u as f32);
            }
        }
        indices.push(sale.len() - 1);
    }
    (sale, sale_pr, indices)
}

/// Un punto del tramo `p1 -> p2` en `u` (0..1), con la forma de Barry y
/// Goldman (`CurvaDelTrazo.centripeta`).
pub fn centripeta(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2], p3: [f64; 2], u: f64) -> [f64; 2] {
    let nudo = |a: [f64; 2], b: [f64; 2]| (b[0] - a[0]).hypot(b[1] - a[1]).sqrt().max(1e-4);
    let t0 = 0.0;
    let t1 = t0 + nudo(p0, p1);
    let t2 = t1 + nudo(p1, p2);
    let t3 = t2 + nudo(p2, p3);
    let t = t1 + (t2 - t1) * u;
    let mezcla = |a: [f64; 2], b: [f64; 2], ta: f64, tb: f64| {
        let (wa, wb) = ((tb - t) / (tb - ta), (t - ta) / (tb - ta));
        [a[0] * wa + b[0] * wb, a[1] * wa + b[1] * wb]
    };
    let a1 = mezcla(p0, p1, t0, t1);
    let a2 = mezcla(p1, p2, t1, t2);
    let a3 = mezcla(p2, p3, t2, t3);
    let b1 = mezcla(a1, a2, t0, t2);
    let b2 = mezcla(a2, a3, t1, t3);
    mezcla(b1, b2, t1, t2)
}

/// **Un trazo de grafito cocido**: su mapa de casillas, tal como se ve.
#[derive(Debug, Clone, PartialEq)]
pub struct Cocido {
    /// Pixeles RGBA **sin premultiplicar**, fila tras fila.
    pub rgba: Vec<u8>,
    pub ancho: u32,
    pub alto: u32,
    /// La esquina del mapa en el dibujo, encajada en la rejilla.
    pub x0: f32,
    pub y0: f32,
    /// Casillas por unidad del dibujo: uno siempre, salvo en lo enorme.
    pub finura: f32,
    /// De que version del elemento sale: si cambia, hay que volver a cocer.
    pub huella: u64,
    /// El giro del elemento. El mapa se cuece **sin girar** —como en el
    /// movil, que gira el lienzo— y quien pinta lo gira alrededor de `centro`.
    pub angulo: f32,
    pub centro: Punto2,
    /// De que elemento es: quien sube el mapa lo guarda por aqui.
    pub id: u64,
    /// Un numero distinto para cada mapa distinto, en este hilo.
    pub generacion: u64,
    /// Si este mapa es el de la generacion `.0` con solo el trozo `.1`
    /// cambiado —el trazo en curso, que crece por la punta—, para que quien
    /// ya subio aquel suba solo ese trozo. `None`: cambio todo.
    pub sucio: Option<(u64, Rect)>,
}

impl Cocido {
    /// Donde cae en el dibujo: `(x, y, ancho, alto)` en unidades del dibujo.
    pub fn caja(&self) -> (f32, f32, f32, f32) {
        (
            self.x0,
            self.y0,
            self.ancho as f32 / self.finura,
            self.alto as f32 / self.finura,
        )
    }

    /// Lo que pesa en memoria, en bytes.
    pub fn peso(&self) -> usize {
        self.rgba.len()
    }
}

/// Si `e` se pinta de grafito. Las figuras son las del movil
/// (`FIGURAS_DE_GRAFITO` y el trazo a mano); lo demas con `cuadritos` sigue
/// liso, que es lo que hace el movil con lo que no sabe estampar.
pub fn es_de_grafito(e: &Elemento) -> bool {
    e.material.es_grafito()
        && !e.borrado
        && matches!(
            e.figura,
            Figura::Lapiz { .. }
                | Figura::Rectangulo
                | Figura::Rombo
                | Figura::Elipse
                | Figura::Linea { .. }
                | Figura::Flecha { .. }
                // Lo que rellena el bote con el grafito en la mano (v0.76-v0.77
                // del movil): su fondo tendido flojo y con el diente del papel.
                | Figura::Region { .. }
        )
}

// -------------------------------------------------------------------------
// El sello
// -------------------------------------------------------------------------

/// **La forma del sello**: un cuadrado rascado, como el «2B pencil» de la
/// ficha. Solo cobertura, `LADO_DEL_SELLO` al cuadrado bytes; se hace una vez.
///
/// Sale del mismo azar de Java con la misma semilla que en el movil
/// (`Random(7)`): trescientas rayitas de un pixel en direcciones al azar, y
/// luego el moteado y la caida hacia los cantos, que sin ella el sello dejaria
/// esquinas.
pub fn sello() -> &'static [u8] {
    static SELLO: OnceLock<Vec<u8>> = OnceLock::new();
    SELLO.get_or_init(tejer_sello)
}

fn tejer_sello() -> Vec<u8> {
    let n = LADO_DEL_SELLO;
    let nf = n as f32;
    // Cobertura blanca de 0 a 1, como el `ARGB_8888` del movil antes de
    // `extractAlpha`.
    let mut base = vec![0f32; n * n];
    let mut r = AzarJava::nuevo(7);
    for _ in 0..300 {
        let x = nf * (0.14 + 0.72 * r.flotante());
        let y = nf * (0.14 + 0.72 * r.flotante());
        let a = r.flotante() * std::f32::consts::PI;
        let l = nf * (0.04 + 0.24 * r.flotante());
        let alfa = (110 + r.entero(146)) as f32 / 255.0;
        let (dx, dy) = (a.cos() * l, a.sin() * l);
        rayita(&mut base, n, (x - dx, y - dy), (x + dx, y + dy), alfa);
    }
    let medio = (nf - 1.0) / 2.0;
    let mut sello = vec![0u8; n * n];
    for y in 0..n {
        for x in 0..n {
            let lejos = (x as f32 - medio).abs().max((y as f32 - medio).abs());
            let caida = (1.3 - lejos / (nf * 0.40)).clamp(0.0, 1.0);
            // Se tira del azar en TODAS las casillas, tambien en las vacias:
            // es lo que hace el movil, y saltarse una desplazaria las motas.
            let mota = (90 + r.entero(166)) as f32 / 255.0;
            let a8 = (base[y * n + x] * 255.0).round();
            sello[y * n + x] = ((a8 * caida * mota) as i32).clamp(0, 255) as u8;
        }
    }
    sello
}

/// Una rayita de un pixel de ancho con las puntas a escuadra, suavizada, con
/// la mezcla `SRC_OVER` sobre lo que ya hay.
fn rayita(base: &mut [f32], n: usize, a: (f32, f32), b: (f32, f32), alfa: f32) {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let largo = (vx * vx + vy * vy).sqrt();
    if largo <= f32::EPSILON {
        return;
    }
    let (ux, uy) = (vx / largo, vy / largo);
    let x0 = (a.0.min(b.0) - 1.0).floor().max(0.0) as usize;
    let x1 = ((a.0.max(b.0) + 1.0).ceil() as usize).min(n);
    let y0 = (a.1.min(b.1) - 1.0).floor().max(0.0) as usize;
    let y1 = ((a.1.max(b.1) + 1.0).ceil() as usize).min(n);
    for py in y0..y1 {
        for px in x0..x1 {
            let (cx, cy) = (px as f32 + 0.5 - a.0, py as f32 + 0.5 - a.1);
            let s = cx * ux + cy * uy;
            let d = (cx * uy - cy * ux).abs();
            let a_lo_ancho = (1.0 - d).clamp(0.0, 1.0);
            let a_lo_largo = (s + 0.5)
                .clamp(0.0, 1.0)
                .min((largo - s + 0.5).clamp(0.0, 1.0));
            let k = alfa * a_lo_ancho * a_lo_largo;
            if k > 0.0 {
                let v = &mut base[py * n + px];
                *v = k + *v * (1.0 - k);
            }
        }
    }
}

// -------------------------------------------------------------------------
// El lienzo donde se estampa
// -------------------------------------------------------------------------

/// RGBA **premultiplicado** en bytes, como el mapa de bits del movil: la mezcla
/// de ocho bits es parte de como sale el grafito (las pasadas casi
/// transparentes se redondean igual).
struct Lienzo {
    px: Vec<u8>,
    ancho: usize,
    alto: usize,
    /// **Las tejas tocadas**: una por cada `TEJA` x `TEJA` pixeles, y dice
    /// si algo se pinto en ella. El diente del papel solo pasa por estas.
    ///
    /// Sin ellas el diente recorria el mapa entero, y el mapa es la caja del
    /// trazo: el de un rectangulo de 1200 x 800 es casi un millon de pixeles
    /// para una raya que ocupa un marco fino alrededor.
    tejas: Vec<bool>,
}

/// El lado de una teja, en pixeles del mapa.
const TEJA: usize = 16;

impl Lienzo {
    fn nuevo(ancho: usize, alto: usize) -> Self {
        Self {
            // `vec![0; n]` pide la memoria ya a cero al sistema: lo que no se
            // toca no se paga.
            px: vec![0; ancho * alto * 4],
            ancho,
            alto,
            tejas: vec![false; ancho.div_ceil(TEJA) * alto.div_ceil(TEJA)],
        }
    }

    /// Marca como tocadas las tejas de los pixeles `[x0, x1) x [y0, y1)`.
    fn tocar(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        let (x1, y1) = (x1.min(self.ancho), y1.min(self.alto));
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let por_fila = self.ancho.div_ceil(TEJA);
        for ty in y0 / TEJA..=(y1 - 1) / TEJA {
            for tx in x0 / TEJA..=(x1 - 1) / TEJA {
                self.tejas[ty * por_fila + tx] = true;
            }
        }
    }

    /// Los rectangulos de las tejas tocadas, fila a fila y juntando las
    /// seguidas: es por donde tiene que pasar el diente.
    fn tocado(&self) -> Vec<Rect> {
        let por_fila = self.ancho.div_ceil(TEJA);
        let mut v = Vec::new();
        for ty in 0..self.alto.div_ceil(TEJA) {
            let mut tx = 0;
            while tx < por_fila {
                if !self.tejas[ty * por_fila + tx] {
                    tx += 1;
                    continue;
                }
                let desde = tx;
                while tx < por_fila && self.tejas[ty * por_fila + tx] {
                    tx += 1;
                }
                let (x, y) = (desde * TEJA, ty * TEJA);
                v.push(Rect {
                    x,
                    y,
                    ancho: (tx * TEJA).min(self.ancho) - x,
                    alto: ((ty + 1) * TEJA).min(self.alto) - y,
                });
            }
        }
        v
    }

    /// Una copia del trozo `r`, con su (0, 0) en la esquina de `r`.
    fn recorte(&self, r: Rect) -> Lienzo {
        let mut t = Lienzo::nuevo(r.ancho, r.alto);
        for fila in 0..r.alto {
            let de = ((r.y + fila) * self.ancho + r.x) * 4;
            let a = fila * r.ancho * 4;
            t.px[a..a + r.ancho * 4].copy_from_slice(&self.px[de..de + r.ancho * 4]);
        }
        t
    }

    /// `SRC_OVER` de un color (0..255, sin premultiplicar) con alfa `a` (0..1).
    #[inline]
    fn encima(&mut self, i: usize, color: [f32; 3], a: f32) {
        let d = &mut self.px[i * 4..i * 4 + 4];
        let k = 1.0 - a;
        d[0] = (color[0] * a + d[0] as f32 * k + 0.5) as u8;
        d[1] = (color[1] * a + d[1] as f32 * k + 0.5) as u8;
        d[2] = (color[2] * a + d[2] as f32 * k + 0.5) as u8;
        d[3] = (255.0 * a + d[3] as f32 * k + 0.5) as u8;
    }

    /// **Un sello**, con centro en `(cx, cy)` pixeles del mapa, `lado`
    /// pixeles, girado `grados` y cargado `pincel` (0..1).
    ///
    /// Es el `drawBitmap(sello, matriz, pincel)` del movil con el filtro
    /// bilineal puesto: cada pixel del mapa mira hacia atras en el sello. El
    /// sello de 48 encogido a cuatro o cinco pixeles **aliasa** a proposito:
    /// ese muestreo grueso es parte del rascado que se ve.
    fn estampar(
        &mut self,
        sello: &[u8],
        centro: (f64, f64),
        lado: f32,
        grados: f32,
        color: [f32; 3],
        pincel: f32,
    ) {
        // Tambien descarta los NaN: un lado o una carga sin numero no estampa.
        if lado.is_nan() || pincel.is_nan() || lado <= 0.0 || pincel <= 0.0 {
            return;
        }
        // En doble precision: asi el mismo sello cae igual en el mapa de un
        // trazo entero que en el del trazo que va creciendo, aunque sus
        // esquinas esten en sitios distintos del dibujo.
        let lado = lado as f64;
        let n = LADO_DEL_SELLO as f64;
        // Multiplicar por la inversa y no dividir en cada pixel: la division
        // en doble precision era lo mas caro del sello.
        let inversa = n / lado;
        let (sen, cos) = (grados as f64).to_radians().sin_cos();
        let (tx, ty) = (centro.0 - lado / 2.0, centro.1 - lado / 2.0);
        let medio = n / 2.0;
        // La caja del cuadrado girado: media diagonal alrededor del centro,
        // mas el medio pixel que va del canto de un pixel a su centro, que es
        // donde se muestrea. Con un pixel entero de holgura, como antes, un
        // sello de dos pixeles miraba 36 y solo 4 caian dentro: el grafito
        // se iba en preguntar por pixeles que nunca tocaba.
        let radio = lado * std::f64::consts::FRAC_1_SQRT_2 + 0.5;
        let x0 = ((centro.0 - radio).floor().max(0.0)) as usize;
        let y0 = ((centro.1 - radio).floor().max(0.0)) as usize;
        let x1 = ((centro.0 + radio).ceil().max(0.0) as usize).min(self.ancho);
        let y1 = ((centro.1 + radio).ceil().max(0.0) as usize).min(self.alto);
        self.tocar(x0, y0, x1, y1);
        for py in y0..y1 {
            // Lo que depende solo de la fila, una vez por fila.
            let dy = (py as f64 + 0.5 - ty) * inversa - medio;
            let (desde_u, desde_v) = (sen * dy + medio, cos * dy + medio);
            for px in x0..x1 {
                // Del mapa al sello: quitar el sitio, deshacer la escala y
                // deshacer el giro alrededor del centro del sello.
                let dx = (px as f64 + 0.5 - tx) * inversa - medio;
                let u = cos * dx + desde_u;
                if !(0.0..n).contains(&u) {
                    continue;
                }
                let v = desde_v - sen * dx;
                if !(0.0..n).contains(&v) {
                    continue;
                }
                let t = bilineal(sello, u, v);
                if t <= 0.0 {
                    continue;
                }
                self.encima(py * self.ancho + px, color, (t / 255.0) as f32 * pincel);
            }
        }
    }

    /// **Un sello de figura, sacado de un molde ya muestreado.**
    ///
    /// Lo mismo que [`Self::estampar`], con el giro redondeado a
    /// `GIROS_DEL_MOLDE` pasos y el centro a un cuarto de pixel: con eso,
    /// los miles de sellos de una figura caben en unos pocos cientos de
    /// moldes, y cada sello pasa a ser sumar su molde, sin muestrear. Es lo
    /// que deja ver de grafito un rectangulo mientras se arrastra —se cuece
    /// entero en cada fotograma— sin que el lienzo vaya a tirones en un
    /// equipo modesto: medido, tres o cuatro veces menos por sello.
    ///
    /// Solo para las figuras: el trazo a mano sigue sello a sello exacto,
    /// que es lo que hace que estamparlo poco a poco de lo mismo que de una
    /// vez. En una raya de dos pixeles, un giro de seis grados y un cuarto de
    /// pixel no se ven.
    fn estampar_con_molde(
        &mut self,
        centro: (f64, f64),
        lado: f32,
        grados: f32,
        color: [f32; 3],
        pincel: f32,
    ) {
        if lado.is_nan() || pincel.is_nan() || lado <= 0.0 || pincel <= 0.0 {
            return;
        }
        let (bx, by) = (centro.0.floor(), centro.1.floor());
        let cuarto = |v: f64| ((v * 4.0) as u8).min(3);
        let (fx, fy) = (cuarto(centro.0 - bx), cuarto(centro.1 - by));
        let giro = ((grados / 360.0 * GIROS_DEL_MOLDE as f32).round() as i64)
            .rem_euclid(GIROS_DEL_MOLDE as i64) as u16;
        let llave = ((lado * 16.0).round() as u32, giro, fx, fy);
        MOLDES.with_borrow_mut(|moldes| {
            if moldes.len() > MOLDES_A_LA_VEZ {
                moldes.clear();
            }
            let molde = moldes.entry(llave).or_insert_with(|| Molde::nuevo(llave));
            let r = molde.radio as i64;
            let (x0, y0) = (bx as i64 - r, by as i64 - r);
            let lado_molde = molde.lado;
            let (x1, y1) = (x0 + lado_molde as i64, y0 + lado_molde as i64);
            let (cx0, cy0) = (x0.max(0) as usize, y0.max(0) as usize);
            let (cx1, cy1) = (
                (x1.max(0) as usize).min(self.ancho),
                (y1.max(0) as usize).min(self.alto),
            );
            self.tocar(cx0, cy0, cx1, cy1);
            for py in cy0..cy1 {
                let fila = (py as i64 - y0) as usize * lado_molde;
                for px in cx0..cx1 {
                    let a = molde.alfa[fila + (px as i64 - x0) as usize];
                    if a > 0.0 {
                        self.encima(py * self.ancho + px, color, a * pincel);
                    }
                }
            }
        });
    }

    /// Rellena un poligono (en pixeles del mapa) con suavizado: cuatro
    /// subfilas por fila y la cobertura exacta a lo ancho. Par/impar.
    fn rellenar(&mut self, poligono: &[(f32, f32)], color: [f32; 3], alfa: f32) {
        if poligono.len() < 3 || alfa.is_nan() || alfa <= 0.0 {
            return;
        }
        let ymin = poligono.iter().map(|p| p.1).fold(f32::MAX, f32::min);
        let ymax = poligono.iter().map(|p| p.1).fold(f32::MIN, f32::max);
        let y0 = (ymin.floor().max(0.0)) as usize;
        let y1 = (ymax.ceil().max(0.0) as usize).min(self.alto);
        const SUB: usize = 4;
        let sub = 1.0 / SUB as f32;
        // Lo tapado de cada pixel de la fila: los cantos, exactos, en
        // `cobertura`; lo de en medio, que se tapa entero, como un escalon en
        // `saltos` que se suma de izquierda a derecha al cerrar la fila. Asi
        // un fondo de mil pixeles de ancho cuesta dos sumas por subfila y no
        // mil: es lo que hacia que el bote de grafito tardara un mundo.
        let mut cobertura = vec![0f32; self.ancho + 1];
        let mut saltos = vec![0f32; self.ancho + 1];
        let mut cortes: Vec<f32> = Vec::new();
        for py in y0..y1 {
            let (mut desde, mut hasta) = (self.ancho, 0usize);
            for k in 0..SUB {
                let sy = py as f32 + (k as f32 + 0.5) / SUB as f32;
                cortes.clear();
                for i in 0..poligono.len() {
                    let a = poligono[i];
                    let b = poligono[(i + 1) % poligono.len()];
                    if (a.1 <= sy && b.1 > sy) || (b.1 <= sy && a.1 > sy) {
                        cortes.push(a.0 + (sy - a.1) / (b.1 - a.1) * (b.0 - a.0));
                    }
                }
                cortes.sort_by(f32::total_cmp);
                for par in cortes.chunks_exact(2) {
                    let (xa, xb) = (par[0].max(0.0), par[1].min(self.ancho as f32));
                    if xb <= xa {
                        continue;
                    }
                    let (ia, ib) = (xa.floor() as usize, (xb.ceil() as usize).min(self.ancho));
                    if ib <= ia {
                        continue;
                    }
                    desde = desde.min(ia);
                    hasta = hasta.max(ib);
                    if ib - ia == 1 {
                        cobertura[ia] += (xb - xa) * sub;
                        continue;
                    }
                    // El primero y el ultimo, a medias; los de en medio, enteros.
                    cobertura[ia] += (ia as f32 + 1.0 - xa) * sub;
                    cobertura[ib - 1] += (xb - (ib - 1) as f32) * sub;
                    saltos[ia + 1] += sub;
                    saltos[ib - 1] -= sub;
                }
            }
            if hasta <= desde {
                continue;
            }
            self.tocar(desde, py, hasta, py + 1);
            let mut lleno = 0.0f32;
            for x in desde..hasta {
                lleno += saltos[x];
                let c = (cobertura[x] + lleno).min(1.0);
                cobertura[x] = 0.0;
                saltos[x] = 0.0;
                if c > 1e-6 {
                    self.encima(py * self.ancho + x, color, alfa * c);
                }
            }
            saltos[hasta] = 0.0;
            cobertura[hasta] = 0.0;
        }
    }
}

/// En cuantos giros se redondea el de un sello de figura (5,6 grados).
const GIROS_DEL_MOLDE: u16 = 64;

/// Cuantos moldes se guardan a la vez: pasado, se tiran todos. Cada uno son
/// unas pocas decenas de numeros; los de un grosor de figura son unos mil.
const MOLDES_A_LA_VEZ: usize = 8192;

thread_local! {
    /// Los moldes ya muestreados, por `(lado * 16, giro, cuarto x, cuarto y)`.
    static MOLDES: RefCell<HashMap<(u32, u16, u8, u8), Molde>> = RefCell::new(HashMap::new());
}

/// Un sello ya muestreado para un lado, un giro y un cuarto de pixel: lo que
/// taparia en cada pixel de su caja, de 0 a 1.
struct Molde {
    /// Del pixel del centro al canto de la caja.
    radio: usize,
    /// La caja es de `lado` x `lado` pixeles.
    lado: usize,
    alfa: Vec<f32>,
}

impl Molde {
    fn nuevo((lado16, giro, fx, fy): (u32, u16, u8, u8)) -> Molde {
        let sello = sello();
        let lado_sello = lado16 as f64 / 16.0;
        let grados = giro as f64 * 360.0 / GIROS_DEL_MOLDE as f64;
        let radio = (lado_sello * std::f64::consts::FRAC_1_SQRT_2 + 0.5).ceil() as usize + 1;
        let lado = radio * 2 + 1;
        let mut alfa = vec![0f32; lado * lado];
        if lado_sello <= 0.0 {
            return Molde { radio, lado, alfa };
        }
        // La misma cuenta que `Lienzo::estampar`, con el centro en el medio
        // de su cuarto de pixel.
        let n = LADO_DEL_SELLO as f64;
        let inversa = n / lado_sello;
        let (sen, cos) = grados.to_radians().sin_cos();
        let centro = (
            radio as f64 + (fx as f64 + 0.5) / 4.0,
            radio as f64 + (fy as f64 + 0.5) / 4.0,
        );
        let (tx, ty) = (centro.0 - lado_sello / 2.0, centro.1 - lado_sello / 2.0);
        let medio = n / 2.0;
        for py in 0..lado {
            let dy = (py as f64 + 0.5 - ty) * inversa - medio;
            for px in 0..lado {
                let dx = (px as f64 + 0.5 - tx) * inversa - medio;
                let u = cos * dx + sen * dy + medio;
                let v = -sen * dx + cos * dy + medio;
                if (0.0..n).contains(&u) && (0.0..n).contains(&v) {
                    alfa[py * lado + px] = (bilineal(sello, u, v) / 255.0) as f32;
                }
            }
        }
        Molde { radio, lado, alfa }
    }
}

/// El sello muestreado en `(u, v)` con filtro bilineal; fuera, nada.
#[inline]
fn bilineal(sello: &[u8], u: f64, v: f64) -> f64 {
    let n = LADO_DEL_SELLO as i32;
    let (u, v) = (u - 0.5, v - 0.5);
    let (i0, j0) = (u.floor() as i32, v.floor() as i32);
    let (fx, fy) = (u - i0 as f64, v - j0 as f64);
    let t = |i: i32, j: i32| -> f64 {
        if i < 0 || j < 0 || i >= n || j >= n {
            0.0
        } else {
            sello[(j * n + i) as usize] as f64
        }
    };
    let arriba = t(i0, j0) * (1.0 - fx) + t(i0 + 1, j0) * fx;
    let abajo = t(i0, j0 + 1) * (1.0 - fx) + t(i0 + 1, j0 + 1) * fx;
    arriba * (1.0 - fy) + abajo * fy
}

// -------------------------------------------------------------------------
// El diente del papel
// -------------------------------------------------------------------------

/// **Cuanto coge del grafito la casilla `(x, y)` del dibujo**, de 0 a 1, y su
/// desvio de tono (de -`TONO_DEL_GRAFITO` a +`TONO_DEL_GRAFITO`).
///
/// La cuenta del movil, entera y con desbordamiento de 32 bits, bit a bit: es
/// lo que hace que la misma casilla salga igual aqui y en el telefono.
pub fn diente(x: i32, y: i32) -> (f32, f32) {
    let mut n = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263));
    n = (n ^ ((n as u32 >> 13) as i32)).wrapping_mul(1_274_126_177);
    let azar = ((n ^ ((n as u32 >> 16) as i32)) & 0xFFFF) as f32 / 65535.0;
    // Ninguna casilla se niega del todo: el 13 % es duro y coge poquisimo,
    // pero coge. Ver la cabecera.
    let coge = if azar < 0.13 {
        0.06 + 0.08 * (azar / 0.13)
    } else {
        0.35 + 0.65 * ((azar - 0.13) / 0.87)
    };
    let otro = (((n ^ ((n as u32 >> 7) as i32)).wrapping_mul(40503) as u32 >> 8) & 0xFF) as i32;
    let tono = (otro - 128) as f32 / 128.0 * TONO_DEL_GRAFITO;
    (coge, tono)
}

/// Pasa el mapa premultiplicado por el diente del papel y lo devuelve **sin
/// premultiplicar**, listo para subir.
fn pasar_el_diente(l: &Lienzo, x0: f32, y0: f32, finura: f32, agarre: f32) -> Vec<u8> {
    let mut salida = vec![0u8; l.px.len()];
    // Solo por donde se pinto: lo demas ya esta a cero.
    let origen = origen_en_casillas(x0, y0, finura);
    for r in l.tocado() {
        diente_en(l, r, origen, agarre, &mut salida);
    }
    salida
}

/// La casilla del dibujo en la que cae el pixel (0, 0) del mapa.
fn origen_en_casillas(x0: f32, y0: f32, finura: f32) -> (i32, i32) {
    ((x0 * finura).round() as i32, (y0 * finura).round() as i32)
}

/// Un rectangulo de pixeles del mapa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub ancho: usize,
    pub alto: usize,
}

impl Rect {
    fn vacio() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 0,
            alto: 0,
        }
    }

    fn es_vacio(self) -> bool {
        self.ancho == 0 || self.alto == 0
    }

    fn union(self, o: Rect) -> Rect {
        if self.es_vacio() {
            return o;
        }
        if o.es_vacio() {
            return self;
        }
        let (x0, y0) = (self.x.min(o.x), self.y.min(o.y));
        let x1 = (self.x + self.ancho).max(o.x + o.ancho);
        let y1 = (self.y + self.alto).max(o.y + o.alto);
        Rect {
            x: x0,
            y: y0,
            ancho: x1 - x0,
            alto: y1 - y0,
        }
    }
}

/// **El diente del papel en un trozo del mapa**: escribe `salida` en `r`
/// entero, tambien lo que queda vacio, que es lo que borra una cola vieja.
///
/// `agarre` es el de la dureza (`Dureza::agarre`): la mina dura coge menos de
/// cada casilla (mas grano) y la blanda tapa el grano.
fn diente_en(l: &Lienzo, r: Rect, (ox, oy): (i32, i32), agarre: f32, salida: &mut [u8]) {
    // `(1 - a)^coge` es `exp(coge * ln(1 - a))`, y el logaritmo solo tiene
    // 255 valores: sacado a una tabla, cada pixel paga una exponencial y no
    // una potencia, que en un fondo de grafito es casi todo lo que cuesta.
    static LOGARITMOS: OnceLock<[f64; 256]> = OnceLock::new();
    let logaritmos = LOGARITMOS.get_or_init(|| {
        let mut t = [0f64; 256];
        for (a, v) in t.iter_mut().enumerate().take(255) {
            *v = (1.0 - a as f64 / 255.0).ln();
        }
        t
    });
    for y in r.y..(r.y + r.alto).min(l.alto) {
        for x in r.x..(r.x + r.ancho).min(l.ancho) {
            let i = (y * l.ancho + x) * 4;
            let a = l.px[i + 3] as u32;
            salida[i..i + 4].fill(0);
            if a == 0 {
                continue;
            }
            // `getPixels` del movil entrega el color sin premultiplicar.
            let sin = |c: u8| ((c as u32 * 255 + a / 2) / a).min(255) as f32;
            let (r, g, b) = (sin(l.px[i]), sin(l.px[i + 1]), sin(l.px[i + 2]));
            let (coge, tono) = diente(x as i32 + ox, y as i32 + oy);
            let coge = coge * agarre;
            // Capas que se suman: `1 - (1 - a)^coge`. Cada repaso le anade un
            // poco a la vacia y otro poco a la tenue.
            let carga = if a >= 255 {
                1.0
            } else {
                1.0 - (coge as f64 * logaritmos[a as usize]).exp()
            };
            let fin = ((carga * 1.2 * 255.0) as i32).min(255);
            if fin < 2 {
                continue;
            }
            let hacia = if tono > 0.0 { 255.0 } else { 0.0 };
            let t = tono.abs();
            let tenir = |c: f32| (c * (1.0 - t) + hacia * t) as u8;
            salida[i] = tenir(r);
            salida[i + 1] = tenir(g);
            salida[i + 2] = tenir(b);
            salida[i + 3] = fin as u8;
        }
    }
}

// -------------------------------------------------------------------------
// Cocer
// -------------------------------------------------------------------------

fn a_bytes(c: ColorRgba) -> [f32; 3] {
    let k = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round();
    [k(c.r), k(c.g), k(c.b)]
}

/// La huella de todo lo que cambia el mapa. La version basta casi siempre; el
/// resto esta por si alguien toca un campo sin subirla.
fn huella_de(e: &Elemento) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mete = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    mete(e.id);
    mete(e.version as u64);
    mete(e.grosor.to_bits() as u64);
    mete(e.x.to_bits() as u64);
    mete(e.y.to_bits() as u64);
    mete(e.ancho.to_bits() as u64);
    mete(e.alto.to_bits() as u64);
    mete(e.angulo.to_bits() as u64);
    mete(e.rugosidad.to_bits() as u64);
    mete(e.semilla as u64);
    mete(es_constante(e) as u64);
    mete(e.redondo as u64);
    for c in [Some(e.trazo), e.relleno].into_iter().flatten() {
        for v in [c.r, c.g, c.b, c.a] {
            mete(v.to_bits() as u64);
        }
    }
    mete(e.estilo_relleno as u64);
    mete(Dureza::de(e) as u64);
    if let Some(p) = e.puntos() {
        mete(p.len() as u64);
        if let Some(u) = p.last() {
            mete(u.x.to_bits() as u64);
            mete(u.y.to_bits() as u64);
        }
    }
    h
}

/// El centro de giro: el de la caja, como en `pintado::ordenes`.
fn centro_de(e: &Elemento) -> Punto2 {
    let (x0, y0, x1, y1) = e.caja();
    Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

/// La esquina y la finura del mapa de una caja, encajada en la rejilla.
fn marco_del_mapa(
    minx: f32,
    miny: f32,
    maxx: f32,
    maxy: f32,
    margen: f32,
) -> Option<(f32, f32, f32, usize, usize)> {
    if !(minx.is_finite() && miny.is_finite() && maxx.is_finite() && maxy.is_finite()) {
        return None;
    }
    let x0 = ((minx - margen) / ENCAJE_DE_LA_REJILLA).floor() * ENCAJE_DE_LA_REJILLA;
    let y0 = ((miny - margen) / ENCAJE_DE_LA_REJILLA).floor() * ENCAJE_DE_LA_REJILLA;
    let (x1, y1) = (maxx + margen, maxy + margen);
    let mut finura = 1.0f32;
    while ((x1 - x0) * finura > LADO_MAXIMO_DEL_LAPIZ || (y1 - y0) * finura > LADO_MAXIMO_DEL_LAPIZ)
        && finura > 0.05
    {
        finura /= 2.0;
    }
    let ancho = (((x1 - x0) * finura) as usize).max(1);
    let alto = (((y1 - y0) * finura) as usize).max(1);
    Some((x0, y0, finura, ancho, alto))
}

/// **Cuece `e` sin guardar nada.** `None` si no es de grafito o si no hay de
/// que (un punto suelto): entonces se pinta liso, como en el movil.
pub fn cocer_sin_horno(e: &Elemento) -> Option<Cocido> {
    if !es_de_grafito(e) {
        return None;
    }
    let mut c = match &e.figura {
        Figura::Lapiz { .. } => Obra::nueva(&datos_del_trazo(e)?, e)?.1,
        _ => cocer_figura(e)?,
    };
    rematar(&mut c, e);
    Some(c)
}

/// Lo que el mapa sabe del elemento del que sale.
fn rematar(c: &mut Cocido, e: &Elemento) {
    c.id = e.id;
    c.huella = huella_de(e);
    c.angulo = e.angulo;
    c.centro = centro_de(e);
}

/// Lo que hace falta para estampar un trazo, sacado de una vez del elemento.
struct Datos {
    crudos: Vec<Punto2>,
    /// La curva que pasa por las muestras (`curva_del_trazo`).
    puntos: Vec<[f64; 2]>,
    presiones: Option<Vec<f32>>,
    /// El punto de la curva en el que cae la penultima muestra: lo de antes
    /// ya no cambia aunque llegue otra (ver `lo_asentado`).
    fijo_hasta: usize,
    gordo: f32,
    /// La dureza de la mina: su carga y su agarre (el grueso ya va en
    /// `gordo`).
    dureza: Dureza,
    tinta: [f32; 3],
    largo: f64,
    semilla: u64,
    /// **Presion: constante** en el panel. El grafito es un lapiz mas y
    /// obedece al mismo mando: sin presion, cada sello va con la carga y el
    /// grueso del cuerpo y las puntas no se afinan, como el trazo liso de
    /// grosor fijo de Excalidraw.
    constante: bool,
}

/// Si el trazo pide grosor fijo (`variability: "constant"`).
fn es_constante(e: &Elemento) -> bool {
    matches!(
        &e.figura,
        Figura::Lapiz { opciones: Some(o), .. } if o.variabilidad == super::Variabilidad::Constante
    )
}

fn datos_del_trazo(e: &Elemento) -> Option<Datos> {
    let Figura::Lapiz {
        puntos: crudos,
        presiones,
        opciones,
    } = &e.figura
    else {
        return None;
    };
    if crudos.len() < 2 {
        return None;
    }
    // Un trazo de antes de E1 guardaba pixeles, no `strokeWidth`.
    let ancho = if opciones.is_some() {
        e.grosor
    } else {
        e.grosor / super::FACTOR_VARIABLE
    };
    let dureza = Dureza::de(e);
    let gordo = (ancho * GORDO_DEL_LAPIZ * dureza.gordo()).max(1.5);
    // **Sin corregir nada** (v0.91 del movil, «el grafito es escritura
    // libre»). Antes pasaba dos veces por `asentar_lo_rapido`, que acerca
    // cada muestra rapida al medio de sus vecinas: aplanaba las curvas
    // rapidas en tramos rectos —«un poligono, se nota en las esquinas»— y el
    // trazo no quedaba donde se puso. Ahora van **las muestras tal cual**, y
    // entre una y otra la curva que pasa por todas, cada `gordo * 0,3`.
    let muestras: Vec<[f64; 2]> = crudos.iter().map(|p| [p.x as f64, p.y as f64]).collect();
    let constante = es_constante(e);
    // Constante manda sobre el lapiz de verdad: el mando dice «sin presion»
    // y asi sale, se pinte con lo que se pinte.
    let con_presion = (!constante && presiones.len() == muestras.len()).then_some(&presiones[..]);
    let (puntos, presiones, indices) =
        curva_del_trazo(&muestras, con_presion, (gordo as f64 * 0.3).max(0.75));
    if puntos.len() < 2 {
        return None;
    }
    let largo = puntos
        .windows(2)
        .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
        .sum();
    // La penultima muestra: el tramo que sale de ella cambia cuando llega
    // otra (su vecino de despues deja de ser un reflejo), los de antes no.
    let fijo_hasta = indices
        .len()
        .checked_sub(2)
        .and_then(|k| indices.get(k).copied())
        .unwrap_or(0);
    Some(Datos {
        crudos: crudos.clone(),
        presiones,
        puntos,
        fijo_hasta,
        gordo,
        dureza,
        tinta: a_bytes(e.trazo),
        largo,
        semilla: e.id,
        constante,
    })
}

/// La carga del pincel para una presion `p`: *Opacity* 79 % por una carga
/// que sube con la presion. En bytes, como el `Paint.alpha` del movil.
///
/// La dureza la multiplica (`dureza.oscuro` del movil): la 8B carga mas de
/// dos veces lo que la HB en cada sello, y la 4H menos de la mitad.
fn carga_del_pincel(p: f32, dureza: Dureza) -> f32 {
    ((255.0 * OPACIDAD_DEL_LAPIZ * (0.16 + 0.34 * p) * dureza.oscuro()) as i32).clamp(1, 255) as f32
        / 255.0
}

/// El lado de un sello: el cuerpo, atado a la presion, y la punta afinada.
fn lado_del_sello(gordo: f32, p: f32, punta: f32) -> f32 {
    gordo
        * (0.55 + 0.45 * p)
        * (PUNTA_DEL_GRAFITO + (1.0 - PUNTA_DEL_GRAFITO) * punta * (2.0 - punta))
}

/// El paso del movil: el 10 % del sello, salvo en lo larguisimo.
///
/// **En lo larguisimo, por escalones del 10 %.** El movil alarga el paso con
/// el largo (`largo / SELLOS_POR_TRAZO`) y reestampa el trazo entero en cada
/// fotograma. Aqui el trazo en curso se estampa solo por lo nuevo y no puede
/// cambiar de paso a cada punto; antes lo dejaba fijo con un margen y, al
/// quedarse quieto, lo recocia con el exacto: el trazo entero salia de golpe
/// mas claro y fino al soltarlo («cuando suelto el dibujo se achica»). Con el
/// paso redondeado hacia arriba al escalon, el trazo en curso lleva ya el
/// mismo paso que tendra cocido de cero: cambia solo al subir de escalon
/// (entonces se estampa entero, como antes al pasarse del margen) y soltarlo
/// no lo toca. Frente al movil, a lo sumo un 10 % menos de sellos, que es
/// para lo que esta ese tope.
fn paso_de(d: &Datos, finura: f32) -> f32 {
    let base = d.gordo * ESPACIADO_DEL_LAPIZ;
    let por_largo = (d.largo / SELLOS_POR_TRAZO) as f32;
    let paso = if por_largo > base {
        let escalon = 1.0 + DERIVA_DEL_PASO;
        base * escalon.powi((por_largo / base).ln().div_euclid(escalon.ln()) as i32 + 1)
    } else {
        base
    };
    paso.max(0.4 / finura)
}

/// **Por donde va el estampado de un trazo**: `estamparElLapiz` del movil
/// partido en un estado que se puede guardar y seguir.
///
/// Es literal —el mismo paso, la misma presion simulada con el dedo y el
/// mismo orden de azar—, y ademas se puede parar en un sello y seguir luego
/// desde ahi sin que cambie nada: es lo que deja estampar el trazo en curso
/// **solo por lo nuevo** en vez de entero en cada fotograma.
#[derive(Clone)]
struct Paseo {
    /// El tramo por el que va: de `puntos[i - 1]` a `puntos[i]`.
    i: usize,
    ax: f64,
    ay: f64,
    pendiente: f32,
    recorrido: f64,
    azar: AzarJava,
}

/// Un sello por estampar: donde (en el dibujo), de que lado (en el dibujo),
/// girado cuanto y con que carga.
type Sello = ((f64, f64), f32, f32, f32);

impl Paseo {
    fn nuevo(d: &Datos) -> Paseo {
        Paseo {
            i: 1,
            ax: d.puntos[0][0],
            ay: d.puntos[0][1],
            pendiente: 0.0,
            recorrido: 0.0,
            azar: AzarJava::nuevo(d.semilla),
        }
    }

    /// Anda estampando hasta el final o hasta `tope` —el ultimo tramo y el
    /// ultimo recorrido que se pueden dar por buenos—, sin pasarse: si el
    /// sello siguiente cae mas alla, se para **antes** de el.
    fn andar(
        &mut self,
        d: &Datos,
        paso: f32,
        tope: Option<(usize, f64)>,
        mut estampa: impl FnMut(Sello),
    ) {
        let largo = d.largo;
        while self.i < d.puntos.len() {
            let i = self.i;
            if tope.is_some_and(|(hasta, _)| i > hasta) {
                return;
            }
            let (bx, by) = (d.puntos[i][0], d.puntos[i][1]);
            let mut tramo = ((bx - self.ax).powi(2) + (by - self.ay).powi(2)).sqrt() as f32;
            while tramo > 0.0 && self.pendiente + tramo >= paso {
                let t = (paso - self.pendiente) / tramo;
                let recorrido = self.recorrido + (paso - self.pendiente) as f64;
                if tope.is_some_and(|(_, hasta)| recorrido > hasta) {
                    return;
                }
                self.ax += (bx - self.ax) * t as f64;
                self.ay += (by - self.ay) * t as f64;
                self.recorrido = recorrido;
                tramo = ((bx - self.ax).powi(2) + (by - self.ay).powi(2)).sqrt() as f32;
                self.pendiente = 0.0;
                let hasta_la_punta = recorrido.min(largo - recorrido) as f32;
                // La presion: la del lapiz si la hay; con el dedo o el raton,
                // media, afinando en las dos puntas.
                // Con la presion constante, la del cuerpo en todo el trazo.
                let p = match &d.presiones {
                    _ if d.constante => 0.55,
                    Some(pr) => (pr[i - 1] + (pr[i] - pr[i - 1]) * t).clamp(0.05, 1.0),
                    None => 0.55 * (hasta_la_punta / (d.gordo * 3.0)).clamp(0.25, 1.0),
                };
                // Empieza y acaba delgadito; el cuerpo, a su grosor. Constante,
                // sin afinar.
                let punta = if d.constante {
                    1.0
                } else {
                    (hasta_la_punta / (d.gordo * LARGO_DE_LA_PUNTA)).clamp(0.0, 1.0)
                };
                let lado = lado_del_sello(d.gordo, p, punta);
                let grados = self.azar.flotante() * 360.0;
                estampa((
                    (self.ax, self.ay),
                    lado,
                    grados,
                    carga_del_pincel(p, d.dureza),
                ));
            }
            self.pendiente += tramo;
            self.recorrido += tramo as f64;
            self.ax = bx;
            self.ay = by;
            self.i += 1;
        }
    }
}

/// **Lo que ya no va a cambiar de un trazo que crece**: hasta que tramo y
/// hasta que recorrido.
///
/// Una muestra nueva al final cambia, en la curva, solo el tramo que sale de
/// la penultima (su vecino de despues deja de ser un reflejo; ver
/// `curva_del_trazo`); y la punta afinada y la presion simulada miran lo que
/// falta hasta el final en `LARGO_DE_LA_PUNTA` gruesos. Lo que queda antes de
/// las dos cosas sale igual lo estampe quien lo estampe y cuando sea: se
/// estampa una vez y ya.
fn lo_asentado(d: &Datos) -> (usize, f64) {
    let cola = (d.gordo * LARGO_DE_LA_PUNTA.max(3.0)) as f64;
    (d.fijo_hasta, d.largo - cola)
}

/// Estampa `s` en `l`, cuyo pixel (0, 0) cae en la casilla `origen` del
/// dibujo a `finura`.
/// `desde` es el pixel del mapa en el que empieza `l` (un trozo del mapa).
fn estampar_en(
    l: &mut Lienzo,
    s: Sello,
    origen: (f32, f32),
    finura: f32,
    desde: (usize, usize),
    tinta: [f32; 3],
) {
    let ((x, y), lado, grados, pincel) = s;
    let f = finura as f64;
    let centro = (
        (x - origen.0 as f64) * f - desde.0 as f64,
        (y - origen.1 as f64) * f - desde.1 as f64,
    );
    l.estampar(sello(), centro, lado * finura, grados, tinta, pincel);
}

/// Los pixeles del mapa que puede tocar un sello, acotados al mapa.
fn huella_del_sello(s: Sello, origen: (f32, f32), finura: f32, ancho: usize, alto: usize) -> Rect {
    let ((x, y), lado, _, _) = s;
    let (cx, cy) = (
        (x as f32 - origen.0) * finura,
        (y as f32 - origen.1) * finura,
    );
    let r = lado * finura * std::f32::consts::FRAC_1_SQRT_2 + 2.0;
    let x0 = (cx - r).floor().max(0.0) as usize;
    let y0 = (cy - r).floor().max(0.0) as usize;
    let x1 = ((cx + r).ceil().max(0.0) as usize).min(ancho);
    let y1 = ((cy + r).ceil().max(0.0) as usize).min(alto);
    if x1 <= x0 || y1 <= y0 {
        return Rect::vacio();
    }
    Rect {
        x: x0,
        y: y0,
        ancho: x1 - x0,
        alto: y1 - y0,
    }
}

/// El escalon del paso en lo larguisimo (ver `paso_de`): donde el movil
/// alarga el paso con el largo (`SELLOS_POR_TRAZO`) y por eso reestampa el
/// trazo entero en cada fotograma, aqui el paso sube a saltos del 10 % y el
/// trazo en curso se reestampa una vez por salto.
const DERIVA_DEL_PASO: f32 = 0.10;

/// Holgura con la que crece el mapa de un trazo que se sale de el mientras se
/// dibuja: asi no se rehace el mapa en cada punto que asoma por un borde.
const HOLGURA_AL_CRECER: f32 = 128.0;

/// **Un trazo a medio estampar**: lo asentado en su lienzo, y por donde va.
///
/// Es lo que hace barato el trazo en curso. Cada fotograma solo se estampa lo
/// que se asento desde el anterior y la cola —la punta afinada y los ultimos
/// puntos, que si cambian—, y solo se pasa el diente del papel por ahi.
struct Obra {
    crudos: Vec<Punto2>,
    con_presiones: bool,
    gordo: f32,
    dureza: Dureza,
    tinta: [f32; 3],
    semilla: u64,
    constante: bool,
    paso: f32,
    x0: f32,
    y0: f32,
    finura: f32,
    /// Solo lo asentado, antes del diente del papel.
    lienzo: Lienzo,
    paseo: Paseo,
    /// Donde se pinto la cola la ultima vez: hay que devolverlo a lo asentado.
    cola: Rect,
}

impl Obra {
    /// Estampa el trazo entero desde cero.
    fn nueva(d: &Datos, e: &Elemento) -> Option<(Obra, Cocido)> {
        let (minx, miny, maxx, maxy) = caja_de(&d.puntos);
        let (x0, y0, finura, ancho, alto) = marco_del_mapa(minx, miny, maxx, maxy, d.gordo)?;
        let mut obra = Obra {
            crudos: d.crudos.clone(),
            con_presiones: d.presiones.is_some(),
            gordo: d.gordo,
            dureza: d.dureza,
            tinta: d.tinta,
            semilla: d.semilla,
            constante: d.constante,
            paso: paso_de(d, finura),
            x0,
            y0,
            finura,
            lienzo: Lienzo::nuevo(ancho, alto),
            paseo: Paseo::nuevo(d),
            cola: Rect::vacio(),
        };
        let (paso, origen, tinta) = (obra.paso, (x0, y0), d.tinta);
        let lienzo = &mut obra.lienzo;
        obra.paseo.andar(d, paso, Some(lo_asentado(d)), |s| {
            estampar_en(lienzo, s, origen, finura, (0, 0), tinta)
        });
        let mut rgba = pasar_el_diente(&obra.lienzo, x0, y0, finura, d.dureza.agarre());
        obra.pintar_la_cola(d, &mut rgba);
        let mut c = Cocido {
            rgba,
            ancho: ancho as u32,
            alto: alto as u32,
            x0,
            y0,
            finura,
            huella: 0,
            angulo: 0.0,
            centro: Punto2::nuevo(0.0, 0.0),
            id: 0,
            generacion: nueva_generacion(),
            sucio: None,
        };
        rematar(&mut c, e);
        Some((obra, c))
    }

    fn peso(&self) -> usize {
        self.lienzo.px.len() + self.crudos.len() * std::mem::size_of::<Punto2>()
    }

    /// Devuelve a lo asentado lo que ocupo la cola anterior, estampa la cola
    /// de ahora en una copia de su trozo y la pone en `rgba`. Devuelve todo
    /// lo que ha tocado.
    fn pintar_la_cola(&mut self, d: &Datos, rgba: &mut [u8]) -> Rect {
        let origen_casillas = origen_en_casillas(self.x0, self.y0, self.finura);
        let antes = self.cola;
        let agarre = self.dureza.agarre();
        diente_en(&self.lienzo, antes, origen_casillas, agarre, rgba);
        let mut sellos: Vec<Sello> = Vec::new();
        self.paseo
            .clone()
            .andar(d, self.paso, None, |s| sellos.push(s));
        let (ancho, alto) = (self.lienzo.ancho, self.lienzo.alto);
        let origen = (self.x0, self.y0);
        let r = sellos
            .iter()
            .map(|s| huella_del_sello(*s, origen, self.finura, ancho, alto))
            .fold(Rect::vacio(), Rect::union);
        self.cola = r;
        if r.es_vacio() {
            return antes;
        }
        let mut trozo = self.lienzo.recorte(r);
        for s in &sellos {
            estampar_en(&mut trozo, *s, origen, self.finura, (r.x, r.y), self.tinta);
        }
        let mut salida = vec![0u8; trozo.px.len()];
        let todo = Rect {
            x: 0,
            y: 0,
            ancho: r.ancho,
            alto: r.alto,
        };
        diente_en(
            &trozo,
            todo,
            (
                origen_casillas.0 + r.x as i32,
                origen_casillas.1 + r.y as i32,
            ),
            agarre,
            &mut salida,
        );
        for fila in 0..r.alto {
            let de = fila * r.ancho * 4;
            let a = ((r.y + fila) * ancho + r.x) * 4;
            rgba[a..a + r.ancho * 4].copy_from_slice(&salida[de..de + r.ancho * 4]);
        }
        antes.union(r)
    }

    /// **Sigue estampando un trazo que ha crecido.** `false` si no se puede
    /// seguir —no es el mismo trazo con puntos de mas, o cambio lo que no se
    /// sigue— y hay que cocerlo entero.
    fn seguir(&mut self, d: &Datos, c: &mut Cocido) -> bool {
        let mismo = d.crudos.len() >= self.crudos.len()
            && d.crudos[..self.crudos.len()] == self.crudos[..]
            && d.gordo == self.gordo
            && d.dureza == self.dureza
            && d.tinta == self.tinta
            && d.semilla == self.semilla
            && d.constante == self.constante
            && d.presiones.is_some() == self.con_presiones;
        if !mismo {
            return false;
        }
        // Subio de escalon (`paso_de`): con otro paso caen otros sellos desde
        // el principio, asi que se estampa entero.
        if paso_de(d, self.finura) != self.paso {
            return false;
        }
        let mut sucio = Rect::vacio();
        let mut entero = false;
        // Si se sale del mapa, se agranda con holgura, copiando lo que habia:
        // la rejilla es la misma, asi que lo copiado cae en sus casillas.
        let (minx, miny, maxx, maxy) = caja_de(&d.puntos);
        let (w, h) = (
            self.lienzo.ancho as f32 / self.finura,
            self.lienzo.alto as f32 / self.finura,
        );
        let g = self.gordo;
        if minx - g < self.x0
            || miny - g < self.y0
            || maxx + g > self.x0 + w
            || maxy + g > self.y0 + h
        {
            // **A la misma finura, aunque sea con menos holgura.** Antes se
            // pedia siempre la holgura entera y solo a finura 1: un trazo
            // que pasaba de unas 1.800 unidades (media hoja de PDF con los
            // margenes, o un garabato largo) ya no cabia con ella en 2.048
            // casillas, `seguir` decia que no, y se estampaba ENTERO en cada
            // punto nuevo —medido: 2.691 cocciones enteras en 3.000 puntos,
            // 37 ms por punto y subiendo—; y pasado de 2.048, a media
            // finura, tambien, siempre. Ese era el lag que crecia con el
            // trazo. Se prueba con menos holgura antes de rendirse, y a la
            // finura que ya tiene el mapa (la rejilla encaja en multiplos de
            // `ENCAJE_DE_LA_REJILLA`, asi que a media o a un cuarto de finura
            // lo copiado sigue cayendo en casillas enteras).
            let marco = [HOLGURA_AL_CRECER, HOLGURA_AL_CRECER / 4.0, 0.0]
                .into_iter()
                .filter_map(|holgura| {
                    marco_del_mapa(
                        minx.min(self.x0 + g),
                        miny.min(self.y0 + g),
                        maxx.max(self.x0 + w - g),
                        maxy.max(self.y0 + h - g),
                        g + holgura,
                    )
                })
                .find(|m| m.2 == self.finura);
            let Some((x0, y0, finura, ancho, alto)) = marco else {
                return false;
            };
            let (dxf, dyf) = ((self.x0 - x0) * finura, (self.y0 - y0) * finura);
            if dxf.fract() != 0.0 || dyf.fract() != 0.0 || dxf < 0.0 || dyf < 0.0 {
                return false;
            }
            let (dx, dy) = (dxf as usize, dyf as usize);
            let mut lienzo = Lienzo::nuevo(ancho, alto);
            let mut rgba = vec![0u8; ancho * alto * 4];
            let (wv, hv) = (self.lienzo.ancho, self.lienzo.alto);
            for fila in 0..hv.min(alto.saturating_sub(dy)) {
                let n = wv.min(ancho.saturating_sub(dx)) * 4;
                let de = fila * wv * 4;
                let a = ((fila + dy) * ancho + dx) * 4;
                lienzo.px[a..a + n].copy_from_slice(&self.lienzo.px[de..de + n]);
                rgba[a..a + n].copy_from_slice(&c.rgba[de..de + n]);
            }
            // Lo copiado cuenta como pintado: si alguien pasara el diente al
            // mapa entero, tiene que pasar tambien por ahi.
            lienzo.tocar(dx, dy, dx + wv, dy + hv);
            self.cola.x += dx;
            self.cola.y += dy;
            self.lienzo = lienzo;
            (self.x0, self.y0) = (x0, y0);
            c.rgba = rgba;
            (c.ancho, c.alto, c.x0, c.y0) = (ancho as u32, alto as u32, x0, y0);
            entero = true;
        }
        // Lo que se asento desde la ultima vez, al lienzo y con su diente.
        let (origen, finura, tinta, paso) =
            ((self.x0, self.y0), self.finura, self.tinta, self.paso);
        let (ancho, alto) = (self.lienzo.ancho, self.lienzo.alto);
        let lienzo = &mut self.lienzo;
        self.paseo.andar(d, paso, Some(lo_asentado(d)), |s| {
            sucio = sucio.union(huella_del_sello(s, origen, finura, ancho, alto));
            estampar_en(lienzo, s, origen, finura, (0, 0), tinta);
        });
        diente_en(
            &self.lienzo,
            sucio,
            origen_en_casillas(self.x0, self.y0, finura),
            self.dureza.agarre(),
            &mut c.rgba,
        );
        sucio = sucio.union(self.pintar_la_cola(d, &mut c.rgba));
        self.crudos
            .extend_from_slice(&d.crudos[self.crudos.len()..]);
        let desde = c.generacion;
        c.generacion = nueva_generacion();
        c.sucio = if entero { None } else { Some((desde, sucio)) };
        true
    }
}

/// La caja de unos puntos.
fn caja_de(puntos: &[[f64; 2]]) -> (f32, f32, f32, f32) {
    let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in puntos {
        let (x, y) = (p[0] as f32, p[1] as f32);
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
    }
    (minx, miny, maxx, maxy)
}

/// Recorre unos caminos a sellos (`estamparElCamino`): la raya de una figura
/// o su rayado. Presion fija; `con_puntas` afina los extremos de lo abierto.
#[allow(clippy::too_many_arguments)]
fn estampar_caminos(
    lienzo: &mut Lienzo,
    azar: &mut AzarJava,
    caminos: &[Vec<Punto2>],
    gordo: f32,
    finura: f32,
    origen: (f32, f32),
    tinta: [f32; 3],
    con_puntas: bool,
    dureza: Dureza,
) {
    let largo_de = |c: &[Punto2]| -> f32 { c.windows(2).map(|w| w[0].distancia(w[1])).sum() };
    let total: f32 = caminos.iter().map(|c| largo_de(c)).sum();
    let paso = (gordo * ESPACIADO_DEL_LAPIZ)
        .max((total as f64 / SELLOS_POR_TRAZO) as f32)
        .max(0.4 / finura);
    let p = 0.6;
    let pincel = carga_del_pincel(p, dureza);
    for c in caminos.iter().filter(|c| c.len() >= 2) {
        let largo = largo_de(c);
        let cerrado = c[0].distancia(*c.last().expect("dos puntos")) < 1e-3;
        let afila = con_puntas && !cerrado;
        let mut d = 0.0f32;
        // El tramo por el que va y lo recorrido hasta su principio.
        let (mut k, mut antes) = (0usize, 0.0f32);
        while d <= largo {
            while k + 2 < c.len() && antes + c[k].distancia(c[k + 1]) < d {
                antes += c[k].distancia(c[k + 1]);
                k += 1;
            }
            let (a, b) = (c[k], c[k + 1]);
            let l = a.distancia(b);
            let t = if l > 0.0 {
                ((d - antes) / l).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let sitio = (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            let punta = if afila {
                (d.min(largo - d) / (gordo * LARGO_DE_LA_PUNTA)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let lado = lado_del_sello(gordo, p, punta) * finura;
            let grados = azar.flotante() * 360.0;
            let centro = (
                ((sitio.0 - origen.0) * finura) as f64,
                ((sitio.1 - origen.1) * finura) as f64,
            );
            lienzo.estampar_con_molde(centro, lado, grados, tinta, pincel);
            d += paso;
        }
    }
}

/// **Una figura de grafito** (`cocerLaFigura`): su raya recorrida a sellos, su
/// fondo liso tendido flojo, y su rayado a rayas de grafito mas finas.
///
/// Las piezas salen de `pintado::ordenes` de la misma figura **lisa y sin
/// girar**: asi la raya temblona de rough.js y el rayado ya recortado a la
/// silueta son exactamente los de la figura lisa, sin una segunda verdad.
fn cocer_figura(e: &Elemento) -> Option<Cocido> {
    let mut lisa = e.clone();
    lisa.angulo = 0.0;
    lisa.material = super::MaterialTinta::Lisa;
    let todas = crate::pintado::ordenes(&lisa);
    // El fondo va primero en las figuras cerradas: se cuenta cuantas ordenes
    // pone quitandolo y mirando cuantas faltan. El rayado usa su propio azar,
    // asi que la raya sale igual con fondo o sin el.
    let cerrada = matches!(
        e.figura,
        Figura::Rectangulo | Figura::Rombo | Figura::Elipse | Figura::Region { .. }
    );
    // El relleno del bote **no tiene raya** (`anillosDeRegion` del movil: su
    // camino de trazo va vacio). El borde ya lo dibujan las figuras que lo
    // encierran; repasarlo a sellos por dentro dejaria una doble linea de
    // grafito justo donde el ojo espera una.
    let sin_raya = matches!(e.figura, Figura::Region { .. });
    let con_fondo = cerrada && e.relleno.is_some_and(|c| c.a > 0.0);
    let de_fondo = if con_fondo {
        let mut sin = lisa.clone();
        sin.relleno = None;
        todas
            .len()
            .saturating_sub(crate::pintado::ordenes(&sin).len())
    } else {
        0
    };
    let (fondo, raya) = todas.split_at(de_fondo.min(todas.len()));

    let mut caminos_fondo = Vec::new();
    let mut manchas_fondo = Vec::new();
    for o in fondo {
        match o {
            Orden::Relleno { puntos, .. } | Orden::Poligono { puntos, .. } => {
                manchas_fondo.push(puntos.clone())
            }
            Orden::Polilinea { puntos, .. } => caminos_fondo.push(puntos.clone()),
            _ => {}
        }
    }
    let mut caminos = Vec::new();
    let mut manchas = Vec::new();
    for o in raya.iter().filter(|_| !sin_raya) {
        match o {
            Orden::Polilinea { puntos, .. } => caminos.push(puntos.clone()),
            // Las puntas macizas de una flecha: tinta tendida, como el fondo.
            Orden::Relleno { puntos, .. } | Orden::Poligono { puntos, .. } => {
                manchas.push(puntos.clone())
            }
            _ => {}
        }
    }
    let todos = caminos
        .iter()
        .chain(&caminos_fondo)
        .chain(&manchas)
        .chain(&manchas_fondo)
        .flatten();
    let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in todos {
        minx = minx.min(p.x);
        miny = miny.min(p.y);
        maxx = maxx.max(p.x);
        maxy = maxy.max(p.y);
    }
    // Las figuras miden su grosor en el doble de escala que el lapiz. Y la
    // dureza las engorda igual que al trazo (`cocerLaFigura` del movil).
    let dureza = Dureza::de(e);
    let gordo = (e.grosor * 0.5 * GORDO_DEL_LAPIZ * dureza.gordo()).max(1.5);
    let (x0, y0, finura, ancho, alto) = marco_del_mapa(minx, miny, maxx, maxy, gordo)?;
    let mut lienzo = Lienzo::nuevo(ancho, alto);
    let origen = (x0, y0);
    let a_mapa = |ps: &Vec<Punto2>| -> Vec<(f32, f32)> {
        ps.iter()
            .map(|p| ((p.x - x0) * finura, (p.y - y0) * finura))
            .collect()
    };
    let tinta = a_bytes(e.trazo);
    let fondo_color = a_bytes(e.relleno.unwrap_or(e.trazo));
    // Un solo azar para todo, sembrado con el elemento: no hierve al repintar.
    let mut azar = AzarJava::nuevo(e.id);
    // `(255 * 0.55).toInt()`: el alfa en bytes, como el movil.
    let flojo = (255.0 * FONDO_DE_GRAFITO) as i32 as f32 / 255.0;
    if con_fondo {
        // Sin rayas que recorrer, liso: el relleno del bote sale siempre
        // macizo en el PC (`pintado`), y sin esto uno rayado de grafito no
        // pintaba nada.
        if e.estilo_relleno == EstiloRelleno::Solido || caminos_fondo.is_empty() {
            for m in &manchas_fondo {
                lienzo.rellenar(&a_mapa(m), fondo_color, flojo);
            }
        } else {
            estampar_caminos(
                &mut lienzo,
                &mut azar,
                &caminos_fondo,
                gordo * 0.6,
                finura,
                origen,
                fondo_color,
                false,
                dureza,
            );
        }
    }
    for m in &manchas {
        lienzo.rellenar(&a_mapa(m), tinta, flojo);
    }
    estampar_caminos(
        &mut lienzo,
        &mut azar,
        &caminos,
        gordo,
        finura,
        origen,
        tinta,
        true,
        dureza,
    );
    Some(Cocido {
        rgba: pasar_el_diente(&lienzo, x0, y0, finura, dureza.agarre()),
        ancho: ancho as u32,
        alto: alto as u32,
        x0,
        y0,
        finura,
        huella: 0,
        angulo: 0.0,
        centro: Punto2::nuevo(0.0, 0.0),
        id: 0,
        generacion: 0,
        sucio: None,
    })
}

// -------------------------------------------------------------------------
// El horno
// -------------------------------------------------------------------------

/// Un mapa cocido y, si es de un trazo a mano, por donde iba su estampado.
struct Hornada {
    cocido: Rc<Cocido>,
    obra: Option<Obra>,
    uso: u64,
}

impl Hornada {
    fn peso(&self) -> usize {
        self.cocido.peso() + self.obra.as_ref().map_or(0, Obra::peso)
    }
}

/// Los mapas ya cocidos, por elemento, con tope de peso.
///
/// Es **por hilo** y no un campo de quien pinta a proposito: las ordenes de
/// dibujo (`pintado::Orden`) no pueden llevar un mapa de pixeles sin obligar a
/// todos los que pintan ordenes —el pin, el chat, la capa— a saber pintarlo, y
/// el editor, que es el unico que lo pinta, lo pide desde el mismo hilo que
/// pinta. Cada hilo que pida grafito cuece el suyo; no se comparte nada.
///
/// La clave es `(espacio, id)` y no solo el id: en un mismo hilo se pintan
/// escenas distintas cuyos ids se repiten —cada hoja de un PDF numera los
/// suyos desde 1, y el lienzo y su hojita tambien—. Con el id a secas, el
/// elemento 1 de una hoja echaba del horno al elemento 1 de la otra y los
/// dos se recocian enteros en CADA fotograma (ver [`en_espacio`]).
#[derive(Default)]
struct Horno {
    mapas: HashMap<(u64, u64), Hornada>,
    reloj: u64,
}

impl Horno {
    fn peso(&self) -> usize {
        self.mapas.values().map(Hornada::peso).sum()
    }

    /// Los mas viejos, fuera, hasta caber; `id`, el recien cocido, se queda.
    fn caber(&mut self, id: (u64, u64)) {
        let mut peso = self.peso();
        while peso > PESO_DE_LOS_LAPICES {
            let Some(mas_viejo) = self
                .mapas
                .iter()
                .filter(|(k, _)| **k != id)
                .min_by_key(|(_, h)| h.uso)
                .map(|(k, _)| *k)
            else {
                break;
            };
            if let Some(h) = self.mapas.remove(&mas_viejo) {
                peso -= h.peso();
            }
        }
    }
}

thread_local! {
    static HORNO: RefCell<Horno> = RefCell::new(Horno::default());
    static MARCA: RefCell<Option<Rc<Cocido>>> = const { RefCell::new(None) };
    static GENERACION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static ESPACIO: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static COCCIONES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// **Cuece dentro del espacio `espacio`** lo que se pida durante `f`: los
/// elementos de una escena no se pisan en el horno con los de otra que tenga
/// los mismos ids. Fuera de toda llamada el espacio es el 0, el de siempre
/// (el lienzo); quien pinta otra escena en el mismo hilo —cada hoja del
/// lector, la hojita del lienzo— la pinta dentro del suyo. Se anida: al
/// acabar vuelve el de antes.
pub fn en_espacio<R>(espacio: u64, f: impl FnOnce() -> R) -> R {
    let antes = ESPACIO.with(|e| e.replace(espacio));
    let r = f();
    ESPACIO.with(|e| e.set(antes));
    r
}

/// Cuantas veces se ha cocido un mapa entero en este hilo (no cuenta seguir
/// un trazo que crece ni devolver uno ya cocido): lo que mide si el horno
/// esta sirviendo o recociendo en cada fotograma.
pub fn cocciones() -> u64 {
    COCCIONES.with(std::cell::Cell::get)
}

/// Un numero que no se ha dado nunca en este hilo: dice de que mapa exacto se
/// habla cuando se pide subir solo lo que cambio.
fn nueva_generacion() -> u64 {
    GENERACION.with(|g| {
        g.set(g.get() + 1);
        g.get()
    })
}

/// **El grafito de `e`, cocido o recien cocido.** `None` si no es de grafito o
/// no hay de que: quien pinta entonces lo pinta liso.
///
/// Mismo elemento y misma huella: el mapa de antes, sin tocar un pixel. Un
/// trazo que ha crecido —el que se esta dibujando— no se estampa entero otra
/// vez: se sigue por donde iba ([`Obra::seguir`]) y el mapa dice que trozo
/// cambio (`Cocido::sucio`), para que quien lo sube suba solo eso.
pub fn cocer(e: &Elemento) -> Option<Rc<Cocido>> {
    if !es_de_grafito(e) {
        return None;
    }
    let huella = huella_de(e);
    let clave = (ESPACIO.with(std::cell::Cell::get), e.id);
    let hecho = HORNO.with_borrow_mut(|horno| {
        horno.reloj += 1;
        let reloj = horno.reloj;
        let h = horno.mapas.get_mut(&clave)?;
        h.uso = reloj;
        if h.cocido.huella == huella {
            // Quieto: el mapa de antes. El trazo en curso ya lleva el paso
            // que tendria cocido de cero (`paso_de`), asi que soltarlo no lo
            // recuece ni lo cambia.
            return Some(h.cocido.clone());
        }
        let obra = h.obra.as_mut()?;
        let d = datos_del_trazo(e)?;
        let c = Rc::make_mut(&mut h.cocido);
        if !obra.seguir(&d, c) {
            return None;
        }
        rematar(c, e);
        Some(h.cocido.clone())
    });
    if hecho.is_some() {
        HORNO.with_borrow_mut(|h| h.caber(clave));
        return hecho;
    }
    let (cocido, obra) = match &e.figura {
        Figura::Lapiz { .. } => {
            let (obra, c) = Obra::nueva(&datos_del_trazo(e)?, e)?;
            (Rc::new(c), Some(obra))
        }
        _ => {
            let mut c = cocer_figura(e)?;
            rematar(&mut c, e);
            c.generacion = nueva_generacion();
            (Rc::new(c), None)
        }
    };
    COCCIONES.with(|n| n.set(n.get() + 1));
    HORNO.with_borrow_mut(|horno| {
        horno.reloj += 1;
        let uso = horno.reloj;
        horno.mapas.insert(
            clave,
            Hornada {
                cocido: cocido.clone(),
                obra,
                uso,
            },
        );
        horno.caber(clave);
    });
    Some(cocido)
}

/// Cuanto pesa lo cocido en este hilo: para las pruebas.
pub fn peso_del_horno() -> usize {
    HORNO.with_borrow(|h| h.peso())
}

/// **El grafito que se esta pintando ahora mismo.**
///
/// Quien reparte las ordenes de un elemento de grafito (`por_cada_orden` del
/// editor) no puede meter el mapa en una `Orden`, asi que lo deja aqui
/// mientras llama al que pinta, y el que pinta lo recoge con [`marca`]. Solo
/// vive durante la llamada: fuera de ella no hay marca, y nada viejo se cuela
/// en el trazo siguiente.
pub fn con_marca<R>(c: Rc<Cocido>, f: impl FnOnce() -> R) -> R {
    let antes = MARCA.with_borrow_mut(|m| m.replace(c));
    let r = f();
    MARCA.with_borrow_mut(|m| *m = antes);
    r
}

/// El mapa puesto por [`con_marca`], si se esta dentro de una.
pub fn marca() -> Option<Rc<Cocido>> {
    MARCA.with_borrow(|m| m.clone())
}

/// **La orden que avisa de que aqui va un grafito**: una tinta sin contorno.
///
/// Quien no sabe de grafito no pinta nada con ella —un contorno vacio no es
/// figura—; quien si sabe, al verla, pinta la [`marca`].
pub fn orden_de_aviso(e: &Elemento) -> Orden {
    Orden::Tinta {
        contorno: Vec::new(),
        // El alfa lleva la opacidad del elemento: el mapa va cocido a plena
        // tinta, como en el movil, y se atenua al pintarlo.
        color: ColorRgba {
            a: e.trazo.a * e.opacidad.clamp(0.0, 1.0),
            ..e.trazo
        },
    }
}

// -------------------------------------------------------------------------
// Fuera del editor
// -------------------------------------------------------------------------

/// **Un grafito ya cocido para pintarlo fuera del editor**: en un pin, en la
/// vista previa del chat, en un PNG, un SVG o un PDF.
///
/// Va **aparte de las ordenes**, con el sitio en el que entra entre ellas
/// (`antes_de`), por lo mismo que el grano de las tintas porosas: una `Orden`
/// no puede llevar un mapa de pixeles sin obligar a todos los que pintan
/// ordenes a saber pintarlo. Asi quien no sabe de grafito sigue pintando lo
/// suyo, y quien si sabe lo mete en su sitio.
#[derive(Debug, Clone, PartialEq)]
pub struct GrafitoSuelto {
    /// Se pinta justo antes de `ordenes[antes_de]` (o al final, si es el
    /// largo de la lista): es lo que respeta lo que va encima y debajo.
    pub antes_de: usize,
    /// Compartido: el pin repinta con cada movimiento del raton y la misma
    /// hoja sale en el PNG y en el SVG; copiar megas cada vez no tiene
    /// sentido. `Arc` y no `Rc` porque una hoja puede irse a otro hilo.
    pub mapa: std::sync::Arc<Cocido>,
    /// La del elemento: el mapa va cocido a plena tinta, como en el movil.
    pub opacidad: f32,
}

/// **Los mapas sueltos ya cocidos de quien repinta a menudo** (el pin
/// mientras se anota): por elemento, con la huella con la que se cocieron.
/// Un elemento que no cambio no se vuelve a cocer, y su mapa conserva el id,
/// asi que la GPU tampoco lo vuelve a subir.
#[derive(Debug, Default)]
pub struct MapasSueltos {
    mapas: HashMap<u64, (u64, std::sync::Arc<Cocido>)>,
}

/// Un numero que no ha tenido ningun mapa suelto en todo el programa.
///
/// Es el `id` del mapa y no el del elemento: quien guarda los bitmaps por id
/// (`CacheGrafito`) recibe mapas de dibujos distintos —dos burbujas del chat,
/// dos pines—, y en cada dibujo los ids de los elementos vuelven a empezar.
/// Con el del elemento, dos dibujos con un elemento 1 de grafito se echarian
/// el uno al otro de la cache en cada fotograma.
fn id_de_mapa_suelto() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    // Por arriba, lejos de los ids de elemento y de las generaciones.
    static SIGUIENTE: AtomicU64 = AtomicU64::new(1 << 62);
    SIGUIENTE.fetch_add(1, Ordering::Relaxed)
}

/// **Las ordenes de una escena con el grafito aparte**: lo mismo que
/// `pintado::ordenes_de_escena`, salvo que lo que se pinta de grafito sale
/// cocido en la segunda lista y no liso en la primera.
///
/// `finura_maxima` encoge los mapas hasta esa finura (una vista previa no
/// necesita una casilla por unidad) y `tope` es lo que pueden pesar todos
/// juntos, en bytes: pasado, lo que queda sale liso, como sale en el movil lo
/// que no cabe en `PESO_DE_LOS_LAPICES`. No toca el horno del hilo: estos
/// mapas se quedan con quien los pide, y meterlos alli echaria los del editor.
pub fn ordenes_con_grafito(
    escena: &crate::escena::Escena,
    finura_maxima: f32,
    tope: usize,
) -> (Vec<Orden>, Vec<GrafitoSuelto>) {
    ordenes_con_grafito_guardando(escena, finura_maxima, tope, &mut MapasSueltos::default())
}

/// **El grafito de un elemento, suelto**, para ir en `antes_de`. `None` si
/// no es de grafito o no hay de que: entonces va liso, con sus ordenes. Es
/// lo que usa exportar (`exportar::Hoja::grafitos`), a una casilla por
/// unidad, que es como se ve de cerca.
pub fn suelto(e: &Elemento, antes_de: usize) -> Option<GrafitoSuelto> {
    let mut c = cocer_sin_horno(e)?;
    c.id = id_de_mapa_suelto();
    c.generacion = c.id;
    Some(GrafitoSuelto {
        antes_de,
        mapa: std::sync::Arc::new(c),
        opacidad: e.trazo.a * e.opacidad.clamp(0.0, 1.0),
    })
}

/// [`ordenes_con_grafito`] reusando lo ya cocido en `guardados`, y dejando
/// en `guardados` solo lo de esta escena.
pub fn ordenes_con_grafito_guardando(
    escena: &crate::escena::Escena,
    finura_maxima: f32,
    tope: usize,
    guardados: &mut MapasSueltos,
) -> (Vec<Orden>, Vec<GrafitoSuelto>) {
    let mut ordenes = Vec::new();
    let mut grafitos = Vec::new();
    let mut peso = 0usize;
    let mut vistos = std::collections::HashSet::new();
    for e in escena.visibles() {
        let mapa = if es_de_grafito(e) {
            let huella = huella_de(e);
            vistos.insert(e.id);
            match guardados.mapas.get(&e.id).filter(|(h, _)| *h == huella) {
                Some((_, m)) => Some(m.clone()),
                None => cocer_sin_horno(e).map(|c| {
                    let mut c = encoger(c, finura_maxima);
                    c.id = id_de_mapa_suelto();
                    c.generacion = c.id;
                    let m = std::sync::Arc::new(c);
                    guardados.mapas.insert(e.id, (huella, m.clone()));
                    m
                }),
            }
        } else {
            None
        };
        match mapa.filter(|m| peso + m.peso() <= tope) {
            Some(m) => {
                peso += m.peso();
                grafitos.push(GrafitoSuelto {
                    antes_de: ordenes.len(),
                    mapa: m,
                    // La misma que lleva el aviso en el editor.
                    opacidad: e.trazo.a * e.opacidad.clamp(0.0, 1.0),
                });
            }
            None => ordenes.extend(crate::pintado::ordenes(e)),
        }
        ordenes.extend(crate::pintado::ordenes_medibles(
            e,
            escena.escala.as_ref(),
            ',',
            escena.fondo,
        ));
    }
    guardados.mapas.retain(|id, _| vistos.contains(id));
    (ordenes, grafitos)
}

/// **El trozo del mapa que tiene algo**, como `(x, y, ancho, alto)` en
/// pixeles del mapa. `None` si esta vacio. Es lo que se escribe en un SVG:
/// la caja de un trazo corto en diagonal es casi toda aire.
pub fn lo_pintado(c: &Cocido) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = (c.ancho as usize, c.alto as usize);
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            if c.rgba[(y * w + x) * 4 + 3] != 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x1 > x0).then(|| (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32))
}

/// **Un trozo del mapa como PNG**, comprimido.
///
/// Con su propio compresor y no el PNG «guardado» de la tela del grano: un
/// mapa de grafito es casi todo casillas vacias —el de un rectangulo son
/// megas de ceros alrededor de un marco fino— y sin comprimir un SVG con
/// cuatro figuras pesaba decenas de megas. Basta con lo mas simple que
/// entiende todo lector de PNG: deflate con los codigos fijos y repeticiones
/// del byte anterior, que es justo lo que es una fila de ceros.
pub fn png_del_trozo(c: &Cocido, (tx, ty, tw, th): (u32, u32, u32, u32)) -> Vec<u8> {
    // Las filas, cada una con su filtro «ninguno» delante.
    let mut crudo = Vec::with_capacity((tw as usize * 4 + 1) * th as usize);
    for y in ty..ty + th {
        crudo.push(0);
        let a = ((y * c.ancho + tx) * 4) as usize;
        crudo.extend_from_slice(&c.rgba[a..a + tw as usize * 4]);
    }
    let mut b = Bits::default();
    // Cabecera zlib: deflate, ventana de 32 K, sin diccionario.
    b.bytes.extend_from_slice(&[0x78, 0x01]);
    // Un solo bloque, el ultimo, con los codigos fijos.
    b.poner(1, 1);
    b.poner(1, 2);
    let mut i = 0;
    while i < crudo.len() {
        let mut largo = 0;
        if i > 0 {
            while largo < 258 && i + largo < crudo.len() && crudo[i + largo] == crudo[i - 1] {
                largo += 1;
            }
        }
        if largo >= 3 {
            b.largo(largo);
            // Distancia 1: codigo 0, cinco bits.
            b.huffman(0, 5);
            i += largo;
        } else {
            b.literal(crudo[i] as u16);
            i += 1;
        }
    }
    b.literal(256);
    b.cerrar();
    let (mut s1, mut s2) = (1u32, 0u32);
    for x in &crudo {
        s1 = (s1 + *x as u32) % 65_521;
        s2 = (s2 + s1) % 65_521;
    }
    b.bytes.extend_from_slice(&((s2 << 16) | s1).to_be_bytes());

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut cabecera = Vec::with_capacity(13);
    cabecera.extend_from_slice(&tw.to_be_bytes());
    cabecera.extend_from_slice(&th.to_be_bytes());
    // 8 bits, RGBA, sin entrelazar.
    cabecera.extend_from_slice(&[8, 6, 0, 0, 0]);
    trozo_png(&mut png, b"IHDR", &cabecera);
    trozo_png(&mut png, b"IDAT", &b.bytes);
    trozo_png(&mut png, b"IEND", &[]);
    png
}

fn trozo_png(png: &mut Vec<u8>, tipo: &[u8; 4], datos: &[u8]) {
    png.extend_from_slice(&(datos.len() as u32).to_be_bytes());
    let mut c = 0xffff_ffffu32;
    for b in tipo.iter().chain(datos) {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    png.extend_from_slice(tipo);
    png.extend_from_slice(datos);
    png.extend_from_slice(&(!c).to_be_bytes());
}

/// Los bits de un bloque deflate, del menos al mas significativo.
#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    actual: u32,
    cuantos: u32,
}

impl Bits {
    fn poner(&mut self, valor: u32, n: u32) {
        self.actual |= valor << self.cuantos;
        self.cuantos += n;
        while self.cuantos >= 8 {
            self.bytes.push(self.actual as u8);
            self.actual >>= 8;
            self.cuantos -= 8;
        }
    }

    /// Un codigo de Huffman va del bit mas significativo al menos.
    fn huffman(&mut self, codigo: u32, n: u32) {
        let mut al_reves = 0;
        for k in 0..n {
            al_reves |= ((codigo >> k) & 1) << (n - 1 - k);
        }
        self.poner(al_reves, n);
    }

    /// Un simbolo de literal o largo con los codigos fijos del deflate.
    fn literal(&mut self, s: u16) {
        let s = s as u32;
        match s {
            0..=143 => self.huffman(0x30 + s, 8),
            144..=255 => self.huffman(0x190 + s - 144, 9),
            256..=279 => self.huffman(s - 256, 7),
            _ => self.huffman(0xc0 + s - 280, 8),
        }
    }

    fn largo(&mut self, largo: usize) {
        const BASE: [usize; 29] = [
            3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99,
            115, 131, 163, 195, 227, 258,
        ];
        const EXTRA: [u32; 29] = [
            0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
        ];
        let k = BASE.iter().rposition(|b| *b <= largo).unwrap_or(0);
        self.literal(257 + k as u16);
        self.poner((largo - BASE[k]) as u32, EXTRA[k]);
    }

    fn cerrar(&mut self) {
        if self.cuantos > 0 {
            self.bytes.push(self.actual as u8);
        }
        self.actual = 0;
        self.cuantos = 0;
    }
}

/// **Un mapa a menos finura**: cada cuatro pixeles, uno, con la media de lo
/// que tapaban. Hasta que la finura no pase de `finura_maxima`.
///
/// La media va ponderada por el alfa (el mapa esta sin premultiplicar): sin
/// eso, el color de un pixel casi transparente pesaria como el de uno lleno
/// y los cantos del trazo saldrian con un halo del color de nada.
pub fn encoger(mut c: Cocido, finura_maxima: f32) -> Cocido {
    while c.finura > finura_maxima && c.ancho > 1 && c.alto > 1 {
        let (w, h) = (c.ancho as usize, c.alto as usize);
        let (nw, nh) = (w.div_ceil(2), h.div_ceil(2));
        let mut v = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                let (mut a, mut r, mut g, mut b) = (0u32, 0u32, 0u32, 0u32);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = (2 * x + dx, 2 * y + dy);
                    if sx >= w || sy >= h {
                        continue;
                    }
                    let i = (sy * w + sx) * 4;
                    let pa = c.rgba[i + 3] as u32;
                    a += pa;
                    r += c.rgba[i] as u32 * pa;
                    g += c.rgba[i + 1] as u32 * pa;
                    b += c.rgba[i + 2] as u32 * pa;
                }
                if a == 0 {
                    continue;
                }
                let o = (y * nw + x) * 4;
                v[o] = (r / a) as u8;
                v[o + 1] = (g / a) as u8;
                v[o + 2] = (b / a) as u8;
                // Lo que falta de un canto impar cuenta como vacio.
                v[o + 3] = (a / 4) as u8;
            }
        }
        c.rgba = v;
        c.ancho = nw as u32;
        c.alto = nh as u32;
        c.finura /= 2.0;
        c.sucio = None;
    }
    c
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::tinta::MaterialTinta;

    fn trazo(puntos: &[(f32, f32)]) -> Elemento {
        Elemento {
            id: 42,
            figura: Figura::Lapiz {
                puntos: puntos.iter().map(|p| Punto2::nuevo(p.0, p.1)).collect(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            grosor: 2.0,
            material: MaterialTinta::Cuadritos,
            trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
            ..Default::default()
        }
    }

    /// Dos escenas numeran sus elementos igual (del 1 en adelante: la hoja 1
    /// y la hoja 2 de un PDF, o el lienzo y su hojita) y se pintan en el
    /// mismo hilo. Un trazo de una no puede salir con el mapa del de la otra.
    #[test]
    fn dos_escenas_con_el_mismo_id_no_se_mezclan_el_grafito() {
        let a = trazo(&recta(10.0, 60.0, 10.0));
        let b = trazo(&recta(300.0, 420.0, 250.0));
        assert_eq!(a.id, b.id);
        let solo_a = cocer_sin_horno(&a).unwrap();
        let solo_b = cocer_sin_horno(&b).unwrap();
        for _ in 0..3 {
            let ca = cocer(&a).unwrap();
            let cb = cocer(&b).unwrap();
            assert_eq!(
                (ca.x0, ca.y0, ca.ancho, ca.alto),
                (solo_a.x0, solo_a.y0, solo_a.ancho, solo_a.alto)
            );
            assert_eq!(
                (cb.x0, cb.y0, cb.ancho, cb.alto),
                (solo_b.x0, solo_b.y0, solo_b.ancho, solo_b.alto)
            );
        }
    }

    /// Un garabato de mano de `n` puntos, ida y vuelta a lo ancho de `ancho`
    /// unidades y bajando, estampado punto a punto como mientras se dibuja.
    /// Devuelve el elemento y el mapa del ultimo fotograma.
    fn garabato_creciendo(id: u64, ancho: f32, n: usize) -> (Elemento, Rc<Cocido>) {
        let mut pts: Vec<Punto2> = Vec::new();
        let mut e = trazo(&[(0.0, 0.0), (1.0, 0.0)]);
        e.id = id;
        let mut ultimo = None;
        for k in 0..n {
            let f = k as f32;
            let fase = (f * 6.0) % (2.0 * ancho);
            let x = if fase < ancho {
                fase
            } else {
                2.0 * ancho - fase
            };
            pts.push(Punto2::nuevo(x, f * 0.3 + 20.0 * (f / 5.0).sin()));
            if pts.len() < 2 {
                continue;
            }
            e.figura = Figura::Lapiz {
                puntos: pts.clone(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            };
            e.version += 1;
            ultimo = cocer(&e);
        }
        (e, ultimo.expect("cocido"))
    }

    /// La carga de tinta de un mapa y cuantas casillas se ven llenas.
    fn carga_y_llenas(c: &Cocido) -> (u64, usize) {
        let px = || c.rgba.chunks_exact(4);
        (
            px().map(|p| p[3] as u64).sum(),
            px().filter(|p| p[3] > 40).count(),
        )
    }

    /// **Un trazo largo no cambia al soltarlo** (lo reporto el usuario en el
    /// lector: «cuando suelto el dibujo se achica»). En lo larguisimo el
    /// paso crece con el largo; mientras se dibujaba se dejaba fijo con un
    /// margen del 10 % y al quedarse quieto se recocia con el exacto, mas
    /// separado: medido, un 19 % menos de casillas llenas de golpe al
    /// soltar, el trazo entero mas fino. Ahora el paso va por escalones y el
    /// trazo en curso ya lleva el suyo: quieto no se recuece y sale igual que
    /// cocido de cero.
    #[test]
    fn un_trazo_largo_suelto_se_ve_igual_que_mientras_se_dibujaba() {
        // 1.200 puntos a 6 unidades: 7.000 de largo, pasado el punto en que
        // el paso empieza a crecer con el (gordo * 0,1 * 9.000 = 4.320).
        let (e, en_curso) = garabato_creciendo(9101, 600.0, 1200);
        let d = datos_del_trazo(&e).unwrap();
        assert!(
            d.largo / SELLOS_POR_TRAZO > (d.gordo * ESPACIADO_DEL_LAPIZ) as f64,
            "lo larguisimo"
        );
        let antes = cocciones();
        let quieto = cocer(&e).unwrap();
        assert_eq!(cocciones(), antes, "quieto no se recuece");
        let de_cero = cocer_sin_horno(&e).unwrap();
        let (a, b, c) = (
            carga_y_llenas(&en_curso),
            carga_y_llenas(&quieto),
            carga_y_llenas(&de_cero),
        );
        assert_eq!(a, b);
        let cerca = |x: f64, y: f64| (x - y).abs() <= y * 0.01;
        assert!(
            cerca(a.0 as f64, c.0 as f64) && cerca(a.1 as f64, c.1 as f64),
            "en curso {a:?}, de cero {c:?}"
        );
    }

    /// Y crecer no lo estampa entero en cada punto, tampoco pasado el tope
    /// del mapa: antes, un garabato de mas de unas 1.800 unidades de ancho
    /// (media hoja de PDF con sus margenes) se recocia entero en CADA punto.
    #[test]
    fn un_garabato_ancho_no_se_recuece_entero_en_cada_punto() {
        for ancho in [1900.0f32, 2600.0] {
            let antes = cocciones();
            garabato_creciendo(9200 + ancho as u64, ancho, 800);
            let n = cocciones() - antes;
            // Algunas: al empezar, al pasar a media finura y en cada
            // escalon del paso. Nunca una por punto.
            assert!(n < 40, "ancho {ancho}: {n} cocciones enteras en 800 puntos");
        }
    }

    #[test]
    fn con_un_espacio_por_escena_alternar_dos_escenas_no_recuece_nada() {
        // Dos hojas del lector, cada una con su elemento 1, pintadas una
        // tras otra en cada fotograma.
        let a = trazo(&recta(10.0, 60.0, 10.0));
        let b = trazo(&recta(300.0, 420.0, 250.0));
        assert_eq!(a.id, b.id);
        en_espacio(101, || cocer(&a));
        en_espacio(102, || cocer(&b));
        let antes = cocciones();
        for _ in 0..5 {
            let ca = en_espacio(101, || cocer(&a)).unwrap();
            let cb = en_espacio(102, || cocer(&b)).unwrap();
            assert!(ca.x0 < 60.0 && cb.x0 > 200.0, "cada una lo suyo");
        }
        assert_eq!(cocciones(), antes, "ni una coccion mas al alternar");
        // Caso negativo: las dos en el mismo espacio se echan la una a la
        // otra y se recuecen enteras en cada vuelta (el fallo del lector).
        let antes = cocciones();
        for _ in 0..3 {
            en_espacio(103, || {
                cocer(&a);
                cocer(&b);
            });
        }
        assert!(
            cocciones() >= antes + 5,
            "{} cocciones",
            cocciones() - antes
        );
        // Y al salir vuelve el espacio de siempre.
        assert_eq!(ESPACIO.with(std::cell::Cell::get), 0);
    }

    fn recta(desde: f32, hasta: f32, y: f32) -> Vec<(f32, f32)> {
        let mut v = Vec::new();
        let mut x = desde;
        while x <= hasta {
            v.push((x, y));
            x += 2.0;
        }
        v
    }

    fn alfa_en(c: &Cocido, x: f32, y: f32) -> u8 {
        let px = ((x - c.x0) * c.finura) as usize;
        let py = ((y - c.y0) * c.finura) as usize;
        c.rgba[(py * c.ancho as usize + px) * 4 + 3]
    }

    /// Alfa medio de una banda vertical del mapa, en unidades del dibujo.
    fn carga_media(c: &Cocido, xa: f32, xb: f32) -> f32 {
        let mut suma = 0u64;
        let mut n = 0u64;
        for py in 0..c.alto as usize {
            for x in xa as usize..xb as usize {
                let px = ((x as f32 - c.x0) * c.finura) as usize;
                suma += c.rgba[(py * c.ancho as usize + px) * 4 + 3] as u64;
                n += 1;
            }
        }
        suma as f32 / n.max(1) as f32
    }

    #[test]
    fn el_sello_es_rascado_siempre_el_mismo_y_sin_esquinas() {
        let s = sello();
        assert_eq!(s.len(), LADO_DEL_SELLO * LADO_DEL_SELLO);
        assert_eq!(
            s,
            tejer_sello().as_slice(),
            "el sello tiene que salir igual siempre"
        );
        let llenas = s.iter().filter(|a| **a > 0).count();
        // Rascado: ni vacio ni macizo.
        assert!(llenas > s.len() / 4, "casi vacio: {llenas}");
        assert!(
            s.iter().filter(|a| **a > 200).count() < s.len() / 2,
            "demasiado macizo"
        );
        // Y la caida a los cantos: la esquina tapa menos que el centro.
        let n = LADO_DEL_SELLO;
        let medio: u32 = (20..28)
            .flat_map(|y| (20..28).map(move |x| (x, y)))
            .map(|(x, y)| s[y * n + x] as u32)
            .sum();
        let esquina: u32 = (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .map(|(x, y)| s[y * n + x] as u32)
            .sum();
        assert!(medio > esquina * 3, "medio {medio}, esquina {esquina}");
    }

    #[test]
    fn el_diente_del_papel_es_la_cuenta_del_movil_y_ninguna_casilla_se_niega() {
        // Mismo sitio, misma casilla: siempre igual.
        assert_eq!(diente(10, 20), diente(10, 20));
        // Ninguna casilla coge cero —«un grafito de verdad pinta»— y las
        // duras cogen poco. Y el tono se queda dentro de su margen.
        let mut duras = 0;
        for y in -50..50 {
            for x in -50..50 {
                let (coge, tono) = diente(x, y);
                assert!((0.06..=1.0).contains(&coge), "({x},{y}) coge {coge}");
                assert!(tono.abs() <= TONO_DEL_GRAFITO + 1e-6);
                if coge < 0.2 {
                    duras += 1;
                }
            }
        }
        // Un trece por ciento, mas o menos.
        assert!((900..1700).contains(&duras), "duras {duras} de 10000");
        // Caso concreto, calculado a mano con la aritmetica de 32 bits de
        // Kotlin: la casilla (0, 0) tiene n = 0 y coge lo minimo.
        assert_eq!(diente(0, 0), (0.06, -TONO_DEL_GRAFITO));
    }

    #[test]
    fn la_rejilla_es_fija_y_encaja_en_multiplos_de_cuatro() {
        let a = cocer_sin_horno(&trazo(&recta(13.0, 91.0, 37.0))).unwrap();
        assert_eq!(a.finura, 1.0);
        assert_eq!(a.x0 % 4.0, 0.0);
        assert_eq!(a.y0 % 4.0, 0.0);
        // Otro trazo en otro sitio cae en la misma rejilla.
        let b = cocer_sin_horno(&trazo(&recta(15.0, 60.0, 39.5))).unwrap();
        assert_eq!((b.x0 - a.x0) % 4.0, 0.0);
        assert_eq!((b.y0 - a.y0) % 4.0, 0.0);
    }

    #[test]
    fn empieza_y_acaba_delgadito_y_el_cuerpo_carga_mas() {
        let c = cocer_sin_horno(&trazo(&recta(0.0, 200.0, 50.0))).unwrap();
        let cuerpo = carga_media(&c, 90.0, 110.0);
        let punta = carga_media(&c, 0.0, 4.0);
        assert!(cuerpo > punta * 1.5, "cuerpo {cuerpo}, punta {punta}");
    }

    #[test]
    fn con_la_presion_constante_del_panel_las_puntas_no_se_afinan() {
        let variable = trazo(&recta(0.0, 200.0, 50.0));
        let mut constante = variable.clone();
        if let Figura::Lapiz { opciones, .. } = &mut constante.figura {
            opciones.as_mut().unwrap().variabilidad = crate::tinta::Variabilidad::Constante;
        }
        let v = cocer_sin_horno(&variable).unwrap();
        let c = cocer_sin_horno(&constante).unwrap();
        let (pv, pc) = (carga_media(&v, 0.0, 4.0), carga_media(&c, 0.0, 4.0));
        assert!(pc > pv * 1.3, "punta variable {pv}, constante {pc}");
        // Y el mando cuenta como cambio del trazo: no se reusa el mapa viejo.
        assert_ne!(huella_de(&variable), huella_de(&constante));
        // Caso negativo: el cuerpo carga lo mismo, que el grueso es el mismo.
        let (cv, cc) = (carga_media(&v, 90.0, 110.0), carga_media(&c, 90.0, 110.0));
        assert!(
            (cv - cc).abs() < cv * 0.35,
            "cuerpo variable {cv}, constante {cc}"
        );
    }

    #[test]
    fn repasar_oscurece_porque_es_mas_grafito_encima() {
        let una = cocer_sin_horno(&trazo(&recta(0.0, 200.0, 50.0))).unwrap();
        let mut ida_y_vuelta = recta(0.0, 200.0, 50.0);
        ida_y_vuelta.extend(recta(0.0, 200.0, 50.0).into_iter().rev());
        let dos = cocer_sin_horno(&trazo(&ida_y_vuelta)).unwrap();
        let (a, b) = (
            carga_media(&una, 80.0, 120.0),
            carga_media(&dos, 80.0, 120.0),
        );
        assert!(b > a * 1.2, "una pasada {a}, dos {b}");
    }

    #[test]
    fn apretar_carga_mas_que_rozar() {
        let puntos = recta(0.0, 200.0, 50.0);
        let con = |p: f32| {
            let mut e = trazo(&puntos);
            if let Figura::Lapiz { presiones, .. } = &mut e.figura {
                *presiones = vec![p; puntos.len()];
            }
            cocer_sin_horno(&e).unwrap()
        };
        let (flojo, fuerte) = (
            carga_media(&con(0.1), 80.0, 120.0),
            carga_media(&con(1.0), 80.0, 120.0),
        );
        assert!(fuerte > flojo * 1.3, "rozando {flojo}, apretando {fuerte}");
    }

    #[test]
    fn el_papel_asoma_y_el_tono_no_es_plano() {
        // Aun repasado, el grafito no es una tinta plana: hay casillas de
        // distinta carga y de distinto tono dentro del cuerpo.
        let c = cocer_sin_horno(&trazo(&recta(0.0, 200.0, 50.0))).unwrap();
        let y = 50.0;
        let fila: Vec<[u8; 4]> = (60..140)
            .map(|x| {
                let i = (((y - c.y0) as usize) * c.ancho as usize + (x as f32 - c.x0) as usize) * 4;
                [c.rgba[i], c.rgba[i + 1], c.rgba[i + 2], c.rgba[i + 3]]
            })
            .collect();
        let cargas: std::collections::HashSet<u8> = fila.iter().map(|p| p[3]).collect();
        let tonos: std::collections::HashSet<u8> =
            fila.iter().filter(|p| p[3] > 0).map(|p| p[0]).collect();
        assert!(cargas.len() > 5, "la carga es plana: {cargas:?}");
        assert!(tonos.len() > 3, "el tono es plano: {tonos:?}");
        assert!(alfa_en(&c, 100.0, 50.0) > 0);
    }

    #[test]
    fn lo_que_no_es_grafito_o_no_tiene_de_que_no_se_cuece() {
        let mut liso = trazo(&recta(0.0, 50.0, 0.0));
        liso.material = MaterialTinta::Lisa;
        assert!(cocer_sin_horno(&liso).is_none());
        // Un punto suelto: sin tramo no hay por donde estampar; se pinta liso.
        assert!(cocer_sin_horno(&trazo(&[(5.0, 5.0)])).is_none());
        // Borrado, tampoco.
        let mut borrado = trazo(&recta(0.0, 50.0, 0.0));
        borrado.borrado = true;
        assert!(cocer_sin_horno(&borrado).is_none());
        // Un texto de cuadritos no es figura de grafito.
        let texto = Elemento {
            figura: Figura::Texto {
                texto: "hola".into(),
                tam: 20.0,
                familia: String::new(),
            },
            material: MaterialTinta::Cuadritos,
            ..Default::default()
        };
        assert!(!es_de_grafito(&texto));
    }

    #[test]
    fn un_trazo_enorme_se_cuece_a_menos_finura_y_no_pide_un_mapa_imposible() {
        let c = cocer_sin_horno(&trazo(&[(0.0, 0.0), (9000.0, 10.0)])).unwrap();
        assert!(c.finura < 1.0);
        assert!(c.ancho as f32 <= LADO_MAXIMO_DEL_LAPIZ + 1.0, "{}", c.ancho);
        assert_eq!(c.rgba.len(), (c.ancho * c.alto * 4) as usize);
    }

    #[test]
    fn el_horno_devuelve_lo_mismo_hasta_que_el_trazo_cambia() {
        let mut e = trazo(&recta(0.0, 100.0, 10.0));
        e.id = 777;
        let a = cocer(&e).unwrap();
        let b = cocer(&e).unwrap();
        assert!(Rc::ptr_eq(&a, &b), "sin cambios no se vuelve a cocer");
        e.tocar();
        let c = cocer(&e).unwrap();
        assert!(!Rc::ptr_eq(&a, &c), "con otra version, si");
        assert!(peso_del_horno() >= c.peso());
    }

    /// El pixel del dibujo `(x, y)` (casilla entera) en un mapa, o nada si cae
    /// fuera.
    fn pixel(c: &Cocido, x: i32, y: i32) -> [u8; 4] {
        let px = x - c.x0 as i32;
        let py = y - c.y0 as i32;
        if px < 0 || py < 0 || px >= c.ancho as i32 || py >= c.alto as i32 {
            return [0; 4];
        }
        let i = (py as usize * c.ancho as usize + px as usize) * 4;
        [c.rgba[i], c.rgba[i + 1], c.rgba[i + 2], c.rgba[i + 3]]
    }

    /// Dibuja `puntos` de tres en tres por el horno, como la tinta viva, y
    /// devuelve lo ultimo cocido y los trozos sucios que fue diciendo.
    fn dibujar_poco_a_poco(
        id: u64,
        puntos: &[(f32, f32)],
    ) -> (Rc<Cocido>, Vec<Option<(u64, Rect)>>) {
        let mut e = trazo(&puntos[..2]);
        e.id = id;
        let mut sucios = Vec::new();
        cocer(&e).unwrap();
        let mut n = 2;
        while n < puntos.len() {
            n = (n + 3).min(puntos.len());
            if let Figura::Lapiz { puntos: p, .. } = &mut e.figura {
                *p = puntos[..n]
                    .iter()
                    .map(|q| Punto2::nuevo(q.0, q.1))
                    .collect();
            }
            e.tocar();
            let c = cocer(&e).unwrap();
            sucios.push(c.sucio);
        }
        // Y quieto: es cuando el trazo largo se cuece exacto.
        (cocer(&e).unwrap(), sucios)
    }

    fn igual_que_entero(id: u64, puntos: &[(f32, f32)]) {
        let (poco_a_poco, sucios) = dibujar_poco_a_poco(id, puntos);
        let mut e = trazo(puntos);
        e.id = id;
        let entero = cocer_sin_horno(&e).unwrap();
        let (x0, y0) = (entero.x0 as i32, entero.y0 as i32);
        let mut distintos = 0;
        for y in y0 - 200..y0 + entero.alto as i32 + 200 {
            for x in x0 - 200..x0 + entero.ancho as i32 + 200 {
                if pixel(&poco_a_poco, x, y) != pixel(&entero, x, y) {
                    distintos += 1;
                }
            }
        }
        assert_eq!(
            distintos, 0,
            "estampar poco a poco no da lo mismo que de una vez"
        );
        // Y mientras crecia, lo normal era subir solo un trozo.
        let trozos = sucios.iter().filter(|s| s.is_some()).count();
        assert!(
            trozos * 2 > sucios.len(),
            "casi siempre se subio el mapa entero: {trozos} de {}",
            sucios.len()
        );
    }

    #[test]
    fn estampar_el_trazo_en_curso_poco_a_poco_da_lo_mismo_que_de_una_vez() {
        let onda: Vec<(f32, f32)> = (0..300)
            .map(|i| {
                (
                    10.0 + i as f32 * 2.5,
                    100.0 + (i as f32 * 0.07).sin() * 60.0,
                )
            })
            .collect();
        igual_que_entero(501, &onda);
    }

    #[test]
    fn un_trazo_larguisimo_poco_a_poco_acaba_igual_que_de_una_vez_al_pararse() {
        // Pasado el tope de sellos, el paso crece con el largo: mientras se
        // dibuja se aproxima, y al pararse se cuece exacto.
        let zigzag: Vec<(f32, f32)> = (0..900)
            .map(|i| {
                (
                    10.0 + (i % 60) as f32 * 12.0,
                    10.0 + (i / 60) as f32 * 30.0 + (i as f32 * 0.3).sin() * 4.0,
                )
            })
            .collect();
        igual_que_entero(502, &zigzag);
    }

    #[test]
    fn otro_trazo_con_el_mismo_id_no_sigue_al_anterior() {
        // Caso negativo: si los puntos de antes no son el principio de los de
        // ahora (se deshizo, se movio), no se sigue: se cuece entero.
        let mut e = trazo(&recta(0.0, 100.0, 10.0));
        e.id = 503;
        let a = cocer(&e).unwrap();
        e = trazo(&recta(0.0, 100.0, 60.0));
        e.id = 503;
        e.version = 9;
        let b = cocer(&e).unwrap();
        assert!(b.sucio.is_none(), "siguio a un trazo que no era el suyo");
        assert_ne!(a.generacion, b.generacion);
        assert_eq!(*b, {
            let mut s = cocer_sin_horno(&e).unwrap();
            s.generacion = b.generacion;
            s
        });
    }

    #[test]
    fn la_marca_solo_vive_durante_la_llamada() {
        let c = Rc::new(cocer_sin_horno(&trazo(&recta(0.0, 30.0, 0.0))).unwrap());
        assert!(marca().is_none());
        let dentro = con_marca(c.clone(), marca);
        assert!(dentro.is_some_and(|m| Rc::ptr_eq(&m, &c)));
        assert!(marca().is_none(), "la marca se quedo puesta despues");
    }

    #[test]
    fn un_rectangulo_de_grafito_lleva_raya_y_fondo_flojo() {
        let mut r = Elemento {
            id: 9,
            figura: Figura::Rectangulo,
            x: 10.0,
            y: 10.0,
            ancho: 120.0,
            alto: 80.0,
            grosor: 2.0,
            material: MaterialTinta::Cuadritos,
            relleno: Some(ColorRgba::opaco(0.9, 0.2, 0.2)),
            estilo_relleno: EstiloRelleno::Solido,
            trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
            ..Default::default()
        };
        let c = cocer_sin_horno(&r).unwrap();
        // Dentro: el fondo, rojo y flojo, desigual por el diente.
        let dentro = alfa_en(&c, 70.0, 50.0);
        assert!(dentro > 0 && dentro < 250, "fondo {dentro}");
        let i = (((50.0 - c.y0) as usize) * c.ancho as usize + (70.0 - c.x0) as usize) * 4;
        assert!(c.rgba[i] > c.rgba[i + 2], "el fondo no es rojo");
        // Sin fondo, dentro no hay nada.
        r.relleno = None;
        let sin = cocer_sin_horno(&r).unwrap();
        assert_eq!(alfa_en(&sin, 70.0, 50.0), 0);
        // Y la raya si esta: por el borde de arriba hay grafito.
        let borde = (15..125)
            .map(|x| alfa_en(&sin, x as f32, 10.0) as u32)
            .sum::<u32>();
        assert!(borde > 0);
    }

    fn rectangulo_de_grafito(ancho: f32, alto: f32, relleno: bool) -> Elemento {
        Elemento {
            id: 11,
            figura: Figura::Rectangulo,
            x: 10.0,
            y: 10.0,
            ancho,
            alto,
            grosor: 2.0,
            material: MaterialTinta::Cuadritos,
            relleno: relleno.then(|| ColorRgba::opaco(0.9, 0.2, 0.2)),
            estilo_relleno: EstiloRelleno::Solido,
            trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
            ..Default::default()
        }
    }

    #[test]
    fn pasar_el_diente_solo_por_las_tejas_tocadas_da_lo_mismo_que_por_el_mapa_entero() {
        // Lo que no se toco esta a cero en el lienzo, y el diente de un pixel
        // a cero es cero: saltarselo no puede cambiar nada.
        let mut l = Lienzo::nuevo(300, 170);
        let todo = Rect {
            x: 0,
            y: 0,
            ancho: l.ancho,
            alto: l.alto,
        };
        // Un lienzo sin tocar no tiene tejas por las que pasar.
        assert!(l.tocado().is_empty());
        l.estampar(sello(), (40.0, 40.0), 6.0, 30.0, [20.0, 20.0, 20.0], 0.5);
        l.rellenar(
            &[(100.0, 60.0), (160.0, 60.0), (160.0, 120.0), (100.0, 120.0)],
            [200.0, 40.0, 40.0],
            0.55,
        );
        let mut entero = vec![0u8; l.px.len()];
        diente_en(&l, todo, (3, 5), 1.0, &mut entero);
        let mut por_tejas = vec![0u8; l.px.len()];
        for r in l.tocado() {
            diente_en(&l, r, (3, 5), 1.0, &mut por_tejas);
        }
        assert!(entero.iter().any(|b| *b > 0), "no se pinto nada");
        assert_eq!(entero, por_tejas);
        // Caso negativo: las tejas lejos de lo pintado siguen sin tocar.
        assert!(
            l.tocado().iter().all(|r| r.x < 200 && r.y < 160),
            "{:?}",
            l.tocado()
        );
    }

    #[test]
    fn un_rectangulo_grande_sin_fondo_solo_paga_el_diente_de_su_marco() {
        // Es lo que deja verlo de grafito mientras se arrastra: el mapa es de
        // 1200 x 800, pero solo se pinto una raya alrededor.
        let e = rectangulo_de_grafito(1200.0, 800.0, false);
        let c = cocer_sin_horno(&e).unwrap();
        let mut l = Lienzo::nuevo(c.ancho as usize, c.alto as usize);
        let origen = (c.x0, c.y0);
        let mut lisa = e.clone();
        lisa.material = MaterialTinta::Lisa;
        let caminos: Vec<Vec<Punto2>> = crate::pintado::ordenes(&lisa)
            .into_iter()
            .filter_map(|o| match o {
                Orden::Polilinea { puntos, .. } => Some(puntos),
                _ => None,
            })
            .collect();
        let mut azar = AzarJava::nuevo(e.id);
        estampar_caminos(
            &mut l,
            &mut azar,
            &caminos,
            2.4,
            1.0,
            origen,
            [0.0; 3],
            true,
            Dureza::Hb,
        );
        let tocado: usize = l.tocado().iter().map(|r| r.ancho * r.alto).sum();
        let total = l.ancho * l.alto;
        assert!(
            tocado * 5 < total,
            "el diente pasaria por {tocado} de {total}"
        );
        // Caso negativo: el centro del mapa cocido esta vacio.
        let (w, h) = (c.ancho as usize, c.alto as usize);
        assert_eq!(c.rgba[((h / 2) * w + w / 2) * 4 + 3], 0);
    }

    #[test]
    fn el_sello_de_molde_carga_lo_mismo_que_el_sello_exacto_y_cae_en_su_sitio() {
        // Doscientos sellos sueltos por una raya, a mano y con molde.
        let mut exacto = Lienzo::nuevo(400, 40);
        let mut molde = Lienzo::nuevo(400, 40);
        let mut azar = AzarJava::nuevo(5);
        for k in 0..200 {
            let centro = (10.0 + k as f64 * 1.83, 20.0 + (k % 5) as f64 * 0.37);
            let grados = azar.flotante() * 360.0;
            exacto.estampar(sello(), centro, 2.2, grados, [0.0; 3], 0.4);
            molde.estampar_con_molde(centro, 2.2, grados, [0.0; 3], 0.4);
        }
        let carga = |l: &Lienzo| l.px.chunks_exact(4).map(|p| p[3] as u64).sum::<u64>() as f64;
        let (a, b) = (carga(&exacto), carga(&molde));
        assert!(a > 0.0);
        assert!((a - b).abs() < a * 0.08, "exacto {a}, molde {b}");
        // Caso negativo: nada fuera de la raya (el molde no se corre de sitio).
        let fuera = |l: &Lienzo| {
            (0..400)
                .map(|x| l.px[(5 * 400 + x) * 4 + 3] as u32)
                .sum::<u32>()
        };
        assert_eq!(fuera(&molde), 0);
        assert_eq!(fuera(&exacto), 0);
    }

    #[test]
    fn el_fondo_tendido_tapa_entero_por_dentro_y_a_medias_en_el_canto() {
        let mut l = Lienzo::nuevo(40, 10);
        // De 5.5 a 30.25 en toda la altura.
        l.rellenar(
            &[(5.5, 0.0), (30.25, 0.0), (30.25, 10.0), (5.5, 10.0)],
            [255.0, 0.0, 0.0],
            1.0,
        );
        let a = |x: usize| l.px[(5 * 40 + x) * 4 + 3];
        assert_eq!(a(4), 0);
        assert!((120..=135).contains(&a(5)), "canto izquierdo {}", a(5));
        assert_eq!(a(6), 255);
        assert_eq!(a(29), 255);
        assert!((55..=70).contains(&a(30)), "canto derecho {}", a(30));
        // Caso negativo: fuera, nada.
        assert_eq!(a(31), 0);
        assert_eq!(a(39), 0);
    }

    #[test]
    fn el_relleno_del_bote_de_grafito_lleva_fondo_y_no_raya_y_respeta_su_agujero() {
        let anillo = |x0: f32, y0: f32, x1: f32, y1: f32| {
            vec![
                Punto2::nuevo(x0, y0),
                Punto2::nuevo(x1, y0),
                Punto2::nuevo(x1, y1),
                Punto2::nuevo(x0, y1),
            ]
        };
        let region = Elemento {
            id: 21,
            figura: Figura::Region {
                contorno: anillo(0.0, 0.0, 200.0, 120.0),
                huecos: vec![anillo(80.0, 40.0, 120.0, 80.0)],
            },
            x: 0.0,
            y: 0.0,
            ancho: 200.0,
            alto: 120.0,
            grosor: 4.0,
            material: MaterialTinta::Cuadritos,
            relleno: Some(ColorRgba::opaco(0.2, 0.3, 0.9)),
            // Rayado a proposito: en el PC el relleno del bote no tiene rayas
            // y aun asi tiene que salir.
            estilo_relleno: EstiloRelleno::Rayado,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            ..Default::default()
        };
        assert!(es_de_grafito(&region));
        let c = cocer_sin_horno(&region).expect("se cuece");
        // Dentro, el fondo con el diente: carga, pero floja.
        let carga = (20..60)
            .map(|x| alfa_en(&c, x as f32, 20.0) as u32)
            .sum::<u32>()
            / 40;
        assert!((40..250).contains(&carga), "fondo {carga}");
        // Y azul, que es el fondo y no la tinta negra del trazo.
        let i = (((20.0 - c.y0) as usize) * c.ancho as usize + (30.0 - c.x0) as usize) * 4;
        assert!(c.rgba[i + 2] > c.rgba[i], "el fondo no es el del bote");
        // En el agujero, nada: ni fondo ni un borde repasado.
        assert_eq!(alfa_en(&c, 100.0, 60.0), 0);
        // Fuera del contorno, nada: sin raya no hay sellos que se salgan.
        let fuera = (10..190)
            .map(|x| alfa_en(&c, x as f32, -2.5) as u32)
            .sum::<u32>();
        assert_eq!(fuera, 0, "el borde se repaso a sellos");
    }

    #[test]
    fn fuera_del_editor_el_grafito_sale_aparte_en_su_sitio_y_lo_liso_sigue_en_las_ordenes() {
        let mut escena = crate::escena::Escena::nueva();
        let mut liso = rectangulo_de_grafito(50.0, 40.0, false);
        liso.material = MaterialTinta::Lisa;
        escena.anadir(liso.clone());
        escena.anadir(trazo(&recta(0.0, 120.0, 50.0)));
        escena.anadir(liso);
        let (ordenes, grafitos) = ordenes_con_grafito(&escena, 1.0, usize::MAX);
        assert_eq!(grafitos.len(), 1);
        let g = &grafitos[0];
        // Entre las dos cajas lisas: todas las de la primera antes, las de la
        // segunda despues.
        assert!(
            g.antes_de > 0 && g.antes_de < ordenes.len(),
            "{} de {}",
            g.antes_de,
            ordenes.len()
        );
        assert_eq!(g.antes_de * 2, ordenes.len());
        // Caso negativo: el trazo de grafito no se cuela liso en las ordenes.
        assert!(
            !ordenes.iter().any(|o| matches!(o, Orden::Tinta { .. })),
            "el grafito tambien salio liso"
        );
        // Y dos mapas nunca comparten id, aunque sean del mismo elemento.
        let (_, otra_vez) = ordenes_con_grafito(&escena, 1.0, usize::MAX);
        assert_ne!(g.mapa.id, otra_vez[0].mapa.id);
    }

    #[test]
    fn guardando_lo_cocido_un_grafito_quieto_no_se_vuelve_a_cocer_y_uno_movido_si() {
        let mut escena = crate::escena::Escena::nueva();
        let id = escena.anadir(trazo(&recta(0.0, 120.0, 50.0)));
        let mut guardados = MapasSueltos::default();
        let (_, a) = ordenes_con_grafito_guardando(&escena, 1.0, usize::MAX, &mut guardados);
        let (_, b) = ordenes_con_grafito_guardando(&escena, 1.0, usize::MAX, &mut guardados);
        // El mismo mapa, no uno igual: ni se coce ni la GPU lo resube.
        assert!(std::sync::Arc::ptr_eq(&a[0].mapa, &b[0].mapa));
        // Caso negativo: movido, otro mapa.
        if let Some(e) = escena.buscar_mut(id) {
            e.x += 10.0;
            e.version += 1;
        }
        let (_, c) = ordenes_con_grafito_guardando(&escena, 1.0, usize::MAX, &mut guardados);
        assert!(!std::sync::Arc::ptr_eq(&a[0].mapa, &c[0].mapa));
        // Y lo que ya no esta en la escena se suelta.
        escena.elementos.clear();
        ordenes_con_grafito_guardando(&escena, 1.0, usize::MAX, &mut guardados);
        assert!(guardados.mapas.is_empty());
    }

    #[test]
    fn pasado_el_tope_de_peso_el_grafito_sale_liso_como_en_el_movil() {
        let mut escena = crate::escena::Escena::nueva();
        escena.anadir(trazo(&recta(0.0, 120.0, 50.0)));
        let (ordenes, grafitos) = ordenes_con_grafito(&escena, 1.0, 16);
        assert!(grafitos.is_empty());
        assert!(ordenes.iter().any(|o| matches!(o, Orden::Tinta { .. })));
    }

    #[test]
    fn encoger_un_mapa_cubre_la_misma_caja_y_guarda_el_color_sin_halo() {
        let c = cocer_sin_horno(&trazo(&recta(0.0, 200.0, 50.0))).unwrap();
        let (x0, y0, w, h) = c.caja();
        let e = encoger(c.clone(), 0.25);
        assert_eq!(e.finura, 0.25);
        let (ex0, ey0, ew, eh) = e.caja();
        assert_eq!((ex0, ey0), (x0, y0));
        // Por el canto impar puede quedar hasta un pixel de mas.
        assert!(ew >= w && ew <= w + 4.0 / 0.25, "{ew} de {w}");
        assert!(eh >= h && eh <= h + 4.0 / 0.25, "{eh} de {h}");
        // El trazo es casi negro: encogido sigue siendo casi negro, no gris
        // por mezclarse con el vacio.
        let mas_cargado = e
            .rgba
            .chunks_exact(4)
            .max_by_key(|p| p[3])
            .expect("hay pixeles");
        assert!(mas_cargado[3] > 0);
        assert!(mas_cargado[0] < 80, "halo: {mas_cargado:?}");
        // Caso negativo: a la finura que ya tiene, no se toca.
        assert_eq!(encoger(c.clone(), 1.0), c);
    }
}
