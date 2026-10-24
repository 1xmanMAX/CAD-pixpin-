//! **Lo anotado sobre un plano viaja con el movil, de verdad**: el PC (su
//! almacen de siempre) y un movil de mentira (`DiscoAndroid`, el puerto de
//! `Disco.kt`) se hablan por un socket con el protocolo y el cifrado de
//! verdad, y lo que llega al movil se mira con las cuentas de Android
//! (`CapaDelPlano.encajeDe`). Y, para mirarlo, el lapiz pintado sobre un
//! plano de las muestras a varios zooms.

use std::io::{Read, Write};

use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::Cuaderno;
use pixpin_proyecto::vista::{self, DiscoPc};
use pixpin_sincro::canonico::Json;
use pixpin_sincro::disco::{Disco, GENERAL};
use pixpin_sincro::disco_android::DiscoAndroid;
use pixpin_sincro::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar, sincronizar};
use pixpin_sincro::kotlin;
use serde_json::{Value, json};

use super::pruebas::{carpeta, plano_de_obra, rutas_en, trazar, vista};
use super::*;

struct Par {
    dir: PathBuf,
    pc: DiscoPc,
    movil: DiscoAndroid,
    reloj: Reloj,
}

impl Drop for Par {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn montar(etiqueta: &str) -> Par {
    let dir = carpeta(&format!("movil-{etiqueta}"));
    let pc = DiscoPc::nuevo(&dir.join("PixPin Max"));
    let movil = DiscoAndroid::nuevo(dir.join("movil/files"), "movil");
    presentar(&pc, "id-pc", "Portátil");
    presentar(&movil, "id-movil", "Teléfono");
    let p = Par {
        dir,
        pc,
        movil,
        reloj: Reloj::nuevo(),
    };
    // El grupo lo crea el movil y el PC se une, como lo hizo el usuario.
    crear_grupo(&p.movil, "ABCDE23456", p.reloj.ahora());
    conectado(&p.pc, &p.movil, true, Some("ABCDE23456"), &p.reloj, |_| {}).unwrap();
    p
}

impl Par {
    fn raiz(&self) -> PathBuf {
        self.pc.raiz()
    }

    /// El PC dirige.
    fn desde_pc(&self) {
        sincronizar(&self.pc, &self.movil, &[GENERAL], &self.reloj, &|| {});
    }

    /// El movil dirige y el PC responde.
    fn desde_movil(&self) {
        sincronizar(&self.movil, &self.pc, &[GENERAL], &self.reloj, &|| {});
    }

    /// Un plano adjunto en el chat general del movil; su codigo de mensaje.
    fn plano_del_movil(&self, id: &str, fichero: &str) -> String {
        let rel = format!("guardados/{fichero}");
        let f = self.movil.raiz().join(&rel);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(
            &f,
            format!("0\nSECTION\n2\nENTITIES\n999\n{id}\n0\nENDSEC\n0\nEOF\n"),
        )
        .unwrap();
        let m = Json::de_valor(&json!({
            "id": id, "cuando": self.reloj.tic(), "clase": "ARCHIVO", "ruta": self.movil.absoluta(&rel),
            "nombre": fichero, "numero": self.movil.leer_mensajes().len() + 1, "letra": "a",
        }));
        self.movil.anadir_mensaje(&m).unwrap();
        kotlin::unico(&m)
    }

    /// Donde esta en el PC el adjunto `id` (la ruta que su mensaje senala).
    fn en_el_pc(&self, id: &str) -> PathBuf {
        let g = self.pc.ficha_de(GENERAL).unwrap();
        let c = Cuaderno::leer_de(&almacen::carpeta(&self.raiz(), &g.id)).unwrap();
        let m = c.mensajes.into_iter().find(|m| m.id == id).unwrap();
        vista::ruta_real(&self.raiz(), &g.id, m.ruta.as_deref().unwrap()).unwrap()
    }

