//! **Ver planos DWG y DXF** (8-oct-2026): el usuario pidio un visor «ligero,
//! preciso y fluido aunque el plano sea grande y el equipo modesto», que se
//! abra como un pin. El visor esta en `pixpin-cad`; aqui, como se abre desde
//! PixPin.
//!
//! **El plano se lee en otro proceso** (`pixpinmax.exe --cad-convertir
//! <plano> <cache>`): la libreria que lee los DWG se defiende de un fichero
//! roto con `catch_unwind`, y el ejecutable va con `panic = "abort"`; asi un
//! plano dañado solo tumba a ese proceso, no a PixPin. De paso, la memoria
//! que usa la lectura se devuelve entera al terminar, y lo leido queda en
//! `<raiz>/cache/cad/`: abrir otra vez el mismo plano es instantaneo.

use std::path::{Path, PathBuf};

use pixpin_store::{Idioma, Ubicacion};

/// Lo anotado sobre el plano (la capa anot-<uid>).
pub(crate) mod anotar;
/// Exportar el plano: HTML por capas, PDF, PNG y DXF (13-oct-2026).
pub(crate) mod exportar;
/// Imprimir los marcos del plano, con su lamina y su membrete.
mod imprimir;
/// Los textos del plano con su sitio, para buscar en ellos (14-oct-2026).
pub(crate) mod textos;

/// Donde mirar al abrir un plano (`[x, y, ancho]` del dibujo): el
/// asistente llevando a un texto encontrado (`abrir` con `zona`). Como la
/// pagina del lector de PDF, se toma al abrir.
static VISTA_AL_ABRIR: std::sync::Mutex<Vec<(PathBuf, [f64; 3])>> =
    std::sync::Mutex::new(Vec::new());

/// **Que la proxima vez que se abra `plano` mire ahi.**
pub fn abrir_en(plano: &Path, vista: [f64; 3]) {
    let mut v = VISTA_AL_ABRIR.lock().unwrap_or_else(|e| e.into_inner());
    v.retain(|(r, _)| r != plano);
    v.push((plano.to_path_buf(), vista));
}

fn tomar_vista_al_abrir(plano: &Path) -> Option<[f64; 3]> {
    let mut v = VISTA_AL_ABRIR.lock().unwrap_or_else(|e| e.into_inner());
    let i = v.iter().position(|(r, _)| r == plano)?;
    Some(v.remove(i).1)
}

/// El plano leido (de la cache o en el proceso aparte), para quien no es
/// el visor: el asistente anotando encima.
pub(crate) fn modelo(raiz: &Path, plano: &Path) -> Result<pixpin_cad::modelo::Modelo, String> {
    cargar(raiz, plano)
}

/// La orden con que PixPin se llama a si misma para exportar un plano.
pub const ORDEN_EXPORTAR: &str = exportar::ORDEN;

/// La orden con que PixPin se llama a si misma para leer un plano.
pub const ORDEN: &str = "--cad-convertir";

/// Cuantos planos se guardan ya leidos (los mas recientes).
const EN_CACHE: usize = 24;

pub fn se_abre(nombre: &str) -> bool {
    matches!(pixpin_docs::extension(nombre).as_str(), "dwg" | "dxf")
}

/// Si PixPin se lanzo para leer un plano, lo lee y dice con que codigo
/// salir. `None`: es un arranque normal.
pub fn convertir_si_toca() -> Option<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == ORDEN_EXPORTAR) {
        return Some(exportar::en_este_proceso(&args[2..]));
    }
    if args.get(1).is_some_and(|a| a == textos::ORDEN) {
        return Some(textos::en_este_proceso(&args[2..]));
    }
    if args.get(1).is_none_or(|a| a != ORDEN) {
        return None;
    }
    let (Some(entrada), Some(salida)) = (args.get(2), args.get(3)) else {
        return Some(2);
    };
    let (entrada, salida) = (PathBuf::from(entrada), PathBuf::from(salida));
    // La carpeta de letras del usuario (el boton «Aa»), si la hay: manda
    // sobre las de Autodesk, Windows y Hershey.
    let letras = args.get(4).map(PathBuf::from);
    Some(
        match pixpin_cad::letras::convertir_con_letras(&entrada, letras) {
            Ok(modelo) => {
                let temporal = salida.with_extension("tmp");
                let escrito = std::fs::write(&temporal, modelo.a_bytes())
                    .and_then(|()| std::fs::rename(&temporal, &salida));
                if escrito.is_ok() { 0 } else { 3 }
            }
            Err(e) => {
                let _ = std::fs::write(salida.with_extension("error"), e);
                1
            }
        },
    )
}

/// El nombre del plano en la cache: su ruta, su tamano y su fecha.
pub fn clave(ruta: &Path) -> Option<String> {
    let m = std::fs::metadata(ruta).ok()?;
    let fecha = m
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    let texto = format!("{}|{}|{}", ruta.display(), m.len(), fecha);
    // FNV-1a de 64 bits: basta para no confundir dos planos.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    Some(format!("{h:016x}"))
}

