//! **Anotar encima del plano** con el motor del lienzo (como PixPin Android
//! v0.114: «anotar en el visor de planos usa las herramientas del motor del
//! lienzo, sin herramientas propias»; y lo que el usuario recordo el
//! 9-oct-2026: el motor de edicion tiene su desarrollo para la maxima
//! fluidez, asi que se integra el que hay, no se hace otra tinta).
//!
//! El visor no sabe nada del motor: le pasa los eventos de su ventana y le
//! deja pintar encima de cada fotograma, en el mismo dispositivo y la misma
//! pasada (asi la tinta no se separa del plano al mover o acercar). Quien lo
//! implementa es la aplicacion (`plano_cad::anotar`), con el anfitrion del
//! lector (`lector_tinta::Tinta`) y la capa `anot-<uid>`.
//!
//! **Las coordenadas de la capa** son las de Android (`CapaDelPlano.kt`,
//! `EscalaDeLaCapa`): la y baja y una unidad es **la vigesima parte de la
//! letra tipica del plano** ([`unidad_de_la_capa`]). Antes era la milesima
//! del lado mayor del plano, y el usuario lo vio (12-oct-2026: «cuando pinto
//! con el lapiz en el archivo CAD esta desproporcionado»): el motor tiene
//! medidas fijas en unidades del dibujo pensadas para una unidad que mide
//! mas o menos un pixel (el radio minimo del trazo, el tramo final que se
//! toma por ruido, lo que se asienta por ir deprisa...), y con una unidad de
//! decimetros o metros, al acercarse a anotar cada unidad media cientos de
//! pixeles: el trazo salia gordo y deformado. Con la de la letra, mirando el
//! plano a un tamano en que se leen sus textos, una unidad es mas o menos un
//! pixel, como en un lienzo. Android lo arreglo asi en su v0.115.
//!
//! Lo que dice donde cae cada fichero en el plano es su **`.hoja`** (el
//! marco de la tinta: la caja del plano en unidades de esa capa, ver
//! [`Encaje`]): asi la capa cae en su sitio en cualquier aparato aunque cada
//! uno use otra unidad, como la de antes en lo ya anotado.

use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};

use crate::modelo::Modelo;

/// **La unidad de antes** (y la de Android antes de su v0.115): la milesima
/// del lado mayor del plano. La lleva lo anotado antes sin `.hoja`.
pub fn unidad_antigua(m: &Modelo) -> f64 {
    let u = ((m.caja[2] - m.caja[0]).max(m.caja[3] - m.caja[1]) as f64) / 1000.0;
    if u > 0.0 && u.is_finite() { u } else { 1.0 }
}

/// **La unidad de la capa** (unidades del plano por unidad de la capa): la
/// altura de letra mas comun del plano entre 20 (`EscalaDeLaCapa.deLasLetras`
/// de Android, la misma cuenta). Cada tramo de letras dice su tamano en
/// potencias de dos (de 2^n a 2^(n+1)); se toma el medio (x1,4). Pesa cada
/// tramo por las letras que lleva. Sin textos, la quinta parte de
/// [`unidad_antigua`]. Asi el texto del motor (20) sale del tamano de los
/// textos del plano y la raya, en proporcion.
pub fn unidad_de_la_capa(m: &Modelo) -> f64 {
    let mut por_tamano: Vec<(f32, u64)> = m
        .tramos_letras
        .iter()
        .filter(|t| t.tamano > 0.0 && t.tamano.is_finite())
        .map(|t| (t.tamano, t.cuantos as u64))
        .collect();
    por_tamano.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: u64 = por_tamano.iter().map(|t| t.1).sum();
    if total > 0 {
        let mut cuenta = 0u64;
        for (tam, n) in &por_tamano {
            cuenta += n;
            if cuenta * 2 >= total {
                let u = *tam as f64 * 1.4 / 20.0;
                if u > 0.0 && u.is_finite() {
                    return u;
                }
                break;
            }
        }
    }
    unidad_antigua(m) / 5.0
}

/// **Como cae una capa en el plano** (`CapaDelPlano.Encaje` de Android):
/// `x = cero_x + x_capa * u`, `y = cero_y - y_capa * u` (la y de la capa baja).
/// Coordenadas del plano respecto a su origen (`Modelo::origen`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Encaje {
    pub u: f64,
    pub cero_x: f64,
    pub cero_y: f64,
}

