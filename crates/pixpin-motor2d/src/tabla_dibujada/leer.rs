//! **Leer una tabla ya dibujada**: de sus rayas y sus textos, otra vez las
//! filas, las columnas y lo escrito en cada celda.
//!
//! La tabla es un grupo de rayas y textos de los de siempre (ver el modulo de
//! arriba), a proposito. El precio era que no se le podia anadir una fila
//! despues; con esto se le puede: se lee lo que hay, se edita en su cajetin y
//! se vuelve a dibujar en el mismo sitio y con el mismo grupo. Lo que no es
//! de la tabla —una figura metida en una celda— se apunta con su celda para
//! volver a encajarla en ella.
//!
//! No hace falta que la tabla la haya dibujado este programa: una del movil,
//! que son las mismas rayas, se lee igual.

use super::{AIRE_DE_CELDA, Celda};
use crate::ecuacion::{Estilo, Medir};
use crate::elemento::{Elemento, Figura};
use crate::escena::Escena;
use crate::vector::Punto2;

/// Lo que se le admite a una raya para contar como vertical u horizontal.
const TOLERANCIA: f32 = 0.75;

/// Una tabla leida de su dibujo.
#[derive(Debug, Clone, PartialEq)]
pub struct TablaLeida {
    /// Las celdas, fila a fila (rejilla regular).
    pub celdas: Vec<Vec<Celda>>,
    /// Si la primera fila lleva el fondo de cabecera.
    pub cabecera: bool,
    /// Donde empiezan las columnas y las filas, del borde de la izquierda y
    /// del de arriba al de la derecha y al de abajo.
    pub columnas: Vec<f32>,
    pub filas: Vec<f32>,
    /// La letra de sus textos, si tiene alguno.
    pub tam: Option<f32>,
    pub familia: Option<String>,
    /// Los ids que son la tabla (marco, cabecera, rayas y textos de celda).
    pub de_la_tabla: Vec<u64>,
    /// Lo demas del grupo, con la celda `(fila, columna)` en que cae su
    /// centro: las figuras metidas en la tabla.
    pub figuras: Vec<(u64, usize, usize)>,
}

impl TablaLeida {
    /// La esquina de arriba a la izquierda.
    pub fn origen(&self) -> Punto2 {
        Punto2::nuevo(self.columnas[0], self.filas[0])
    }

    /// La caja de la celda `(fila, columna)`.
    pub fn caja_de(&self, fila: usize, columna: usize) -> Option<(f32, f32, f32, f32)> {
        Some((
            *self.columnas.get(columna)?,
            *self.filas.get(fila)?,
            *self.columnas.get(columna + 1)?,
            *self.filas.get(fila + 1)?,
        ))
    }

    /// En que celda cae un punto.
    pub fn celda_en(&self, p: Punto2) -> Option<(usize, usize)> {
        let c = self
            .columnas
            .windows(2)
            .position(|w| p.x >= w[0] && p.x <= w[1])?;
        let f = self
            .filas
            .windows(2)
            .position(|w| p.y >= w[0] && p.y <= w[1])?;
        Some((f, c))
    }
}

fn es_raya(e: &Elemento) -> Option<(Punto2, Punto2)> {
    match &e.figura {
        Figura::Linea { puntos } if puntos.len() == 2 && e.angulo == 0.0 => {
            Some((puntos[0], puntos[1]))
        }
        _ => None,
    }
}

