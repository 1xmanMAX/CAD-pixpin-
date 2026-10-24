//! **La pastilla del anotador de pantalla**: las acciones de la capa, en una
//! ventana aparte que siempre recibe el raton.
//!
//! Es la barra de `capa/CapaPantalla.kt` del movil, con su misma decision de
//! fondo: **dos ventanas y no una**. El lienzo ocupa la pantalla y, al pasar
//! a «atravesar», deja de recibir el raton (`FLAG_NOT_TOUCHABLE` alli,
//! `WS_EX_TRANSPARENT` + `WS_EX_LAYERED` aqui: `VentanaOverlay::poner_pasante`)
//! con la tinta todavia a la vista. Si la barra fuera parte del lienzo, al
//! atravesar se volveria intocable con el y no habria forma de volver. Por
//! eso va en su propia ventana, abajo en el centro como la del movil
//! (`Gravity.BOTTOM | CENTER_HORIZONTAL`, 12 dp), que:
//!
//! - **nunca coge el foco** (`WS_EX_NOACTIVATE`, el `FLAG_NOT_FOCUSABLE` de
//!   alli): pulsarla con el clic a traves puesto no le quita el teclado a la
//!   aplicacion en la que se esta trabajando;
//! - tiene las acciones del movil —atravesar, limpiar, copiar, cerrar— y
//!   la que pidio el usuario para el PC: **guardar en «Mensajes guardados»**
//!   (`anotador_al_chat`).
//!
//! Las herramientas siguen en la barra de arriba del anotador, la del lienzo
//! (`dibujo::permitidas`, agrupada), y como en el movil desaparecen al
//! atravesar: no se puede dibujar, y ensenarlas seria ofrecer algo que no
//! responde.
//!
//! **Volver a dibujar** con el clic a traves puesto: el boton de atravesar de
//! la pastilla (que sigue recibiendo el raton), o **Alt + doble clic
//! central** otra vez, en cualquier sitio (`pixpin_shell::gestos::
//! EscuchaAnotador`). La pastilla lo dice debajo de los botones.
//!
//! Una diferencia con el movil, pedida por el usuario: **limpiar se puede
//! deshacer** (alli no, «la capa es un borrador»). Aqui el anotador es el
//! editor entero, con su historial, y un Ctrl+Z que no devuelve cinco
//! minutos de flechas seria el peor fallo posible.

use pixpin_geom::{Monitor, Rect};
use pixpin_render::{Color, Pintor, RectF};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};

/// Lo que hace cada boton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Accion {
    /// El clic a traves: el raton va a lo de debajo, la tinta se sigue viendo.
    Atravesar,
    /// Borrar toda la tinta (se deshace con Ctrl+Z).
    Limpiar,
    /// La pantalla con lo anotado, al portapapeles.
    Copiar,
    /// La captura de debajo con la tinta editable, a «Mensajes guardados»,
    /// AHORA. Salir tambien guarda (2026-09-29); esto es para tenerlo ya en
    /// el chat sin salir. Lo que se dibuje despues pone al dia ese lienzo.
    Guardar,
    /// Salir (lo mismo que Escape).
    Salir,
}

impl Accion {
    pub const TODAS: [Accion; 5] = [
        Accion::Atravesar,
        Accion::Limpiar,
        Accion::Copiar,
        Accion::Guardar,
        Accion::Salir,
    ];
}

const LADO: f32 = 36.0;
const HUECO: f32 = 4.0;
const SEPARADOR: f32 = 12.0;
const ALTO_ROTULO: f32 = 18.0;
const TAM_ROTULO: f32 = 11.0;
/// Separacion del borde de abajo del area de trabajo (los 12 dp del movil).
const MARGEN_ABAJO: f32 = 12.0;

