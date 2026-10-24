//! No volver a calcular el garabato de lo que no ha cambiado.
//!
//! `pintado::ordenes()` regenera la geometria de **cada elemento en cada
//! fotograma**. Un dibujo de trabajo son ocho mil elementos: ocho mil
//! generaciones de ruido, sesenta veces por segundo, para un dibujo que no
//! ha cambiado. El campo `version` de `Elemento` existe justo para esto —lo
//! dice su propio comentario— y no lo usaba nadie.
//!
//! # Por que la clave lleva el nivel y no el zoom
//!
//! La geometria depende del aumento: `ordenes_a_distancia` adelgaza los
//! trazos que a esa distancia no se ven. Si la clave llevara el zoom en
//! bruto, mover la rueda un grado invalidaria los ocho mil elementos y la
//! cache no serviria de nada justo cuando mas falta hace.
//!
//! Por eso la clave lleva la **octava**: `log2(zoom)` redondeado hacia
//! abajo. Entre los topes de la camara —5 % y 3.000 %— son diez valores, y
//! solo cambia al doblar o partir por la mitad el aumento. Encuadrar no lo
//! cambia nunca, asi que arrastrar el lienzo no invalida nada.

use std::collections::HashMap;

use crate::elemento::Elemento;
use crate::elemento::{ColorRgba, Figura};
use crate::medida::Escala;
use crate::pintado::{Orden, ordenes_a_distancia, ordenes_medibles};
#[cfg(test)]
use crate::vector::Punto2;

/// El nivel del 5 % de aumento, el tope de alejarse de la camara.
pub const NIVEL_MINIMO: i8 = -5;
/// El nivel del 3.000 %, el tope de acercarse.
pub const NIVEL_MAXIMO: i8 = 4;

/// La octava del aumento: `log2(zoom)` redondeado hacia abajo, sujeto a los
/// topes de la camara.
///
/// Sujetarlo no es paranoia: `log2(0)` es menos infinito y `log2` de un
/// negativo es NaN, y convertir cualquiera de los dos a `i8` es
/// comportamiento que no queremos ni mirar.
pub fn nivel_de_detalle(zoom: f32) -> i8 {
    if !zoom.is_finite() {
        return if zoom > 0.0 {
            NIVEL_MAXIMO
        } else {
            NIVEL_MINIMO
        };
    }
    if zoom <= 0.0 {
        return NIVEL_MINIMO;
    }
    let n = zoom.log2().floor();
    if n < NIVEL_MINIMO as f32 {
        NIVEL_MINIMO
    } else if n > NIVEL_MAXIMO as f32 {
        NIVEL_MAXIMO
    } else {
        n as i8
    }
}

#[derive(Debug, Clone)]
struct Entrada {
    version: u32,
    nivel: i8,
    ordenes: Vec<Orden>,
}

/// Guarda la geometria ya calculada de cada elemento, indexada por su
/// version y su nivel de detalle, para no volver a calcularla si ninguno de
/// los dos cambio.
///
/// **Una por escena.** La clave es el id, y los ids solo son unicos dentro
/// de una escena: dos escenas (dos hojas de un PDF) tienen cada una su
/// elemento 1, y dos trazos con el mismo numero de puntos llegan a la misma
/// version. Compartida, una escena pintaria la geometria de la otra (ver
/// `lector_tinta::Calculado`).
///
/// **No tiene techo de memoria.** Guarda la geometria de los elementos que
/// se han pintado, que son los que caben en pantalla mas los que se hayan
/// visto al pasar; con ocho mil elementos y unas pocas ordenes por elemento
/// es del orden de megas, no decenas. Si la medicion dice otra cosa, el
/// arreglo es tirar las entradas de los elementos que llevan N fotogramas
/// sin pintarse. No hacerlo antes de medirlo.
/// Lo que decide el aspecto de una cota o una barra ademas del elemento:
/// la escala (el numero y los cuadros), el separador decimal y el papel (la
/// tinta adaptada y su halo). Nada de esto sube la version.
#[derive(Debug, Clone, PartialEq)]
struct ClaveMedible {
    version: u32,
    escala: Option<(u32, String, u8)>,
    coma: char,
    papel: [u32; 4],
}

fn bits(c: ColorRgba) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

