//! **El plano listo para la tarjeta grafica**: todo se trocea **una vez** al
//! abrir y se sube a la GPU; mover y acercar solo cambia una matriz.
//!
//! Medido con los planos del usuario (8-oct-2026): en uno de obra de 7 MB
//! los sombreados eran 12 millones de vertices y los textos 7,6; las rayas,
//! dos. Por eso tres clases de cosas, cada una como menos pesa:
//!
//! - **Rayas y rellenos** (`vertices`, `lineas`, `triangulos`): lo de
//!   siempre.
//! - **Sombreados con patron** (`tramas`, `familias`): solo se guarda el
//!   contorno relleno; las rayas del patron las calcula la tarjeta pixel a
//!   pixel (la idea de OpenCADStudio). Pesan lo que su contorno.
//! - **Letras** (`malla_letras`, `glifos`, `letras`): cada letra distinta
//!   se rellena una vez; cada vez que sale en el plano es una instancia de
//!   32 bytes (donde, con que matriz, de que color).
//!
//! Para que vaya fluido se dibuja menos:
//!
//! - **Por tamano.** Cada cosa cae en un *nivel* (potencias de dos de su
//!   tamano). Lo que en pantalla mediria menos de un pixel no se dibuja.
//! - **Por sitio.** El plano se parte en una rejilla; de cada nivel solo se
//!   dibujan las celdas que caen en la ventana.
//!
//! Coordenadas en `f32` **respecto a un origen** en `f64` (el centro): un
//! plano en UTM se veria a saltos con `f32` a secas.

/// Un vertice de raya o relleno. Color RGBA; alfa 0 es «el color 7» (blanco
/// en fondo oscuro, negro en claro): lo decide el sombreador.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Vertice {
    pub x: f32,
    pub y: f32,
    pub color: u32,
}

/// Un vertice del contorno de un sombreado con patron, con su sombreado.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct VerticeTrama {
    pub x: f32,
    pub y: f32,
    pub color: u32,
    pub trama: u32,
}

/// Un sombreado con patron: como pasar del plano a su sistema (donde estan
/// definidas sus rayas) y que familias de rayas lleva.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Trama {
    /// Un punto del sombreado, respecto al origen del modelo.
    pub centro: [f32; 2],
    /// Del plano (respecto a `centro`) al sistema del patron: matriz 2x2.
    pub inversa: [f32; 4],
    /// Unidades del plano por unidad del patron.
    pub escala: f32,
    pub desde: u32,
    pub cuantas: u32,
}

/// Una familia de rayas paralelas de un patron, en su sistema y con el
/// origen ya traido cerca de `Trama::centro` (para no perder precision).
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Familia {
    pub base: [f32; 2],
    pub dir: [f32; 2],
    /// Distancia entre dos rayas (perpendicular).
    pub paso: f32,
    /// Cuanto se corre el trazo de una raya a la siguiente.
    pub corrimiento: f32,
    /// Largo del ciclo de trazos (0: raya continua).
    pub largo: f32,
    pub n: u32,
    /// Trazos (>0 raya, <0 hueco, 0 punto), hasta 8.
    pub trazos: [f32; 8],
}

/// Una letra puesta en el plano: posicion, matriz 2x2 (de em al plano),
/// color y que letra.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Letra {
    pub pos: [f32; 2],
    pub m: [f32; 4],
    pub color: u32,
    pub glifo: u32,
}

/// Un circulo o un arco continuo, que la tarjeta pinta exacto a cualquier
/// zoom (un cuadrado por arco; el sombreador mira la distancia al centro).
/// La idea es de OpenCADStudio (su arquitectura, no su codigo).
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Arco {
    pub centro: [f32; 2],
    pub radio: f32,
    /// Donde empieza (radianes, contra reloj) y cuanto barre (2π: entero).
    pub inicio: f32,
    pub barrido: f32,
    pub color: u32,
    pub relleno: [u32; 2],
}

/// En `glifos`: el bit alto del numero de vertices dice que la letra es de
/// rayas (una fuente SHX: pares de puntos, `LINELIST`) y no de triangulos.
pub const GLIFO_DE_RAYAS: u32 = 0x8000_0000;

/// El indice que corta una tira de lineas (`strip cut` de D3D11).
pub const CORTE: u32 = u32::MAX;

/// Un trozo seguido que se dibuja de una vez.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tramo {
    pub desde: u32,
    pub cuantos: u32,
    /// La caja de lo que dibuja: `[x0, y0, x1, y1]`.
    pub caja: [f32; 4],
    /// El tamano de lo que lleva: si en pantalla queda por debajo de un
    /// pixel, el tramo no se dibuja.
    pub tamano: f32,
    /// En las letras: cuantos vertices tiene la mas grande del tramo (se
    /// dibujan todas con ese numero; las demas sobran y se descartan).
    pub clase: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Modelo {
    pub origen: [f64; 2],
    pub caja: [f32; 4],
    pub vertices: Vec<Vertice>,
    /// Tiras de lineas, separadas por [`CORTE`].
    pub lineas: Vec<u32>,
    pub triangulos: Vec<u32>,
    pub tramos_lineas: Vec<Tramo>,
    pub tramos_triangulos: Vec<Tramo>,
    pub vertices_trama: Vec<VerticeTrama>,
    pub triangulos_trama: Vec<u32>,
    pub tramos_trama: Vec<Tramo>,
    pub tramas: Vec<Trama>,
    pub familias: Vec<Familia>,
    /// Los triangulos de todas las letras distintas, en em.
    pub malla_letras: Vec<[f32; 2]>,
    /// De cada letra: desde donde y cuantos vertices de `malla_letras`.
    pub glifos: Vec<[u32; 2]>,
    pub letras: Vec<Letra>,
    pub tramos_letras: Vec<Tramo>,
    pub arcos: Vec<Arco>,
    pub tramos_arcos: Vec<Tramo>,
    /// Las unidades del plano (`$INSUNITS`: 4 mm, 5 cm, 6 m…; 0 sin decir).
    pub unidades: u32,
    /// Lo que no se pudo dibujar, por tipo (para decirlo, no para fallar).
    pub sin_dibujar: Vec<(String, u32)>,
}

