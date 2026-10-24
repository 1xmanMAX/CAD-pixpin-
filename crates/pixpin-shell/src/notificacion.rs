//! **Las notificaciones de Windows con botones** (toast nativo, 15-oct-2026),
//! para toda la app: el asistente al acabar un encargo («Abrir»), los
//! recordatorios de mensajes y tareas («Hecha», «+10 min», «Abrir») y las
//! capturas que se van a ir («Conservar», «Abrir»).
//!
//! ## Por que ahora si (ver la cabecera de [`crate::aviso`])
//!
//! `ToastNotificationManager` exige un AppUserModelID que Windows conozca.
//! Antes se descarto porque PixPin no tenia instalador; hay dos formas de
//! darselo a un programa sin empaquetar:
//!
//! 1. el acceso directo del menu Inicio con la propiedad
//!    `System.AppUserModel.ID` y la misma id puesta en el proceso con
//!    `SetCurrentProcessExplicitAppUserModelID`;
//! 2. la clave `HKCU\Software\Classes\AppUserModelId\<id>` con
//!    `DisplayName` (e `IconUri`): `CreateToastNotifierWithId(<id>)` funciona
//!    desde cualquier proceso, sin acceso directo.
//!
//! Se usa **la 2**. La 1 cambia la identidad de la ventana en la barra de
//! tareas: un PixPin anclado a la barra (otro `.lnk`, sin la propiedad) y el
//! que corre dejarian de agruparse, y la Jump List (`jump_list`) cuelga de esa
//! identidad. La clave se escribe al arrancar (fuera del modo portable, como
//! `asociaciones`), asi que `instalar.ps1` no tiene que tocar el `.lnk`. La
//! id es [`AUMID`]; con el paquete MSIX (identidad `ForgePixPin.PixPinMax`)
//! no hace falta: [`mostrar`] usa la del paquete ([`tiene_identidad`]).
//!
//! ## Los botones: un pedido por argumento
//!
//! Un programa sin empaquetar no recibe la activacion normal de un toast
//! (haria falta un servidor COM registrado). La de **protocolo** si: cada
//! boton lleva `<esquema>:pedido?json=<pedido>` (`pixpin:` si es nuestro,
//! si no `pixpinmax:`: `orden::esquema_de_la_casa`), que Windows abre con
//! `"pixpinmax.exe" --url "%1"`. Es **el mismo protocolo** que el de la
//! linea de ordenes y los enlaces (`orden`, `docs/integraciones.md`): un
//! solo traductor (`orden::de_enlace`) y una sola lista blanca
//! (`orden::seguridad`, `orden::SEGURAS`). `--atajo url "%1"`, como se
//! registraba antes, es un alias de `--url`.
//!
//! Cualquier pagina web puede lanzar el enlace, asi que solo pasa directo lo
//! de la lista blanca: abrir y ver cosas, escribir, y lo de los botones
//! (marcar una tarea, posponer un recordatorio, conservar una captura). Lo
//! demas pregunta con un cuadro; una ruta de red, nunca.
//!
//! Lo puro (el XML, las entradas del registro, la seña) se prueba sin
//! Windows; [`mostrar`] es lo unico que saca un toast de verdad.

use std::path::{Path, PathBuf};

/// La identidad de PixPin Max para las notificaciones sin paquete MSIX.
pub const AUMID: &str = "PixPinMax.App";
/// El nombre que sale en la notificacion.
pub const NOMBRE: &str = "PixPin Max";
/// El esquema que siempre es nuestro y que se registra al arrancar
/// (`pixpin:` solo si esta libre: [`registrar`]).
pub const ESQUEMA: &str = "pixpinmax";

/// Un boton: su rotulo y el pedido que manda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boton {
    pub rotulo: String,
    pub pedido: serde_json::Value,
}

impl Boton {
    pub fn nuevo(rotulo: impl Into<String>, pedido: serde_json::Value) -> Boton {
        Boton {
            rotulo: rotulo.into(),
            pedido,
        }
    }
}

/// Lo que dice una notificacion.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Toast {
    pub titulo: String,
    pub texto: String,
    /// Una imagen grande dentro (una captura).
    pub imagen: Option<PathBuf>,
    /// El pedido de pulsar la notificacion (fuera de los botones).
    pub al_pulsar: Option<serde_json::Value>,
    /// Hasta 5 (lo que admite Windows); los de mas no salen.
    pub botones: Vec<Boton>,
}

