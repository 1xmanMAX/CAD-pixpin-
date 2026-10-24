//! Copiar y pegar elementos del lienzo.
//!
//! Dos cosas que parecen detalles y no lo son:
//!
//! - **Lo pegado estrena identidad.** Ids nuevos, claro, pero tambien nombres
//!   de grupo nuevos: si la copia conservara el grupo del original, mover uno
//!   arrastraria al otro y no habria forma de separarlos. Un grupo copiado es
//!   otro grupo.
//! - **Se pega desplazado y queda elegido**, como en Excalidraw. Pegar encima
//!   del original deja al usuario sin saber si paso algo.
//!
//! Las posiciones relativas se respetan: lo que estaba junto sigue junto.

use crate::elemento::Elemento;
use crate::escena::Escena;
use crate::seleccion::Seleccion;

/// Lo que se desplaza lo pegado respecto al original, en unidades de lienzo.
/// El mismo salto que usa duplicar.
pub const DESPLAZAMIENTO: f32 = 10.0;

/// Se lleva una copia de lo elegido, en el orden en que esta en la escena.
///
/// El orden importa: pegarlo despues tiene que reponer quien tapa a quien.
pub fn copiar(escena: &Escena, sel: &Seleccion) -> Vec<Elemento> {
    escena
        .visibles()
        .filter(|e| sel.contiene(e.id))
        .cloned()
        .collect()
}

/// Mete una copia en la escena, desplazada, y devuelve los ids nuevos para
/// poder elegirlos.
///
/// Todo en un solo paso de deshacer: pegar cinco figuras y tener que deshacer
/// cinco veces seria un castigo.
pub fn pegar(escena: &mut Escena, elementos: &[Elemento], dx: f32, dy: f32) -> Vec<u64> {
    if elementos.is_empty() {
        return Vec::new();
    }
    // Cada grupo del original necesita su equivalente en la copia, y el mismo
    // para todos los elementos que lo compartan: si no, dos figuras del mismo
    // grupo acabarian en dos grupos distintos.
    let mut equivalente: std::collections::HashMap<String, String> = Default::default();

    escena.abrir_paso();
    let mut nuevos = Vec::with_capacity(elementos.len());
    for original in elementos {
        let mut copia = original.clone();
        copia.mover(dx, dy);
        let grupos: Vec<String> = copia
            .grupos
            .iter()
            .map(|g| match equivalente.get(g) {
                Some(nuevo) => nuevo.clone(),
                None => {
                    // El mismo criterio que `organizar::agrupar`: el contador
                    // de la escena, que ya garantiza que no se repite, con
                    // prefijo para no confundirlo con un id al mirar el JSON.
                    let nuevo = format!("g{}", escena.siguiente_id);
                    escena.siguiente_id += 1;
                    equivalente.insert(g.clone(), nuevo.clone());
                    nuevo
                }
            })
            .collect();
        copia.grupos = grupos;
        nuevos.push(escena.anadir(copia));
    }
    escena.cerrar_paso();
    nuevos
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};

    fn rect(x: f32, y: f32) -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho: 20.0,
            alto: 10.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    fn escena_con(cuantos: usize) -> (Escena, Vec<u64>) {
        let mut e = Escena::nueva();
        let ids = (0..cuantos)
            .map(|i| e.anadir(rect(i as f32 * 100.0, 0.0)))
            .collect();
        (e, ids)
    }

    #[test]
    fn lo_pegado_conserva_las_distancias_y_queda_desplazado() {
        let (mut escena, ids) = escena_con(2);
        let mut sel = Seleccion::default();
        sel.poner_todos(ids.clone());
        let copiado = copiar(&escena, &sel);
        assert_eq!(copiado.len(), 2);

        let nuevos = pegar(&mut escena, &copiado, DESPLAZAMIENTO, DESPLAZAMIENTO);
        assert_eq!(nuevos.len(), 2);
        assert_eq!(escena.visibles().count(), 4, "los originales siguen ahi");

        let a = escena.buscar(nuevos[0]).unwrap();
        let b = escena.buscar(nuevos[1]).unwrap();
        assert_eq!((a.x, a.y), (DESPLAZAMIENTO, DESPLAZAMIENTO));
        // Lo que estaba a cien de distancia sigue a cien.
        assert_eq!(b.x - a.x, 100.0);
        // Y son otros elementos, no los mismos.
        assert!(!ids.contains(&nuevos[0]));
    }

    #[test]
    fn un_grupo_copiado_es_otro_grupo() {
        let (mut escena, ids) = escena_con(2);
        let mut sel = Seleccion::default();
        sel.poner_todos(ids.clone());
        let grupo = crate::organizar::agrupar(&mut escena, &sel).expect("dos elementos se agrupan");

        let copiado = copiar(&escena, &sel);
        let nuevos = pegar(&mut escena, &copiado, 10.0, 10.0);

        let g0 = escena.buscar(nuevos[0]).unwrap().grupos.clone();
        let g1 = escena.buscar(nuevos[1]).unwrap().grupos.clone();
        // Las dos copias comparten grupo entre ellas...
        assert_eq!(g0, g1);
        assert_eq!(g0.len(), 1);
        // ...pero NO el del original: si no, mover una arrastraria la otra y
        // no habria forma de separarlas.
        assert_ne!(g0[0], grupo);
    }

    #[test]
    fn pegar_es_un_solo_paso_de_deshacer() {
        let (mut escena, ids) = escena_con(3);
        let mut sel = Seleccion::default();
        sel.poner_todos(ids);
        let copiado = copiar(&escena, &sel);
        pegar(&mut escena, &copiado, 10.0, 10.0);
        assert_eq!(escena.visibles().count(), 6);
        // Una sola vez, no tres: pegar tres figuras y deshacer tres veces
        // seria un castigo.
        escena.deshacer();
        assert_eq!(escena.visibles().count(), 3);
    }

    #[test]
    fn sin_nada_elegido_no_se_copia_ni_se_pega_nada() {
        let (mut escena, _) = escena_con(2);
        let vacia = Seleccion::default();
        assert!(copiar(&escena, &vacia).is_empty());
        // Y pegar nada no abre un paso de deshacer vacio, que dejaria al
        // usuario pulsando Ctrl+Z sin que pase nada.
        assert!(pegar(&mut escena, &[], 10.0, 10.0).is_empty());
        assert_eq!(escena.visibles().count(), 2);
        escena.deshacer();
        assert_eq!(escena.visibles().count(), 1, "deshizo el ultimo anadir");
    }
}
