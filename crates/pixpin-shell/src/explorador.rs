//! Que ficheros hay seleccionados en el Explorador que esta delante (P1.6).
//!
//! Sirve para pinear lo que el usuario ya tenia marcado sin obligarle a
//! copiarlo antes. Y ese «sin copiarlo» es el motivo de todo lo que sigue:
//! la via facil seria mandar un Ctrl+C sintetico y leer el portapapeles,
//! pero eso pisa lo que el usuario tuviera guardado ahi. El portapapeles es
//! suyo, no nuestro, asi que se pregunta a la Shell por automatizacion.
//!
//! El camino es el documentado: `ShellWindows` enumera las ventanas de la
//! Shell, se busca la que coincide con la ventana en primer plano, y de
//! ella se baja por `IServiceProvider` -> `IShellBrowser` -> `IShellView`
//! -> `IFolderView` hasta los elementos con `SVGIO_SELECTION`.
//!
//! Nada de esto puede reventar ni esperar: lo llama el bucle de interfaz.
//! Cualquier eslabon que falle corta y devuelve la lista vacia.

use std::collections::HashSet;
use std::path::PathBuf;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize, IDispatch, IServiceProvider,
};
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::Shell::{
    IFolderView, IShellBrowser, IShellItemArray, IShellWindows, IWebBrowserApp,
    SID_STopLevelBrowser, SIGDN_FILESYSPATH, SVGIO_SELECTION, SWC_DESKTOP, SWFO_NEEDDISPATCH,
    ShellWindows,
};
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow};
use windows::core::Interface;

/// Las clases de ventana de nivel superior que son el escritorio.
///
/// «Progman» es el escritorio de toda la vida. «WorkerW» aparece cuando el
/// fondo de pantalla es una presentacion: Windows intercala otra capa y el
/// foco acaba en ella. Si solo se mirara «Progman», la funcion dejaria de
/// dar la seleccion del escritorio a quien tenga fondo rotatorio, y ese es
/// un fallo dificilisimo de atribuir.
const CLASES_DEL_ESCRITORIO: [&str; 2] = ["Progman", "WorkerW"];

/// Los ficheros seleccionados en el Explorador que esta delante.
///
/// Vacio si delante no hay un Explorador, si no hay nada seleccionado, o
/// si la automatizacion no responde. Nunca entra en panico ni bloquea.
pub fn seleccion_del_explorador() -> Vec<PathBuf> {
    // `_com` se declara PRIMERO para que se destruya el ULTIMO: en Rust los
    // locales mueren en orden inverso. Ademas, todas las interfaces COM
    // nacen y mueren dentro de `recolectar`, asi que cuando corre el
    // CoUninitialize de este guarda ya no queda ninguna viva. Soltar una
    // interfaz sobre un apartamento cerrado mata el proceso con
    // ACCESS_VIOLATION; en este repositorio ya paso una vez, en el hilo de
    // UIA, y por eso aqui el orden esta escrito y no confiado.
    let _com = ComDelHilo::iniciar();
    let nombres = recolectar();
    depurar_rutas(&nombres)
}

/// Deja solo rutas de verdad del sistema de ficheros, sin repetidos y en el
/// mismo orden en que las dio la Shell.
///
/// Esta aparte y es pura a proposito: es la unica parte que se puede probar
/// sin un Explorador abierto, y es donde estan los errores que de verdad
/// duelen. Una carpeta virtual («Este equipo», «Red», la papelera) no tiene
/// ruta; si se colara como `PathBuf`, quien la reciba intentaria abrir un
/// fichero que no existe y el fallo apareceria lejisimos de aqui.
pub fn depurar_rutas(nombres: &[String]) -> Vec<PathBuf> {
    let mut vistos: HashSet<String> = HashSet::new();
    let mut rutas = Vec::new();
    for nombre in nombres {
        let limpio = nombre.trim();
        if !es_ruta_del_sistema(limpio) {
            continue;
        }
        // Windows no distingue mayusculas en las rutas: el mismo fichero
        // escrito de dos formas es un solo fichero. Sin esta clave en
        // minusculas, una seleccion podria producir dos fichas identicas.
        if !vistos.insert(limpio.to_lowercase()) {
            continue;
        }
        rutas.push(PathBuf::from(limpio));
    }
    rutas
}