    fn en_el_movil(&self, rel: &str) -> PathBuf {
        self.movil.raiz().join(rel)
    }
}

fn de_gz(ruta: &Path) -> String {
    let b = std::fs::read(ruta).unwrap();
    let mut s = String::new();
    flate2::read::GzDecoder::new(&b[..])
        .read_to_string(&mut s)
        .unwrap();
    s
}

fn gz(ruta: &Path, texto: &str) {
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    let mut e = flate2::write::GzEncoder::new(
        std::fs::File::create(ruta).unwrap(),
        flate2::Compression::default(),
    );
    e.write_all(texto.as_bytes()).unwrap();
    e.finish().unwrap();
}

/// Deja la fecha de un fichero un minuto por delante: dos escrituras del
/// mismo largo en el mismo milisegundo se tomarian por la misma.
fn adelantar(f: &Path) {
    let t = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    std::fs::OpenOptions::new()
        .write(true)
        .open(f)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

/// El encaje con que el movil lee su capa: el de su `.hoja`.
fn encaje_del_movil(p: &Par, u: &str, m: &Modelo) -> Encaje {
    let t = std::fs::read_to_string(p.en_el_movil(&format!("pins/draw/anot-{u}.hoja")))
        .expect("la hoja llego al movil");
    let h = MarcoDeLaHoja::de_texto(&t).expect("hoja de dos lineas");
    Encaje::de_la_hoja([h.x0 as f64, h.y0 as f64, h.x1 as f64, h.y1 as f64], m.caja).unwrap()
}

fn vivos(texto: &str) -> Vec<Value> {
    let v: Value = serde_json::from_str(texto).unwrap();
    v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["isDeleted"] != true)
        .cloned()
        .collect()
}

/// Un punto de un trazo del fichero: `[x, y]` o `{"x", "y"}` (el del movil).
fn punto(v: &Value) -> (f64, f64) {
    match v {
        Value::Array(a) => (a[0].as_f64().unwrap(), a[1].as_f64().unwrap()),
        o => (o["x"].as_f64().unwrap(), o["y"].as_f64().unwrap()),
    }
}

/// **Ida y vuelta de lo anotado sobre un plano del chat**: el PC anota,
/// acota y pone un marco; llega al movil con los nombres, la `.hoja` y las
/// figuras que lee `CapaDelPlano`, en su sitio con la cuenta de Android. El
/// movil borra el trazo y pone su cota; vuelve al PC. El PC quita el marco;
/// llega borrado. Casos negativos: lo de otro plano no se mezcla y un
/// fichero danado del movil no rompe nada.
#[test]
fn lo_anotado_sobre_un_plano_viaja_en_los_dos_sentidos_y_lo_ultimo_gana() {
    let p = montar("ida-y-vuelta");
    let u = p.plano_del_movil("pl", "planta.dxf");
    let u_otro = p.plano_del_movil("ot", "otro.dxf");
    p.desde_pc();
    let plano = p.en_el_pc("pl");
    assert!(plano.is_file(), "el plano llego al PC");
    let m = plano_de_obra();
    let r = rutas_de_la_capa(&p.raiz(), &plano);
    assert!(
        r.tinta.to_string_lossy().contains(&format!("anot-{u}"))
            && r.hoja
                .to_string_lossy()
                .ends_with(&format!("anot-{u}.hoja")),
        "{r:?}"
    );

    // 1. En el PC: un trazo, una cota y un marco.
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m);
    let v = vista(2.5 / 20.0, pixpin_cad::anotado::unidad_de_la_capa(&m));
    let trazo = trazar(&mut motor, &v);
    motor.terminar();
    let cota = vec![[100.0, 200.0], [1100.0, 200.0]];
    let marco = [-2000.0, -1000.0, 3000.0, 1500.0];
    motor.poner_cotas_y_marcos(std::slice::from_ref(&cota), &[marco]);
    p.desde_pc();
    let capa_movil = p.en_el_movil(&format!("pins/draw/anot-{u}.excalidraw.gz"));
    let alla = vivos(&de_gz(&capa_movil));
    assert_eq!(alla.len(), 3, "{alla:?}");
    // En la forma que lee la `Scene` del movil: los puntos como `Pt`.
    assert!(
        alla.iter().all(|x| x.get("points").is_none_or(|p| p
            .as_array()
            .unwrap()
            .iter()
            .all(|q| q.get("x").is_some()))),
        "{alla:?}"
    );
    let e = encaje_del_movil(&p, &u, &m);
    // La cota cae en el plano donde se puso, con la cuenta de Android.
    let c = alla
        .iter()
        .find(|x| x["groupIds"][0] == "plano-cota")
        .expect("cota");
    assert_eq!(c["type"], "line");
    let (x0, y0) = (c["x"].as_f64().unwrap(), c["y"].as_f64().unwrap());
    let pts: Vec<[f64; 2]> = c["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(punto)
        .map(|(dx, dy)| e.a_plano([x0 + dx, y0 + dy]))
        .collect();
    assert_eq!(pts.len(), 2);
    for (a, b) in pts.iter().zip(&cota) {
        assert!(
            (a[0] - b[0]).abs() < 0.5 && (a[1] - b[1]).abs() < 0.5,
            "{a:?} contra {b:?}"
        );
    }
    let f = alla
        .iter()
        .find(|x| x["groupIds"][0] == "plano-marco")
        .expect("marco");
    assert_eq!(f["type"], "frame");
    let esquina = e.a_plano([f["x"].as_f64().unwrap(), f["y"].as_f64().unwrap()]);
    assert!(
        (esquina[0] - marco[0]).abs() < 0.5 && (esquina[1] - marco[3]).abs() < 0.5,
        "{esquina:?}"
    );
    // El trazo, con su grosor a la escala de la letra.
    let t = alla
        .iter()
        .find(|x| x["type"] == "freedraw")
        .expect("trazo");
    assert!(
        (t["strokeWidth"].as_f64().unwrap() - trazo.grosor as f64).abs() < 1e-3,
        "{t}"
    );
    let id_trazo = t["id"].as_str().unwrap().to_string();

    // 2. En el movil: borra el trazo (con version nueva) y acota otra cosa,
    // como su `CapaDelPlano.con`.
    p.reloj.saltar(10_000);
    let mut todo: Value = serde_json::from_str(&de_gz(&capa_movil)).unwrap();
    let ahora = p.reloj.tic();
    let elementos = todo["elements"].as_array_mut().unwrap();
    for x in elementos
        .iter_mut()
        .filter(|x| x["id"] == id_trazo.as_str())
    {
        x["isDeleted"] = json!(true);
        x["version"] = json!(x["version"].as_i64().unwrap() + 1);
        x["updated"] = json!(ahora);
    }
    let suya = [[-500.0, -500.0], [-500.0, 700.0]];
    let (a, b) = (e.a_capa(suya[0]), e.a_capa(suya[1]));
    let id_suya = pixpin_cad::anotado::huella(
        GRUPO_COTA,
        &[suya[0][0], suya[0][1], suya[1][0], suya[1][1]],
    );
    elementos.push(json!({
        "id": id_suya, "type": "line", "x": a[0], "y": a[1], "width": 0.0, "height": (b[1] - a[1]).abs(),
        "strokeColor": "#ff8a00", "strokeWidth": 1.0, "groupIds": ["plano-cota"], "seed": 4,
        "version": 1, "versionNonce": 7, "updated": ahora, "isDeleted": false,
        "points": [{"x": 0.0, "y": 0.0}, {"x": b[0] - a[0], "y": b[1] - a[1]}],
    }));
    gz(&capa_movil, &todo.to_string());
    adelantar(&capa_movil);
    p.desde_movil();
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m);
    assert!(
        motor
            .capa
            .escena
            .visibles()
            .all(|x| !matches!(x.figura, Figura::Lapiz { .. })),
        "el trazo borrado en el movil se borra en el PC"
    );
    let (cotas, marcos) = motor.cotas_y_marcos();
    assert_eq!((cotas.len(), marcos.len()), (2, 1), "{cotas:?}");
    let la_suya = cotas
        .iter()
        .find(|c| (c[0][0] + 500.0).abs() < 0.5)
        .expect("la cota del movil");
    assert!((la_suya[1][1] - 700.0).abs() < 0.5, "{la_suya:?}");