impl Encaje {
    /// El de una unidad con el cero en el origen del plano (el de un plano
    /// sin `.hoja`).
    pub fn de_unidad(u: f64) -> Encaje {
        Encaje {
            u,
            cero_x: 0.0,
            cero_y: 0.0,
        }
    }

    pub fn a_capa(&self, p: [f64; 2]) -> [f64; 2] {
        [
            (p[0] - self.cero_x) / self.u,
            -(p[1] - self.cero_y) / self.u,
        ]
    }

    pub fn a_plano(&self, q: [f64; 2]) -> [f64; 2] {
        [self.cero_x + q[0] * self.u, self.cero_y - q[1] * self.u]
    }

    /// **La `.hoja` de la capa** (`CapaDelPlano.hojaDe`): la caja del plano
    /// en unidades de la capa, `[x0, y0, x1, y1]` con `y0` la y de arriba
    /// (la maxima del plano).
    pub fn hoja(&self, caja: [f32; 4]) -> [f64; 4] {
        let a = self.a_capa([caja[0] as f64, caja[3] as f64]);
        let b = self.a_capa([caja[2] as f64, caja[1] as f64]);
        [a[0], a[1], b[0], b[1]]
    }

    /// Al reves (`CapaDelPlano.encajeDe`): el encaje que lleva la `hoja`
    /// (escrita aqui o en el movil) a la caja del plano que se ve aqui.
    /// `None` si la hoja no sirve (sin area, numeros imposibles).
    pub fn de_la_hoja(hoja: [f64; 4], caja: [f32; 4]) -> Option<Encaje> {
        let [x0, y0, x1, y1] = hoja;
        if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) || x1 - x0 <= 1e-3 || y1 - y0 <= 1e-3 {
            return None;
        }
        let ancho = (caja[2] - caja[0]) as f64;
        let alto = (caja[3] - caja[1]) as f64;
        let u = if ancho > 1e-12 {
            ancho / (x1 - x0)
        } else if alto > 1e-12 {
            alto / (y1 - y0)
        } else {
            return None;
        };
        if !(u > 0.0 && u.is_finite()) {
            return None;
        }
        Some(Encaje {
            u,
            cero_x: caja[0] as f64 - x0 * u,
            cero_y: caja[3] as f64 + y0 * u,
        })
    }
}

/// El grupo de las cotas guardadas en la capa (`CapaDelPlano.GRUPO_COTA`).
pub const GRUPO_COTA: &str = "plano-cota";
/// El de los marcos para imprimir (`GRUPO_MARCO`).
pub const GRUPO_MARCO: &str = "plano-marco";

/// Un numero con cuatro decimales como lo escribe Kotlin
/// (`String.format(Locale.ROOT, "%.4f")`: la mitad hacia arriba, lejos del cero).
fn cuatro_decimales(v: f64) -> String {
    let t = v.abs() * 10_000.0;
    if t.fract() == 0.5 {
        // Un empate exacto: Rust lo llevaria al par, Java hacia arriba.
        let r = t.ceil() / 10_000.0;
        return format!("{}{r:.4}", if v < 0.0 { "-" } else { "" });
    }
    format!("{v:.4}")
}

/// **El id de una cota o un marco** (`CapaDelPlano.huella`): sale de su
/// forma (FNV-1a de las coordenadas con cuatro decimales), asi que dos
/// aparatos que hacen la misma cota no la duplican.
pub fn huella(prefijo: &str, numeros: &[f64]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for n in numeros {
        for b in format!("{};", cuatro_decimales(*n)).bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{prefijo}-{h:x}")
}

/// Del lienzo de la capa (y baja) al plano (y sube), y al reves.
pub fn de_la_capa(p: [f64; 2], u: f64) -> [f64; 2] {
    [p[0] * u, -p[1] * u]
}

pub fn a_la_capa(p: [f64; 2], u: f64) -> [f64; 2] {
    [p[0] / u, -p[1] / u]
}

/// Lo que ve la ventana del plano, para el motor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VistaPlano {
    /// El punto del plano (respecto a su origen) en el centro de la ventana.
    pub centro: [f64; 2],
    /// Unidades del plano por pixel.
    pub px: f64,
    pub ancho: u32,
    pub alto: u32,
    /// Donde empieza el area de la ventana en el escritorio (los eventos van
    /// en coordenadas del escritorio, como los de las ventanas del motor).
    pub origen_pantalla: (i32, i32),
    /// 100 a 100 %, 150 a 150 %...
    pub escala_por_cien: u32,
    /// [`unidad_de_la_capa`].
    pub unidad: f64,
    pub claro: bool,
}

