//! Quitar puntos que a esta distancia no se ven.
//!
//! Un trazo a mano guarda un punto por cada aviso del raton: los del fichero
//! del movil llegan a 492 en un solo trazo, y un dibujo de trabajo suma ocho
//! mil. A tamano natural hacen falta todos. Vistos al 20 %, ese trazo de 492
//! puntos ocupa un pulgar en la pantalla y veinte puntos lo dibujan igual.
//!
//! De ahi sale la unica idea de este modulo: **la tolerancia se piensa en
//! pixeles de pantalla y se aplica en el mundo**. Medio pixel es la mitad de
//! lo que el ojo distingue, asi que quitar todo lo que se aparte menos de eso
//! no se puede notar por definicion. `Camara::en_mundo` hace la conversion.
//!
//! # Esto es maquillaje, no cirugia
//!
//! Se adelgaza **la copia que se pinta**, jamas los puntos guardados del
//! elemento. Si tocara el modelo, alejarse y volver a acercarse destruiria el
//! trazo del usuario poco a poco, y ademas seria irreversible: los puntos
//! quitados no se pueden inventar de vuelta.
//!
//! El algoritmo es el de Ramer, Douglas y Peucker, de 1972: se queda con los
//! dos extremos y va reincorporando el punto que mas se sale, hasta que
//! ninguno se sale lo suficiente. Es el que conserva la forma, no el que
//! reparte puntos — que es justo lo que hace falta aqui, porque lo que se ve
//! de un trazo son sus curvas y sus picos, no su relleno.

use crate::vector::{Punto2, distancia_a_segmento};

