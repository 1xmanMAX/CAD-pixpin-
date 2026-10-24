//! **La Jump List del icono** (13-oct-2026): lo que sale al pulsar con el
//! boton derecho el icono de PixPin en la barra de tareas. Es lo que en
//! Android son los atajos del icono (`atajos/Atajos.kt` y
//! `xml/shortcuts.xml`, v0.99): Capturar, Mensajes guardados, Proyectos,
//! Sincronizar y **los tres ultimos proyectos tocados**.
//!
//! Cada entrada es un acceso directo al propio ejecutable con
//! `--atajo <que> [<id>]` (la seña `pixpin://atajo/...` de Android). Como
//! PixPin es de instancia unica, esa segunda copia no hace nada mas que
//! convertir el atajo en un **pedido** ([`pedido_de_atajo`]) y mandarlo a la
//! que ya corre por el mismo camino que los pedidos de fuera
//! (`mensajero::enviar_pedido`, `docs/protocolo-pedidos.md`). Asi el atajo
//! guarda solo la seña y lo que abre se decide al tocarlo, con la
//! aplicacion ya al dia, como en el movil.
//!
//! Lo puro (que entradas van y que pedido sale de cada una) se prueba sin
//! Windows; [`poner`] habla con `ICustomDestinationList`.

use std::path::Path;

/// La orden de la linea de mandatos de un atajo.
pub const ORDEN: &str = "--atajo";

/// Los atajos, por su nombre en la seña (los mismos de `Atajos.kt`).
pub const CAPTURAR: &str = "capturar";
pub const MENSAJES: &str = "mensajes";
pub const PROYECTOS: &str = "proyectos";
pub const SINCRONIZAR: &str = "sincronizar";
pub const PROYECTO: &str = "proyecto";
/// La seña de un protocolo `pixpinmax:` (el «Abrir» de una notificacion).
pub const URL: &str = "url";

/// Cuantos proyectos recientes (los de Android: «con tres, asoman sin echar
/// a nadie»).
pub const CUANTOS_PROYECTOS: usize = 3;

/// Un proyecto que puede ir en la lista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reciente {
    pub id: String,
    pub nombre: String,
    /// Cuando se toco por ultima vez (ms).
    pub tocado: i64,
}

/// Una entrada de la lista: lo que se lee y los argumentos del acceso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    pub titulo: String,
    pub descripcion: String,
    pub argumentos: String,
}

/// Los textos de la lista, ya traducidos.
#[derive(Debug, Clone)]
pub struct Textos {
    pub capturar: String,
    pub capturar_largo: String,
    pub mensajes: String,
    pub mensajes_largo: String,
    pub proyectos: String,
    pub proyectos_largo: String,
    pub sincronizar: String,
    pub sincronizar_largo: String,
    /// El titulo del grupo de los recientes.
    pub recientes: String,
    /// Con `{}`: «Abrir el proyecto {}».
    pub proyecto_largo: String,
}

impl Default for Textos {
    fn default() -> Self {
        Self {
            capturar: "Capturar".into(),
            capturar_largo: "Capturar una zona de la pantalla".into(),
            mensajes: "Mensajes guardados".into(),
            mensajes_largo: "Abrir Mensajes guardados".into(),
            proyectos: "Proyectos".into(),
            proyectos_largo: "Ver tus proyectos".into(),
            sincronizar: "Sincronizar".into(),
            sincronizar_largo: "Sincronizar con tus aparatos".into(),
            recientes: "Proyectos recientes".into(),
            proyecto_largo: "Abrir el proyecto {}".into(),
        }
    }
}

/// Lo que va en la lista.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lista {
    /// Las fijas («Tareas» en Windows).
    pub tareas: Vec<Entrada>,
    /// Los ultimos proyectos, en su grupo.
    pub recientes: Vec<Entrada>,
}

impl Lista {
    /// Una firma de lo que va: si no cambia, no se toca el sistema
    /// (`ultimaFirma` de Android).
    pub fn firma(&self) -> String {
        self.tareas
            .iter()
            .chain(&self.recientes)
            .map(|e| format!("{}\u{1}{}", e.titulo, e.argumentos))
            .collect::<Vec<_>>()
            .join("\u{0}")
    }
}

