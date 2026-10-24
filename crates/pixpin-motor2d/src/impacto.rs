//! Que elemento hay bajo el cursor.
//!
//! La regla que decide todo: **una figura sin relleno se toca por su borde, no
//! por su interior.** Un rectangulo vacio dibujado alrededor de un texto no
//! puede robar los clics de ese texto; si lo hiciera, encuadrar algo lo
//! volveria inalcanzable.
//!
//! La tolerancia existe porque nadie acierta un trazo de un pixel: se toca lo
//! que esta "cerca", y cerca significa la mitad del grosor mas un margen fijo
//! del tamaño de la punta de un dedo en raton.

use crate::elemento::{Elemento, Figura};
use crate::vector::{Punto2, distancia_a_segmento};

/// Margen extra de tolerancia, en pixeles del documento. Un trazo de 1 px se
/// sigue pudiendo tocar sin apuntar al pixel exacto.
pub const TOLERANCIA: f32 = 6.0;

/// **En que mundo se trabaja**: el del dibujo o el de las guias (el modo
/// guia del movil, `DrawController.editable`).
///
/// Es toda la regla: dentro del modo guia se elige, se mueve, se estira y se
/// borra **solo** el andamio; fuera, solo el dibujo, y **ninguna guia se
/// puede borrar por accidente**. El andamio esta para pasarle el lapiz por
/// encima, asi que la goma y el clic pasan por el todo el rato: sin separar
/// los dos mundos, media faena era volver a trazar guias que uno mismo se
/// habia llevado por delante. Y una guia escondida no se toca en ninguno.
///
/// Va aqui, en el picado, por lo mismo que lo bloqueado: lo respetan a la
/// vez el clic, la marquesina, el lazo y la goma sin que nadie se acuerde.
/// Quien sabe el modo (el gesto) lo pone mientras atiende un evento
/// ([`con_mundo`]); fuera de eso rige el de fabrica, el del dibujo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mundo {
    /// El modo guia puesto: lo tocable son las guias.
    pub guias: bool,
    /// Las guias a la vista (`Escena::referencias_visibles`).
    pub guias_visibles: bool,
}

impl Default for Mundo {
    fn default() -> Self {
        Mundo {
            guias: false,
            guias_visibles: true,
        }
    }
}

impl Mundo {
    /// Si `e` es de este mundo (sin mirar si esta borrado o bloqueado).
    pub fn admite(&self, e: &Elemento) -> bool {
        let guia = e.extras.referencia;
        guia == self.guias && (self.guias_visibles || !guia)
    }
}

thread_local! {
    /// El mundo en el que se pica ahora, en este hilo (cada editor tiene el
    /// suyo, ver `panel_dibujo`).
    static MUNDO: std::cell::Cell<Mundo> = std::cell::Cell::new(Mundo::default());
}

/// Hace `f` picando en el mundo `m`, y deja el de antes al acabar.
pub fn con_mundo<R>(m: Mundo, f: impl FnOnce() -> R) -> R {
    let antes = MUNDO.with(|c| c.replace(m));
    // Se devuelve el de antes tambien si `f` se rompe: un panico a mitad de
    // un evento no puede dejar el hilo picando para siempre en las guias.
    struct Devolver(Mundo);
    impl Drop for Devolver {
        fn drop(&mut self) {
            MUNDO.with(|c| c.set(self.0));
        }
    }
    let _devolver = Devolver(antes);
    f()
}

/// El mundo en el que se pica ahora.
pub fn mundo() -> Mundo {
    MUNDO.with(std::cell::Cell::get)
}

/// Si `e` se puede elegir, mover o borrar ahora: ni borrado, ni bloqueado, y
/// de este mundo.
pub fn tocable(e: &Elemento) -> bool {
    !e.borrado && !e.bloqueado && mundo().admite(e)
}

