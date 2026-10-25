//! Pruebas con ficheros de verdad: los Revit sinteticos de rvt-rs (corpus
//! tier1, Apache-2.0), su familia vacia (MIT, RevitTestFramework), sus IFC
//! sinteticos y un IFC hecho a mano con Psets, Qtos, capas y clasificacion.

use std::path::{Path, PathBuf};

use cad_bim_datos::exportar::{self, Celda};
use cad_bim_datos::{Formato, ModeloBim, Opciones, TipoConjunto, Valor, leer, metrado, planta};

fn datos(n: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/datos")
        .join(n)
}

fn abrir(n: &str) -> ModeloBim {
    leer(&datos(n), Opciones::default(), &mut |_| {}).unwrap_or_else(|e| panic!("{n}: {e}"))
}

fn cuantos(m: &ModeloBim, categoria: &str) -> usize {
    m.categorias
        .iter()
        .find(|c| c.nombre == categoria)
        .map_or(0, |c| c.elementos)
}

fn temporal(n: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cad-bim-datos-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(n)
}

#[test]
fn revit_2023_proyecto_real() {
    // Revit_IFC5_Einhoven.rvt (magnetar-io/revit-test-datasets, MIT).
    let m = abrir("Revit_IFC5_Einhoven.rvt");
    assert_eq!(m.archivo.formato, Formato::Revit);
    assert_eq!(m.archivo.version.as_deref(), Some("Revit 2023"));
    assert_eq!(m.archivo.compilacion.as_deref(), Some("20230828_1515(x64)"));
    assert_eq!(m.archivo.trabajo_compartido.as_deref(), Some("Not enabled"));
    assert!(m.archivo.datos.iter().any(|d| d.clave == "Worksharing"));
    assert!(
        m.archivo
            .vista_previa
            .as_ref()
            .is_some_and(|p| p.starts_with(b"\x89PNG"))
    );
    let px = cad_bim_datos::png_a_rgba(m.archivo.vista_previa.as_ref().unwrap()).unwrap();
    assert_eq!(px.rgba.len(), (px.ancho * px.alto * 4) as usize);
    assert!(cad_bim_datos::png_a_rgba(b"no es png").is_none());
    assert_eq!(m.niveles.len(), 4, "{:?}", m.niveles);
    assert_eq!(m.niveles[0].nombre, "Level 1");
    assert_eq!(m.elementos.len(), 37);
    assert_eq!(cuantos(&m, "Muros"), 26, "{:?}", m.categorias);
    assert_eq!(cuantos(&m, "Ventanas"), 2, "{:?}", m.categorias);
    assert_eq!(m.materiales.len(), 36);
    // Cada elemento con su ElementId en su conjunto de identidad, y familia y tipo.
    for e in &m.elementos {
        let id = e.propiedad(Some("Identidad"), "ElementId").unwrap();
        assert_eq!(id.valor, Valor::Texto(e.id.clone()));
    }
    let con_global = m
        .elementos
        .iter()
        .filter(|e| e.global_id.as_deref().is_some_and(|g| g.len() == 22));
    assert!(con_global.count() >= 30);
    let muro = m.elementos.iter().find(|e| e.id == "2808").unwrap();
    assert_eq!(
        (muro.familia.as_str(), muro.tipo.as_str()),
        ("Basic Wall", "Wall 1")
    );
    let c = muro.caja.unwrap();
    assert!((c.medidas()[2] - 3.0).abs() < 0.01, "{c:?}");
    assert!(muro.propiedad(None, "BodySource").is_some());
    assert!(!m.tipos.is_empty());
}

#[test]
fn revit_sinteticos_sin_elementos() {
    // Los Revit sinteticos de rvt-rs no traen registros de elementos de
    // verdad: se leen sus datos y se dice que no hay elementos.
    for (n, v) in [
        ("architectural-2024.rvt", "Revit 2024"),
        ("structural-2023.rvt", "Revit 2023"),
        ("mep-2024.rvt", "Revit 2024"),
    ] {
        let m = abrir(n);
        assert_eq!(m.archivo.formato, Formato::Revit, "{n}");
        assert_eq!(m.archivo.version.as_deref(), Some(v));
        assert_eq!(m.archivo.compilacion.as_deref(), Some("20230101_0000(x64)"));
        assert!(m.elementos.is_empty());
        assert!(
            m.avisos.iter().any(|a| a.contains("no encontro elementos")),
            "{:?}",
            m.avisos
        );
        assert_eq!(m.proyecto.nombre.as_deref(), Some("synthetic fixture"));
    }
}