/// El argumento entre comillas, como lo lee `CommandLineToArgvW`: las
/// comillas de dentro con barra y las barras que las preceden dobladas.
fn entre_comillas(a: &str) -> String {
    let mut s = String::from("\"");
    let mut barras = 0;
    for c in a.chars() {
        match c {
            '\\' => barras += 1,
            '"' => {
                s.push_str(&"\\".repeat(barras * 2 + 1));
                s.push('"');
                barras = 0;
                continue;
            }
            _ => {
                s.push_str(&"\\".repeat(barras));
                barras = 0;
                s.push(c);
                continue;
            }
        }
    }
    s.push_str(&"\\".repeat(barras * 2));
    s.push('"');
    s
}

fn entrada(titulo: &str, descripcion: &str, que: &str, id: Option<&str>) -> Entrada {
    let mut argumentos = format!("{ORDEN} {que}");
    if let Some(id) = id {
        argumentos.push(' ');
        argumentos.push_str(&entre_comillas(id));
    }
    Entrada {
        titulo: titulo.to_string(),
        descripcion: descripcion.to_string(),
        argumentos,
    }
}

/// Corta a `n` letras (Android: 24 la corta, 48 la larga).
fn corto(t: &str, n: usize) -> String {
    t.trim().chars().take(n).collect()
}

/// **La lista que se arma**: las cuatro fijas y los [`CUANTOS_PROYECTOS`]
/// ultimos proyectos tocados (sin nombre no van; «Mensajes guardados» ya
/// esta arriba y no se repite).
pub fn armar(proyectos: &[Reciente], t: &Textos, guardados: &str) -> Lista {
    let tareas = vec![
        entrada(&t.capturar, &t.capturar_largo, CAPTURAR, None),
        entrada(&t.mensajes, &t.mensajes_largo, MENSAJES, None),
        entrada(&t.proyectos, &t.proyectos_largo, PROYECTOS, None),
        entrada(&t.sincronizar, &t.sincronizar_largo, SINCRONIZAR, None),
    ];
    let mut v: Vec<&Reciente> = proyectos
        .iter()
        .filter(|p| !p.nombre.trim().is_empty() && !p.id.trim().is_empty() && p.id != guardados)
        .collect();
    v.sort_by(|a, b| {
        b.tocado
            .cmp(&a.tocado)
            .then_with(|| a.nombre.cmp(&b.nombre))
    });
    let recientes = v
        .into_iter()
        .take(CUANTOS_PROYECTOS)
        .map(|p| {
            entrada(
                &corto(&p.nombre, 24),
                &corto(&t.proyecto_largo.replace("{}", &p.nombre), 48),
                PROYECTO,
                Some(&p.id),
            )
        })
        .collect();
    Lista { tareas, recientes }
}

/// **El pedido de un atajo** (`AtajoActivity` de Android): de los
/// argumentos de la linea de mandatos (sin el ejecutable) al JSON del
/// protocolo de pedidos. `None` si no es un atajo o no se entiende.
pub fn pedido_de_atajo(args: &[String]) -> Option<String> {
    let i = args.iter().position(|a| a == ORDEN)?;
    let que = args.get(i + 1)?.as_str();
    // `--atajo url "pixpinmax:…"`: el «Abrir» de una notificacion
    // (`notificacion`), que solo puede abrir cosas.
    if que == URL {
        return crate::notificacion::pedido_de(args.get(i + 2)?);
    }
    let json = match que {
        CAPTURAR => serde_json::json!({ "pixpin": 1, "accion": "capturar" }),
        MENSAJES => {
            serde_json::json!({ "pixpin": 1, "accion": "abrir", "que": { "tipo": "proyecto", "proyecto": null } })
        }
        PROYECTOS => serde_json::json!({ "pixpin": 1, "accion": "ventana_principal" }),
        SINCRONIZAR => {
            serde_json::json!({ "pixpin": 1, "accion": "ventana", "cual": "sincronizar" })
        }
        PROYECTO => {
            let id = args
                .get(i + 2)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())?;
            serde_json::json!({ "pixpin": 1, "accion": "abrir", "que": { "tipo": "proyecto", "proyecto": id } })
        }
        _ => return None,
    };
    Some(json.to_string())
}

