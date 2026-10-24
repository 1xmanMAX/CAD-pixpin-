//! **Lo que cuesta cada fotograma de ARRASTRAR**, medido sin ventana (la
//! queja del usuario del 27-sep: «la lupa se mueve lento», «la grafica se
//! mueve lento», «corrige todo lo que se mueve lento cuando lo muevo»).
//!
//! Reproduce, aviso a aviso, lo que hace el bucle del editor al mover lo
//! elegido: el gesto (`Gesto::evento`), la decision de si el aviso se
//! atiende en la capa de la tinta (`atender_en_capa`: la composicion lo
//! corre y la escena no se toca) y, si no, el fotograma de `pintar` (la capa
//! congelada volcada entera, lo elegido encima y la pasada de las lupas).
//! Lo que no se puede medir sin ventana —el `Commit` de DirectComposition y
//! la capa de la interfaz— se deja fuera, y se dice.
//!
//! ```text
//! cargo test --release -p pixpin --bin pixpinmax medir_el_arrastre -- --ignored --nocapture --test-threads=1
//! ```

use std::time::Instant;

use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::indice::Rejilla;
use pixpin_motor2d::vector::Punto2;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;

use super::*;

/// Una pantalla de portatil corriente a 100 %: la del equipo suelo.
const ANCHO: u32 = 1920;
const ALTO: u32 = 1080;
/// Avisos del raton por arrastre (medio segundo a 60 Hz).
const AVISOS: usize = 30;

struct Banco {
    dispositivo: pixpin_capture::Dispositivo,
    motor: MotorRender,
    destino: FueraDePantalla,
    /// La capa congelada: lo que no se mueve, horneado al pulsar.
    capa: FueraDePantalla,
    /// La capa de la tinta, donde va lo elegido cuando compone.
    tinta: FueraDePantalla,
    imagenes: ImagenesLienzo,
}

impl Banco {
    fn nuevo() -> Banco {
        let dispositivo = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(dispositivo.d3d()).expect("motor");
        let nuevo = |m: &MotorRender| {
            FueraDePantalla::nuevo(m, dispositivo.d3d(), ANCHO, ALTO).expect("textura")
        };
        let (destino, capa, tinta) = (nuevo(&motor), nuevo(&motor), nuevo(&motor));
        let imagenes = ImagenesLienzo::nuevo(motor.lado_maximo_bitmap());
        Banco {
            dispositivo,
            motor,
            destino,
            capa,
            tinta,
            imagenes,
        }
    }

    /// Pinta en `destino` los candidatos que `quien` acepta, como `pintar`.
    #[allow(clippy::too_many_arguments)]
    fn escena_en(
        &self,
        destino: &FueraDePantalla,
        escena: &Escena,
        rejilla: &Rejilla,
        cache: &mut Cache,
        cache_tinta: &mut pixpin_render::CacheTinta,
        camara: &Camara,
        limpiar: bool,
        quien: &dyn Fn(u64) -> bool,
    ) {
        let vista = camara.ventana(ANCHO as f32, ALTO as f32);
        let candidatos = rejilla.candidatos(vista);
        let imagenes = &self.imagenes;
        self.motor
            .dibujar(&destino.destino, |p| {
                if limpiar {
                    p.limpiar(crate::dibujo::pintar::a_color(escena.fondo));
                }
                let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
                p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));
                for id in candidatos {
                    if !quien(id) {
                        continue;
                    }
                    let Some(e) = elemento_por_id(escena, rejilla, id) else {
                        continue;
                    };
                    if e.borrado {
                        continue;
                    }
                    let mut indice = 0u32;
                    let grano = pixpin_motor2d::pintado::grano_de(e);
                    por_cada_orden(cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
                        dibujar_orden(
                            p,
                            orden,
                            vista,
                            Some((&mut *cache_tinta, (e.id, e.version, indice))),
                            imagenes,
                            camara.zoom,
                            grano,
                        );
                        indice += 1;
                    });
                }
            })
            .expect("fotograma");
    }
}

/// Lo que cuesta un arrastre, repartido por aviso.
#[derive(Default, Debug)]
struct Arrastre {
    avisos: usize,
    gesto_ms: f64,
    /// Avisos atendidos por composicion (sin repintar la escena).
    compuestos: usize,
    /// Avisos que rehicieron el fotograma entero.
    enteros: usize,
    fotograma_ms: f64,
    fotograma_peor: f64,
    /// De `fotograma_ms`, la pasada de las lupas.
    lupas_ms: f64,
    /// Lo que costo pintar lo elegido en su capa al empezar.
    empezar_ms: f64,
    /// Cuantas veces se volvio a pintar lo de dentro de la lupa.
    rehechas: u64,
    /// De los enteros, cuantos rehicieron solo el trozo de lo elegido.
    en_zona: usize,
}

impl Arrastre {
    fn por_aviso(&self) -> f64 {
        (self.gesto_ms + self.fotograma_ms) / self.avisos.max(1) as f64
    }
}

