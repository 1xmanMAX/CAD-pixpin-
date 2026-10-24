//! **Ver el plano sin abrir ventanas** (solo pruebas): pinta un [`Modelo`]
//! a PNG con un rasterizador sencillo en la CPU (rayas de un pixel,
//! triangulos, arcos troceados, letras; los sombreados con patron, solo su
//! contorno en tenue). No es el pintor de la tarjeta, pero dibuja lo mismo
//! que le llega, que es lo que se quiere comprobar al leer un plano.
//!
//! `PIXPIN_CAD_PLANO=<dwg> PIXPIN_CAD_VISTA="x,y,ancho" PIXPIN_CAD_PNG=<png>
//! cargo test -p pixpin-cad --lib pintar_vista -- --ignored`

use crate::modelo::{CORTE, GLIFO_DE_RAYAS, Modelo};

pub struct Lienzo {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 4]>,
    /// Esquina de abajo a la izquierda (plano, relativo al origen) y
    /// unidades por pixel.
    x0: f64,
    y0: f64,
    upp: f64,
}

fn rgb(c: u32) -> [u8; 3] {
    if c >> 24 == 0 {
        return [0, 0, 0]; // el color 7 en fondo claro
    }
    [
        (c & 0xff) as u8,
        (c >> 8 & 0xff) as u8,
        (c >> 16 & 0xff) as u8,
    ]
}

impl Lienzo {
    /// `centro` y `ancho` en coordenadas del plano (absolutas).
    pub fn nuevo(m: &Modelo, centro: [f64; 2], ancho: f64, w: usize, h: usize) -> Lienzo {
        let upp = ancho / w as f64;
        Lienzo {
            w,
            h,
            px: vec![[255, 255, 255, 255]; w * h],
            x0: centro[0] - m.origen[0] - ancho / 2.0,
            y0: centro[1] - m.origen[1] - upp * h as f64 / 2.0,
            upp,
        }
    }

    pub fn a_pixel(&self, p: [f32; 2]) -> [f64; 2] {
        [
            (p[0] as f64 - self.x0) / self.upp,
            self.h as f64 - (p[1] as f64 - self.y0) / self.upp,
        ]
    }

