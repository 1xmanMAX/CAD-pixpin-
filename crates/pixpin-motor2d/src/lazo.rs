//! **El lazo: elegir rodeando a mano.**
//!
//! Port de `getElementsWithinLasso` (`Collision.kt:394-409`) del PixPin de
//! Android, con una diferencia medida que se explica abajo.
//!
//! # Por que hace falta si ya hay marquesina
//!
//! Porque el recuadro coge por la caja, y una caja es un rectangulo paralelo
//! a los ejes. En un croquis apretado —una cota al lado de una raya al lado
//! de un rotulo— cualquier recuadro que coja lo que quieres coge tambien dos
//! cosas que no quieres, y quitarlas despues a Mayus+clic cuesta mas que
//! haberlas rodeado bien a la primera. El lazo es el recuadro sin la
//! obligacion de ser rectangular.
//!
//! # En que se aparta del movil, y por que
//!
//! El movil decide con dos reglas baratas: **el centro de la caja dentro del
//! lazo**, o **algun punto del lazo tocando el elemento**. Funciona casi
//! siempre y se equivoca en dos sitios que aqui se notan mas porque el raton
//! traza lazos mas finos que un dedo:
//!
//! - un lazo trazado **dentro** de un rectangulo grande y vacio no toca nada
//!   y no contiene ningun centro... salvo el del propio rectangulo, que cae
//!   justo en medio. El movil se lleva el rectangulo entero cuando lo que se
//!   pedia era lo de dentro;
//! - un lazo que **cruza** una raya larga sin acercarse a ninguno de sus
//!   puntos muestreados no la coge, porque el movil comprueba «puntos del
//!   lazo contra el elemento» y no «tramos del lazo contra tramos del
//!   elemento».
//!
//! Aqui se cruza de verdad, porque el cimiento para hacerlo ya existe:
//! [`crate::perimetros`] reduce cualquier figura a tramos rectos y el lazo
//! tambien es una cadena de tramos rectos. Cruzar dos cadenas de tramos es el
//! mismo algoritmo que ya usa el iman.

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo};
use crate::impacto::esquinas_giradas;
use crate::perimetros::{self, PASO_PERIMETRO};
use crate::pintado::Orden;
use crate::vector::Punto2;

/// Que cuenta como «cogido».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoLazo {
    /// Solo lo que queda **entero** dentro del contorno (`CONTAIN`).
    ///
    /// Es el de por omision por lo mismo que en la marquesina: rodear algo
    /// es una peticion precisa, y coger de mas obliga a deshacer la
    /// seleccion a mano.
    #[default]
    Encerrar,
    /// Tambien lo que el trazo **roza** (`OVERLAP`).
    Rozar,
}

/// Lo minimo que se separan dos puntos del trazo, en pixeles de escena.
///
/// Sin esta criba un lazo lento acumula cientos de puntos a un decimo de
/// pixel unos de otros: el contorno resultante no cambia y cruzarlo contra
/// la escena cuesta el cuadrado de esos puntos de mas.
pub const PASO_MINIMO: f32 = 2.0;

/// Tope de puntos del trazo. Un lazo es un gesto corto; mas alla de esto lo
/// que hay es un dedo apoyado, y el coste de cruzarlo crece al cuadrado.
const MAX_PUNTOS: usize = 600;

/// El trazo del lazo mientras se dibuja.
///
/// Acumula, criba y sabe a quien ha cogido. Se guarda entero y no solo su
/// caja porque **su forma es el dato**: la caja de un lazo en herradura
/// abarca justo lo que el lazo viene a dejar fuera.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lazo {
    puntos: Vec<Punto2>,
}

impl Lazo {
    pub fn empezar(p: Punto2) -> Self {
        Self { puntos: vec![p] }
    }

    /// Anota un punto mas. Devuelve si lo acepto, para que quien pinta sepa
    /// si hay algo nuevo que repintar.
    pub fn mover(&mut self, p: Punto2) -> bool {
        if self.puntos.len() >= MAX_PUNTOS {
            return false;
        }
        match self.puntos.last() {
            Some(u) if u.distancia(p) < PASO_MINIMO => false,
            _ => {
                self.puntos.push(p);
                true
            }
        }
    }