impl VistaPlano {
    /// La camara del motor: el punto de la capa en la esquina de arriba a
    /// la izquierda y los pixeles por unidad de la capa.
    pub fn camara(&self) -> (f64, f64, f64) {
        let x = (self.centro[0] - self.ancho as f64 / 2.0 * self.px) / self.unidad;
        let y = -(self.centro[1] + self.alto as f64 / 2.0 * self.px) / self.unidad;
        (x, y, self.unidad / self.px)
    }
}

/// Un evento de la ventana del plano, en coordenadas del escritorio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventoPlano {
    Mover(i32, i32),
    Bajar(i32, i32),
    Subir(i32, i32),
    BajarCentral(i32, i32),
    SubirCentral(i32, i32),
    Rueda(i32),
    Tecla {
        vk: u32,
        shift: bool,
        ctrl: bool,
        alt: bool,
    },
    TeclaSoltada(u32),
    Caracter(char),
}

/// Lo que dice el motor de un evento.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Respuesta {
    /// Era suyo: el visor no hace nada mas con el.
    pub consumido: bool,
    /// Se pulso «Salir» en su barra: dejar de anotar.
    pub salir: bool,
    /// Empezo un arrastre: capturar el raton.
    pub arrastra: bool,
}

/// **El motor del lienzo, visto desde el visor.**
pub trait Anotador {
    /// Un evento mientras se anota.
    fn evento(&mut self, ev: EventoPlano, vista: &VistaPlano) -> Respuesta;
    /// Pinta lo anotado (siempre) y, si `anotando`, la barra y el panel del
    /// motor, encima del fotograma ya hecho (`destino`, el bufer de la
    /// ventana) y antes de presentarlo.
    fn pintar(
        &mut self,
        dispositivo: &ID3D11Device,
        destino: &ID3D11Texture2D,
        vista: &VistaPlano,
        anotando: bool,
    );
    /// Si lo ultimo pintado no quedo nitido (lo anotado iba en su foto
    /// estirada mientras se movia la camara, o con trazos aun sin rehacer a
    /// la escala nueva): el visor pinta otro fotograma aunque no cambie nada.
    fn falta_pintar(&self) -> bool {
        false
    }
    /// Dejar de anotar: cerrar lo que se escribia y guardar la capa.
    fn terminar(&mut self);
    /// Si quiere el Esc (cerrar un texto, soltar lo elegido) antes que el visor.
    fn quiere_escape(&self) -> bool;
    /// **Las cotas y los marcos para imprimir guardados en la capa** (como
    /// Android: viajan con ella, ver `CapaDelPlano.kt`), en coordenadas del
    /// plano respecto a su origen: cada cota, sus puntos; cada marco,
    /// `[x0, y0, x1, y1]`.
    fn cotas_y_marcos(&self) -> (Vec<Vec<[f64; 2]>>, Vec<[f64; 4]>) {
        (Vec::new(), Vec::new())
    }
    /// Guarda en la capa estas cotas y estos marcos (los de ahora en el
    /// visor): lo que ya estaba igual se queda, lo nuevo entra y lo que ya
    /// no esta se marca borrado.
    fn poner_cotas_y_marcos(&mut self, _cotas: &[Vec<[f64; 2]>], _marcos: &[[f64; 4]]) {}
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_capa_y_el_plano_van_y_vuelven() {
        let u = 0.25;
        let p = [12.0, -7.5];
        let q = a_la_capa(p, u);
        // En la capa la y baja.
        assert_eq!(q, [48.0, 30.0]);
        assert_eq!(de_la_capa(q, u), p);
    }

    fn tramo(tamano: f32, cuantos: u32) -> crate::modelo::Tramo {
        crate::modelo::Tramo {
            desde: 0,
            cuantos,
            caja: [0.0; 4],
            tamano,
            clase: 0,
        }
    }

    #[test]
    fn la_unidad_antigua_es_la_milesima_del_lado_mayor() {
        let m = Modelo {
            caja: [0.0, 0.0, 500.0, 2000.0],
            ..Default::default()
        };
        assert!((unidad_antigua(&m) - 2.0).abs() < 1e-9);
        // Caso negativo: un plano sin caja no da una unidad absurda.
        assert_eq!(unidad_antigua(&Modelo::default()), 1.0);
    }