impl Modelo {
    /// **La caja de lo que importa** para encuadrar: sin los pocos puntos
    /// sueltos muy lejos (una raya con una cota absurda, un objeto olvidado a
    /// kilometros) que dejan el plano diminuto, como si no abriera (10-oct-
    /// 2026, el usuario: «hay un dwg que no abre»). Del 0,5 % al 99,5 % de
    /// los vertices en cada eje, con un margen; igual que el visor 3D.
    pub fn caja_util(&self) -> [f32; 4] {
        let n = self.vertices.len();
        if n < 200 {
            return self.caja;
        }
        let paso = (n / 40_000).max(1);
        let mut out = self.caja;
        for k in 0..2 {
            let mut v: Vec<f32> = self
                .vertices
                .iter()
                .step_by(paso)
                .map(|p| if k == 0 { p.x } else { p.y })
                .filter(|x| x.is_finite())
                .collect();
            if v.len() < 100 {
                continue;
            }
            v.sort_by(f32::total_cmp);
            let (a, b) = (v[v.len() / 200], v[v.len() - 1 - v.len() / 200]);
            let margen = (b - a) * 0.05;
            out[k] = (a - margen).max(self.caja[k]);
            out[k + 2] = (b + margen).min(self.caja[k + 2]);
        }
        out
    }

    pub fn vacio(&self) -> bool {
        self.lineas.is_empty()
            && self.triangulos.is_empty()
            && self.triangulos_trama.is_empty()
            && self.letras.is_empty()
            && self.arcos.is_empty()
    }

    /// Lo que ocupara en la tarjeta, en bytes (aproximado).
    pub fn bytes_gpu(&self) -> usize {
        self.vertices.len() * 12
            + (self.lineas.len() + self.triangulos.len() + self.triangulos_trama.len()) * 4
            + self.vertices_trama.len() * 16
            + self.tramas.len() * 36
            + self.familias.len() * 64
            + self.malla_letras.len() * 8
            + self.letras.len() * 32
            + self.arcos.len() * 32
    }
}

// ------------------------------------------------------------- construir

/// Una pieza antes de ordenar: sus indices (o instancias) en el monton.
#[derive(Clone, Copy)]
struct Pieza {
    desde: u32,
    cuantos: u32,
    caja: [f64; 4],
    nivel: i32,
    clase: u32,
}

/// Una familia tal como viene del plano (en el sistema del sombreado).
#[derive(Debug, Clone)]
pub struct FamiliaPlano {
    pub angulo: f64,
    pub base: [f64; 2],
    pub desplazamiento: [f64; 2],
    pub trazos: Vec<f64>,
}

/// Una trama a medias: su centro (aun en f64), su caja, su angulo y sus
/// tramos de puntos y triangulos.
type TramaPendiente = ([f64; 2], [f64; 4], f64, u32, u32);

/// Junta lo dibujado (en coordenadas del plano, `f64`) y al final lo ordena
/// en un [`Modelo`].
#[derive(Default)]
pub struct Constructor {
    puntos: Vec<([f64; 2], u32)>,
    lineas: Vec<u32>,
    triangulos: Vec<u32>,
    piezas_l: Vec<Pieza>,
    piezas_t: Vec<Pieza>,
    puntos_trama: Vec<([f64; 2], u32, u32)>,
    triangulos_trama: Vec<u32>,
    piezas_tr: Vec<Pieza>,
    /// Tramas con su centro aun en f64.
    tramas: Vec<TramaPendiente>,
    familias: Vec<Familia>,
    malla_letras: Vec<[f32; 2]>,
    glifos: Vec<[u32; 2]>,
    glifo_de: std::collections::HashMap<(u32, char), u32>,
    letras: Vec<([f64; 2], [f32; 4], u32, u32)>,
    piezas_le: Vec<Pieza>,
    arcos: Vec<([f64; 2], f64, f64, f64, u32)>,
    piezas_ar: Vec<Pieza>,
    pub unidades: u32,
    caja: Option<[f64; 4]>,
    pub sin_dibujar: std::collections::BTreeMap<String, u32>,
    /// Tope de vertices, para no ahogar un portatil con un plano enorme.
    pub tope: usize,
}

fn nivel_de(tamano: f64) -> i32 {
    if tamano <= 0.0 || !tamano.is_finite() {
        return -1000;
    }
    tamano.log2().floor().clamp(-60.0, 60.0) as i32
}

fn crecer(c: &mut Option<[f64; 4]>, p: [f64; 2]) {
    match c {
        None => *c = Some([p[0], p[1], p[0], p[1]]),
        Some(c) => {
            c[0] = c[0].min(p[0]);
            c[1] = c[1].min(p[1]);
            c[2] = c[2].max(p[0]);
            c[3] = c[3].max(p[1]);
        }
    }
}

fn valido(p: [f64; 2]) -> bool {
    p[0].is_finite() && p[1].is_finite() && p[0].abs() < 1e12 && p[1].abs() < 1e12
}

fn lado(c: &[f64; 4]) -> f64 {
    (c[2] - c[0]).max(c[3] - c[1])
}

impl Constructor {
    /// **Lo que importa del plano**: del 0,5 % al 99,5 % de los puntos en
    /// cada eje (la caja entera si son pocos). Su centro es el origen de las
    /// coordenadas `f32` y en ella va la rejilla para dibujar solo lo que se
    /// ve.
    ///
    /// Un `f32` guarda unos 7 digitos: a 4 000 km del origen ya solo
    /// distingue medio metro. Con el centro de la caja, un objeto olvidado
    /// lejos (en el plano de señalizacion del usuario, un cuadradito de 10 cm
    /// al doble de sus coordenadas UTM) dejaba el plano entero a 4 000 km
    /// del origen: cada vertice caia a la rejilla de 0,5 m mas cercana, las
    /// letras de las placas salian partidas y a saltos y las franjas de un
    /// paso de cebra o una rampa, torcidas (12-oct-2026); y todo el plano en
    /// una sola celda de la rejilla. Con esta caja lo que se mira va preciso;
    /// lo suelto y lejano pierde decimales, no el plano.
    fn caja_util(&self, caja: [f64; 4]) -> [f64; 4] {
        let n = self.puntos.len() + self.puntos_trama.len() + self.letras.len() + self.arcos.len();
        if n < 200 {
            return caja;
        }
        let paso = (n / 40_000).max(1);
        let todos = self
            .puntos
            .iter()
            .map(|p| p.0)
            .chain(self.puntos_trama.iter().map(|p| p.0))
            .chain(self.letras.iter().map(|l| l.0))
            .chain(self.arcos.iter().map(|a| a.0))
            .step_by(paso);
        let (mut xs, mut ys): (Vec<f64>, Vec<f64>) = todos.map(|p| (p[0], p[1])).unzip();
        let mut out = caja;
        for (k, v) in [&mut xs, &mut ys].into_iter().enumerate() {
            v.sort_by(f64::total_cmp);
            out[k] = v[v.len() / 200];
            out[k + 2] = v[v.len() - 1 - v.len() / 200];
        }
        out
    }