fn centro(e: &Elemento) -> Punto2 {
    let (x0, y0, x1, y1) = e.caja();
    Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

/// Mete `v` en la lista ordenada si no hay ya uno a menos de un pixel.
fn meter(lista: &mut Vec<f32>, v: f32) {
    if lista.iter().all(|x| (x - v).abs() > 1.0) {
        lista.push(v);
        lista.sort_by(f32::total_cmp);
    }
}

/// **La tabla que forman estos elementos**, o `None` si no son una tabla: un
/// marco (el rectangulo con trazo mas grande), al menos una raya recta que
/// lo cruce de lado a lado o por tramos, y nada girado.
pub fn leer_tabla(elementos: &[&Elemento]) -> Option<TablaLeida> {
    let vivos: Vec<&Elemento> = elementos.iter().copied().filter(|e| !e.borrado).collect();
    let marco = vivos
        .iter()
        .filter(|e| matches!(e.figura, Figura::Rectangulo) && e.trazo.a > 0.0 && e.angulo == 0.0)
        .max_by(|a, b| (a.ancho * a.alto).total_cmp(&(b.ancho * b.alto)))?;
    let (x0, y0, x1, y1) = (
        marco.x,
        marco.y,
        marco.x + marco.ancho,
        marco.y + marco.alto,
    );
    let dentro =
        |p: Punto2| p.x >= x0 - 1.0 && p.x <= x1 + 1.0 && p.y >= y0 - 1.0 && p.y <= y1 + 1.0;
    let mut columnas = vec![x0, x1];
    let mut filas = vec![y0, y1];
    let mut de_la_tabla = vec![marco.id];
    for e in &vivos {
        let Some((a, b)) = es_raya(e) else { continue };
        if !dentro(a) || !dentro(b) {
            continue;
        }
        if (a.x - b.x).abs() < TOLERANCIA {
            meter(&mut columnas, a.x);
            de_la_tabla.push(e.id);
        } else if (a.y - b.y).abs() < TOLERANCIA {
            meter(&mut filas, a.y);
            de_la_tabla.push(e.id);
        }
    }
    if columnas.len() < 3 && filas.len() < 3 {
        return None;
    }
    let (n_filas, n_columnas) = (filas.len() - 1, columnas.len() - 1);
    // La cabecera: el rectangulo relleno y sin trazo pegado arriba, de la
    // altura de la primera fila.
    let cabecera = vivos.iter().find(|e| {
        e.id != marco.id
            && matches!(e.figura, Figura::Rectangulo)
            && e.relleno.is_some_and(|c| c.a > 0.0)
            && (e.y - y0).abs() < 1.0
            && (e.alto - (filas[1] - y0)).abs() < 1.5
    });
    if let Some(c) = cabecera {
        de_la_tabla.push(c.id);
    }
    let mut celdas = vec![vec![Celda::de(""); n_columnas]; n_filas];
    let mut tam = None;
    let mut familia = None;
    let mut figuras = Vec::new();
    let leida = TablaLeida {
        celdas: Vec::new(),
        cabecera: cabecera.is_some(),
        columnas,
        filas,
        tam: None,
        familia: None,
        de_la_tabla: Vec::new(),
        figuras: Vec::new(),
    };
    for e in &vivos {
        if de_la_tabla.contains(&e.id) {
            continue;
        }
        let Some((f, c)) = leida.celda_en(centro(e)) else {
            continue;
        };
        match &e.figura {
            // Un texto pegado a la izquierda de su celda es lo escrito en
            // ella; uno suelto por ahi dentro es una figura mas.
            Figura::Texto {
                texto,
                tam: t,
                familia: fam,
            } if (e.x - leida.columnas[c] - AIRE_DE_CELDA).abs() < 3.0 => {
                let celda = &mut celdas[f][c];
                if !celda.texto.is_empty() {
                    celda.texto.push(' ');
                }
                celda.texto.push_str(texto);
                celda.negrita |= e.extras.negrita;
                tam.get_or_insert(*t);
                familia.get_or_insert_with(|| fam.clone());
                de_la_tabla.push(e.id);
            }
            _ => figuras.push((e.id, f, c)),
        }
    }
    Some(TablaLeida {
        celdas,
        tam,
        familia,
        de_la_tabla,
        figuras,
        ..leida
    })
}

/// Los elementos de la escena con estos ids, en su orden.
fn de_la_escena<'a>(escena: &'a Escena, ids: &[u64]) -> Vec<&'a Elemento> {
    escena
        .elementos
        .iter()
        .filter(|e| ids.contains(&e.id))
        .collect()
}

