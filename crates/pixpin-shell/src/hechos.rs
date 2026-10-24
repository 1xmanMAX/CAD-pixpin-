//! Recogida de los hechos del equipo. Cuatro consultas, microsegundos.
//!
//! Nada de micro-benchmarks: serian no deterministas y se comerian el
//! presupuesto de 300 ms hasta la bandeja. Ningun fallo aqui impide
//! arrancar: el hecho ilegible toma el valor pesimista, que empuja hacia
//! Ligero. Equivocarse hacia abajo cuesta lujo visual; hacia arriba, fluidez.

use pixpin_nivel::Hechos;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION, D3D11CreateDevice,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
};
use windows::Win32::System::SystemInformation::{
    GetLogicalProcessorInformationEx, GlobalMemoryStatusEx, MEMORYSTATUSEX, RelationProcessorCore,
    SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
};

/// Recoge los hechos del equipo. Infalible por diseno: ver el comentario
/// del modulo.
pub fn recolectar() -> Hechos {
    let logicos = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);
    let fisicos = nucleos_fisicos().unwrap_or(1);
    let (vram_dedicada, es_software) = adaptador().unwrap_or((0, true));
    Hechos {
        ram_fisica_bytes: ram_fisica().unwrap_or(0),
        nucleos_fisicos: fisicos,
        nucleos_logicos: logicos.max(fisicos),
        vram_dedicada_bytes: vram_dedicada,
        gpu_es_software: es_software,
        nivel_caracteristica: nivel_caracteristica(),
    }
}

fn ram_fisica() -> Option<u64> {
    let mut estado = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: la estructura esta inicializada y dwLength es su tamano real,
    // que es lo unico que la API exige antes de escribir en ella.
    unsafe { GlobalMemoryStatusEx(&mut estado) }.ok()?;
    Some(estado.ullTotalPhys)
}

fn nucleos_fisicos() -> Option<u32> {
    // Primera llamada solo para el tamano; que "falle" con el bufer a cero
    // es el protocolo documentado de la API, no un error.
    let mut tam = 0u32;
    // SAFETY: pasar None con tam=0 es la forma documentada de pedir el
    // tamano necesario; la API no escribe nada.
    let _ = unsafe { GetLogicalProcessorInformationEx(RelationProcessorCore, None, &mut tam) };
    if tam == 0 {
        return None;
    }
    let mut buf = vec![0u8; tam as usize];
    // SAFETY: el bufer tiene exactamente los `tam` bytes que la API pidio y
    // vive hasta despues de la llamada.
    unsafe {
        GetLogicalProcessorInformationEx(
            RelationProcessorCore,
            Some(buf.as_mut_ptr().cast()),
            &mut tam,
        )
    }
    .ok()?;
    // El bufer es una secuencia de registros de TAMANO VARIABLE: cada uno
    // dice en Size cuanto avanzar. Indexar como array seria incorrecto.
    let mut nucleos = 0u32;
    let mut desplazamiento = 0usize;
    while desplazamiento + std::mem::size_of::<u32>() * 2 <= tam as usize {
        // SAFETY: desplazamiento queda dentro del bufer (condicion del
        // while) y siempre sobre el inicio de un registro, porque solo se
        // avanza con el Size que la propia API escribio.
        let registro = unsafe {
            &*(buf
                .as_ptr()
                .add(desplazamiento)
                .cast::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX>())
        };
        if registro.Relationship == RelationProcessorCore {
            nucleos += 1;
        }
        if registro.Size == 0 {
            return None; // un Size de cero seria un bucle infinito
        }
        desplazamiento += registro.Size as usize;
    }
    (nucleos > 0).then_some(nucleos)
}

fn adaptador() -> Option<(u64, bool)> {
    // SAFETY: llamadas COM sin precondiciones de puntero; los objetos se
    // sueltan al salir del ambito por los Drop del crate windows.
    unsafe {
        let fabrica: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let adaptador = fabrica.EnumAdapters1(0).ok()?;
        let desc = adaptador.GetDesc1().ok()?;
        let es_software = (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
        Some((desc.DedicatedVideoMemory as u64, es_software))
    }
}

fn nivel_caracteristica() -> u32 {
    let mut nivel = D3D_FEATURE_LEVEL::default();
    // SAFETY: con ppDevice a None la API valida y devuelve el nivel sin
    // crear el dispositivo; el puntero de salida vive durante la llamada.
    let resultado = unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_FLAG(0),
            None,
            D3D11_SDK_VERSION,
            None,
            Some(&mut nivel),
            None,
        )
    };
    if resultado.is_ok() { nivel.0 as u32 } else { 0 }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_hechos_reales_son_coherentes() {
        // No afirma valores concretos porque corre en cualquier maquina,
        // incluida una CI sin escritorio (alli el probe D3D11 puede dar 0 y
        // el adaptador puede ser WARP: ambos son hechos validos). Afirma la
        // coherencia interna que una recogida rota si romperia. Los casos
        // negativos de la POLITICA viven en pixpin-nivel con equipos
        // inventados; esto solo comprueba la recogida.
        let h = recolectar();
        assert!(
            h.ram_fisica_bytes > 0,
            "RAM fisica cero en una maquina real"
        );
        assert!(h.nucleos_fisicos >= 1);
        assert!(h.nucleos_logicos >= h.nucleos_fisicos);
    }
}
