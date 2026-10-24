//! La decision del nivel: una funcion pura de los hechos.
//!
//! Se decide UNA vez al arrancar y no cambia en caliente: un nivel que muta a
//! mitad de sesion crearia rutas mixtas imposibles de reproducir y de medir.
//! Un cambio del ajuste se aplica al reiniciar.

use crate::hechos::{FL_11_0, GIB, Hechos};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nivel {
    Completo,
    Ligero,
}

/// Lo que pide el fichero de ajustes. `Auto` deja decidir a los hechos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preferencia {
    Auto,
    Forzado(Nivel),
}

/// Por que se decidio lo que se decidio. Va al registro: es lo que permite
/// diagnosticar un "me va lento" sin telemetria y sin adivinar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Razon {
    PocaRam,
    PocosNucleos,
    GpuPorSoftware,
    NivelCaracteristicaBajo,
    GraficosIntegradosConPocaRam,
    ForzadoPorAjuste,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub nivel: Nivel,
    pub razones: Vec<Razon>,
}

/// Umbrales provisionales del diseno (4.3): se confirman midiendo en la
/// maquina suelo. Cae a `Ligero` si se cumple CUALQUIERA.
pub fn decidir(hechos: &Hechos, preferencia: Preferencia) -> Decision {
    if let Preferencia::Forzado(nivel) = preferencia {
        return Decision {
            nivel,
            razones: vec![Razon::ForzadoPorAjuste],
        };
    }
    let mut razones = Vec::new();
    if hechos.ram_fisica_bytes < 6 * GIB {
        razones.push(Razon::PocaRam);
    }
    if hechos.nucleos_fisicos <= 2 {
        razones.push(Razon::PocosNucleos);
    }
    if hechos.gpu_es_software {
        razones.push(Razon::GpuPorSoftware);
    }
    if hechos.nivel_caracteristica < FL_11_0 {
        razones.push(Razon::NivelCaracteristicaBajo);
    }
    if hechos.vram_dedicada_bytes == 0 && hechos.ram_fisica_bytes < 8 * GIB {
        razones.push(Razon::GraficosIntegradosConPocaRam);
    }
    let nivel = if razones.is_empty() {
        Nivel::Completo
    } else {
        Nivel::Ligero
    };
    Decision { nivel, razones }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::hechos::{FL_11_0, GIB, Hechos};

    /// La maquina suelo del diseno: Core i3 de 3a generacion, 4 GB, HD 4000.
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
            nivel_caracteristica: 0xb100, // 11_1
        }
    }

    #[test]
    fn la_maquina_suelo_cae_a_ligero_por_dos_razones_independientes() {
        let d = decidir(&maquina_suelo(), Preferencia::Auto);
        assert_eq!(d.nivel, Nivel::Ligero);
        // Dos razones distintas a proposito: si manana se afina un umbral,
        // la maquina suelo sigue cayendo por la otra y este test no se rompe.
        assert!(
            d.razones.contains(&Razon::PocaRam),
            "razones: {:?}",
            d.razones
        );
        assert!(
            d.razones.contains(&Razon::PocosNucleos),
            "razones: {:?}",
            d.razones
        );
    }

    #[test]
    fn una_maquina_potente_queda_en_completo_sin_razones() {
        // El caso negativo de la degradacion: si esto fallara, todo el mundo
        // estaria en ligero y ningun otro test lo notaria.
        let d = decidir(&maquina_potente(), Preferencia::Auto);
        assert_eq!(d.nivel, Nivel::Completo);
        assert!(d.razones.is_empty(), "razones: {:?}", d.razones);
    }

    #[test]
    fn los_umbrales_son_estrictos_en_la_frontera() {
        // Exactamente 6 GiB y 3 nucleos fisicos ya NO caen: los umbrales son
        // "< 6 GiB" y "<= 2", no "<=" y "<". Una implementacion ingenua con
        // el comparador cambiado falla aqui.
        let h = Hechos {
            ram_fisica_bytes: 6 * GIB,
            nucleos_fisicos: 3,
            nucleos_logicos: 6,
            vram_dedicada_bytes: 2 * GIB,
            gpu_es_software: false,
            nivel_caracteristica: FL_11_0,
        };
        let d = decidir(&h, Preferencia::Auto);
        assert_eq!(d.nivel, Nivel::Completo, "razones: {:?}", d.razones);
    }

    #[test]
    fn warp_cae_a_ligero_aunque_sobren_ram_y_nucleos() {
        let h = Hechos {
            gpu_es_software: true,
            ..maquina_potente()
        };
        let d = decidir(&h, Preferencia::Auto);
        assert_eq!(d.nivel, Nivel::Ligero);
        assert_eq!(d.razones, vec![Razon::GpuPorSoftware]);
    }

    #[test]
    fn integrada_con_poca_ram_cae_pero_integrada_con_mucha_no() {
        let justa = Hechos {
            ram_fisica_bytes: 6 * GIB,
            nucleos_fisicos: 4,
            nucleos_logicos: 8,
            vram_dedicada_bytes: 0,
            gpu_es_software: false,
            nivel_caracteristica: FL_11_0,
        };
        let d = decidir(&justa, Preferencia::Auto);
        assert_eq!(d.nivel, Nivel::Ligero);
        assert_eq!(d.razones, vec![Razon::GraficosIntegradosConPocaRam]);
        // Con 8 GiB la misma integrada ya no arrastra: es el caso negativo.
        let holgada = Hechos {
            ram_fisica_bytes: 8 * GIB,
            ..justa
        };
        assert_eq!(decidir(&holgada, Preferencia::Auto).nivel, Nivel::Completo);
    }

    #[test]
    fn forzar_un_nivel_gana_a_los_hechos_y_queda_registrado() {
        // Forzar ligero en una maquina potente es la via que mantiene viva
        // la ruta ligera: sin ella solo se ejecutaria en hardware flojo.
        let d = decidir(&maquina_potente(), Preferencia::Forzado(Nivel::Ligero));
        assert_eq!(d.nivel, Nivel::Ligero);
        assert_eq!(d.razones, vec![Razon::ForzadoPorAjuste]);
        // Y al reves: completo forzado en la maquina suelo. Bajo su propia
        // responsabilidad, pero es su maquina.
        let d = decidir(&maquina_suelo(), Preferencia::Forzado(Nivel::Completo));
        assert_eq!(d.nivel, Nivel::Completo);
        assert_eq!(d.razones, vec![Razon::ForzadoPorAjuste]);
    }
}