/// Escapa un texto para meterlo en el XML (contenido o atributo).
pub fn escapar(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            '\'' => s.push_str("&apos;"),
            // Los de control (salvo el salto) no caben en XML 1.0.
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => s.push(' '),
            c => s.push(c),
        }
    }
    s
}

/// `file:///C:/a%20b/c.png`, para el `src` de una imagen.
fn uri_de_fichero(r: &Path) -> String {
    let t = r.display().to_string().replace('\\', "/");
    let mut s = String::from("file:///");
    for b in t.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/:".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

/// **El XML del toast** (`ToastGeneric`): titulo, una linea, la imagen y
/// los botones, todos por protocolo.
pub fn xml(t: &Toast) -> String {
    let mut x = String::from("<toast");
    if let Some(p) = &t.al_pulsar {
        x.push_str(&format!(
            " activationType=\"protocol\" launch=\"{}\"",
            escapar(&sena(p))
        ));
    }
    x.push_str("><visual><binding template=\"ToastGeneric\">");
    x.push_str(&format!("<text>{}</text>", escapar(&t.titulo)));
    if !t.texto.trim().is_empty() {
        x.push_str(&format!("<text>{}</text>", escapar(&t.texto)));
    }
    if let Some(i) = &t.imagen {
        x.push_str(&format!(
            "<image placement=\"hero\" src=\"{}\"/>",
            escapar(&uri_de_fichero(i))
        ));
    }
    x.push_str("</binding></visual>");
    if !t.botones.is_empty() {
        x.push_str("<actions>");
        for b in t.botones.iter().take(5) {
            x.push_str(&format!(
                "<action content=\"{}\" activationType=\"protocol\" arguments=\"{}\"/>",
                escapar(&b.rotulo),
                escapar(&sena(&b.pedido))
            ));
        }
        x.push_str("</actions>");
    }
    x.push_str("</toast>");
    x
}

/// Lo que se escribe en `HKCU`, como (clave, valor, texto): la identidad y
/// el protocolo. `icono`: un PNG para la notificacion, si hay.
pub fn entradas(exe: &Path, icono: Option<&Path>) -> Vec<(String, String, String)> {
    entradas_con(exe, icono, &[ESQUEMA])
}

/// [`entradas`] con los esquemas que se registran. La orden es la misma de
/// `herramientas/integraciones.ps1`: `--url "%1"`.
pub fn entradas_con(
    exe: &Path,
    icono: Option<&Path>,
    esquemas: &[&str],
) -> Vec<(String, String, String)> {
    let exe = exe.display().to_string();
    let id = format!(r"Software\Classes\AppUserModelId\{AUMID}");
    let mut v = vec![(id.clone(), "DisplayName".to_string(), NOMBRE.to_string())];
    if let Some(i) = icono {
        v.push((id, "IconUri".into(), i.display().to_string()));
    }
    for esquema in esquemas {
        let p = format!(r"Software\Classes\{esquema}");
        v.push((p.clone(), String::new(), format!("URL:{NOMBRE}")));
        v.push((p.clone(), "URL Protocol".into(), String::new()));
        v.push((
            format!(r"{p}\DefaultIcon"),
            String::new(),
            format!("\"{exe}\",0"),
        ));
        v.push((
            format!(r"{p}\shell\open\command"),
            String::new(),
            format!("\"{exe}\" --url \"%1\""),
        ));
    }
    v
}

/// **Da de alta la identidad y el protocolo** (idempotente: se llama en
/// cada arranque, y si el exe se movio deja la ruta nueva). `pixpinmax:`
/// siempre; `pixpin:` solo si esta libre o ya es nuestro
/// (`orden::pixpin_es_nuestro_o_libre`): no se le quita al PixPin original.
pub fn registrar(exe: &Path, icono: Option<&Path>) -> windows::core::Result<()> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};
    use windows::core::HSTRING;
    let esquemas: &[&str] = if crate::orden::pixpin_es_nuestro_o_libre() {
        &[ESQUEMA, "pixpin"]
    } else {
        &[ESQUEMA]
    };
    for (clave, nombre, texto) in entradas_con(exe, icono, esquemas) {
        let v = HSTRING::from(texto.as_str());
        let bytes = (v.len() + 1) * 2;
        // SAFETY: las cadenas son HSTRING vivas durante la llamada, con su
        // cero final; `bytes` cubre el texto y ese cero.
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(clave.as_str()),
                &HSTRING::from(nombre.as_str()),
                REG_SZ.0,
                Some(v.as_ptr() as *const _),
                bytes as u32,
            )
            .ok()?;
        }
    }
    Ok(())
}