/// **La carpeta local de los marcadores de un plano o un modelo** que no es
/// de ningun chat (19-oct-2026): `<raiz>/marcadores/<huella de su ruta>/`,
/// con `vistas.json`, `marcos.json` y, en un modelo, la capa de lo anotado
/// sobre cada marcador (`<id>.excalidraw` y `<id>.hoja`). Por la ruta y no
/// por la fecha: lo marcado sigue alli aunque el fichero se guarde de
/// nuevo. Fuera de la cache, que se poda. Solo en este PC: la ruta es de
/// este aparato (hasta el 19-oct aqui iban tambien los de los adjuntos del
/// chat; ver [`SitioDeMarcadores::abrir`]).
pub fn carpeta_de_marcadores(raiz: &Path, ruta: &Path) -> PathBuf {
    let texto = ruta.to_string_lossy().to_lowercase().replace('/', "\\");
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    raiz.join("marcadores").join(format!("{h:016x}"))
}

/// La marca de que lo de una carpeta local ya paso a los ficheros del
/// mensaje (se pasa una vez; lo de antes se queda, como `copiarSiFalta`).
const YA_PASADO: &str = "pasado-al-chat.txt";

/// **Donde viven los marcadores de un plano o un modelo** (19-oct-2026).
///
/// - De un adjunto del chat, **en los ficheros de su mensaje, que viajan**
///   (`pins/draw/` portatil, sin subcarpetas: `pixpin_sincro::anotado`):
///   `anot-<uid>.vistas.json` (los marcadores, del plano y del modelo),
///   `anot-<uid>.marcos.json` (los marcos para imprimir del modelo) y, por
///   cada marcador del modelo con algo anotado, `anot-<uid>-m<id>.excalidraw.gz`
///   y `anot-<uid>-m<id>.hoja` (`<id>` es el `id` del marcador tal cual,
///   asi que en el PC sale `-mm…`).
/// - Si no, en la carpeta local ([`carpeta_de_marcadores`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SitioDeMarcadores {
    Adjunto(pixpin_proyecto::anotado::Base),
    Local(PathBuf),
}

/// El id de un marcador sin nada que salga de su carpeta (lo pone el visor:
/// `m` y cifras).
fn id_limpio(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

impl SitioDeMarcadores {
    /// El sitio de `ruta`, sin tocar nada.
    pub fn de(raiz: &Path, ruta: &Path) -> SitioDeMarcadores {
        use pixpin_proyecto::anotado as an;
        if let Some(r) = an::raiz_de(ruta)
            && let Some(b) = an::base_del_documento(&r, ruta)
        {
            return SitioDeMarcadores::Adjunto(b);
        }
        SitioDeMarcadores::Local(carpeta_de_marcadores(raiz, ruta))
    }

    /// **El sitio de `ruta` al abrirlo**: si es un adjunto y en la carpeta
    /// local quedaba algo de antes (lo del 19-oct, por la ruta), pasa a los
    /// ficheros del mensaje una vez, sumado a lo que ya hubiera (lo que
    /// trajo el movil se queda).
    pub fn abrir(raiz: &Path, ruta: &Path) -> SitioDeMarcadores {
        let s = SitioDeMarcadores::de(raiz, ruta);
        if matches!(s, SitioDeMarcadores::Adjunto(_)) {
            s.pasar_de(&carpeta_de_marcadores(raiz, ruta));
        }
        s
    }

    /// Pasa lo de la carpeta local `vieja` a este sitio, una vez. Dice si paso algo.
    pub fn pasar_de(&self, vieja: &Path) -> bool {
        let fichero = vieja.join("vistas.json");
        let marca = vieja.join(YA_PASADO);
        if self == &SitioDeMarcadores::Local(vieja.to_path_buf())
            || marca.exists()
            || !(fichero.is_file() || vieja.join("marcos.json").is_file())
        {
            return false;
        }
        let antes = pixpin_cad::marcadores::leer(&fichero);
        let mut aqui = pixpin_cad::marcadores::Guardados::de(Some(&self.vistas()));
        let sumado = aqui.sumar(&antes);
        if sumado {
            aqui.guardar();
        }
        // Lo anotado sobre cada marcador del modelo, a su fichero del mensaje.
        for m in &antes.modelo {
            let viejas =
                anotar::rutas_de_marcador(&SitioDeMarcadores::Local(vieja.to_path_buf()), &m.id);
            let nuevas = anotar::rutas_de_marcador(self, &m.id);
            pixpin_proyecto::anotado::copiar_si_falta(&viejas.tinta, &nuevas.tinta);
            pixpin_proyecto::anotado::copiar_si_falta(&viejas.hoja, &nuevas.hoja);
        }
        let destino = self.vistas();
        if let Err(e) = std::fs::write(&marca, destino.to_string_lossy().as_bytes()) {
            tracing::warn!(?e, carpeta = %vieja.display(), "marcadores: no se pudo marcar lo pasado");
        }
        tracing::info!(sumado, a = %destino.display(), "marcadores de antes pasados al mensaje");
        true
    }

    /// El fichero de los marcadores (el de los marcos va a su lado:
    /// `pixpin_cad::marcadores::marcos_junto_a`).
    pub fn vistas(&self) -> PathBuf {
        match self {
            SitioDeMarcadores::Adjunto(b) => b.fichero(pixpin_sincro::anotado::VISTAS),
            SitioDeMarcadores::Local(c) => c.join("vistas.json"),
        }
    }

    /// El fichero de los marcos para imprimir del modelo (la ventana lo saca
    /// sola del de los marcadores; aqui, para las pruebas).
    #[cfg(test)]
    pub fn marcos(&self) -> PathBuf {
        pixpin_cad::marcadores::marcos_junto_a(&self.vistas())
    }
}

/// La base de los ficheros de lo anotado sobre el marcador `id` del
/// adjunto de base `b` (`anot-<uid>`): `anot-<uid>-m<id>`.
fn base_de_marcador(
    b: &pixpin_proyecto::anotado::Base,
    id: &str,
) -> pixpin_proyecto::anotado::Base {
    pixpin_proyecto::anotado::Base {
        base: format!("{}-m{}", b.base, id_limpio(id)),
        ..b.clone()
    }
}

/// Cambia cuando cambia lo que se guarda del plano leido: la `l1` trae las
/// letras que faltan (lo leido antes no las tenia y se vuelve a leer).
const REVISION_DE_LETRAS: &str = "l1";

/// El nombre del plano leido en la cache: su clave, la revision y, si el
/// usuario puso letras, su huella (`huellaDeLetras` de Android): con una
/// letra nueva el plano se vuelve a leer con ella.
fn nombre_en_cache(clave: &str, huella: &str) -> String {
    if huella.is_empty() {
        format!("{clave}-{REVISION_DE_LETRAS}.pxcad")
    } else {
        format!("{clave}-{REVISION_DE_LETRAS}-{huella}.pxcad")
    }
}

/// Deja solo los `EN_CACHE` planos mas recientes.
fn podar(carpeta: &Path) {
    let Ok(d) = std::fs::read_dir(carpeta) else {
        return;
    };
    let mut v: Vec<(std::time::SystemTime, PathBuf)> = d
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.0));
    for (_, p) in v.into_iter().skip(EN_CACHE * 2) {
        let _ = std::fs::remove_file(p);
    }
}