/// **Pura**: la medida de la ventana, en pixeles fisicos. `ancho_rotulo` es
/// lo que mide el rotulo mas largo a la escala (lo mide quien tiene el motor).
pub(crate) fn medida(escala_por_cien: u32, ancho_rotulo: f32) -> (u32, u32) {
    let k = escala_por_cien as f32 / 100.0;
    let botones = HUECO * k + Accion::TODAS.len() as f32 * (LADO + HUECO) * k + SEPARADOR * k;
    let ancho = botones.max(ancho_rotulo + 2.0 * LADO / 2.0 * k);
    let alto = (LADO + 2.0 * HUECO + ALTO_ROTULO) * k;
    (ancho.ceil() as u32, alto.ceil() as u32)
}

/// **Pura**: donde va la ventana: abajo en el centro del area de trabajo del
/// monitor principal (sin tapar la barra de tareas), como la del movil.
pub(crate) fn marco(principal: &Monitor, (ancho, alto): (u32, u32)) -> Rect {
    let k = principal.escala_por_cien as f32 / 100.0;
    let t = principal.area_trabajo;
    Rect {
        x: t.x + (t.ancho as i32 - ancho as i32) / 2,
        y: t.abajo() - alto as i32 - (MARGEN_ABAJO * k).round() as i32,
        ancho,
        alto,
    }
}

/// **Pura**: los botones, en pixeles de la ventana, centrados en su ancho.
pub(crate) fn botones(escala_por_cien: u32, ancho_ventana: u32) -> Vec<(Accion, RectF)> {
    let k = escala_por_cien as f32 / 100.0;
    let fila = HUECO * k + Accion::TODAS.len() as f32 * (LADO + HUECO) * k + SEPARADOR * k;
    let mut x = (ancho_ventana as f32 - fila) / 2.0 + HUECO * k;
    let mut v = Vec::with_capacity(Accion::TODAS.len());
    for a in Accion::TODAS {
        // Un respiro antes de salir, como en la pastilla de presentar: es el
        // que no se quiere pulsar sin querer.
        if a == Accion::Salir {
            x += SEPARADOR * k;
        }
        v.push((
            a,
            RectF {
                x,
                y: HUECO * k,
                ancho: LADO * k,
                alto: LADO * k,
            },
        ));
        x += (LADO + HUECO) * k;
    }
    v
}

/// **Pura**: el boton bajo un punto de la ventana (pixeles de la ventana).
pub(crate) fn accion_en(
    x: f32,
    y: f32,
    escala_por_cien: u32,
    ancho_ventana: u32,
) -> Option<Accion> {
    botones(escala_por_cien, ancho_ventana)
        .into_iter()
        .find(|(_, r)| x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto)
        .map(|(a, _)| a)
}

/// **Pura**: borra toda la tinta en UN paso de deshacer. Devuelve cuantos
/// elementos borro; sin nada que borrar no deja paso en el historial (un
/// Ctrl+Z que no hace nada visible se vive como una averia).
pub(crate) fn limpiar(escena: &mut pixpin_motor2d::Escena) -> usize {
    let ids: Vec<u64> = escena.visibles().map(|e| e.id).collect();
    if ids.is_empty() {
        return 0;
    }
    escena.abrir_paso();
    let mut n = 0;
    for id in ids {
        n += usize::from(escena.borrar_apuntando(id));
    }
    escena.cerrar_paso();
    n
}

fn icono_de(a: Accion) -> &'static pixpin_render::icono::Icono {
    use pixpin_render::icono::material as m;
    use pixpin_render::iconos_excalidraw as i;
    match a {
        Accion::Atravesar => &i::HAND_ICON,
        Accion::Limpiar => &i::TRASH_ICON,
        Accion::Copiar => &i::COPY_ICON,
        Accion::Guardar => &m::BOOKMARK_ADD,
        Accion::Salir => &i::CLOSE_ICON,
    }
}

const FONDO: Color = Color {
    r: 0.07,
    g: 0.07,
    b: 0.09,
    a: 0.86,
};

