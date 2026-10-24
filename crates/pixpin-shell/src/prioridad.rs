//! Bajar la prioridad de un hilo o del proceso entero, congelar un proceso,
//! y cuanta memoria ha llegado a usar el proceso o le queda al sistema. Lo
//! usa el compresor de PDF (`apps/pixpin/src/aligerar.rs` y el ejecutable
//! `apps/pixpin-aligerar`), que trabaja de fondo y no puede quitarle CPU a lo
//! que el usuario tiene delante.
//!
//! **Por debajo de lo normal, y no el «modo de fondo» de Windows**
//! (`PROCESS_MODE_BACKGROUND_BEGIN`, el `THREAD_MODE_BACKGROUND_BEGIN` del
//! movil). Medido el 2026-09-24 con pdfsqueeze sobre un PDF de 6 MB en un
//! equipo con otras compilaciones en marcha: en modo de fondo, 78 s y un pico
//! de 33 MB; por debajo de lo normal, 3,5 s y 329 MB. El modo de fondo baja
//! tambien la prioridad de la MEMORIA, Windows le recorta el conjunto de
//! trabajo sin parar y el proceso se pasa el rato trayendo paginas del disco:
//! veinte veces mas lento, y el disco ocupado, que es justo lo que no se
//! queria. Por debajo de lo normal solo cede la CPU a lo que corre a
//! prioridad normal —la ventana, el lapiz—, que es lo que se buscaba.
//!
//! Aqui y no en la aplicacion porque la aplicacion prohibe `unsafe`.

use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{
    BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, GetCurrentThread, SetPriorityClass,
    SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
};

/// Pone el proceso actual por debajo de lo normal. `false` si Windows no deja.
pub fn proceso_por_debajo_de_lo_normal() -> bool {
    // SAFETY: el seudo-manejador del proceso actual no se cierra ni se
    // comparte; solo se cambia su prioridad.
    unsafe { SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS).is_ok() }
}

/// Lo mismo para el hilo que llama.
pub fn hilo_por_debajo_de_lo_normal() -> bool {
    // SAFETY: el seudo-manejador del hilo actual; solo cambia su prioridad.
    unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL).is_ok() }
}

/// El pico de memoria del proceso actual (working set), en bytes.
pub fn memoria_pico() -> Option<usize> {
    let mut c = PROCESS_MEMORY_COUNTERS::default();
    // SAFETY: `c` es nuestro y se le pasa su tamano exacto.
    unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut c,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .ok()?;
    }
    Some(c.PeakWorkingSetSize)
}

/// La memoria del sistema: `(total, disponible)` en bytes. Lo que usa el
/// compresor de PDF en un equipo justo de RAM para no arrancar (o pausar) un
/// trabajo cuando Windows ya va apurado.
pub fn memoria_del_sistema() -> Option<(u64, u64)> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut m = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `m` es nuestro y lleva su tamano en `dwLength`, como pide la API.
    unsafe { GlobalMemoryStatusEx(&mut m).ok()? };
    Some((m.ullTotalPhys, m.ullAvailPhys))
}

/// **Congela o descongela un proceso** (todos sus hilos). Es la pausa del
/// compresor de PDF cuando la memoria del sistema se acaba: congelado no pide
/// mas, y Windows puede sacar a disco lo que ya tiene. `false` si no se pudo
/// con ningun hilo.
pub fn congelar(pid: u32, congelado: bool) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows::Win32::System::Threading::{
        OpenThread, ResumeThread, SuspendThread, THREAD_SUSPEND_RESUME,
    };
    let mut alguno = false;
    // SAFETY: la foto de hilos y cada manejador abierto se cierran aqui
    // mismo; `e` es nuestro y lleva su tamano.
    unsafe {
        let Ok(foto) = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) else {
            return false;
        };
        let mut e = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        let mut hay = Thread32First(foto, &mut e).is_ok();
        while hay {
            if e.th32OwnerProcessID == pid
                && let Ok(h) = OpenThread(THREAD_SUSPEND_RESUME, false, e.th32ThreadID)
            {
                let r = if congelado {
                    SuspendThread(h)
                } else {
                    ResumeThread(h)
                };
                alguno |= r != u32::MAX;
                let _ = CloseHandle(h);
            }
            hay = Thread32Next(foto, &mut e).is_ok();
        }
        let _ = CloseHandle(foto);
    }
    alguno
}

#[cfg(test)]
mod pruebas {
    #[test]
    fn el_pico_de_memoria_de_este_proceso_se_sabe_y_no_es_cero() {
        assert!(super::memoria_pico().is_some_and(|b| b > 0));
    }

    #[test]
    fn un_hilo_se_puede_poner_por_debajo_de_lo_normal() {
        // En un hilo propio: el de la prueba lo comparten otras.
        assert!(
            std::thread::spawn(super::hilo_por_debajo_de_lo_normal)
                .join()
                .unwrap()
        );
    }

    #[test]
    fn la_memoria_del_sistema_se_sabe_y_la_disponible_no_pasa_del_total() {
        let (total, libre) = super::memoria_del_sistema().unwrap();
        assert!(total > 0 && libre <= total);
    }

    #[test]
    fn un_proceso_se_congela_y_se_descongela() {
        let mut hijo = std::process::Command::new("cmd")
            .args(["/c", "ping -n 4 127.0.0.1 >nul"])
            .spawn()
            .unwrap();
        assert!(super::congelar(hijo.id(), true));
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(hijo.try_wait().unwrap().is_none(), "congelado no acaba");
        assert!(super::congelar(hijo.id(), false));
        let _ = hijo.kill();
        let _ = hijo.wait();
        // Caso negativo: un proceso que no existe no tiene hilos que congelar.
        assert!(!super::congelar(u32::MAX - 7, true));
    }
}
