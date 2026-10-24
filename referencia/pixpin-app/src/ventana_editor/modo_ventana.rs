//! **El lienzo en ventana o a pantalla completa** (14-oct-2026, el usuario:
//! «haz que los canvas puedan pasar a estar en ventana en vez de pantalla
//! completa, que use la misma barra que los pines, la barra superior»).
//!
//! En ventana, el lienzo es un pin mas: la ventana y su barra de fuera son
//! las de los visores (`lector_pin::PinLector`), no otra barra. Aqui solo
//! esta lo que es del lienzo:
//!
//! - **Lo que se recuerda**: el modo (F11 o el boton de la barra) y el
//!   sitio de la ventana, UNO para todos los lienzos y entre sesiones
//!   (`estado.toml`). Por lienzo no: un lienzo nuevo no tendria sitio, y
//!   quien trabaja en ventana la quiere donde la dejo, sea cual sea la hoja.
//!   Nunca tocado, a pantalla completa, como siempre.
//! - **Donde abre**: en el sitio recordado si sigue cayendo en un monitor
//!   (se pudo desenchufar), si no centrado en el del raton.
//! - **Lo minimo**: lo que deja caber la barra de herramientas partida en
//!   dos filas, con su aire; el panel de la izquierda ya se reparte en
//!   columnas cuando no le cabe el alto.
//! - **Que pasa con la vista al cambiar la ventana**: lo dibujado se queda
//!   quieto en la pantalla (estirar por la izquierda, pasar a pantalla
//!   completa y volver), salvo al llevarla por el asa, que se va con ella.
//!
//! Android no tiene nada equivalente: su lienzo es una actividad que ocupa
//! la pantalla. Esto es cosa de Windows.

use std::sync::Mutex;

use pixpin_geom::Rect;
use pixpin_motor2d::camara::Camara;
use pixpin_ui::{BotonCaja, CajaHerramientas};

/// Lo que se recuerda del lienzo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Recordado {
    pub pantalla_completa: bool,
    /// El sitio en ventana (pixeles fisicos del escritorio).
    pub ventana: Option<Rect>,
}

impl Default for Recordado {
    fn default() -> Self {
        Recordado {
            pantalla_completa: true,
            ventana: None,
        }
    }
}

/// Lo recordado en esta sesion; `None` hasta leerlo del disco la primera vez.
static RECORDADO: Mutex<Option<Recordado>> = Mutex::new(None);

/// Lo recordado en `estado.toml`, con lo de fabrica en lo que no este.
pub(crate) fn de_estado(e: &pixpin_store::estado::Estado) -> Recordado {
    Recordado {
        pantalla_completa: e.lienzo_pantalla_completa.unwrap_or(true),
        ventana: e
            .lienzo_ventana
            .filter(|v| v[2] > 0 && v[3] > 0)
            .map(|[x, y, w, h]| Rect {
                x,
                y,
                ancho: w as u32,
                alto: h as u32,
            }),
    }
}

/// Lo de ahora, para el lienzo que se abre.
pub(crate) fn recordado() -> Recordado {
    let mut r = RECORDADO.lock().unwrap_or_else(|e| e.into_inner());
    *r.get_or_insert_with(|| {
        crate::oscuro::ubicacion()
            .map(|u| de_estado(&pixpin_store::estado::cargar(&u)))
            .unwrap_or_default()
    })
}

/// Lo apunta para el siguiente lienzo y lo guarda, sin tocar lo demas del
/// estado. Si no cambio nada, no escribe.
pub(crate) fn recordar(nuevo: Recordado) {
    {
        let mut r = RECORDADO.lock().unwrap_or_else(|e| e.into_inner());
        if *r == Some(nuevo) {
            return;
        }
        *r = Some(nuevo);
    }
    let Some(u) = crate::oscuro::ubicacion() else {
        return;
    };
    let mut estado = pixpin_store::estado::cargar(&u);
    estado.lienzo_pantalla_completa = Some(nuevo.pantalla_completa);
    if let Some(v) = nuevo.ventana {
        estado.lienzo_ventana = Some([v.x, v.y, v.ancho as i32, v.alto as i32]);
    }
    if let Err(e) = pixpin_store::estado::guardar(&u, &estado) {
        // Solo se pierde que se recuerde.
        tracing::warn!(?e, "no se pudo guardar como queda el lienzo");
    }
}