    // 3. Otra vez en el PC: se quita el marco; llega como borrado.
    p.reloj.saltar(10_000);
    motor.poner_cotas_y_marcos(&cotas, &[]);
    adelantar(&r.tinta);
    p.desde_pc();
    let alla: Value = serde_json::from_str(&de_gz(&capa_movil)).unwrap();
    let f = alla["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["groupIds"][0] == "plano-marco")
        .unwrap()
        .clone();
    assert_eq!(f["isDeleted"], true, "{f}");
    let quedan = vivos(&alla.to_string());
    assert_eq!(quedan.len(), 2);
    // La cota del movil sigue siendo la suya (mismo id): no se borro y
    // volvio a nacer con otro al recalcularla en el PC.
    assert!(
        quedan.iter().any(|x| x["id"] == id_suya.as_str()),
        "{quedan:?}"
    );
    let todas = alla["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["groupIds"][0] == "plano-cota")
        .count();
    assert_eq!(todas, 2, "sin cotas de mas, ni borradas: {alla}");

    // Casos negativos. Lo de otro plano no se mezcla: el otro no tiene nada.
    let otro = MotorDelPlano::nuevo(rutas_de_la_capa(&p.raiz(), &p.en_el_pc("ot")), &m);
    assert!(otro.capa.vacia() && otro.cotas_y_marcos().0.is_empty());
    // Y un fichero danado del movil (capa ilegible y hoja sin sentido)
    // llega y se lee vacio, sin romper la vuelta ni el visor.
    gz(
        &p.en_el_movil(&format!("pins/draw/anot-{u_otro}.excalidraw.gz")),
        "{\"elements\": [ roto",
    );
    std::fs::write(
        p.en_el_movil(&format!("pins/draw/anot-{u_otro}.hoja")),
        "a,b,c,d\nv1\n",
    )
    .unwrap();
    p.desde_movil();
    let otro = MotorDelPlano::nuevo(rutas_de_la_capa(&p.raiz(), &p.en_el_pc("ot")), &m);
    assert!(otro.capa.vacia() && otro.cotas_y_marcos().0.is_empty());
    // El de antes sigue igual.
    assert_eq!(MotorDelPlano::nuevo(r, &m).cotas_y_marcos().0.len(), 2);
}

// ------------------------------------------- el camino real del 13-oct

/// El plano visto por el PC del usuario: el mismo de [`plano_de_obra`] con
/// **otro origen** (el PC pone el cero en el centro de lo que importa y el
/// movil en el de la caja: en `11.15 PLANO DE SENALIZACION VIAL HORIZONTAL`
/// van 16,8 y 18,1 unidades de diferencia). Con la `.hoja` da igual; sin
/// probarlo, la prueba de arriba no lo veia.
fn plano_del_pc() -> Modelo {
    let mut m = plano_de_obra();
    m.caja = [
        m.caja[0] + CORRIDO[0],
        m.caja[1] + CORRIDO[1],
        m.caja[2] + CORRIDO[0],
        m.caja[3] + CORRIDO[1],
    ];
    m
}

/// Lo que va del cero del movil al del PC: un punto del plano en las
/// coordenadas del PC es el del movil mas esto.
const CORRIDO: [f32; 2] = [1234.0, -567.0];

/// La capa como la escribe `CapaDelPlano` del movil: su encaje es la unidad
/// de la letra con el cero en SU origen.
fn encaje_propio_del_movil(m: &Modelo) -> Encaje {
    Encaje::de_unidad(pixpin_cad::anotado::unidad_de_la_capa(m))
}

/// Un trazo del movil (`freedraw` del motor de Android, id de 21 letras)
/// por los puntos del plano `pts`, en las unidades de su capa.
fn trazo_del_movil(id: &str, pts: &[[f64; 2]], e: &Encaje, ahora: i64) -> Value {
    let q: Vec<[f64; 2]> = pts.iter().map(|p| e.a_capa(*p)).collect();
    let (x0, y0) = (q[0][0], q[0][1]);
    json!({
        "id": id, "type": "freedraw", "x": x0, "y": y0, "width": 10.0, "height": 10.0,
        "strokeColor": "#1e1e1e", "strokeWidth": 1.0, "groupIds": [], "seed": 3,
        "version": 1, "versionNonce": 11, "updated": ahora, "isDeleted": false,
        "points": q.iter().map(|p| json!({"x": p[0] - x0, "y": p[1] - y0})).collect::<Vec<_>>(),
    })
}

/// **Guardar del movil** (`PlanoActivity.guardarCapa`): la `.hoja` y la capa
/// entera, sin lo borrado (`purgeDeleted`): lo que el movil quita ya no esta.
fn guardar_en_el_movil(p: &Par, u: &str, elementos: &[Value], m_movil: &Modelo) {
    let h = encaje_propio_del_movil(m_movil).hoja(m_movil.caja);
    let hoja = MarcoDeLaHoja::nuevo(h[0] as f32, h[1] as f32, h[2] as f32, h[3] as f32);
    let ruta_hoja = p.en_el_movil(&format!("pins/draw/anot-{u}.hoja"));
    std::fs::create_dir_all(ruta_hoja.parent().unwrap()).unwrap();
    std::fs::write(&ruta_hoja, hoja.a_texto()).unwrap();
    let capa = p.en_el_movil(&format!("pins/draw/anot-{u}.excalidraw.gz"));
    gz(
        &capa,
        &json!({"elements": elementos, "files": {}}).to_string(),
    );
    adelantar(&capa);
}

/// Lo que tiene ahora el movil en la capa del plano `u`.
fn en_el_movil_ahora(p: &Par, u: &str) -> Vec<Value> {
    vivos(&de_gz(
        &p.en_el_movil(&format!("pins/draw/anot-{u}.excalidraw.gz")),
    ))
}

fn ids(l: &[Value]) -> Vec<String> {
    l.iter()
        .map(|x| x["id"].as_str().unwrap().to_string())
        .collect()
}

/// Un trazo del lapiz del PC a la altura `y` de la ventana (otra que la de
/// [`trazar`], para no pintar encima).
fn trazar_en(motor: &mut MotorDelPlano, v: &VistaPlano, y: i32) -> Elemento {
    motor
        .tinta
        .gesto
        .tomar_herramienta(pixpin_motor2d::gesto::Herramienta::Lapiz);
    motor.evento(EventoPlano::Mover(300, y), v);
    motor.evento(EventoPlano::Bajar(300, y), v);
    for x in (303..=600).step_by(3) {
        motor.evento(EventoPlano::Mover(x, y + (x / 3 % 2)), v);
    }
    motor.evento(EventoPlano::Subir(600, y), v);
    motor
        .capa
        .escena
        .visibles()
        .filter(|e| matches!(e.figura, Figura::Lapiz { .. }))
        .last()
        .cloned()
        .expect("trazo")
}

/// **El fallo del usuario (13-oct-2026): lo anotado en el PC no llegaba al
/// movil.** El camino de verdad: un DWG que mando el movil (`pixpin:files/
/// guardados/…`), anotado alli primero (su `.hoja`, con SU origen), luego
/// en el PC; el movil borra uno de sus trazos (y al guardar lo quita del
/// fichero, `purgeDeleted`); el PC vuelve a abrir y anota otra vez.
///
/// El PC numeraba lo nuevo `pc<n>` con el numero de elementos que habia
/// al abrir mas uno: con uno menos, el trazo nuevo salia con el id del
/// trazo del PC de la vez anterior. Dos figuras con el mismo id no se
/// juntan figura por figura al sincronizar (`fusion::con_ids`) y en el
/// movil una tapa a la otra: lo nuevo del PC no aparecia.
#[test]
fn lo_anotado_en_el_pc_llega_al_movil_aunque_el_movil_haya_borrado_algo() {
    let p = montar("pc-tras-borrar");
    let u = p.plano_del_movil("pl", "11.15 PLANO.dwg");
    p.desde_pc();
    let (m_movil, m_pc) = (plano_de_obra(), plano_del_pc());
    let e_movil = encaje_propio_del_movil(&m_movil);
    // 1. En el movil: tres trazos.
    let ahora = p.reloj.tic();
    let suyos: Vec<Value> = (0..3)
        .map(|i| {
            let y = -2000.0 + 1000.0 * i as f64;
            trazo_del_movil(
                &format!("movilTrazo{i}aaaaaaaaaa"),
                &[[-1000.0, y], [1000.0, y]],
                &e_movil,
                ahora,
            )
        })
        .collect();
    guardar_en_el_movil(&p, &u, &suyos, &m_movil);
    p.desde_movil();
    let r = rutas_de_la_capa(&p.raiz(), &p.en_el_pc("pl"));
    assert!(
        r.tinta.ends_with(format!("lienzos/anot-{u}.excalidraw")),
        "{r:?}"
    );

    // 2. En el PC: un trazo; viaja.
    let v = vista(2.5 / 20.0, pixpin_cad::anotado::unidad_de_la_capa(&m_pc));
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m_pc);
    let primero = trazar_en(&mut motor, &v, 300);
    motor.terminar();
    p.desde_pc();
    let alla = en_el_movil_ahora(&p, &u);
    assert_eq!(alla.len(), 4, "{:?}", ids(&alla));
    let id_primero = ids(&alla)
        .into_iter()
        .find(|i| !i.starts_with("movil"))
        .unwrap();

    // 3. En el movil: borra su primer trazo (y lo quita del fichero).
    p.reloj.saltar(10_000);
    let quedan: Vec<Value> = de_gz(&p.en_el_movil(&format!("pins/draw/anot-{u}.excalidraw.gz")))
        .parse::<Value>()
        .unwrap()["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["id"] != "movilTrazo0aaaaaaaaaa")
        .cloned()
        .collect();
    guardar_en_el_movil(&p, &u, &quedan, &m_movil);
    p.desde_movil();

    // 4. En el PC, otra vez: otro trazo.
    p.reloj.saltar(10_000);
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m_pc);
    let segundo = trazar_en(&mut motor, &v, 600);
    motor.terminar();
    p.desde_pc();

    // En el movil: sus dos trazos y los dos del PC, cada uno con su id.
    let alla = en_el_movil_ahora(&p, &u);
    let todos = ids(&alla);
    let distintos: std::collections::BTreeSet<&String> = todos.iter().collect();
    assert_eq!(
        distintos.len(),
        todos.len(),
        "ids repetidos en el movil: {todos:?}"
    );
    assert_eq!(alla.len(), 4, "{todos:?}");
    assert!(
        todos.contains(&id_primero),
        "el primero del PC sigue: {todos:?}"
    );
    assert!(
        !todos.contains(&"movilTrazo0aaaaaaaaaa".to_string()),
        "lo borrado en el movil no vuelve"
    );
    // Y cada trazo del PC cae en el movil donde se dibujo, con la cuenta de
    // Android (`encajeDe` con la caja de SU plano, la de su origen).
    let e = encaje_del_movil(&p, &u, &m_movil);
    let memoria = Encaje::de_unidad(pixpin_cad::anotado::unidad_de_la_capa(&m_pc));
    for (t, nacido) in [
        (&id_primero, &primero),
        (
            &todos
                .iter()
                .find(|i| !i.starts_with("movil") && **i != id_primero)
                .unwrap()
                .clone(),
            &segundo,
        ),
    ] {
        let x = alla.iter().find(|x| x["id"] == t.as_str()).unwrap();
        let en_movil = e.a_plano([x["x"].as_f64().unwrap(), x["y"].as_f64().unwrap()]);
        let en_pc = memoria.a_plano([nacido.x as f64, nacido.y as f64]);
        let fuera = [
            (en_movil[0] + CORRIDO[0] as f64 - en_pc[0]).abs(),
            (en_movil[1] + CORRIDO[1] as f64 - en_pc[1]).abs(),
        ];
        assert!(
            fuera[0] < 0.5 && fuera[1] < 0.5,
            "{t}: {en_movil:?} + corrido contra {en_pc:?}"
        );
    }
}

/// **Lo que llega del movil con el plano abierto en el PC no se pisa al
/// guardar** (el reflejo del fallo de Android, ver
/// `docs/investigacion/2026-10-13-para-android.md`): el PC guardaba la capa
/// que leyo al abrir mas lo suyo, y lo que el movil mando mientras tanto
/// desaparecia (y, al sincronizar, desaparecia tambien en el movil). Ahora
/// se junta con lo del disco como junta la sincronizacion (`fusion::json`).
/// Caso negativo: lo que el movil borro mientras tanto no vuelve.
#[test]
fn lo_que_llega_con_el_plano_abierto_se_junta_al_guardar_y_lo_borrado_no_vuelve() {
    let p = montar("abierto");
    let u = p.plano_del_movil("pl", "plano.dwg");
    p.desde_pc();
    let (m_movil, m_pc) = (plano_de_obra(), plano_del_pc());
    let e_movil = encaje_propio_del_movil(&m_movil);
    let ahora = p.reloj.tic();
    let suyo = trazo_del_movil(
        "movilTrazoAaaaaaaaaaa",
        &[[0.0, 0.0], [500.0, 500.0]],
        &e_movil,
        ahora,
    );
    guardar_en_el_movil(&p, &u, std::slice::from_ref(&suyo), &m_movil);
    p.desde_movil();
    let r = rutas_de_la_capa(&p.raiz(), &p.en_el_pc("pl"));
    let v = vista(2.5 / 20.0, pixpin_cad::anotado::unidad_de_la_capa(&m_pc));
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m_pc);
    trazar_en(&mut motor, &v, 300);
    motor.terminar();
    p.desde_pc();
    let id_viejo_del_pc = ids(&en_el_movil_ahora(&p, &u))
        .into_iter()
        .find(|i| !i.starts_with("movil"))
        .unwrap();