    pub fn nuevo() -> Self {
        Self {
            tope: 30_000_000,
            ..Default::default()
        }
    }

    pub fn vertices(&self) -> usize {
        self.puntos.len() + self.puntos_trama.len() + self.letras.len() * 3 + self.arcos.len() * 3
    }

    pub fn lleno(&self) -> bool {
        self.vertices() >= self.tope
    }

    fn sumar_caja(&mut self, c: [f64; 4]) {
        crecer(&mut self.caja, [c[0], c[1]]);
        crecer(&mut self.caja, [c[2], c[3]]);
    }

    /// Una polilinea (abierta). `tamano` es lo que mide la cosa entera
    /// (`None`: el de su caja).
    pub fn polilinea(&mut self, puntos: &[[f64; 2]], color: u32, tamano: Option<f64>) {
        if puntos.len() < 2 || self.lleno() {
            return;
        }
        let mut caja = None;
        let desde = self.lineas.len() as u32;
        let mut n = 0;
        for p in puntos.iter().copied().filter(|p| valido(*p)) {
            crecer(&mut caja, p);
            self.lineas.push(self.puntos.len() as u32);
            self.puntos.push((p, color));
            n += 1;
        }
        if n < 2 {
            self.lineas.truncate(desde as usize);
            self.puntos.truncate(self.puntos.len() - n);
            return;
        }
        self.lineas.push(CORTE);
        let caja = caja.unwrap_or_default();
        let t = tamano.unwrap_or_else(|| lado(&caja));
        let t = if t <= 0.0 { 1e-9 } else { t };
        self.sumar_caja(caja);
        self.piezas_l.push(Pieza {
            desde,
            cuantos: self.lineas.len() as u32 - desde,
            caja,
            nivel: nivel_de(t),
            clase: 0,
        });
    }

    /// Triangulos sueltos (cada tres puntos, uno).
    pub fn triangulos(&mut self, puntos: &[[f64; 2]], color: u32, tamano: Option<f64>) {
        if puntos.len() < 3 || self.lleno() {
            return;
        }
        let mut caja = None;
        let desde = self.triangulos.len() as u32;
        for tri in puntos.chunks_exact(3) {
            if !tri.iter().all(|p| valido(*p)) {
                continue;
            }
            for p in tri {
                crecer(&mut caja, *p);
                self.triangulos.push(self.puntos.len() as u32);
                self.puntos.push((*p, color));
            }
        }
        let Some(caja) = caja else { return };
        let t = tamano.unwrap_or_else(|| lado(&caja));
        self.sumar_caja(caja);
        self.piezas_t.push(Pieza {
            desde,
            cuantos: self.triangulos.len() as u32 - desde,
            caja,
            nivel: nivel_de(t),
            clase: 0,
        });
    }

    /// Una malla de triangulos con sus puntos compartidos (un relleno).
    pub fn malla(&mut self, puntos: &[[f64; 2]], indices: &[u32], color: u32, tamano: Option<f64>) {
        if indices.len() < 3 || self.lleno() || puntos.iter().any(|p| !valido(*p)) {
            return;
        }
        let base = self.puntos.len() as u32;
        let mut caja = None;
        for p in puntos {
            crecer(&mut caja, *p);
            self.puntos.push((*p, color));
        }
        let desde = self.triangulos.len() as u32;
        for tri in indices.chunks_exact(3) {
            if tri.iter().all(|&i| (i as usize) < puntos.len()) {
                self.triangulos.extend(tri.iter().map(|&i| base + i));
            }
        }
        let Some(caja) = caja else { return };
        let t = tamano.unwrap_or_else(|| lado(&caja));
        self.sumar_caja(caja);
        self.piezas_t.push(Pieza {
            desde,
            cuantos: self.triangulos.len() as u32 - desde,
            caja,
            nivel: nivel_de(t),
            clase: 0,
        });
    }

    /// Un sombreado con patron: su contorno relleno (malla en el plano),
    /// la matriz del plano al sistema del sombreado (`al_plano`: 2x2 + punto,
    /// la que lleva del sistema del sombreado al plano) y sus familias.
    pub fn trama(
        &mut self,
        puntos: &[[f64; 2]],
        indices: &[u32],
        color: u32,
        al_plano: [f64; 6],
        familias: &[FamiliaPlano],
    ) {
        if indices.len() < 3
            || familias.is_empty()
            || self.lleno()
            || puntos.iter().any(|p| !valido(*p))
        {
            return;
        }
        let mut caja = None;
        for p in puntos {
            crecer(&mut caja, *p);
        }
        let Some(caja) = caja else { return };
        let centro = [(caja[0] + caja[2]) / 2.0, (caja[1] + caja[3]) / 2.0];
        // Inversa de la 2x2 [a b; c d] (columnas: ejes x e y del sombreado).
        let [a, b, c, d, tx, ty] = al_plano;
        let det = a * d - b * c;
        if det.abs() < 1e-300 || !det.is_finite() {
            return;
        }
        let inv = [d / det, -b / det, -c / det, a / det];
        // El centro en el sistema del sombreado.
        let (ux, uy) = (centro[0] - tx, centro[1] - ty);
        let centro_local = [inv[0] * ux + inv[1] * uy, inv[2] * ux + inv[3] * uy];
        let escala = det.abs().sqrt();
        let id = self.tramas.len() as u32;
        let desde_fam = self.familias.len() as u32;
        let mut paso_min = f64::MAX;
        for f in familias.iter().take(32) {
            let dir = [f.angulo.cos(), f.angulo.sin()];
            let n = [-dir[1], dir[0]];
            let paso = f.desplazamiento[0] * n[0] + f.desplazamiento[1] * n[1];
            if paso.abs() < 1e-12 || !paso.is_finite() {
                continue;
            }
            let corr = f.desplazamiento[0] * dir[0] + f.desplazamiento[1] * dir[1];
            // La base, llevada a la raya mas cercana al centro: asi lo que se
            // resta en la tarjeta son numeros pequenos.
            let (vx, vy) = (centro_local[0] - f.base[0], centro_local[1] - f.base[1]);
            let k = ((vx * n[0] + vy * n[1]) / paso).round();
            let base = [
                f.base[0] + k * f.desplazamiento[0] - centro_local[0],
                f.base[1] + k * f.desplazamiento[1] - centro_local[1],
            ];
            let mut trazos = [0f32; 8];
            let nt = f.trazos.len().min(8);
            for (i, t) in f.trazos.iter().take(8).enumerate() {
                trazos[i] = *t as f32;
            }
            let largo: f64 = f.trazos.iter().take(8).map(|t| t.abs()).sum();
            paso_min = paso_min.min(paso.abs() * escala);
            self.familias.push(Familia {
                base: [base[0] as f32, base[1] as f32],
                dir: [dir[0] as f32, dir[1] as f32],
                paso: paso as f32,
                corrimiento: corr as f32,
                largo: largo as f32,
                n: nt as u32,
                trazos,
            });
        }
        let cuantas = self.familias.len() as u32 - desde_fam;
        if cuantas == 0 {
            return;
        }
        self.tramas.push((centro, inv, escala, desde_fam, cuantas));
        let desde = self.triangulos_trama.len() as u32;
        let base = self.puntos_trama.len() as u32;
        for p in puntos {
            self.puntos_trama.push((*p, color, id));
        }
        for tri in indices.chunks_exact(3) {
            if tri.iter().all(|&i| (i as usize) < puntos.len()) {
                self.triangulos_trama.extend(tri.iter().map(|&i| base + i));
            }
        }
        self.sumar_caja(caja);
        let _ = paso_min;
        self.piezas_tr.push(Pieza {
            desde,
            cuantos: self.triangulos_trama.len() as u32 - desde,
            caja,
            nivel: nivel_de(lado(&caja)),
            clase: 0,
        });
    }