/// Core Interior (Revit 2024, 33 MB, magnetar-io/revit-test-datasets):
/// `RVT_CORE_INTERIOR=ruta cargo test -p cad-bim-datos -- --ignored`.
#[test]
#[ignore = "pide 2024_Core_Interior.rvt (33 MB) en RVT_CORE_INTERIOR"]
fn revit_2024_core_interior() {
    let Ok(r) = std::env::var("RVT_CORE_INTERIOR") else {
        return;
    };
    let m = leer(Path::new(&r), Opciones::default(), &mut |_| {}).unwrap();
    assert_eq!(m.niveles.len(), 15);
    assert_eq!(cuantos(&m, "Muros"), 360);
    assert_eq!(cuantos(&m, "Puertas"), 132);
    assert_eq!(cuantos(&m, "Ventanas"), 6);
    assert_eq!(cuantos(&m, "Columnas"), 256);
    assert_eq!(cuantos(&m, "Losas"), 80);
    assert_eq!(m.espacios.len(), 116);
    assert!(m.espacios.iter().all(|s| !s.numero.is_empty()));
    assert!(m.espacios.iter().filter(|s| s.area.is_some()).count() > 100);
}

#[test]
fn familia_de_revit() {
    let m = abrir("empty.rfa");
    assert_eq!(m.archivo.formato, Formato::FamiliaRevit);
    assert!(m.elementos.is_empty());
    assert!(m.archivo.version.as_deref().unwrap().starts_with("Revit "));
    assert!(m.avisos.iter().any(|a| a.contains("familia")));
}

#[test]
fn fichero_roto() {
    let r = temporal("roto.rvt");
    std::fs::write(&r, b"esto no es un revit").unwrap();
    assert!(leer(&r, Opciones::default(), &mut |_| {}).is_err());
    let r = temporal("roto.ifc");
    std::fs::write(&r, b"\x00\x01\x02 basura").unwrap();
    assert!(leer(&r, Opciones::default(), &mut |_| {}).is_err());
    // Un IFC cortado a la mitad se lee hasta donde llega.
    let t = std::fs::read(datos("rico.ifc")).unwrap();
    let r = temporal("cortado.ifc");
    std::fs::write(&r, &t[..t.len() / 2]).unwrap();
    assert!(leer(&r, Opciones::default(), &mut |_| {}).is_ok());
}

#[test]
fn ifc_vacio() {
    let r = temporal("vacio.ifc");
    std::fs::write(
        &r,
        "ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n",
    )
    .unwrap();
    let m = leer(&r, Opciones::default(), &mut |_| {}).unwrap();
    assert!(m.elementos.is_empty());
    assert!(!m.avisos.is_empty());
    assert_eq!(m.archivo.version.as_deref(), Some("IFC4"));
}