/// Un punto que agarra lo elegido: el primero de una rejilla por las cajas
/// de lo elegido que el picado reconoce.
fn punto_que_agarra(escena: &Escena, gesto: &Gesto, escala: f32) -> Punto2 {
    // Ni el borde de la caja ni un tirador: ahi se estira, no se mueve.
    let tiradores = gesto.tiradores(escena, escala);
    for id in gesto.seleccion.ids() {
        let Some(e) = escena.buscar(*id) else {
            continue;
        };
        let (x0, y0, x1, y1) = e.caja();
        // Un marco solo se coge por su raya: la de arriba, lejos de los
        // tiradores.
        if matches!(e.figura, Figura::Marco { .. }) {
            return Punto2::nuevo(x0 + (x1 - x0) / 4.0, y0);
        }
        for i in 1..8 {
            for j in 1..8 {
                let p = Punto2::nuevo(
                    x0 + (x1 - x0) * i as f32 / 8.0,
                    y0 + (y1 - y0) * j as f32 / 8.0,
                );
                if pixpin_motor2d::impacto::toca(e, p)
                    && tiradores.as_ref().is_none_or(|t| t.en(p, escala).is_none())
                {
                    return p;
                }
            }
        }
    }
    panic!("nada de lo elegido se deja agarrar");
}

/// **Un arrastre de lo elegido** con lo que haria el bucle del editor en
/// cada aviso. `camara` es la efectiva; la escena se devuelve como estaba.
fn arrastrar(
    b: &mut Banco,
    escena: &mut Escena,
    ids: &[u64],
    camara: &Camara,
    estirar: bool,
) -> Arrastre {
    let copia = escena.clone();
    let mut gesto = Gesto::nuevo();
    gesto.herramienta = Herramienta::Mano;
    gesto.seleccion.poner_todos(ids.iter().copied());
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(escena);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(camara.zoom);
    let escala = 1.0 / camara.zoom;
    // Calentar: la escena entera una vez, como la ve el usuario antes.
    b.escena_en(
        &b.destino,
        escena,
        &rejilla,
        &mut cache,
        &mut cache_tinta,
        camara,
        true,
        &|_| true,
    );
    let _ = lupas::pasar(
        &mut b.motor,
        &b.destino.destino,
        escena,
        camara,
        None,
        &b.imagenes,
        ANCHO,
        ALTO,
        &|_| false,
    );
    b.destino.esperar_gpu().expect("GPU");

    // Estirar: por la esquina de abajo a la derecha (el tirador sureste).
    let desde = if estirar {
        gesto.tiradores(escena, escala).expect("tiradores").tamano[4].1
    } else {
        punto_que_agarra(escena, &gesto, escala)
    };
    gesto.evento(
        EventoGesto::Pulsar {
            p: desde,
            shift: false,
            alt: false,
            presion: None,
        },
        escena,
        escala,
    );
    let rehechas_antes = lupas::rehechas();
    let mut a = Arrastre::default();
    let mut en_capa = false;
    let mut capa_lista = false;
    let mut zona_antes: Option<(i32, i32, i32, i32)> = None;
    for i in 1..=AVISOS {
        let q = Punto2::nuevo(
            desde.x + i as f32 * 6.0 * escala,
            desde.y + i as f32 * 3.0 * escala,
        );
        let t = Instant::now();
        gesto.evento(
            EventoGesto::Mover {
                p: q,
                shift: false,
                alt: false,
                presion: None,
            },
            escena,
            escala,
        );
        a.gesto_ms += t.elapsed().as_secs_f64() * 1000.0;
        a.avisos += 1;
        // La misma criba que `atender_en_capa`.
        let caja = gesto.seleccion.caja(escena);
        let dentro = caja.is_some_and(|c| {
            let r = caja_en_pantalla(camara, c, 16.0);
            r.0 >= 0 && r.1 >= 0 && r.2 <= ANCHO as i32 && r.3 <= ALTO as i32
        });
        let compone =
            en_capa || (gesto.moviendo() && gesto.flechas_que_siguen().is_empty() && dentro);
        if compone {
            if !en_capa {
                // Empezar: lo elegido, una vez, en su capa.
                let t = Instant::now();
                let sel = gesto.seleccion.clone();
                b.escena_en(
                    &b.tinta,
                    escena,
                    &rejilla,
                    &mut cache,
                    &mut cache_tinta,
                    camara,
                    true,
                    &|id| sel.contiene(id),
                );
                b.tinta.esperar_gpu().expect("GPU");
                a.empezar_ms = t.elapsed().as_secs_f64() * 1000.0;
                en_capa = true;
            }
            a.compuestos += 1;
            continue;
        }
        // El fotograma entero de `pintar`: la capa congelada (horneada una
        // vez, al primer aviso: `capa.preparar`), volcada entera, lo elegido
        // encima sin cache de tinta y la pasada de las lupas.
        rejilla.sincronizar(escena);
        if !capa_lista {
            let sel = gesto.seleccion.clone();
            b.escena_en(
                &b.capa,
                escena,
                &rejilla,
                &mut cache,
                &mut cache_tinta,
                camara,
                true,
                &|id| !sel.contiene(id),
            );
            b.capa.esperar_gpu().expect("GPU");
            capa_lista = true;
        }
        let t = Instant::now();
        // Estirando: solo el trozo de lo elegido, el de antes y el de ahora
        // (`congelar::zona_al_transformar`), como el bucle del editor.
        let ahora = congelar::zona_al_transformar(escena, &gesto, camara, &mut cache);
        let zona = match (ahora, zona_antes) {
            (Some(z), Some(p)) if capa_lista => {
                Some((p.0.min(z.0), p.1.min(z.1), p.2.max(z.2), p.3.max(z.3)))
            }
            _ => None,
        };
        zona_antes = ahora;
        a.en_zona += zona.is_some() as usize;
        b.destino.copiar_desde(&b.capa, zona).expect("volcar");
        let sel = gesto.seleccion.clone();
        let vista = camara.ventana(ANCHO as f32, ALTO as f32);
        let imagenes = &b.imagenes;
        b.motor
            .dibujar(&b.destino.destino, |p| {
                let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
                p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));
                for e in escena
                    .elementos
                    .iter()
                    .filter(|e| sel.contiene(e.id) && !e.borrado)
                {
                    let grano = pixpin_motor2d::pintado::grano_de(e);
                    por_cada_orden(
                        &mut cache,
                        e,
                        camara.zoom,
                        escena.escala.as_ref(),
                        |orden| {
                            dibujar_orden(p, orden, vista, None, imagenes, camara.zoom, grano);
                        },
                    );
                }
            })
            .expect("lo elegido");
        let tl = Instant::now();
        let _ = lupas::pasar(
            &mut b.motor,
            &b.destino.destino,
            escena,
            camara,
            None,
            &b.imagenes,
            ANCHO,
            ALTO,
            &|_| false,
        );
        b.destino.esperar_gpu().expect("GPU");
        let f = t.elapsed().as_secs_f64() * 1000.0;
        a.lupas_ms += tl.elapsed().as_secs_f64() * 1000.0;
        a.enteros += 1;
        a.fotograma_ms += f;
        a.fotograma_peor = a.fotograma_peor.max(f);
    }
    a.rehechas = lupas::rehechas() - rehechas_antes;
    *escena = copia;
    a
}