/// Lee el plano: de la cache, o en el proceso aparte.
fn cargar(raiz: &Path, ruta: &Path) -> Result<pixpin_cad::modelo::Modelo, String> {
    let carpeta = raiz.join("cache").join("cad");
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let clave = clave(ruta).ok_or("no se puede leer el fichero")?;
    let letras = pixpin_cad::letras::carpeta_en(raiz);
    let destino = carpeta.join(nombre_en_cache(
        &clave,
        &pixpin_cad::letras::huella(&letras),
    ));
    if let Ok(b) = std::fs::read(&destino)
        && let Some(m) = pixpin_cad::modelo::Modelo::de_bytes(&b)
    {
        // Tocarlo: es de los recientes.
        let _ = std::fs::File::options().append(true).open(&destino);
        return Ok(m);
    }
    // En las pruebas el ejecutable es el de las pruebas, que no sabe la
    // orden: se lee aqui mismo.
    if cfg!(test) {
        return pixpin_cad::letras::convertir_con_letras(ruta, Some(letras));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut orden = std::process::Command::new(exe);
    orden.arg(ORDEN).arg(ruta).arg(&destino).arg(&letras);
    {
        use std::os::windows::process::CommandExt;
        // Sin ventana de consola.
        orden.creation_flags(0x0800_0000);
    }
    let t = std::time::Instant::now();
    let estado = orden.status().map_err(|e| e.to_string())?;
    tracing::info!(ms = t.elapsed().as_millis() as u64, codigo = ?estado.code(), ruta = %ruta.display(), "plano leido aparte");
    if !estado.success() {
        let motivo = std::fs::read_to_string(destino.with_extension("error")).unwrap_or_default();
        let _ = std::fs::remove_file(destino.with_extension("error"));
        return Err(if motivo.is_empty() {
            "el plano no se pudo leer (puede estar dañado)".into()
        } else {
            motivo
        });
    }
    podar(&carpeta);
    let b = std::fs::read(&destino).map_err(|e| e.to_string())?;
    pixpin_cad::modelo::Modelo::de_bytes(&b).ok_or_else(|| "lo leido no se entiende".to_string())
}

fn textos(idioma: Idioma) -> pixpin_cad::ventana::TextosUi {
    match idioma {
        Idioma::Ingles => pixpin_cad::ventana::TextosUi {
            abriendo: "Opening the drawing…".into(),
            vacio: "The drawing has nothing to show".into(),
            error: "Could not open the drawing".into(),
            pista_acotar: "Click points to measure · Double-click or Enter: finish · Esc: exit".into(),
            todo: "Show whole drawing	F".into(),
            tema: "Light or dark background	B".into(),
            acotar: "Measure	D".into(),
            borrar_cotas: "Clear measurements	Del".into(),
            encima: "Always on top	T".into(),
            cerrar: "Close	Esc".into(),
            tres: "View in 3D	3".into(),
            marcos: "Frames to print	I".into(),
            imprimir: "Print…	Ctrl+P".into(),
            exportar: "Export (HTML, PDF, PNG, DXF…)…	Ctrl+E".into(),
            borrar_marcos: "Clear frames".into(),
            pista_marcos: "Drag to frame what is printed · Ctrl+P: print · Del: remove the last one · Esc: exit".into(),
            marcador: "Bookmark here	Ctrl+M".into(),
            botones: pixpin_cad::ventana::NombresDeBotones::ingles(),
            anotar: "Annotate	A".into(),
            sin_dibujo_civil: "{n} Civil 3D objects were saved without their graphics: save with PROXYGRAPHICS = 1 or export to LandXML".into(),
            letras: "Drawing fonts…	L".into(),
            letras_faltan: "This drawing asks for fonts that are not here; its texts use similar ones and may not fit their boxes:".into(),
            letras_no_falta: "No font that the drawing names is missing.".into(),
            letras_donde: concat!(
                r"SHX fonts are usually in C:\Program Files\Autodesk\AutoCAD 20xx\Fonts or next to the drawing you were sent;",
                "\n",
                r"TrueType ones, in C:\Windows\Fonts. The ones you add here win over AutoCAD's, Windows' and PixPin's."
            )
            .into(),
            letras_anadir: "Add fonts (SHX, TTF, OTF, TTC)…".into(),
            letras_quitar: "Remove mine ({n})".into(),
            faltan_letras: "{n} fonts of the drawing are missing: click Aa above to add them".into(),
            releyendo: "Reading the drawing with your fonts…".into(),
        },
        _ => pixpin_cad::ventana::TextosUi::default(),
    }
}

/// **El globo de los botones de la barra** de los planos y los modelos
/// (19-oct-2026): su barra es una ventana aparte, asi que el nombre y la
/// tecla salen debajo en la ventanita de `globo_pista`, la de las barras de
/// los demas pines. Texto vacio: esconderlo.
pub fn globo_de_la_barra() -> pixpin_cad::ventana::Globo {
    Box::new(|texto: &str, r: [i32; 4], e: f64| {
        if texto.is_empty() {
            crate::globo_pista::ocultar();
        } else {
            let boton = pixpin_geom::Rect {
                x: r[0],
                y: r[1],
                ancho: r[2].max(1) as u32,
                alto: r[3].max(1) as u32,
            };
            crate::globo_pista::mostrar(texto, boton, e as f32);
        }
    })
}

/// Lee el plano en otro hilo; llega por el canal.
fn leer_aparte(
    raiz: &Path,
    ruta: &Path,
) -> std::sync::mpsc::Receiver<Result<pixpin_cad::modelo::Modelo, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let (r2, raiz2) = (ruta.to_path_buf(), raiz.to_path_buf());
    let _ = std::thread::Builder::new()
        .name("plano-cad-leer".into())
        .spawn(move || {
            let _ = tx.send(cargar(&raiz2, &r2));
        });
    rx
}

