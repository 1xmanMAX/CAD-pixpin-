//! La matematica del cosido de la captura con scroll: pura, sin pantalla ni
//! GPU, para poder probarla en CI (S1 §4, D73).
//!
//! Es el puerto de `ScrollMatcher`, `ScrollPlan` y `ScrollStitcher` del
//! Android. Cada fila de un fotograma se resume en una **firma** (la suma de
//! las luminancias de uno de cada N pixeles) y el desplazamiento entre dos
//! fotogramas se busca sobre ese vector: comparar imagenes enteras seria
//! carisimo y no haria falta.
//!
//! Lo importante no es acertar siempre, sino **no acertar por casualidad**:
//! un fotograma mal cosido estropea la imagen entera sin que se note hasta el
//! final. Por eso se rechaza lo dudoso (bandas lisas, coincidencias
//! ambiguas) y se espera al siguiente fotograma, que llega en milisegundos.
//!
//! Lo que el Android no resolvia y aqui si (D74): las **franjas fijas**. Una
//! barra que no se mueve al hacer scroll se repite en cada fotograma; entre
//! dos fotogramas que si se han desplazado, las filas identicas por arriba
//! son cabecera y por abajo pie. La cabecera entra una vez, con el primer
//! fotograma; el pie se excluye de cada tira y se anade una sola vez al
//! final.

use crate::ImagenRgba;

/// `encontrar_desplazamiento` no encontro un encaje fiable.
pub const SIN_ENCAJE: i32 = -1;
/// Filas de referencia que se buscan en el fotograma siguiente.
pub const FILAS_COLA: usize = 48;
/// Hasta donde se agranda la referencia si la banda no tiene textura.
pub const MAX_FILAS_COLA: usize = 384;
/// Se muestrea uno de cada N pixeles por fila: sobra para distinguirlas.
pub const PASO_MUESTREO: usize = 5;
pub const TOLERANCIA: i64 = 40;
pub const VARIACION_MINIMA: i64 = 300;
/// Distancia minima entre dos candidatos para considerarlos alternativas.
const SEPARACION_MINIMA: i32 = 4;
/// Cuanto peor tiene que ser la segunda opcion para fiarse de la primera.
const FACTOR_AMBIGUEDAD: i64 = 2;
/// Una franja fija nunca ocupa mas de esta fraccion del fotograma: si lo
/// hiciera, seria "todo igual", no una barra.
const FRACCION_FIJA_MAXIMA: f32 = 0.4;

/// Firma de una fila RGBA (`ancho * 4` bytes), muestreando uno de cada `paso`.
pub fn firma_de_fila(fila: &[u8], paso: usize) -> i64 {
    let paso = paso.max(1);
    let mut suma: i64 = 0;
    let mut x = 0;
    while x * 4 + 2 < fila.len() {
        let r = fila[x * 4] as i64;
        let g = fila[x * 4 + 1] as i64;
        let b = fila[x * 4 + 2] as i64;
        // Luminancia entera: 0,299 / 0,587 / 0,114 en escala de 256.
        suma += (r * 77 + g * 151 + b * 28) >> 8;
        x += paso;
    }
    suma
}

/// Las firmas de todas las filas de la imagen.
pub fn firmas(imagen: &ImagenRgba, paso: usize) -> Vec<i64> {
    let fila_bytes = imagen.ancho as usize * 4;
    if fila_bytes == 0 {
        return Vec::new();
    }
    imagen
        .pixeles
        .chunks_exact(fila_bytes)
        .map(|f| firma_de_fila(f, paso))
        .collect()
}

