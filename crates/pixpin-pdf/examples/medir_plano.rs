//! **Mide el plano vectorial contra la foto** con PDF de verdad: cuanto
//! tarda leer cada pagina como lineas, cuanto pesa el paquete de la web y
//! cuanto pesaba el JPEG que sustituye (el de `documento_web`: 1600 px de
//! ancho a calidad 72), y cuanto tarda Windows en pintar una tesela.
//!
//! `cargo run --release -p pixpin-pdf --example medir_plano -- <pdf>...`

use std::time::Instant;

/// **Un plano como los de AutoCAD**, para medir sin depender de uno de
/// verdad: una A1 apaisada con veinte capas OCG y `tramos` segmentos, cada
/// uno escrito por separado (`m l S`) como los escribe AutoCAD aunque sigan
/// una polilinea, en un contenido comprimido.
fn generar(ruta: &std::path::Path, tramos: usize) {
    use std::fmt::Write as _;
    use std::io::Write as _;
    let (an, al) = (2384.0f64, 1684.0f64);
    let mut c = String::with_capacity(tramos * 40);
    c.push_str("0.12 0 0 0.12 0 0 cm 1 J 1 j\n");
    let por_capa = tramos / 20 + 1;
    let mut semilla: u64 = 7;
    let mut azar = || {
        semilla = semilla
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (semilla >> 33) as f64 / (1u64 << 31) as f64
    };
    let mut hechos = 0;
    for capa in 0..20 {
        let _ = writeln!(
            c,
            "/OC /oc{capa} BDC {} {} {} RG {} w",
            (capa % 3) as f64 / 3.0,
            (capa % 5) as f64 / 5.0,
            (capa % 7) as f64 / 7.0,
            1 + capa % 4
        );
        let mut n = 0;
        while n < por_capa && hechos < tramos {
            // Una polilinea de 10 a 60 tramos que va dando vueltas.
            let largo = 10 + (azar() * 50.0) as usize;
            let (mut x, mut y) = (azar() * an / 0.12, azar() * al / 0.12);
            for _ in 0..largo {
                let (nx, ny) = (
                    (x + (azar() - 0.5) * 400.0).clamp(0.0, an / 0.12),
                    (y + (azar() - 0.5) * 400.0).clamp(0.0, al / 0.12),
                );
                let _ = writeln!(c, "{x:.1} {y:.1} m {nx:.1} {ny:.1} l S");
                (x, y) = (nx, ny);
                n += 1;
                hechos += 1;
            }
        }
        c.push_str("EMC\n");
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(c.as_bytes()).unwrap();
    let contenido = z.finish().unwrap();
    let ocgs: String = (0..20).map(|i| format!("{} 0 R ", 5 + i)).collect();
    let props: String = (0..20).map(|i| format!("/oc{i} {} 0 R ", 5 + i)).collect();
    let mut objetos: Vec<Vec<u8>> = vec![
        format!("1 0 obj\n<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [{ocgs}] /D << /Order [{ocgs}] /OFF [24 0 R] >> >> >>\nendobj\n").into_bytes(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {an} {al}] /Contents 4 0 R /Resources << /Properties << {props}>> >> >>\nendobj\n").into_bytes(),
    ];
    let mut f = format!(
        "4 0 obj\n<< /Length {} /Filter /FlateDecode >>\nstream\n",
        contenido.len()
    )
    .into_bytes();
    f.extend_from_slice(&contenido);
    f.extend_from_slice(b"\nendstream\nendobj\n");
    objetos.push(f);
    for i in 0..20 {
        objetos.push(
            format!(
                "{} 0 obj\n<< /Type /OCG /Name (A-CAPA-{i:02}) >>\nendobj\n",
                5 + i
            )
            .into_bytes(),
        );
    }
    let mut s = b"%PDF-1.6\n".to_vec();
    let mut donde = Vec::new();
    for o in &objetos {
        donde.push(s.len());
        s.extend_from_slice(o);
    }
    let inicio = s.len();
    let mut t = format!("xref\n0 {}\n0000000000 65535 f \n", objetos.len() + 1);
    for d in donde {
        let _ = writeln!(t, "{d:010} 00000 n ");
    }
    let _ = write!(
        t,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{inicio}\n%%EOF\n",
        objetos.len() + 1
    );
    s.extend_from_slice(t.as_bytes());
    std::fs::write(ruta, s).unwrap();
}

fn main() {
    let _com = ComDelHilo::nuevo();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--generar") {
        let tramos = args.get(2).and_then(|n| n.parse().ok()).unwrap_or(300_000);
        generar(std::path::Path::new(&args[1]), tramos);
        return;
    }
    for ruta in args {
        let ruta = std::path::PathBuf::from(ruta);
        let Ok(bytes) = std::fs::read(&ruta) else {
            println!("{}: no se lee", ruta.display());
            continue;
        };
        let doc = pixpin_pdf::Documento::abrir(&ruta).ok();
        let paginas = doc.as_ref().map_or(1, |d| d.paginas()).min(6) as usize;
        println!(
            "== {} ({} KB, {} paginas miradas)",
            ruta.display(),
            bytes.len() / 1024,
            paginas
        );
        for p in 0..paginas {
            let t0 = Instant::now();
            let plano = pixpin_pdf::plano::de_bytes(&bytes, p);
            let leer = t0.elapsed();
            let Some(plano) = plano else {
                println!("  pag {p}: no se entiende");
                continue;
            };
            let t1 = Instant::now();
            let json =
                pixpin_pdf::plano_web::a_json(&plano, pixpin_pdf::plano_web::ANCHO_EN_UNIDADES);
            let empaquetar = t1.elapsed();
            let (jpeg, pintar) = match &doc {
                Some(d) => {
                    let t2 = Instant::now();
                    let img = d.renderizar(p as u32, 1600).ok();
                    if let (Some(i), Some(dir)) = (&img, std::env::var_os("PIXPIN_PLANO_PNG")) {
                        let _ = pixpin_codec::imagen::codificar_jpg(i, 80).map(|j| {
                            std::fs::write(
                                std::path::Path::new(&dir).join(format!("windows-{p}.jpg")),
                                j,
                            )
                        });
                    }
                    if let Some(m) = d.medidas().get(p) {
                        println!("  medidas de Windows: {:?}", m);
                    }
                    let pintar = t2.elapsed();
                    (
                        img.and_then(|i| pixpin_codec::imagen::codificar_jpg(&i, 72).ok())
                            .map_or(0, |j| j.len()),
                        pintar,
                    )
                }
                None => (0, Default::default()),
            };
            let tesela = doc.as_ref().map(|d| {
                let t3 = Instant::now();
                let _ = d.renderizar_trozo(p as u32, (0.25, 0.25, 0.125, 0.125), 512, 512);
                t3.elapsed()
            });
            println!(
                "  pag {p}: {:.0}x{:.0} pt · {} puntos · {} brochas · {} textos · {} fotos · {} capas · sin entender {} · cortado {} · lineas {}",
                plano.ancho,
                plano.alto,
                plano.puntos(),
                plano.brochas.len(),
                plano.textos.len(),
                plano.fotos.len(),
                plano.capas.len(),
                plano.sin_entender,
                plano.cortado,
                plano.se_manda_como_lineas()
            );
            println!(
                "          leer {:?} · empaquetar {:?} · web {} KB · JPEG 1600 {} KB (pintar {:?}) · tesela 512 {:?}",
                leer,
                empaquetar,
                json.len() / 1024,
                jpeg / 1024,
                pintar,
                tesela.unwrap_or_default()
            );
        }
    }
}

/// COM en este hilo, para `Windows.Data.Pdf`.
struct ComDelHilo;
impl ComDelHilo {
    fn nuevo() -> ComDelHilo {
        // SAFETY: se inicia COM en el hilo principal del ejemplo y se cierra
        // al soltarlo; nada mas lo usa.
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            );
        }
        ComDelHilo
    }
}
impl Drop for ComDelHilo {
    fn drop(&mut self) {
        // SAFETY: empareja el `CoInitializeEx` de `nuevo`.
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    }
}