/// **Lo minimo de la ventana del lienzo**, en pixeles fisicos: la barra de
/// herramientas partida en dos filas con su aire a los lados, y un alto en
/// el que aun se dibuja con la barra y el panel a la vista.
pub(crate) fn minimo(escala_por_cien: u32, botones: &'static [BotonCaja]) -> (u32, u32) {
    let e = |v: u32| v * escala_por_cien.max(25) / 100;
    // Con un area de un pixel de ancho la barra se parte siempre en dos.
    let estrecha = Rect {
        x: 0,
        y: 0,
        ancho: 1,
        alto: 4000,
    };
    let barra = CajaHerramientas::barra_superior(estrecha, escala_por_cien, botones).marco;
    let ancho = (barra.ancho + 2 * e(16)).max(e(480));
    let alto = (barra.alto + e(16) + e(300)).max(e(400));
    (ancho, alto)
}

/// **Donde va el panel de la izquierda** (y su boton de mostrar y quitar,
/// encima): el `area` de la interfaz, salvo que la barra de herramientas
/// les caiga encima (en una ventana estrecha, donde se parte en dos filas o
/// llega hasta la izquierda): entonces empiezan debajo de ella. El panel ya
/// se reparte en columnas con el alto que le quede.
pub(crate) fn area_del_panel(area: Rect, barra: Rect, escala_por_cien: u32) -> Rect {
    let e = |v: u32| (v * escala_por_cien / 100) as i32;
    // Donde empieza el boton de quitar el panel (16, `panel_lateral::
    // boton_plegar`) y lo que ocupa a lo ancho (16 de margen y el panel).
    let (arriba, ancho) = (e(16), e(16 + pixpin_ui::panel_lateral::ANCHO_LOGICO));
    let pisa = barra.x < area.x + ancho && barra.derecha() > area.x;
    let falta = barra.abajo() + e(8) - (area.y + arriba);
    if !pisa || falta <= 0 {
        return area;
    }
    let falta = falta.min(area.alto as i32 / 2);
    Rect {
        y: area.y + falta,
        alto: area.alto - falta as u32,
        ..area
    }
}

/// El sitio de la ventana la primera vez: el 80 % del area de trabajo,
/// centrado (como abria antes el lienzo en ventana).
pub(crate) fn por_defecto(trabajo: Rect) -> Rect {
    let (w, h) = (trabajo.ancho * 4 / 5, trabajo.alto * 4 / 5);
    Rect {
        x: trabajo.x + ((trabajo.ancho - w) / 2) as i32,
        y: trabajo.y + ((trabajo.alto - h) / 2) as i32,
        ancho: w,
        alto: h,
    }
}

/// **Donde abre la ventana**: la recordada si de ella se ve lo bastante en
/// alguno de los monitores (`trabajos`, sus areas de trabajo) para cogerla
/// por el asa; si no, `defecto`. Nunca menor que `minimo` ni mayor que el
/// monitor en el que cae.
pub(crate) fn sitio_al_abrir(
    recordada: Option<Rect>,
    trabajos: &[Rect],
    defecto: Rect,
    minimo: (u32, u32),
) -> Rect {
    // Lo que tiene que asomar: la esquina de arriba, donde va el asa.
    const VISIBLE: (u32, u32) = (120, 60);
    let cabe = |r: &Rect| {
        trabajos.iter().find(|t| {
            r.ancho <= t.ancho
                && r.alto <= t.alto
                && t.interseccion(*r)
                    .is_some_and(|i| i.ancho >= VISIBLE.0 && i.alto >= VISIBLE.1)
        })
    };
    let r = recordada.filter(|r| cabe(r).is_some()).unwrap_or(defecto);
    let r = Rect {
        ancho: r.ancho.max(minimo.0),
        alto: r.alto.max(minimo.1),
        ..r
    };
    // Si al crecer hasta el minimo ya no cabe en su monitor, a su medida.
    match trabajos.iter().find(|t| t.interseccion(r).is_some()) {
        Some(t) => Rect {
            ancho: r.ancho.min(t.ancho.max(minimo.0)),
            alto: r.alto.min(t.alto.max(minimo.1)),
            ..r
        },
        None => r,
    }
}