/// Donde vuelven a aparecer, dentro de `marco`, las filas de `cola` (el
/// final de lo ya acumulado). El resultado es la fila de `marco` donde
/// empieza la cola; `SIN_ENCAJE` si no encaja con confianza o encaja en
/// varios sitios parecidos.
pub fn encontrar_desplazamiento(cola: &[i64], marco: &[i64], tolerancia: i64) -> i32 {
    if cola.is_empty() || marco.len() < cola.len() {
        return SIN_ENCAJE;
    }
    let n = marco.len() - cola.len() + 1;
    let mut puntuaciones = vec![0i64; n];
    let mut mejor = SIN_ENCAJE;
    let mut mejor_puntuacion = i64::MAX;
    for (d, puntuacion) in puntuaciones.iter_mut().enumerate() {
        let mut p: i64 = 0;
        for (k, c) in cola.iter().enumerate() {
            p += (c - marco[d + k]).abs();
        }
        *puntuacion = p;
        if p < mejor_puntuacion {
            mejor_puntuacion = p;
            mejor = d as i32;
        }
    }
    if mejor == SIN_ENCAJE {
        return SIN_ENCAJE;
    }
    let media_mejor = mejor_puntuacion / cola.len() as i64;
    if media_mejor > tolerancia {
        return SIN_ENCAJE;
    }

    // Segunda mejor opcion lo bastante lejos como para ser otra alternativa.
    let mut segunda = i64::MAX;
    for (d, p) in puntuaciones.iter().enumerate() {
        if (d as i32 - mejor).abs() < SEPARACION_MINIMA {
            continue;
        }
        if *p < segunda {
            segunda = *p;
        }
    }
    if segunda == i64::MAX {
        return mejor;
    }
    // El margen aditivo es lo que salva de los patrones que se repiten: una
    // cabecera cada N filas encaja PERFECTO en varios sitios a la vez, y
    // "el doble de malo" se cumpliria con dos ceros.
    let media_segunda = segunda / cola.len() as i64;
    if media_segunda >= media_mejor * FACTOR_AMBIGUEDAD + tolerancia / 2 {
        mejor
    } else {
        SIN_ENCAJE
    }
}

/// Una banda sin textura (un fondo liso, un degradado suave) encaja en
/// cualquier sitio: no sirve como referencia y hay que esperar.
pub fn es_lisa(cola: &[i64], variacion_minima: i64) -> bool {
    if cola.len() < 2 {
        return true;
    }
    let min = cola.iter().copied().min().unwrap_or(0);
    let max = cola.iter().copied().max().unwrap_or(0);
    max - min < variacion_minima
}

/// Filas identicas por arriba (cabecera) y por abajo (pie) entre dos
/// fotogramas consecutivos que SI se han desplazado (D74). Si todo es igual
/// (la pantalla no se movio) no hay nada que distinguir: `(0, 0)`.
pub fn franjas_fijas(anterior: &[i64], actual: &[i64], tolerancia: i64) -> (usize, usize) {
    let n = anterior.len().min(actual.len());
    if n == 0 {
        return (0, 0);
    }
    let tope = (n as f32 * FRACCION_FIJA_MAXIMA) as usize;
    let mut cabecera = 0;
    while cabecera < tope && (anterior[cabecera] - actual[cabecera]).abs() <= tolerancia {
        cabecera += 1;
    }
    let (la, lb) = (anterior.len(), actual.len());
    let mut pie = 0;
    while pie < tope && (anterior[la - 1 - pie] - actual[lb - 1 - pie]).abs() <= tolerancia {
        pie += 1;
    }
    // Si por arriba y por abajo llega hasta el tope, es que no se movio.
    if cabecera >= tope && pie >= tope {
        return (0, 0);
    }
    (cabecera, pie)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resultado {
    /// Primer fotograma: es la base.
    Primero,
    /// Se ha anadido contenido nuevo.
    Anadido,
    /// La pantalla no se ha movido desde el fotograma anterior.
    SinMovimiento,
    /// No se puede encajar con confianza: se descarta y se espera.
    Incierto,
    /// Se ha alcanzado el alto maximo.
    Lleno,
}

/// La orden: que franja del fotograma hay que quedarse. `filas` a cero
/// significa que no hay nada que copiar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Orden {
    pub resultado: Resultado,
    pub desde: usize,
    pub filas: usize,
}

impl Orden {
    fn solo(resultado: Resultado) -> Orden {
        Orden {
            resultado,
            desde: 0,
            filas: 0,
        }
    }
}

/// Que hacer con cada fotograma, sin tocar un solo pixel. Ante la duda, no
/// cose: descartar un fotograma cuesta milisegundos; coserlo mal, la imagen.
#[derive(Debug)]
pub struct Plan {
    alto_maximo: usize,
    alto: usize,
    /// La banda de referencia, tomada del final del contenido (sin pie).
    cola: Vec<i64>,
    /// Las firmas enteras del ultimo fotograma cosido, para detectar las
    /// franjas fijas contra el siguiente.
    anterior: Vec<i64>,
    /// Cabecera y pie fijos, una vez detectados. Se detectan con el primer
    /// par de fotogramas que se mueve y no cambian: una barra fija lo es
    /// durante toda la captura.
    franjas: Option<(usize, usize)>,
}