/// Si el punto toca el elemento.
pub fn toca(e: &Elemento, p: Punto2) -> bool {
    // Un elemento bloqueado se ve pero no se toca: es lo que se pide al
    // bloquear un plano de fondo para dibujar encima sin arrastrarlo. Va
    // aqui, en el picado, y no en cada sitio que elige: asi lo respetan a la
    // vez el clic, la marquesina y la goma, sin que nadie se acuerde. Lo
    // mismo con el mundo: una guia fuera del modo guia (ver `Mundo`).
    if !tocable(e) {
        return false;
    }
    // El angulo se deshace sobre el punto, no sobre la figura: girar el punto
    // es una operacion; girar toda la geometria, cientos.
    let (x0, y0, x1, y1) = e.caja();
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let p = if e.angulo != 0.0 {
        p.girar(centro, -e.angulo)
    } else {
        p
    };

    let margen = e.grosor / 2.0 + TOLERANCIA;

    // **La flecha curva se toca por la curva**, que es lo que se ve, y no
    // por la quebrada de sus puntos: en una vuelta cerrada las dos se
    // separan varios pixeles y se agarraba aire. Su caja tambien es la de la
    // curva, que se pasa un poco de la de los puntos.
    let curva = crate::curva::trazado_curvo(e);
    let (x0, y0, x1, y1) = match curva.as_deref().and_then(crate::curva::caja) {
        Some((cx0, cy0, cx1, cy1)) => (x0.min(cx0), y0.min(cy0), x1.max(cx1), y1.max(cy1)),
        None => (x0, y0, x1, y1),
    };

    // Descarte rapido por caja: barato y evita el trabajo fino en la inmensa
    // mayoria de los elementos de una escena grande.
    if p.x < x0 - margen || p.x > x1 + margen || p.y < y0 - margen || p.y > y1 + margen {
        return false;
    }
    if let Some(c) = &curva {
        return cerca_de_la_polilinea(c, p, margen);
    }

    match &e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. }
        | Figura::Cota { puntos } => cerca_de_la_polilinea(puntos, p, margen),

        // La barra va con las de caja, como el rectangulo (D37): nunca
        // lleva relleno, asi que se toca por su borde.
        Figura::Rectangulo | Figura::Rombo | Figura::Mosaico { .. } | Figura::EscalaGrafica => {
            if e.tiene_relleno() {
                dentro_de_la_caja(p, e, margen)
            } else {
                cerca_del_borde_del_rectangulo(p, e, margen)
            }
        }

        // Un marco se agarra por su borde, nunca por dentro: dentro esta lo
        // que contiene, y pinchar ahi tiene que elegir eso y no el marco.
        Figura::Marco { .. } => cerca_del_borde_del_rectangulo(p, e, margen),

        // Los instrumentos se cogen por dentro: son una lamina con sus reglas
        // (`Collision.kt`, pedido del usuario el 2-sep-2026).
        Figura::Instrumento(_) => dentro_de_la_caja(p, e, margen),

        // Lo que se ve del foco es el hueco: se agarra por dentro.
        Figura::Foco { .. } => dentro_de_la_caja(p, e, margen),

        // La lupa se agarra por su cristal entero, como el foco: lo de
        // dentro es ella (`hitElementItself` del movil).
        Figura::Lupa { .. } => dentro_de_la_caja(p, e, margen),

        // El arco se toca por la raya que se ve, no por su caja: la caja es
        // la del ovalo entero y agarrarlo por ahi seria agarrar aire en tres
        // cuartos de ella.
        // El sector, el segmento y la marca de angulo: por su borde, y por
        // dentro si van rellenos.
        Figura::Arco { .. } if crate::geometria_exacta::contorno_cerrado(e).is_some() => {
            let mut borde = crate::geometria_exacta::contorno_cerrado(e).unwrap_or_default();
            let dentro = (e.tiene_relleno() || e.extras.degradado.is_some())
                && crate::lazo::dentro_del_poligono(&borde, p);
            if let Some(q) = borde.first().copied() {
                borde.push(q);
            }
            dentro || cerca_de_la_polilinea(&borde, p, margen)
        }
        Figura::Arco { inicio, barrido } => cerca_de_la_polilinea(
            &crate::pintado::arco_muestreado(e, *inicio, *barrido),
            p,
            margen,
        ),

        // La region se agarra por su contorno encontrado, y por dentro solo
        // si de verdad esta pintada: una region sin relleno es un borde.
        Figura::Region { contorno, .. } => {
            cerca_de_la_polilinea(contorno, p, margen)
                || (e.tiene_relleno() && dentro_de_la_caja(p, e, margen))
        }

        // El punto no tiene caja: su `caja()` es un solo sitio, asi que esto
        // es un circulo de radio `margen` alrededor de el, que es justo el
        // area con la que se puede pinchar algo que no tiene tamano.
        Figura::Punto { .. } => dentro_de_la_caja(p, e, margen),

        // El numero de serie es un circulo dentro de su caja, no la caja: va
        // con la elipse.
        Figura::Elipse | Figura::Serie { .. } => {
            let rx = (e.ancho / 2.0).max(0.001);
            let ry = (e.alto / 2.0).max(0.001);
            let cx = e.x + rx;
            let cy = e.y + ry;
            // Distancia normalizada al centro: 1 es justo el borde.
            let d = ((p.x - cx) / rx).powi(2) + ((p.y - cy) / ry).powi(2);
            if e.tiene_relleno() {
                d <= 1.0
            } else {
                // Sin relleno solo cuenta el anillo del borde. El margen se
                // normaliza con el radio menor, que es el caso mas estrecho.
                let holgura = margen / rx.min(ry);
                let dentro = (1.0 - holgura).max(0.0).powi(2);
                let fuera = (1.0 + holgura).powi(2);
                d >= dentro && d <= fuera
            }
        }

        // Texto e imagen son cajas solidas: su interior SI cuenta, porque es
        // donde esta el contenido.
        Figura::Texto { .. } | Figura::Imagen { .. } | Figura::Emoji { .. } => {
            dentro_de_la_caja(p, e, margen)
        }
    }
}

