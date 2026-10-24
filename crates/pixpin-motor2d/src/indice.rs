//! Que elementos pueden estar en una zona, sin recorrerlos todos.
//!
//! `elemento_en` y `recortar` recorren la lista entera. Con ocho mil
//! elementos, mover el raton por encima cuesta ocho mil pruebas de impacto
//! por cada aviso del raton.
//!
//! # Por que una rejilla y no un arbol (D27)
//!
//! Un quadtree o un R-tree serian mas elegantes y escalarian mejor a
//! millones. Pero aqui hablamos de ocho mil, y a esa escala la rejilla ya
//! gana por goleada. Ademas:
//!
//! - **No reasigna al consultar.** Un arbol recorre nodos y va acumulando;
//!   la rejilla lee celdas contiguas.
//! - **Se demuestra correcta contra la fuerza bruta**, que es justo lo que
//!   hace la prueba principal. Un arbol mal equilibrado da resultados casi
//!   correctos, que es la peor clase de fallo.
//!
//! # La regla que no se puede romper
//!
//! La rejilla puede devolver **de mas** —quien la usa filtra despues— pero
//! jamas **de menos**. Un elemento que no salga de aqui es un elemento que
//! desaparece de la pantalla sin ningun error visible.

use std::collections::{HashMap, HashSet};

use crate::elemento::Elemento;
use crate::escena::Escena;
use crate::impacto::esquinas_giradas;

/// Lado de la celda, en unidades del mundo.
///
/// 256 sale de que un elemento tipico de trabajo mide entre 20 y 300: con
/// celdas mucho mas pequenas, cada elemento se apunta en decenas de ellas;
/// con celdas mucho mas grandes, cada consulta devuelve medio dibujo.
pub const CELDA: f32 = 256.0;

/// La caja con la que se indexa un elemento: la envolvente de sus esquinas
/// YA giradas, no `e.caja()` a secas.
///
/// `e.caja()` es la caja sin girar. Un cuadrado de 100 girado 45 grados mide
/// 141 en diagonal, asi que indexar con la caja sin girar deja fuera de la
/// celda un trozo real del elemento: si ese trozo cae en el borde de una
/// consulta, el elemento desaparece de la pantalla sin ningun error visible,
/// que es justo lo que promete no pasar el comentario de `candidatos()`.
///
/// Con `angulo == 0.0` la guarda evita el trabajo de girar cuatro puntos por
/// nada: devuelve exactamente `e.caja()`, asi que no cambia nada de lo que
/// ya funcionaba sin giro.
fn caja_indexable(e: &Elemento) -> (f32, f32, f32, f32) {
    if e.angulo == 0.0 {
        return e.caja();
    }
    let esquinas = esquinas_giradas(e);
    let xs = esquinas.iter().map(|p| p.x);
    let ys = esquinas.iter().map(|p| p.y);
    (
        xs.clone().fold(f32::MAX, f32::min),
        ys.clone().fold(f32::MAX, f32::min),
        xs.fold(f32::MIN, f32::max),
        ys.fold(f32::MIN, f32::max),
    )
}

#[derive(Debug, Clone)]
pub struct Rejilla {
    lado: f32,
    celdas: HashMap<(i32, i32), Vec<u64>>,
    /// La `version` con la que se apunto cada elemento, para saber cual ha
    /// cambiado sin comparar elementos enteros.
    versiones: HashMap<u64, u32>,
    /// El orden de la escena la ultima vez que se sincronizo, y el sitio de
    /// cada id en el. Es lo que deja devolver los candidatos **en orden de
    /// pintado**: las celdas guardan los ids en el orden en que se apuntaron
    /// (el de su ultima edicion), asi que sin esto «Traer al frente» y
    /// «Enviar al fondo» cambiaban la escena y la pantalla seguia igual.
    orden: Vec<u64>,
    posicion: HashMap<u64, usize>,
    /// Las celdas (x0, y0, x1, y1) en que se apunto cada id. Sin esto,
    /// quitar un elemento recorria TODAS las celdas de la rejilla (un
    /// `retain` sobre el mapa entero) por cada elemento que cambiaba: al
    /// soltar un trazo o deshacer, el coste crecia con todo lo dibujado y no
    /// con lo que cambio.
    ocupadas: HashMap<u64, (i32, i32, i32, i32)>,
}

impl Default for Rejilla {
    fn default() -> Self {
        Self::con_celda(CELDA)
    }
}

impl Rejilla {
    pub fn nueva() -> Self {
        Self::default()
    }

    pub fn con_celda(lado: f32) -> Self {
        Self {
            lado: lado.max(1.0),
            celdas: HashMap::new(),
            versiones: HashMap::new(),
            orden: Vec::new(),
            posicion: HashMap::new(),
            ocupadas: HashMap::new(),
        }
    }