impl Plan {
    pub fn nuevo(alto_maximo: usize) -> Plan {
        Plan {
            alto_maximo,
            alto: 0,
            cola: Vec::new(),
            anterior: Vec::new(),
            franjas: None,
        }
    }

    /// Lo que llevamos cosido, en filas (sin contar el pie, que va al final).
    pub fn alto(&self) -> usize {
        self.alto
    }

    pub fn esta_vacio(&self) -> bool {
        self.alto == 0
    }

    /// Filas de pie fijo detectadas, si se detectaron.
    pub fn pie(&self) -> usize {
        self.franjas.map_or(0, |(_, p)| p)
    }

    pub fn reiniciar(&mut self) {
        self.alto = 0;
        self.cola.clear();
        self.anterior.clear();
        self.franjas = None;
    }

    /// Decide que hacer con un fotograma. Solo cambia el estado con
    /// `Primero` o `Anadido`: quien llama puede descartar el fotograma sin
    /// haber ensuciado nada.
    pub fn plan(&mut self, firmas: &[i64], filas: usize) -> Orden {
        if filas <= FILAS_COLA || firmas.len() < filas {
            return Orden::solo(Resultado::Incierto);
        }

        if self.esta_vacio() {
            if filas > self.alto_maximo {
                return Orden::solo(Resultado::Lleno);
            }
            self.alto = filas;
            self.anterior = firmas[..filas].to_vec();
            self.cola = elegir_cola(&self.anterior);
            return Orden {
                resultado: Resultado::Primero,
                desde: 0,
                filas,
            };
        }

        // Las franjas fijas se buscan con el primer par que se mueve. Hasta
        // saberlo, la cola incluye el posible pie y el encaje falla contra
        // un fotograma con el pie excluido: por eso se recalcula la cola
        // desde el contenido del fotograma anterior en cuanto se detectan.
        let (cabecera, pie) = match self.franjas {
            Some(f) => f,
            None => {
                let f = franjas_fijas(&self.anterior, &firmas[..filas], TOLERANCIA);
                if f != (0, 0) {
                    self.franjas = Some(f);
                    let fin = self.anterior.len().saturating_sub(f.1);
                    self.cola = elegir_cola(&self.anterior[f.0.min(fin)..fin]);
                }
                f
            }
        };

        let contenido = &firmas[cabecera.min(filas)..filas.saturating_sub(pie).max(cabecera)];
        let desplazamiento = encontrar_desplazamiento(&self.cola, contenido, TOLERANCIA);
        if desplazamiento == SIN_ENCAJE {
            return Orden::solo(Resultado::Incierto);
        }

        let nuevo_desde = desplazamiento as usize + self.cola.len();
        if nuevo_desde >= contenido.len() {
            return Orden::solo(Resultado::SinMovimiento);
        }
        let filas_nuevas = contenido.len() - nuevo_desde;
        if self.alto + filas_nuevas > self.alto_maximo {
            return Orden::solo(Resultado::Lleno);
        }

        self.alto += filas_nuevas;
        self.anterior = firmas[..filas].to_vec();
        self.cola = elegir_cola(contenido);
        Orden {
            resultado: Resultado::Anadido,
            desde: cabecera + nuevo_desde,
            filas: filas_nuevas,
        }
    }
}

/// Banda de referencia del final de lo acumulado. Si no tiene textura (un
/// fondo liso, el final de una pagina en blanco) se agranda hasta encontrar
/// algo distinguible: si no, encajaria en cualquier sitio.
fn elegir_cola(firmas: &[i64]) -> Vec<i64> {
    let mut filas = FILAS_COLA;
    while filas < MAX_FILAS_COLA && filas < firmas.len() {
        let candidata = &firmas[firmas.len() - filas..];
        if !es_lisa(candidata, VARIACION_MINIMA) {
            return candidata.to_vec();
        }
        filas *= 2;
    }
    let tomar = filas.min(firmas.len());
    firmas[firmas.len() - tomar..].to_vec()
}