    /// Un circulo o un arco (ya en el plano): `barrido` de 2π es entero.
    pub fn arco(&mut self, centro: [f64; 2], radio: f64, inicio: f64, barrido: f64, color: u32) {
        if !valido(centro) || !(radio > 0.0) || !radio.is_finite() || self.lleno() {
            return;
        }
        let caja = [
            centro[0] - radio,
            centro[1] - radio,
            centro[0] + radio,
            centro[1] + radio,
        ];
        self.sumar_caja(caja);
        let i = self.arcos.len() as u32;
        self.arcos.push((centro, radio, inicio, barrido, color));
        self.piezas_ar.push(Pieza {
            desde: i,
            cuantos: 1,
            caja,
            nivel: nivel_de(radio * 2.0),
            clase: 0,
        });
    }

    /// El numero de una letra distinta (fuente y caracter); la rellena con
    /// `malla` la primera vez. `rayas`: la malla son pares de puntos.
    pub fn glifo(
        &mut self,
        fuente: u32,
        ch: char,
        rayas: bool,
        malla: impl FnOnce() -> Vec<[f32; 2]>,
    ) -> u32 {
        if let Some(g) = self.glifo_de.get(&(fuente, ch)) {
            return *g;
        }
        let m = malla();
        let desde = self.malla_letras.len() as u32;
        self.malla_letras.extend_from_slice(&m);
        let g = self.glifos.len() as u32;
        self.glifos.push([
            desde,
            m.len() as u32 | if rayas { GLIFO_DE_RAYAS } else { 0 },
        ]);
        self.glifo_de.insert((fuente, ch), g);
        g
    }

    /// Una letra puesta: `pos` en el plano y `m` (2x2) de em al plano.
    /// `alto` es el del texto, para no dibujarla si de lejos no se ve.
    pub fn letra(&mut self, glifo: u32, pos: [f64; 2], m: [f64; 4], color: u32, alto: f64) {
        let Some(&[_, n]) = self.glifos.get(glifo as usize) else {
            return;
        };
        let rayas = n & GLIFO_DE_RAYAS;
        let n = n & !GLIFO_DE_RAYAS;
        if n == 0 || !valido(pos) || self.lleno() {
            return;
        }
        // La caja de la letra: su em cuadrado llevado al plano.
        let mut caja = None;
        for (x, y) in [(0.0, -0.25), (1.0, -0.25), (0.0, 1.0), (1.0, 1.0)] {
            crecer(
                &mut caja,
                [pos[0] + m[0] * x + m[1] * y, pos[1] + m[2] * x + m[3] * y],
            );
        }
        let caja = caja.unwrap_or_default();
        self.sumar_caja(caja);
        let i = self.letras.len() as u32;
        self.letras.push((
            pos,
            [m[0] as f32, m[1] as f32, m[2] as f32, m[3] as f32],
            color,
            glifo,
        ));
        self.piezas_le.push(Pieza {
            desde: i,
            cuantos: 1,
            caja,
            nivel: nivel_de(alto),
            clase: clase_de(n) | rayas,
        });
    }

    pub fn no_dibujado(&mut self, tipo: &str) {
        *self.sin_dibujar.entry(tipo.to_string()).or_default() += 1;
    }