    pub fn puntos(&self) -> &[Punto2] {
        &self.puntos
    }

    /// La caja del trazo, para repintar solo lo suyo.
    pub fn caja(&self) -> Option<(f32, f32, f32, f32)> {
        caja_de(&self.puntos)
    }

    /// A quien ha cogido. Ver [`atrapados`].
    pub fn atrapados(&self, elementos: &[Elemento], modo: ModoLazo) -> Vec<u64> {
        atrapados(elementos, &self.puntos, modo)
    }

    /// Como se ve mientras se traza: la raya a trazos del color del editor.
    ///
    /// **Se cierra sola en la pista**, con el tramo que va del ultimo punto
    /// al primero. Es lo que se va a usar para decidir, asi que ensenar el
    /// contorno abierto seria ensenar una cosa y aplicar otra.
    pub fn orden(&self, escala: f32) -> Orden {
        let mut puntos = self.puntos.clone();
        if puntos.len() > 2 {
            puntos.push(puntos[0]);
        }
        Orden::Polilinea {
            puntos,
            color: crate::pintado::COLOR_SELECCION,
            grosor: (1.0 * escala).max(0.5),
            estilo: EstiloTrazo::Discontinuo,
        }
    }

    /// El velo tenue de lo que el lazo encierra, para que se lea como un
    /// area y no como un garabato.
    pub fn relleno(&self) -> Option<Orden> {
        if self.puntos.len() < 3 {
            return None;
        }
        Some(Orden::Relleno {
            puntos: self.puntos.clone(),
            color: ColorRgba {
                a: 0.10,
                ..crate::pintado::COLOR_SELECCION
            },
        })
    }
}

/// La caja de una nube de puntos.
fn caja_de(puntos: &[Punto2]) -> Option<(f32, f32, f32, f32)> {
    let primero = puntos.first()?;
    let mut caja = (primero.x, primero.y, primero.x, primero.y);
    for p in puntos {
        caja.0 = caja.0.min(p.x);
        caja.1 = caja.1.min(p.y);
        caja.2 = caja.2.max(p.x);
        caja.3 = caja.3.max(p.y);
    }
    Some(caja)
}