    pub fn mezclar(&mut self, x: i64, y: i64, c: [u8; 3], a: f32) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return;
        }
        let p = &mut self.px[y as usize * self.w + x as usize];
        for k in 0..3 {
            p[k] = (p[k] as f32 * (1.0 - a) + c[k] as f32 * a) as u8;
        }
    }

    pub fn raya(&mut self, a: [f32; 2], b: [f32; 2], c: u32) {
        let (p, q) = (self.a_pixel(a), self.a_pixel(b));
        let n = ((q[0] - p[0]).abs().max((q[1] - p[1]).abs()).ceil() as usize).clamp(1, 20_000);
        let col = rgb(c);
        for i in 0..=n {
            let t = i as f64 / n as f64;
            self.mezclar(
                (p[0] + (q[0] - p[0]) * t) as i64,
                (p[1] + (q[1] - p[1]) * t) as i64,
                col,
                1.0,
            );
        }
    }

    pub fn triangulo(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], color: u32, alfa: f32) {
        let (p, q, r) = (self.a_pixel(a), self.a_pixel(b), self.a_pixel(c));
        let x0 = p[0].min(q[0]).min(r[0]).floor().max(0.0) as i64;
        let x1 = p[0].max(q[0]).max(r[0]).ceil().min(self.w as f64) as i64;
        let y0 = p[1].min(q[1]).min(r[1]).floor().max(0.0) as i64;
        let y1 = p[1].max(q[1]).max(r[1]).ceil().min(self.h as f64) as i64;
        let area = (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
        if area.abs() < 1e-12 {
            return;
        }
        let col = rgb(color);
        for y in y0..y1 {
            for x in x0..x1 {
                let (sx, sy) = (x as f64 + 0.5, y as f64 + 0.5);
                let e = |u: [f64; 2], v: [f64; 2]| {
                    ((v[0] - u[0]) * (sy - u[1]) - (v[1] - u[1]) * (sx - u[0])) / area
                };
                if e(p, q) >= 0.0 && e(q, r) >= 0.0 && e(r, p) >= 0.0 {
                    self.mezclar(x, y, col, alfa);
                }
            }
        }
    }

    /// Todo el modelo.
    pub fn pintar(&mut self, m: &Modelo) {
        let v = |i: u32| [m.vertices[i as usize].x, m.vertices[i as usize].y];
        // Para mirar solo las letras (en negro): PIXPIN_CAD_SOLO_LETRAS=1.
        let solo_letras = std::env::var("PIXPIN_CAD_SOLO_LETRAS").is_ok();
        if !solo_letras {
            for t in m.triangulos_trama.chunks_exact(3) {
                let p: Vec<_> = t.iter().map(|&i| m.vertices_trama[i as usize]).collect();
                self.triangulo(
                    [p[0].x, p[0].y],
                    [p[1].x, p[1].y],
                    [p[2].x, p[2].y],
                    p[0].color,
                    0.25,
                );
            }
            for t in m.triangulos.chunks_exact(3) {
                self.triangulo(
                    v(t[0]),
                    v(t[1]),
                    v(t[2]),
                    m.vertices[t[0] as usize].color,
                    1.0,
                );
            }
            for a in &m.arcos {
                let n = 96;
                for k in 0..n {
                    let t0 = a.inicio + a.barrido * k as f32 / n as f32;
                    let t1 = a.inicio + a.barrido * (k + 1) as f32 / n as f32;
                    let p = [
                        a.centro[0] + a.radio * t0.cos(),
                        a.centro[1] + a.radio * t0.sin(),
                    ];
                    let q = [
                        a.centro[0] + a.radio * t1.cos(),
                        a.centro[1] + a.radio * t1.sin(),
                    ];
                    self.raya(p, q, a.color);
                }
            }
            for w in m.lineas.windows(2) {
                if w[0] != CORTE && w[1] != CORTE {
                    self.raya(v(w[0]), v(w[1]), m.vertices[w[0] as usize].color);
                }
            }
        }
        for l in &m.letras {
            let color = if solo_letras { 0xff00_0000 } else { l.color };
            let [desde, cuantos] = m.glifos[l.glifo as usize];
            let rayas = cuantos & GLIFO_DE_RAYAS != 0;
            let n = (cuantos & !GLIFO_DE_RAYAS) as usize;
            let em = &m.malla_letras[desde as usize..desde as usize + n];
            let a_plano = |e: [f32; 2]| {
                [
                    l.pos[0] + l.m[0] * e[0] + l.m[1] * e[1],
                    l.pos[1] + l.m[2] * e[0] + l.m[3] * e[1],
                ]
            };
            if rayas {
                for s in em.chunks_exact(2) {
                    self.raya(a_plano(s[0]), a_plano(s[1]), color);
                }
            } else {
                for t in em.chunks_exact(3) {
                    // Las letras pueden venir en cualquier sentido de giro.
                    let (a, b, c) = (a_plano(t[0]), a_plano(t[1]), a_plano(t[2]));
                    self.triangulo(a, b, c, color, 1.0);
                    self.triangulo(a, c, b, color, 1.0);
                }
            }
        }
    }

    pub fn guardar(&self, ruta: &str) {
        let f = std::fs::File::create(ruta).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(f), self.w as u32, self.h as u32);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().unwrap();
        wr.write_image_data(self.px.as_flattened()).unwrap();
    }
}

/// Pinta un modelo a PNG (para las pruebas de cada arreglo).
pub fn pintar_a_png(m: &Modelo, centro: [f64; 2], ancho: f64, ruta: &str) {
    let mut l = Lienzo::nuevo(m, centro, ancho, 1000, 750);
    l.pintar(m);
    l.guardar(ruta);
}