/// Va cosiendo los fotogramas en una sola imagen larga. Guarda solo las
/// **tiras nuevas** de cada uno; al terminar las pinta seguidas, recortando
/// el pie de la primera y anadiendolo una sola vez al final (D74).
#[derive(Debug)]
pub struct Cosedor {
    ancho: u32,
    plan: Plan,
    /// Cada tira: sus filas ya copiadas (ancho * 4 bytes por fila).
    tiras: Vec<Vec<u8>>,
    /// El pie del ultimo fotograma, por si hay que anadirlo al final.
    ultimo_pie: Vec<u8>,
}

impl Cosedor {
    pub fn nuevo(ancho: u32, alto_maximo: usize) -> Cosedor {
        Cosedor {
            ancho,
            plan: Plan::nuevo(alto_maximo),
            tiras: Vec::new(),
            ultimo_pie: Vec::new(),
        }
    }

    pub fn alto(&self) -> usize {
        self.plan.alto()
    }

    pub fn esta_vacio(&self) -> bool {
        self.tiras.is_empty()
    }

    /// `marco` es el recorte de la region elegida en el fotograma actual.
    pub fn anadir(&mut self, marco: &ImagenRgba) -> Resultado {
        if marco.ancho != self.ancho || marco.pixeles.len() != marco.bytes_esperados() {
            return Resultado::Incierto;
        }
        let firmas = firmas(marco, PASO_MUESTREO);
        let orden = self.plan.plan(&firmas, marco.alto as usize);
        if orden.filas > 0 {
            let fila_bytes = self.ancho as usize * 4;
            let desde = orden.desde * fila_bytes;
            let hasta = (orden.desde + orden.filas) * fila_bytes;
            self.tiras.push(marco.pixeles[desde..hasta].to_vec());
        }
        if matches!(orden.resultado, Resultado::Primero | Resultado::Anadido) {
            let pie = self.plan.pie();
            let fila_bytes = self.ancho as usize * 4;
            let alto = marco.alto as usize;
            self.ultimo_pie = if pie > 0 && pie < alto {
                marco.pixeles[(alto - pie) * fila_bytes..].to_vec()
            } else {
                Vec::new()
            };
        }
        orden.resultado
    }