#[test]
fn ifc_estructura_psets_y_qtos() {
    let m = abrir("rico.ifc");
    assert_eq!(m.archivo.autor.as_deref(), Some("Ana Quispe"));
    assert_eq!(m.archivo.organizacion.as_deref(), Some("Estudio Andino"));
    assert_eq!(m.proyecto.nombre.as_deref(), Some("Edificio Los Andes"));
    assert_eq!(m.proyecto.numero.as_deref(), Some("P-042"));
    assert_eq!(m.proyecto.estado.as_deref(), Some("Construccion"));
    assert_eq!(m.proyecto.cliente.as_deref(), Some("Municipalidad"));
    assert_eq!(m.proyecto.edificio.as_deref(), Some("Bloque A"));
    assert!(
        m.proyecto
            .direccion
            .as_deref()
            .unwrap()
            .contains("Arequipa")
    );
    // Proyecto ▸ Sitio ▸ Edificio ▸ 2 niveles ▸ espacio.
    let clases: Vec<&str> = m.espacial.iter().map(|n| n.clase.as_str()).collect();
    assert_eq!(
        clases,
        [
            "IfcProject",
            "IfcSite",
            "IfcBuilding",
            "IfcBuildingStorey",
            "IfcSpace",
            "IfcBuildingStorey"
        ]
    );
    assert_eq!(m.espacial[4].padre, Some(3));
    assert_eq!(m.niveles.len(), 2);
    assert_eq!(m.niveles[1].cota, Some(3.0), "milimetros a metros");
    assert_eq!(m.niveles[1].edificio.as_deref(), Some("Bloque A"));
    // Sin el hueco.
    assert_eq!(m.elementos.len(), 5);
    assert_eq!(cuantos(&m, "Muros"), 2);
    assert_eq!(cuantos(&m, "Habitaciones"), 1);

    let muro = m.elementos.iter().find(|e| e.id == "#20").unwrap();
    assert_eq!(muro.familia, "Muro basico");
    assert_eq!(muro.tipo, "Ladrillo 150");
    assert_eq!(m.nivel_de(muro), Some("Piso 1"));
    assert_eq!(muro.materiales, ["Tarrajeo", "Ladrillo King Kong"]);
    let p = muro
        .propiedad(Some("Pset_WallCommon"), "IsExternal")
        .unwrap();
    assert_eq!(p.valor, Valor::Logico(Some(true)));
    let p = muro
        .propiedad(Some("Pset_WallCommon"), "FireRating")
        .unwrap();
    assert_eq!(p.valor, Valor::Texto("F60".into()));
    let p = muro
        .propiedad(Some("Qto_WallBaseQuantities"), "Length")
        .unwrap();
    assert_eq!(p.unidad.as_deref(), Some("mm"));
    assert!((p.numero_si().unwrap() - 4.5).abs() < 1e-9);
    let p = muro
        .propiedad(Some("Qto_WallBaseQuantities"), "NetVolume")
        .unwrap();
    assert_eq!(p.unidad.as_deref(), Some("m³"));
    // Del tipo.
    let p = muro
        .propiedad(Some("Pset_WallTypeCommon"), "Width")
        .unwrap();
    assert!((p.numero_si().unwrap() - 0.15).abs() < 1e-9);
    let c = muro
        .conjuntos
        .iter()
        .find(|c| c.nombre == "Pset_WallTypeCommon")
        .unwrap();
    assert_eq!(c.tipo, TipoConjunto::Tipo);
    // Capas y clasificacion.
    let p = muro
        .propiedad(Some("Capas de material"), "Espesor total")
        .unwrap();
    assert!((p.numero_si().unwrap() - 0.15).abs() < 1e-9);
    let p = muro
        .propiedad(Some("Clasificacion"), "Uniclass 2015")
        .unwrap();
    assert!(p.valor.texto().starts_with("Ss_25_10_30"));
    // El otro muro, en el piso 2.
    let otro = m.elementos.iter().find(|e| e.id == "#21").unwrap();
    assert_eq!(m.nivel_de(otro), Some("Piso 2"));
    // El material con comilla escapada.
    let losa = m.elementos.iter().find(|e| e.id == "#23").unwrap();
    assert_eq!(losa.materiales, ["Concreto f'c=210"]);
    // El espacio, en su planta, con su area y volumen.
    assert_eq!(m.espacios.len(), 1);
    let s = &m.espacios[0];
    assert_eq!(
        (s.numero.as_str(), s.nombre.as_str()),
        ("101", "Sala de estar")
    );
    assert_eq!(s.area, Some(24.5));
    assert_eq!(s.volumen, Some(66.15));
    assert_eq!(m.nivel_de(&m.elementos[s.elemento]), Some("Piso 1"));
    // Tipos y materiales.
    assert_eq!(m.tipos.len(), 1);
    assert_eq!(m.tipos[0].elementos, 2);
    assert_eq!(m.materiales.len(), 3);
    // La busqueda encuentra por valor de propiedad.
    assert_eq!(m.buscar("f60").len(), 2);
    assert_eq!(m.buscar("SALA").len(), 1);
}

#[test]
fn ifc_de_rvt_rs_con_cajas() {
    let m = abrir("synthetic-project.ifc");
    assert_eq!(m.niveles.len(), 3);
    assert_eq!(cuantos(&m, "Muros"), 4, "{:?}", m.categorias);
    assert!(cuantos(&m, "Ventanas") == 2 && cuantos(&m, "Puertas") == 1);
    assert!(m.elementos.iter().any(|e| {
        !e.conjuntos
            .iter()
            .all(|c| c.tipo == TipoConjunto::Atributos)
    }));
    let con_caja: Vec<_> = m.elementos.iter().filter(|e| e.caja.is_some()).collect();
    assert!(!con_caja.is_empty(), "las cajas de la geometria");
    // Los muros del segundo piso, arriba de 3 m (Z hacia arriba).
    for e in &con_caja {
        let c = e.caja.unwrap();
        assert!(c.max[2] >= c.min[2]);
        if m.nivel_de(e) == Some("Second Floor") && e.categoria == "Muros" {
            assert!(c.min[2] > 2.9, "{e:?}");
        }
    }
    let m = abrir("synthetic-structural.ifc");
    assert!(!m.elementos.is_empty());
}