    // El PC abre el plano y se queda abierto.
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m_pc);
    // Mientras, el movil anota otro trazo y borra el del PC; sincroniza.
    p.reloj.saltar(10_000);
    let nuevo = trazo_del_movil(
        "movilTrazoBbbbbbbbbbb",
        &[[-3000.0, 0.0], [-2000.0, 100.0]],
        &e_movil,
        p.reloj.tic(),
    );
    guardar_en_el_movil(&p, &u, &[suyo.clone(), nuevo], &m_movil);
    p.desde_movil();
    // El PC, con lo que leyo al abrir, anota y guarda.
    let _ = trazar_en(&mut motor, &v, 600);
    motor.terminar();
    let aqui = vivos(&std::fs::read_to_string(&r.tinta).unwrap());
    let aqui_ids = ids(&aqui);
    assert!(
        aqui_ids.iter().any(|i| i == "movilTrazoBbbbbbbbbbb"),
        "lo del movil se queda: {aqui_ids:?}"
    );
    assert!(
        !aqui_ids.contains(&id_viejo_del_pc),
        "lo borrado en el movil no vuelve: {aqui_ids:?}"
    );
    assert_eq!(aqui.len(), 3, "{aqui_ids:?}");
    // Lo que queda en memoria es lo del disco (se sigue anotando sobre ello).
    assert_eq!(motor.capa.escena.visibles().count(), 3);
    // Y al movil llega lo mismo.
    p.desde_pc();
    let mut alla = ids(&en_el_movil_ahora(&p, &u));
    let mut aqui_ids = aqui_ids;
    alla.sort();
    aqui_ids.sort();
    assert_eq!(alla, aqui_ids);
}

