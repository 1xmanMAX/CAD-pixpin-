//! El filtro «1 euro», reimplementado desde el articulo.
//!
//! Fuente: Gery Casiez, Nicolas Roussel, Daniel Vogel, «1 € Filter: A Simple
//! Speed-based Low-pass Filter for Noisy Input in Interactive Systems»,
//! CHI 2012. Escrito a partir de la DESCRIPCION del algoritmo, no del
//! repositorio de los autores (que no declara licencia). El algoritmo en si
//! es publico.
//!
//! Por que existe: `streamline` de perfect-freehand es un paso bajo de
//! parametro FIJO, asi que retrasa la punta lo mismo yendo despacio que
//! yendo deprisa — y es yendo deprisa donde ese retraso se siente como una
//! goma tirando del lapiz. El filtro de 1 euro sube el corte con la
//! velocidad: quieto filtra mucho (quita el temblor), corriendo casi no
//! filtra (no retrasa).

use std::f32::consts::PI;

/// Un paso bajo exponencial con el alfa dado en cada paso.
#[derive(Debug, Clone, Copy, Default)]
struct PasoBajo {
    anterior: Option<f32>,
}

impl PasoBajo {
    fn filtrar(&mut self, x: f32, alfa: f32) -> f32 {
        let y = match self.anterior {
            None => x,
            Some(p) => alfa * x + (1.0 - alfa) * p,
        };
        self.anterior = Some(y);
        y
    }
}

/// `alfa` del articulo: `1 / (1 + tau/dt)` con `tau = 1 / (2·pi·corte)`.
///
/// `dt` en segundos. Un `dt` de cero o negativo (dos muestras con la misma
/// marca de tiempo, que pasa cuando el raton manda dos puntos en el mismo
/// milisegundo) daria una division por cero: se trata como «sin tiempo», es
/// decir, alfa 1, que deja pasar la muestra tal cual.
fn alfa(corte: f32, dt: f32) -> f32 {
    if dt <= 0.0 || corte <= 0.0 {
        return 1.0;
    }
    let tau = 1.0 / (2.0 * PI * corte);
    1.0 / (1.0 + tau / dt)
}

/// Los dos mandos del filtro, con los nombres del articulo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mandos {
    /// `mincutoff`, en hercios: el corte con el puntero QUIETO. Mas bajo =
    /// menos temblor y mas retraso al arrancar.
    pub corte_minimo: f32,
    /// `beta`: cuanto sube el corte con la velocidad. Mas alto = menos
    /// retraso a alta velocidad y mas temblor.
    pub beta: f32,
    /// `dcutoff`: el corte del filtro de la propia velocidad. El articulo
    /// recomienda 1 Hz y dice que casi nunca hay que tocarlo.
    pub corte_velocidad: f32,
}

impl Default for Mandos {
    /// Conservador a proposito (D: «valor por defecto conservador»): con
    /// estos numeros el trazo se parece al de `streamline`, no al de un
    /// lapiz sin filtrar. El ajuste del TOML es el que los sube.
    fn default() -> Self {
        Self {
            corte_minimo: 1.0,
            beta: 0.007,
            corte_velocidad: 1.0,
        }
    }
}

/// El filtro de un eje.
#[derive(Debug, Clone, Default)]
struct Eje {
    x: PasoBajo,
    dx: PasoBajo,
    anterior: Option<f32>,
}

impl Eje {
    fn filtrar(&mut self, x: f32, dt: f32, m: &Mandos) -> f32 {
        // Velocidad cruda, ya suavizada con su propio paso bajo: sin eso, el
        // ruido de la posicion se convertiria en ruido del corte y el filtro
        // se volveria inestable justo cuando mas hace falta.
        let cruda = match self.anterior {
            Some(p) if dt > 0.0 => (x - p) / dt,
            _ => 0.0,
        };
        self.anterior = Some(x);
        let velocidad = self.dx.filtrar(cruda, alfa(m.corte_velocidad, dt));
        let corte = m.corte_minimo + m.beta * velocidad.abs();
        self.x.filtrar(x, alfa(corte, dt))
    }
}

/// El filtro de un punto: un eje por coordenada, con la misma velocidad
/// calculada por separado (es lo que hace el articulo para el raton).
#[derive(Debug, Clone, Default)]
pub struct FiltroUnEuro {
    mandos: Mandos,
    ejes: [Eje; 2],
    ultimo_ms: Option<f64>,
}

impl FiltroUnEuro {
    pub fn nuevo(mandos: Mandos) -> Self {
        Self {
            mandos,
            ..Default::default()
        }
    }