#[test]
fn exportaciones() {
    let m = abrir("rico.ifc");
    // CSV: una fila por elemento y las propiedades como columnas.
    let t = exportar::tabla(&m, &(0..m.elementos.len()).collect::<Vec<_>>(), true);
    assert!(t.columnas.iter().any(|c| c == "Pset_WallCommon.FireRating"));
    assert!(
        t.columnas
            .iter()
            .any(|c| c == "Qto_WallBaseQuantities.Length (mm)")
    );
    let csv = exportar::csv_todo(&m);
    let lineas: Vec<&str> = csv.trim_end().split("\r\n").collect();
    assert_eq!(lineas.len(), m.elementos.len() + 1);
    let base = temporal("rico.csv");
    let hechos = exportar::csv_por_categoria(&m, &base).unwrap();
    assert_eq!(hechos.len(), m.categorias.len());
    let muros = hechos
        .iter()
        .find(|p| p.to_string_lossy().ends_with("Muros.csv"))
        .unwrap();
    let t = std::fs::read_to_string(muros).unwrap();
    assert_eq!(t.trim_end().split("\r\n").count(), 3);
    // JSON: valido y de vuelta igual.
    let j = exportar::json(&m);
    let v: ModeloBim = serde_json::from_str(&j).unwrap();
    assert_eq!(v.elementos.len(), m.elementos.len());
    assert_eq!(v.elementos[0].conjuntos, m.elementos[0].conjuntos);
    // XLSX: un zip con una hoja por categoria (y las de proyecto).
    let x = exportar::xlsx(&m).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(x)).unwrap();
    let hojas = (0..z.len())
        .filter(|&i| {
            z.by_index(i)
                .unwrap()
                .name()
                .starts_with("xl/worksheets/sheet")
        })
        .count();
    assert_eq!(hojas, m.categorias.len() + 4);
    let mut libro = String::new();
    std::io::Read::read_to_string(&mut z.by_name("xl/workbook.xml").unwrap(), &mut libro).unwrap();
    assert!(libro.contains("name=\"Muros\"") && libro.contains("name=\"Proyecto\""));
    // Por extension.
    for ext in ["csv", "json", "xlsx"] {
        let r = temporal(&format!("todo.{ext}"));
        exportar::exportar(&m, &r).unwrap();
        assert!(std::fs::metadata(&r).unwrap().len() > 100);
    }
}

#[test]
fn tabla_ordenada() {
    let m = abrir("rico.ifc");
    let mut t = exportar::tabla(&m, &(0..m.elementos.len()).collect::<Vec<_>>(), false);
    let col = t.columnas.iter().position(|c| c == "Nombre").unwrap();
    t.ordenar(col, true);
    let nombres: Vec<String> = t.filas.iter().map(|f| f[col].texto()).collect();
    let mut ordenados = nombres.clone();
    ordenados.sort_by(|a, b| exportar::orden_natural(a, b));
    assert_eq!(nombres, ordenados);
    assert_eq!(t.filas.len(), t.elementos.len());
    assert!(Celda::Numero(2.0).comparar(&Celda::Numero(10.0)).is_lt());
}

#[test]
fn metrado_con_datos() {
    let m = abrir("rico.ifc");
    let f = metrado::por_tipo(&m);
    let muro = f
        .iter()
        .find(|f| f.categoria == "Muros" && f.nivel == "Piso 1")
        .unwrap();
    assert_eq!(muro.cantidad, 1);
    assert!((muro.longitud - 4.5).abs() < 1e-9);
    assert!((muro.volumen - 1.8225).abs() < 1e-9);
    assert!(muro.materiales.contains("Ladrillo King Kong"));
    assert!(f.iter().all(|f| f.categoria != "Habitaciones"));
    let csv = metrado::csv(&f);
    assert_eq!(csv.trim_end().split("\r\n").count(), f.len() + 1);
}

#[test]
fn revit_exporta_ifc_glb_y_planta() {
    let r = datos("Revit_IFC5_Einhoven.rvt");
    let ifc = exportar::revit_a_ifc(&r).unwrap();
    assert!(ifc.starts_with("ISO-10303-21"));
    // El IFC de vuelta se lee con sus niveles y sus elementos.
    let m = cad_bim_datos::leer_ifc(ifc.as_bytes(), Opciones { cajas: false }).unwrap();
    assert_eq!(m.niveles.len(), 4);
    assert_eq!(cuantos(&m, "Muros"), 26, "{:?}", m.categorias);
    let glb = exportar::revit_a_glb(&r).unwrap();
    assert_eq!(&glb[..4], b"glTF");
    let p = planta::de_revit(&r).unwrap();
    assert_eq!(p.niveles.len(), 4);
    assert!(!p.trazos.is_empty());
    assert!(p.trazos.iter().any(|t| t.capa == "BIM-Muros"));
    assert!(p.trazos.iter().all(|t| t.puntos.len() >= 2));
    assert_eq!(p.buscar_nivel("1"), Some(0));
    assert!(exportar::revit_a_ifc(&datos("rico.ifc")).is_err());
}
