//! El presupuesto de recursos, derivado de los hechos y del nivel.

use crate::hechos::Hechos;
use crate::nivel::Nivel;

/// Topes que las caches y los pools reciben POR PARAMETRO. Un tope que no se
/// inyecta no existe: nada lee esta estructura desde un global.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presupuesto {
    /// Copias vivas de la imagen capturada. Un bufer a resolucion reducida
    /// cuenta por su tamano real (media resolucion = un cuarto de copia).
    pub copias_vivas_max: u8,
    pub cache_bytes_max: u64,
    /// Depende solo de los nucleos fisicos, no del nivel: limitar los hilos
    /// de un equipo potente forzado a ligero no lo haria mas fluido.
    pub hilos_trabajo: u32,
    pub ram_reposo_objetivo_bytes: u64,
}

impl Presupuesto {
    pub fn desde(hechos: &Hechos, nivel: Nivel) -> Presupuesto {
        // La cache es el 1% (completo) o el 0,5% (ligero) de la RAM fisica,
        // con tope. Aritmetica entera en por-miles: sin coma flotante.
        let (copias, por_mil, tope_cache, reposo_mb) = match nivel {
            Nivel::Completo => (6, 10u64, 128 * 1024 * 1024u64, 40u64),
            Nivel::Ligero => (3, 5, 16 * 1024 * 1024, 30),
        };
        Presupuesto {
            copias_vivas_max: copias,
            cache_bytes_max: (hechos.ram_fisica_bytes * por_mil / 1000).min(tope_cache),
            hilos_trabajo: hechos.nucleos_fisicos.saturating_sub(1).max(1),
            ram_reposo_objetivo_bytes: reposo_mb * 1024 * 1024,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::hechos::{FL_11_0, GIB, Hechos};
    use crate::nivel::Nivel;

    fn maquina_suelo() -> Hechos {
        Hechos {
            ram_fisica_bytes: 4 * GIB,
            nucleos_fisicos: 2,
            nucleos_logicos: 4,
            vram_dedicada_bytes: 0,
            gpu_es_software: false,
            nivel_caracteristica: FL_11_0,
        }
    }

    fn maquina_potente() -> Hechos {
        Hechos {
            ram_fisica_bytes: 32 * GIB,
            nucleos_fisicos: 8,
            nucleos_logicos: 16,
            vram_dedicada_bytes: 8 * GIB,
            gpu_es_software: false,
            nivel_caracteristica: 0xb100,
        }
    }

    #[test]
    fn en_la_maquina_suelo_un_hilo_tres_copias_y_cache_al_tope_chico() {
        let p = Presupuesto::desde(&maquina_suelo(), Nivel::Ligero);
        assert_eq!(p.hilos_trabajo, 1); // 2 fisicos - 1
        assert_eq!(p.copias_vivas_max, 3);
        // 0,5% de 4 GiB son ~21 MB; gana el tope de 16 MB.
        assert_eq!(p.cache_bytes_max, 16 * 1024 * 1024);
        assert_eq!(p.ram_reposo_objetivo_bytes, 30 * 1024 * 1024);
    }

    #[test]
    fn los_hilos_dependen_de_los_nucleos_y_no_del_nivel() {
        // Un equipo potente forzado a ligero conserva sus nucleos: limitar
        // hilos no lo haria mas fluido, solo mas lento.
        let potente = maquina_potente();
        assert_eq!(Presupuesto::desde(&potente, Nivel::Ligero).hilos_trabajo, 7);
        assert_eq!(
            Presupuesto::desde(&potente, Nivel::Completo).hilos_trabajo,
            7
        );
    }

    #[test]
    fn la_cache_escala_con_la_ram_hasta_su_tope() {
        // 2 GiB en completo: el 1% (~21 MB) queda por debajo del tope de
        // 128 MB, asi que manda la RAM. Si la formula ignorase la RAM y
        // devolviera siempre el tope, este test falla.
        let pequena = Hechos {
            ram_fisica_bytes: 2 * GIB,
            ..maquina_potente()
        };
        assert_eq!(
            Presupuesto::desde(&pequena, Nivel::Completo).cache_bytes_max,
            2 * GIB * 10 / 1000
        );
        // 64 GiB en completo: el 1% (655 MB) pierde contra el tope.
        let grande = Hechos {
            ram_fisica_bytes: 64 * GIB,
            ..maquina_potente()
        };
        assert_eq!(
            Presupuesto::desde(&grande, Nivel::Completo).cache_bytes_max,
            128 * 1024 * 1024
        );
    }

    #[test]
    fn un_solo_nucleo_no_deja_cero_hilos() {
        let uno = Hechos {
            nucleos_fisicos: 1,
            nucleos_logicos: 1,
            ..maquina_suelo()
        };
        assert_eq!(Presupuesto::desde(&uno, Nivel::Ligero).hilos_trabajo, 1);
    }
}
