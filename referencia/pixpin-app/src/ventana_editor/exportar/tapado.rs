//! **Lo pixelado sale pixelado al exportar** (F18).
//!
//! En pantalla el mosaico tapa leyendo los pixeles ya pintados (`tapar.rs`),
//! pero al exportar no hay pantalla: las hojas son ordenes y el mosaico salia
//! como su banda gris maciza (tapaba, pero no se veia pixelado), y el usuario
//! lo vio: «el pixelado no se ve al exportar».
//!
//! El movil lo resuelve pintando **lo de debajo** del mosaico a la
//! resolucion de su grano (`fondoDelDibujo`/`miniaturaDe` de `Renderer.kt`) y
//! metiendo esa miniatura como imagen en el PNG, el SVG y el PDF. Aqui igual:
//! antes de exportar, cada mosaico se cambia por una **imagen ya tapada** de
//! lo que tiene debajo (el papel, las fotos y lo dibujado antes que el, sin
//! otros mosaicos ni focos, como alli), pixelada o desenfocada con la misma
//! receta de la pantalla (`mosaico::tapar_rgba`). Como es una imagen normal,
//! sale igual en todos los formatos —PNG, JPG, SVG, PDF, la web, imprimir,
//! copiar, compartir y la foto de una zona— sin que cada uno tenga que saber
//! nada del mosaico.
//!
//! **Privacidad antes que nada**: si no se puede pintar lo de debajo (sin
//! GPU, una caja rara), el mosaico se queda como estaba —su banda maciza—,
//! que tapa. Nunca sale en claro.

use std::collections::HashMap;

use pixpin_codec::ImagenRgba;
use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::{Escena, mosaico};

use super::Lienzo;

/// Los ids de las imagenes tapadas: por debajo del papel (`ID_PAPEL` =
/// `u64::MAX`), lejos de cualquier id de imagen de verdad.
const PRIMER_ID: u64 = u64::MAX - 1;

/// Lo que mide como mucho, por lado, la imagen de un mosaico. A la escala
/// de la hoja (x2) sobra para que los cuadros salgan con el canto duro; mas
/// seria pesar sin ensenar nada, porque lo de dentro esta tapado.
const LADO_MAXIMO: f32 = 2048.0;

/// La escala a la que se pinta lo de debajo: el doble, como el PNG de fabrica,
/// para que al exportar a x2 los cuadros no salgan con el borde emborronado.
const ESCALA: f32 = 2.0;

/// **La escena con cada mosaico cambiado por su imagen tapada**, y esas
/// imagenes por id. `None` si no hay ningun mosaico (lo normal: exportar no
/// paga nada).
pub(crate) struct Tapada {
    pub escena: Escena,
    pub imagenes: HashMap<u64, ImagenRgba>,
}

/// Lo que se pinta debajo del mosaico `i`: lo visible anterior a el, sin
/// mosaicos ni focos (`fondoDelDibujo`: un mosaico que se pixela a si mismo
/// se degradaria, y la sombra del foco taparia el grano con una mancha).
pub(crate) fn escena_de_debajo(escena: &Escena, i: usize) -> Escena {
    let mut debajo = escena.clone();
    debajo.elementos = escena.elementos[..i]
        .iter()
        .filter(|e| !e.borrado && !matches!(e.figura, Figura::Mosaico { .. } | Figura::Foco { .. }))
        .cloned()
        .collect();
    debajo
}

/// El elemento que ocupa el sitio del mosaico: una imagen derecha sobre la
/// caja que tapa (la girada, que tapa de mas y nunca de menos), con el mismo
/// id para que la seleccion y los marcos lo sigan contando.
pub(crate) fn imagen_en_su_sitio(
    m: &Elemento,
    caja: (f32, f32, f32, f32),
    id_objeto: u64,
) -> Elemento {
    Elemento {
        figura: Figura::Imagen { id_objeto },
        x: caja.0,
        y: caja.1,
        ancho: caja.2 - caja.0,
        alto: caja.3 - caja.1,
        angulo: 0.0,
        // Sin la opacidad del mosaico: uno a medias no tapa (regla 1 de
        // `mosaico.rs`).
        opacidad: 1.0,
        relleno: None,
        ..m.clone()
    }
}