/// La escena «normal»: la sintetica de 300 elementos (trazos, formas con
/// sombreado, flechas y rotulos) y, en la parte que se ve, una grafica, una
/// tabla, un grupo de cuarenta figuras, dos figuras sueltas y una lupa.
struct Lamina {
    escena: Escena,
    camara: Camara,
    grafica: Vec<u64>,
    tabla: Vec<u64>,
    grupo: Vec<u64>,
    suelta: u64,
    lupa: u64,
}

fn lamina(b: &Banco) -> Lamina {
    use pixpin_motor2d::grafica::{self, Peticion};
    pixpin_motor2d::texto::instalar_medidor(crate::dibujo::pintar::medir_para_el_motor);
    let mut escena = super::medir::escena_sintetica(300);
    let mut gesto = Gesto::nuevo();
    let medir = figuras::medidor(&b.motor, pixpin_motor2d::texto::nombre_de_familia(None));
    let estilo = figuras::estilo_del_pincel(&gesto);
    let p = Peticion {
        formulas: vec![
            "sin(x)/x".into(),
            "sqrt(x+4) - 1".into(),
            "x^2/4 si x < 0; -x/2 si x >= 0".into(),
        ],
        x_desde: -6.0,
        x_hasta: 6.0,
        y_desde: -3.0,
        y_hasta: 3.0,
        escala: 50.0,
    };
    let v = grafica::elementos(&p, &estilo, &medir).expect("grafica");
    figuras::estampar_en_la_vista(
        &mut escena,
        &mut gesto,
        Punto2::nuevo(700.0, 500.0),
        "grafica",
        &v,
    );
    let grafica = gesto.seleccion.ids().to_vec();
    let filas: Vec<Vec<String>> = (0..8)
        .map(|f| (0..5).map(|c| format!("celda {f},{c}")).collect())
        .collect();
    let v = pixpin_motor2d::tabla_dibujada::elementos_de_tabla(
        &filas,
        &estilo,
        Punto2::nuevo(0.0, 0.0),
        &medir,
        true,
    );
    figuras::estampar_en_la_vista(
        &mut escena,
        &mut gesto,
        Punto2::nuevo(1500.0, 500.0),
        "tabla",
        &v,
    );
    let tabla = gesto.seleccion.ids().to_vec();
    let mut piezas = Vec::new();
    for i in 0..40 {
        let mut e = Elemento {
            figura: if i % 2 == 0 {
                Figura::Rectangulo
            } else {
                Figura::Elipse
            },
            x: (i % 8) as f32 * 40.0,
            y: (i / 8) as f32 * 40.0,
            ancho: 34.0,
            alto: 34.0,
            trazo: ColorRgba::opaco(0.1, 0.3, 0.7),
            relleno: Some(ColorRgba::opaco(0.8, 0.9, 1.0)),
            grosor: 2.0,
            ..Default::default()
        };
        e.semilla = 7 + i;
        piezas.push(e);
    }
    figuras::estampar_en_la_vista(
        &mut escena,
        &mut gesto,
        Punto2::nuevo(700.0, 1000.0),
        "grupo",
        &piezas,
    );
    let grupo = gesto.seleccion.ids().to_vec();
    let suelta = escena.anadir(Elemento {
        figura: Figura::Rectangulo,
        x: 1300.0,
        y: 950.0,
        ancho: 120.0,
        alto: 80.0,
        trazo: ColorRgba::opaco(0.8, 0.2, 0.2),
        relleno: Some(ColorRgba::opaco(1.0, 0.9, 0.9)),
        grosor: 2.0,
        ..Default::default()
    });
    // La lupa: un circulo sobre la grafica convertido con la varita y
    // apartado a la derecha, como la deja el usuario.
    let circulo = escena.anadir(Elemento {
        figura: Figura::Elipse,
        x: 600.0,
        y: 420.0,
        ancho: 160.0,
        alto: 160.0,
        trazo: ColorRgba::opaco(0.2, 0.2, 0.2),
        grosor: 2.0,
        ..Default::default()
    });
    let lupa = crate::dibujo::lupa::convertir(&mut escena, &mut gesto, circulo).expect("lupa");
    escena.mover(lupa, 400.0, 250.0);
    let camara = Camara {
        x: 0.0,
        y: 0.0,
        zoom: 0.6,
    };
    let _ = ALTO;
    Lamina {
        escena,
        camara,
        grafica,
        tabla,
        grupo,
        suelta,
        lupa,
    }
}

