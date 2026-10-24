//! Pruebas de la foto de lo anotado (13-oct-2026, «el zoom va lagueado»):
//! que mientras se mueve se vea lo que hay y NADA de lo que ya no hay
//! (anadir, borrar, deshacer, otro plano), y que lo teselado a otra escala
//! se rehaga poco a poco hasta quedar nitido. Pintan de verdad, en la
//! tarjeta, sobre una textura sin ventana, y miran los pixeles.

use pixpin_render::fuera_de_pantalla::FueraDePantalla;

use super::pruebas::{carpeta, plano_de_obra, rutas_en};
use super::*;

const W: u32 = 800;
const H: u32 = 500;

/// La vista de una ventana de 800 x 500 que mira `centro` con las letras
/// del plano a un tamano en que se leen.
fn vista_en(m: &Modelo, centro: [f64; 2], px: f64) -> VistaPlano {
    let unidad = pixpin_cad::anotado::unidad_de_la_capa(m);
    VistaPlano {
        centro,
        px,
        ancho: W,
        alto: H,
        origen_pantalla: (0, 0),
        escala_por_cien: 100,
        unidad,
        claro: true,
    }
}

const PX: f64 = 2.5 / 20.0;

/// Un trazo del lapiz de x = 250 a 550 a la altura `y` de la ventana.
fn trazar_en(motor: &mut MotorDelPlano, v: &VistaPlano, y: i32) -> u64 {
    motor
        .tinta
        .gesto
        .tomar_herramienta(pixpin_motor2d::gesto::Herramienta::Lapiz);
    motor.evento(EventoPlano::Mover(250, y), v);
    motor.evento(EventoPlano::Bajar(250, y), v);
    for x in (253..=550).step_by(3) {
        motor.evento(EventoPlano::Mover(x, y + (x / 3 % 2)), v);
    }
    motor.evento(EventoPlano::Subir(550, y), v);
    motor
        .capa
        .escena
        .visibles()
        .filter(|e| matches!(e.figura, Figura::Lapiz { .. }))
        .map(|e| e.id)
        .max()
        .expect("el lapiz dejo un trazo")
}

/// La tarjeta, un Direct2D para borrar entre fotogramas y el destino.
struct Banco {
    d: pixpin_capture::Dispositivo,
    r: MotorRender,
    fp: FueraDePantalla,
}

impl Banco {
    fn nuevo() -> Banco {
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let r = MotorRender::nuevo(d.d3d()).expect("motor");
        let fp = FueraDePantalla::nuevo(&r, d.d3d(), W, H).expect("destino");
        Banco { d, r, fp }
    }

    /// Un fotograma del visor: el fondo (blanco) y lo anotado encima, sin
    /// anotar. Devuelve los pixeles.
    fn pintar(&self, motor: &mut MotorDelPlano, v: &VistaPlano) -> Vec<u8> {
        self.r
            .dibujar(&self.fp.destino, |p| {
                p.limpiar(Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                })
            })
            .unwrap();
        motor.pintar(self.d.d3d(), self.fp.textura(), v, false);
        self.fp.leer_rgba().unwrap().2
    }
}

/// Cuantos pixeles de tinta (oscuros) hay en la franja de la ventana de
/// `y - 10` a `y + 10`, entre x = 260 y 540.
fn tinta_en(px: &[u8], y: i32) -> usize {
    let mut n = 0;
    for fila in (y - 10).max(0)..(y + 10).min(H as i32) {
        for x in 260..540 {
            let i = (fila as usize * W as usize + x) * 4;
            if px[i] < 128 && px[i + 1] < 128 && px[i + 2] < 128 {
                n += 1;
            }
        }
    }
    n
}

/// La vista corrida `dx` pixeles: la camara se mueve (un arrastre), y la
/// tinta sigue casi en la misma franja.
fn corrida(v: &VistaPlano, dx: f64) -> VistaPlano {
    VistaPlano {
        centro: [v.centro[0] + dx * v.px, v.centro[1]],
        ..*v
    }
}