fn dentro_de_la_caja(p: Punto2, e: &Elemento, margen: f32) -> bool {
    let (x0, y0, x1, y1) = e.caja();
    p.x >= x0 - margen && p.x <= x1 + margen && p.y >= y0 - margen && p.y <= y1 + margen
}

/// Si el punto cae dentro de la caja del elemento, **contando su giro** y sin
/// ninguna tolerancia.
///
/// No es lo mismo que [`toca`] y por eso existe aparte: aquel pregunta «se
/// esta picando esto», con su margen del tamano de la punta del raton y con
/// la regla de que una figura vacia solo se toca por el borde. Este pregunta
/// «esta el punto en el area que ocupa», que es lo que necesita el enganche
/// de flechas para decidir si la punta se solto **encima** de una caja —vacia
/// o no— y no al lado.
///
/// El angulo se deshace sobre el punto y no sobre la figura, igual que en
/// [`toca`]: girar un punto es una operacion; girar la geometria, cientos.
pub fn dentro_de_la_caja_girada(e: &Elemento, p: Punto2) -> bool {
    let (x0, y0, x1, y1) = e.caja();
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let local = if e.angulo == 0.0 {
        p
    } else {
        p.girar(centro, -e.angulo)
    };
    local.x >= x0 && local.x <= x1 && local.y >= y0 && local.y <= y1
}

fn cerca_del_borde_del_rectangulo(p: Punto2, e: &Elemento, margen: f32) -> bool {
    let (x0, y0, x1, y1) = (e.x, e.y, e.x + e.ancho, e.y + e.alto);
    let esquinas = [
        Punto2::nuevo(x0, y0),
        Punto2::nuevo(x1, y0),
        Punto2::nuevo(x1, y1),
        Punto2::nuevo(x0, y1),
    ];
    (0..4).any(|i| distancia_a_segmento(p, esquinas[i], esquinas[(i + 1) % 4]) <= margen)
}

fn cerca_de_la_polilinea(puntos: &[Punto2], p: Punto2, margen: f32) -> bool {
    match puntos.len() {
        0 => false,
        // Un trazo de un punto es un circulo: se toca por su radio.
        1 => p.distancia(puntos[0]) <= margen,
        _ => puntos
            .windows(2)
            .any(|w| distancia_a_segmento(p, w[0], w[1]) <= margen),
    }
}