/// Encaja en su celda cada figura de `figuras` (juntas las de una misma
/// celda: una figura de varias piezas se encaja entera), la mete en el grupo
/// de la tabla y la sube al frente, por encima del fondo de la cabecera.
/// Todo dentro del paso que haya abierto.
fn encajar_figuras(
    escena: &mut Escena,
    tabla: &TablaLeida,
    figuras: &[(u64, usize, usize)],
    grupos: &[String],
) {
    let mut celdas: Vec<(usize, usize)> = figuras.iter().map(|&(_, f, c)| (f, c)).collect();
    celdas.sort_unstable();
    celdas.dedup();
    escena.apuntar_reordenamiento();
    for (f, c) in celdas {
        let Some(caja) = tabla.caja_de(f, c) else {
            continue;
        };
        let ids: Vec<u64> = figuras
            .iter()
            .filter(|x| (x.1, x.2) == (f, c))
            .map(|x| x.0)
            .collect();
        let piezas: Vec<Elemento> = de_la_escena(escena, &ids).into_iter().cloned().collect();
        for n in crate::estirar_bloque::encajar(&piezas, caja, AIRE_DE_CELDA) {
            escena.apuntar_edicion(n.id);
            if let Some(e) = escena.buscar_mut(n.id) {
                (e.figura, e.x, e.y, e.ancho, e.alto) = (n.figura, n.x, n.y, n.ancho, n.alto);
                // En el grupo de la tabla, por fuera de los suyos: se mueve
                // con ella y sigue siendo la figura que era.
                for g in grupos {
                    if !e.grupos.contains(g) {
                        e.grupos.push(g.clone());
                    }
                }
                e.tocar();
            }
            escena.traer_al_frente(n.id);
        }
    }
}

/// **Mete en su celda lo que no es de la tabla**: la figura que se dejo
/// encima de una celda y se eligio junto con la tabla sale encajada en ella,
/// sin deformar, centrada y dentro del grupo de la tabla. Un paso de
/// deshacer. `false` si no habia nada que meter.
pub fn meter_en_celdas(escena: &mut Escena, elegidos: &[u64]) -> bool {
    let Some(tabla) = leer_tabla(&de_la_escena(escena, elegidos)) else {
        return false;
    };
    if tabla.figuras.is_empty() {
        return false;
    }
    let grupos = escena
        .buscar(tabla.de_la_tabla[0])
        .map(|e| e.grupos.clone())
        .unwrap_or_default();
    escena.abrir_paso();
    encajar_figuras(escena, &tabla, &tabla.figuras, &grupos);
    escena.cerrar_paso();
    true
}