fn escribir(que: &str, a: &Arrastre) {
    println!(
        "{que:<40} {:>6.2} ms/aviso | gesto {:>6.3} ms | compuestos {:>2}/{:<2} enteros {:>2} (media {:>6.2}, peor {:>6.2}, lupas {:>6.2}) | empezar {:>6.2} ms | lupa rehecha {} | en zona {}",
        a.por_aviso(),
        a.gesto_ms / a.avisos.max(1) as f64,
        a.compuestos,
        a.avisos,
        a.enteros,
        a.fotograma_ms / a.enteros.max(1) as f64,
        a.fotograma_peor,
        a.lupas_ms / a.enteros.max(1) as f64,
        a.empezar_ms,
        a.rehechas,
        a.en_zona,
    );
}

#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn medir_el_arrastre_de_lupa_grafica_tabla_grupo_y_sueltas() {
    let mut b = Banco::nuevo();
    let l = lamina(&b);
    let mut escena = l.escena.clone();
    // La pasada de las lupas con nada cambiado: solo copiar lo de dentro.
    {
        let _ = lupas::pasar(
            &mut b.motor,
            &b.destino.destino,
            &escena,
            &l.camara,
            None,
            &b.imagenes,
            ANCHO,
            ALTO,
            &|_| false,
        );
        b.destino.esperar_gpu().expect("GPU");
        let antes = lupas::rehechas();
        let t = Instant::now();
        for _ in 0..AVISOS {
            let _ = lupas::pasar(
                &mut b.motor,
                &b.destino.destino,
                &escena,
                &l.camara,
                None,
                &b.imagenes,
                ANCHO,
                ALTO,
                &|_| false,
            );
            b.destino.esperar_gpu().expect("GPU");
        }
        println!(
            "pasada de lupas sin cambios: {:.2} ms (rehechas {})",
            t.elapsed().as_secs_f64() * 1000.0 / AVISOS as f64,
            lupas::rehechas() - antes
        );
    }
    let casos: [(&str, Vec<u64>); 5] = [
        ("lupa", vec![l.lupa]),
        ("grafica", l.grafica.clone()),
        ("tabla", l.tabla.clone()),
        ("grupo de 40", l.grupo.clone()),
        ("figura suelta", vec![l.suelta]),
    ];
    for con_clavos in [false, true] {
        if con_clavos {
            // Un clavo lejos de todo lo que se arrastra: dos trazos de la
            // esquina de abajo, fuera de la vista.
            let a = escena.elementos[0].clone();
            let (x0, y0, x1, y1) = a.caja();
            let _ = (x0, y0);
            {
                // Dos rayas cruzadas fuera de la vista, clavadas en su cruce.
                let r = |x0: f32, y0: f32, x1: f32, y1: f32| Elemento {
                    figura: Figura::Linea {
                        puntos: vec![Punto2::nuevo(x0, y0), Punto2::nuevo(x1, y1)],
                    },
                    x: x0.min(x1),
                    y: y0.min(y1),
                    ancho: (x1 - x0).abs(),
                    alto: (y1 - y0).abs(),
                    grosor: 2.0,
                    ..Default::default()
                };
                escena.anadir(r(x1 + 5000.0, y1, x1 + 5100.0, y1 + 100.0));
                escena.anadir(r(x1 + 5000.0, y1 + 100.0, x1 + 5100.0, y1));
                assert!(pixpin_motor2d::nudos::soldar(
                    &mut escena,
                    Punto2::nuevo(x1 + 5050.0, y1 + 50.0),
                    10.0
                ));
            }
            assert!(!escena.alfileres.is_empty());
        }
        let clavos = if con_clavos {
            "con clavo lejos"
        } else {
            "sin clavo"
        };
        for (que, ids) in &casos {
            let a = arrastrar(&mut b, &mut escena, ids, &l.camara, false);
            escribir(&format!("mover {que} ({clavos})"), &a);
        }
        for (que, ids) in casos.iter().filter(|(_, ids)| ids.len() > 1) {
            let a = arrastrar(&mut b, &mut escena, ids, &l.camara, true);
            escribir(&format!("estirar {que} ({clavos})"), &a);
        }
    }
    // Apuntar la lupa a otro sitio (`MoviendoElFoco`): cada aviso cambia lo
    // mirado, asi que lo de dentro se rehace si o si; lo que se mide es el
    // resto del fotograma.
    let a = apuntar_la_lupa(&mut b, &mut escena, l.lupa, &l.camara);
    escribir("apuntar la lupa (mover lo mirado)", &a);
    lupas::olvidar();
    let _ = &b.dispositivo;
}