/// **Pinta la pastilla** en una ventana de `ancho x alto`. `pasante` pone
/// el boton de atravesar encendido y cambia el rotulo; `pulsado`, el boton
/// que tiene el raton abajo.
pub(crate) fn pintar(
    p: &Pintor<'_>,
    escala_por_cien: u32,
    (ancho, alto): (u32, u32),
    pasante: bool,
    pulsado: Option<Accion>,
    rotulo: &str,
) {
    let k = escala_por_cien as f32 / 100.0;
    let isla = RectF {
        x: 0.0,
        y: 0.0,
        ancho: ancho as f32,
        alto: alto as f32,
    };
    p.rellenar_redondeado(isla, (LADO / 2.0 + HUECO) * k, FONDO);
    for (a, r) in botones(escala_por_cien, ancho) {
        let encendido = a == Accion::Atravesar && pasante;
        if encendido {
            p.rellenar_redondeado(r, r.alto / 2.0, Color::ACENTO);
        } else if pulsado == Some(a) {
            p.rellenar_redondeado(
                r,
                r.alto / 2.0,
                Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.25,
                },
            );
        }
        let m = 8.0 * k;
        let caja = RectF {
            x: r.x + m,
            y: r.y + m,
            ancho: r.ancho - 2.0 * m,
            alto: r.alto - 2.0 * m,
        };
        p.icono(icono_de(a), caja, Color::BLANCO);
    }
    // Como se vuelve (o como se atraviesa) sin buscar la pastilla.
    let tam = TAM_ROTULO * k;
    let (w, h) = p.medir_texto(rotulo, tam);
    let y = (LADO + 2.0 * HUECO) * k - 2.0 * k + (ALTO_ROTULO * k - h) / 2.0;
    p.texto(
        rotulo,
        (ancho as f32 - w) / 2.0,
        y,
        tam,
        Color {
            r: 0.82,
            g: 0.83,
            b: 0.86,
            a: 1.0,
        },
    );
}

/// **La ventana de la pastilla**, viva mientras lo este el anotador.
pub(crate) struct Pastilla {
    ventana: VentanaOverlay,
    superficie: pixpin_render::Superficie,
    escala: u32,
    medida: (u32, u32),
    /// Los rotulos: dibujando y con el clic a traves.
    rotulos: (String, String),
    pasante: bool,
    pulsado: Option<Accion>,
    sucia: bool,
}

impl Pastilla {
    /// Crea la ventana (sin ensenarla) con el dispositivo y el motor del
    /// anotador: no cuesta otro dispositivo de GPU. `dueno` es la ventana
    /// del anotador: poseida por ella, Windows la deja siempre ENCIMA
    /// (`VentanaOverlay::poner_dueno`), se active o se reordene el anotador.
    pub fn nueva(
        motor: &pixpin_render::MotorRender,
        d3d: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
        principal: &Monitor,
        rotulos: (String, String),
        dueno: windows::Win32::Foundation::HWND,
    ) -> anyhow::Result<Self> {
        let escala = principal.escala_por_cien;
        let k = escala as f32 / 100.0;
        let ancho_rotulo = [&rotulos.0, &rotulos.1]
            .iter()
            .map(|t| motor.medir_texto(t, TAM_ROTULO * k, f32::MAX).0)
            .fold(0.0, f32::max);
        let medida = medida(escala, ancho_rotulo);
        let ventana = VentanaOverlay::nueva(marco(principal, medida))?;
        ventana.poner_sin_activar();
        ventana.poner_dueno(dueno);
        let superficie =
            pixpin_render::Superficie::nueva(motor, d3d, ventana.handle(), medida.0, medida.1)?;
        Ok(Self {
            ventana,
            superficie,
            escala,
            medida,
            rotulos,
            pasante: false,
            pulsado: None,
            sucia: true,
        })
    }

    pub fn handle(&self) -> windows::Win32::Foundation::HWND {
        self.ventana.handle()
    }

    /// La ensena por encima de todo (tambien del anotador) sin activarla.
    pub fn mostrar(&self) {
        self.ventana.mostrar();
        self.ventana.traer_encima();
    }

    pub fn ocultar(&self) {
        self.ventana.ocultar();
    }

    /// El estado del clic a traves, para el boton y el rotulo.
    pub fn poner_pasante(&mut self, pasante: bool) {
        if self.pasante != pasante {
            self.pasante = pasante;
            self.sucia = true;
        }
        // El anotador acaba de cambiar su estilo: que la pastilla siga
        // encima de el.
        self.ventana.traer_encima();
    }

