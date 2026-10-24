//! Cache con presupuesto en bytes y expulsion de lo menos usado. Logica pura
//! (sin Direct2D) para poder probarla: la usan las caches de imagenes de
//! `pagina_office` y `hoja_office`.
//!
//! Lo usado en el fotograma actual no se expulsa nunca (aunque eso deje la
//! cache por encima del presupuesto un momento): si no, una pagina con mas
//! imagenes que presupuesto las decodificaria otra vez en cada fotograma.

use std::collections::HashMap;
use std::hash::Hash;

/// Presupuesto por defecto de las imagenes decodificadas (ancho x alto x 4).
pub const PRESUPUESTO_IMAGENES: usize = 256 * 1024 * 1024;

struct Entrada<V> {
    valor: V,
    bytes: usize,
    /// Marca de la ultima vez que se pidio (orden de uso).
    uso: u64,
    /// Fotograma en que se pidio por ultima vez.
    fotograma: u64,
}

pub struct CachePresupuesto<K, V> {
    mapa: HashMap<K, Entrada<V>>,
    presupuesto: usize,
    total: usize,
    reloj: u64,
    fotograma: u64,
}

impl<K: Hash + Eq + Clone, V: Clone> CachePresupuesto<K, V> {
    pub fn nueva(presupuesto: usize) -> Self {
        CachePresupuesto {
            mapa: HashMap::new(),
            presupuesto,
            total: 0,
            reloj: 0,
            fotograma: 1,
        }
    }

    /// Empieza un fotograma nuevo: lo pedido antes ya se puede expulsar.
    pub fn nuevo_fotograma(&mut self) {
        self.fotograma += 1;
    }

    /// `None` si no esta; `Some(valor)` (que puede ser un `None` interno)
    /// si esta, y lo marca como usado ahora.
    pub fn obtener(&mut self, k: &K) -> Option<V> {
        self.reloj += 1;
        let (reloj, fot) = (self.reloj, self.fotograma);
        self.mapa.get_mut(k).map(|e| {
            e.uso = reloj;
            e.fotograma = fot;
            e.valor.clone()
        })
    }

    /// Guarda `valor` (que pesa `bytes`) y expulsa lo menos usado hasta caber.
    pub fn insertar(&mut self, k: K, valor: V, bytes: usize) {
        self.reloj += 1;
        if let Some(viejo) = self.mapa.remove(&k) {
            self.total -= viejo.bytes;
        }
        self.mapa.insert(
            k,
            Entrada {
                valor,
                bytes,
                uso: self.reloj,
                fotograma: self.fotograma,
            },
        );
        self.total += bytes;
        while self.total > self.presupuesto {
            let fot = self.fotograma;
            let victima = self
                .mapa
                .iter()
                .filter(|(_, e)| e.fotograma != fot && e.bytes > 0)
                .min_by_key(|(_, e)| e.uso)
                .map(|(k, _)| k.clone());
            let Some(v) = victima else { break };
            if let Some(e) = self.mapa.remove(&v) {
                self.total -= e.bytes;
            }
        }
    }

    pub fn vaciar(&mut self) {
        self.mapa.clear();
        self.total = 0;
    }

    pub fn bytes(&self) -> usize {
        self.total
    }

    pub fn len(&self) -> usize {
        self.mapa.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mapa.is_empty()
    }
}

impl<K: Hash + Eq + Clone, V: Clone> Default for CachePresupuesto<K, V> {
    fn default() -> Self {
        Self::nueva(PRESUPUESTO_IMAGENES)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn expulsa_lo_menos_usado() {
        let mut c: CachePresupuesto<&str, u8> = CachePresupuesto::nueva(100);
        c.insertar("a", 1, 40);
        c.insertar("b", 2, 40);
        c.nuevo_fotograma();
        assert_eq!(c.obtener(&"a"), Some(1)); // a pasa a ser lo mas reciente
        c.insertar("c", 3, 40); // 120 > 100: sale b
        assert_eq!(c.obtener(&"b"), None);
        assert_eq!(c.obtener(&"a"), Some(1));
        assert_eq!(c.obtener(&"c"), Some(3));
        assert_eq!(c.bytes(), 80);
    }

    #[test]
    fn lo_del_fotograma_actual_no_se_expulsa() {
        let mut c: CachePresupuesto<u32, u32> = CachePresupuesto::nueva(100);
        for i in 0..5 {
            c.insertar(i, i, 60); // todo en el mismo fotograma
        }
        assert_eq!(c.len(), 5);
        assert!(c.bytes() > 100);
        c.nuevo_fotograma();
        c.insertar(9, 9, 60); // ahora lo anterior si sale, menos lo nuevo
        assert_eq!(c.len(), 1);
        assert_eq!(c.obtener(&9), Some(9));
    }

    #[test]
    fn reinsertar_no_duplica_bytes_y_vaciar_limpia() {
        let mut c: CachePresupuesto<u8, u8> = CachePresupuesto::nueva(1000);
        c.insertar(1, 1, 10);
        c.insertar(1, 2, 30);
        assert_eq!(c.bytes(), 30);
        assert_eq!(c.obtener(&1), Some(2));
        c.vaciar();
        assert!(c.is_empty());
        assert_eq!(c.bytes(), 0);
    }

    #[test]
    fn las_entradas_vacias_no_pesan() {
        let mut c: CachePresupuesto<u8, Option<u8>> = CachePresupuesto::nueva(10);
        c.insertar(1, None, 0);
        c.nuevo_fotograma();
        c.insertar(2, Some(1), 50);
        assert_eq!(c.obtener(&1), Some(None));
    }
}