    fn celda_de(&self, x: f32, y: f32) -> (i32, i32) {
        (
            (x / self.lado).floor() as i32,
            (y / self.lado).floor() as i32,
        )
    }

    /// Pone la rejilla al dia con la escena.
    ///
    /// Se llama en cada fotograma, asi que **no reconstruye**: mira la
    /// `version` de cada elemento y solo reapunta los que han cambiado. Si
    /// reconstruyera, seria mas cara que la fuerza bruta que viene a
    /// evitar.
    pub fn sincronizar(&mut self, escena: &Escena) {
        // El orden solo se rehace si cambio: compararlo es un recorrido sin
        // reservar nada, y lo normal (dibujar, mover) es que no cambie.
        let mismo_orden = self.orden.len() == escena.elementos.len()
            && self
                .orden
                .iter()
                .zip(&escena.elementos)
                .all(|(id, e)| *id == e.id);
        if !mismo_orden {
            self.orden.clear();
            self.orden.extend(escena.elementos.iter().map(|e| e.id));
            self.posicion.clear();
            self.posicion
                .extend(self.orden.iter().enumerate().map(|(i, id)| (*id, i)));
        }
        for e in &escena.elementos {
            let cambio = match self.versiones.get(&e.id) {
                Some(v) => *v != e.version,
                None => true,
            };
            if !cambio {
                continue;
            }
            self.quitar(e.id);
            if !e.borrado {
                self.meter(e.id, caja_indexable(e));
            }
            self.versiones.insert(e.id, e.version);
        }

        // Los que ya no estan en la escena (compactar los saco de verdad).
        //
        // Todo id de la escena acaba de quedar en `versiones`, asi que solo
        // puede sobrar alguno si `versiones` tiene MAS entradas que la
        // escena. Mirarlo antes ahorra un `HashSet` del tamano de la escena
        // en cada fotograma, que es lo que se pagaba aunque no sobrara nada.
        if self.versiones.len() <= escena.elementos.len() {
            return;
        }
        let vistos: HashSet<u64> = escena.elementos.iter().map(|e| e.id).collect();
        let sobrantes: Vec<u64> = self
            .versiones
            .keys()
            .copied()
            .filter(|id| !vistos.contains(id))
            .collect();
        for id in sobrantes {
            self.quitar(id);
            self.versiones.remove(&id);
        }
    }