/// **Arrastrar lo mirado de una lupa** como lo hace el bucle del editor:
/// `ManoDeLupa` cambia el foco y pide el fotograma entero. Con `capa`, lo
/// quieto sale de la capa congelada (horneada sin la lupa, al empezar); sin
/// ella, se pinta la escena entera en cada aviso.
fn apuntar_la_lupa(b: &mut Banco, escena: &mut Escena, id: u64, camara: &Camara) -> Arrastre {
    let copia = escena.clone();
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(escena);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(camara.zoom);
    b.escena_en(
        &b.destino,
        escena,
        &rejilla,
        &mut cache,
        &mut cache_tinta,
        camara,
        true,
        &|_| true,
    );
    let _ = lupas::pasar(
        &mut b.motor,
        &b.destino.destino,
        escena,
        camara,
        None,
        &b.imagenes,
        ANCHO,
        ALTO,
        &|_| false,
    );
    b.destino.esperar_gpu().expect("GPU");
    let con_capa = ventana_editor_apunta_con_capa();
    if con_capa {
        b.escena_en(
            &b.capa,
            escena,
            &rejilla,
            &mut cache,
            &mut cache_tinta,
            camara,
            true,
            &|x| x != id,
        );
        b.capa.esperar_gpu().expect("GPU");
    }
    let mut a = Arrastre::default();
    for _ in 0..AVISOS {
        let t = Instant::now();
        let antes = lupas::huella_elegida(escena, &[id], camara);
        if let Some(e) = escena.buscar_mut(id)
            && let Figura::Lupa { cristal } = &mut e.figura
        {
            let f = cristal.foco.unwrap_or(Punto2::nuevo(0.0, 0.0));
            cristal.foco = Some(Punto2::nuevo(f.x + 4.0, f.y + 2.0));
            e.tocar();
        }
        a.gesto_ms += t.elapsed().as_secs_f64() * 1000.0;
        a.avisos += 1;
        let t = Instant::now();
        rejilla.sincronizar(escena);
        if con_capa {
            // Solo el trozo de la lupa, el de antes y el de ahora.
            let ahora = lupas::huella_elegida(escena, &[id], camara);
            let u = [antes, ahora]
                .into_iter()
                .flatten()
                .fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |s, c| {
                    (s.0.min(c.0), s.1.min(c.1), s.2.max(c.2), s.3.max(c.3))
                });
            let zona = (
                u.0.max(0.0) as i32,
                u.1.max(0.0) as i32,
                (u.2 as i32).min(ANCHO as i32),
                (u.3 as i32).min(ALTO as i32),
            );
            b.destino.copiar_desde(&b.capa, Some(zona)).expect("volcar");
            b.escena_en(
                &b.destino,
                escena,
                &rejilla,
                &mut cache,
                &mut cache_tinta,
                camara,
                false,
                &|x| x == id,
            );
        } else {
            b.escena_en(
                &b.destino,
                escena,
                &rejilla,
                &mut cache,
                &mut cache_tinta,
                camara,
                true,
                &|_| true,
            );
        }
        let tl = Instant::now();
        let _ = lupas::pasar(
            &mut b.motor,
            &b.destino.destino,
            escena,
            camara,
            None,
            &b.imagenes,
            ANCHO,
            ALTO,
            &|_| false,
        );
        b.destino.esperar_gpu().expect("GPU");
        a.lupas_ms += tl.elapsed().as_secs_f64() * 1000.0;
        let f = t.elapsed().as_secs_f64() * 1000.0;
        a.enteros += 1;
        a.fotograma_ms += f;
        a.fotograma_peor = a.fotograma_peor.max(f);
    }
    *escena = copia;
    a
}