/// **Lo que llega del movil se ve sin cerrar el plano**: el visor relee la
/// capa cuando cambia en el disco y aqui no hay nada sin guardar. Caso
/// negativo: con algo anotado sin guardar, no se relee (se juntara al
/// guardar).
#[test]
fn lo_que_llega_del_movil_se_relee_con_el_plano_abierto_si_aqui_no_hay_nada_a_medias() {
    let p = montar("releer");
    let u = p.plano_del_movil("pl", "plano.dwg");
    p.desde_pc();
    let m_movil = plano_de_obra();
    let e_movil = encaje_propio_del_movil(&m_movil);
    let uno = trazo_del_movil(
        "movilTrazoAaaaaaaaaaa",
        &[[0.0, 0.0], [500.0, 500.0]],
        &e_movil,
        p.reloj.tic(),
    );
    guardar_en_el_movil(&p, &u, std::slice::from_ref(&uno), &m_movil);
    p.desde_movil();
    let r = rutas_de_la_capa(&p.raiz(), &p.en_el_pc("pl"));
    let m_pc = plano_del_pc();
    let mut motor = MotorDelPlano::nuevo(r.clone(), &m_pc);
    assert_eq!(motor.capa.escena.visibles().count(), 1);
    assert!(!motor.al_dia(), "sin cambios en el disco no se relee");
    // Llega otro.
    p.reloj.saltar(10_000);
    let dos = trazo_del_movil(
        "movilTrazoBbbbbbbbbbb",
        &[[100.0, 0.0], [600.0, 500.0]],
        &e_movil,
        p.reloj.tic(),
    );
    guardar_en_el_movil(&p, &u, &[uno.clone(), dos.clone()], &m_movil);
    p.desde_movil();
    assert!(motor.al_dia(), "cambio en el disco: se relee");
    assert_eq!(motor.capa.escena.visibles().count(), 2);
    // Caso negativo: con un trazo sin guardar no se relee y no se pierde.
    let v = vista(2.5 / 20.0, pixpin_cad::anotado::unidad_de_la_capa(&m_pc));
    trazar_en(&mut motor, &v, 300);
    p.reloj.saltar(10_000);
    let tres = trazo_del_movil(
        "movilTrazoCcccccccccc",
        &[[200.0, 0.0], [700.0, 500.0]],
        &e_movil,
        p.reloj.tic(),
    );
    guardar_en_el_movil(&p, &u, &[uno, dos, tres], &m_movil);
    p.desde_movil();
    assert!(!motor.al_dia());
    assert_eq!(
        motor.capa.escena.visibles().count(),
        3,
        "el trazo del PC sigue en memoria"
    );
    motor.terminar();
    assert_eq!(
        vivos(&std::fs::read_to_string(&r.tinta).unwrap()).len(),
        4,
        "y al guardar se junta con lo que llego"
    );
}

