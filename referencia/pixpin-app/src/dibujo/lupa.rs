//! **La lupa del lienzo**: la varita que convierte una figura cerrada en una
//! lupa, y el arrastre que apunta la lupa a otro sitio (`Tool.LUPA` y
//! `Gesture.MoviendoElFoco` de `DrawController.kt`).
//!
//! La geometria —que contorno, a donde mira, cuanto agranda— es de
//! `pixpin_motor2d::lupa_elemento`, puro y con sus pruebas. Aqui solo se
//! decide que hace cada clic con la escena, que es lo que no puede vivir en
//! el motor (necesita saber que herramienta hay y que esta elegido) ni en el
//! editor (el lector y el pin comparten esta mano).
//!
//! # Por que es una varita y no un recuadro
//!
//! El movil la cambio asi a proposito: se dibuja lo que sea con las
//! herramientas de siempre —un circulo, un rombo, un garabato cerrado— y se
//! toca con la lupa: ese mismo contorno pasa a ensenar en grande lo que hay
//! debajo. Sobre el vacio no hace nada, porque no hay nada que convertir.
//! La figura tocada se va con ella (se ha convertido, no duplicado), y en un
//! solo paso de deshacer.

use pixpin_geom::Punto;
use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{Gesto, Herramienta};
use pixpin_motor2d::lupa_elemento as lupa;
use pixpin_motor2d::vector::Punto2;
use pixpin_shell::overlay::EventoOverlay;

/// Cuanto margen tiene el dedo al coger la zona mirada, en pixeles de
/// pantalla (`UMBRAL_GUIA` del movil).
const MARGEN_PX: f32 = 10.0;

/// El arrastre de la zona mirada en curso: que lupa, desde donde y a donde
/// miraba al empezar.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Moviendo {
    id: u64,
    inicio: Punto2,
    foco: Punto2,
}

/// Lo que la mano recuerda de la lupa entre un evento y el siguiente.
#[derive(Debug, Default)]
pub struct ManoDeLupa {
    moviendo: Option<Moviendo>,
}

/// **Convierte la figura `id` en una lupa** con el estilo puesto (la tinta
/// y el grueso de la montura). Devuelve el id de la lupa, ya elegida. La
/// figura de origen se borra en el mismo paso: deshacer la devuelve a su
/// sitio y quita la lupa de una vez.
pub fn convertir(escena: &mut Escena, gesto: &mut Gesto, id: u64) -> Option<u64> {
    let fuente = escena.buscar(id)?.clone();
    // De fabrica, como el movil (`ItemStyle`): el doble, redonda y con
    // flecha. La forma copiada de la figura manda sobre «redonda».
    let (cristal, caja) =
        lupa::desde_figura(&fuente, gesto.estilo.aumento, true, gesto.estilo.guia)?;
    let estilo = &gesto.estilo;
    let nueva = Elemento {
        figura: Figura::Lupa { cristal },
        x: caja.0,
        y: caja.1,
        ancho: caja.2,
        alto: caja.3,
        trazo: estilo.trazo,
        grosor: estilo.grosor.de_forma(),
        opacidad: estilo.opacidad,
        semilla: fuente.semilla,
        ..Default::default()
    };
    escena.abrir_paso();
    escena.borrar_apuntando(id);
    let nuevo = escena.anadir(nueva);
    escena.cerrar_paso();
    gesto.seleccion.limpiar();
    gesto.seleccion.poner(nuevo);
    // Hecha la lupa, lo siguiente es apartarla: con la mano, como el movil
    // (`selectTool(Tool.SELECTION)`).
    gesto.herramienta = Herramienta::Mano;
    Some(nuevo)
}

/// **Convierte la figura `id` en un foco** (`focoDesdeFigura` del movil): el
/// hueco es la figura tal cual (su forma, su sitio, su tamano) y el marco
/// nace `MARCO_DEL_FOCO` veces mas grande alrededor, que es el anillo que se
/// oscurece. El marco se estira luego con sus tiradores y el hueco no se
/// mueve. Como la lupa: la figura se va con el, en un paso, y queda elegido
/// con la mano puesta.
pub fn convertir_en_foco(escena: &mut Escena, gesto: &mut Gesto, id: u64) -> Option<u64> {
    let fuente = escena.buscar(id)?.clone();
    let (mut cristal, caja) = lupa::desde_figura(
        &fuente,
        lupa::MARCO_DEL_FOCO,
        false,
        lupa::GuiaDeLupa::Ninguna,
    )?;
    // Con cuanto oscurece el movil de fabrica (`ItemStyle.oscurecer`).
    cristal.oscurecer = Some(lupa::OSCURECER_POR_DEFECTO);
    let nuevo_foco = Elemento {
        figura: Figura::Foco { cristal },
        x: caja.0,
        y: caja.1,
        ancho: caja.2,
        alto: caja.3,
        trazo: gesto.estilo.trazo,
        grosor: gesto.estilo.grosor.de_forma(),
        semilla: fuente.semilla,
        ..Default::default()
    };
    escena.abrir_paso();
    escena.borrar_apuntando(id);
    let nuevo = escena.anadir(nuevo_foco);
    escena.cerrar_paso();
    gesto.seleccion.limpiar();
    gesto.seleccion.poner(nuevo);
    gesto.herramienta = Herramienta::Mano;
    Some(nuevo)
}