/// Si `p` cae dentro del poligono, por la regla par/impar.
///
/// Es el mismo criterio con el que se pinta un anillo, asi que «dentro del
/// lazo» significa lo mismo que «pintado» cuando el trazo se cruza consigo
/// mismo. Un lazo en ocho deja fuera su lobulo repetido, que es lo que se ve.
pub fn dentro_del_poligono(poligono: &[Punto2], p: Punto2) -> bool {
    if poligono.len() < 3 {
        return false;
    }
    let mut dentro = false;
    let mut j = poligono.len() - 1;
    for i in 0..poligono.len() {
        let (a, b) = (poligono[i], poligono[j]);
        // El rayo va hacia +x desde `p`. La comparacion asimetrica de los
        // dos `y` —uno estricto y otro no— es lo que hace que un vertice
        // exactamente a la altura del rayo se cuente una sola vez.
        if (a.y > p.y) != (b.y > p.y) {
            let corte = (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x;
            if p.x < corte {
                dentro = !dentro;
            }
        }
        j = i;
    }
    dentro
}

/// Los ids que el contorno coge, del fondo hacia el frente.
///
/// `contorno` es el trazo **abierto**: aqui se cierra solo, como en la pista.
///
/// Lo bloqueado y lo borrado se quedan fuera, igual que en el recuadro y por
/// lo mismo: bloquear un plano de fondo es pedir que no se arrastre, y un
/// lazo alrededor de todo lo arrastraria.
pub fn atrapados(elementos: &[Elemento], contorno: &[Punto2], modo: ModoLazo) -> Vec<u64> {
    if contorno.len() < 3 {
        return Vec::new();
    }
    let Some(caja_lazo) = caja_de(contorno) else {
        return Vec::new();
    };
    let tramos_lazo = tramos_cerrados(contorno);

    let mut salida = Vec::new();
    for e in elementos {
        // Lo de otro mundo tampoco: fuera del modo guia el lazo no se lleva
        // guias, y dentro no se lleva dibujo (`impacto::Mundo`).
        if !crate::impacto::tocable(e) {
            continue;
        }
        // Criba por caja antes de sacar perimetros: es lo que hace que un
        // plano importado siga respondiendo mientras se traza el lazo.
        let (x0, y0, x1, y1) = e.caja();
        if x1 < caja_lazo.0 || x0 > caja_lazo.2 || y1 < caja_lazo.1 || y0 > caja_lazo.3 {
            continue;
        }

        let puntos = puntos_de_silueta(e);
        if puntos.is_empty() {
            continue;
        }
        let todos_dentro = puntos.iter().all(|p| dentro_del_poligono(contorno, *p));
        let cogido = match modo {
            ModoLazo::Encerrar => todos_dentro,
            // Rozar: o entra algo, o el trazo lo cruza. Las dos hacen falta:
            // una raya que atraviesa el lazo de lado a lado no tiene ningun
            // punto dentro y hay que cogerla; una figura enteramente dentro
            // no cruza ningun tramo y tambien.
            ModoLazo::Rozar => {
                puntos.iter().any(|p| dentro_del_poligono(contorno, *p))
                    || cruza(&tramos_lazo, &puntos, cerrada(e))
            }
        };
        if cogido {
            salida.push(e.id);
        }
    }
    salida
}

/// Los tramos del lazo, ya cerrado sobre si mismo.
fn tramos_cerrados(contorno: &[Punto2]) -> Vec<(Punto2, Punto2)> {
    let mut tramos: Vec<(Punto2, Punto2)> =
        contorno.windows(2).map(|par| (par[0], par[1])).collect();
    if let (Some(primero), Some(ultimo)) = (contorno.first(), contorno.last()) {
        tramos.push((*ultimo, *primero));
    }
    tramos
}

/// La silueta de `e` como lista de puntos, en coordenadas del mundo.
///
/// Sale de [`crate::perimetros`], que es quien sabe por donde pasa cada
/// figura. Los tres que alli no tienen borde —texto, foco y punto— se
/// resuelven por las **esquinas de su caja girada**: no tienen contorno que
/// dibujar, pero ocupan un sitio en la pantalla y rodear ese sitio tiene que
/// cogerlos. Un rotulo que no se puede elegir con el lazo es un rotulo que no
/// se puede mover con lo que rotula.
fn puntos_de_silueta(e: &Elemento) -> Vec<Punto2> {
    let contornos = perimetros::contornos_de(e, PASO_PERIMETRO);
    if contornos.is_empty() {
        return esquinas_giradas(e).to_vec();
    }
    contornos.into_iter().flat_map(|c| c.puntos).collect()
}

/// Si la silueta de la figura se cierra (y por tanto su ultimo tramo va del
/// ultimo punto al primero).
fn cerrada(e: &Elemento) -> bool {
    perimetros::contornos_de(e, PASO_PERIMETRO)
        .first()
        .is_none_or(|c| c.cerrado)
}

/// Si algun tramo del lazo corta algun tramo de la silueta.
fn cruza(tramos_lazo: &[(Punto2, Punto2)], silueta: &[Punto2], cerrada: bool) -> bool {
    let mut pares: Vec<(Punto2, Punto2)> = silueta.windows(2).map(|par| (par[0], par[1])).collect();
    if cerrada && silueta.len() > 2 {
        if let (Some(a), Some(b)) = (silueta.last(), silueta.first()) {
            pares.push((*a, *b));
        }
    }
    tramos_lazo.iter().any(|(a1, a2)| {
        pares
            .iter()
            .any(|(b1, b2)| perimetros::interseccion(*a1, *a2, *b1, *b2).is_some())
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::Figura;

    fn caja(id: u64, figura: Figura, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id,
            figura,
            x,
            y,
            ancho,
            alto,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            ..Default::default()
        }
    }

    /// Un cuadrado de lazo, en sentido horario.
    fn cuadrado(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Punto2> {
        vec![
            Punto2::nuevo(x0, y0),
            Punto2::nuevo(x1, y0),
            Punto2::nuevo(x1, y1),
            Punto2::nuevo(x0, y1),
        ]
    }

    #[test]
    fn lo_que_queda_entero_dentro_del_contorno_se_coge() {
        let dentro = caja(1, Figura::Rectangulo, 20.0, 20.0, 10.0, 10.0);
        let fuera = caja(2, Figura::Rectangulo, 500.0, 500.0, 10.0, 10.0);
        let cogidos = atrapados(
            &[dentro, fuera],
            &cuadrado(0.0, 0.0, 100.0, 100.0),
            ModoLazo::Encerrar,
        );
        assert_eq!(cogidos, vec![1]);
    }

    #[test]
    fn lo_que_asoma_por_fuera_no_se_coge_encerrando_pero_si_rozando() {
        // Es toda la diferencia entre los dos modos, y la razon de que
        // `Encerrar` sea el de por omision: rodear algo es una peticion
        // precisa.
        let medio_fuera = caja(1, Figura::Rectangulo, 90.0, 40.0, 40.0, 10.0);
        let lazo = cuadrado(0.0, 0.0, 100.0, 100.0);
        assert!(
            atrapados(
                std::slice::from_ref(&medio_fuera),
                &lazo,
                ModoLazo::Encerrar
            )
            .is_empty()
        );
        assert_eq!(
            atrapados(&[medio_fuera], &lazo, ModoLazo::Rozar),
            vec![1],
            "rozando si"
        );
    }

    #[test]
    fn un_lazo_dentro_de_un_rectangulo_vacio_no_se_lleva_el_rectangulo() {
        // **La diferencia medida con el movil.** Alli basta con que el
        // centro de la caja caiga dentro del lazo, y el centro de un
        // rectangulo grande cae justo en medio: rodear algo que esta DENTRO
        // de un marco se llevaba el marco entero.
        let marco = caja(1, Figura::Rectangulo, 0.0, 0.0, 400.0, 400.0);
        let lazo = cuadrado(150.0, 150.0, 250.0, 250.0);
        assert!(atrapados(std::slice::from_ref(&marco), &lazo, ModoLazo::Encerrar).is_empty());
        assert!(
            atrapados(&[marco], &lazo, ModoLazo::Rozar).is_empty(),
            "ni rozando: el lazo no toca ninguna de sus paredes"
        );
    }

    #[test]
    fn un_lazo_que_cruza_una_raya_larga_la_coge_aunque_no_roce_sus_puntas() {
        // La otra diferencia: el movil compara PUNTOS del lazo contra el
        // elemento, asi que una raya cruzada por el medio, lejos de sus
        // extremos, se le escapaba. Aqui se cruzan tramos contra tramos.
        let mut raya = caja(1, Figura::Rectangulo, 0.0, 50.0, 1000.0, 0.0);
        raya.figura = Figura::Linea {
            puntos: vec![Punto2::nuevo(0.0, 50.0), Punto2::nuevo(1000.0, 50.0)],
        };
        let lazo = cuadrado(400.0, 0.0, 500.0, 100.0);
        assert_eq!(atrapados(&[raya], &lazo, ModoLazo::Rozar), vec![1]);
    }

    #[test]
    fn un_lazo_en_herradura_deja_fuera_lo_que_rodea_sin_encerrar() {
        // La razon de ser del lazo: su CAJA abarca lo que el trazo viene a
        // dejar fuera. Si se decidiera por la caja, esta prueba cogeria el
        // elemento.
        let en_el_hueco = caja(1, Figura::Rectangulo, 45.0, 60.0, 10.0, 10.0);
        let herradura = vec![
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(100.0, 0.0),
            Punto2::nuevo(100.0, 100.0),
            Punto2::nuevo(60.0, 100.0),
            Punto2::nuevo(60.0, 40.0),
            Punto2::nuevo(40.0, 40.0),
            Punto2::nuevo(40.0, 100.0),
            Punto2::nuevo(0.0, 100.0),
        ];
        assert!(
            atrapados(&[en_el_hueco], &herradura, ModoLazo::Encerrar).is_empty(),
            "esta en el hueco de la herradura, no dentro de ella"
        );
    }

    #[test]
    fn un_rotulo_se_puede_coger_con_el_lazo_aunque_no_tenga_contorno() {
        // El texto no da perimetro a proposito (`perimetros.rs`), pero ocupa
        // un sitio: un rotulo que no se puede elegir con el lazo es un
        // rotulo que no se puede mover con lo que rotula.
        let rotulo = caja(
            1,
            Figura::Texto {
                texto: "A".into(),
                tam: 20.0,
                familia: "Segoe UI".into(),
            },
            20.0,
            20.0,
            30.0,
            20.0,
        );
        assert_eq!(
            atrapados(
                &[rotulo],
                &cuadrado(0.0, 0.0, 100.0, 100.0),
                ModoLazo::Encerrar
            ),
            vec![1]
        );
    }

    #[test]
    fn lo_bloqueado_y_lo_borrado_se_quedan_fuera() {
        // Caso negativo: bloquear un plano de fondo es pedir que no se
        // arrastre, y un lazo alrededor de todo lo arrastraria.
        let mut bloqueado = caja(1, Figura::Rectangulo, 10.0, 10.0, 10.0, 10.0);
        bloqueado.bloqueado = true;
        let mut borrado = caja(2, Figura::Rectangulo, 30.0, 30.0, 10.0, 10.0);
        borrado.borrado = true;
        assert!(
            atrapados(
                &[bloqueado, borrado],
                &cuadrado(0.0, 0.0, 100.0, 100.0),
                ModoLazo::Rozar
            )
            .is_empty()
        );
    }

    #[test]
    fn un_trazo_de_menos_de_tres_puntos_no_coge_nada() {
        // Caso negativo: un clic suelto con el lazo puesto no puede vaciar ni
        // llenar la seleccion por accidente.
        let e = caja(1, Figura::Rectangulo, 0.0, 0.0, 10.0, 10.0);
        for contorno in [
            Vec::new(),
            vec![Punto2::nuevo(0.0, 0.0)],
            vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(5.0, 5.0)],
        ] {
            assert!(atrapados(std::slice::from_ref(&e), &contorno, ModoLazo::Rozar).is_empty());
        }
    }

    #[test]
    fn el_trazo_criba_los_puntos_pegados_y_tiene_tope() {
        let mut l = Lazo::empezar(Punto2::nuevo(0.0, 0.0));
        assert!(
            !l.mover(Punto2::nuevo(0.5, 0.0)),
            "medio pixel no anade nada al contorno y cuesta al cuadrado"
        );
        assert!(l.mover(Punto2::nuevo(0.0, PASO_MINIMO)));
        assert_eq!(l.puntos().len(), 2);

        for i in 0..(MAX_PUNTOS * 2) {
            l.mover(Punto2::nuevo(i as f32 * 10.0, 0.0));
        }
        assert_eq!(l.puntos().len(), MAX_PUNTOS, "el tope aguanta");
    }

    #[test]
    fn la_pista_se_cierra_para_ensenar_lo_que_de_verdad_se_aplica() {
        let mut l = Lazo::empezar(Punto2::nuevo(0.0, 0.0));
        l.mover(Punto2::nuevo(100.0, 0.0));
        l.mover(Punto2::nuevo(100.0, 100.0));
        let Orden::Polilinea { puntos, .. } = l.orden(1.0) else {
            panic!("la pista es una polilinea");
        };
        assert_eq!(puntos.len(), 4);
        assert_eq!(puntos[0], puntos[3], "ensena el contorno que se va a usar");
    }

    #[test]
    fn dentro_del_poligono_acierta_en_el_borde_y_fuera() {
        let p = cuadrado(0.0, 0.0, 10.0, 10.0);
        assert!(dentro_del_poligono(&p, Punto2::nuevo(5.0, 5.0)));
        assert!(!dentro_del_poligono(&p, Punto2::nuevo(15.0, 5.0)));
        // Caso negativo: un poligono degenerado no encierra nada.
        assert!(!dentro_del_poligono(
            &[Punto2::nuevo(0.0, 0.0), Punto2::nuevo(1.0, 1.0)],
            Punto2::nuevo(0.5, 0.5)
        ));
    }
}