// ------------------------------------------------------- para mirarlo

/// **El lapiz sobre un plano de verdad, a tres zooms**, antes (la unidad de
/// la milesima del lado) y ahora (la de la letra): PNG para mirarlos.
/// `PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin --bin pixpinmax muestra_del_lapiz_sobre_un_plano -- --ignored --nocapture`
#[test]
#[ignore = "necesita GPU y una muestra de E:\\pixpin-cad\\muestras; genera PNG"]
fn muestra_del_lapiz_sobre_un_plano_a_varios_zooms() {
    let carpeta_png = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = carpeta("muestra");
    // Una copia: el lector nunca abre el original.
    let copia = d.join("plano.dwg");
    std::fs::copy(
        r"E:\pixpin-cad\muestras\1791378283866_11.18 PLANO DE CUNETAS FLORIDA.dwg",
        &copia,
    )
    .expect("muestra");
    let (m, _) = pixpin_cad::convertir::convertir_fichero(&copia).expect("plano");
    let u = pixpin_cad::anotado::unidad_de_la_capa(&m);
    let letra = u * 20.0 / 1.4;
    // Donde hay letras: el tramo con mas.
    let cu = m
        .tramos_letras
        .iter()
        .max_by_key(|t| t.cuantos)
        .map_or(m.caja_util(), |t| t.caja);
    let centro = [
        ((cu[0] + cu[2]) / 2.0) as f64,
        ((cu[1] + cu[3]) / 2.0) as f64,
    ];
    // Mirando a leer: la letra tipica del plano a unos 16 pixeles.
    let leer = letra / 16.0;
    println!(
        "unidad ahora {u} (letra {letra}), antes {}",
        pixpin_cad::anotado::unidad_antigua(&m)
    );
    println!(
        "origen {:?} caja {:?} util {:?} centro {centro:?}",
        m.origen,
        m.caja,
        m.caja_util()
    );
    let (w, h) = (1000u32, 640u32);
    let dispositivo = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor_d2d = MotorRender::nuevo(dispositivo.d3d()).expect("motor");
    let fuera = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(
        &motor_d2d,
        dispositivo.d3d(),
        w,
        h,
    )
    .expect("superficie");
    for (nombre, unidad) in [
        ("ahora", u),
        ("antes", pixpin_cad::anotado::unidad_antigua(&m)),
    ] {
        let mut motor = MotorDelPlano::nuevo(rutas_en(&d.join(nombre)), &m);
        let v = VistaPlano {
            centro,
            px: leer,
            ancho: w,
            alto: h,
            origen_pantalla: (0, 0),
            escala_por_cien: 100,
            unidad,
            claro: true,
        };
        // Una onda de lado a lado y un circulo, con el lapiz medio.
        motor
            .tinta
            .gesto
            .tomar_herramienta(pixpin_motor2d::gesto::Herramienta::Lapiz);
        motor.evento(EventoPlano::Bajar(150, 380), &v);
        for x in 151..850 {
            let y = 380.0 + 60.0 * ((x - 150) as f32 / 70.0).sin();
            motor.evento(EventoPlano::Mover(x, y as i32), &v);
        }
        motor.evento(EventoPlano::Subir(850, 380), &v);
        motor.evento(EventoPlano::Bajar(560, 200), &v);
        for i in 1..=120 {
            let a = i as f32 / 120.0 * std::f32::consts::TAU;
            motor.evento(
                EventoPlano::Mover(500 + (60.0 * a.cos()) as i32, 200 + (60.0 * a.sin()) as i32),
                &v,
            );
        }
        motor.evento(EventoPlano::Subir(560, 200), &v);
        for (z, k) in [("lejos", 6.0), ("leer", 1.0), ("cerca", 1.0 / 6.0)] {
            let vz = VistaPlano { px: leer * k, ..v };
            let mitad = [w as f64 / 2.0 * vz.px, h as f64 / 2.0 * vz.px];
            let marco = [
                centro[0] - mitad[0],
                centro[1] - mitad[1],
                centro[0] + mitad[0],
                centro[1] + mitad[1],
            ];
            // El plano (con el pintor de imprimir) y la tinta encima por el
            // camino del visor: `Anotador::pintar`, con su propio Direct2D
            // sobre la textura del fotograma.
            motor_d2d
                .dibujar(&fuera.destino, |p| {
                    p.limpiar(Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    });
                    super::super::imprimir::dibujar(
                        p,
                        &m,
                        &marco,
                        &[],
                        None,
                        (0.0, 0.0, w as f32, h as f32),
                        Some((1.0 / vz.px) as f32),
                    );
                })
                .expect("pintar el plano");
            motor.pintar(dispositivo.d3d(), fuera.textura(), &vz, false);
            fuera.esperar_gpu().expect("esperar");
            let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
            let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba {
                ancho,
                alto,
                pixeles,
            })
            .expect("png");
            let salida = carpeta_png.join(format!("cad-lapiz-{nombre}-{z}.png"));
            std::fs::write(&salida, png).expect("guardar");
            println!("PNG {}", salida.display());
        }
    }
    let _ = std::fs::remove_dir_all(&d);
}