    /// Un evento de SU ventana. Devuelve la accion al SOLTAR sobre el mismo
    /// boton que se pulso, como un boton de Windows: pulsar y arrastrar fuera
    /// es arrepentirse.
    pub fn evento(&mut self, ev: &EventoOverlay) -> Option<Accion> {
        let area = self.ventana.area();
        let local = |p: pixpin_geom::Punto| ((p.x - area.x) as f32, (p.y - area.y) as f32);
        match *ev {
            EventoOverlay::BotonPulsado(p) => {
                let (x, y) = local(p);
                self.pulsado = accion_en(x, y, self.escala, self.medida.0);
                self.sucia = true;
                None
            }
            EventoOverlay::BotonSoltado(p) => {
                let (x, y) = local(p);
                let antes = self.pulsado.take();
                self.sucia |= antes.is_some();
                antes.filter(|a| accion_en(x, y, self.escala, self.medida.0) == Some(*a))
            }
            EventoOverlay::Pintar | EventoOverlay::CambioDpi => {
                self.sucia = true;
                None
            }
            _ => None,
        }
    }

    /// Repinta si hace falta. Es una ventana de 250 x 60: pintarla no se nota.
    pub fn al_dia(&mut self, motor: &pixpin_render::MotorRender) {
        if !std::mem::take(&mut self.sucia) {
            return;
        }
        let Ok(destino) = self.superficie.empezar(motor) else {
            return;
        };
        let rotulo = if self.pasante {
            &self.rotulos.1
        } else {
            &self.rotulos.0
        };
        let _ = motor.dibujar(&destino, |p| {
            p.limpiar_transparente();
            pintar(
                p,
                self.escala,
                self.medida,
                self.pasante,
                self.pulsado,
                rotulo,
            );
        });
        let _ = self.superficie.presentar();
    }
}