/// **El recuadro del tipo** para la barra de los visores de planos y
/// modelos: el mismo rotulo y color que en los demas pines (la tabla del
/// chat: amarillo los DWG, los IFC, los RVT...).
pub fn tipo_de_archivo(ruta: &Path) -> Option<pixpin_cad::ventana::TipoDeArchivo> {
    let ext = ruta.extension()?.to_string_lossy().into_owned();
    let nombre = format!("x.{ext}");
    let rotulo = pixpin_ui::color_de_extension::rotulo(&nombre);
    if rotulo.is_empty() {
        return None;
    }
    let (r, g, b) = pixpin_ui::chat::color_de_extension(&ext);
    let a_byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(pixpin_cad::ventana::TipoDeArchivo {
        rotulo,
        fondo: [a_byte(r), a_byte(g), a_byte(b)],
        tinta_oscura: pixpin_ui::color_de_extension::de(&nombre).texto_oscuro,
    })
}

/// El nombre para la barra y el titulo: sin la extension ni el numero con
/// que Android guarda los adjuntos, como en los demas pines.
pub fn nombre_para_la_barra(ruta: &Path) -> String {
    crate::lector_pin::nombre_limpio(
        &ruta
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
}

/// Abre el plano en su ventana, en su propio hilo.
pub fn lanzar(idioma: Idioma, ubicacion: Ubicacion, ruta: &Path) {
    let ruta = ruta.to_path_buf();
    let raiz = ubicacion.raiz().to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("plano-cad".into())
        .spawn(move || {
            // El dialogo «Abrir» del boton «Aa» es COM.
            let _com = pixpin_shell::ComDelHilo::iniciar();
            // Las letras: fuera lo que llego y no lo es, y que viajen las que el
            // usuario ya tenia. Antes de leer, para leer con lo que queda.
            crate::letras_que_viajan::al_abrir(&raiz);
            let rx = leer_aparte(&raiz, &ruta);
            let titulo = nombre_para_la_barra(&ruta);
            // El boton «3D»: el mismo plano en el visor de modelos.
            let (u3, r3) = (ubicacion.clone(), ruta.clone());
            let abrir_3d: Box<dyn Fn()> =
                Box::new(move || crate::modelo_bim::lanzar(idioma, u3.clone(), &r3));
            // Imprimir: el dialogo de Windows con la vista previa, cada marco una hoja.
            let (r4, raiz4) = (ruta.clone(), raiz.clone());
            let imprimir: Box<
                dyn Fn(isize, &pixpin_cad::modelo::Modelo, &[[f64; 4]], &[Vec<[f64; 2]>]),
            > = Box::new(move |v, m, marcos, cotas| {
                let anotado = anotar::ParaImprimir::leer(&anotar::rutas_de_la_capa(&raiz4, &r4), m);
                if let Err(e) = imprimir::imprimir(v, m, marcos, cotas, anotado, &r4) {
                    tracing::warn!(?e, "no se pudo imprimir el plano");
                }
            });
            // Anotar: el motor del lienzo sobre la capa del plano (anot-<uid>).
            let capa = anotar::rutas_de_la_capa(&raiz, &ruta);
            let anotador: Box<
                dyn FnOnce(&pixpin_cad::modelo::Modelo) -> Box<dyn pixpin_cad::anotado::Anotador>,
            > = Box::new(move |m| {
                Box::new(anotar::MotorDelPlano::nuevo(capa, m))
                    as Box<dyn pixpin_cad::anotado::Anotador>
            });
            // El boton «Aa»: las letras del usuario, en `<raiz>/fuentes-shx`, que
            // viajan con sus otros aparatos (`letras_que_viajan`).
            let carpeta = pixpin_cad::letras::carpeta_en(&raiz);
            let (c1, c4, r5, raiz5) = (carpeta.clone(), carpeta, ruta.clone(), raiz.clone());
            let (raiz_a, raiz_q) = (raiz.clone(), raiz.clone());
            let letras = pixpin_cad::ventana::Letras {
                mias: Box::new(move || pixpin_cad::letras::letras_en(&c1).len()),
                anadir: Box::new(move |v| {
                    let elegidas = pixpin_shell::elegir::pedir_ficheros(
                        windows::Win32::Foundation::HWND(v as *mut _),
                    );
                    let ahora = pixpin_shell::entorno::ahora_utc_ms();
                    crate::letras_que_viajan::anadir(
                        &raiz_a,
                        &crate::letras_que_viajan::aparato(&raiz_a),
                        ahora,
                        &elegidas,
                    )
                }),
                quitar: Box::new(move || {
                    crate::letras_que_viajan::quitar_todas(
                        &raiz_q,
                        pixpin_shell::entorno::ahora_utc_ms(),
                    )
                }),
                releer: Box::new(move || leer_aparte(&raiz5, &r5)),
                huella: Box::new(move || pixpin_cad::letras::huella(&c4)),
            };
            // Exportar: el componente comun con los formatos del plano.
            let (r6, raiz6, u6) = (ruta.clone(), raiz.clone(), ubicacion.clone());
            let exportar: Box<
                dyn Fn(isize, &pixpin_cad::modelo::Modelo, &[[f64; 4]], &[Vec<[f64; 2]>]),
            > = Box::new(move |_, m, marcos, cotas| {
                let p = exportar::Plano::nuevo(&raiz6, &r6, m, cotas, marcos);
                crate::compartir::ventana::abrir(
                    idioma,
                    u6.clone(),
                    crate::compartir::Cosa::Visor(exportar::exportable(p, idioma)),
                );
            });
            let acciones = pixpin_cad::ventana::Acciones {
                abrir_3d: Some(abrir_3d),
                imprimir: Some(imprimir),
                anotador: Some(anotador),
                letras: Some(letras),
                exportar: Some(exportar),
                vista_inicial: tomar_vista_al_abrir(&ruta),
                tipo: tipo_de_archivo(&ruta),
                marcadores: Some(SitioDeMarcadores::abrir(&raiz, &ruta).vistas()),
                globo: Some(globo_de_la_barra()),
            };
            if let Err(e) = pixpin_cad::ventana::ver(&titulo, rx, textos(idioma), acciones) {
                tracing::warn!(%e, "no se pudo abrir el visor de planos");
                let _ = pixpin_shell::abrir(&ruta);
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el visor de planos");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_abren_los_dwg_y_los_dxf() {
        assert!(se_abre("Planta.DWG") && se_abre("corte.dxf"));
        // Caso negativo: lo demas no.
        assert!(!se_abre("plano.pdf") && !se_abre("dwg"));
    }

    /// **Los iconos de la barra y el riel de los planos y los modelos son
    /// los mismos que los de los demas visores** (el usuario, 19-oct-2026:
    /// «no variar, para evitar confusiones»): `pixpin-cad` los lleva
    /// copiados y aqui se comprueba que siguen siendo identicos.
    #[test]
    fn los_iconos_de_los_planos_son_los_de_los_demas_visores() {
        use pixpin_render::icono::material as m;
        let de_material = |nombre: &str| -> &'static pixpin_render::icono::Icono {
            match nombre {
                "DRAG_INDICATOR" => &m::DRAG_INDICATOR,
                "STRAIGHTEN" => &m::STRAIGHTEN,
                "CROP_FREE" => &m::CROP_FREE,
                "EDIT" => &m::EDIT,
                "VIEW_IN_AR" => &m::VIEW_IN_AR,
                "FONT_DOWNLOAD" => &m::FONT_DOWNLOAD,
                "CONTENT_CUT" => &m::CONTENT_CUT,
                "LAYERS" => &m::LAYERS,
                "DARK_MODE" => &m::DARK_MODE,
                "LIGHT_MODE" => &m::LIGHT_MODE,
                "FULLSCREEN" => &m::FULLSCREEN,
                "FULLSCREEN_EXIT" => &m::FULLSCREEN_EXIT,
                "PUSH_PIN" => &m::PUSH_PIN,
                "CLOSE" => &m::CLOSE,
                "BOOKMARK_ADD" => &m::BOOKMARK_ADD,
                otro => panic!("{otro} no esta en la lista: anadelo aqui"),
            }
        };
        for (nombre, i) in pixpin_cad::iconos::TODOS {
            let t = de_material(nombre).trazos;
            assert_eq!(t.len(), 1, "{nombre}");
            assert_eq!(
                (i.d, i.par_impar),
                (t[0].d, t[0].par_impar),
                "{nombre} no es el de material"
            );
        }
        // Los de la barra comun del lector, con su mismo boton.
        let mismo = |b: &str, claro: bool, icono: &pixpin_render::icono::Icono| {
            assert_eq!(
                pixpin_cad::iconos::de_boton(b, claro, false).map(|i| i.d),
                Some(icono.trazos[0].d),
                "{b}"
            );
        };
        mismo("fijar", false, &m::PUSH_PIN);
        mismo("cerrar", false, &m::CLOSE);
        mismo("pantalla", false, &m::FULLSCREEN);
        mismo("asa", false, &m::DRAG_INDICATOR);
        mismo("anotar", false, &m::EDIT);
        mismo("marcos", false, &m::CROP_FREE);
        mismo("marcador", false, &m::BOOKMARK_ADD);
        // En oscuro, el sol para volver a claro (como el lector); en claro, la luna.
        mismo("tema", false, &m::LIGHT_MODE);
        mismo("tema", true, &m::DARK_MODE);
    }

    #[test]
    fn la_barra_lleva_el_tipo_y_el_nombre_como_los_demas_pines() {
        let t =
            tipo_de_archivo(Path::new(r"C:\planos\1791239986208_Planta baja.DWG")).expect("tipo");
        assert_eq!(t.rotulo, pixpin_ui::color_de_extension::rotulo("x.dwg"));
        let c = crate::lector_pin::color_de_tipo("dwg");
        assert_eq!(t.fondo, [c.r, c.g, c.b].map(|v| (v * 255.0).round() as u8));
        assert_eq!(
            nombre_para_la_barra(Path::new(r"C:\planos\1791239986208_Planta baja.DWG")),
            "Planta baja"
        );
        // Caso negativo: sin extension no hay recuadro.
        assert!(tipo_de_archivo(Path::new(r"C:\planos\plano")).is_none());
    }

    #[test]
    fn con_letras_nuevas_el_plano_se_vuelve_a_leer() {
        // Sin letras del usuario, el nombre no lleva huella.
        assert_eq!(nombre_en_cache("abc", ""), "abc-l1.pxcad");
        let d = std::env::temp_dir().join(format!("pixpin-cad-huella-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let vacia = pixpin_cad::letras::huella(&d);
        assert_eq!(nombre_en_cache("abc", &vacia), "abc-l1.pxcad");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("romans.shx"), b"x").unwrap();
        let con = nombre_en_cache("abc", &pixpin_cad::letras::huella(&d));
        assert_ne!(con, "abc-l1.pxcad");
        assert!(con.starts_with("abc-l1-") && con.ends_with(".pxcad"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Los marcadores viven por la ruta del plano** (no por su fecha):
    /// guardar el fichero otra vez no los pierde; otro fichero, otra carpeta.
    #[test]
    fn los_marcadores_van_por_la_ruta_y_no_por_la_fecha() {
        let raiz = Path::new(r"C:\raiz");
        let a = carpeta_de_marcadores(raiz, Path::new(r"D:\obra\Planta.DWG"));
        assert_eq!(
            a,
            carpeta_de_marcadores(raiz, Path::new(r"d:/obra/planta.dwg")),
            "la misma ruta escrita de otra manera"
        );
        assert!(
            a.starts_with(raiz.join("marcadores")) && !a.starts_with(raiz.join("cache")),
            "fuera de la cache, que se poda"
        );
        // Un plano que no es de ningun chat: en la carpeta local.
        let s = SitioDeMarcadores::de(raiz, Path::new(r"D:\obra\Planta.DWG"));
        assert_eq!(s, SitioDeMarcadores::Local(a.clone()));
        assert_eq!(s.vistas(), a.join("vistas.json"));
        assert_eq!(s.marcos(), a.join("marcos.json"));
        // Caso negativo: otro plano, otra carpeta.
        assert_ne!(
            a,
            carpeta_de_marcadores(raiz, Path::new(r"D:\obra\Alzado.dwg"))
        );
    }

    /// Lo anotado sobre un marcador 3D: una capa por marcador, con la unidad
    /// de la pantalla (1) y sin salir de su carpeta.
    #[test]
    fn la_tinta_de_un_marcador_3d_tiene_su_capa_y_su_unidad() {
        let c = Path::new(r"C:\raiz\marcadores\abc");
        let s = SitioDeMarcadores::Local(c.to_path_buf());
        let r = anotar::rutas_de_marcador(&s, "m18f2a");
        assert_eq!(r.tinta, c.join("m18f2a.excalidraw"));
        assert_eq!(r.hoja, c.join("m18f2a.hoja"));
        assert!(
            anotar::rutas_de_marcador(&s, r"..\..\x")
                .tinta
                .starts_with(c)
        );
        assert!(
            (pixpin_cad::anotado::unidad_de_la_capa(&anotar::modelo_de_pantalla()) - 1.0).abs()
                < 1e-12
        );
    }

    /// Una capa con un trazo, como la escribe el motor.
    const CAPA_CON_UN_TRAZO: &str = r##"{"type":"excalidraw","version":2,"source":"pixpin","elements":[{"id":"t1","type":"freedraw","x":0,"y":0,"width":10,"height":10,"angle":0,"strokeColor":"#e03131","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,"strokeStyle":"solid","roughness":0,"opacity":100,"groupIds":[],"seed":1,"version":1,"versionNonce":1,"isDeleted":false,"boundElements":null,"updated":1,"link":null,"locked":false,"points":[[0,0],[10,10]],"pressures":[],"simulatePressure":true}],"appState":{},"files":{}}"##;

    /// **De un adjunto del chat, todo en los ficheros de su mensaje**: los
    /// nombres planos `anot-<uid>…` que la sincronizacion reconoce y manda
    /// con el mensaje; quitar no borra ningun fichero.
    #[test]
    fn de_un_adjunto_los_marcadores_van_en_los_ficheros_de_su_mensaje_y_viajan() {
        use crate::anotado_del_adjunto::pruebas::{con_adjunto, raiz};
        use pixpin_sincro::anotado as an;
        let r = raiz("cad-marcadores");
        let doc = con_adjunto(&r, "obra.dwg", b"AC1032");
        let s = SitioDeMarcadores::abrir(&r, &doc);
        let SitioDeMarcadores::Adjunto(b) = &s else {
            panic!("es un adjunto: {s:?}")
        };
        let uid = b.base.strip_prefix("anot-").unwrap().to_string();
        let nombre = |p: &Path| p.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(nombre(&s.vistas()), format!("anot-{uid}.vistas.json"));
        assert_eq!(nombre(&s.marcos()), format!("anot-{uid}.marcos.json"));
        assert_eq!(
            s.marcos().parent(),
            s.vistas().parent(),
            "en la carpeta de lo anotado del mensaje"
        );
        let t = anotar::rutas_de_marcador(&s, "m18f2a");
        assert_eq!(
            t.tinta,
            b.fichero(".excalidraw.gz")
                .with_file_name(format!("anot-{uid}-mm18f2a.excalidraw"))
        );
        assert_eq!(nombre(&t.hoja), format!("anot-{uid}-mm18f2a.hoja"));
        // Los nombres portatiles son de su mensaje para la sincronizacion.
        let portatiles = [
            an::rel(&b.base, an::VISTAS),
            an::rel(&b.base, an::MARCOS),
            an::rel(&format!("{}-mm18f2a", b.base), ".excalidraw.gz"),
            an::rel(&format!("{}-mm18f2a", b.base), an::HOJA),
        ];
        let por = an::por_uid(portatiles.iter().map(String::as_str));
        assert_eq!(por.get(&uid).map(Vec::len), Some(4), "{por:?}");
        // Un marcador del modelo, con su tinta, y un marco.
        let mut g = pixpin_cad::marcadores::Guardados::de(Some(&s.vistas()));
        let estado = pixpin_cad::marcadores::Estado3d {
            objetivo: [1.0, 2.0, 3.0],
            rumbo: 0.5,
            altura: 0.3,
            dist: 20.0,
            seccion: None,
            aislado: None,
            aislado_clave: None,
            ocultos: Vec::new(),
            claro: false,
            aristas: true,
        };
        g.c.modelo.push(pixpin_cad::marcadores::Marcador3d {
            id: "m18f2a".into(),
            nombre: String::new(),
            estado: estado.clone(),
        });
        g.c.marcos.push(pixpin_cad::marcadores::Marco3d {
            id: "m1".into(),
            caja: [-0.1, -0.1, 0.1, 0.1],
            estado,
        });
        g.guardar();
        pixpin_proyecto::anotado::escribir(&t.tinta, CAPA_CON_UN_TRAZO).unwrap();
        pixpin_proyecto::anotado::escribir(&t.hoja, "-2500,-2500,2500,2500\n1\n").unwrap();
        // Lo manda la sincronizacion con su mensaje.
        let chat = pixpin_proyecto::vista::chat_de_ficha(
            &r,
            &pixpin_proyecto::almacen::Indice::leer(&r).proyectos[0].id,
        )
        .unwrap();
        let alcance: Vec<String> = pixpin_sincro::disco::Disco::alcance(
            &pixpin_proyecto::vista::DiscoPc::nuevo(&r),
            &chat,
        )
        .unwrap()
        .into_iter()
        .map(|(x, _)| x)
        .collect();
        for p in &portatiles {
            assert!(alcance.contains(p), "{p} no viaja: {alcance:?}");
        }
        // **Quitar no borra**: el marcador, los marcos y su tinta quedan escritos vacios.
        anotar::vaciar_capa(&t).unwrap();
        g.c.modelo.clear();
        g.c.marcos.clear();
        g.guardar();
        let json = |p: &Path| {
            serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(p).unwrap()).unwrap()
        };
        assert_eq!(json(&s.vistas())["modelo"], serde_json::json!([]));
        assert_eq!(json(&s.marcos())["marcos"], serde_json::json!([]));
        let capa = json(&t.tinta);
        let elementos = capa["elements"].as_array().unwrap();
        assert!(
            !elementos.is_empty() && elementos.iter().all(|e| e["isDeleted"] == true),
            "{capa}"
        );
        assert!(t.hoja.is_file());
        // Caso negativo: vaciar lo que no existe no crea nada.
        let otra = anotar::rutas_de_marcador(&s, "m-no-esta");
        anotar::vaciar_capa(&otra).unwrap();
        assert!(!otra.tinta.exists());
        let _ = std::fs::remove_dir_all(&r);
    }

    /// **Lo de antes** (`<raiz>/marcadores/<huella>/`, el 19-oct, por la
    /// ruta) pasa a los ficheros del mensaje una vez, sumado a lo que ya
    /// trajera el movil.
    #[test]
    fn los_marcadores_de_antes_pasan_al_mensaje_una_vez() {
        use crate::anotado_del_adjunto::pruebas::{con_adjunto, raiz};
        let r = raiz("cad-marcadores-antes");
        let doc = con_adjunto(&r, "edificio.ifc", b"ISO-10303-21;");
        let vieja = carpeta_de_marcadores(&r, &doc);
        std::fs::create_dir_all(&vieja).unwrap();
        let estado = r#"{"objetivo":[0,0,0],"rumbo":0,"altura":0,"dist":1}"#;
        std::fs::write(
            vieja.join("vistas.json"),
            format!(r#"{{"plano":[{{"id":"p1","x":1,"y":2,"ancho":3,"alto":4}}],"modelo":[{{"id":"m1","estado":{estado}}}],"marcos":[{{"caja":[0,0,0.1,0.1],"estado":{estado}}}]}}"#),
        )
        .unwrap();
        std::fs::write(vieja.join("m1.excalidraw"), CAPA_CON_UN_TRAZO).unwrap();
        std::fs::write(vieja.join("m1.hoja"), "-2500,-2500,2500,2500\n1\n").unwrap();
        // Lo que ya trajo el movil se queda.
        let s = SitioDeMarcadores::de(&r, &doc);
        std::fs::create_dir_all(s.vistas().parent().unwrap()).unwrap();
        std::fs::write(s.vistas(), r#"{"version":1,"plano":[],"modelo":[{"id":"movil","estado":{"objetivo":[0,0,0],"rumbo":1,"altura":0,"dist":1}}]}"#).unwrap();
        assert_eq!(SitioDeMarcadores::abrir(&r, &doc), s);
        let c = pixpin_cad::marcadores::leer(&s.vistas());
        let ids: Vec<&str> = c.modelo.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["movil", "m1"]);
        assert_eq!(c.plano.len(), 1);
        assert_eq!(c.marcos.len(), 1);
        assert!(!c.marcos[0].id.is_empty(), "los marcos de antes, con su id");
        let t = anotar::rutas_de_marcador(&s, "m1");
        assert_eq!(
            std::fs::read_to_string(&t.tinta).unwrap(),
            CAPA_CON_UN_TRAZO
        );
        assert!(t.hoja.is_file());
        // Lo de antes se queda donde estaba.
        assert!(vieja.join("vistas.json").is_file() && vieja.join("m1.excalidraw").is_file());
        // Una vez: quitado aqui, no vuelve al abrir otra vez.
        let mut g = pixpin_cad::marcadores::Guardados::de(Some(&s.vistas()));
        g.c.modelo.retain(|m| m.id != "m1");
        g.guardar();
        SitioDeMarcadores::abrir(&r, &doc);
        assert_eq!(pixpin_cad::marcadores::leer(&s.vistas()).modelo.len(), 1);
        let _ = std::fs::remove_dir_all(&r);
    }
    #[test]
    fn la_clave_cambia_si_cambia_el_fichero() {
        let d = std::env::temp_dir().join(format!("pixpin-cad-clave-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("a.dxf");
        std::fs::write(&f, "0\nEOF\n").unwrap();
        let k1 = clave(&f).unwrap();
        std::fs::write(&f, "0\nSECTION\n0\nEOF\n").unwrap();
        assert_ne!(clave(&f).unwrap(), k1);
        assert_eq!(clave(&d.join("no-esta.dwg")), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