/// Si el editor hornea la capa congelada al apuntar una lupa (lo que hace
/// el bucle de verdad: ver `ventana_editor.rs`, «apuntando una lupa»).
fn ventana_editor_apunta_con_capa() -> bool {
    true
}

// ---------------------------------------------------------------------------
// **Cotas y marcos** (queja del 28-sep: «cuando pongo la cota esta se mueve
// lento» y «al poner los frames pasa lo mismo»).
//
// ```text
// cargo test --release -p pixpin --bin pixpinmax medir_el_arrastre_de_cotas -- --ignored --nocapture --test-threads=1
// ```

/// Una cota de `a` a `b` como las deja la herramienta (rugosa, con semilla).
fn cota_de(a: Punto2, b: Punto2, semilla: u32) -> Elemento {
    let mut e = Elemento {
        figura: Figura::Cota { puntos: vec![a, b] },
        x: a.x.min(b.x),
        y: a.y.min(b.y),
        ancho: (b.x - a.x).abs(),
        alto: (b.y - a.y).abs(),
        trazo: ColorRgba::opaco(0.05, 0.87, 1.0),
        grosor: 2.5,
        rugosidad: 1.0,
        ..Default::default()
    };
    e.semilla = semilla;
    e
}

/// La lamina de antes con veinte cotas repartidas por lo que se ve, una
/// escala puesta (las cotas dicen centimetros) y dos marcos: uno vacio y
/// otro con cosas dentro.
struct LaminaDeCotas {
    escena: Escena,
    camara: Camara,
    cotas: Vec<u64>,
    suelta: u64,
    marco_vacio: u64,
    marco_lleno: u64,
}

fn lamina_de_cotas(b: &Banco) -> LaminaDeCotas {
    let l = lamina(b);
    let mut escena = l.escena;
    escena.escala = Some(pixpin_motor2d::Escala {
        unidades_por_pixel: 0.0166,
        unidad: "cm".into(),
        decimales: 2,
    });
    let mut cotas = Vec::new();
    for i in 0..20u32 {
        let x = 150.0 + (i % 5) as f32 * 520.0;
        let y = 150.0 + (i / 5) as f32 * 280.0;
        let a = Punto2::nuevo(x, y);
        let b = Punto2::nuevo(x + 300.0 - (i % 3) as f32 * 60.0, y + (i % 4) as f32 * 45.0);
        cotas.push(escena.anadir(cota_de(a, b, 40 + i)));
    }
    let marco = |x: f32, y: f32| Elemento {
        figura: Figura::Marco {
            nombre: "Lamina".into(),
        },
        x,
        y,
        ancho: 420.0,
        alto: 300.0,
        grosor: 1.5,
        ..Default::default()
    };
    let marco_vacio = escena.anadir(marco(2200.0, 1250.0));
    let marco_lleno = escena.anadir(marco(100.0, 1250.0));
    // Lo de dentro: una cota y unas figuras, lo que hay en una lamina.
    escena.anadir(cota_de(
        Punto2::nuevo(140.0, 1300.0),
        Punto2::nuevo(400.0, 1300.0),
        99,
    ));
    for i in 0..6 {
        escena.anadir(Elemento {
            figura: Figura::Rectangulo,
            x: 130.0 + i as f32 * 60.0,
            y: 1400.0,
            ancho: 50.0,
            alto: 50.0,
            trazo: ColorRgba::opaco(0.2, 0.2, 0.2),
            grosor: 2.0,
            ..Default::default()
        });
    }
    LaminaDeCotas {
        escena,
        camara: l.camara,
        cotas,
        suelta: l.suelta,
        marco_vacio,
        marco_lleno,
    }
}