impl ClaveMedible {
    fn de(e: &Elemento, escala: Option<&Escala>, coma: char, papel: ColorRgba) -> Self {
        ClaveMedible {
            version: e.version,
            escala: escala.map(|x| {
                (
                    x.unidades_por_pixel.to_bits(),
                    x.unidad.clone(),
                    x.decimales,
                )
            }),
            coma,
            papel: bits(papel),
        }
    }

    /// La misma pregunta que `de(...) == self` sin clonar la unidad: se hace
    /// por cada cota visible en cada fotograma.
    fn vale(&self, e: &Elemento, escala: Option<&Escala>, coma: char, papel: ColorRgba) -> bool {
        self.version == e.version
            && self.coma == coma
            && self.papel == bits(papel)
            && match (&self.escala, escala) {
                (None, None) => true,
                (Some((u, unidad, d)), Some(x)) => {
                    *u == x.unidades_por_pixel.to_bits() && *unidad == x.unidad && *d == x.decimales
                }
                _ => false,
            }
    }
}

#[derive(Debug, Default)]
pub struct Cache {
    mapa: HashMap<u64, Entrada>,
    /// Las ordenes de las cotas y las barras (`ordenes_medibles`), aparte
    /// porque su clave no es la de la geometria: no dependen del aumento y
    /// si de la escala y del papel.
    medibles: HashMap<u64, (ClaveMedible, Vec<Orden>)>,
    aciertos: u64,
    fallos: u64,
}

impl Cache {
    pub fn nueva() -> Self {
        Self::default()
    }

    /// La geometria del elemento a ese aumento, calculandola solo si hace
    /// falta.
    pub fn ordenes(&mut self, e: &Elemento, zoom: f32) -> &[Orden] {
        let nivel = nivel_de_detalle(zoom);
        let vale = self
            .mapa
            .get(&e.id)
            .is_some_and(|x| x.version == e.version && x.nivel == nivel);

        if vale {
            self.aciertos += 1;
        } else {
            self.fallos += 1;
            let ordenes = ordenes_a_distancia(e, zoom);
            self.mapa.insert(
                e.id,
                Entrada {
                    version: e.version,
                    nivel,
                    ordenes,
                },
            );
        }
        // El `unwrap` es seguro: o valia, o se acaba de meter.
        &self.mapa.get(&e.id).expect("recien puesto").ordenes
    }

    /// **La octava a la que esta guardada la geometria de `e`**, si la hay
    /// de su version (de cualquier aumento). Quien pinta con prisa (un zoom
    /// que no puede pararse a recalcular cientos de trazos al cambiar de
    /// octava) pide las ordenes a esa octava y las rehace luego.
    pub fn nivel_guardado(&self, e: &Elemento) -> Option<i8> {
        self.mapa
            .get(&e.id)
            .filter(|x| x.version == e.version)
            .map(|x| x.nivel)
    }

    /// **La cota entera y los cuadros de la barra**, calculandolos solo si
    /// cambio el elemento, la escala, la coma o el papel.
    ///
    /// Antes salian de `ordenes_medibles` en CADA pintado de cada cota
    /// visible: el pulso de rough.js de sus ocho rayas y medir el numero con
    /// DirectWrite (dos disposiciones nuevas por cota), fotograma a
    /// fotograma aunque nada hubiera cambiado. La queja: «cuando pongo la
    /// cota esta se mueve lento» (28-sep-2026). Lo que se esta trazando sube
    /// de version en cada aviso y se recalcula solo el.
    ///
    /// Una figura que no mide no se guarda: son casi todas, y no tiene
    /// sentido llenar el mapa de listas vacias.
    pub fn medibles(
        &mut self,
        e: &Elemento,
        escala: Option<&Escala>,
        coma: char,
        papel: ColorRgba,
    ) -> &[Orden] {
        if e.borrado || !matches!(e.figura, Figura::Cota { .. } | Figura::EscalaGrafica) {
            return &[];
        }
        let vale = self
            .medibles
            .get(&e.id)
            .is_some_and(|(clave, _)| clave.vale(e, escala, coma, papel));
        if vale {
            self.aciertos += 1;
        } else {
            self.fallos += 1;
            let ordenes = ordenes_medibles(e, escala, coma, papel);
            self.medibles
                .insert(e.id, (ClaveMedible::de(e, escala, coma, papel), ordenes));
        }
        &self.medibles.get(&e.id).expect("recien puesto").1
    }

    /// Cuantas cotas y barras hay guardadas. Para las pruebas.
    pub fn cuantos_medibles(&self) -> usize {
        self.medibles.len()
    }