/// **Tapa los mosaicos de `lienzo`** como en el movil. Pinta con la GPU (su
/// propio dispositivo, como `a_imagen`).
pub(crate) fn tapar(lienzo: &Lienzo<'_>) -> Option<Tapada> {
    tapar_dentro(lienzo, None)
}

/// Como [`tapar`], solo con los mosaicos que tocan `caja` (la foto de una
/// zona no tiene por que pintar los de la otra punta del lienzo).
pub(crate) fn tapar_dentro(
    lienzo: &Lienzo<'_>,
    caja: Option<(f32, f32, f32, f32)>,
) -> Option<Tapada> {
    let toca = |c: (f32, f32, f32, f32)| {
        caja.is_none_or(|z| c.0 < z.2 && c.2 > z.0 && c.1 < z.3 && c.3 > z.1)
    };
    let tapados: Vec<(usize, mosaico::Tapado)> = lienzo
        .escena
        .elementos
        .iter()
        .enumerate()
        .filter_map(|(i, e)| mosaico::tapado_de(e).map(|t| (i, t)))
        .filter(|(_, t)| toca(t.caja))
        .collect();
    if tapados.is_empty() {
        return None;
    }
    let mut escena = lienzo.escena.clone();
    let mut imagenes = HashMap::new();
    let papel = lienzo.papel.map(|(_, w, h)| (w, h));
    for (k, (i, t)) in tapados.into_iter().enumerate() {
        let debajo = escena_de_debajo(lienzo.escena, i);
        let (ancho, alto) = (t.caja.2 - t.caja.0, t.caja.3 - t.caja.1);
        let escala = ESCALA
            .min(LADO_MAXIMO / ancho.max(1.0))
            .min(LADO_MAXIMO / alto.max(1.0));
        // Sin nada debajo (ni papel ni dibujo) lo tapado es papel liso: se
        // pinta igual, una hoja vacia del color del papel.
        let hoja = pixpin_motor2d::exportar::de_una_zona(&debajo, t.caja, papel).unwrap_or(
            pixpin_motor2d::exportar::Hoja {
                nombre: String::new(),
                caja: t.caja,
                ordenes: Vec::new(),
                marcos: Vec::new(),
                granos: Vec::new(),
                grafitos: Vec::new(),
            },
        );
        let fotos = |id: u64| (lienzo.fotos)(id);
        let de_debajo = Lienzo {
            escena: &debajo,
            seleccion: &[],
            papel: lienzo.papel,
            fotos: &fotos,
            nombre: String::new(),
        };
        let mut img = match super::a_imagen(&hoja, escala, Some(lienzo.escena.fondo), &de_debajo) {
            Ok(img) => img,
            Err(e) => {
                // Se queda la banda maciza: tapa igual, solo que sin grano.
                tracing::warn!(?e, "no se pudo pixelar un mosaico al exportar; va macizo");
                continue;
            }
        };
        let lado = mosaico::lado_en_pantalla(t.grano, img.ancho as f32 / ancho.max(1.0));
        let caja = (0.0, 0.0, img.ancho as f32, img.alto as f32);
        if !mosaico::tapar_rgba(
            &mut img.pixeles,
            img.ancho,
            img.alto,
            caja,
            lado,
            t.desenfoque,
        ) {
            continue;
        }
        let id_objeto = PRIMER_ID - k as u64;
        if let Some(e) = escena.elementos.get_mut(i) {
            *e = imagen_en_su_sitio(e, t.caja, id_objeto);
        }
        imagenes.insert(id_objeto, img);
    }
    Some(Tapada { escena, imagenes })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::ColorRgba;

    fn rect(id: u64, x: f32) -> Elemento {
        Elemento {
            id,
            figura: Figura::Rectangulo,
            x,
            y: 0.0,
            ancho: 50.0,
            alto: 50.0,
            ..Default::default()
        }
    }

    fn mosaico(id: u64) -> Elemento {
        Elemento {
            id,
            figura: Figura::Mosaico { desenfoque: false },
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 40.0,
            ..Default::default()
        }
    }

    #[test]
    fn lo_de_debajo_de_un_mosaico_es_lo_anterior_sin_otros_mosaicos_ni_focos() {
        let mut escena = Escena::nueva();
        escena.elementos = vec![
            rect(1, 0.0),
            mosaico(2),
            Elemento {
                id: 3,
                figura: Figura::Foco {
                    cristal: Default::default(),
                },
                ..rect(3, 0.0)
            },
            rect(4, 10.0),
            mosaico(5),
            rect(6, 20.0),
        ];
        let ids: Vec<u64> = escena_de_debajo(&escena, 4)
            .elementos
            .iter()
            .map(|e| e.id)
            .collect();
        // Caso negativo: ni el mosaico 2, ni el foco 3, ni lo de encima (6).
        assert_eq!(ids, [1, 4]);
    }

    #[test]
    fn el_mosaico_se_cambia_por_una_imagen_opaca_derecha_con_su_mismo_id() {
        let m = Elemento {
            angulo: 0.5,
            opacidad: 0.3,
            relleno: Some(ColorRgba::opaco(0.5, 0.5, 0.5)),
            ..mosaico(9)
        };
        let caja = mosaico::caja_girada(&m);
        let e = imagen_en_su_sitio(&m, caja, 77);
        assert_eq!(e.id, 9);
        assert_eq!(e.figura, Figura::Imagen { id_objeto: 77 });
        assert_eq!(e.angulo, 0.0);
        // Cubre la caja girada entera (tapa de mas, nunca de menos).
        assert_eq!((e.x, e.y, e.x + e.ancho, e.y + e.alto), caja);
        assert!(e.ancho > m.ancho, "la girada es mas ancha");
        // Opaca aunque el mosaico fuera translucido: a medias no tapa.
        assert_eq!(e.opacidad, 1.0);
    }

    fn texto(id: u64, t: &str, x: f32, y: f32, tam: f32) -> Elemento {
        Elemento {
            id,
            figura: Figura::Texto {
                texto: t.into(),
                tam,
                familia: "Excalifont".into(),
            },
            x,
            y,
            ancho: t.len() as f32 * tam * 0.6,
            alto: tam * 1.25,
            ..Default::default()
        }
    }

    fn guardar(nombre: &str, img: &ImagenRgba) {
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&carpeta).unwrap();
        let ruta = carpeta.join(format!("{nombre}.png"));
        std::fs::write(&ruta, pixpin_codec::imagen::codificar_png(img).unwrap()).unwrap();
        println!("{nombre}: {}", ruta.display());
    }

    /// Cuantos colores distintos hay en un trozo: uno solo es la banda maciza.
    fn colores_en(img: &ImagenRgba, caja: (u32, u32, u32, u32)) -> usize {
        let mut v = std::collections::HashSet::new();
        for y in caja.1..caja.3 {
            for x in caja.0..caja.2 {
                let i = ((y * img.ancho + x) * 4) as usize;
                v.insert([img.pixeles[i], img.pixeles[i + 1], img.pixeles[i + 2]]);
            }
        }
        v.len()
    }

    /// **Lo pixelado sale pixelado en el PNG exportado** y no en claro ni
    /// como una banda gris, con el mismo `atender` que el menu. Deja
    /// `mosaico-exportado-{antes,despues}.png` y `foco-y-pasos-exportado.png`.
    /// Necesita GPU: `cargo test -p pixpin --bin pixpinmax muestra_del_mosaico_exportado -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_mosaico_exportado() {
        pixpin_motor2d::texto::instalar_medidor(crate::dibujo::pintar::medir_para_el_motor);
        let mut escena = Escena::nueva();
        escena.anadir(Elemento {
            figura: Figura::Rectangulo,
            x: 10.0,
            y: 10.0,
            ancho: 420.0,
            alto: 110.0,
            relleno: Some(ColorRgba::opaco(0.65, 0.85, 1.0)),
            ..Default::default()
        });
        escena.anadir(texto(0, "Cuenta: 4557 8812 0093", 20.0, 20.0, 28.0));
        escena.anadir(texto(0, "Clave: hunter2", 20.0, 70.0, 28.0));
        let m = escena.anadir(Elemento {
            figura: Figura::Mosaico { desenfoque: false },
            x: 110.0,
            y: 15.0,
            ancho: 300.0,
            alto: 45.0,
            grosor: 1.0,
            ..Default::default()
        });
        escena.anadir(Elemento {
            figura: Figura::Mosaico { desenfoque: true },
            x: 100.0,
            y: 66.0,
            ancho: 180.0,
            alto: 45.0,
            grosor: 2.0,
            ..Default::default()
        });
        // Una flecha DESPUES del mosaico: no se pixela (va por encima).
        escena.anadir(Elemento {
            figura: Figura::Flecha {
                puntos: vec![
                    pixpin_motor2d::vector::Punto2::nuevo(470.0, 40.0),
                    pixpin_motor2d::vector::Punto2::nuevo(380.0, 40.0),
                ],
                punta_inicio: pixpin_motor2d::formas::TipoPunta::Ninguna,
                punta_fin: pixpin_motor2d::formas::TipoPunta::Flecha,
                codos: false,
            },
            trazo: ColorRgba::opaco(0.9, 0.1, 0.1),
            grosor: 3.0,
            ..Default::default()
        });
        let lienzo = Lienzo {
            escena: &escena,
            seleccion: &[],
            papel: None,
            fotos: &|_| None,
            nombre: "mosaico".into(),
        };
        let hojas = |e: &Escena| {
            pixpin_motor2d::exportar::hojas(e, pixpin_motor2d::exportar::Alcance::Todo, &[], None)
        };
        let antes =
            super::super::a_imagen(&hojas(&escena)[0], 2.0, Some(escena.fondo), &lienzo).unwrap();
        guardar("mosaico-exportado-antes", &antes);
        let t = tapar(&lienzo).expect("hay mosaicos");
        assert_eq!(t.imagenes.len(), 2);
        let fotos = |id: u64| t.imagenes.get(&id);
        let tapado = Lienzo {
            escena: &t.escena,
            seleccion: &[],
            papel: None,
            fotos: &fotos,
            nombre: String::new(),
        };
        let h = hojas(&t.escena);
        let despues = super::super::a_imagen(&h[0], 2.0, Some(escena.fondo), &tapado).unwrap();
        guardar("mosaico-exportado-despues", &despues);
        // En el SVG (y el PDF, que lee las mismas imagenes) va como imagen.
        let svg = super::super::svg_de(&h[0], None, false, &tapado);
        assert!(svg.contains("<image"), "el mosaico va incrustado en el SVG");
        // Donde esta el mosaico de bloques, en pixeles de la imagen.
        let (x0, y0) = (h[0].caja.0, h[0].caja.1);
        let e = t.escena.buscar(m).unwrap();
        let k = despues.ancho as f32 / (h[0].caja.2 - h[0].caja.0);
        let px = |v: f32, o: f32| ((v - o) * k) as u32;
        let zona = (
            px(e.x, x0) + 4,
            px(e.y, y0) + 4,
            // La mitad izquierda: la flecha de encima entra por la derecha.
            px(e.x + e.ancho / 2.0, x0),
            px(e.y + e.alto, y0) - 4,
        );
        let en_antes = colores_en(&antes, zona);
        let en_despues = colores_en(&despues, zona);
        // Antes: la banda maciza (un color). Despues: cuadros de varios
        // colores, pero muchos menos que el texto en claro con su suavizado.
        assert!(en_antes <= 2, "antes era la banda: {en_antes}");
        assert!(en_despues > 2, "despues se ve el pixelado: {en_despues}");
        let mut en_claro = escena.clone();
        en_claro
            .elementos
            .retain(|e| !matches!(e.figura, Figura::Mosaico { .. }));
        let claro = super::super::a_imagen(
            &hojas(&en_claro)[0],
            2.0,
            Some(escena.fondo),
            &Lienzo {
                escena: &en_claro,
                seleccion: &[],
                papel: None,
                fotos: &|_| None,
                nombre: String::new(),
            },
        )
        .unwrap();
        let en_claro = colores_en(&claro, zona);
        assert!(
            en_despues * 2 < en_claro,
            "tapa: {en_despues} frente a {en_claro} en claro"
        );
    }

    #[test]
    fn sin_mosaicos_no_hay_nada_que_tapar() {
        let mut escena = Escena::nueva();
        escena.anadir(rect(0, 0.0));
        let lienzo = Lienzo {
            escena: &escena,
            seleccion: &[],
            papel: None,
            fotos: &|_| None,
            nombre: String::new(),
        };
        assert!(tapar(&lienzo).is_none());
    }
}