/// Como acabo [`mostrar`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mostrado {
    Si,
    /// Windows no la deja salir (apagadas para PixPin o para todo, o por
    /// directiva): quien llama usa otra forma de avisar.
    Apagadas,
}

/// **Saca el toast.** `Err` si Windows no tiene la identidad o no hay
/// WinRT; [`Mostrado::Apagadas`] si las notificaciones estan apagadas. En
/// los dos casos quien llama cae a su aviso de siempre. Tarda unos
/// milisegundos (WinRT): mejor fuera del hilo de una ventana.
pub fn mostrar(t: &Toast) -> windows::core::Result<Mostrado> {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{
        NotificationSetting, ToastNotification, ToastNotificationManager,
    };
    use windows::core::HSTRING;
    let _com = crate::explorador::ComDelHilo::iniciar();
    let doc = XmlDocument::new()?;
    doc.LoadXml(&HSTRING::from(xml(t)))?;
    // Con el paquete de identidad (MSIX, `ForgePixPin.PixPinMax`), Windows ya
    // sabe quien es: sin id. Sin el, la id dada de alta en [`registrar`].
    let notificador = if tiene_identidad() {
        ToastNotificationManager::CreateToastNotifier()?
    } else {
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?
    };
    // Recien dada de alta la id (el primer arranque), `Setting` puede fallar
    // con «Element not found» y `Show` funcionar igual (medido 15-oct): solo
    // un «apagadas» de verdad corta aqui.
    if let Ok(s) = notificador.Setting()
        && s != NotificationSetting::Enabled
    {
        return Ok(Mostrado::Apagadas);
    }
    let toast = ToastNotification::CreateToastNotification(&doc)?;
    notificador.Show(&toast)?;
    Ok(Mostrado::Si)
}

/// Si el proceso corre con identidad de paquete (MSIX): la de `compartido`.
pub fn tiene_identidad() -> bool {
    crate::compartido::tiene_identidad()
}

/// [`mostrar`] en su hilo; si no sale, `si_no` (el aviso de siempre).
pub fn mostrar_o(t: Toast, si_no: impl FnOnce(&Toast) + Send + 'static) {
    let _ = std::thread::Builder::new()
        .name("notificacion".into())
        .spawn(move || match mostrar(&t) {
            Ok(Mostrado::Si) => {}
            Ok(Mostrado::Apagadas) => si_no(&t),
            Err(e) => {
                tracing::warn!(?e, "sin notificacion de Windows: va el aviso de siempre");
                si_no(&t);
            }
        });
}

// --- La seña de los botones -------------------------------------------------

/// La seña de un pedido: `<esquema de la casa>:pedido?json=…` (el mismo
/// enlace que `pixpin://pedido?json=…` de la linea de ordenes).
///
/// **Firmada** (`orden::firma`): un boton lo hizo la propia app, asi que se
/// hace sin preguntar aunque lleve una ruta («Abrir» una captura). En las
/// pruebas, sin firmar: no se toca la clave del equipo.
pub fn sena(pedido: &serde_json::Value) -> String {
    let s = sena_con(crate::orden::esquema_de_la_casa(), pedido);
    if cfg!(test) {
        return s;
    }
    crate::orden::firma::firmar(&s).unwrap_or(s)
}

/// [`sena`] con un esquema dado (las pruebas, que no deben depender de lo
/// que haya en el registro de este equipo).
pub fn sena_con(esquema: &str, pedido: &serde_json::Value) -> String {
    format!(
        "{esquema}:pedido?json={}",
        crate::orden::codificar(&pedido.to_string())
    )
}

/// Si un pedido puede llegar por el protocolo sin preguntar: la lista blanca
/// de los enlaces (`orden::seguridad`), que es una sola.
pub fn permitido(p: &serde_json::Value) -> bool {
    p.is_object()
        && p.get("pixpin").and_then(|v| v.as_u64()) == Some(crate::mensajero::VERSION_PEDIDO)
        && crate::orden::seguridad(p) == crate::orden::Seguridad::Directa
}