/// **Vuelve a dibujar la tabla con otras celdas**, en el mismo sitio y en el
/// mismo grupo: lo que deja el cajetin de la tabla (una fila mas, una
/// columna menos, lo escrito en una celda). Las figuras que tenia metidas
/// vuelven a encajarse en su celda, si sigue existiendo. Un paso de deshacer.
/// Devuelve los ids de la tabla nueva y sus figuras, para dejarlos elegidos.
pub fn rehacer_en_escena(
    escena: &mut Escena,
    vieja: &TablaLeida,
    celdas: &[Vec<Celda>],
    cabecera: bool,
    estilo: &Estilo,
    medir: Medir<'_>,
) -> Vec<u64> {
    let nuevos =
        super::elementos_de_tabla_con_juntas(celdas, estilo, vieja.origen(), medir, cabecera);
    if nuevos.is_empty() || vieja.de_la_tabla.is_empty() {
        return Vec::new();
    }
    let grupos = escena
        .buscar(vieja.de_la_tabla[0])
        .map(|e| e.grupos.clone())
        .unwrap_or_default();
    escena.abrir_paso();
    for &id in &vieja.de_la_tabla {
        escena.borrar_apuntando(id);
    }
    let mut ids: Vec<u64> = nuevos
        .into_iter()
        .map(|mut e| {
            e.grupos = grupos.clone();
            escena.anadir(e)
        })
        .collect();
    if let Some(nueva) = leer_tabla(&de_la_escena(escena, &ids)) {
        // Una figura cuya celda ya no existe se queda donde estaba.
        let siguen: Vec<(u64, usize, usize)> = vieja
            .figuras
            .iter()
            .copied()
            .filter(|&(_, f, c)| nueva.caja_de(f, c).is_some())
            .collect();
        encajar_figuras(escena, &nueva, &siguen, &grupos);
    }
    ids.extend(vieja.figuras.iter().map(|x| x.0));
    escena.cerrar_paso();
    ids
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ecuacion::Estilo;
    use crate::elemento::ColorRgba;

    fn medir(texto: &str, tam: f32) -> (f32, f32) {
        (texto.chars().count() as f32 * tam * 0.5, tam * 1.2)
    }

    fn estilo() -> Estilo {
        Estilo {
            color: ColorRgba::opaco(0.1, 0.1, 0.1),
            tam: 20.0,
            opacidad: 1.0,
            familia: "Excalifont".into(),
        }
    }

    fn con_ids(mut v: Vec<Elemento>) -> Vec<Elemento> {
        for (i, e) in v.iter_mut().enumerate() {
            e.id = i as u64 + 1;
        }
        v
    }

    #[test]
    fn una_tabla_dibujada_se_lee_con_sus_celdas_su_cabecera_y_su_letra() {
        let filas = vec![
            vec!["Material".to_string(), "Cantidad".into()],
            vec!["Cemento".into(), "12".into()],
            vec!["Arena".into(), "".into()],
        ];
        let v = con_ids(super::super::elementos_de_tabla(
            &filas,
            &estilo(),
            Punto2::nuevo(10.0, 20.0),
            &medir,
            true,
        ));
        let refs: Vec<&Elemento> = v.iter().collect();
        let t = leer_tabla(&refs).expect("no la reconocio");
        let textos: Vec<Vec<&str>> = t
            .celdas
            .iter()
            .map(|f| f.iter().map(|c| c.texto.as_str()).collect())
            .collect();
        assert_eq!(
            textos,
            vec![
                vec!["Material", "Cantidad"],
                vec!["Cemento", "12"],
                vec!["Arena", ""]
            ]
        );
        assert!(t.cabecera);
        assert_eq!(t.origen(), Punto2::nuevo(10.0, 20.0));
        assert_eq!(t.tam, Some(20.0));
        assert_eq!(t.de_la_tabla.len(), v.len(), "todo es de la tabla");
        assert!(t.figuras.is_empty());
    }

    #[test]
    fn lo_que_no_es_de_la_tabla_se_apunta_con_su_celda() {
        let filas = vec![
            vec!["a".to_string(), "b".into()],
            vec!["c".into(), "d".into()],
        ];
        let mut v = super::super::elementos_de_tabla(
            &filas,
            &estilo(),
            Punto2::nuevo(0.0, 0.0),
            &medir,
            false,
        );
        let t0 = {
            let v = con_ids(v.clone());
            let refs: Vec<&Elemento> = v.iter().collect();
            leer_tabla(&refs).unwrap()
        };
        let (cx0, cy0, cx1, cy1) = t0.caja_de(1, 1).unwrap();
        v.push(Elemento {
            figura: Figura::Elipse,
            x: (cx0 + cx1) / 2.0 - 3.0,
            y: (cy0 + cy1) / 2.0 - 3.0,
            ancho: 6.0,
            alto: 6.0,
            ..Default::default()
        });
        let v = con_ids(v);
        let refs: Vec<&Elemento> = v.iter().collect();
        let t = leer_tabla(&refs).unwrap();
        assert_eq!(t.figuras, vec![(v.len() as u64, 1, 1)]);
        assert!(!t.cabecera);
    }

    #[test]
    fn unas_cajas_sueltas_no_son_una_tabla() {
        // Caso negativo: un rectangulo sin rayas, o con una raya en diagonal.
        let caja = Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            ancho: 100.0,
            alto: 50.0,
            ..Default::default()
        };
        assert!(leer_tabla(&[&caja]).is_none());
        let diagonal = crate::ecuacion::elemento_linea(
            vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 50.0)],
            1.0,
            &estilo(),
        );
        assert!(leer_tabla(&[&caja, &diagonal]).is_none());
    }
}

#[cfg(test)]
mod pruebas_en_la_escena {
    use super::*;
    use crate::ecuacion::Estilo;
    use crate::elemento::ColorRgba;

    fn medir(texto: &str, tam: f32) -> (f32, f32) {
        (texto.chars().count() as f32 * tam * 0.5, tam * 1.2)
    }

    fn estilo() -> Estilo {
        Estilo {
            color: ColorRgba::opaco(0.1, 0.1, 0.1),
            tam: 20.0,
            opacidad: 1.0,
            familia: "Excalifont".into(),
        }
    }