/// Quita los puntos que se aparten de la linea menos de `tolerancia`.
///
/// La tolerancia va en unidades del mundo. Cero o menos devuelve los puntos
/// tal cual: es la forma de decir «no toques nada», y conviene que sea
/// explicita en vez de un caso raro que se cuela.
///
/// Los dos extremos se conservan siempre. Un trazo que empieza o acaba en
/// otro sitio no es el mismo trazo.
pub fn aligerar(puntos: &[Punto2], tolerancia: f32) -> Vec<Punto2> {
    // Con dos puntos no hay nada intermedio que quitar.
    if puntos.len() < 3 || tolerancia <= 0.0 {
        return puntos.to_vec();
    }

    // `guardar[i]` dice si el punto i sobrevive. Se empieza con los extremos
    // y se van rescatando los que se salen.
    let mut guardar = vec![false; puntos.len()];
    guardar[0] = true;
    guardar[puntos.len() - 1] = true;

    // Pila explicita y no recursion: un trazo largo con la forma justa haria
    // una recursion tan profunda como puntos tenga, y reventar la pila por
    // dibujar una raya seria un mal final.
    let mut pendientes = vec![(0usize, puntos.len() - 1)];
    while let Some((desde, hasta)) = pendientes.pop() {
        if hasta <= desde + 1 {
            continue;
        }
        let mut peor = 0.0_f32;
        let mut cual = desde;
        for (i, p) in puntos.iter().enumerate().take(hasta).skip(desde + 1) {
            let d = distancia_a_segmento(*p, puntos[desde], puntos[hasta]);
            if d > peor {
                peor = d;
                cual = i;
            }
        }
        if peor > tolerancia {
            guardar[cual] = true;
            pendientes.push((desde, cual));
            pendientes.push((cual, hasta));
        }
    }

    puntos
        .iter()
        .zip(guardar)
        .filter(|(_, q)| *q)
        .map(|(p, _)| *p)
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn p(x: f32, y: f32) -> Punto2 {
        Punto2::nuevo(x, y)
    }

    #[test]
    fn una_recta_con_muchos_puntos_se_queda_en_dos() {
        // El caso que mas se gana: cien puntos alineados dibujan lo mismo que
        // sus dos extremos, y cuestan cincuenta veces mas.
        let recta: Vec<Punto2> = (0..100).map(|i| p(i as f32, 0.0)).collect();
        assert_eq!(aligerar(&recta, 0.5), vec![p(0.0, 0.0), p(99.0, 0.0)]);
    }

    #[test]
    fn un_pico_no_se_pierde() {
        // Lo que separa este algoritmo de quitar uno de cada dos: un pico es
        // justo lo que se ve de un trazo, y tiene que sobrevivir.
        let con_pico = vec![p(0.0, 0.0), p(5.0, 40.0), p(10.0, 0.0)];
        assert_eq!(aligerar(&con_pico, 0.5), con_pico, "se comio el pico");
    }

    #[test]
    fn cuanto_mas_lejos_se_mira_menos_puntos_hacen_falta() {
        // Una onda de cien puntos: de cerca hacen falta casi todos, de lejos
        // un punado. Es la razon de ser del modulo.
        let onda: Vec<Punto2> = (0..100)
            .map(|i| p(i as f32, (i as f32 * 0.3).sin() * 20.0))
            .collect();
        let cerca = aligerar(&onda, 0.1).len();
        let lejos = aligerar(&onda, 8.0).len();
        assert!(
            lejos < cerca && lejos < 20,
            "de lejos deberian bastar pocos: {lejos} de {cerca}"
        );
        assert!(cerca > 40, "de cerca no se puede tirar tanto: {cerca}");
    }

    #[test]
    fn los_extremos_no_se_tocan_nunca() {
        // Un trazo que empieza o acaba en otro sitio no es el mismo trazo, y
        // en un dibujo tecnico eso descoloca lo que hubiera enganchado.
        let onda: Vec<Punto2> = (0..50)
            .map(|i| p(i as f32, (i as f32).cos() * 3.0))
            .collect();
        let flaco = aligerar(&onda, 1000.0);
        assert_eq!(flaco.len(), 2, "con tolerancia enorme quedan los extremos");
        assert_eq!(flaco[0], onda[0]);
        assert_eq!(flaco[1], onda[onda.len() - 1]);
    }

    #[test]
    fn sin_tolerancia_no_se_toca_nada() {
        // Caso negativo: cero es la forma explicita de decir «no adelgaces»,
        // y tiene que devolver los puntos identicos, no parecidos.
        let onda: Vec<Punto2> = (0..30).map(|i| p(i as f32, (i % 3) as f32)).collect();
        assert_eq!(aligerar(&onda, 0.0), onda);
        assert_eq!(aligerar(&onda, -1.0), onda);
    }

    #[test]
    fn los_trazos_cortos_pasan_de_largo() {
        // Caso negativo: sin puntos intermedios no hay nada que decidir, y no
        // debe inventarse ni perderse nada.
        assert!(aligerar(&[], 1.0).is_empty());
        assert_eq!(aligerar(&[p(1.0, 2.0)], 1.0), vec![p(1.0, 2.0)]);
        let dos = vec![p(0.0, 0.0), p(9.0, 9.0)];
        assert_eq!(aligerar(&dos, 100.0), dos);
    }

    #[test]
    fn el_resultado_conserva_el_orden_del_trazo() {
        // Un trazo es una secuencia: desordenarlo lo convierte en otra cosa.
        let onda: Vec<Punto2> = (0..60)
            .map(|i| p(i as f32, (i as f32 * 0.5).sin() * 10.0))
            .collect();
        let flaco = aligerar(&onda, 1.0);
        assert!(flaco.windows(2).all(|v| v[0].x < v[1].x), "se desordeno");
    }

    #[test]
    fn un_trazo_muy_largo_no_revienta_la_pila() {
        // Por que la pila es explicita: una escalera de diez mil puntos hace
        // que el peor punto caiga siempre en un extremo, y con recursion eso
        // baja diez mil niveles.
        let escalera: Vec<Punto2> = (0..10_000).map(|i| p(i as f32, (i % 2) as f32)).collect();
        assert!(aligerar(&escalera, 0.25).len() > 2);
    }
}