/// **Pone la lista en el icono.** El hilo tiene que tener COM iniciado. Lo
/// que el usuario quito a mano de los recientes no se vuelve a poner (el
/// sistema lo prohibe y fallaria la lista entera).
pub fn poner(exe: &Path, lista: &Lista, titulo_recientes: &str) -> windows::core::Result<()> {
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
    use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
    use windows::Win32::UI::Shell::{
        DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW,
    };
    use windows::core::{HSTRING, Interface};

    // SAFETY: objetos COM locales que se sueltan al salir; las cadenas son
    // HSTRING vivas durante cada llamada.
    unsafe {
        let destinos: ICustomDestinationList =
            CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        let mut huecos = 0u32;
        let quitados: IObjectArray = destinos.BeginList(&mut huecos)?;
        let mut quitados_args: Vec<String> = Vec::new();
        for i in 0..quitados.GetCount().unwrap_or(0) {
            if let Ok(l) = quitados.GetAt::<IShellLinkW>(i) {
                let mut b = [0u16; 1024];
                if l.GetArguments(&mut b).is_ok() {
                    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
                    quitados_args.push(String::from_utf16_lossy(&b[..n]));
                }
            }
        }
        let recientes: Vec<&Entrada> = lista
            .recientes
            .iter()
            .filter(|e| !quitados_args.contains(&e.argumentos))
            .collect();
        if !recientes.is_empty() {
            let col: IObjectCollection =
                CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
            for e in &recientes {
                col.AddObject(&enlace(exe, e)?)?;
            }
            let arr: IObjectArray = col.cast()?;
            // Si falla el grupo (raro), que no se lleve las tareas.
            let _ = destinos.AppendCategory(&HSTRING::from(titulo_recientes), &arr);
        }
        let col: IObjectCollection =
            CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        for e in &lista.tareas {
            col.AddObject(&enlace(exe, e)?)?;
        }
        let arr: IObjectArray = col.cast()?;
        destinos.AddUserTasks(&arr)?;
        destinos.CommitList()
    }
}