/// El elemento de mas arriba que toca el punto, o `None`.
///
/// Se recorre al reves porque el ultimo de la lista es el que esta encima: al
/// hacer clic donde se solapan dos, se selecciona el que se ve.
pub fn elemento_en(elementos: &[Elemento], p: Punto2) -> Option<u64> {
    elementos.iter().rev().find(|e| toca(e, p)).map(|e| e.id)
}

/// Las cuatro esquinas de la caja del elemento, ya giradas.
///
/// En orden: noroeste, noreste, sureste, suroeste. Es lo que hace falta
/// para saber si cabe dentro de algo: la caja sin girar de un cuadrado de
/// 100 girado 45 grados mide 141 en diagonal, y usarla daria por dentro
/// cosas que se salen.
pub fn esquinas_giradas(e: &Elemento) -> [Punto2; 4] {
    let (x0, y0, x1, y1) = e.caja();
    let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    [
        Punto2::nuevo(x0, y0),
        Punto2::nuevo(x1, y0),
        Punto2::nuevo(x1, y1),
        Punto2::nuevo(x0, y1),
    ]
    .map(|p| {
        if e.angulo == 0.0 {
            p
        } else {
            p.girar(c, e.angulo)
        }
    })
}

/// Todos los que tocan el punto, **de arriba abajo**.
///
/// El orden de la lista es el de pintado —el ultimo se pinta encima—, asi
/// que se recorre al reves: el de encima va primero, que es el que el
/// usuario cree que esta tocando.
pub fn elementos_en(elementos: &[Elemento], p: Punto2) -> Vec<u64> {
    elementos
        .iter()
        .rev()
        .filter(|e| toca(e, p))
        .map(|e| e.id)
        .collect()
}