    /// Se llama al borrar de verdad un elemento (`Escena::compactar`).
    pub fn olvidar(&mut self, id: u64) {
        self.mapa.remove(&id);
        self.medibles.remove(&id);
    }

    /// **Echa la geometria de lo que ya no se ve nunca**: los elementos que
    /// salieron de la escena o estan borrados.
    ///
    /// `olvidar` no lo llamaba nadie, asi que cada trazo borrado, deshecho o
    /// convertido en forma dejaba aqui su geometria para toda la sesion: la
    /// cache crecia con todo lo que se habia dibujado alguna vez y no con lo
    /// que hay. Quien pinta lo llama de vez en cuando (cuando hay bastantes
    /// mas entradas que elementos vivos), no en cada fotograma: recorre la
    /// escena entera. Lo borrado que vuelva con un Ctrl+Z se recalcula una
    /// vez, que es lo mismo que cuesta pintarlo por primera vez.
    pub fn podar(&mut self, escena: &crate::escena::Escena) {
        let vivos: std::collections::HashSet<u64> = escena.visibles().map(|e| e.id).collect();
        self.mapa.retain(|id, _| vivos.contains(id));
        self.medibles.retain(|id, _| vivos.contains(id));
    }

    /// Si ya merece la pena `podar`: bastantes mas entradas que elementos
    /// vivos. La holgura evita podar por un solo borrado, que no pesa nada.
    pub fn sobran(&self, vivos: usize) -> bool {
        self.mapa.len() > vivos + vivos / 4 + 64
    }

    /// Se llama al abrir otro documento.
    pub fn vaciar(&mut self) {
        self.mapa.clear();
        self.medibles.clear();
    }

    pub fn cuantos(&self) -> usize {
        self.mapa.len()
    }

    /// Cuantas veces valio lo cacheado. Para las pruebas y la medicion.
    pub fn aciertos(&self) -> u64 {
        self.aciertos
    }