    /// Lo ordena todo y lo pasa a `f32` respecto al centro.
    pub fn terminar(self) -> Modelo {
        let Some(caja) = self.caja else {
            return Modelo {
                sin_dibujar: self.sin_dibujar.into_iter().collect(),
                ..Default::default()
            };
        };
        let util = self.caja_util(caja);
        let origen = [(util[0] + util[2]) / 2.0, (util[1] + util[3]) / 2.0];
        let rel = |c: [f64; 4]| -> [f32; 4] {
            [
                (c[0] - origen[0]) as f32,
                (c[1] - origen[1]) as f32,
                (c[2] - origen[0]) as f32,
                (c[3] - origen[1]) as f32,
            ]
        };
        const REJILLA: f64 = 24.0;
        let (ancho, alto) = ((util[2] - util[0]).max(1e-9), (util[3] - util[1]).max(1e-9));
        let celda = |c: &[f64; 4]| -> u32 {
            let cx = (((c[0] + c[2]) / 2.0 - util[0]) / ancho * REJILLA).clamp(0.0, REJILLA - 1.0);
            let cy = (((c[1] + c[3]) / 2.0 - util[1]) / alto * REJILLA).clamp(0.0, REJILLA - 1.0);
            cy as u32 * REJILLA as u32 + cx as u32
        };
        // Ordena las piezas por (nivel de mayor a menor, celda, clase) y
        // devuelve el nuevo orden de sus trozos y los tramos.
        let ordenar = |piezas: Vec<Pieza>| -> (Vec<(u32, u32)>, Vec<Tramo>) {
            let mut piezas: Vec<(i32, u32, Pieza)> = piezas
                .into_iter()
                .map(|p| (p.nivel, celda(&p.caja), p))
                .collect();
            piezas.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then(a.1.cmp(&b.1))
                    .then(a.2.clase.cmp(&b.2.clase))
            });
            let mut orden = Vec::with_capacity(piezas.len());
            let mut tramos: Vec<Tramo> = Vec::new();
            let mut actual: Option<(i32, u32, Tramo)> = None;
            let mut puesto = 0u32;
            for (nivel, c, p) in piezas {
                orden.push((p.desde, p.cuantos));
                let pc = rel(p.caja);
                match &mut actual {
                    Some((n, cc, t)) if *n == nivel && *cc == c && t.clase == p.clase => {
                        t.cuantos += p.cuantos;
                        t.caja = [
                            t.caja[0].min(pc[0]),
                            t.caja[1].min(pc[1]),
                            t.caja[2].max(pc[2]),
                            t.caja[3].max(pc[3]),
                        ];
                    }
                    _ => {
                        if let Some((_, _, t)) = actual.take() {
                            tramos.push(t);
                        }
                        actual = Some((
                            nivel,
                            c,
                            Tramo {
                                desde: puesto,
                                cuantos: p.cuantos,
                                caja: pc,
                                tamano: 2f32.powi(nivel.max(-126)),
                                clase: p.clase,
                            },
                        ));
                    }
                }
                puesto += p.cuantos;
            }
            if let Some((_, _, t)) = actual {
                tramos.push(t);
            }
            (orden, tramos)
        };
        let reordenar = |orden: &[(u32, u32)], fuente: &[u32]| -> Vec<u32> {
            let mut v = Vec::with_capacity(fuente.len());
            for &(d, n) in orden {
                v.extend_from_slice(&fuente[d as usize..(d + n) as usize]);
            }
            v
        };
        let (ol, tramos_lineas) = ordenar(self.piezas_l);
        let lineas = reordenar(&ol, &self.lineas);
        let (ot, tramos_triangulos) = ordenar(self.piezas_t);
        let triangulos = reordenar(&ot, &self.triangulos);
        let (otr, tramos_trama) = ordenar(self.piezas_tr);
        let triangulos_trama = reordenar(&otr, &self.triangulos_trama);
        let (ole, tramos_letras) = ordenar(self.piezas_le);
        let (oar, tramos_arcos) = ordenar(self.piezas_ar);
        let arcos = oar
            .iter()
            .map(|&(d, _)| {
                let (c, r, a0, b, color) = self.arcos[d as usize];
                Arco {
                    centro: [(c[0] - origen[0]) as f32, (c[1] - origen[1]) as f32],
                    radio: r as f32,
                    inicio: a0 as f32,
                    barrido: b as f32,
                    color,
                    relleno: [0; 2],
                }
            })
            .collect();
        let letras = ole
            .iter()
            .map(|&(d, _)| {
                let (p, m, color, glifo) = self.letras[d as usize];
                Letra {
                    pos: [(p[0] - origen[0]) as f32, (p[1] - origen[1]) as f32],
                    m,
                    color,
                    glifo,
                }
            })
            .collect();
        let vertices = self
            .puntos
            .iter()
            .map(|(p, c)| Vertice {
                x: (p[0] - origen[0]) as f32,
                y: (p[1] - origen[1]) as f32,
                color: *c,
            })
            .collect();
        let vertices_trama = self
            .puntos_trama
            .iter()
            .map(|(p, c, t)| VerticeTrama {
                x: (p[0] - origen[0]) as f32,
                y: (p[1] - origen[1]) as f32,
                color: *c,
                trama: *t,
            })
            .collect();
        let tramas = self
            .tramas
            .iter()
            .map(|(c, inv, e, d, n)| Trama {
                centro: [(c[0] - origen[0]) as f32, (c[1] - origen[1]) as f32],
                inversa: [inv[0] as f32, inv[1] as f32, inv[2] as f32, inv[3] as f32],
                escala: *e as f32,
                desde: *d,
                cuantas: *n,
            })
            .collect();
        Modelo {
            origen,
            caja: rel(caja),
            vertices,
            lineas,
            triangulos,
            tramos_lineas,
            tramos_triangulos,
            vertices_trama,
            triangulos_trama,
            tramos_trama,
            tramas,
            familias: self.familias,
            malla_letras: self.malla_letras,
            glifos: self.glifos,
            letras,
            tramos_letras,
            arcos,
            tramos_arcos,
            unidades: self.unidades,
            sin_dibujar: self.sin_dibujar.into_iter().collect(),
        }
    }
}

/// Las letras se dibujan todas con el mismo numero de vertices dentro de un
/// tramo: se agrupan por el que necesitan, redondeado a potencias de dos.
fn clase_de(n: u32) -> u32 {
    n.max(24).next_power_of_two()
}

// ------------------------------------------------------------- guardar

/// 7: el origen va en el centro de lo que importa, no en el de la caja.
const MAGIA: &[u8; 8] = b"PXCAD\0\0\x07";

/// Escribe y lee todo como palabras de 4 bytes (lo que tambien se sube a la
/// tarjeta tal cual).
trait Palabras: Sized {
    const N: usize;
    fn poner(&self, v: &mut Vec<u32>);
    fn tomar(p: &[u32]) -> Self;
}

fn f(x: f32) -> u32 {
    x.to_bits()
}
fn g(x: u32) -> f32 {
    f32::from_bits(x)
}