    /// Una tabla de 2x2 en la escena, agrupada, y sus ids.
    fn tabla(escena: &mut Escena) -> Vec<u64> {
        let filas = vec![
            vec!["Nombre".to_string(), "Foto".into()],
            vec!["Norte".into(), "".into()],
        ];
        super::super::elementos_de_tabla(&filas, &estilo(), Punto2::nuevo(0.0, 0.0), &medir, true)
            .into_iter()
            .map(|mut e| {
                e.grupos = vec!["t".into()];
                escena.anadir(e)
            })
            .collect()
    }

    fn leida(escena: &Escena, ids: &[u64]) -> TablaLeida {
        leer_tabla(&de_la_escena(escena, ids)).unwrap()
    }

    #[test]
    fn una_figura_encima_de_una_celda_entra_en_ella_sin_deformarse_y_con_la_tabla() {
        let mut escena = Escena::nueva();
        let mut ids = tabla(&mut escena);
        let (x0, y0, x1, y1) = leida(&escena, &ids).caja_de(1, 1).unwrap();
        // Un ovalo grande, el doble de ancho que alto, con el centro en la celda.
        let ovalo = escena.anadir(Elemento {
            figura: Figura::Elipse,
            x: (x0 + x1) / 2.0 - 100.0,
            y: (y0 + y1) / 2.0 - 50.0,
            ancho: 200.0,
            alto: 100.0,
            ..Default::default()
        });
        ids.push(ovalo);
        assert!(meter_en_celdas(&mut escena, &ids));
        let o = escena.buscar(ovalo).unwrap();
        assert!(
            o.x >= x0 && o.x + o.ancho <= x1 + 0.01 && o.y >= y0 && o.y + o.alto <= y1 + 0.01,
            "no quedo dentro"
        );
        assert!(
            (o.ancho / o.alto - 2.0).abs() < 0.01,
            "se deformo: {}x{}",
            o.ancho,
            o.alto
        );
        assert!(
            o.grupos.contains(&"t".to_string()),
            "no se fue con la tabla"
        );
        assert_eq!(
            escena.elementos.last().unwrap().id,
            ovalo,
            "quedo debajo de la tabla"
        );
        // Un paso de deshacer.
        escena.deshacer();
        assert_eq!(escena.buscar(ovalo).unwrap().ancho, 200.0);
    }

    #[test]
    fn sin_nada_encima_de_una_celda_no_se_mete_nada() {
        let mut escena = Escena::nueva();
        let ids = tabla(&mut escena);
        assert!(!meter_en_celdas(&mut escena, &ids));
    }

    #[test]
    fn rehacer_con_una_fila_mas_la_deja_en_su_sitio_con_su_grupo_y_su_figura() {
        let mut escena = Escena::nueva();
        let mut ids = tabla(&mut escena);
        let (x0, y0, x1, y1) = leida(&escena, &ids).caja_de(1, 1).unwrap();
        let punto = escena.anadir(Elemento {
            figura: Figura::Rectangulo,
            x: (x0 + x1) / 2.0 - 2.0,
            y: (y0 + y1) / 2.0 - 2.0,
            ancho: 4.0,
            alto: 4.0,
            ..Default::default()
        });
        ids.push(punto);
        let vieja = leida(&escena, &ids);
        let mut celdas = vieja.celdas.clone();
        celdas.push(vec![Celda::de("Sur"), Celda::de("")]);
        celdas[0][0].texto = "Lugar".into();
        let nuevos = rehacer_en_escena(&mut escena, &vieja, &celdas, true, &estilo(), &medir);
        assert!(nuevos.contains(&punto));
        let nueva = leida(&escena, &nuevos);
        assert_eq!(nueva.celdas.len(), 3);
        assert_eq!(nueva.celdas[0][0].texto, "Lugar");
        assert_eq!(nueva.celdas[2][0].texto, "Sur");
        assert_eq!(nueva.origen(), vieja.origen());
        assert!(
            escena
                .visibles()
                .all(|e| e.grupos.contains(&"t".to_string()))
        );
        // La figura sigue en su celda (1, 1).
        assert_eq!(
            nueva.figuras.iter().map(|x| (x.1, x.2)).collect::<Vec<_>>(),
            vec![(1, 1)]
        );
        // Y deshacer devuelve la de antes entera.
        escena.deshacer();
        let otra_vez: Vec<u64> = escena.visibles().map(|e| e.id).collect();
        assert_eq!(leida(&escena, &otra_vez).celdas.len(), 2);
    }
}