impl Drop for Pastilla {
    fn drop(&mut self) {
        // Que no se quede una ventana TOPMOST huerfana en la pantalla.
        self.ventana.ocultar();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::{Elemento, Escena, Figura, Punto2};

    fn monitor(escala: u32) -> Monitor {
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        Monitor {
            id: 1,
            area,
            // La barra de tareas abajo: 48 px.
            area_trabajo: Rect { alto: 1032, ..area },
            escala_por_cien: escala,
            principal: true,
        }
    }

    #[test]
    fn la_pastilla_va_abajo_en_el_centro_sin_tapar_la_barra_de_tareas() {
        for escala in [100, 125, 150] {
            let m = monitor(escala);
            let medida = medida(escala, 180.0);
            let r = marco(&m, medida);
            assert!(
                r.abajo() < m.area_trabajo.abajo(),
                "{escala}: encima de la barra de tareas"
            );
            assert!(
                r.abajo() >= m.area_trabajo.abajo() - 30,
                "{escala}: pegada abajo"
            );
            let centro = r.x + r.ancho as i32 / 2;
            assert!((centro - 960).abs() <= 1, "{escala}: centrada");
        }
        // Crece con la escala, como la barra.
        assert!(medida(150, 0.0).0 > medida(100, 0.0).0);
        // Un rotulo largo la ensancha; uno corto no la estrecha.
        assert!(medida(100, 600.0).0 > medida(100, 10.0).0);
        assert_eq!(medida(100, 10.0), medida(100, 0.0));
    }

    #[test]
    fn cada_boton_responde_en_su_sitio_y_los_huecos_y_el_rotulo_no() {
        let (ancho, _) = medida(125, 300.0);
        let bs = botones(125, ancho);
        assert_eq!(
            bs.iter().map(|(a, _)| *a).collect::<Vec<_>>(),
            Accion::TODAS.to_vec()
        );
        for (a, r) in &bs {
            assert_eq!(
                accion_en(r.x + r.ancho / 2.0, r.y + r.alto / 2.0, 125, ancho),
                Some(*a)
            );
            assert!(
                r.x >= 0.0 && r.x + r.ancho <= ancho as f32,
                "{a:?} dentro de la ventana"
            );
        }
        // Casos negativos: el hueco entre dos, el separador antes de salir y
        // la fila del rotulo no son ningun boton.
        let (_, a) = bs[0];
        let (_, b) = bs[1];
        assert_eq!(
            accion_en((a.x + a.ancho + b.x) / 2.0, a.y + 5.0, 125, ancho),
            None
        );
        let (_, g) = bs[3];
        let (_, s) = bs[4];
        assert!(
            s.x - (g.x + g.ancho) > b.x - (a.x + a.ancho),
            "salir va apartado"
        );
        assert_eq!(accion_en(a.x + 5.0, a.y + a.alto + 3.0, 125, ancho), None);
        assert_eq!(accion_en(-1.0, a.y + 5.0, 125, ancho), None);
    }

    fn raya(x: f32) -> Elemento {
        Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(x, 0.0), Punto2::nuevo(x + 10.0, 10.0)],
                presiones: Vec::new(),
                opciones: None,
            },
            ..Elemento::default()
        }
    }

    #[test]
    fn limpiar_borra_toda_la_tinta_y_un_solo_ctrl_z_la_devuelve() {
        let mut e = Escena::nueva();
        for i in 0..5 {
            e.anadir(raya(i as f32 * 20.0));
        }
        e.cerrar_paso();
        assert_eq!(limpiar(&mut e), 5);
        assert_eq!(e.cuantos_visibles(), 0);
        assert!(e.deshacer());
        assert_eq!(e.cuantos_visibles(), 5, "todo de vuelta de una vez");
        assert!(e.rehacer());
        assert_eq!(e.cuantos_visibles(), 0);
    }

    #[test]
    fn limpiar_sin_nada_no_deja_un_paso_vacio_que_deshacer() {
        let mut e = Escena::nueva();
        assert_eq!(limpiar(&mut e), 0);
        assert!(!e.hay_que_deshacer());
        // Y lo ya borrado no se vuelve a contar.
        let id = e.anadir(raya(0.0));
        e.borrar_apuntando(id);
        let pasos = e.pasos_cerrados();
        assert_eq!(limpiar(&mut e), 0);
        assert_eq!(e.pasos_cerrados(), pasos);
    }

    #[test]
    fn soltar_fuera_del_boton_pulsado_es_arrepentirse() {
        // Sin ventana: la maquina de pulsar y soltar, con la misma regla.
        let (ancho, _) = medida(100, 0.0);
        let bs = botones(100, ancho);
        let centro = |i: usize| {
            (
                bs[i].1.x + bs[i].1.ancho / 2.0,
                bs[i].1.y + bs[i].1.alto / 2.0,
            )
        };
        let pulsado = accion_en(centro(1).0, centro(1).1, 100, ancho);
        assert_eq!(pulsado, Some(Accion::Limpiar));
        let soltado_en = accion_en(centro(2).0, centro(2).1, 100, ancho);
        assert_ne!(pulsado, soltado_en, "soltar en otro no dispara ninguno");
    }

    /// **La pastilla no puede quedar debajo del anotador** (2026-09-29: el
    /// usuario abrio con Alt + doble clic central y no veia el clic a
    /// traves). Sin ensenar nada: se crean las dos ventanas OCULTAS, como las
    /// crea el editor, y se mira lo que decide el orden Z: que la pastilla es
    /// PROPIEDAD del anotador (Windows deja lo poseido siempre encima de su
    /// dueno, aunque el dueno se active al primer clic), que es TOPMOST como
    /// el, que no coge el foco y que el clic a traves del anotador no se la
    /// lleva. Y que cae dentro del area de trabajo del monitor principal.
    /// `cargo test -p pixpin --bin pixpinmax la_pastilla_es_de_la_ventana_del_anotador -- --ignored`.
    #[test]
    #[ignore = "necesita GPU y sesion de escritorio (crea ventanas ocultas)"]
    fn la_pastilla_es_de_la_ventana_del_anotador_y_queda_siempre_encima_de_ella() {
        use windows::Win32::UI::WindowsAndMessaging::{
            WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
        };
        let d = pixpin_capture::enumerar_monitores().expect("monitores");
        let principal = *d.principal().expect("principal");
        let disp = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = pixpin_render::MotorRender::nuevo(disp.d3d()).expect("motor");
        let anotador = VentanaOverlay::nueva(d.escritorio_virtual()).expect("anotador");
        let ps = Pastilla::nueva(
            &motor,
            disp.d3d(),
            &principal,
            ("a".into(), "b".into()),
            anotador.handle(),
        )
        .expect("pastilla");
        assert_eq!(
            ps.ventana.dueno(),
            Some(anotador.handle()),
            "poseida por el anotador"
        );
        let e = ps.ventana.estilo_extendido();
        assert!(e & WS_EX_TOPMOST.0 != 0, "TOPMOST como el anotador");
        assert!(e & WS_EX_NOACTIVATE.0 != 0, "no coge el foco");
        // Con el clic a traves puesto el anotador deja pasar el raton; la
        // pastilla no, que es desde donde se vuelve.
        anotador.poner_pasante(true);
        assert!(anotador.estilo_extendido() & WS_EX_TRANSPARENT.0 != 0);
        assert!(ps.ventana.estilo_extendido() & WS_EX_TRANSPARENT.0 == 0);
        // Dentro del area de trabajo del principal (no fuera de pantalla).
        let r = ps.ventana.area();
        let t = principal.area_trabajo;
        assert!(
            r.x >= t.x && r.x + r.ancho as i32 <= t.x + t.ancho as i32,
            "{r:?} en {t:?}"
        );
        assert!(r.y >= t.y && r.abajo() <= t.abajo(), "{r:?} en {t:?}");
        // Caso negativo: una ventana sin dueno no dice tenerlo.
        assert_eq!(anotador.dueno(), None);
    }

    /// **La pastilla del anotador**, sobre un trozo de «pantalla»: dibujando
    /// y con el clic a traves encendido. Rotulos en espanol, del `.ftl`.
    /// `cargo test -p pixpin --bin pixpinmax muestra_de_la_pastilla_del_anotador -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_la_pastilla_del_anotador() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let textos = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let rotulos = (
            textos.t("anotador-rotulo-dibujando"),
            textos.t("anotador-rotulo-atravesando"),
        );
        assert!(
            !rotulos.0.is_empty() && !rotulos.0.starts_with("anotador-"),
            "{}",
            rotulos.0
        );
        let escala = 125;
        let k = escala as f32 / 100.0;
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let ancho_rotulo = [&rotulos.0, &rotulos.1]
            .iter()
            .map(|t| motor.medir_texto(t, TAM_ROTULO * k, f32::MAX).0)
            .fold(0.0, f32::max);
        let (w, h) = medida(escala, ancho_rotulo);
        let (lw, lh) = (w + 40, 2 * h + 60);
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), lw, lh).expect("superficie");
        motor
            .dibujar(&fuera.destino, |p| {
                // Un escritorio claro con una ventana oscura detras: la
                // pastilla tiene que leerse sobre los dos.
                p.limpiar(Color {
                    r: 0.93,
                    g: 0.94,
                    b: 0.96,
                    a: 1.0,
                });
                p.rellenar(
                    RectF {
                        x: lw as f32 / 2.0,
                        y: 0.0,
                        ancho: lw as f32 / 2.0,
                        alto: lh as f32,
                    },
                    Color {
                        r: 0.12,
                        g: 0.13,
                        b: 0.16,
                        a: 1.0,
                    },
                );
                p.desplazar(20.0, 20.0);
                pintar(p, escala, (w, h), false, None, &rotulos.0);
                p.desplazar(20.0, 40.0 + h as f32);
                pintar(p, escala, (w, h), true, Some(Accion::Guardar), &rotulos.1);
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba {
            ancho: lw,
            alto: lh,
            pixeles,
        })
        .unwrap();
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let ruta = carpeta.join("pastilla-del-anotador.png");
        std::fs::write(&ruta, png).unwrap();
        println!("pastilla-del-anotador ({w}x{h}): {}", ruta.display());
    }
}