impl Palabras for Vertice {
    const N: usize = 3;
    fn poner(&self, v: &mut Vec<u32>) {
        v.extend([f(self.x), f(self.y), self.color]);
    }
    fn tomar(p: &[u32]) -> Self {
        Vertice {
            x: g(p[0]),
            y: g(p[1]),
            color: p[2],
        }
    }
}
impl Palabras for VerticeTrama {
    const N: usize = 4;
    fn poner(&self, v: &mut Vec<u32>) {
        v.extend([f(self.x), f(self.y), self.color, self.trama]);
    }
    fn tomar(p: &[u32]) -> Self {
        VerticeTrama {
            x: g(p[0]),
            y: g(p[1]),
            color: p[2],
            trama: p[3],
        }
    }
}
impl Palabras for Trama {
    const N: usize = 9;
    fn poner(&self, v: &mut Vec<u32>) {
        v.extend([f(self.centro[0]), f(self.centro[1])]);
        v.extend(self.inversa.map(f));
        v.extend([f(self.escala), self.desde, self.cuantas]);
    }
    fn tomar(p: &[u32]) -> Self {
        Trama {
            centro: [g(p[0]), g(p[1])],
            inversa: [g(p[2]), g(p[3]), g(p[4]), g(p[5])],
            escala: g(p[6]),
            desde: p[7],
            cuantas: p[8],
        }
    }
}
impl Palabras for Familia {
    const N: usize = 16;
    fn poner(&self, v: &mut Vec<u32>) {
        v.extend([
            f(self.base[0]),
            f(self.base[1]),
            f(self.dir[0]),
            f(self.dir[1]),
            f(self.paso),
            f(self.corrimiento),
            f(self.largo),
            self.n,
        ]);
        v.extend(self.trazos.map(f));
    }
    fn tomar(p: &[u32]) -> Self {
        let mut trazos = [0f32; 8];
        for i in 0..8 {
            trazos[i] = g(p[8 + i]);
        }
        Familia {
            base: [g(p[0]), g(p[1])],
            dir: [g(p[2]), g(p[3])],
            paso: g(p[4]),
            corrimiento: g(p[5]),
            largo: g(p[6]),
            n: p[7],
            trazos,
        }
    }
}
impl Palabras for Letra {
    const N: usize = 8;
    fn poner(&self, v: &mut Vec<u32>) {
        v.extend([f(self.pos[0]), f(self.pos[1])]);
        v.extend(self.m.map(f));
        v.extend([self.color, self.glifo]);
    }
    fn tomar(p: &[u32]) -> Self {
        Letra {
            pos: [g(p[0]), g(p[1])],
            m: [g(p[2]), g(p[3]), g(p[4]), g(p[5])],
            color: p[6],
            glifo: p[7],
        }
    }
}
impl Palabras for Arco {
    const N: usize = 8;
    fn poner(&self, v: &mut Vec<u32>) {
        v.extend([
            f(self.centro[0]),
            f(self.centro[1]),
            f(self.radio),
            f(self.inicio),
            f(self.barrido),
            self.color,
            0,
            0,
        ]);
    }
    fn tomar(p: &[u32]) -> Self {
        Arco {
            centro: [g(p[0]), g(p[1])],
            radio: g(p[2]),
            inicio: g(p[3]),
            barrido: g(p[4]),
            color: p[5],
            relleno: [0; 2],
        }
    }
}
impl Palabras for Tramo {
    const N: usize = 8;
    fn poner(&self, v: &mut Vec<u32>) {
        v.extend([self.desde, self.cuantos]);
        v.extend(self.caja.map(f));
        v.extend([f(self.tamano), self.clase]);
    }
    fn tomar(p: &[u32]) -> Self {
        Tramo {
            desde: p[0],
            cuantos: p[1],
            caja: [g(p[2]), g(p[3]), g(p[4]), g(p[5])],
            tamano: g(p[6]),
            clase: p[7],
        }
    }
}
impl Palabras for u32 {
    const N: usize = 1;
    fn poner(&self, v: &mut Vec<u32>) {
        v.push(*self);
    }
    fn tomar(p: &[u32]) -> Self {
        p[0]
    }
}
impl Palabras for [f32; 2] {
    const N: usize = 2;
    fn poner(&self, v: &mut Vec<u32>) {
        v.extend(self.map(f));
    }
    fn tomar(p: &[u32]) -> Self {
        [g(p[0]), g(p[1])]
    }
}
impl Palabras for [u32; 2] {
    const N: usize = 2;
    fn poner(&self, v: &mut Vec<u32>) {
        v.extend(*self);
    }
    fn tomar(p: &[u32]) -> Self {
        [p[0], p[1]]
    }
}

fn poner_lista<T: Palabras>(v: &mut Vec<u32>, l: &[T]) {
    v.push(l.len() as u32);
    for x in l {
        x.poner(v);
    }
}

struct Lector<'a> {
    p: &'a [u32],
    i: usize,
}

impl Lector<'_> {
    fn uno(&mut self) -> Option<u32> {
        let x = *self.p.get(self.i)?;
        self.i += 1;
        Some(x)
    }
    fn lista<T: Palabras>(&mut self) -> Option<Vec<T>> {
        let n = self.uno()? as usize;
        let fin = self.i.checked_add(n.checked_mul(T::N)?)?;
        let trozo = self.p.get(self.i..fin)?;
        self.i = fin;
        Some(trozo.chunks_exact(T::N).map(T::tomar).collect())
    }
}

impl Modelo {
    /// En bytes, para la cache (`<raiz>/cache/cad/`): abrir otra vez el
    /// mismo plano es leer esto, sin volver a pasar por el DWG.
    pub fn a_bytes(&self) -> Vec<u8> {
        let mut w: Vec<u32> = Vec::with_capacity(self.bytes_gpu() / 4 + 64);
        w.extend([
            self.origen[0].to_bits() as u32,
            (self.origen[0].to_bits() >> 32) as u32,
        ]);
        w.extend([
            self.origen[1].to_bits() as u32,
            (self.origen[1].to_bits() >> 32) as u32,
        ]);
        w.extend(self.caja.map(f));
        poner_lista(&mut w, &self.vertices);
        poner_lista(&mut w, &self.lineas);
        poner_lista(&mut w, &self.triangulos);
        poner_lista(&mut w, &self.tramos_lineas);
        poner_lista(&mut w, &self.tramos_triangulos);
        poner_lista(&mut w, &self.vertices_trama);
        poner_lista(&mut w, &self.triangulos_trama);
        poner_lista(&mut w, &self.tramos_trama);
        poner_lista(&mut w, &self.tramas);
        poner_lista(&mut w, &self.familias);
        poner_lista(&mut w, &self.malla_letras);
        poner_lista(&mut w, &self.glifos);
        poner_lista(&mut w, &self.letras);
        poner_lista(&mut w, &self.tramos_letras);
        poner_lista(&mut w, &self.arcos);
        poner_lista(&mut w, &self.tramos_arcos);
        w.push(self.unidades);
        w.push(self.sin_dibujar.len() as u32);
        for (t, n) in &self.sin_dibujar {
            let b = t.as_bytes();
            w.push(b.len() as u32);
            for trozo in b.chunks(4) {
                let mut x = [0u8; 4];
                x[..trozo.len()].copy_from_slice(trozo);
                w.push(u32::from_le_bytes(x));
            }
            w.push(*n);
        }
        let mut b = Vec::with_capacity(8 + w.len() * 4);
        b.extend_from_slice(MAGIA);
        for x in w {
            b.extend_from_slice(&x.to_le_bytes());
        }
        b
    }