/// **Trazar una figura nueva** con lo que hace el bucle del editor: pulsar
/// hornea la capa congelada sin lo que nace (`congelar::hornear`), y cada
/// aviso copia de ella el trozo sucio (`Region::Caja` del gesto, el de antes
/// y el de ahora) y repinta encima lo que se esta trazando.
fn trazar(
    b: &mut Banco,
    escena: &mut Escena,
    herramienta: Herramienta,
    desde: Punto2,
    camara: &Camara,
) -> Arrastre {
    let copia = escena.clone();
    let mut gesto = Gesto::nuevo();
    gesto.herramienta = herramienta;
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(escena);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(camara.zoom);
    let escala = 1.0 / camara.zoom;
    b.escena_en(
        &b.destino,
        escena,
        &rejilla,
        &mut cache,
        &mut cache_tinta,
        camara,
        true,
        &|_| true,
    );
    b.destino.esperar_gpu().expect("GPU");
    gesto.evento(
        EventoGesto::Pulsar {
            p: desde,
            shift: false,
            alt: false,
            presion: None,
        },
        escena,
        escala,
    );
    let mut a = Arrastre::default();
    let (nuevo, _) = gesto.elemento_en_curso().expect("algo nace");
    // Hornear la capa sin lo que nace, como `congelar::hornear`.
    let t = Instant::now();
    rejilla.sincronizar(escena);
    b.escena_en(
        &b.capa,
        escena,
        &rejilla,
        &mut cache,
        &mut cache_tinta,
        camara,
        true,
        &|id| id != nuevo,
    );
    b.capa.esperar_gpu().expect("GPU");
    a.empezar_ms = t.elapsed().as_secs_f64() * 1000.0;
    let mut zona_antes: Option<(i32, i32, i32, i32)> = None;
    for i in 1..=AVISOS {
        let q = Punto2::nuevo(
            desde.x + i as f32 * 11.0 * escala,
            desde.y + i as f32 * 5.0 * escala,
        );
        let t = Instant::now();
        let r = gesto.evento(
            EventoGesto::Mover {
                p: q,
                shift: false,
                alt: false,
                presion: None,
            },
            escena,
            escala,
        );
        a.gesto_ms += t.elapsed().as_secs_f64() * 1000.0;
        a.avisos += 1;
        let t = Instant::now();
        rejilla.sincronizar(escena);
        let ahora = match r.region {
            pixpin_motor2d::gesto::Region::Caja(x0, y0, x1, y1) => {
                Some(caja_en_pantalla(camara, (x0, y0, x1, y1), 2.0))
            }
            _ => None,
        };
        let zona = match (ahora, zona_antes) {
            (Some(z), Some(p)) => Some((p.0.min(z.0), p.1.min(z.1), p.2.max(z.2), p.3.max(z.3))),
            (Some(z), None) => Some(z),
            _ => None,
        };
        zona_antes = ahora;
        a.en_zona += zona.is_some() as usize;
        let zona = zona.map(|z| {
            (
                z.0.max(0),
                z.1.max(0),
                z.2.min(ANCHO as i32),
                z.3.min(ALTO as i32),
            )
        });
        b.destino.copiar_desde(&b.capa, zona).expect("volcar");
        let vista = camara.ventana(ANCHO as f32, ALTO as f32);
        let imagenes = &b.imagenes;
        b.motor
            .dibujar(&b.destino.destino, |p| {
                if let Some((x0, y0, x1, y1)) = zona {
                    p.empujar_recorte(pixpin_render::RectF {
                        x: x0 as f32,
                        y: y0 as f32,
                        ancho: (x1 - x0).max(0) as f32,
                        alto: (y1 - y0).max(0) as f32,
                    });
                }
                let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
                p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));
                if let Some(e) = escena.buscar(nuevo) {
                    let grano = pixpin_motor2d::pintado::grano_de(e);
                    por_cada_orden(
                        &mut cache,
                        e,
                        camara.zoom,
                        escena.escala.as_ref(),
                        |orden| {
                            dibujar_orden(p, orden, vista, None, imagenes, camara.zoom, grano);
                        },
                    );
                }
                p.desplazar(0.0, 0.0);
                if zona.is_some() {
                    p.soltar_recorte();
                }
            })
            .expect("lo que nace");
        b.destino.esperar_gpu().expect("GPU");
        let f = t.elapsed().as_secs_f64() * 1000.0;
        a.enteros += 1;
        a.fotograma_ms += f;
        a.fotograma_peor = a.fotograma_peor.max(f);
    }
    *escena = copia;
    a
}

/// La escena entera, `AVISOS` veces: lo que cuesta un fotograma sin capa
/// congelada (con el universo detras, al acercar o al soltar lo arrastrado).
fn escena_entera(b: &mut Banco, escena: &Escena, camara: &Camara) -> f64 {
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(escena);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(camara.zoom);
    b.escena_en(
        &b.destino,
        escena,
        &rejilla,
        &mut cache,
        &mut cache_tinta,
        camara,
        true,
        &|_| true,
    );
    b.destino.esperar_gpu().expect("GPU");
    let t = Instant::now();
    for _ in 0..AVISOS {
        b.escena_en(
            &b.destino,
            escena,
            &rejilla,
            &mut cache,
            &mut cache_tinta,
            camara,
            true,
            &|_| true,
        );
        b.destino.esperar_gpu().expect("GPU");
    }
    t.elapsed().as_secs_f64() * 1000.0 / AVISOS as f64
}