#[test]
#[ignore = "pinta PIXPIN_CAD_PLANO en PIXPIN_CAD_VISTA a PIXPIN_CAD_PNG"]
fn pintar_vista() {
    let plano = std::env::var("PIXPIN_CAD_PLANO").unwrap();
    let (m, _) = crate::convertir::convertir_fichero(std::path::Path::new(&plano)).unwrap();
    println!("origen {:?} caja {:?}", m.origen, m.caja);
    for (i, vista) in std::env::var("PIXPIN_CAD_VISTA")
        .unwrap()
        .split(';')
        .enumerate()
    {
        let n: Vec<f64> = vista
            .split(',')
            .filter_map(|t| t.trim().parse().ok())
            .collect();
        let (centro, ancho) = if n.len() == 3 {
            ([n[0], n[1]], n[2])
        } else {
            let c = m.caja_util();
            (
                [
                    m.origen[0] + (c[0] + c[2]) as f64 / 2.0,
                    m.origen[1] + (c[1] + c[3]) as f64 / 2.0,
                ],
                (c[2] - c[0]).max(c[3] - c[1]) as f64,
            )
        };
        let ruta = std::env::var("PIXPIN_CAD_PNG")
            .unwrap()
            .replace("{}", &i.to_string());
        pintar_a_png(&m, centro, ancho, &ruta);
        println!("{ruta}: centro {centro:?} ancho {ancho}");
    }
}

/// Vuelca las entidades del espacio modelo con algun punto en la caja
/// `PIXPIN_CAD_CAJA="x0,y0,x1,y1"` (para investigar un fallo de dibujo).
#[test]
#[ignore = "vuelca las entidades de PIXPIN_CAD_PLANO en PIXPIN_CAD_CAJA"]
fn volcar_zona() {
    use opencadcodec::entities::EntityType;
    let plano = std::env::var("PIXPIN_CAD_PLANO").unwrap();
    let c: Vec<f64> = std::env::var("PIXPIN_CAD_CAJA")
        .unwrap()
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .collect();
    let dentro = |x: f64, y: f64| x >= c[0] && x <= c[2] && y >= c[1] && y <= c[3];
    let doc = crate::leer::leer(std::path::Path::new(&plano)).unwrap();
    let largo = std::env::var("PIXPIN_CAD_LARGO").map_or(600, |v| v.parse().unwrap_or(600));
    for e in doc.model_space_entities() {
        let pts: Vec<(f64, f64)> = match e {
            EntityType::Line(l) => vec![(l.start.x, l.start.y), (l.end.x, l.end.y)],
            EntityType::LwPolyline(p) => p
                .vertices
                .iter()
                .map(|v| (v.location.x, v.location.y))
                .collect(),
            EntityType::Polyline2D(p) => p
                .vertices
                .iter()
                .map(|v| (v.location.x, v.location.y))
                .collect(),
            EntityType::Solid(s) => vec![
                (s.first_corner.x, s.first_corner.y),
                (s.fourth_corner.x, s.fourth_corner.y),
            ],
            EntityType::Insert(i) => vec![(i.insert_point.x, i.insert_point.y)],
            EntityType::Text(t) => vec![(t.insertion_point.x, t.insertion_point.y)],
            EntityType::MText(t) => vec![(t.insertion_point.x, t.insertion_point.y)],
            EntityType::Hatch(h) => h
                .paths
                .iter()
                .flat_map(|p| p.edges.iter())
                .filter_map(|b| match b {
                    opencadcodec::entities::BoundaryEdge::Line(l) => Some((l.start.x, l.start.y)),
                    opencadcodec::entities::BoundaryEdge::Polyline(p) => {
                        p.vertices.first().map(|v| (v.x, v.y))
                    }
                    _ => None,
                })
                .collect(),
            EntityType::Circle(k) => vec![(k.center.x, k.center.y)],
            EntityType::Arc(k) => vec![(k.center.x, k.center.y)],
            _ => vec![],
        };
        if pts.iter().any(|p| dentro(p.0, p.1)) {
            let mut s = format!("{e:?}");
            s.truncate(largo);
            println!(
                "{} {} :: {s}\n",
                e.as_entity().entity_type(),
                e.common().layer
            );
        }
    }
}