    /// Cuantas veces hubo que calcular. Para las pruebas y la medicion.
    pub fn fallos(&self) -> u64 {
        self.fallos
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::azar::Azar;
    use crate::camara::{ZOOM_MAXIMO, ZOOM_MINIMO};
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
    use crate::pintado::{ordenes_a_distancia, ordenes_medibles};

    fn trazo(semilla: u32) -> Elemento {
        let mut azar = Azar::nuevo(semilla);
        let puntos: Vec<Punto2> = (0..40)
            .map(|i| Punto2::nuevo(i as f32 * 3.0, azar.siguiente() * 50.0))
            .collect();
        Elemento {
            id: semilla as u64,
            figura: Figura::Lapiz {
                puntos,
                presiones: Vec::new(),
                opciones: None,
            },
            x: 0.0,
            y: 0.0,
            ancho: 120.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 3.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla,
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

    #[test]
    fn lo_cacheado_es_igual_a_lo_recien_generado() {
        // La prueba que hace la cache digna de confianza. Si esto falla,
        // el dibujo cambia de aspecto segun si venia de la cache o no, que
        // es peor que no tener cache.
        let mut cache = Cache::nueva();
        for semilla in 1..60u32 {
            let e = trazo(semilla);
            for zoom in [0.1, 0.5, 1.0, 2.0, 7.0] {
                let esperado = ordenes_a_distancia(&e, zoom);
                assert_eq!(
                    cache.ordenes(&e, zoom),
                    esperado.as_slice(),
                    "semilla {semilla} al {zoom}"
                );
                // Y la segunda vez, que ya viene de la cache, tambien.
                assert_eq!(cache.ordenes(&e, zoom), esperado.as_slice(), "cacheado");
            }
        }
    }

    #[test]
    fn encuadrar_no_invalida_nada() {
        // Mover el lienzo no cambia el zoom, asi que la cache entera vale.
        // Es lo que hace que arrastrar el lienzo con ocho mil elementos no
        // cueste nada.
        let mut cache = Cache::nueva();
        let e = trazo(1);
        cache.ordenes(&e, 1.0);
        let fallos = cache.fallos();

        for _ in 0..100 {
            cache.ordenes(&e, 1.0);
        }
        assert_eq!(cache.fallos(), fallos, "ni un fallo mas");
        assert_eq!(cache.aciertos(), 100);
    }

    #[test]
    fn mover_la_rueda_un_poco_no_tira_la_cache() {
        // El motivo de que el nivel sea la octava y no el zoom en bruto: si
        // fuera el zoom, un grado de rueda tiraria los ocho mil elementos.
        let mut cache = Cache::nueva();
        let e = trazo(1);
        cache.ordenes(&e, 1.0);
        let fallos = cache.fallos();

        cache.ordenes(&e, 1.3);
        cache.ordenes(&e, 1.9);
        assert_eq!(
            cache.fallos(),
            fallos,
            "1,0 / 1,3 / 1,9 son la misma octava"
        );
    }

    #[test]
    fn cruzar_la_octava_si_la_tira() {
        let mut cache = Cache::nueva();
        let e = trazo(1);
        cache.ordenes(&e, 1.9);
        let fallos = cache.fallos();

        cache.ordenes(&e, 2.1);
        assert_eq!(cache.fallos(), fallos + 1, "2,0 empieza otra octava");
    }

    #[test]
    fn tocar_el_elemento_invalida_su_entrada() {
        let mut cache = Cache::nueva();
        let mut e = trazo(1);
        cache.ordenes(&e, 1.0);
        let fallos = cache.fallos();

        e.tocar();
        cache.ordenes(&e, 1.0);
        assert_eq!(cache.fallos(), fallos + 1, "la version subio");
    }

    #[test]
    fn el_nivel_de_detalle_es_la_octava_del_aumento() {
        assert_eq!(nivel_de_detalle(1.0), 0);
        assert_eq!(nivel_de_detalle(1.9), 0);
        assert_eq!(nivel_de_detalle(2.0), 1);
        assert_eq!(nivel_de_detalle(4.0), 2);
        assert_eq!(nivel_de_detalle(0.5), -1);
        assert_eq!(nivel_de_detalle(0.25), -2);
    }

    #[test]
    fn hay_diez_niveles_entre_los_topes_de_la_camara() {
        // Del 5 % al 3.000 %, que son los topes que ya tiene la camara.
        assert_eq!(nivel_de_detalle(ZOOM_MINIMO), NIVEL_MINIMO);
        assert_eq!(nivel_de_detalle(ZOOM_MAXIMO), NIVEL_MAXIMO);
        assert_eq!((NIVEL_MAXIMO - NIVEL_MINIMO + 1), 10);
    }

    #[test]
    fn un_zoom_absurdo_no_produce_un_nivel_absurdo() {
        // log2(0) es menos infinito, y un i8 no lo aguanta. Que no reviente
        // ni produzca un nivel de mil.
        assert_eq!(nivel_de_detalle(0.0), NIVEL_MINIMO);
        assert_eq!(nivel_de_detalle(-3.0), NIVEL_MINIMO);
        assert_eq!(nivel_de_detalle(f32::INFINITY), NIVEL_MAXIMO);
        assert_eq!(nivel_de_detalle(f32::NAN), NIVEL_MINIMO);
    }

    fn cota(id: u64) -> Elemento {
        let (a, b) = (Punto2::nuevo(10.0, 20.0), Punto2::nuevo(310.0, 95.0));
        Elemento {
            id,
            figura: Figura::Cota { puntos: vec![a, b] },
            ancho: 300.0,
            alto: 75.0,
            x: 10.0,
            y: 20.0,
            trazo: ColorRgba::opaco(0.05, 0.87, 1.0),
            grosor: 2.5,
            ..trazo(id as u32)
        }
    }

    fn cm(por_pixel: f32) -> crate::medida::Escala {
        crate::medida::Escala {
            unidades_por_pixel: por_pixel,
            unidad: "cm".into(),
            decimales: 2,
        }
    }

    const BLANCO: ColorRgba = ColorRgba {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    const NOCHE: ColorRgba = ColorRgba {
        r: 0.07,
        g: 0.07,
        b: 0.07,
        a: 1.0,
    };

    #[test]
    fn la_cota_cacheada_es_identica_a_la_recalculada() {
        let mut cache = Cache::nueva();
        let e = cota(3);
        for escala in [None, Some(cm(0.0166)), Some(cm(0.5))] {
            for papel in [BLANCO, NOCHE] {
                let esperado = ordenes_medibles(&e, escala.as_ref(), ',', papel);
                assert!(!esperado.is_empty());
                assert_eq!(
                    cache.medibles(&e, escala.as_ref(), ',', papel),
                    esperado.as_slice()
                );
                assert_eq!(
                    cache.medibles(&e, escala.as_ref(), ',', papel),
                    esperado.as_slice(),
                    "la segunda vez, de la cache"
                );
            }
        }
    }

    #[test]
    fn la_misma_cota_con_todo_igual_no_se_recalcula() {
        let mut cache = Cache::nueva();
        let e = cota(3);
        let escala = cm(0.0166);
        cache.medibles(&e, Some(&escala), ',', BLANCO);
        let fallos = cache.fallos();
        for _ in 0..50 {
            cache.medibles(&e, Some(&escala), ',', BLANCO);
        }
        assert_eq!(cache.fallos(), fallos, "ni un fallo mas");
    }

    #[test]
    fn cambiar_el_papel_invalida_la_cota_cacheada() {
        let mut cache = Cache::nueva();
        let e = cota(3);
        let blanco = cache.medibles(&e, None, ',', BLANCO).to_vec();
        let noche = cache.medibles(&e, None, ',', NOCHE).to_vec();
        assert_ne!(blanco, noche, "la tinta se adapta al papel");
        assert_eq!(noche, ordenes_medibles(&e, None, ',', NOCHE));
    }

    #[test]
    fn cambiar_la_escala_invalida_la_cota_cacheada() {
        // Calibrar no toca la version de la cota: si la clave no llevara la
        // escala, se seguirian viendo pixeles despues de calibrar.
        let mut cache = Cache::nueva();
        let e = cota(3);
        let sin = cache.medibles(&e, None, ',', BLANCO).to_vec();
        let con = cache.medibles(&e, Some(&cm(0.0166)), ',', BLANCO).to_vec();
        let otra = cache.medibles(&e, Some(&cm(0.5)), ',', BLANCO).to_vec();
        let otra_unidad = cache
            .medibles(
                &e,
                Some(&crate::medida::Escala {
                    unidad: "m".into(),
                    ..cm(0.5)
                }),
                ',',
                BLANCO,
            )
            .to_vec();
        assert_ne!(sin, con);
        assert_ne!(con, otra);
        assert_ne!(otra, otra_unidad);
        assert_eq!(
            otra_unidad,
            ordenes_medibles(
                &e,
                Some(&crate::medida::Escala {
                    unidad: "m".into(),
                    ..cm(0.5)
                }),
                ',',
                BLANCO
            )
        );
    }

    #[test]
    fn mover_la_cota_la_recalcula_y_la_coma_tambien_cuenta() {
        let mut cache = Cache::nueva();
        let mut e = cota(3);
        let antes = cache.medibles(&e, Some(&cm(0.0166)), ',', BLANCO).to_vec();
        e.mover(40.0, 0.0);
        let despues = cache.medibles(&e, Some(&cm(0.0166)), ',', BLANCO).to_vec();
        assert_ne!(antes, despues, "la version subio");
        let punto = cache.medibles(&e, Some(&cm(0.0166)), '.', BLANCO).to_vec();
        assert_ne!(despues, punto, "el separador decimal va en el numero");
    }

    #[test]
    fn una_figura_que_no_mide_no_ocupa_sitio_en_la_cache_de_medibles() {
        let mut cache = Cache::nueva();
        let e = trazo(1);
        assert!(cache.medibles(&e, None, ',', BLANCO).is_empty());
        assert_eq!(cache.cuantos_medibles(), 0);
        let mut c = cota(9);
        cache.medibles(&c, None, ',', BLANCO);
        assert_eq!(cache.cuantos_medibles(), 1);
        c.borrado = true;
        assert!(
            cache.medibles(&c, None, ',', BLANCO).is_empty(),
            "borrada no pinta"
        );
        cache.olvidar(9);
        assert_eq!(
            cache.cuantos_medibles(),
            0,
            "olvidar saca tambien su numero"
        );
    }

    #[test]
    fn olvidar_saca_el_elemento_y_vaciar_los_saca_todos() {
        let mut cache = Cache::nueva();
        let a = trazo(1);
        let b = trazo(2);
        cache.ordenes(&a, 1.0);
        cache.ordenes(&b, 1.0);
        assert_eq!(cache.cuantos(), 2);

        cache.olvidar(a.id);
        assert_eq!(cache.cuantos(), 1);
        cache.vaciar();
        assert_eq!(cache.cuantos(), 0);
    }
}