    pub fn de_bytes(b: &[u8]) -> Option<Modelo> {
        if b.len() < 8 || &b[..8] != MAGIA || (b.len() - 8) % 4 != 0 {
            return None;
        }
        let w: Vec<u32> = b[8..]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let mut r = Lector { p: &w, i: 0 };
        let o0 = f64::from_bits(r.uno()? as u64 | (r.uno()? as u64) << 32);
        let o1 = f64::from_bits(r.uno()? as u64 | (r.uno()? as u64) << 32);
        let caja = [g(r.uno()?), g(r.uno()?), g(r.uno()?), g(r.uno()?)];
        let m = Modelo {
            origen: [o0, o1],
            caja,
            vertices: r.lista()?,
            lineas: r.lista()?,
            triangulos: r.lista()?,
            tramos_lineas: r.lista()?,
            tramos_triangulos: r.lista()?,
            vertices_trama: r.lista()?,
            triangulos_trama: r.lista()?,
            tramos_trama: r.lista()?,
            tramas: r.lista()?,
            familias: r.lista()?,
            malla_letras: r.lista()?,
            glifos: r.lista()?,
            letras: r.lista()?,
            tramos_letras: r.lista()?,
            arcos: r.lista()?,
            tramos_arcos: r.lista()?,
            unidades: r.uno()?,
            sin_dibujar: {
                let n = r.uno()? as usize;
                let mut v = Vec::new();
                for _ in 0..n.min(10_000) {
                    let largo = r.uno()? as usize;
                    let mut bytes = Vec::new();
                    for _ in 0..largo.div_ceil(4) {
                        bytes.extend_from_slice(&r.uno()?.to_le_bytes());
                    }
                    bytes.truncate(largo);
                    v.push((String::from_utf8_lossy(&bytes).into_owned(), r.uno()?));
                }
                v
            },
        };
        m.valido().then_some(m)
    }