/// Si esa cadena es una ruta absoluta del sistema de ficheros.
///
/// Se acepta `X:\...`, `X:/...` y `\\servidor\recurso`. Todo lo demas se
/// descarta: las carpetas virtuales llegan como `::{20D04FE0-...}` o
/// directamente como su nombre para enseñar. Filtrar por ese nombre
/// («Este equipo», «This PC», «Dieser PC») dejaria de funcionar en cuanto
/// Windows estuviera en otro idioma, asi que se filtra por la FORMA de la
/// ruta, que no se traduce.
fn es_ruta_del_sistema(texto: &str) -> bool {
    // Recurso de red. Cubre tambien `\\?\C:\...`, que es una ruta valida.
    if texto.starts_with(r"\\") {
        return true;
    }
    let mut letras = texto.chars();
    matches!(
        (letras.next(), letras.next(), letras.next()),
        (Some(unidad), Some(':'), Some('\\' | '/')) if unidad.is_ascii_alphabetic()
    )
}

/// Si esa clase de ventana es la del escritorio.
///
/// Se compara sin distinguir mayusculas porque la clase la escribe quien
/// registro la ventana y no hay ninguna garantia de que respete el caso.
fn es_clase_del_escritorio(clase: &str) -> bool {
    let clase = clase.trim();
    !clase.is_empty()
        && CLASES_DEL_ESCRITORIO
            .iter()
            .any(|conocida| conocida.eq_ignore_ascii_case(clase))
}

/// Inicializa COM en este hilo y lo suelta al morir, pero solo si fue este
/// guarda quien lo inicializo.
///
/// Hace falta porque la funcion se llama tanto desde el hilo de interfaz,
/// que ya tiene COM en STA, como desde una prueba, que no tiene nada.
pub struct ComDelHilo {
    hay_que_soltar: bool,
}

impl ComDelHilo {
    pub fn iniciar() -> ComDelHilo {
        // SAFETY: CoInitializeEx no tiene precondiciones. Devuelve
        // RPC_E_CHANGED_MODE (un Err) si el hilo ya esta en otro
        // apartamento: en ese caso COM sirve igual y NO hay que emparejar
        // con CoUninitialize, porque cerrar el apartamento seria cerrarselo
        // a quien lo abrio. S_FALSE (ya inicializado en el mismo modo) si
        // cuenta como Ok y si pide su CoUninitialize.
        let resultado = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        ComDelHilo {
            hay_que_soltar: resultado.is_ok(),
        }
    }
}

impl Drop for ComDelHilo {
    fn drop(&mut self) {
        if self.hay_que_soltar {
            // SAFETY: empareja exactamente el CoInitializeEx de iniciar(),
            // y para cuando esto corre ya no vive ninguna interfaz COM de
            // este hilo (todas nacen y mueren dentro de `recolectar`).
            unsafe { CoUninitialize() };
        }
    }
}

