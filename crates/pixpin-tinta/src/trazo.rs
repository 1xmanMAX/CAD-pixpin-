//! El trazo en curso: las muestras que han llegado y que parte del contorno
//! hay que rehacer en cada fotograma.
//!
//! El problema que resuelve: perfect-freehand recalcula el contorno ENTERO
//! en cada punto nuevo. A los 5.000 puntos de un trazo largo eso son 5.000
//! operaciones por fotograma, y con la punta predicha encendida, dos veces
//! (el contorno real y el de la copia con la punta). El coste por fotograma
//! crece con lo que llevas dibujado, que es justo lo contrario de lo que
//! quiere quien esta dibujando.
//!
//! La salida: trocear. La idea es de tldraw (que corta el trazo en una forma
//! nueva cada 600 puntos) pero el codigo es propio: aqui el trazo NO se
//! parte en varios elementos —eso rompe la paridad del fichero con
//! Excalidraw—, se parte solo el CONTORNO que se pinta mientras dura el
//! gesto. Al soltar, el motor 2D de siempre recibe todos los puntos y hace
//! un unico contorno, exactamente como hoy.
//!
//! Los tramos se solapan unos puntos para que la union no se vea: dos
//! manchas que comparten varios puntos se funden en una sola al rellenarse.

use crate::{Muestra, Punto};

/// Cuantas muestras de la cola entran en el contorno de la punta predicha.
///
/// Solo hacen falta las ultimas: la punta es un pegote corto en la direccion
/// del movimiento, y pasarle el trazo entero era lo que obligaba a clonar el
/// elemento y a recalcular todo el contorno una segunda vez.
pub const PUNTOS_DE_PUNTA: usize = 32;

/// El trazo en curso y su troceado.
#[derive(Debug, Default)]
pub struct TrazoVivo {
    /// Lo que se pinta (ya filtrado si el suavizado esta puesto en natural).
    puntos: Vec<Muestra>,
    /// Contornos de los tramos ya cerrados: no se vuelven a calcular.
    congelados: Vec<Vec<Punto>>,
    /// Donde empieza, dentro de `puntos`, el tramo que sigue vivo.
    inicio_cola: usize,
    /// El contorno de la cola, rehecho en cada `actualizar`.
    cola: Vec<Punto>,
    /// Cuantas veces se ha pedido el contorno de algo: lo mira la prueba que
    /// fija que congelar de verdad congela.
    calculos: usize,
    /// Cuantos puntos como mucho tiene un tramo, y cuantos comparte con el
    /// siguiente. `por_tramo` a 0 apaga el troceado.
    por_tramo: usize,
    solape: usize,
}

impl TrazoVivo {
    /// `por_tramo` a 0 deja el trazo entero en un solo contorno, que es el
    /// comportamiento de siempre y el que conserva la paridad exacta con el
    /// oraculo de Excalidraw.
    pub fn nuevo(por_tramo: usize, solape: usize) -> Self {
        Self {
            por_tramo,
            // El solape nunca puede llegar al tamano del tramo: si lo
            // hiciera, cerrar un tramo no avanzaria y el trazo se quedaria
            // dando vueltas cerrando tramos vacios.
            solape: solape.min(por_tramo.saturating_sub(1)),
            ..Default::default()
        }
    }

    /// Empieza un trazo nuevo. No hereda nada del anterior.
    pub fn empezar(&mut self) {
        self.puntos.clear();
        self.congelados.clear();
        self.cola.clear();
        self.inicio_cola = 0;
        self.calculos = 0;
    }

    /// Anade una muestra ya filtrada.
    pub fn anadir(&mut self, m: Muestra) {
        self.puntos.push(m);
    }

    pub fn puntos(&self) -> &[Muestra] {
        &self.puntos
    }

    pub fn vacio(&self) -> bool {
        self.puntos.is_empty()
    }

    /// Los contornos ya cerrados: pintarlos no cuesta CPU, ya estan hechos.
    pub fn congelados(&self) -> &[Vec<Punto>] {
        &self.congelados
    }

    /// El contorno de la cola, el unico que cambia de un fotograma al
    /// siguiente.
    pub fn cola(&self) -> &[Punto] {
        &self.cola
    }

    /// Cuantos contornos se han calculado desde `empezar`.
    pub fn calculos(&self) -> usize {
        self.calculos
    }

    /// Las muestras que hacen falta para dibujar la punta predicha: las
    /// ultimas de la cola mas `q`. No clona el elemento ni recalcula el
    /// contorno entero — es lo que costaba 2x O(n) por fotograma.
    pub fn cola_con_punta(&self, q: Punto) -> Vec<Muestra> {
        let desde = self.puntos.len().saturating_sub(PUNTOS_DE_PUNTA);
        let mut v: Vec<Muestra> = self.puntos[desde..].to_vec();
        if let Some(u) = v.last().copied() {
            v.push(Muestra {
                x: q.x,
                y: q.y,
                ..u
            });
        }
        v
    }