/// La lupa elegida, si hay exactamente una elegida y es lupa.
fn lupa_elegida(gesto: &Gesto, escena: &Escena) -> Option<u64> {
    let ids = gesto.seleccion.ids();
    let [id] = ids else { return None };
    escena
        .buscar(*id)
        .filter(|e| !e.borrado && lupa::de(e).is_some())
        .map(|e| e.id)
}

/// Si el punto coge la zona mirada de la lupa `e` y no su cristal. Solo
/// **fuera del cristal**: una lupa recien puesta esta encima de lo que mira,
/// y ahi arrastrar significa apartarla, que es el gesto que se hace primero.
pub fn coge_la_zona(e: &Elemento, p: Punto2, zoom: f32) -> bool {
    let Some(cr) = lupa::de(e) else { return false };
    let caja = lupa::caja_de(e);
    let margen = MARGEN_PX / zoom.max(0.0001);
    let dentro = lupa::dentro_del_contorno(p, &lupa::puntos_del_cristal(cr, caja));
    !dentro && lupa::toca_el_foco(cr, caja, p, margen)
}

impl ManoDeLupa {
    /// Si se esta apuntando una lupa a otro sitio.
    pub fn ocupada(&self) -> bool {
        self.moviendo.is_some()
    }

    /// **Atiende el evento si es de la lupa.** `Some(repintar)` si era
    /// suyo; `None` si le toca a la mano de siempre. `al_documento` pasa un
    /// punto de la ventana al dibujo; `lienzo` dice si el anfitrion sabe
    /// pintar el contenido de una lupa (en la pantalla la herramienta Lupa
    /// es la lupa viva, que no es esto).
    pub fn atender(
        &mut self,
        ev: &EventoOverlay,
        gesto: &mut Gesto,
        escena: &mut Escena,
        al_documento: impl Fn(Punto) -> Punto2,
        zoom: f32,
        lienzo: bool,
    ) -> Option<bool> {
        // Apuntando: todo el raton es suyo hasta soltar.
        if let Some(m) = self.moviendo {
            return match *ev {
                EventoOverlay::RatonMovido(p) => {
                    let q = al_documento(p);
                    let foco =
                        Punto2::nuevo(m.foco.x + (q.x - m.inicio.x), m.foco.y + (q.y - m.inicio.y));
                    if let Some(e) = escena.buscar_mut(m.id)
                        && let Figura::Lupa { cristal } = &mut e.figura
                    {
                        cristal.foco = Some(foco);
                        e.tocar();
                    }
                    Some(true)
                }
                EventoOverlay::BotonSoltado(_) => {
                    self.moviendo = None;
                    escena.cerrar_paso();
                    Some(true)
                }
                EventoOverlay::Muestra(_) => Some(false),
                _ => None,
            };
        }
        // **El foco es la misma varita** (`Tool.SPOTLIGHT` del movil): toca
        // una figura cerrada y la convierte en un foco que oscurece solo el
        // anillo de su marco. En todos los anfitriones: no necesita pintar
        // nada de dentro, como la lupa.
        if gesto.herramienta == Herramienta::Foco {
            return match *ev {
                EventoOverlay::BotonPulsado(p) => {
                    let q = al_documento(p);
                    Some(
                        lupa::figura_bajo(&escena.elementos, q)
                            .and_then(|id| convertir_en_foco(escena, gesto, id))
                            .is_some(),
                    )
                }
                EventoOverlay::RatonMovido(_)
                | EventoOverlay::BotonSoltado(_)
                | EventoOverlay::Muestra(_) => Some(false),
                _ => None,
            };
        }
        if !lienzo {
            return None;
        }
        match gesto.herramienta {
            // La varita. Sobre el vacio no hace nada, pero el clic es suyo:
            // pasarlo al motor no dibujaria nada y soltaria lo elegido.
            Herramienta::Lupa => match *ev {
                EventoOverlay::BotonPulsado(p) => {
                    let q = al_documento(p);
                    Some(
                        lupa::figura_bajo(&escena.elementos, q)
                            .and_then(|id| convertir(escena, gesto, id))
                            .is_some(),
                    )
                }
                EventoOverlay::RatonMovido(_)
                | EventoOverlay::BotonSoltado(_)
                | EventoOverlay::Muestra(_) => Some(false),
                _ => None,
            },
            // Con la mano y una lupa elegida, la zona mirada se coge antes
            // que la figura de debajo: lo mirado suele caer encima de algo
            // dibujado, y sin esto no habria forma de apuntarla a otro sitio.
            Herramienta::Mano => {
                let EventoOverlay::BotonPulsado(p) = *ev else {
                    return None;
                };
                let id = lupa_elegida(gesto, escena)?;
                let e = escena.buscar(id)?;
                let q = al_documento(p);
                if !coge_la_zona(e, q, zoom) {
                    return None;
                }
                let caja = lupa::caja_de(e);
                let foco = lupa::foco_de(lupa::de(e)?, caja);
                escena.abrir_paso();
                escena.apuntar_edicion(id);
                self.moviendo = Some(Moviendo {
                    id,
                    inicio: q,
                    foco,
                });
                Some(true)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn escena_con_circulo() -> (Escena, u64) {
        let mut e = Escena::nueva();
        let id = e.anadir(Elemento {
            figura: Figura::Elipse,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            grosor: 2.0,
            ..Default::default()
        });
        (e, id)
    }

    fn en(x: i32, y: i32) -> EventoOverlay {
        EventoOverlay::BotonPulsado(Punto { x, y })
    }

    /// La ventana en el origen y la camara a 1:1: el punto es el del dibujo.
    fn directo(p: Punto) -> Punto2 {
        Punto2::nuevo(p.x as f32, p.y as f32)
    }

    #[test]
    fn tocar_un_circulo_con_la_lupa_lo_convierte_y_deshacer_lo_devuelve() {
        let (mut escena, circulo) = escena_con_circulo();
        let mut gesto = Gesto::nuevo();
        gesto.herramienta = Herramienta::Lupa;
        let mut mano = ManoDeLupa::default();
        let r = mano.atender(&en(50, 50), &mut gesto, &mut escena, directo, 1.0, true);
        assert_eq!(r, Some(true));
        let vivos: Vec<_> = escena.visibles().collect();
        assert_eq!(vivos.len(), 1, "el circulo se convirtio, no se duplico");
        assert!(matches!(vivos[0].figura, Figura::Lupa { .. }));
        assert_eq!(
            gesto.herramienta,
            Herramienta::Mano,
            "lo siguiente es apartarla"
        );
        assert_eq!(gesto.seleccion.ids(), &[vivos[0].id]);
        // Un solo paso: deshacer quita la lupa y devuelve el circulo.
        assert!(escena.deshacer());
        let vivos: Vec<_> = escena.visibles().collect();
        assert_eq!(vivos.len(), 1);
        assert_eq!(vivos[0].id, circulo);
    }

    #[test]
    fn tocar_un_circulo_con_el_foco_lo_convierte_en_un_marco_que_oscurece_alrededor() {
        let (mut escena, circulo) = escena_con_circulo();
        let mut gesto = Gesto::nuevo();
        gesto.herramienta = Herramienta::Foco;
        let mut mano = ManoDeLupa::default();
        // Tambien en la pantalla (sin lienzo): el foco no pinta nada dentro.
        let r = mano.atender(&en(50, 50), &mut gesto, &mut escena, directo, 1.0, false);
        assert_eq!(r, Some(true));
        let vivos: Vec<_> = escena.visibles().collect();
        assert_eq!(vivos.len(), 1, "el circulo se convirtio, no se duplico");
        let Figura::Foco { cristal } = &vivos[0].figura else {
            panic!("{:?}", vivos[0].figura);
        };
        // El marco, 1,8 veces el circulo y centrado en el; el hueco, el
        // circulo tal cual y con su forma.
        let (x0, y0, x1, y1) = vivos[0].caja();
        assert!(
            ((x1 - x0) - 100.0 * lupa::MARCO_DEL_FOCO).abs() < 2.0,
            "{x0} {x1}"
        );
        assert!((((x0 + x1) / 2.0) - 50.0).abs() < 1.0 && (((y0 + y1) / 2.0) - 50.0).abs() < 1.0);
        let hueco = lupa::region(cristal, lupa::caja_de(vivos[0]));
        assert!((hueco.2 - 100.0).abs() < 2.0, "{hueco:?}");
        assert!(
            cristal.forma.as_ref().is_some_and(|f| f.len() > 8),
            "con la forma del circulo"
        );
        assert_eq!(
            lupa::oscurecimiento_de(cristal),
            lupa::OSCURECER_POR_DEFECTO
        );
        assert_eq!(gesto.herramienta, Herramienta::Mano);
        // Un solo paso de deshacer.
        assert!(escena.deshacer());
        assert_eq!(escena.visibles().next().unwrap().id, circulo);
        // Caso negativo: sobre el vacio no nace nada.
        gesto.herramienta = Herramienta::Foco;
        let r = mano.atender(&en(900, 900), &mut gesto, &mut escena, directo, 1.0, true);
        assert_eq!(r, Some(false));
        assert_eq!(escena.cuantos_visibles(), 1);
    }

    #[test]
    fn la_lupa_sobre_el_vacio_no_hace_nada_y_en_la_pantalla_no_es_suya() {
        let (mut escena, _) = escena_con_circulo();
        let mut gesto = Gesto::nuevo();
        gesto.herramienta = Herramienta::Lupa;
        let mut mano = ManoDeLupa::default();
        let r = mano.atender(&en(500, 500), &mut gesto, &mut escena, directo, 1.0, true);
        assert_eq!(r, Some(false), "el clic es suyo, pero no cambia nada");
        assert_eq!(escena.cuantos_visibles(), 1);
        assert_eq!(gesto.herramienta, Herramienta::Lupa);
        // En el anotador de pantalla la Lupa es la lupa viva: no se toca.
        let r = mano.atender(&en(50, 50), &mut gesto, &mut escena, directo, 1.0, false);
        assert_eq!(r, None);
        assert!(matches!(
            escena.visibles().next().unwrap().figura,
            Figura::Elipse
        ));
    }

    #[test]
    fn apartada_la_lupa_arrastrar_su_zona_la_apunta_a_otro_sitio_sin_mover_el_cristal() {
        let (mut escena, _) = escena_con_circulo();
        let mut gesto = Gesto::nuevo();
        gesto.herramienta = Herramienta::Lupa;
        let mut mano = ManoDeLupa::default();
        mano.atender(&en(50, 50), &mut gesto, &mut escena, directo, 1.0, true);
        let id = gesto.seleccion.ids()[0];
        // Se aparta el cristal 400 a la derecha (como lo haria la mano).
        escena.mover(id, 400.0, 0.0);
        let antes = escena.buscar(id).unwrap().clone();
        // Pulsar en la zona mirada (el circulo de antes, en 50,50) la coge.
        assert_eq!(
            mano.atender(&en(50, 50), &mut gesto, &mut escena, directo, 1.0, true),
            Some(true)
        );
        assert!(mano.ocupada());
        mano.atender(
            &EventoOverlay::RatonMovido(Punto { x: 80, y: 60 }),
            &mut gesto,
            &mut escena,
            directo,
            1.0,
            true,
        );
        mano.atender(
            &EventoOverlay::BotonSoltado(Punto { x: 80, y: 60 }),
            &mut gesto,
            &mut escena,
            directo,
            1.0,
            true,
        );
        let despues = escena.buscar(id).unwrap();
        let Figura::Lupa { cristal } = &despues.figura else {
            panic!()
        };
        let f = cristal.foco.unwrap();
        assert!(
            (f.x - 80.0).abs() < 0.01 && (f.y - 60.0).abs() < 0.01,
            "{f:?}"
        );
        assert_eq!(
            (despues.x, despues.y),
            (antes.x, antes.y),
            "el cristal no se movio"
        );
        // Y deshacer devuelve el foco a donde miraba.
        assert!(escena.deshacer());
        let Figura::Lupa { cristal } = &escena.buscar(id).unwrap().figura else {
            panic!()
        };
        assert!((cristal.foco.unwrap().x - 50.0).abs() < 0.5);
    }

    #[test]
    fn con_la_lupa_encima_de_lo_que_mira_pulsar_dentro_no_coge_la_zona() {
        let (mut escena, _) = escena_con_circulo();
        let mut gesto = Gesto::nuevo();
        gesto.herramienta = Herramienta::Lupa;
        let mut mano = ManoDeLupa::default();
        mano.atender(&en(50, 50), &mut gesto, &mut escena, directo, 1.0, true);
        // Recien puesta, la zona mirada esta dentro del cristal: pulsar ahi
        // es apartar la lupa, que es cosa de la mano de siempre.
        let r = mano.atender(&en(50, 50), &mut gesto, &mut escena, directo, 1.0, true);
        assert_eq!(r, None);
        assert!(!mano.ocupada());
    }
}
