//! Que programa esta delante ahora mismo (P1.8).
//!
//! Sirve para la lista de programas a ignorar: mientras uno de ellos tenga
//! el foco, los atajos de PixPin no actuan. El caso de uso es un juego o
//! un programa que use `Ctrl+1` para lo suyo — desregistrar los atajos a
//! mano cada vez seria peor que no tenerlos.
//!
//! Se compara por NOMBRE de ejecutable y no por ruta completa. La ruta
//! cambia entre equipos, entre versiones y entre instalaciones portables,
//! asi que una lista escrita a mano con rutas no valdria para nadie mas
//! que para quien la escribio.

/// El nombre del ejecutable que tiene el foco, en minusculas y con su
/// extension: `"notepad.exe"`.
///
/// `None` si no hay ventana en primer plano (pasa un instante al cambiar
/// de escritorio) o si el sistema no deja mirar ese proceso. Ninguna de
/// las dos es un fallo: significan «no lo se», y quien pregunta debe
/// seguir como si no estuviera en la lista.
#[cfg(windows)]
pub fn programa_delante() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    // SAFETY: GetForegroundWindow no tiene precondiciones y puede devolver
    // una ventana nula, que `programa_de` descarta.
    programa_de(unsafe { GetForegroundWindow() })
}

/// El nombre del ejecutable de la ventana `hwnd`, como [`programa_delante`].
#[cfg(windows)]
pub fn programa_de(hwnd: windows::Win32::Foundation::HWND) -> Option<String> {
    use windows::Win32::Foundation::{CloseHandle, MAX_PATH};
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    use windows::core::PWSTR;

    if hwnd.0.is_null() {
        return None;
    }
    let mut pid = 0u32;
    // SAFETY: `hwnd` acaba de venir del sistema y `pid` es local.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }
    // LIMITED_INFORMATION y no QUERY_INFORMATION: con el derecho amplio,
    // preguntar por un proceso de mas privilegios falla, y entonces la
    // lista dejaria de funcionar justo con las ventanas de administrador,
    // que son de las que mas interesa apartarse.
    //
    // SAFETY: pid valido; el handle se cierra en todos los caminos de
    // abajo, incluido el de error de QueryFullProcessImageNameW.
    let proceso = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buffer = [0u16; MAX_PATH as usize];
    let mut largo = buffer.len() as u32;
    // SAFETY: el buffer y el largo son locales y concuerdan; el handle
    // sigue abierto.
    let obtenido = unsafe {
        QueryFullProcessImageNameW(
            proceso,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut largo,
        )
    };
    // SAFETY: el handle vino de OpenProcess y no se ha cerrado antes. Se
    // cierra ANTES de mirar si la consulta fue bien: un `?` entre la
    // apertura y el cierre seria una fuga en el camino de error.
    unsafe {
        let _ = CloseHandle(proceso);
    }
    obtenido.ok()?;
    let ruta = String::from_utf16_lossy(&buffer[..largo as usize]);
    nombre_de_ruta(&ruta)
}

#[cfg(not(windows))]
pub fn programa_delante() -> Option<String> {
    None
}

/// El nombre del ejecutable de una ruta completa, en minusculas.
///
/// Aparte y pura para poder probarla: en CI no hay escritorio, pero
/// equivocarse con la barra o con las mayusculas es un fallo de logica que
/// no necesita uno.
pub fn nombre_de_ruta(ruta: &str) -> Option<String> {
    let nombre = ruta
        .rsplit(['\\', '/'])
        .next()
        .filter(|n| !n.is_empty())?
        .to_lowercase();
    Some(nombre)
}

/// Si el programa esta en la lista de los que hay que ignorar.
///
/// Compara sin distinguir mayusculas y admite las entradas con o sin la
/// extension: quien escribe la lista a mano pone «notepad» tan a menudo
/// como «notepad.exe», y fallar por eso sin decir nada seria de las cosas
/// mas dificiles de averiguar.
pub fn esta_en_la_lista(programa: &str, lista: &[String]) -> bool {
    let programa = programa.to_lowercase();
    let sin_extension = programa.strip_suffix(".exe").unwrap_or(&programa);
    lista.iter().any(|entrada| {
        let entrada = entrada.trim().to_lowercase();
        if entrada.is_empty() {
            return false;
        }
        let entrada_sin = entrada.strip_suffix(".exe").unwrap_or(&entrada);
        entrada_sin == sin_extension
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_nombre_sale_de_cualquiera_de_las_dos_barras() {
        assert_eq!(
            nombre_de_ruta("C:\\Windows\\System32\\notepad.exe").as_deref(),
            Some("notepad.exe")
        );
        assert_eq!(
            nombre_de_ruta("C:/Juegos/Mi Juego/juego.EXE").as_deref(),
            Some("juego.exe")
        );
    }

    #[test]
    fn una_ruta_sin_nombre_no_da_nombre() {
        // Caso negativo: una ruta que acaba en barra, o vacia, no puede
        // producir una cadena vacia que luego cuadre con una entrada vacia
        // de la lista y lo ignore todo.
        assert_eq!(nombre_de_ruta(""), None);
        assert_eq!(nombre_de_ruta("C:\\Windows\\"), None);
    }

    #[test]
    fn la_lista_no_distingue_mayusculas_ni_la_extension() {
        let lista = vec!["Notepad.exe".to_string(), "juego".to_string()];
        assert!(esta_en_la_lista("notepad.exe", &lista));
        assert!(esta_en_la_lista("NOTEPAD.EXE", &lista));
        // La entrada sin extension caza el ejecutable con ella, que es como
        // la escribe cualquiera que no lo piense.
        assert!(esta_en_la_lista("juego.exe", &lista));
        assert!(esta_en_la_lista("juego", &lista));
    }

    #[test]
    fn una_lista_vacia_no_ignora_nada() {
        // Es el caso por defecto y el mas importante de todos: si esto
        // fallara, PixPin dejaria de responder a los atajos sin que nadie
        // hubiera pedido nada.
        assert!(!esta_en_la_lista("notepad.exe", &[]));
        assert!(!esta_en_la_lista("notepad.exe", &["".to_string()]));
        assert!(!esta_en_la_lista("notepad.exe", &["   ".to_string()]));
    }

    #[test]
    fn no_caza_por_parecido() {
        // Caso negativo: «note» no puede ignorar «notepad.exe». Una lista
        // que casara por prefijo apagaria los atajos en programas que el
        // usuario no nombro, y averiguar por que seria muy cuesta arriba.
        let lista = vec!["note".to_string(), "pad".to_string()];
        assert!(!esta_en_la_lista("notepad.exe", &lista));
    }
}