    #[test]
    fn la_unidad_sale_de_la_letra_mas_comun_como_en_android() {
        // 30 letras de 2,5 (tramo de 2 a 4: se toma 2 * 1,4) y 10 de 64.
        let m = Modelo {
            caja: [0.0, 0.0, 100_000.0, 50_000.0],
            tramos_letras: vec![tramo(64.0, 10), tramo(2.0, 30)],
            ..Default::default()
        };
        assert!((unidad_de_la_capa(&m) - 2.0 * 1.4 / 20.0).abs() < 1e-9);
        // Mitad y mitad: la mediana es la del tramo pequeno (como `cuenta * 2 >= total`).
        let m2 = Modelo {
            tramos_letras: vec![tramo(64.0, 10), tramo(2.0, 10)],
            ..m.clone()
        };
        assert!((unidad_de_la_capa(&m2) - 2.0 * 1.4 / 20.0).abs() < 1e-9);
        // Caso negativo: sin letras (o con tamanos imposibles), la quinta parte de la antigua.
        let sin = Modelo {
            tramos_letras: vec![tramo(f32::NAN, 5), tramo(0.0, 3)],
            ..m
        };
        assert!((unidad_de_la_capa(&sin) - 100.0 / 5.0).abs() < 1e-9);
    }

    #[test]
    fn la_hoja_lleva_la_capa_a_la_caja_del_plano_aunque_cambie_la_unidad() {
        let caja = [-300.0, -120.0, 500.0, 80.0];
        let e = Encaje {
            u: 0.35,
            cero_x: 12.0,
            cero_y: -7.0,
        };
        let hoja = e.hoja(caja);
        // y0 es la de arriba del plano: en la capa, la menor.
        assert!(hoja[1] < hoja[3]);
        let de_vuelta = Encaje::de_la_hoja(hoja, caja).unwrap();
        let p = [123.0, -45.0];
        let q = e.a_capa(p);
        let r = de_vuelta.a_plano(q);
        assert!(
            (r[0] - p[0]).abs() < 1e-9 && (r[1] - p[1]).abs() < 1e-9,
            "{r:?}"
        );
        // Sin hoja, el cero en el origen.
        assert_eq!(Encaje::de_unidad(2.0).a_plano([1.0, 1.0]), [2.0, -2.0]);
        // Casos negativos: hojas sin area, al reves o con numeros imposibles.
        assert!(Encaje::de_la_hoja([0.0, 0.0, 0.0, 10.0], caja).is_none());
        assert!(Encaje::de_la_hoja([10.0, 0.0, 0.0, 10.0], caja).is_none());
        assert!(Encaje::de_la_hoja([0.0, f64::NAN, 1.0, 10.0], caja).is_none());
        assert!(Encaje::de_la_hoja([0.0, 0.0, 1.0, 1.0], [0.0; 4]).is_none());
    }

    #[test]
    fn la_huella_es_la_de_kotlin() {
        // FNV-1a de "1.0000;2.5000;" a mano.
        let mut h: u64 = 0xcbf29ce484222325;
        for b in b"1.0000;2.5000;" {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        assert_eq!(huella(GRUPO_COTA, &[1.0, 2.5]), format!("plano-cota-{h:x}"));
        // Los empates exactos van hacia arriba, como `%.4f` de Java.
        assert_eq!(cuatro_decimales(1.03125), "1.0313");
        assert_eq!(cuatro_decimales(-1.03125), "-1.0313");
        assert_eq!(cuatro_decimales(0.1), "0.1000");
        // Caso negativo: otra forma, otro id.
        assert_ne!(
            huella(GRUPO_COTA, &[1.0, 2.5]),
            huella(GRUPO_COTA, &[1.0, 2.6])
        );
    }

    #[test]
    fn la_camara_del_motor_pone_el_mismo_punto_en_el_mismo_pixel() {
        let v = VistaPlano {
            centro: [100.0, 50.0],
            px: 0.5,
            ancho: 800,
            alto: 600,
            origen_pantalla: (0, 0),
            escala_por_cien: 100,
            unidad: 2.0,
            claro: false,
        };
        let (x, y, zoom) = v.camara();
        // Un punto del plano: su pixel por la camara del plano y por la del motor.
        let p = [130.0, 20.0];
        let px_plano = [
            (p[0] - v.centro[0]) / v.px + 400.0,
            300.0 - (p[1] - v.centro[1]) / v.px,
        ];
        let q = a_la_capa(p, v.unidad);
        let px_motor = [(q[0] - x) * zoom, (q[1] - y) * zoom];
        assert!(
            (px_plano[0] - px_motor[0]).abs() < 1e-9 && (px_plano[1] - px_motor[1]).abs() < 1e-9
        );
    }
}
