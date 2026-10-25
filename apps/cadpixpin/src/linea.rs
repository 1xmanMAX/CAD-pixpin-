//! **La linea de ordenes**: lo que se va escribiendo, el historial que se
//! recorre con las flechas arriba y abajo, y el autocompletado con Tab (de
//! las ordenes, o de las opciones `[A/B/C]` de lo que pide la orden en
//! marcha), como en AutoCAD. Sin Windows: se prueba en cualquier sistema.

use cad_nucleo::ordenes;

/// Cuantas entradas recuerda el historial.
const HISTORIAL: usize = 100;

/// **La linea donde se escribe.**
#[derive(Debug, Default)]
pub struct Linea {
    /// Lo escrito hasta ahora.
    pub escrito: String,
    /// Lo que se escribio antes (lo mas reciente al final), y si era una
    /// orden (escrita sin orden en marcha) o una respuesta a una.
    historial: Vec<(String, bool)>,
    /// Recorriendo el historial: en que entrada, y lo que habia escrito antes.
    paseo: Option<(usize, String)>,
    /// Completando con Tab: lo que se escribio y el candidato de ahora.
    tab: Option<(String, usize)>,
}

impl Linea {
    /// Una letra mas.
    pub fn poner(&mut self, c: char) {
        self.soltar();
        self.escrito.push(c);
    }

    /// Borra la ultima letra.
    pub fn atras(&mut self) {
        self.soltar();
        self.escrito.pop();
    }

    /// Borra todo lo escrito.
    pub fn limpiar(&mut self) {
        self.soltar();
        self.escrito.clear();
    }

    /// Deja de recorrer el historial o de completar (lo escrito se queda).
    fn soltar(&mut self) {
        self.paseo = None;
        self.tab = None;
    }

    /// **Enter**: da lo escrito y lo apunta en el historial (sin repetir
    /// lo ultimo ni apuntar lo vacio). `es_orden`: se escribio sin orden en
    /// marcha.
    pub fn tomar(&mut self, es_orden: bool) -> String {
        self.soltar();
        let t = std::mem::take(&mut self.escrito);
        let limpio = t.trim();
        if !limpio.is_empty() && self.historial.last().map(|(h, _)| h.as_str()) != Some(limpio) {
            self.historial.push((limpio.to_string(), es_orden));
            if self.historial.len() > HISTORIAL {
                self.historial.remove(0);
            }
        }
        t
    }

    /// **Flecha arriba**: lo escrito antes (cada vez, uno mas viejo). Con
    /// `solo_ordenes` (en «Orden:»), solo las ordenes, como en AutoCAD.
    pub fn subir(&mut self, solo_ordenes: bool) {
        self.tab = None;
        let desde = self
            .paseo
            .as_ref()
            .map_or(self.historial.len(), |(i, _)| *i);
        let Some(i) = (0..desde)
            .rev()
            .find(|k| !solo_ordenes || self.historial[*k].1)
        else {
            return;
        };
        match &mut self.paseo {
            Some((k, _)) => *k = i,
            None => self.paseo = Some((i, self.escrito.clone())),
        }
        self.escrito = self.historial[i].0.clone();
    }

    /// **Flecha abajo**: lo escrito despues; al pasar del ultimo, lo que
    /// habia antes de subir.
    pub fn bajar(&mut self, solo_ordenes: bool) {
        let Some((i, borrador)) = self.paseo.take() else {
            return;
        };
        self.tab = None;
        match (i + 1..self.historial.len()).find(|k| !solo_ordenes || self.historial[*k].1) {
            Some(j) => {
                self.escrito = self.historial[j].0.clone();
                self.paseo = Some((j, borrador));
            }
            None => self.escrito = borrador,
        }
    }

    /// **Tab**: cambia lo escrito por el siguiente de los `candidatos` para
    /// lo que se habia escrito (Mayus+Tab: el anterior). `candidatos` se
    /// pide con lo escrito antes del primer Tab.
    pub fn completar(&mut self, atras: bool, candidatos: impl FnOnce(&str) -> Vec<String>) {
        self.paseo = None;
        let (base, i) = match self.tab.take() {
            Some((base, i)) => (base, Some(i)),
            None => (self.escrito.clone(), None),
        };
        let v = candidatos(&base);
        if v.is_empty() {
            return;
        }
        let n = v.len();
        let k = match (i, atras) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
        };
        self.escrito = v[k].clone();
        self.tab = Some((base, k));
    }
}

/// **Las ordenes que completan** lo escrito: primero las que se llaman
/// asi, de la mas corta a la mas larga, y luego las que tienen un alias
/// que empieza asi.
pub fn ordenes_para(escrito: &str) -> Vec<String> {
    let t = ordenes::sin_tildes(escrito.trim()).to_uppercase();
    let mut v = ordenes::sugerencias(&t);
    v.sort_by_key(|o| (!o.nombre.starts_with(&t), o.nombre.len(), o.nombre));
    let mut nombres: Vec<String> = Vec::new();
    for o in v {
        if !nombres.iter().any(|n| n == o.nombre) {
            nombres.push(o.nombre.to_string());
        }
    }
    nombres
}

