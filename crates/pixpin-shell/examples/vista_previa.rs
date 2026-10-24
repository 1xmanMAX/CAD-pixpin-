//! **Probar la vista previa de Office a mano** y medir lo que tarda.
//!
//! ```text
//! cargo run --release -p pixpin-shell --example vista_previa -- "C:\ruta\Carta.doc" [otro.xls ...]
//! ```
//!
//! Por cada fichero abre una ventana, pide la vista previa dos veces (las
//! dos con Office arrancando: no se guarda nada entre una y otra), escribe lo que
//! tardo cada una y se cierra sola: unos segundos en pantalla y nada mas.
//! No toca el teclado ni el raton de nadie.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use pixpin_geom::Rect;
use pixpin_shell::overlay::{self, VentanaOverlay};
use pixpin_shell::vista_previa::{self, Carteles, Estado, VistaPrevia};

/// Lo que se espera a que una vista este como mucho.
const TOPE: Duration = Duration::from_secs(40);
/// Lo que se deja vista cada una, para verla.
const A_LA_VISTA: Duration = Duration::from_millis(1200);

fn esperar(t: Duration) {
    let fin = Instant::now() + t;
    while Instant::now() < fin {
        overlay::bombear_pendientes();
        let _ = overlay::tomar_eventos_pendientes();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Una foto de la ventana `hwnd` (lo que hay en pantalla en su sitio) en
/// `destino`, con PowerShell y GDI+. Solo para mirar a mano.
fn capturar(hwnd: isize, destino: &std::path::Path) {
    let guion = format!(
        r#"Add-Type -AssemblyName System.Drawing
Add-Type -Namespace V -Name W -MemberDefinition '[DllImport("user32.dll")] public static extern bool GetWindowRect(System.IntPtr h, out RECT r); [DllImport("user32.dll")] public static extern bool SetProcessDPIAware(); public struct RECT {{ public int L, T, R, B; }}'
[V.W]::SetProcessDPIAware() | Out-Null
$r = New-Object V.W+RECT
[V.W]::GetWindowRect([IntPtr]{hwnd}, [ref]$r) | Out-Null
$b = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
$g = [System.Drawing.Graphics]::FromImage($b)
$g.CopyFromScreen($r.L, $r.T, 0, 0, $b.Size)
$b.Save('{}', [System.Drawing.Imaging.ImageFormat]::Png)"#,
        destino.display()
    );
    // Mientras PowerShell hace la foto, la ventana sigue atendiendo.
    let hijo = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &guion])
        .spawn();
    if let Ok(mut h) = hijo {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(15) {
            if let Ok(Some(_)) = h.try_wait() {
                break;
            }
            esperar(Duration::from_millis(50));
        }
        println!("foto: {}", destino.display());
    }
}

fn una_vez(ruta: &std::path::Path, vuelta: &str) -> Option<Estado> {
    let area = Rect {
        x: 120,
        y: 80,
        ancho: 900,
        alto: 700,
    };
    let ventana = VentanaOverlay::nueva_llana(area, "PixPin - vista previa (prueba)").ok()?;
    ventana.poner_siempre_encima(true);
    ventana.mostrar();
    let carteles = Carteles {
        abriendo: "Abriendo…".into(),
        tarda: "Tarda mas de lo normal…".into(),
        fallo: "No se pudo previsualizar".into(),
    };
    let t = Instant::now();
    let dentro = Rect {
        x: 7,
        y: 7,
        ancho: area.ancho - 14,
        alto: area.alto - 14,
    };
    // Como la app: lo que viene de internet no se previsualiza, sale el aviso.
    let vista = if vista_previa::bloqueado_por_internet(ruta) {
        VistaPrevia::solo_cartel(
            ventana.handle(),
            dentro,
            "Viene de internet: Windows no deja previsualizarlo",
        )
    } else {
        VistaPrevia::abrir(ventana.handle(), ruta, dentro, carteles)
    };
    let Some(vista) = vista else {
        println!("{vuelta}: sin previsualizador para {}", ruta.display());
        return None;
    };
    vista_previa::pintar_fondo(ventana.handle());
    let mut estado = vista.estado();
    while estado == Estado::Abriendo && t.elapsed() < TOPE {
        overlay::bombear_pendientes();
        let _ = overlay::tomar_eventos_pendientes();
        std::thread::sleep(Duration::from_millis(5));
        estado = vista.estado();
    }
    println!(
        "{vuelta}: {:?} (visto por la app a los {} ms)",
        estado,
        t.elapsed().as_millis()
    );
    esperar(A_LA_VISTA);
    // Con `PIXPIN_VP_CAPTURAS=<carpeta>`, una foto de la ventana tal cual
    // (para ver si Office la pinto): la hace PowerShell, solo de esta ventana.
    if let Ok(carpeta) = std::env::var("PIXPIN_VP_CAPTURAS") {
        let nombre = ruta
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let destino = PathBuf::from(carpeta).join(format!("{nombre}-{vuelta}.png"));
        capturar(ventana.handle().0 as isize, &destino);
    }
    // Probar el estirar: la vista sigue a la ventana.
    vista.mover(Rect {
        x: 7,
        y: 7,
        ancho: 700,
        alto: 500,
    });
    esperar(Duration::from_millis(300));
    let tc = Instant::now();
    let cerrada = vista.cerrar(Duration::from_secs(15));
    println!(
        "{vuelta}: cerrada={cerrada} en {} ms",
        tc.elapsed().as_millis()
    );
    if !cerrada {
        ventana.ocultar();
        std::mem::forget(ventana);
    }
    Some(estado)
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();
    let rutas: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if rutas.is_empty() {
        eprintln!("uso: vista_previa <fichero> [fichero...]");
        return;
    }
    for ruta in &rutas {
        println!(
            "== {} (manejador {:?}, de internet: {})",
            ruta.display(),
            vista_previa::manejador_de(ruta),
            vista_previa::bloqueado_por_internet(ruta)
        );
        una_vez(ruta, "primera");
        // Sin ventana en pantalla: el servidor de Office de la primera tiene
        // que irse (no se guarda nada); la segunda vuelve a arrancarlo.
        esperar(Duration::from_secs(3));
        una_vez(ruta, "segunda");
    }
    vista_previa::soltar_todo(Duration::from_secs(10));
    if let Ok(s) = std::env::var("PIXPIN_VP_ESPERA_FINAL") {
        esperar(Duration::from_secs(s.parse().unwrap_or(0)));
    }
}