    fn meter(&mut self, id: u64, caja: (f32, f32, f32, f32)) {
        let (x0, y0, x1, y1) = caja;
        let (cx0, cy0) = self.celda_de(x0, y0);
        let (cx1, cy1) = self.celda_de(x1, y1);
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                self.celdas.entry((cx, cy)).or_default().push(id);
            }
        }
        self.ocupadas.insert(id, (cx0, cy0, cx1, cy1));
    }

    fn quitar(&mut self, id: u64) {
        // Solo las celdas en que se apunto: son unas pocas, y no dependen de
        // cuanto haya dibujado en el resto del papel.
        let Some((cx0, cy0, cx1, cy1)) = self.ocupadas.remove(&id) else {
            return;
        };
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                if let Some(ids) = self.celdas.get_mut(&(cx, cy)) {
                    ids.retain(|x| *x != id);
                    if ids.is_empty() {
                        self.celdas.remove(&(cx, cy));
                    }
                }
            }
        }
    }

    /// Donde estaba `id` en la lista de la escena la ultima vez que se
    /// sincronizo. Quien pinta lo usa para ir derecho al elemento en vez de
    /// buscarlo recorriendo la escena (`Escena::buscar` es lineal, y por
    /// cada candidato de cada fotograma eso es cuadratico en lo visible).
    pub fn posicion(&self, id: u64) -> Option<usize> {
        self.posicion.get(&id).copied()
    }

    /// Los que **pueden** estar en la caja. Puede devolver de mas, nunca de
    /// menos: quien lo use filtra con `toca` o con `dentro_de`.
    pub fn candidatos(&self, caja: (f32, f32, f32, f32)) -> Vec<u64> {
        let (x0, y0, x1, y1) = caja;
        let (x0, x1) = (x0.min(x1), x0.max(x1));
        let (y0, y1) = (y0.min(y1), y0.max(y1));
        let (cx0, cy0) = self.celda_de(x0, y0);
        let (cx1, cy1) = self.celda_de(x1, y1);

        let mut fuera: Vec<u64> = Vec::new();
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                if let Some(ids) = self.celdas.get(&(cx, cy)) {
                    fuera.extend_from_slice(ids);
                }
            }
        }
        // En el orden de la escena, que es el de pintado: quien pinta los
        // recorre tal cual, y el que va encima tiene que salir despues. Un id
        // que la rejilla no conoce (no deberia haberlo) va al final, por id.
        //
        // Los repetidos (un elemento que cruza varias celdas) se quitan
        // DESPUES de ordenar, cuando ya estan juntos. Antes se miraba con
        // `contains` antes de meter cada uno, y eso es cuadratico en lo que
        // se ve: con cientos de trazos en pantalla era la parte mas cara de
        // decidir que pintar.
        fuera
            .sort_unstable_by_key(|id| (self.posicion.get(id).copied().unwrap_or(usize::MAX), *id));
        fuera.dedup();
        fuera
    }

    /// Cuantas celdas ocupadas hay. Para las pruebas.
    pub fn cuantas_celdas(&self) -> usize {
        self.celdas.len()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::azar::Azar;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};

    fn rect(id: u64, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho,
            alto,
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

    /// Lo mismo que hace la rejilla, pero recorriendo todo. Es la verdad
    /// contra la que se compara.
    fn a_lo_bruto(escena: &Escena, caja: (f32, f32, f32, f32)) -> Vec<u64> {
        let (bx0, by0, bx1, by1) = caja;
        let mut fuera: Vec<u64> = escena
            .elementos
            .iter()
            .filter(|e| !e.borrado)
            .filter(|e| {
                let (x0, y0, x1, y1) = e.caja();
                x1 >= bx0 && x0 <= bx1 && y1 >= by0 && y0 <= by1
            })
            .map(|e| e.id)
            .collect();
        fuera.sort_unstable();
        fuera
    }

    #[test]
    fn la_rejilla_dice_lo_mismo_que_la_fuerza_bruta() {
        // La prueba que justifica la rejilla entera. Escenas al azar pero
        // reproducibles: Azar es el Lehmer que ya usa el motor para que un
        // dibujo tenga el mismo garabato en cada apertura.
        let mut azar = Azar::nuevo(20260906);
        let mut escena = Escena::nueva();
        for i in 0..800 {
            let x = azar.siguiente() * 4000.0 - 2000.0;
            let y = azar.siguiente() * 4000.0 - 2000.0;
            let ancho = 1.0 + azar.siguiente() * 300.0;
            let alto = 1.0 + azar.siguiente() * 300.0;
            escena.anadir(rect(i + 1, x, y, ancho, alto));
        }

        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);

        for _ in 0..200 {
            let x = azar.siguiente() * 4000.0 - 2000.0;
            let y = azar.siguiente() * 4000.0 - 2000.0;
            let caja = (
                x,
                y,
                x + azar.siguiente() * 900.0,
                y + azar.siguiente() * 900.0,
            );

            let mut de_la_rejilla = rejilla.candidatos(caja);
            de_la_rejilla.sort_unstable();
            // La rejilla puede devolver de mas —quien la usa filtra— pero
            // JAMAS de menos: un elemento que no salga aqui es un elemento
            // que desaparece de la pantalla.
            for id in a_lo_bruto(&escena, caja) {
                assert!(
                    de_la_rejilla.contains(&id),
                    "la rejilla se dejo el {id} en la caja {caja:?}"
                );
            }
        }
    }

    #[test]
    fn un_elemento_que_cambia_de_sitio_cambia_de_celda() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(1, 0.0, 0.0, 10.0, 10.0));
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);
        assert_eq!(rejilla.candidatos((0.0, 0.0, 20.0, 20.0)), vec![id]);

        escena.buscar_mut(id).unwrap().mover(5000.0, 5000.0);
        rejilla.sincronizar(&escena);

        assert!(
            rejilla.candidatos((0.0, 0.0, 20.0, 20.0)).is_empty(),
            "ya no esta donde estaba"
        );
        assert_eq!(
            rejilla.candidatos((4990.0, 4990.0, 5020.0, 5020.0)),
            vec![id]
        );
    }

    #[test]
    fn sincronizar_sin_cambios_no_hace_nada() {
        // Se llama en cada fotograma. Si reconstruyera la rejilla entera
        // cada vez, seria mas cara que la fuerza bruta que viene a evitar.
        let mut escena = Escena::nueva();
        for i in 0..100 {
            escena.anadir(rect(i + 1, i as f32 * 10.0, 0.0, 5.0, 5.0));
        }
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);
        let celdas = rejilla.cuantas_celdas();

        rejilla.sincronizar(&escena);
        assert_eq!(rejilla.cuantas_celdas(), celdas, "ni una celda de mas");
    }

    #[test]
    fn un_elemento_borrado_sale_de_la_rejilla() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(1, 0.0, 0.0, 10.0, 10.0));
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);

        escena.borrar(id);
        rejilla.sincronizar(&escena);

        assert!(
            rejilla
                .candidatos((-100.0, -100.0, 100.0, 100.0))
                .is_empty()
        );
    }

    #[test]
    fn un_elemento_restaurado_vuelve_a_la_rejilla() {
        // Deshacer un borrado es exactamente esto: restaurar sube la
        // version igual que borrar, y la rejilla tiene que volver a
        // apuntar el elemento o desaparece de la pantalla sin explicacion.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(1, 0.0, 0.0, 10.0, 10.0));
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);

        escena.borrar(id);
        rejilla.sincronizar(&escena);
        assert!(
            rejilla
                .candidatos((-100.0, -100.0, 100.0, 100.0))
                .is_empty(),
            "borrado, no deberia estar"
        );

        escena.restaurar(id);
        rejilla.sincronizar(&escena);
        assert_eq!(
            rejilla.candidatos((-100.0, -100.0, 100.0, 100.0)),
            vec![id],
            "restaurado, deberia volver a salir"
        );
    }

    #[test]
    fn un_elemento_enorme_ocupa_todas_las_celdas_que_cruza() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(1, 0.0, 0.0, 1000.0, 1000.0));
        let mut rejilla = Rejilla::con_celda(100.0);
        rejilla.sincronizar(&escena);

        // Se encuentra tocando cualquiera de sus rincones.
        assert_eq!(rejilla.candidatos((5.0, 5.0, 6.0, 6.0)), vec![id]);
        assert_eq!(rejilla.candidatos((995.0, 995.0, 996.0, 996.0)), vec![id]);
    }

    #[test]
    fn no_devuelve_el_mismo_id_dos_veces() {
        // Un elemento que cruza cuatro celdas y una consulta que abarca las
        // cuatro: si no se deduplicara, se pintaria cuatro veces.
        let mut escena = Escena::nueva();
        escena.anadir(rect(1, 90.0, 90.0, 20.0, 20.0));
        let mut rejilla = Rejilla::con_celda(100.0);
        rejilla.sincronizar(&escena);

        assert_eq!(rejilla.candidatos((0.0, 0.0, 300.0, 300.0)).len(), 1);
    }

    #[test]
    fn un_elemento_girado_no_se_pierde_cerca_del_borde_de_la_consulta() {
        // Cuadrado de 100x100 centrado en el origen, girado 45 grados: su
        // caja SIN girar es (-50,-50,50,50), pero girada la diagonal llega a
        // ±70.71. Con celdas de 64, la caja sin girar solo ocupa las celdas
        // (-1,-1)..(0,0); una consulta en (65,0,70,5) cae en la celda (1,0),
        // que la indexacion vieja jamas apuntaba. Es el hueco exacto que
        // describe el comentario de `candidatos()`: el elemento
        // desaparecia de la pantalla sin ningun error.
        let mut escena = Escena::nueva();
        let mut cuadrado = rect(1, -50.0, -50.0, 100.0, 100.0);
        cuadrado.angulo = std::f32::consts::FRAC_PI_4;
        let id = escena.anadir(cuadrado);

        let mut rejilla = Rejilla::con_celda(64.0);
        rejilla.sincronizar(&escena);

        assert_eq!(
            rejilla.candidatos((65.0, 0.0, 70.0, 5.0)),
            vec![id],
            "el cuadrado girado deberia aparecer cerca de su diagonal"
        );
    }

    #[test]
    fn con_angulo_cero_la_caja_indexable_es_la_de_siempre() {
        // La guarda de `caja_indexable`: sin giro, no puede cambiar nada de
        // lo que ya funcionaba.
        let e = rect(1, 10.0, 20.0, 30.0, 40.0);
        assert_eq!(super::caja_indexable(&e), e.caja());
    }

    #[test]
    fn los_candidatos_salen_en_el_orden_de_pintado_aunque_se_reordene_sin_editar() {
        // «Traer al frente» cambia el orden de la escena sin tocar la version
        // de nadie. La rejilla guardaba los ids por orden de edicion, y quien
        // pinta los recorre tal cual: la escena cambiaba y la pantalla no.
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0, 0.0, 0.0, 50.0, 50.0));
        let b = escena.anadir(rect(0, 10.0, 10.0, 50.0, 50.0));
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);
        let caja = (0.0, 0.0, 100.0, 100.0);
        assert_eq!(rejilla.candidatos(caja), vec![a, b]);

        let mut sel = crate::seleccion::Seleccion::nueva();
        sel.poner(a);
        crate::organizar::al_frente(&mut escena, &sel);
        rejilla.sincronizar(&escena);
        assert_eq!(
            rejilla.candidatos(caja),
            vec![b, a],
            "a tiene que ir encima"
        );

        // Caso negativo: editar a `b` (que la reapunta al final de su celda)
        // no la sube por encima de `a`, que sigue delante en la escena.
        escena.buscar_mut(b).unwrap().tocar();
        rejilla.sincronizar(&escena);
        assert_eq!(rejilla.candidatos(caja), vec![b, a]);
    }
}