/// Un acceso directo a `exe` con los argumentos de la entrada y su titulo
/// (la propiedad `PKEY_Title`, que es lo que lee la Jump List).
fn enlace(
    exe: &Path,
    e: &Entrada,
) -> windows::core::Result<windows::Win32::UI::Shell::IShellLinkW> {
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::StructuredStorage::{
        PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
    };
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{GUID, HSTRING, Interface, PWSTR};

    // PKEY_Title, sin traer todo `Storage_EnhancedStorage` por una constante.
    const PKEY_TITLE: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0xf29f85e0_4ff9_1068_ab91_08002b27b3d9),
        pid: 2,
    };

    // SAFETY: objetos COM locales; el texto del titulo vive en `titulo`
    // mientras dura SetValue (el almacen lo copia) y no se libera con
    // PropVariantClear porque es nuestro.
    unsafe {
        let l: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let ruta = HSTRING::from(exe.as_os_str());
        l.SetPath(&ruta)?;
        l.SetArguments(&HSTRING::from(e.argumentos.as_str()))?;
        l.SetDescription(&HSTRING::from(e.descripcion.as_str()))?;
        l.SetIconLocation(&ruta, 0)?;
        let mut titulo: Vec<u16> = e.titulo.encode_utf16().chain(std::iter::once(0)).collect();
        let valor = PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_LPWSTR,
                    Anonymous: PROPVARIANT_0_0_0 {
                        pwszVal: PWSTR(titulo.as_mut_ptr()),
                    },
                    ..Default::default()
                }),
            },
        };
        let almacen: IPropertyStore = l.cast()?;
        almacen.SetValue(&PKEY_TITLE, &valor)?;
        almacen.Commit()?;
        // El texto es nuestro: que el PROPVARIANT no intente soltarlo.
        std::mem::forget(valor);
        Ok(l)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn p(id: &str, nombre: &str, tocado: i64) -> Reciente {
        Reciente {
            id: id.into(),
            nombre: nombre.into(),
            tocado,
        }
    }

    fn args(linea: &str) -> Vec<String> {
        // Lo que haria Windows con la linea (para estas, sin comillas raras).
        let mut v = Vec::new();
        let mut actual = String::new();
        let mut dentro = false;
        let mut cs = linea.chars().peekable();
        while let Some(c) = cs.next() {
            match c {
                '\\' if cs.peek() == Some(&'"') => {
                    actual.push('"');
                    cs.next();
                }
                '"' => dentro = !dentro,
                ' ' if !dentro => {
                    if !actual.is_empty() {
                        v.push(std::mem::take(&mut actual));
                    }
                }
                _ => actual.push(c),
            }
        }
        if !actual.is_empty() {
            v.push(actual);
        }
        v
    }

    #[test]
    fn la_lista_lleva_las_cuatro_fijas_y_los_tres_ultimos_proyectos() {
        let proyectos = vec![
            p("a", "Tesis", 10),
            p("guardados", "Mensajes guardados", 99),
            p("b", "Obra Lima", 30),
            p("c", "  ", 50),
            p("d", "Charla", 20),
            p("e", "Viejo", 1),
        ];
        let l = armar(&proyectos, &Textos::default(), "guardados");
        let titulos: Vec<&str> = l.tareas.iter().map(|e| e.titulo.as_str()).collect();
        assert_eq!(
            titulos,
            ["Capturar", "Mensajes guardados", "Proyectos", "Sincronizar"]
        );
        let recientes: Vec<&str> = l.recientes.iter().map(|e| e.titulo.as_str()).collect();
        // Los mas tocados primero; sin nombre o «Mensajes guardados», no.
        assert_eq!(recientes, ["Obra Lima", "Charla", "Tesis"]);
        assert_eq!(l.recientes[0].argumentos, "--atajo proyecto \"b\"");
        assert_eq!(l.recientes[0].descripcion, "Abrir el proyecto Obra Lima");
    }

    #[test]
    fn sin_proyectos_solo_las_fijas_y_la_firma_cambia_con_ellos() {
        let vacia = armar(&[], &Textos::default(), "guardados");
        assert_eq!(vacia.tareas.len(), 4);
        assert!(vacia.recientes.is_empty());
        let una = armar(&[p("x", "Uno", 1)], &Textos::default(), "guardados");
        assert_ne!(vacia.firma(), una.firma());
        assert_eq!(
            una.firma(),
            armar(&[p("x", "Uno", 1)], &Textos::default(), "guardados").firma()
        );
    }

    #[test]
    fn cada_entrada_da_su_pedido_y_el_pedido_lo_entiende_el_protocolo() {
        let l = armar(
            &[p("tesis-2026", "Tesis \"final\"", 5)],
            &Textos::default(),
            "guardados",
        );
        let pedidos: Vec<serde_json::Value> = l
            .tareas
            .iter()
            .chain(&l.recientes)
            .map(|e| {
                serde_json::from_str(&pedido_de_atajo(&args(&e.argumentos)).expect(&e.argumentos))
                    .unwrap()
            })
            .collect();
        assert_eq!(pedidos[0]["accion"], "capturar");
        assert_eq!(pedidos[1]["accion"], "abrir");
        assert!(
            pedidos[1]["que"]["proyecto"].is_null(),
            "Mensajes guardados es el proyecto nulo"
        );
        assert_eq!(pedidos[2]["accion"], "ventana_principal");
        assert_eq!(pedidos[3]["cual"], "sincronizar");
        assert_eq!(pedidos[4]["que"]["proyecto"], "tesis-2026");
        for p in &pedidos {
            assert_eq!(p["pixpin"], 1);
            // La ventana de mensajes lo acepta (version y accion conocidas).
            assert_eq!(crate::mensajero::validar_pedido(&p.to_string()), 1, "{p}");
        }
    }

    #[test]
    fn lo_que_no_es_un_atajo_no_da_pedido() {
        assert_eq!(pedido_de_atajo(&[]), None);
        assert_eq!(pedido_de_atajo(&["C:\\foto.png".into()]), None);
        assert_eq!(pedido_de_atajo(&[ORDEN.into()]), None);
        assert_eq!(pedido_de_atajo(&[ORDEN.into(), "borrar_todo".into()]), None);
        // Un proyecto sin id no se inventa.
        assert_eq!(pedido_de_atajo(&[ORDEN.into(), PROYECTO.into()]), None);
        assert_eq!(
            pedido_de_atajo(&[ORDEN.into(), PROYECTO.into(), "  ".into()]),
            None
        );
    }

    #[test]
    fn las_comillas_y_las_barras_de_un_id_sobreviven() {
        assert_eq!(entre_comillas("abc"), "\"abc\"");
        assert_eq!(entre_comillas("a\"b"), "\"a\\\"b\"");
        assert_eq!(entre_comillas("c:\\x\\"), "\"c:\\x\\\\\"");
    }
}