/// El desglose de una cota: cuanto de CPU es la raya de rough.js, cuanto
/// medir el numero con DirectWrite y cuanto adaptar el color; y de GPU,
/// cuanto el numero con su halo de 24 copias frente al numero solo.
fn desglose_de_la_cota(b: &Banco, escena: &Escena, cotas: &[u64], camara: &Camara) {
    use pixpin_motor2d::pintado::Orden;
    let vueltas = 200;
    let e = escena.buscar(cotas[0]).expect("cota").clone();
    let papel = ColorRgba::opaco(1.0, 1.0, 1.0);
    let t = Instant::now();
    for _ in 0..vueltas {
        std::hint::black_box(pixpin_motor2d::pintado::ordenes_medibles(
            &e,
            escena.escala.as_ref(),
            ',',
            papel,
        ));
    }
    let total = t.elapsed().as_secs_f64() * 1000.0 / vueltas as f64;
    let texto = pixpin_motor2d::medida::texto_de_cota(&e, escena.escala.as_ref(), ',');
    let t = Instant::now();
    for _ in 0..vueltas {
        std::hint::black_box(pixpin_motor2d::texto::medida(
            &texto,
            20.0,
            "Segoe UI",
            pixpin_motor2d::texto::EstiloDeTexto::default(),
        ));
    }
    let medir = t.elapsed().as_secs_f64() * 1000.0 / vueltas as f64;
    let t = Instant::now();
    for _ in 0..vueltas {
        std::hint::black_box(pixpin_motor2d::contraste::adaptar(
            e.trazo,
            pixpin_motor2d::contraste::papel_de(papel),
        ));
    }
    let contraste = t.elapsed().as_secs_f64() * 1000.0 / vueltas as f64;
    println!(
        "una cota, CPU: ordenes_medibles {total:.3} ms (medir el numero {medir:.3}, contraste {contraste:.4}, rough y resto {:.3})",
        total - medir - contraste
    );
    // GPU: las veinte cotas ya calculadas, pintadas con y sin halo.
    let ordenes: Vec<Orden> = cotas
        .iter()
        .filter_map(|id| escena.buscar(*id))
        .flat_map(|e| {
            pixpin_motor2d::pintado::ordenes_medibles(e, escena.escala.as_ref(), ',', papel)
        })
        .collect();
    let vista = camara.ventana(ANCHO as f32, ALTO as f32);
    let pasar = |filtro: &dyn Fn(&Orden) -> Option<Orden>| {
        let t = Instant::now();
        for _ in 0..AVISOS {
            b.motor
                .dibujar(&b.destino.destino, |p| {
                    p.limpiar(crate::dibujo::pintar::a_color(papel));
                    let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
                    p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));
                    for o in &ordenes {
                        if let Some(o) = filtro(o) {
                            dibujar_orden(p, &o, vista, None, &b.imagenes, camara.zoom, None);
                        }
                    }
                })
                .expect("fotograma");
            b.destino.esperar_gpu().expect("GPU");
        }
        t.elapsed().as_secs_f64() * 1000.0 / AVISOS as f64
    };
    let nada = pasar(&|_| None);
    let todo = pasar(&|o| Some(o.clone()));
    let sin_numero = pasar(&|o| (!matches!(o, Orden::Rotulo { .. })).then(|| o.clone()));
    let sin_halo = pasar(&|o| {
        let mut o = o.clone();
        if let Orden::Rotulo { grosor_halo, .. } = &mut o {
            *grosor_halo = 0.0;
        }
        Some(o)
    });
    println!(
        "veinte cotas, GPU (fotograma con limpiar {nada:.2} ms): todo {todo:.2} ms | sin numero {sin_numero:.2} | numero sin halo {sin_halo:.2}"
    );
}

#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn medir_el_arrastre_de_cotas_y_marcos() {
    let mut b = Banco::nuevo();
    let l = lamina_de_cotas(&b);
    let mut escena = l.escena.clone();
    desglose_de_la_cota(&b, &escena, &l.cotas, &l.camara);
    println!(
        "escena entera con 20 cotas: {:.2} ms/fotograma",
        escena_entera(&mut b, &escena, &l.camara)
    );
    {
        let mut sin = escena.clone();
        for id in &l.cotas {
            if let Some(e) = sin.buscar_mut(*id) {
                e.borrado = true;
            }
        }
        println!(
            "escena entera sin las cotas: {:.2} ms/fotograma",
            escena_entera(&mut b, &sin, &l.camara)
        );
    }
    let a = trazar(
        &mut b,
        &mut escena,
        Herramienta::Cota,
        Punto2::nuevo(1300.0, 700.0),
        &l.camara,
    );
    escribir("trazar una cota nueva", &a);
    let a = trazar(
        &mut b,
        &mut escena,
        Herramienta::Marco,
        Punto2::nuevo(1300.0, 700.0),
        &l.camara,
    );
    escribir("trazar un marco nuevo", &a);
    for (que, id) in [
        ("una cota", l.cotas[7]),
        ("marco vacio", l.marco_vacio),
        ("marco con cosas", l.marco_lleno),
        ("figura suelta (20 cotas)", l.suelta),
    ] {
        let a = arrastrar(&mut b, &mut escena, &[id], &l.camara, false);
        escribir(&format!("mover {que}"), &a);
        let a = arrastrar(&mut b, &mut escena, &[id], &l.camara, true);
        escribir(&format!("estirar {que}"), &a);
    }
    lupas::olvidar();
    let _ = &b.dispositivo;
}