#[test]
fn la_foto_de_lo_anotado_no_deja_trazos_viejos_al_anadir_borrar_y_deshacer() {
    let d = carpeta("foto");
    let m = plano_de_obra();
    let mut motor = MotorDelPlano::nuevo(rutas_en(&d), &m);
    let b = Banco::nuevo();
    let v = vista_en(&m, [0.0, 0.0], PX);
    let a = trazar_en(&mut motor, &v, 200);

    // Quieta: nitida, y sin nada que repintar.
    let px = b.pintar(&mut motor, &v);
    assert!(tinta_en(&px, 200) > 50, "el trazo A se ve");
    assert_eq!(tinta_en(&px, 320), 0, "no hay trazo B");
    assert!(!motor.falta_pintar(), "quieta y nitida no pide mas");

    // Moviendose: va en su foto, y al pararse pide otro fotograma.
    let v1 = corrida(&v, 1.0);
    let px = b.pintar(&mut motor, &v1);
    assert!(tinta_en(&px, 200) > 50, "moviendose, el trazo A sigue");
    assert!(
        motor.falta_pintar(),
        "la foto movida no es la ultima palabra"
    );
    let px = b.pintar(&mut motor, &v1);
    assert!(tinta_en(&px, 200) > 50);
    assert!(!motor.falta_pintar(), "parada, nitida otra vez");

    // Un trazo nuevo: la foto de antes no lo tiene y no vale.
    let tb = trazar_en(&mut motor, &v1, 320);
    let v2 = corrida(&v1, 1.0);
    let px = b.pintar(&mut motor, &v2);
    assert!(
        tinta_en(&px, 320) > 50,
        "el trazo nuevo se ve aunque la camara se mueva"
    );
    assert!(tinta_en(&px, 200) > 50);

    // Caso negativo: borrado, no se queda pintado en la foto.
    assert!(motor.capa.escena.borrar(a));
    let v3 = corrida(&v2, 1.0);
    let px = b.pintar(&mut motor, &v3);
    assert_eq!(
        tinta_en(&px, 200),
        0,
        "el trazo borrado no sigue en la foto"
    );
    assert!(tinta_en(&px, 320) > 50);
    let px = b.pintar(&mut motor, &v3);
    assert_eq!(tinta_en(&px, 200), 0, "ni quieta");

    // Deshacer el borrado: vuelve; deshacer el trazo B: se va.
    assert!(motor.capa.escena.restaurar(a));
    let v4 = corrida(&v3, 1.0);
    let px = b.pintar(&mut motor, &v4);
    assert!(tinta_en(&px, 200) > 50, "restaurado, vuelve");
    assert!(motor.capa.deshacer());
    let v5 = corrida(&v4, 1.0);
    let px = b.pintar(&mut motor, &v5);
    let quedan: Vec<u64> = motor.capa.escena.visibles().map(|e| e.id).collect();
    if !quedan.contains(&tb) {
        assert_eq!(
            tinta_en(&px, 320),
            0,
            "deshecho el trazo B, no se queda pintado"
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn otro_plano_no_hereda_lo_pintado_del_anterior() {
    let d = carpeta("foto-otro");
    let m = plano_de_obra();
    let b = Banco::nuevo();
    let v = vista_en(&m, [0.0, 0.0], PX);
    let mut uno = MotorDelPlano::nuevo(rutas_en(&d.join("uno")), &m);
    trazar_en(&mut uno, &v, 200);
    let px = b.pintar(&mut uno, &v);
    assert!(tinta_en(&px, 200) > 50);
    let px = b.pintar(&mut uno, &corrida(&v, 1.0));
    assert!(tinta_en(&px, 200) > 50);

    // Otro plano en el mismo dispositivo: solo lo suyo, ni movido ni quieto.
    let mut dos = MotorDelPlano::nuevo(rutas_en(&d.join("dos")), &m);
    trazar_en(&mut dos, &v, 320);
    for vista in [v, corrida(&v, 1.0), corrida(&v, 1.0)] {
        let px = b.pintar(&mut dos, &vista);
        assert_eq!(tinta_en(&px, 200), 0, "lo del otro plano no se cuela");
        assert!(tinta_en(&px, 320) > 50);
    }

    // Un plano sin nada anotado no pinta nada ni guarda foto.
    let mut tres = MotorDelPlano::nuevo(rutas_en(&d.join("tres")), &m);
    let px = b.pintar(&mut tres, &v);
    assert_eq!(tinta_en(&px, 200) + tinta_en(&px, 320), 0);
    assert_eq!(tres.foto.bytes(), 0);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn al_cambiar_de_zoom_se_tesela_poco_a_poco_y_acaba_nitido() {
    let d = carpeta("foto-zoom");
    let m = plano_de_obra();
    let b = Banco::nuevo();
    let v = vista_en(&m, [0.0, 0.0], PX);
    let mut motor = MotorDelPlano::nuevo(rutas_en(&d), &m);
    for y in (120..400).step_by(7) {
        trazar_en(&mut motor, &v, y);
    }
    let cuantos = motor.capa.escena.visibles().count();
    assert!(cuantos >= 30);
    b.pintar(&mut motor, &v);
    assert!(!motor.falta_pintar());

    // Casi sin tiempo de teselar: el primer fotograma a otra escala no lo
    // rehace todo (pinta lo de antes) y pide mas.
    motor.presupuesto_quieto = std::time::Duration::from_nanos(1);
    let cerca = vista_en(&m, [0.0, 0.0], PX / 3.0);
    let antes = motor.tinta.realizaciones();
    let px = b.pintar(&mut motor, &cerca);
    let primero = motor.tinta.realizaciones() - antes;
    assert!(
        primero < cuantos as u64 / 2,
        "de golpe se teselaron {primero} de {cuantos}"
    );
    assert!(motor.falta_pintar(), "queda por rehacer");
    // Caso negativo: lo de antes se ve mientras (no desaparece nada).
    assert!(
        tinta_en(&px, 250) > 0,
        "mientras, se pinta lo de otra escala"
    );

    // Fotograma a fotograma, hasta nitido, y entonces no pide mas.
    let mut vueltas = 0;
    while motor.falta_pintar() {
        b.pintar(&mut motor, &cerca);
        vueltas += 1;
        assert!(vueltas < 500, "no acaba de rehacerse");
    }
    let hecho = motor.tinta.realizaciones() - antes;
    assert!(
        hecho >= cuantos as u64 / 2,
        "se rehizo lo que se ve: {hecho}"
    );
    let antes = motor.tinta.realizaciones();
    b.pintar(&mut motor, &cerca);
    assert_eq!(
        motor.tinta.realizaciones(),
        antes,
        "nitido: nada mas que teselar"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **Lo que trae la sincronizacion** con el plano abierto: la capa del disco
/// cambia por fuera con el MISMO id y la MISMA version (lo peor: una capa
/// releida en otras unidades), el trazo en otro sitio. Al releerla
/// (`al_dia`) y al guardar juntando con el disco, la foto de antes no vale.
#[test]
fn lo_releido_del_disco_no_deja_la_foto_vieja_pintada() {
    let d = carpeta("foto-releer");
    let m = plano_de_obra();
    let r = rutas_en(&d);
    let b = Banco::nuevo();
    let v = vista_en(&m, [0.0, 0.0], PX);
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m);
    let a = trazar_en(&mut motor, &v, 200);
    motor.terminar();
    let px = b.pintar(&mut motor, &v);
    assert!(tinta_en(&px, 200) > 50);
    assert!(!motor.al_dia(), "sin cambios en el disco no se relee");

    // Por fuera: el mismo trazo (id y version) 120 pixeles mas abajo.
    // Tocando el fichero (no por `Capa`, que al guardar sube la version):
    // el trazo mas bajo, sus puntos van respecto a su x, y.
    let bajar = |ruta: &Path, dy_pantalla: f64, cambio: bool| {
        let mut j: serde_json::Value =
            serde_json::from_str(&pixpin_proyecto::anotado::leer(ruta).unwrap()).unwrap();
        let zoom = v.camara().2;
        let els = j["elements"].as_array_mut().unwrap();
        let e = els
            .iter_mut()
            .filter(|e| e["isDeleted"] != true)
            .max_by(|a, b| {
                a["y"]
                    .as_f64()
                    .unwrap()
                    .total_cmp(&b["y"].as_f64().unwrap())
            })
            .unwrap();
        e["y"] = (e["y"].as_f64().unwrap() + dy_pantalla / zoom).into();
        if cambio {
            // Un cambio de verdad en el otro aparato (al juntar, gana el nuevo).
            e["version"] = (e["version"].as_i64().unwrap() + 5).into();
            e["updated"] = (e["updated"].as_i64().unwrap_or(0) + 60_000).into();
        }
        pixpin_proyecto::anotado::escribir(ruta, &j.to_string()).unwrap();
        let t = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        std::fs::OpenOptions::new()
            .write(true)
            .open(ruta)
            .unwrap()
            .set_modified(t)
            .unwrap();
    };
    let antes = motor.capa.escena.buscar(a).map(|e| e.version);
    bajar(&r.tinta, 120.0, false);
    assert!(motor.al_dia(), "la sincronizacion trajo algo: se relee");
    assert_eq!(
        motor.capa.escena.buscar(a).map(|e| e.version),
        antes,
        "lo peor: misma version, otro sitio"
    );
    // Moviendose (la foto valdria si no se invalidara) y quieta.
    for vista in [corrida(&v, 1.0), corrida(&v, 1.0)] {
        let px = b.pintar(&mut motor, &vista);
        assert_eq!(
            tinta_en(&px, 200),
            0,
            "caso negativo: el trazo no sigue donde estaba"
        );
        assert!(
            tinta_en(&px, 320) > 50,
            "el trazo, donde lo dejo la sincronizacion"
        );
    }

    // Guardar juntando: algo hecho aqui y, mientras, otra vez por fuera.
    let vb = corrida(&v, 1.0);
    trazar_en(&mut motor, &vb, 120);
    bajar(&r.tinta, 100.0, true);
    motor.terminar();
    for vista in [corrida(&vb, 1.0), corrida(&vb, 1.0)] {
        let px = b.pintar(&mut motor, &vista);
        assert_eq!(
            tinta_en(&px, 320),
            0,
            "caso negativo: tras juntar, nada de la foto de antes"
        );
        assert!(tinta_en(&px, 420) > 50, "lo del disco, junto");
        assert!(tinta_en(&px, 120) > 50, "lo de aqui, junto");
    }
    let _ = std::fs::remove_dir_all(&d);
}