    /// Que ningun indice se salga: una cache rota no tumba la tarjeta.
    pub fn valido(&self) -> bool {
        let nv = self.vertices.len() as u32;
        let nt = self.vertices_trama.len() as u32;
        let ok_tramos = |t: &[Tramo], n: usize| {
            t.iter()
                .all(|t| (t.desde as usize + t.cuantos as usize) <= n)
        };
        self.lineas.iter().all(|&i| i == CORTE || i < nv)
            && self.triangulos.iter().all(|&i| i < nv)
            && self.triangulos_trama.iter().all(|&i| i < nt)
            && self
                .vertices_trama
                .iter()
                .all(|v| (v.trama as usize) < self.tramas.len())
            && self
                .tramas
                .iter()
                .all(|t| (t.desde as usize + t.cuantas as usize) <= self.familias.len())
            && self.glifos.iter().all(|g| {
                (g[0] as usize + (g[1] & !GLIFO_DE_RAYAS) as usize) <= self.malla_letras.len()
            })
            && self
                .letras
                .iter()
                .all(|l| (l.glifo as usize) < self.glifos.len())
            && ok_tramos(&self.tramos_lineas, self.lineas.len())
            && ok_tramos(&self.tramos_triangulos, self.triangulos.len())
            && ok_tramos(&self.tramos_trama, self.triangulos_trama.len())
            && ok_tramos(&self.tramos_letras, self.letras.len())
            && ok_tramos(&self.tramos_arcos, self.arcos.len())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_objeto_suelto_a_kilometros_no_cuenta_para_encuadrar() {
        // El plano de demolicion del usuario: todo en una esquina de 1,5 km
        // y dos cositas a 340 km, en la otra.
        let mut c = Constructor::nuevo();
        for i in 0..400 {
            let x = -170_000.0 + (i % 20) as f64 * 70.0;
            let y = -35_000.0 + (i / 20) as f64 * 40.0;
            c.polilinea(&[[x, y], [x + 50.0, y + 20.0]], 0xff00_00ff, None);
        }
        c.polilinea(
            &[[170_600.0, 35_000.0], [170_700.0, 35_200.0]],
            0xff00_00ff,
            None,
        );
        let m = c.terminar();
        let u = m.caja_util();
        assert!(
            u[2] - u[0] < 3_000.0 && u[3] - u[1] < 3_000.0,
            "{u:?} de {:?}",
            m.caja
        );
        // Caso negativo: un plano normal (pocos puntos) se encuadra entero.
        let mut c = Constructor::nuevo();
        c.polilinea(&[[0.0, 0.0], [100.0, 50.0]], 0xff00_00ff, None);
        let m = c.terminar();
        assert_eq!(m.caja_util(), m.caja);
    }

    #[test]
    fn un_objeto_suelto_lejos_no_quita_precision_al_plano() {
        // El plano de señalizacion del usuario (12-oct-2026): calles en UTM
        // (378 900, 8 285 000) y un cuadradito de 10 cm al doble de eso.
        let (x0, y0) = (378_900.123_4, 8_285_000.371_9);
        let mut puntos = Vec::new();
        let mut c = Constructor::nuevo();
        for i in 0..300 {
            let p = [x0 + (i % 20) as f64 * 3.17, y0 + (i / 20) as f64 * 0.659];
            let q = [p[0] + 0.413, p[1] + 0.251];
            c.polilinea(&[p, q], 0xff00_00ff, None);
            puntos.extend([p, q]);
        }
        let g = c.glifo(0, 'J', false, || vec![[0.0, 0.0], [0.5, 0.7], [0.1, 0.0]]);
        let letra = [378_979.965_3, 8_285_059.720_7];
        c.letra(g, letra, [0.92, 0.016, -0.016, 0.92], 0xffff_ffff, 0.66);
        let suelto = [
            [758_222.917, 16_570_059.798],
            [758_223.025, 16_570_059.798],
            [758_223.025, 16_570_059.690],
        ];
        c.polilinea(&suelto, 0xff00_00ff, None);
        let m = c.terminar();
        // La causa: con el origen en el centro de la caja, un f32 solo
        // distingue medio metro en el plano.
        let medio = [(x0 + suelto[0][0]) / 2.0, (y0 + suelto[0][1]) / 2.0];
        let viejo = puntos
            .iter()
            .map(|p| (((p[1] - medio[1]) as f32) as f64 + medio[1] - p[1]).abs())
            .fold(0.0, f64::max);
        assert!(viejo > 0.1, "{viejo}");
        // Con el origen en lo que importa, cada vertice vuelve a su sitio.
        let mut peor = 0.0f64;
        for (v, p) in m.vertices.iter().zip(puntos.iter()) {
            peor = peor
                .max((v.x as f64 + m.origen[0] - p[0]).abs())
                .max((v.y as f64 + m.origen[1] - p[1]).abs());
        }
        assert!(peor < 1e-3, "error de {peor} m");
        let l = m.letras[0];
        assert!(
            (l.pos[0] as f64 + m.origen[0] - letra[0]).abs() < 1e-3
                && (l.pos[1] as f64 + m.origen[1] - letra[1]).abs() < 1e-3
        );
        // Caso negativo: lo suelto no se pierde (solo va menos fino) y la
        // caja lo sigue contando.
        assert_eq!(m.vertices.len(), puntos.len() + suelto.len());
        let s = m.vertices[puntos.len()];
        assert!(
            (s.x as f64 + m.origen[0] - suelto[0][0]).abs() < 1.0
                && (s.y as f64 + m.origen[1] - suelto[0][1]).abs() < 1.0
        );
        assert!(m.caja[3] as f64 + m.origen[1] > 16_000_000.0);
        // Y la rejilla reparte el plano en celdas (con la caja entera, todo
        // caia en una y se dibujaba siempre entero).
        assert!(m.tramos_lineas.len() >= 4, "{}", m.tramos_lineas.len());
    }

    fn de_prueba() -> Modelo {
        let mut c = Constructor::nuevo();
        c.polilinea(&[[0.0, 0.0], [1000.0, 0.0]], 0xff0000ff, None);
        c.polilinea(&[[10.0, 10.0], [10.5, 10.0]], 0xff00ff00, None);
        c.triangulos(&[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], 0xffffffff, None);
        c.trama(
            &[[0.0, 0.0], [10.0, 0.0], [0.0, 10.0]],
            &[0, 1, 2],
            0xff00ffff,
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            &[FamiliaPlano {
                angulo: 0.0,
                base: [0.0, 0.0],
                desplazamiento: [0.0, 1.0],
                trazos: vec![],
            }],
        );
        let g = c.glifo(0, 'A', false, || vec![[0.0, 0.0], [0.5, 1.0], [1.0, 0.0]]);
        assert_eq!(c.glifo(0, 'A', false, || panic!("ya estaba")), g);
        let r = c.glifo(1, 'L', true, || {
            vec![[0.0, 1.0], [0.0, 0.0], [0.0, 0.0], [0.5, 0.0]]
        });
        c.letra(r, [6.0, 5.0], [2.0, 0.0, 0.0, 2.0], 0xffffffff, 2.0);
        c.arco([3.0, 3.0], 2.0, 0.0, std::f64::consts::TAU, 0xff0000ff);
        c.unidades = 6;
        c.letra(g, [5.0, 5.0], [2.0, 0.0, 0.0, 2.0], 0xffffffff, 2.0);
        c.no_dibujado("SOLID3D");
        c.terminar()
    }

    #[test]
    fn lo_grande_va_antes_y_cada_tramo_sabe_su_caja() {
        let m = de_prueba();
        assert_eq!(m.origen, [500.0, 5.0]);
        assert_eq!(m.tramos_lineas.len(), 2);
        assert!(m.tramos_lineas[0].tamano > m.tramos_lineas[1].tamano);
        assert_eq!(m.tramos_lineas[0].caja, [-500.0, -5.0, 500.0, -5.0]);
        assert_eq!(m.lineas.iter().filter(|i| **i == CORTE).count(), 2);
        assert_eq!(m.triangulos.len(), 3);
        assert_eq!(
            (m.tramas.len(), m.familias.len(), m.triangulos_trama.len()),
            (1, 1, 3)
        );
        assert_eq!((m.glifos.len(), m.letras.len()), (2, 2));
        assert!(
            m.tramos_letras
                .iter()
                .any(|t| t.clase & GLIFO_DE_RAYAS != 0)
        );
        assert!(
            m.tramos_letras
                .iter()
                .any(|t| t.clase & GLIFO_DE_RAYAS == 0)
        );
        assert_eq!((m.arcos.len(), m.unidades), (1, 6));
        assert!(m.valido());
        // Casos negativos: lo que no es un numero no entra, y una linea de
        // un solo punto tampoco.
        let mut c = Constructor::nuevo();
        c.polilinea(&[[f64::NAN, 0.0], [1.0, 1.0]], 0, None);
        assert!(c.terminar().vacio());
    }

    #[test]
    fn la_base_del_patron_se_trae_cerca_del_centro() {
        let mut c = Constructor::nuevo();
        // Un sombreado lejos de su base (UTM): la base queda a menos de un paso.
        c.trama(
            &[[8_000_000.0, 0.0], [8_000_010.0, 0.0], [8_000_000.0, 10.0]],
            &[0, 1, 2],
            0,
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            &[FamiliaPlano {
                angulo: 0.0,
                base: [0.0, 0.3],
                desplazamiento: [0.0, 1.0],
                trazos: vec![1.0, -1.0],
            }],
        );
        let m = c.terminar();
        assert!(
            m.familias[0].base[1].abs() <= 0.5 + 1e-6,
            "{:?}",
            m.familias[0]
        );
        assert_eq!(m.familias[0].largo, 2.0);
    }

    #[test]
    fn la_cache_vuelve_igual_y_rechaza_lo_roto() {
        let m = de_prueba();
        let b = m.a_bytes();
        assert_eq!(Modelo::de_bytes(&b), Some(m));
        assert_eq!(Modelo::de_bytes(&b[..b.len() - 4]), None);
        assert_eq!(Modelo::de_bytes(b"basura"), None);
    }
}