/// Los que caben **enteros** dentro de la caja. Es la marquesina (D29).
///
/// Enteros y no «los que toquen» a proposito: con trazos largos, tocar
/// selecciona cosas que el usuario no ve venir. Un trazo que cruza la
/// pantalla entraria en cualquier marquesina que roce su camino, y el
/// usuario acabaria moviendo medio dibujo sin saber por que.
pub fn dentro_de(elementos: &[Elemento], caja: (f32, f32, f32, f32)) -> Vec<u64> {
    let (mx0, my0, mx1, my1) = caja;
    let (mx0, mx1) = (mx0.min(mx1), mx0.max(mx1));
    let (my0, my1) = (my0.min(my1), my0.max(my1));
    elementos
        .iter()
        .filter(|e| tocable(e))
        .filter(|e| {
            esquinas_giradas(e)
                .iter()
                .all(|q| q.x >= mx0 && q.x <= mx1 && q.y >= my0 && q.y <= my1)
        })
        .map(|e| e.id)
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo};

    fn base() -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            x: 100.0,
            y: 100.0,
            ancho: 200.0,
            alto: 100.0,
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

    #[test]
    fn una_flecha_curva_se_toca_por_la_curva_que_se_ve() {
        let flecha = |redondo| Elemento {
            figura: Figura::Flecha {
                puntos: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(100.0, 100.0),
                    Punto2::nuevo(200.0, 0.0),
                ],
                punta_inicio: crate::formas::TipoPunta::Ninguna,
                punta_fin: crate::formas::TipoPunta::Flecha,
                codos: false,
            },
            redondo,
            ..base()
        };
        // A mitad del primer tramo la curva pasa por (43.75, 56.25), casi
        // nueve pixeles fuera de la quebrada.
        let en_la_curva = Punto2::nuevo(43.75, 56.25);
        assert!(toca(&flecha(true), en_la_curva));
        // Caso negativo: la misma flecha recta no se agarra por ahi, que
        // alli no hay nada pintado.
        assert!(!toca(&flecha(false), en_la_curva));
        // Y el vertice, por donde pasan las dos, se toca en las dos.
        assert!(toca(&flecha(true), Punto2::nuevo(100.0, 99.0)));
    }

    #[test]
    fn una_linea_fina_se_toca_sin_apuntar_al_pixel_exacto() {
        let e = Elemento {
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(0.0, 100.0), Punto2::nuevo(200.0, 100.0)],
            },
            grosor: 1.0,
            ..base()
        };
        assert!(toca(&e, Punto2::nuevo(100.0, 103.0)), "3 px deberia bastar");
        assert!(!toca(&e, Punto2::nuevo(100.0, 140.0)), "40 px es demasiado");
    }

    #[test]
    fn un_rectangulo_sin_relleno_se_toca_por_el_borde_y_no_por_dentro() {
        // La regla que sostiene todo: encuadrar algo no puede volverlo
        // inalcanzable.
        let e = base();
        assert!(toca(&e, Punto2::nuevo(100.0, 150.0)), "el borde izquierdo");
        assert!(toca(&e, Punto2::nuevo(200.0, 100.0)), "el borde superior");
        assert!(
            !toca(&e, Punto2::nuevo(200.0, 150.0)),
            "el centro de un rectangulo vacio NO se toca"
        );
    }

    #[test]
    fn un_rectangulo_relleno_si_se_toca_por_dentro() {
        // Caso complementario del anterior: con relleno, el interior es el
        // elemento.
        let e = Elemento {
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 0.0)),
            ..base()
        };
        assert!(toca(&e, Punto2::nuevo(200.0, 150.0)));
    }

    #[test]
    fn una_elipse_sin_relleno_se_toca_por_el_anillo() {
        let e = Elemento {
            figura: Figura::Elipse,
            ..base()
        };
        // Extremo derecho del borde.
        assert!(toca(&e, Punto2::nuevo(300.0, 150.0)));
        // Centro: hueco.
        assert!(!toca(&e, Punto2::nuevo(200.0, 150.0)));
        // La esquina de la caja queda FUERA de la elipse: es el caso que
        // distingue una elipse de verdad de un rectangulo disfrazado.
        assert!(!toca(&e, Punto2::nuevo(100.0, 100.0)));
    }

    #[test]
    fn un_elemento_borrado_no_se_toca() {
        let e = Elemento {
            borrado: true,
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            ..base()
        };
        assert!(!toca(&e, Punto2::nuevo(200.0, 150.0)));
    }

    #[test]
    fn el_texto_y_la_imagen_se_tocan_por_dentro() {
        let texto = Elemento {
            figura: Figura::Texto {
                texto: "hola".into(),
                tam: 14.0,
                familia: "Segoe UI".into(),
            },
            ..base()
        };
        assert!(toca(&texto, Punto2::nuevo(200.0, 150.0)));
        let imagen = Elemento {
            figura: Figura::Imagen { id_objeto: 3 },
            ..base()
        };
        assert!(toca(&imagen, Punto2::nuevo(200.0, 150.0)));
    }

    #[test]
    fn gana_el_elemento_de_arriba() {
        // Dos rectangulos rellenos superpuestos: al hacer clic se selecciona
        // el que se VE, que es el ultimo de la lista.
        let abajo = Elemento {
            id: 1,
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            ..base()
        };
        let arriba = Elemento {
            id: 2,
            relleno: Some(ColorRgba::opaco(0.0, 1.0, 0.0)),
            ..base()
        };
        assert_eq!(
            elemento_en(&[abajo, arriba], Punto2::nuevo(200.0, 150.0)),
            Some(2)
        );
    }

    #[test]
    fn en_un_hueco_no_hay_ningun_elemento() {
        assert_eq!(elemento_en(&[base()], Punto2::nuevo(900.0, 900.0)), None);
    }

    #[test]
    fn un_elemento_girado_se_toca_donde_se_ve() {
        // Un rectangulo apaisado girado un cuarto de vuelta pasa a ser
        // vertical: un punto sobre su nuevo borde debe tocarlo, y el hueco de
        // donde estaba antes, no.
        let e = Elemento {
            angulo: std::f32::consts::FRAC_PI_2,
            relleno: Some(ColorRgba::opaco(0.0, 0.0, 1.0)),
            ..base()
        };
        // El centro sigue siendo el centro, gire lo que gire.
        assert!(toca(&e, Punto2::nuevo(200.0, 150.0)));
        // A 80 px del centro en vertical: fuera del original (alto 100, o sea
        // 50 a cada lado), dentro tras girar (ancho 200 = 100 a cada lado).
        assert!(
            toca(&e, Punto2::nuevo(200.0, 230.0)),
            "tras girar, el lado largo es el vertical"
        );
    }

    #[test]
    fn un_trazo_de_un_solo_punto_se_toca_por_su_radio() {
        let e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(50.0, 50.0)],
                presiones: vec![],
                opciones: None,
            },
            grosor: 10.0,
            ..base()
        };
        assert!(toca(&e, Punto2::nuevo(53.0, 53.0)));
        assert!(!toca(&e, Punto2::nuevo(150.0, 150.0)));
    }

    #[test]
    fn el_foco_se_agarra_por_dentro_de_su_marco() {
        // Lo que se ve es el anillo de su marco: es lo que se intenta mover.
        let e = Elemento {
            figura: Figura::Foco {
                cristal: Default::default(),
            },
            ..base()
        };
        assert!(toca(&e, Punto2::nuevo(200.0, 150.0)));
        assert!(!toca(&e, Punto2::nuevo(500.0, 400.0)));
    }

    #[test]
    fn la_marquesina_coge_lo_que_esta_entero_dentro_y_no_lo_que_toca() {
        // D29. Con trazos largos, «lo que toque» selecciona cosas que el
        // usuario no ve venir: un trazo de dos metros que cruza la pantalla
        // entraria en cualquier marquesina que roce su camino.
        let dentro = Elemento {
            x: 10.0,
            y: 10.0,
            ancho: 20.0,
            alto: 20.0,
            ..base()
        };
        let a_medias = Elemento {
            x: 90.0,
            y: 10.0,
            ancho: 40.0,
            alto: 20.0,
            ..base()
        };
        let fuera = Elemento {
            x: 500.0,
            y: 500.0,
            ancho: 10.0,
            alto: 10.0,
            ..base()
        };
        let mut lista = vec![dentro, a_medias, fuera];
        for (i, e) in lista.iter_mut().enumerate() {
            e.id = i as u64 + 1;
        }

        let cogidos = dentro_de(&lista, (0.0, 0.0, 100.0, 100.0));
        assert_eq!(cogidos, vec![1], "solo el que cabe entero");
    }

    #[test]
    fn un_elemento_girado_cuenta_por_sus_esquinas_giradas() {
        // Un cuadrado de 100 girado 45 grados mide 141 en diagonal: cabe en su
        // caja sin girar pero NO en una marquesina justa.
        use std::f32::consts::FRAC_PI_4;
        let e = Elemento {
            id: 1,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            angulo: FRAC_PI_4,
            ..base()
        };
        let lista = vec![e];

        assert!(
            dentro_de(&lista, (0.0, 0.0, 100.0, 100.0)).is_empty(),
            "girado, se sale de su propia caja"
        );
        assert_eq!(
            dentro_de(&lista, (-30.0, -30.0, 130.0, 130.0)),
            vec![1],
            "con sitio de sobra, si"
        );
    }

    #[test]
    fn la_marquesina_no_coge_los_borrados() {
        let mut a = Elemento {
            id: 1,
            x: 0.0,
            y: 0.0,
            ancho: 10.0,
            alto: 10.0,
            ..base()
        };
        a.borrado = true;
        let lista = vec![a];
        assert!(dentro_de(&lista, (-100.0, -100.0, 100.0, 100.0)).is_empty());
    }

    #[test]
    fn la_marquesina_funciona_igual_arrastrada_en_cualquier_direccion() {
        // La marquesina se arrastra en cualquier direccion, y de derecha a
        // izquierda o de abajo a arriba llega con x1 < x0 o y1 < y0. La
        // normalizacion debe asegurar que da el mismo resultado.
        let dentro = Elemento {
            id: 1,
            x: 10.0,
            y: 10.0,
            ancho: 20.0,
            alto: 20.0,
            ..base()
        };
        let fuera = Elemento {
            id: 2,
            x: 500.0,
            y: 500.0,
            ancho: 10.0,
            alto: 10.0,
            ..base()
        };
        let lista = vec![dentro, fuera];

        let normal = dentro_de(&lista, (0.0, 0.0, 100.0, 100.0));
        let invertido = dentro_de(&lista, (100.0, 100.0, 0.0, 0.0));
        assert_eq!(
            normal, invertido,
            "el sentido del arrastre no debe cambiar el resultado"
        );
    }

    #[test]
    fn elementos_en_los_devuelve_de_arriba_abajo() {
        // El orden de la lista ES el orden de pintado: el ultimo se pinta
        // encima. Al picar, el de encima va primero, que es lo que el usuario
        // cree que esta tocando.
        let a = Elemento {
            id: 1,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            ..base()
        };
        let b = Elemento { id: 2, ..a.clone() };
        let lista = vec![a, b];

        assert_eq!(elementos_en(&lista, Punto2::nuevo(50.0, 50.0)), vec![2, 1]);
    }

    #[test]
    fn elementos_en_y_elemento_en_estan_de_acuerdo() {
        let a = Elemento {
            id: 1,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            ..base()
        };
        let b = Elemento {
            id: 2,
            x: 20.0,
            y: 20.0,
            ..a.clone()
        };
        let lista = vec![a, b];
        let p = Punto2::nuevo(50.0, 50.0);

        assert_eq!(
            elemento_en(&lista, p),
            elementos_en(&lista, p).first().copied(),
            "el primero de la lista es el que devuelve elemento_en"
        );
    }

    #[test]
    fn las_esquinas_giradas_de_un_elemento_sin_giro_son_su_caja() {
        let e = Elemento {
            x: 10.0,
            y: 20.0,
            ancho: 30.0,
            alto: 40.0,
            angulo: 0.0,
            ..base()
        };
        let c = esquinas_giradas(&e);
        let (x0, y0, x1, y1) = e.caja();
        assert_eq!(c[0], Punto2::nuevo(x0, y0));
        assert_eq!(c[2], Punto2::nuevo(x1, y1));
    }

    #[test]
    fn estar_dentro_de_la_caja_no_es_lo_mismo_que_tocar() {
        // La diferencia que justifica que existan las dos: en medio de un
        // rectangulo VACIO no se pica nada —el clic tiene que llegar al texto
        // de debajo— pero la punta de una flecha soltada ahi si esta
        // «encima» de la caja y tiene que atarse a ella.
        let vacio = Elemento {
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            relleno: None,
            ..base()
        };
        let en_medio = Punto2::nuevo(50.0, 50.0);
        assert!(
            !toca(&vacio, en_medio),
            "una figura vacia se toca por el borde"
        );
        assert!(dentro_de_la_caja_girada(&vacio, en_medio));

        // Caso negativo: fuera es fuera, sin tolerancia ninguna. Con el
        // margen de `toca` (seis pixeles) este punto si contaria.
        assert!(!dentro_de_la_caja_girada(
            &vacio,
            Punto2::nuevo(103.0, 50.0)
        ));
    }

    #[test]
    fn dentro_de_la_caja_deshace_el_giro_del_elemento() {
        // Sin deshacerlo, un cuadrado a 45 grados aceptaria puntos de las
        // esquinas de su caja sin girar, por las que la figura no pasa.
        let mut e = Elemento {
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            ..base()
        };
        e.angulo = std::f32::consts::FRAC_PI_4;
        // La esquina (0,0) sin girar queda fuera de la figura girada.
        assert!(!dentro_de_la_caja_girada(&e, Punto2::nuevo(0.0, 0.0)));
        // Y el centro sigue dentro, gire lo que gire.
        assert!(dentro_de_la_caja_girada(&e, Punto2::nuevo(50.0, 50.0)));
    }
}