/// **La camara cuando la ventana pasa de `antes` a `ahora`**. Si solo se
/// movio (llevada por el asa), lo dibujado se va con ella: la camara no
/// cambia. Si cambio de tamano (estirada, a pantalla completa y de vuelta),
/// lo dibujado se queda quieto en la pantalla: la camara corre lo mismo que
/// la esquina de la ventana. Ida y vuelta deja la misma vista.
pub(crate) fn camara_al_cambiar(
    camara: Camara,
    zoom_efectivo: f32,
    antes: Rect,
    ahora: Rect,
) -> Camara {
    let solo_movida = (antes.ancho, antes.alto) == (ahora.ancho, ahora.alto);
    if solo_movida || zoom_efectivo <= 0.0 || !zoom_efectivo.is_finite() {
        return camara;
    }
    Camara {
        x: camara.x + (ahora.x - antes.x) as f32 / zoom_efectivo,
        y: camara.y + (ahora.y - antes.y) as f32 / zoom_efectivo,
        ..camara
    }
}

/// Lo que hace el lienzo con lo que se pulso en la barra del pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeLaBarra {
    /// Pantalla completa o de vuelta a ventana (lo mismo que F11).
    Alternar,
    /// Siempre encima o no.
    Fijar,
    /// Cerrar el lienzo guardando: lo mismo que Escape o Salir.
    Cerrar,
    /// El lapiz: de solo mirar a editar y vuelta (19-oct-2026).
    Lapiz,
    /// Nada (el nombre de una hoja se cambia en el chat).
    Nada,
}

/// El boton propio del lapiz en la barra del pin del lienzo.
pub(crate) const BOTON_LAPIZ: u8 = 1;