/// El pedido de una seña, si pasa sin preguntar (o va firmada por la app,
/// `orden::firma`). Lo que no se entiende o pide confirmar es `None`: quien
/// quiera el cuadro de confirmar usa la linea de ordenes (`--url`), que es
/// lo que registra el protocolo.
pub fn pedido_de(sena: &str) -> Option<String> {
    let l = crate::orden::de_enlace(sena).ok()?;
    let crate::orden::Orden::Pedidos(v) = l.orden else {
        return None;
    };
    let [p] = v.as_slice() else { return None };
    let firmado = matches!(l.origen, crate::orden::Origen::Enlace { firmado: true });
    let vale = permitido(p)
        || (firmado
            && !matches!(
                crate::orden::seguridad(p),
                crate::orden::Seguridad::Prohibida(_)
            ));
    vale.then(|| p.to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    fn abrir() -> serde_json::Value {
        json!({"pixpin": 1, "accion": "abrir", "que": {"tipo": "mensaje", "proyecto": "a b", "codigo": "m&1"}})
    }

    fn toast() -> Toast {
        Toast {
            titulo: "Hecho: apunta <yeso> & cal".into(),
            texto: "Apunté \"comprar yeso\" en Inbox".into(),
            ..Default::default()
        }
    }

    #[test]
    fn el_xml_escapa_y_cada_boton_lleva_su_pedido_por_protocolo() {
        let mut t = toast();
        t.al_pulsar = Some(abrir());
        t.botones = vec![
            Boton::nuevo(
                "Hecha",
                json!({"pixpin": 1, "accion": "marcar_tarea", "codigo": "L", "indice": 2, "hecha": true}),
            ),
            Boton::nuevo("Abrir", abrir()),
        ];
        let x = xml(&t);
        assert!(
            x.starts_with(&format!(
                "<toast activationType=\"protocol\" launch=\"{}\">",
                sena(&abrir())
            )),
            "{x}"
        );
        assert!(x.contains("<text>Hecho: apunta &lt;yeso&gt; &amp; cal</text>"));
        assert!(x.contains("<text>Apunté &quot;comprar yeso&quot; en Inbox</text>"));
        assert!(x.contains(&format!("<action content=\"Hecha\" activationType=\"protocol\" arguments=\"{}:pedido?json=%7B%22", crate::orden::esquema_de_la_casa())), "{x}");
        assert_eq!(x.matches("<action ").count(), 2);
        // La seña no lleva nada que haya que escapar en XML.
        assert!(!sena(&abrir()).contains(['&', '"', '<']));
    }

    #[test]
    fn sin_botones_ni_imagen_no_salen() {
        let x = xml(&toast());
        assert!(
            !x.contains("<actions>") && !x.contains("launch=") && !x.contains("<image"),
            "{x}"
        );
        assert!(
            !xml(&Toast {
                texto: "  ".into(),
                ..toast()
            })
            .contains("<text>  </text>")
        );
        // Mas de cinco botones: Windows rechazaria el toast entero.
        let t = Toast {
            botones: vec![Boton::nuevo("x", abrir()); 7],
            ..toast()
        };
        assert_eq!(xml(&t).matches("<action ").count(), 5);
    }

    #[test]
    fn la_imagen_va_como_file_uri() {
        let t = Toast {
            imagen: Some(PathBuf::from(r"C:\Datos\capturas\captura 0001.png")),
            ..toast()
        };
        assert!(
            xml(&t).contains(
                "<image placement=\"hero\" src=\"file:///C:/Datos/capturas/captura%200001.png\"/>"
            ),
            "{}",
            xml(&t)
        );
    }

    #[test]
    fn los_caracteres_de_control_no_rompen_el_xml() {
        assert_eq!(escapar("a\u{1}b\nc"), "a b\nc");
    }

    #[test]
    fn la_sena_va_y_vuelve() {
        for p in [
            abrir(),
            json!({"pixpin": 1, "accion": "asistente"}),
            json!({"pixpin": 1, "accion": "posponer", "proyecto": "ñu", "mensaje": "m1", "minutos": 10}),
            json!({"pixpin": 1, "accion": "conservar_captura", "ruta": r"C:\a b\c.png"}),
        ] {
            assert_eq!(pedido_de(&sena(&p)), Some(p.to_string()), "{}", sena(&p));
        }
        // Lo que dejan los navegadores (`pixpinmax://pedido/?json=`) vale igual.
        let s = sena_con("pixpinmax", &abrir()).replace("pixpinmax:pedido", "pixpinmax://pedido/");
        assert_eq!(pedido_de(&s), Some(abrir().to_string()));
    }

    #[test]
    fn por_el_protocolo_no_pasa_lo_que_borra_envia_o_escribe() {
        for p in [
            json!({"pixpin": 1, "accion": "borrar_lista", "codigo": "L"}),
            json!({"pixpin": 1, "accion": "enviar_al_movil", "aparato": "x"}),
            json!({"pixpin": 1, "accion": "asistente", "texto": "borra todo"}),
            json!({"pixpin": 1, "accion": "asistente", "automatizacion": "a1"}),
            json!({"pixpin": 2, "accion": "abrir"}),
            json!({"accion": "abrir"}),
            json!(["abrir"]),
        ] {
            assert_eq!(pedido_de(&sena_con("pixpinmax", &p)), None, "{p}");
        }
        for s in [
            "",
            "http:pedido?json=%7B%7D",
            "pixpinmax:borrar_todo",
            "pixpinmax:pedido?json=%ZZ",
            "pixpinmax:pedido?x=1",
            "pixpinmax:pedido",
        ] {
            assert_eq!(pedido_de(s), None, "{s}");
        }
    }

    #[test]
    fn la_copia_que_abre_windows_lo_convierte_en_pedido() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let s = sena(&json!({"pixpin": 1, "accion": "asistente"}));
        assert_eq!(
            crate::jump_list::pedido_de_atajo(&args(&["--atajo", "url", &s])).as_deref(),
            Some(r#"{"accion":"asistente","pixpin":1}"#)
        );
        assert_eq!(
            crate::jump_list::pedido_de_atajo(&args(&["--atajo", "url"])),
            None
        );
        assert_eq!(
            crate::jump_list::pedido_de_atajo(&args(&["--atajo", "url", "pixpinmax:borrar"])),
            None
        );
    }

    #[test]
    fn el_registro_lleva_identidad_y_protocolo() {
        let v = entradas(
            Path::new(r"C:\P\pixpinmax.exe"),
            Some(Path::new(r"C:\P\i.png")),
        );
        let valor = |c: &str, n: &str| {
            v.iter()
                .find(|(a, b, _)| a == c && b == n)
                .map(|x| x.2.clone())
        };
        assert_eq!(
            valor(
                r"Software\Classes\AppUserModelId\PixPinMax.App",
                "DisplayName"
            )
            .as_deref(),
            Some("PixPin Max")
        );
        assert_eq!(
            valor(r"Software\Classes\AppUserModelId\PixPinMax.App", "IconUri").as_deref(),
            Some(r"C:\P\i.png")
        );
        assert_eq!(
            valor(r"Software\Classes\pixpinmax", "URL Protocol").as_deref(),
            Some("")
        );
        assert_eq!(
            valor(r"Software\Classes\pixpinmax\shell\open\command", "").as_deref(),
            Some(r#""C:\P\pixpinmax.exe" --url "%1""#)
        );
        // Sin icono, no se escribe IconUri.
        assert!(
            !entradas(Path::new("x.exe"), None)
                .iter()
                .any(|e| e.1 == "IconUri")
        );
    }

    /// Saca un toast de verdad. A mano, nunca en las pruebas automaticas:
    /// `cargo test -p pixpin-shell -- --ignored toast_de_verdad`.
    #[test]
    #[ignore]
    fn toast_de_verdad() {
        // El exe instalado, no el de la prueba: el protocolo tiene que seguir
        // abriendo la PixPin de verdad.
        let exe = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap())
            .join(r"Programs\PixPinMax\pixpinmax.exe");
        registrar(&exe, None).unwrap();
        let r = mostrar(&Toast {
            titulo: "Hecho: prueba".into(),
            texto: "Una notificacion de prueba de PixPin Max".into(),
            botones: vec![Boton::nuevo(
                "Abrir",
                json!({"pixpin": 1, "accion": "asistente"}),
            )],
            ..Default::default()
        });
        println!("{r:?}");
    }
}