    /// Olvida el trazo anterior. Un trazo nuevo no hereda ni posicion ni
    /// velocidad: sin esto, el primer punto de un trazo se veria arrastrado
    /// hacia donde acabo el anterior.
    pub fn reiniciar(&mut self) {
        self.ejes = Default::default();
        self.ultimo_ms = None;
    }

    /// El punto filtrado. `t_ms` es la hora real de la muestra (reloj
    /// monotono): el filtro NO supone un ritmo fijo, porque el lapiz no lo
    /// tiene.
    pub fn filtrar(&mut self, x: f32, y: f32, t_ms: f64) -> (f32, f32) {
        let dt = match self.ultimo_ms {
            Some(t) => ((t_ms - t) / 1000.0) as f32,
            None => 0.0,
        };
        self.ultimo_ms = Some(t_ms);
        let m = self.mandos;
        (
            self.ejes[0].filtrar(x, dt, &m),
            self.ejes[1].filtrar(y, dt, &m),
        )
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Trazo recto a velocidad constante con temblor de amplitud `ruido`.
    fn recorrido(v_px_s: f32, ruido: f32, n: usize) -> Vec<(f32, f32, f64)> {
        (0..n)
            .map(|i| {
                let t = i as f64 * 4.0;
                let x = v_px_s * (t as f32) / 1000.0;
                // Temblor determinista: alterna de signo en cada muestra, que
                // es la frecuencia mas alta que puede tener una senal
                // muestreada — lo que un paso bajo tiene que quitar.
                let e = if i % 2 == 0 { ruido } else { -ruido };
                (x + e, e, t)
            })
            .collect()
    }

    /// Energia de alta frecuencia: cuanto se sale cada muestra de la recta
    /// que unen sus dos vecinas. Mide el TEMBLOR y no el retraso — un trazo
    /// entero retrasado tiene este numero a cero.
    fn temblor(v: &[f32]) -> f32 {
        if v.len() < 3 {
            return 0.0;
        }
        v.windows(3)
            .map(|w| (w[1] - (w[0] + w[2]) / 2.0).abs())
            .sum::<f32>()
            / (v.len() - 2) as f32
    }

    /// Cuantos pixeles va la salida por detras de la senal limpia.
    fn retraso(salida: &[f32], limpio: &[f32]) -> f32 {
        salida
            .iter()
            .zip(limpio)
            .map(|(s, l)| (l - s).abs())
            .sum::<f32>()
            / salida.len() as f32
    }

    /// Filtra `entrada` y devuelve solo la coordenada X.
    fn filtrar_x(f: &mut FiltroUnEuro, entrada: &[(f32, f32, f64)]) -> Vec<f32> {
        entrada
            .iter()
            .map(|&(x, y, t)| f.filtrar(x, y, t).0)
            .collect()
    }

    fn limpio(v_px_s: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| v_px_s * (i as f32 * 4.0) / 1000.0).collect()
    }

    #[test]
    fn quieto_quita_el_temblor_y_corriendo_casi_no_retrasa() {
        // El resultado que justifica el cambio: el mismo filtro, con los
        // mismos mandos, filtra mucho despacio y poco deprisa. Un paso bajo
        // de parametro fijo no puede hacer las dos cosas.
        let mandos = Mandos {
            corte_minimo: 1.0,
            beta: 0.05,
            corte_velocidad: 1.0,
        };
        let despacio = recorrido(20.0, 1.0, 80);
        let deprisa = recorrido(2000.0, 1.0, 80);
        let sal_lenta = filtrar_x(&mut FiltroUnEuro::nuevo(mandos), &despacio);
        let sal_rapida = filtrar_x(&mut FiltroUnEuro::nuevo(mandos), &deprisa);

        let entrada_lenta: Vec<f32> = despacio.iter().map(|&(x, _, _)| x).collect();
        let t_entrada = temblor(&entrada_lenta[20..]);
        let t_salida = temblor(&sal_lenta[20..]);
        assert!(
            t_salida < t_entrada * 0.1,
            "despacio tiene que quitar temblor: entro {t_entrada}, salio {t_salida}"
        );
        // Deprisa, lo que importa es el retraso en pixeles pese a ir a
        // 2.000 px/s (medio milimetro de la pantalla por milisegundo).
        let r = retraso(&sal_rapida[20..], &limpio(2000.0, 80)[20..]);
        assert!(r < 12.0, "retraso a alta velocidad: {r} px");
    }