    /// Junta todas las tiras en la imagen final. `None` si no hay nada.
    pub fn terminar(mut self) -> Option<ImagenRgba> {
        if self.tiras.is_empty() {
            return None;
        }
        let fila_bytes = self.ancho as usize * 4;
        // La primera tira entro con el pie (todavia no se conocia): fuera.
        let pie = self.plan.pie();
        if pie > 0 && self.tiras.len() > 1 {
            let primera = &mut self.tiras[0];
            let recorte = pie * fila_bytes;
            if primera.len() > recorte {
                primera.truncate(primera.len() - recorte);
            }
        }
        let mut pixeles: Vec<u8> = Vec::with_capacity(self.tiras.iter().map(Vec::len).sum());
        for t in &self.tiras {
            pixeles.extend_from_slice(t);
        }
        if self.tiras.len() > 1 {
            pixeles.extend_from_slice(&self.ultimo_pie);
        }
        let alto = (pixeles.len() / fila_bytes) as u32;
        if alto == 0 {
            return None;
        }
        Some(ImagenRgba {
            ancho: self.ancho,
            alto,
            pixeles,
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Generador determinista (LCG de Lehmer, el mismo del motor 2D): el
    /// `Random` de Kotlin no es reproducible desde Rust y aqui solo hace
    /// falta textura repetible.
    struct Azar(u32);
    impl Azar {
        fn nuevo(semilla: u32) -> Azar {
            Azar(semilla.max(1))
        }
        fn siguiente(&mut self, tope: i64) -> i64 {
            self.0 = self.0.wrapping_mul(48271) % 0x7FFF_FFFF;
            (self.0 as i64) % tope
        }
    }

    /// Una "pagina" con textura: cada fila tiene su propia firma.
    fn pagina(filas: usize, semilla: u32) -> Vec<i64> {
        let mut a = Azar::nuevo(semilla);
        (0..filas).map(|_| a.siguiente(100_000)).collect()
    }

    #[test]
    fn sin_desplazamiento_el_encaje_es_donde_estaba() {
        let contenido = pagina(200, 7);
        let cola = contenido[160..200].to_vec();
        assert_eq!(encontrar_desplazamiento(&cola, &contenido, 40), 160);
    }

    #[test]
    fn detecta_cuanto_ha_subido_el_contenido() {
        let contenido = pagina(400, 7);
        // El fotograma nuevo ensena la pagina 120 filas mas abajo.
        let marco = contenido[120..320].to_vec();
        let cola = contenido[280..320].to_vec();
        // Dentro del fotograma nuevo, esas filas empiezan en 280-120 = 160.
        assert_eq!(encontrar_desplazamiento(&cola, &marco, 40), 160);
    }

    #[test]
    fn aguanta_ruido_de_compresion() {
        let contenido = pagina(400, 7);
        let mut a = Azar::nuevo(1);
        let marco: Vec<i64> = (0..200)
            .map(|i| contenido[120 + i] + a.siguiente(17) - 8)
            .collect();
        let cola = contenido[280..320].to_vec();
        assert_eq!(encontrar_desplazamiento(&cola, &marco, 40), 160);
    }

    #[test]
    fn una_banda_lisa_se_rechaza_en_vez_de_encajar_en_cualquier_sitio() {
        let lisa = vec![5_000i64; 40];
        assert!(es_lisa(&lisa, 100));
        assert!(!es_lisa(&pagina(40, 7), 100));
    }

    #[test]
    fn un_patron_que_se_repite_se_considera_ambiguo() {
        // Cabecera repetida cada 20 filas: varios encajes igual de buenos.
        let repetido: Vec<i64> = (0..200).map(|i| (i % 20) * 1000).collect();
        let cola: Vec<i64> = (0..40).map(|i| (i % 20) * 1000).collect();
        assert_eq!(encontrar_desplazamiento(&cola, &repetido, 40), SIN_ENCAJE);
    }

    #[test]
    fn contenido_totalmente_distinto_no_encaja() {
        assert_eq!(
            encontrar_desplazamiento(&pagina(40, 1), &pagina(200, 99), 40),
            SIN_ENCAJE
        );
    }

    #[test]
    fn un_fotograma_mas_corto_que_la_referencia_no_encaja() {
        assert_eq!(
            encontrar_desplazamiento(&pagina(40, 7), &pagina(20, 7), 40),
            SIN_ENCAJE
        );
    }

    #[test]
    fn la_firma_de_una_fila_resume_su_contenido() {
        let negra = [0u8, 0, 0, 255].repeat(4);
        let blanca = [255u8, 255, 255, 255].repeat(4);
        assert_eq!(firma_de_fila(&negra, 1), 0);
        assert!(firma_de_fila(&blanca, 1) > 900);
    }

    #[test]
    fn las_firmas_se_calculan_fila_a_fila() {
        let img = ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![
                0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            ],
        };
        let f = firmas(&img, 1);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0], 0);
        assert!(f[1] > f[0]);
    }

    // ---- lo que el Android no tenia: paginas de verdad, cosidas ----

    /// Una pagina sintetica: `ancho` px, cada fila de un gris propio con
    /// textura horizontal para que las firmas no sean lisas.
    fn pagina_imagen(ancho: u32, filas: usize, semilla: u32) -> ImagenRgba {
        let mut a = Azar::nuevo(semilla);
        let mut pixeles = Vec::with_capacity(ancho as usize * filas * 4);
        for _ in 0..filas {
            let base = a.siguiente(200) as u8;
            for x in 0..ancho {
                let v = base.wrapping_add((x * 7 % 50) as u8);
                pixeles.extend_from_slice(&[v, v, v, 255]);
            }
        }
        ImagenRgba {
            ancho,
            alto: filas as u32,
            pixeles,
        }
    }

    fn recorte(pagina: &ImagenRgba, desde: usize, filas: usize) -> ImagenRgba {
        let fb = pagina.ancho as usize * 4;
        ImagenRgba {
            ancho: pagina.ancho,
            alto: filas as u32,
            pixeles: pagina.pixeles[desde * fb..(desde + filas) * fb].to_vec(),
        }
    }

