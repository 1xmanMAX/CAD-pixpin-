//! El fichero `.pixpin2d`.
//!
//! JSON, con la misma disciplina que el resto del proyecto: `serde(default)`
//! en todo, claves desconocidas ignoradas, y escritura por temporal + rename.
//! Esa ultima parte no es adorno — es lo que hace que un corte de luz a mitad
//! de guardar deje el fichero ANTERIOR intacto en vez de uno a medias que no
//! abre. Es la leccion que el proyecto Android aprendio a golpes.
//!
//! Al guardar se compacta: los elementos borrados no llegan al disco. Deshacer
//! deja de funcionar sobre lo guardado, y eso es lo correcto — una sesion
//! nueva no hereda el "deshacer" de otra.

use std::fs;
use std::path::{Path, PathBuf};

use crate::escena::Escena;

#[derive(Debug, thiserror::Error)]
pub enum ErrorFormato {
    #[error("no se pudo acceder a {1}: {0}")]
    Io(#[source] std::io::Error, PathBuf),
    #[error("el fichero de dibujo tiene un error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Extension de los dibujos de PixPin Max.
pub const EXTENSION: &str = "pixpin2d";

/// Lee un dibujo. Un fichero que no existe **no** es un error: es un dibujo
/// vacio, que es lo que espera quien abre un pin sin anotaciones todavia.
pub fn cargar(ruta: &Path) -> Result<Escena, ErrorFormato> {
    match fs::read_to_string(ruta) {
        Ok(texto) => Ok(serde_json::from_str(&texto)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Escena::nueva()),
        Err(e) => Err(ErrorFormato::Io(e, ruta.to_path_buf())),
    }
}

/// Escribe el dibujo, compactando primero. Temporal + rename.
pub fn guardar(ruta: &Path, escena: &Escena) -> Result<(), ErrorFormato> {
    let mut copia = escena.clone();
    copia.compactar();

    if let Some(padre) = ruta.parent() {
        if !padre.as_os_str().is_empty() {
            fs::create_dir_all(padre).map_err(|e| ErrorFormato::Io(e, padre.to_path_buf()))?;
        }
    }

    let texto = serde_json::to_string_pretty(&copia)?;
    let temporal = ruta.with_extension(format!("{EXTENSION}.tmp"));
    fs::write(&temporal, texto).map_err(|e| ErrorFormato::Io(e, temporal.clone()))?;
    fs::rename(&temporal, ruta).map_err(|e| ErrorFormato::Io(e, ruta.to_path_buf()))?;
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
    use crate::vector::Punto2;

    fn dir(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-motor2d-{etiqueta}"));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn trazo() -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(1.0, 2.0), Punto2::nuevo(30.0, 40.0)],
                presiones: vec![],
                opciones: None,
            },
            x: 0.0,
            y: 0.0,
            ancho: 0.0,
            alto: 0.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.1, 0.2, 0.3),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 3.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 12345,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    #[test]
    fn un_dibujo_va_y_vuelve_intacto() {
        let d = dir("ida-vuelta");
        let ruta = d.join("prueba.pixpin2d");
        let mut e = Escena::nueva();
        e.anadir(trazo());

        guardar(&ruta, &e).unwrap();
        let vuelta = cargar(&ruta).unwrap();

        assert_eq!(vuelta.elementos.len(), 1);
        assert_eq!(vuelta.elementos[0].semilla, 12345, "la semilla es sagrada");
        assert_eq!(vuelta.elementos[0].figura, e.elementos[0].figura);
    }

    #[test]
    fn un_fichero_que_no_existe_es_un_dibujo_vacio() {
        // Abrir un pin recien creado no puede dar error: todavia no ha
        // anotado nadie.
        let e = cargar(Path::new("Z:/no/existe/nada.pixpin2d")).unwrap();
        assert_eq!(e.elementos.len(), 0);
        assert_eq!(e.siguiente_id, 1);
    }

    #[test]
    fn guardar_compacta_los_borrados() {
        let d = dir("compactar");
        let ruta = d.join("c.pixpin2d");
        let mut e = Escena::nueva();
        e.anadir(trazo());
        let dos = e.anadir(trazo());
        e.borrar(dos);

        guardar(&ruta, &e).unwrap();

        assert_eq!(e.elementos.len(), 2, "guardar no toca la escena en memoria");
        assert_eq!(
            cargar(&ruta).unwrap().elementos.len(),
            1,
            "pero al disco solo va lo que se ve"
        );
    }

    #[test]
    fn un_fichero_de_una_version_futura_sigue_abriendo() {
        // La regla de compatibilidad del proyecto: claves desconocidas se
        // ignoran, campos ausentes toman su valor por defecto.
        let d = dir("futuro");
        let ruta = d.join("f.pixpin2d");
        fs::write(
            &ruta,
            r#"{
                "version": 99,
                "siguiente_id": 5,
                "capas_del_futuro": ["algo"],
                "elementos": [{
                    "id": 4,
                    "figura": { "tipo": "elipse" },
                    "x": 0.0, "y": 0.0, "ancho": 10.0, "alto": 10.0,
                    "trazo": { "r": 0, "g": 0, "b": 0, "a": 1 },
                    "grosor": 1.0
                }]
            }"#,
        )
        .unwrap();

        let e = cargar(&ruta).unwrap();

        assert_eq!(e.elementos.len(), 1);
        assert_eq!(e.elementos[0].semilla, 1, "sin semilla se usa 1, no 0");
        assert_eq!(e.siguiente_id, 5);
    }

    #[test]
    fn un_json_roto_da_error_en_vez_de_un_dibujo_a_medias() {
        // Caso negativo: tragarse un fichero corrupto y devolver una escena
        // vacia perderia el trabajo del usuario en silencio.
        let d = dir("roto");
        let ruta = d.join("r.pixpin2d");
        fs::write(&ruta, "{ esto no es json ").unwrap();
        assert!(cargar(&ruta).is_err());
    }

    #[test]
    fn guardar_no_deja_ficheros_temporales_por_medio() {
        let d = dir("temporal");
        let ruta = d.join("t.pixpin2d");
        guardar(&ruta, &Escena::nueva()).unwrap();
        let sobrantes: Vec<_> = fs::read_dir(&d)
            .unwrap()
            .filter_map(|x| x.ok())
            .filter(|x| x.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(sobrantes.is_empty(), "quedo un temporal: {sobrantes:?}");
    }

    #[test]
    fn guardar_dos_veces_deja_el_ultimo_dibujo_y_no_los_dos() {
        let d = dir("sobrescribir");
        let ruta = d.join("s.pixpin2d");
        let mut e = Escena::nueva();
        e.anadir(trazo());
        guardar(&ruta, &e).unwrap();
        e.anadir(trazo());
        guardar(&ruta, &e).unwrap();
        assert_eq!(cargar(&ruta).unwrap().elementos.len(), 2);
    }
}