/// Los nombres tal cual los devuelve la Shell, sin filtrar.
///
/// Devuelve cadenas y no `PathBuf` para dejar la frontera clara: aqui esta
/// todo lo que solo se puede comprobar a mano, y en `depurar_rutas` todo lo
/// que se prueba en frio.
fn recolectar() -> Vec<String> {
    // SAFETY: GetForegroundWindow no tiene precondiciones y puede devolver
    // una ventana nula (pasa un instante al cambiar de escritorio virtual),
    // que se descarta aqui mismo.
    let delante = unsafe { GetForegroundWindow() };
    if delante.0.is_null() {
        return Vec::new();
    }
    // SAFETY: CoCreateInstance del CLSID documentado de la Shell. Es un
    // servidor local (explorer.exe): si no esta o no responde devuelve Err
    // y aqui se acaba, sin lista y sin ruido.
    let ventanas: IShellWindows = match unsafe { CoCreateInstance(&ShellWindows, None, CLSCTX_ALL) }
    {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    match buscar_ventana_de_shell(&ventanas, delante) {
        Some(navegador) => rutas_seleccionadas(&navegador),
        None => Vec::new(),
    }
}

/// El objeto de automatizacion de la ventana de Shell que esta delante.
///
/// Se identifica por HWND y no por clase de ventana: con tres Explorador
/// abiertos la clase es la misma en los tres, y solo el handle dice cual
/// esta mirando el usuario.
fn buscar_ventana_de_shell(ventanas: &IShellWindows, delante: HWND) -> Option<IDispatch> {
    let objetivo = delante.0 as isize;
    // SAFETY: llamadas de solo lectura sobre la coleccion de la Shell; cada
    // elemento que no conteste se salta con `continue` en vez de cortar,
    // porque una ventana cerrandose no debe ocultar a las demas.
    unsafe {
        let total = ventanas.Count().unwrap_or(0);
        for indice in 0..total {
            let Ok(elemento) = ventanas.Item(&variante_entera(indice)) else {
                continue;
            };
            let Ok(aplicacion) = elemento.cast::<IWebBrowserApp>() else {
                continue;
            };
            let Ok(handle) = aplicacion.HWND() else {
                continue;
            };
            if handle.0 == objetivo {
                return Some(elemento);
            }
        }
    }
    // El escritorio va aparte. Sus iconos son una vista de la Shell como
    // cualquier carpeta, pero la ventana que tiene el foco es Progman (o
    // WorkerW) y su handle no siempre es el que publica la coleccion, asi
    // que el bucle de arriba puede no encontrarla. Se pregunta por el
    // escritorio SOLO si la clase confirma que eso es lo que hay delante:
    // sin esa comprobacion, tener Word en primer plano devolveria la
    // seleccion del escritorio, que es bastante peor que no devolver nada.
    if es_clase_del_escritorio(&clase_de_ventana(delante)?) {
        return ventana_del_escritorio(ventanas);
    }
    None
}

/// La ventana del escritorio dentro de la coleccion de la Shell.
fn ventana_del_escritorio(ventanas: &IShellWindows) -> Option<IDispatch> {
    // VT_EMPTY es lo que la API pide para «en cualquier sitio»; un VARIANT
    // a cero es exactamente eso y no lleva memoria detras que liberar.
    let sin_sitio = VARIANT::default();
    let mut handle = 0i32;
    // SAFETY: los dos VARIANT viven durante toda la llamada y `handle` es
    // local. SWFO_NEEDDISPATCH es lo que hace que devuelva el objeto de
    // automatizacion, y no solo el handle que ya tenemos.
    unsafe {
        ventanas
            .FindWindowSW(
                &sin_sitio,
                &sin_sitio,
                SWC_DESKTOP,
                &mut handle,
                SWFO_NEEDDISPATCH,
            )
            .ok()
    }
}

/// Las rutas de lo seleccionado en esa ventana de la Shell.
fn rutas_seleccionadas(navegador: &IDispatch) -> Vec<String> {
    // SAFETY: la cadena QueryInterface -> QueryService -> vista es la
    // documentada, y cada eslabon corta con `else { return }` si el objeto
    // no responde. Las interfaces las suelta el crate `windows` al
    // dropearlas; la unica memoria a mano es la de GetDisplayName, que se
    // libera mas abajo.
    unsafe {
        let Ok(proveedor) = navegador.cast::<IServiceProvider>() else {
            return Vec::new();
        };
        let Ok(explorador) = proveedor.QueryService::<IShellBrowser>(&SID_STopLevelBrowser) else {
            return Vec::new();
        };
        let Ok(vista) = explorador.QueryActiveShellView() else {
            return Vec::new();
        };
        let Ok(carpeta) = vista.cast::<IFolderView>() else {
            return Vec::new();
        };
        // Sin seleccion, Items puede devolver Err o una lista de cero. Las
        // dos cosas significan lo mismo y las dos valen.
        let Ok(elementos) = carpeta.Items::<IShellItemArray>(SVGIO_SELECTION) else {
            return Vec::new();
        };
        let total = elementos.GetCount().unwrap_or(0);
        let mut rutas = Vec::with_capacity(total as usize);
        for indice in 0..total {
            let Ok(elemento) = elementos.GetItemAt(indice) else {
                continue;
            };
            // SIGDN_FILESYSPATH falla justamente en lo que no tiene ruta
            // («Este equipo», «Red»): ese Err es la primera criba, y
            // `es_ruta_del_sistema` es la segunda, por si una extension de
            // shell devuelve un nombre cualquiera en vez de fallar.
            let Ok(nombre) = elemento.GetDisplayName(SIGDN_FILESYSPATH) else {
                continue;
            };
            let texto = nombre.to_string().ok();
            // La memoria de GetDisplayName es COM (CoTaskMemAlloc) y se
            // libera AQUI, despues de copiarla a String y ANTES de
            // cualquier `?` o `continue`: la fuga de appdata() en S1-A fue
            // exactamente un salto metido entre la reserva y la liberacion.
            CoTaskMemFree(Some(nombre.as_ptr() as *const _));
            if let Some(texto) = texto {
                rutas.push(texto);
            }
        }
        rutas
    }
}

/// Un VARIANT de tipo VT_I4, que es como `IShellWindows::Item` pide el
/// indice.
///
/// Se construye a mano porque un entero no lleva memoria detras: no hay
/// nada que liberar con VariantClear y el valor se copia sin mas. Para
/// cualquier otro tipo (una cadena, un objeto) esto seria una fuga.
fn variante_entera(valor: i32) -> VARIANT {
    let mut variante = VARIANT::default();
    // SAFETY: escribir en la union es correcto porque en la misma respiracion
    // se marca vt = VT_I4, que es precisamente el miembro (lVal) que se
    // acaba de rellenar; a partir de ahi el valor se describe a si mismo.
    // El `*` desenvuelve el ManuallyDrop del campo, y es inocuo aqui porque
    // lo que hay dentro es un entero: no hay destructor que se salte.
    unsafe {
        (*variante.Anonymous.Anonymous).vt = VT_I4;
        (*variante.Anonymous.Anonymous).Anonymous.lVal = valor;
    }
    variante
}

/// La clase de una ventana, o None si el sistema no la da.
fn clase_de_ventana(hwnd: HWND) -> Option<String> {
    let mut buffer = [0u16; 64];
    // SAFETY: consulta de solo lectura; el buffer es local y se pasa entero,
    // asi que GetClassNameW no puede escribir de mas. Devuelve 0 si falla.
    let escritos = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if escritos <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..escritos as usize]))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_rutas_absolutas_pasan_y_conservan_el_orden() {
        // El orden importa: es el que el usuario ve en la ventana, y
        // devolverlo cambiado haria que las fichas salieran barajadas.
        let crudas = vec![
            r"C:\Users\yo\Imagenes\b.png".to_string(),
            r"C:\Users\yo\Imagenes\a.png".to_string(),
            r"D:/otro/disco/c.txt".to_string(),
        ];
        assert_eq!(
            depurar_rutas(&crudas),
            vec![
                PathBuf::from(r"C:\Users\yo\Imagenes\b.png"),
                PathBuf::from(r"C:\Users\yo\Imagenes\a.png"),
                PathBuf::from(r"D:/otro/disco/c.txt"),
            ]
        );
    }

    #[test]
    fn un_recurso_de_red_tambien_es_una_ruta_valida() {
        // Media oficina trabaja sobre unidades de red; descartar UNC dejaria
        // la funcion inutil justo ahi.
        let crudas = vec![r"\\servidor\comun\informe.pdf".to_string()];
        assert_eq!(
            depurar_rutas(&crudas),
            vec![PathBuf::from(r"\\servidor\comun\informe.pdf")]
        );
    }

    #[test]
    fn una_carpeta_virtual_no_tiene_ruta_y_se_descarta() {
        // Caso negativo. «Este equipo» o «Red» son carpetas de la Shell sin
        // fichero detras: la Shell devuelve su nombre o su CLSID, y colarlo
        // como ruta haria fallar a quien intente abrirlo, lejos de aqui.
        let crudas = vec![
            "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}".to_string(),
            "Este equipo".to_string(),
            "Red".to_string(),
            "Papelera de reciclaje".to_string(),
            String::new(),
            "   ".to_string(),
        ];
        assert!(depurar_rutas(&crudas).is_empty());
    }

    #[test]
    fn una_ruta_relativa_no_se_cuela() {
        // Caso negativo. Una ruta relativa se resolveria contra el
        // directorio de trabajo de PixPin, que no tiene nada que ver con la
        // carpeta que el usuario esta mirando: abriria otro fichero.
        let crudas = vec![
            r"Imagenes\a.png".to_string(),
            "a.png".to_string(),
            r"\solo\una\barra.txt".to_string(),
            "C:".to_string(),
            r"1:\no\es\una\unidad.txt".to_string(),
        ];
        assert!(depurar_rutas(&crudas).is_empty());
    }

    #[test]
    fn el_mismo_fichero_no_sale_dos_veces() {
        // Windows no distingue mayusculas: las tres primeras entradas son el
        // mismo fichero, y devolverlas todas crearia fichas duplicadas.
        let crudas = vec![
            r"C:\tmp\Foto.png".to_string(),
            r"C:\TMP\FOTO.PNG".to_string(),
            r"  C:\tmp\Foto.png  ".to_string(),
            r"C:\tmp\otra.png".to_string(),
        ];
        assert_eq!(
            depurar_rutas(&crudas),
            vec![
                PathBuf::from(r"C:\tmp\Foto.png"),
                PathBuf::from(r"C:\tmp\otra.png"),
            ]
        );
    }

    #[test]
    fn sin_nada_seleccionado_la_lista_sale_vacia() {
        // El caso mas comun de todos: hay un Explorador delante pero el
        // usuario no ha marcado nada. Tiene que ser una lista vacia, no un
        // error ni una entrada fantasma.
        assert!(depurar_rutas(&[]).is_empty());
    }

    #[test]
    fn el_escritorio_se_reconoce_por_su_clase_de_ventana() {
        assert!(es_clase_del_escritorio("Progman"));
        // Con el fondo de pantalla en presentacion, el foco cae en WorkerW.
        assert!(es_clase_del_escritorio("WorkerW"));
        assert!(es_clase_del_escritorio("progman"));
    }

    #[test]
    fn una_ventana_cualquiera_no_es_el_escritorio() {
        // Caso negativo, y el que evita el fallo mas feo: si esto diera
        // cierto de mas, con cualquier programa delante se devolveria la
        // seleccion del escritorio como si fuera la suya.
        assert!(!es_clase_del_escritorio("CabinetWClass"));
        assert!(!es_clase_del_escritorio("ExploreWClass"));
        assert!(!es_clase_del_escritorio("Notepad"));
        assert!(!es_clase_del_escritorio(""));
        assert!(!es_clase_del_escritorio("   "));
        assert!(!es_clase_del_escritorio("Prog"));
    }

    #[test]
    fn preguntar_dos_veces_seguidas_no_revienta_ni_se_cuelga() {
        // El contrato que sostiene todo: esto lo llama el bucle de
        // interfaz. Sin Explorador delante, sin escritorio o con COM caido
        // tiene que devolver una lista, nunca entrar en panico.
        //
        // Y dos veces a proposito: si CoUninitialize no emparejara con su
        // CoInitializeEx, la segunda llamada se encontraria el apartamento
        // cerrado y esto se caeria. Es la unica forma de comprobar en frio
        // que el guarda de COM esta equilibrado.
        let _ = seleccion_del_explorador();
        let _ = seleccion_del_explorador();
    }

    #[test]
    #[ignore = "necesita un Explorador delante con ficheros seleccionados; ejecutar con --ignored"]
    fn con_ficheros_marcados_en_el_explorador_salen_sus_rutas() {
        // Manual: la consola tiene el foco cuando arranca la prueba, asi que
        // hay que darle tiempo a quien la ejecuta para pasar al Explorador.
        println!("Pon delante un Explorador con dos ficheros seleccionados. 6 segundos...");
        std::thread::sleep(std::time::Duration::from_secs(6));
        let rutas = seleccion_del_explorador();
        println!("seleccion: {rutas:#?}");
        assert!(
            !rutas.is_empty(),
            "no llego nada: habia un Explorador delante con algo seleccionado?"
        );
        assert!(
            rutas.iter().all(|r| r.exists()),
            "alguna ruta no existe de verdad: {rutas:?}"
        );
    }

    #[test]
    #[ignore = "necesita el escritorio delante con iconos seleccionados; ejecutar con --ignored"]
    fn con_iconos_marcados_en_el_escritorio_salen_sus_rutas() {
        // El escritorio es el caso que se olvida: sus iconos son una vista
        // de la Shell y el usuario espera que esto funcione ahi igual que en
        // una carpeta cualquiera.
        println!("Minimiza todo y selecciona dos iconos del escritorio. 6 segundos...");
        std::thread::sleep(std::time::Duration::from_secs(6));
        let rutas = seleccion_del_explorador();
        println!("seleccion del escritorio: {rutas:#?}");
        assert!(
            !rutas.is_empty(),
            "no llego nada: habia iconos seleccionados en el escritorio?"
        );
    }
}