    #[test]
    fn retrasa_menos_que_un_paso_bajo_fijo_quitando_el_mismo_temblor() {
        // La promesa del articulo, medida: a igualdad de temblor quitado, el
        // de 1 euro va mas pegado al cursor a alta velocidad que un paso
        // bajo de parametro fijo — que es lo que hace hoy `streamline`.
        let mandos = Mandos {
            corte_minimo: 1.0,
            beta: 0.05,
            corte_velocidad: 1.0,
        };
        let fijo = |a: f32, entrada: &[(f32, f32, f64)]| {
            let mut y = entrada[0].0;
            entrada
                .iter()
                .map(|&(x, _, _)| {
                    y = a * x + (1.0 - a) * y;
                    y
                })
                .collect::<Vec<f32>>()
        };

        // Comparar a IGUALDAD de temblor quitado, que es como lo enuncia el
        // articulo. Se busca el alfa del paso bajo fijo que deja el mismo
        // temblor que el de 1 euro a baja velocidad, y solo entonces se
        // comparan los retrasos. Comparar con un alfa cualquiera no diria
        // nada: cualquier filtro retrasa menos si filtra menos.
        let despacio = recorrido(20.0, 1.0, 80);
        let t_euro = temblor(&filtrar_x(&mut FiltroUnEuro::nuevo(mandos), &despacio)[20..]);
        let (mut bajo, mut alto) = (1e-4f32, 1.0f32);
        for _ in 0..40 {
            let medio = (bajo + alto) / 2.0;
            // Mas alfa = menos filtrado = mas temblor.
            if temblor(&fijo(medio, &despacio)[20..]) < t_euro {
                bajo = medio;
            } else {
                alto = medio;
            }
        }
        let a = (bajo + alto) / 2.0;
        let t_fijo = temblor(&fijo(a, &despacio)[20..]);
        assert!(
            (t_fijo - t_euro).abs() < t_euro * 0.1,
            "el alfa buscado no iguala el temblor: {t_fijo} contra {t_euro}"
        );

        // Y a esa igualdad, a alta velocidad el de 1 euro tiene que ir
        // claramente mas pegado al cursor.
        let deprisa = recorrido(2000.0, 1.0, 80);
        let l = limpio(2000.0, 80);
        let r_euro = retraso(
            &filtrar_x(&mut FiltroUnEuro::nuevo(mandos), &deprisa)[20..],
            &l[20..],
        );
        let r_fijo = retraso(&fijo(a, &deprisa)[20..], &l[20..]);
        assert!(
            r_euro < r_fijo * 0.6,
            "1 euro {r_euro} px contra paso bajo fijo de igual temblor {r_fijo} px"
        );
    }

    #[test]
    fn dos_muestras_en_el_mismo_milisegundo_no_revientan_el_filtro() {
        // Caso negativo: el raton manda dos `MOUSEMOVEPOINT` con la misma
        // marca de tiempo mas a menudo de lo que parece. Con dt = 0 el alfa
        // seria una division por cero.
        let mut f = FiltroUnEuro::nuevo(Mandos::default());
        f.filtrar(0.0, 0.0, 10.0);
        let (x, y) = f.filtrar(5.0, 5.0, 10.0);
        assert!(x.is_finite() && y.is_finite(), "{x} {y}");
        assert_eq!((x, y), (5.0, 5.0), "sin tiempo, la muestra pasa tal cual");
    }

    #[test]
    fn el_primer_punto_de_un_trazo_sale_donde_se_pulso() {
        // Si el primer punto saliera filtrado contra el trazo anterior, la
        // tinta empezaria separada del cursor: el fallo mas visible posible.
        let mut f = FiltroUnEuro::nuevo(Mandos::default());
        for i in 0..50 {
            f.filtrar(1000.0, 1000.0, i as f64 * 4.0);
        }
        f.reiniciar();
        assert_eq!(f.filtrar(3.0, 7.0, 400.0), (3.0, 7.0));
    }

    #[test]
    fn sin_reiniciar_el_primer_punto_arrastra_el_trazo_anterior() {
        // El caso negativo del de arriba: esto es lo que pasaria si alguien
        // olvidara `reiniciar`, y por eso la prueba lo fija.
        let mut f = FiltroUnEuro::nuevo(Mandos::default());
        for i in 0..50 {
            f.filtrar(1000.0, 1000.0, i as f64 * 4.0);
        }
        let (x, _) = f.filtrar(3.0, 7.0, 204.0);
        assert!(x > 100.0, "sin reiniciar tira del punto viejo, salio {x}");
    }
}