    fn apilar(partes: &[&ImagenRgba]) -> ImagenRgba {
        let mut pixeles = Vec::new();
        let mut alto = 0;
        for p in partes {
            pixeles.extend_from_slice(&p.pixeles);
            alto += p.alto;
        }
        ImagenRgba {
            ancho: partes[0].ancho,
            alto,
            pixeles,
        }
    }

    #[test]
    fn una_pagina_recorrida_en_pasos_se_cose_igual_que_el_original() {
        let pagina = pagina_imagen(40, 1000, 3);
        let mut c = Cosedor::nuevo(40, 20_000);
        let ventana = 300;
        let mut desde = 0;
        let mut resultados = Vec::new();
        while desde + ventana <= 1000 {
            resultados.push(c.anadir(&recorte(&pagina, desde, ventana)));
            desde += 120;
        }
        // El ultimo tramo: la pagina termina y la ventana se queda pegada
        // al final (como una pagina real que ya no baja mas).
        resultados.push(c.anadir(&recorte(&pagina, 700, ventana)));
        resultados.push(c.anadir(&recorte(&pagina, 700, ventana)));
        assert_eq!(resultados[0], Resultado::Primero);
        assert!(resultados[1..6].iter().all(|r| *r == Resultado::Anadido));
        assert_eq!(*resultados.last().unwrap(), Resultado::SinMovimiento);
        let cosida = c.terminar().expect("hay imagen");
        assert_eq!(cosida.alto, 1000);
        assert_eq!(
            cosida.pixeles, pagina.pixeles,
            "la pagina cosida no es la original"
        );
    }

    #[test]
    fn la_cabecera_y_el_pie_fijos_salen_una_sola_vez() {
        // D74: una barra de 20 filas arriba y otra de 15 abajo que no se
        // mueven. Cada fotograma las repite; la imagen final no.
        let cabecera = pagina_imagen(40, 20, 11);
        let pie = pagina_imagen(40, 15, 13);
        let cuerpo = pagina_imagen(40, 900, 5);
        let ventana = 265; // 20 + 230 de cuerpo + 15
        let mut c = Cosedor::nuevo(40, 20_000);
        let mut desde = 0;
        while desde + 230 <= 900 {
            let marco = apilar(&[&cabecera, &recorte(&cuerpo, desde, 230), &pie]);
            assert_eq!(marco.alto as usize, ventana);
            let r = c.anadir(&marco);
            assert!(
                matches!(r, Resultado::Primero | Resultado::Anadido),
                "paso desde={desde}: {r:?}"
            );
            desde += 100;
        }
        let cosida = c.terminar().unwrap();
        let esperada = apilar(&[&cabecera, &recorte(&cuerpo, 0, desde - 100 + 230), &pie]);
        assert_eq!(
            cosida.alto, esperada.alto,
            "alto: cabecera + cuerpo + pie, una vez cada uno"
        );
        assert_eq!(cosida.pixeles, esperada.pixeles);
    }

    #[test]
    fn tres_marcos_iguales_no_anaden_nada() {
        let pagina = pagina_imagen(40, 400, 9);
        let mut c = Cosedor::nuevo(40, 20_000);
        let marco = recorte(&pagina, 0, 200);
        assert_eq!(c.anadir(&marco), Resultado::Primero);
        for _ in 0..3 {
            assert_eq!(c.anadir(&marco), Resultado::SinMovimiento);
        }
        assert_eq!(c.alto(), 200);
    }

    #[test]
    fn el_alto_maximo_devuelve_lleno() {
        let pagina = pagina_imagen(40, 600, 9);
        let mut c = Cosedor::nuevo(40, 250);
        assert_eq!(c.anadir(&recorte(&pagina, 0, 200)), Resultado::Primero);
        // 200 + 100 nuevas = 300 > 250: lleno, y no se cose.
        assert_eq!(c.anadir(&recorte(&pagina, 100, 200)), Resultado::Lleno);
        assert_eq!(c.alto(), 200);
    }

    #[test]
    fn un_marco_de_otro_ancho_es_incierto_y_no_ensucia() {
        let mut c = Cosedor::nuevo(40, 1000);
        let raro = pagina_imagen(30, 100, 1);
        assert_eq!(c.anadir(&raro), Resultado::Incierto);
        assert!(c.esta_vacio());
    }
}