/// **Las opciones `[A/B/C]`** de una indicacion que empiezan por lo escrito.
pub fn opciones_para(indicacion: &str, escrito: &str) -> Vec<String> {
    let t = ordenes::sin_tildes(escrito.trim()).to_uppercase();
    let (Some(a), Some(b)) = (indicacion.find('['), indicacion.find(']')) else {
        return Vec::new();
    };
    if b <= a {
        return Vec::new();
    }
    indicacion[a + 1..b]
        .split('/')
        .map(|o| {
            // La primera palabra (lo que se escribe), sin el valor de ahora.
            let o = o.split(':').next().unwrap_or("");
            o.split_whitespace().next().unwrap_or("").to_string()
        })
        .filter(|o| !o.is_empty() && ordenes::sin_tildes(o).to_uppercase().starts_with(&t))
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn escribir(l: &mut Linea, t: &str) {
        for c in t.chars() {
            l.poner(c);
        }
    }

    #[test]
    fn el_historial_sube_baja_y_vuelve_a_lo_escrito() {
        let mut l = Linea::default();
        for (t, orden) in [
            ("l", true),
            ("0,0", false),
            ("0,0", false),
            ("", false),
            ("co", true),
        ] {
            escribir(&mut l, t);
            l.tomar(orden);
        }
        escribir(&mut l, "zo");
        l.subir(false);
        assert_eq!(l.escrito, "co");
        l.subir(false);
        assert_eq!(l.escrito, "0,0");
        l.subir(false);
        assert_eq!(l.escrito, "l");
        // Del mas viejo no se pasa.
        l.subir(false);
        assert_eq!(l.escrito, "l");
        l.bajar(false);
        l.bajar(false);
        assert_eq!(l.escrito, "co");
        // Y al bajar del ultimo, lo que se estaba escribiendo.
        l.bajar(false);
        assert_eq!(l.escrito, "zo");
        l.bajar(false);
        assert_eq!(l.escrito, "zo");
        // Escribir suelta el paseo: subir empieza de nuevo por el ultimo.
        l.subir(false);
        l.poner('x');
        l.subir(false);
        assert_eq!(l.escrito, "co");
        // En «Orden:», solo las ordenes (se salta el punto).
        l.limpiar();
        l.subir(true);
        l.subir(true);
        assert_eq!(l.escrito, "l");
        l.bajar(true);
        assert_eq!(l.escrito, "co");
    }

    #[test]
    fn sin_historial_las_flechas_no_hacen_nada() {
        let mut l = Linea::default();
        escribir(&mut l, "ab");
        l.subir(false);
        l.bajar(false);
        assert_eq!(l.escrito, "ab");
        // Con solo respuestas, en «Orden:» no hay nada que recordar.
        l.tomar(false);
        l.subir(true);
        assert_eq!(l.escrito, "");
    }

    #[test]
    fn tab_recorre_los_candidatos_y_vuelve() {
        let mut l = Linea::default();
        escribir(&mut l, "co");
        let c = |b: &str| ordenes_para(b);
        l.completar(false, c);
        let primero = l.escrito.clone();
        assert!(primero.starts_with("CO"), "{primero}");
        l.completar(false, c);
        assert_ne!(l.escrito, primero);
        l.completar(true, c);
        assert_eq!(l.escrito, primero);
        // Nada que completar: lo escrito se queda.
        let mut l = Linea::default();
        escribir(&mut l, "qqq");
        l.completar(false, c);
        assert_eq!(l.escrito, "qqq");
    }

    #[test]
    fn las_ordenes_que_se_llaman_asi_van_primero() {
        let v = ordenes_para("lin");
        assert_eq!(v.first().map(String::as_str), Some("LINEA"));
        assert!(v.len() < 10);
        // Por alias tambien (TR: RECORTAR).
        assert!(ordenes_para("tr").iter().any(|n| n == "RECORTAR"));
        assert!(ordenes_para("").is_empty());
    }

    #[test]
    fn las_opciones_de_la_indicacion() {
        let i = "Siguiente punto [Cerrar/Deshacer] (Enter termina):";
        assert_eq!(opciones_para(i, "c"), vec!["Cerrar"]);
        assert_eq!(opciones_para(i, ""), vec!["Cerrar", "Deshacer"]);
        assert!(opciones_para("Primer punto:", "c").is_empty());
        assert_eq!(
            opciones_para("Punto base [Conservar originales: Si] (C cambia):", "co"),
            vec!["Conservar"]
        );
    }
}