pub(crate) fn de_la_barra(a: crate::lector_pin::AccionPin) -> DeLaBarra {
    use crate::lector_pin::AccionPin;
    match a {
        AccionPin::PantallaCompleta => DeLaBarra::Alternar,
        AccionPin::Fijar => DeLaBarra::Fijar,
        AccionPin::Cerrar => DeLaBarra::Cerrar,
        AccionPin::Propio(BOTON_LAPIZ) => DeLaBarra::Lapiz,
        AccionPin::Nombre | AccionPin::Propio(_) => DeLaBarra::Nada,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const MONITOR: Rect = Rect {
        x: 0,
        y: 0,
        ancho: 1920,
        alto: 1040,
    };

    #[test]
    fn sin_tocar_abre_a_pantalla_completa_y_lo_guardado_manda() {
        let nada = pixpin_store::estado::Estado::default();
        assert_eq!(de_estado(&nada), Recordado::default());
        assert!(de_estado(&nada).pantalla_completa);
        let e = pixpin_store::estado::Estado {
            lienzo_pantalla_completa: Some(false),
            lienzo_ventana: Some([100, 80, 900, 600]),
            ..Default::default()
        };
        let r = de_estado(&e);
        assert!(!r.pantalla_completa);
        assert_eq!(
            r.ventana,
            Some(Rect {
                x: 100,
                y: 80,
                ancho: 900,
                alto: 600
            })
        );
        // Caso negativo: un tamano sin sentido no se recuerda.
        let roto = pixpin_store::estado::Estado {
            lienzo_ventana: Some([0, 0, -5, 0]),
            ..Default::default()
        };
        assert_eq!(de_estado(&roto).ventana, None);
    }

    #[test]
    fn abre_donde_se_dejo_si_sigue_en_un_monitor() {
        let defecto = por_defecto(MONITOR);
        let dejada = Rect {
            x: 200,
            y: 100,
            ancho: 1000,
            alto: 700,
        };
        assert_eq!(
            sitio_al_abrir(Some(dejada), &[MONITOR], defecto, (600, 400)),
            dejada
        );
        // Caso negativo: en un monitor que ya no esta, al de por defecto.
        let fuera = Rect {
            x: -2500,
            y: 100,
            ancho: 1000,
            alto: 700,
        };
        assert_eq!(
            sitio_al_abrir(Some(fuera), &[MONITOR], defecto, (600, 400)),
            defecto
        );
        // Ni asomando solo un pico por el borde.
        let pico = Rect { x: 1900, ..dejada };
        assert_eq!(
            sitio_al_abrir(Some(pico), &[MONITOR], defecto, (600, 400)),
            defecto
        );
        // Mas pequena que el minimo (de una version de antes): crece.
        let chica = Rect {
            ancho: 300,
            alto: 200,
            ..dejada
        };
        let r = sitio_al_abrir(Some(chica), &[MONITOR], defecto, (600, 400));
        assert_eq!((r.x, r.y, r.ancho, r.alto), (200, 100, 600, 400));
        // La primera vez, centrada y al 80 %.
        assert_eq!(
            sitio_al_abrir(None, &[MONITOR], defecto, (600, 400)),
            defecto
        );
        assert_eq!((defecto.ancho, defecto.alto), (1536, 832));
        assert_eq!((defecto.x, defecto.y), (192, 104));
    }

    #[test]
    fn el_minimo_deja_caber_la_barra_en_dos_filas() {
        let botones =
            crate::dibujo::permitidas::botones(crate::dibujo::permitidas::Anfitrion::Lienzo);
        for escala in [100, 150] {
            let (w, h) = minimo(escala, botones);
            let area = Rect {
                x: 0,
                y: 0,
                ancho: w,
                alto: h,
            };
            let barra = CajaHerramientas::barra_superior(area, escala, botones).marco;
            assert!(
                barra.x >= 0 && barra.derecha() <= w as i32,
                "la barra cabe a {escala} %: {barra:?} en {w}"
            );
            assert!(
                barra.abajo() < h as i32 / 2,
                "y queda sitio para dibujar a {escala} %"
            );
            // Caso negativo: una pizca mas estrecha que la barra ya no cabe
            // (el minimo no es de adorno).
            let justa = Rect {
                ancho: barra.ancho - 1,
                ..area
            };
            let b = CajaHerramientas::barra_superior(justa, escala, botones).marco;
            assert!(b.derecha() > justa.derecha() || b.ancho > justa.ancho);
        }
        // A mas escala, mas minimo.
        assert!(minimo(150, botones).0 > minimo(100, botones).0);
    }

    #[test]
    fn en_una_ventana_estrecha_el_panel_no_pisa_la_barra() {
        use pixpin_motor2d::gesto::{Gesto, Herramienta};
        let botones =
            crate::dibujo::permitidas::botones(crate::dibujo::permitidas::Anfitrion::Lienzo);
        let escena = pixpin_motor2d::escena::Escena::nueva();
        let mut gesto = Gesto::nuevo();
        crate::dibujo::teclas::elegir_herramienta(&mut gesto, Herramienta::Rectangulo);
        let (w, h) = minimo(100, botones);
        let chica = Rect {
            x: 500,
            y: 300,
            ancho: w,
            alto: h,
        };
        let barra = CajaHerramientas::barra_superior(chica, 100, botones).marco;
        crate::panel_dibujo::con_boton_plegar(true);
        let panel = crate::panel_dibujo::panel_para(
            &gesto,
            &escena,
            area_del_panel(chica, barra, 100),
            100,
        )
        .expect("el rectangulo tiene panel");
        assert!(
            panel.marco.y >= barra.abajo(),
            "panel {:?} bajo la barra {barra:?}",
            panel.marco
        );
        let boton = panel.plegar.expect("con su boton de quitarlo");
        assert!(
            boton.y >= barra.abajo(),
            "el boton {boton:?} tambien bajo la barra"
        );
        assert!(
            panel.marco.abajo() <= chica.abajo(),
            "y dentro de la ventana"
        );
        // Caso negativo: con la barra en una fila no se mueve nada (el panel
        // sigue donde siempre en pantalla completa).
        let grande = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let barra = CajaHerramientas::barra_superior(grande, 100, botones).marco;
        assert_eq!(area_del_panel(grande, barra, 100), grande);
    }

    #[test]
    fn la_vista_no_se_pierde_al_ir_y_volver_de_pantalla_completa() {
        let c = Camara {
            x: 50.0,
            y: -20.0,
            zoom: 1.5,
        };
        let ventana = Rect {
            x: 300,
            y: 200,
            ancho: 1000,
            alto: 700,
        };
        let a_pc = camara_al_cambiar(c, 1.5, ventana, MONITOR);
        // Lo que estaba en la esquina de la ventana sigue en el mismo sitio
        // de la pantalla: (300, 200) del escritorio.
        let mundo = |c: &Camara, area: Rect, px: i32, py: i32| {
            (
                c.x + (px - area.x) as f32 / 1.5,
                c.y + (py - area.y) as f32 / 1.5,
            )
        };
        assert_eq!(
            mundo(&c, ventana, 300, 200),
            mundo(&a_pc, MONITOR, 300, 200)
        );
        let vuelta = camara_al_cambiar(a_pc, 1.5, MONITOR, ventana);
        assert!((vuelta.x - c.x).abs() < 1e-4 && (vuelta.y - c.y).abs() < 1e-4);
        assert_eq!(vuelta.zoom, c.zoom);
        // Estirar por la derecha no mueve nada.
        let mas_ancha = Rect {
            ancho: 1200,
            ..ventana
        };
        assert_eq!(camara_al_cambiar(c, 1.5, ventana, mas_ancha), c);
        // Caso negativo: llevarla por el asa se lleva el dibujo con ella.
        let movida = Rect {
            x: 10,
            y: 10,
            ..ventana
        };
        assert_eq!(camara_al_cambiar(c, 1.5, ventana, movida), c);
        // Ni un zoom roto la mueve.
        assert_eq!(camara_al_cambiar(c, 0.0, ventana, MONITOR), c);
    }

    #[test]
    fn cerrar_desde_la_barra_es_salir_guardando() {
        use crate::lector_pin::AccionPin;
        assert_eq!(de_la_barra(AccionPin::Cerrar), DeLaBarra::Cerrar);
        assert_eq!(
            de_la_barra(AccionPin::PantallaCompleta),
            DeLaBarra::Alternar
        );
        assert_eq!(de_la_barra(AccionPin::Fijar), DeLaBarra::Fijar);
        // Caso negativo: el nombre y los propios no hacen nada en el lienzo.
        assert_eq!(de_la_barra(AccionPin::Nombre), DeLaBarra::Nada);
        assert_eq!(de_la_barra(AccionPin::Propio(3)), DeLaBarra::Nada);
    }
}

#[cfg(test)]
mod muestras {
    use super::*;
    use crate::dibujo::pintar::{a_color, dibujar_orden, por_cada_orden};
    use crate::imagenes_lienzo::ImagenesLienzo;
    use pixpin_motor2d::cache::Cache;
    use pixpin_motor2d::escena::Escena;
    use pixpin_motor2d::gesto::{Gesto, Herramienta};
    use pixpin_motor2d::vector::Punto2;
    use pixpin_motor2d::{ColorRgba, Elemento, EstiloTrazo, Figura};
    use pixpin_render::{Color, MotorRender, RectF};

    fn elemento(
        id: u64,
        figura: Figura,
        x: f32,
        y: f32,
        ancho: f32,
        alto: f32,
        trazo: ColorRgba,
    ) -> Elemento {
        Elemento {
            id,
            figura,
            x,
            y,
            ancho,
            alto,
            angulo: 0.0,
            trazo,
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: id as u32 + 7,
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

    fn escena() -> Escena {
        let mut s = Escena::nueva();
        let azul = ColorRgba::opaco(0.11, 0.38, 0.85);
        let rojo = ColorRgba::opaco(0.88, 0.19, 0.19);
        s.anadir(elemento(
            1,
            Figura::Rectangulo,
            40.0,
            60.0,
            260.0,
            160.0,
            azul,
        ));
        s.anadir(elemento(2, Figura::Elipse, 360.0, 90.0, 200.0, 140.0, rojo));
        s.anadir(elemento(
            3,
            Figura::Rombo,
            120.0,
            280.0,
            180.0,
            150.0,
            ColorRgba::opaco(0.1, 0.55, 0.3),
        ));
        let n = 80;
        let puntos: Vec<Punto2> = (0..n)
            .map(|i| {
                let t = i as f32 / (n - 1) as f32;
                Punto2::nuevo(t * 360.0, (t * 12.0).sin() * 40.0)
            })
            .collect();
        let mut trazo = elemento(
            4,
            Figura::Lapiz {
                puntos,
                presiones: vec![0.5; n],
                opciones: Some(Default::default()),
            },
            360.0,
            330.0,
            360.0,
            80.0,
            ColorRgba::opaco(0.1, 0.1, 0.12),
        );
        trazo.grosor = 3.0;
        s.anadir(trazo);
        s
    }

    /// Una ventana del lienzo en el «escritorio» `p`: el dibujo con la vista
    /// en `camara`, la barra de herramientas y el panel dentro, y la barra
    /// del pin encima, fuera, pegada a su izquierda.
    fn pintar_ventana(
        p: &pixpin_render::Pintor,
        escritorio: Rect,
        ventana: Rect,
        camara: Camara,
        nombre: &str,
        pantalla_completa: bool,
    ) {
        let escena = escena();
        let botones =
            crate::dibujo::permitidas::botones(crate::dibujo::permitidas::Anfitrion::Lienzo);
        let r = RectF {
            x: ventana.x as f32,
            y: ventana.y as f32,
            ancho: ventana.ancho as f32,
            alto: ventana.alto as f32,
        };
        p.empujar_recorte(r);
        p.rellenar(r, a_color(escena.fondo));
        p.poner_vista(
            (0.0, 0.0),
            camara.zoom,
            (
                ventana.x as f32 - camara.x * camara.zoom,
                ventana.y as f32 - camara.y * camara.zoom,
            ),
        );
        let mut cache = Cache::nueva();
        let imagenes = ImagenesLienzo::nuevo(1);
        let vista = (
            camara.x,
            camara.y,
            camara.x + ventana.ancho as f32 / camara.zoom,
            camara.y + ventana.alto as f32 / camara.zoom,
        );
        for e in escena.visibles() {
            por_cada_orden(&mut cache, e, camara.zoom, None, |o| {
                dibujar_orden(p, o, vista, None, &imagenes, camara.zoom, None);
            });
        }
        p.desplazar(0.0, 0.0);
        let caja = CajaHerramientas::barra_superior(ventana, 100, botones);
        crate::caja_dibujo::pintar_barra(p, &caja, Herramienta::Rectangulo, 100, None, |_| None);
        let mut gesto = Gesto::nuevo();
        crate::dibujo::teclas::elegir_herramienta(&mut gesto, Herramienta::Rectangulo);
        if let Some(panel) = crate::panel_dibujo::panel_para(
            &gesto,
            &escena,
            area_del_panel(ventana, caja.marco, 100),
            100,
        ) {
            crate::panel_dibujo::pintar(p, &panel, 100);
        }
        p.soltar_recorte();
        let (w, h) = crate::lector_pin::medida_de_la_barra(1.0, nombre, "pixpin", false);
        let (bx, by) = crate::lector_pin::colocar(ventana, escritorio, w, h, 8);
        crate::lector_pin::pintar_barra_de_muestra(
            p,
            (bx as f32, by as f32),
            1.0,
            nombre,
            "pixpin",
            false,
            pantalla_completa,
        );
    }

    /// **El lienzo en ventana con la barra de los pines**, a PNG, en dos
    /// tamanos: uno corriente y el minimo (la barra de herramientas en dos
    /// filas y el panel en columnas). Y el mismo lienzo a pantalla completa
    /// con la barra asomando dentro.
    /// `cargo test -p pixpin --bin pixpinmax muestra_del_lienzo_en_ventana -- --ignored --nocapture`
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_lienzo_en_ventana() {
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let botones =
            crate::dibujo::permitidas::botones(crate::dibujo::permitidas::Anfitrion::Lienzo);
        let (mw, mh) = minimo(100, botones);
        let escritorio = Rect {
            x: 0,
            y: 0,
            ancho: 1100 + 40 + mw + 40,
            alto: 120 + 720.max(mh) + 40,
        };
        let fondo = Color {
            r: 0.24,
            g: 0.30,
            b: 0.38,
            a: 1.0,
        };
        let guardar = |nombre: &str, fuera: &FueraDePantalla| {
            fuera.esperar_gpu().expect("esperar");
            let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
            let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
                ancho,
                alto,
                pixeles,
            })
            .expect("png");
            let salida = carpeta.join(nombre);
            std::fs::write(&salida, png).expect("guardar");
            println!("{}", salida.display());
        };
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), escritorio.ancho, escritorio.alto)
            .expect("superficie");
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(fondo);
                let grande = Rect {
                    x: 40,
                    y: 120,
                    ancho: 1100,
                    alto: 720,
                };
                pintar_ventana(
                    p,
                    escritorio,
                    grande,
                    Camara {
                        x: -40.0,
                        y: -110.0,
                        zoom: 1.0,
                    },
                    "Plano de la casa",
                    false,
                );
                let chica = Rect {
                    x: 40 + 1100 + 40,
                    y: 120,
                    ancho: mw,
                    alto: mh,
                };
                pintar_ventana(
                    p,
                    escritorio,
                    chica,
                    Camara {
                        x: 0.0,
                        y: -110.0,
                        zoom: 0.6,
                    },
                    "Minima",
                    false,
                );
            })
            .expect("pintar");
        guardar("lienzo-en-ventana.png", &fuera);
        // A pantalla completa: la ventana es el escritorio y la barra va
        // dentro, arriba a la izquierda.
        let monitor = Rect {
            x: 0,
            y: 0,
            ancho: 1440,
            alto: 900,
        };
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), monitor.ancho, monitor.alto)
            .expect("superficie");
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(fondo);
                pintar_ventana(
                    p,
                    monitor,
                    monitor,
                    Camara {
                        x: -120.0,
                        y: -140.0,
                        zoom: 1.0,
                    },
                    "Plano de la casa",
                    true,
                );
            })
            .expect("pintar");
        guardar("lienzo-a-pantalla-completa.png", &fuera);
    }
}