    /// Rehace lo que haga falta y nada mas: cierra tramos si toca y
    /// recalcula SOLO la cola. `contorno(puntos, ultimo)` devuelve el
    /// contorno cerrado de un tramo; `ultimo` dice si ese tramo termina el
    /// trazo (la tapa del final de perfect-freehand).
    ///
    /// `fin` a `true` es el fotograma en que se suelta.
    pub fn actualizar(
        &mut self,
        fin: bool,
        mut contorno: impl FnMut(&[Muestra], bool) -> Vec<Punto>,
    ) {
        if self.por_tramo > 0 {
            while self.puntos.len() - self.inicio_cola > self.por_tramo {
                let corte = self.inicio_cola + self.por_tramo;
                // Un tramo cerrado NUNCA es el final del trazo: su tapa
                // tiene que quedar dentro de la mancha del siguiente, que
                // empieza `solape` puntos antes.
                let c = contorno(&self.puntos[self.inicio_cola..corte], false);
                self.calculos += 1;
                self.congelados.push(c);
                self.inicio_cola = corte - self.solape;
            }
        }
        self.cola = contorno(&self.puntos[self.inicio_cola..], fin);
        self.calculos += 1;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un contorno de mentira: devuelve un punto por muestra. Lo que importa
    /// de estas pruebas es CUANTAS muestras se recorren, no la forma.
    fn falso(puntos: &[Muestra], _ultimo: bool) -> Vec<Punto> {
        puntos.iter().map(|m| Punto { x: m.x, y: m.y }).collect()
    }

    fn muestra(i: usize) -> Muestra {
        Muestra {
            x: i as f32,
            y: 0.0,
            presion: None,
            t_ms: i as f64 * 4.0,
        }
    }

    /// Un trazo de `n` puntos, actualizado en cada punto como hace el bucle
    /// del editor. Devuelve el trazo y el mayor numero de muestras que se
    /// recorrieron en una sola actualizacion.
    fn dibujar(n: usize, por_tramo: usize, solape: usize) -> (TrazoVivo, usize) {
        let mut t = TrazoVivo::nuevo(por_tramo, solape);
        t.empezar();
        let mut peor = 0usize;
        for i in 0..n {
            t.anadir(muestra(i));
            let mut esta = 0usize;
            t.actualizar(false, |p, u| {
                esta += p.len();
                falso(p, u)
            });
            peor = peor.max(esta);
        }
        (t, peor)
    }

    #[test]
    fn el_coste_por_fotograma_deja_de_crecer_con_el_largo_del_trazo() {
        // La razon de ser del troceado. Con 5.000 puntos y tramos de 600, un
        // fotograma nunca recorre mas de 600 + el tramo que se cierra.
        let (_, peor) = dibujar(5_000, 600, 8);
        assert!(peor <= 1_300, "un fotograma recorrio {peor} muestras");
        // Caso negativo: sin trocear, el peor fotograma recorre el trazo
        // entero. Es exactamente lo que se viene a arreglar.
        let (_, peor_sin) = dibujar(5_000, 0, 0);
        assert_eq!(peor_sin, 5_000);
    }

    #[test]
    fn un_tramo_congelado_no_se_vuelve_a_calcular() {
        let (t, _) = dibujar(2_000, 600, 8);
        // 2.000 puntos con tramos de 600 y solape de 8: tres cierres.
        assert_eq!(t.congelados().len(), 3);
        // Un calculo por punto (la cola) mas uno por tramo cerrado. Si los
        // congelados se rehicieran, este numero creceria con n².
        assert_eq!(t.calculos(), 2_000 + 3);
    }

    #[test]
    fn los_tramos_se_solapan_para_que_la_union_no_se_vea() {
        let mut t = TrazoVivo::nuevo(10, 3);
        t.empezar();
        for i in 0..12 {
            t.anadir(muestra(i));
        }
        t.actualizar(false, falso);
        assert_eq!(t.congelados().len(), 1);
        let cerrado = &t.congelados()[0];
        // El tramo cerrado llega al punto 9 y la cola empieza en el 7: tres
        // puntos compartidos.
        assert_eq!(cerrado.last().unwrap().x, 9.0);
        assert_eq!(t.cola().first().unwrap().x, 7.0);
    }

    #[test]
    fn un_trazo_corto_sale_de_una_pieza_y_conserva_la_paridad() {
        // Mientras el trazo no pase del tamano del tramo, el contorno es
        // exactamente el de siempre: un solo calculo con todos los puntos.
        // Es lo que hace que trocear no cambie el aspecto de los trazos
        // normales ni rompa el oraculo de Excalidraw.
        let (t, _) = dibujar(300, 600, 8);
        assert!(t.congelados().is_empty());
        assert_eq!(t.cola().len(), 300);
    }

    #[test]
    fn un_solape_mayor_que_el_tramo_no_deja_el_troceado_dando_vueltas() {
        // Caso negativo: con solape >= tramo, cerrar un tramo no avanzaria
        // el inicio de la cola y el bucle no terminaria nunca.
        let (t, _) = dibujar(100, 10, 50);
        assert!(t.congelados().len() < 100, "el troceado avanza");
        assert!(!t.cola().is_empty());
    }

    #[test]
    fn la_punta_predicha_solo_mira_la_cola() {
        let (t, _) = dibujar(5_000, 600, 8);
        let v = t.cola_con_punta(Punto { x: 9_999.0, y: 1.0 });
        assert_eq!(v.len(), PUNTOS_DE_PUNTA + 1);
        assert_eq!(v.last().unwrap().x, 9_999.0);
        // La presion del punto predicho es la de la ultima muestra real: una
        // punta que naciera con presion cero se veria como un pico afilado.
        assert_eq!(v.last().unwrap().presion, v[v.len() - 2].presion);
    }

    #[test]
    fn un_trazo_sin_puntos_no_inventa_una_punta() {
        let t = TrazoVivo::nuevo(600, 8);
        assert!(t.cola_con_punta(Punto { x: 1.0, y: 1.0 }).is_empty());
    }
}
