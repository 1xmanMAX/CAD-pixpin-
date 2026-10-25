//! **Las ordenes**: su nombre en castellano, los alias de siempre (los de
//! AutoCAD en ingles tambien: quien viene de alli escribe `L`, `TR`, `O`…) y
//! el grupo de la cinta en el que salen.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Id {
    Linea,
    Polilinea,
    Rectangulo,
    Circulo,
    Arco,
    Texto,
    Mover,
    Copiar,
    Girar,
    Escalar,
    Simetria,
    Desfase,
    Recortar,
    Alargar,
    Empalme,
    Chaflan,
    MatrizRect,
    MatrizPolar,
    Borrar,
    Descomponer,
    Cota,
    Nivel,
    Norte,
    Muro,
    Ejes,
    Puerta,
    Ventana,
    Lamina,
    Distancia,
    Area,
    Deshacer,
    Rehacer,
    /// Una orden de [`crate::herramientas`] (por su nombre).
    Otra(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Grupo {
    Dibujo,
    Modificar,
    Anotar,
    Arquitectura,
    Medir,
    Edicion,
    Bloques,
    Vista,
}

/// Las pestanas de la cinta, en su orden.
pub const GRUPOS: &[Grupo] = &[
    Grupo::Dibujo,
    Grupo::Modificar,
    Grupo::Arquitectura,
    Grupo::Anotar,
    Grupo::Bloques,
    Grupo::Medir,
    Grupo::Vista,
];

impl Grupo {
    pub fn nombre(self) -> &'static str {
        match self {
            Grupo::Dibujo => "Dibujo",
            Grupo::Modificar => "Modificar",
            Grupo::Anotar => "Anotar",
            Grupo::Arquitectura => "Arquitectura",
            Grupo::Medir => "Medir",
            Grupo::Edicion => "Edicion",
            Grupo::Bloques => "Bloques",
            Grupo::Vista => "Vista",
        }
    }
}

pub struct Orden {
    pub id: Id,
    /// Como se llama (y como sale en la linea de ordenes).
    pub nombre: &'static str,
    /// Lo que tambien vale escribir (en mayusculas).
    pub alias: &'static [&'static str],
    pub grupo: Grupo,
    /// Para el globo del boton.
    pub ayuda: &'static str,
    /// Si trabaja sobre objetos elegidos (los pide si no hay).
    pub con_objetos: bool,
}

macro_rules! o {
    ($id:ident, $n:literal, [$($a:literal),*], $g:ident, $obj:literal, $ay:literal) => {
        Orden { id: Id::$id, nombre: $n, alias: &[$($a),*], grupo: Grupo::$g, ayuda: $ay, con_objetos: $obj }
    };
}

pub const ORDENES: &[Orden] = &[
    o!(
        Linea,
        "LINEA",
        ["L", "LINE"],
        Dibujo,
        false,
        "Lineas sueltas punto a punto"
    ),
    o!(
        Polilinea,
        "POLILINEA",
        ["PL", "PLINE"],
        Dibujo,
        false,
        "Una polilinea de varios tramos"
    ),
    o!(
        Rectangulo,
        "RECTANGULO",
        ["REC", "RECTANG"],
        Dibujo,
        false,
        "Un rectangulo por dos esquinas"
    ),
    o!(
        Circulo,
        "CIRCULO",
        ["C", "CIRCLE"],
        Dibujo,
        false,
        "Un circulo por centro y radio"
    ),
    o!(
        Arco,
        "ARCO",
        ["A", "ARC"],
        Dibujo,
        false,
        "Un arco por tres puntos"
    ),
    o!(
        Texto,
        "TEXTO",
        ["T", "TX", "DT", "TEXT"],
        Anotar,
        false,
        "Un texto de una linea"
    ),
    o!(
        Mover,
        "MOVER",
        ["M", "MOVE", "DESPLAZA"],
        Modificar,
        true,
        "Mover lo elegido"
    ),
    o!(
        Copiar,
        "COPIAR",
        ["CO", "CP", "COPY"],
        Modificar,
        true,
        "Copiar lo elegido"
    ),
    o!(
        Girar,
        "GIRAR",
        ["GI", "RO", "ROTATE"],
        Modificar,
        true,
        "Girar lo elegido alrededor de un punto"
    ),
    o!(
        Escalar,
        "ESCALA",
        ["SC", "ES", "SCALE"],
        Modificar,
        true,
        "Agrandar o achicar lo elegido"
    ),
    o!(
        Simetria,
        "SIMETRIA",
        ["MI", "SI", "MIRROR"],
        Modificar,
        true,
        "Reflejar lo elegido"
    ),
    o!(
        Desfase,
        "DESFASE",
        ["O", "DE", "OFFSET", "EQUIDISTANCIA"],
        Modificar,
        false,
        "Una copia paralela a una distancia"
    ),
    o!(
        Recortar,
        "RECORTAR",
        ["TR", "RE", "TRIM"],
        Modificar,
        false,
        "Cortar lo que sobra hasta lo que cruza"
    ),
    o!(
        Alargar,
        "ALARGAR",
        ["EX", "AL", "EXTEND"],
        Modificar,
        false,
        "Alargar hasta lo siguiente"
    ),
    o!(
        Empalme,
        "EMPALME",
        ["F", "EM", "FILLET"],
        Modificar,
        false,
        "Unir dos lineas en esquina o con un arco"
    ),
    o!(
        Chaflan,
        "CHAFLAN",
        ["CHA", "CH", "CHAMFER"],
        Modificar,
        false,
        "Cortar una esquina en diagonal"
    ),
    o!(
        MatrizRect,
        "MATRIZ",
        ["AR", "ARRAY", "ARRAYRECT"],
        Modificar,
        true,
        "Copias en filas y columnas"
    ),
    o!(
        MatrizPolar,
        "MATRIZPOLAR",
        ["MP", "ARRAYPOLAR"],
        Modificar,
        true,
        "Copias alrededor de un centro"
    ),
    o!(
        Borrar,
        "BORRAR",
        ["E", "ERASE", "SUPR"],
        Modificar,
        true,
        "Borrar lo elegido"
    ),
    o!(
        Descomponer,
        "DESCOMPONER",
        ["X", "DESC", "EXPLODE"],
        Modificar,
        true,
        "Desarmar en piezas simples"
    ),
    o!(
        Cota,
        "COTA",
        ["COT"],
        Anotar,
        false,
        "Una cota alineada con sus marcas"
    ),
    o!(
        Nivel,
        "NIVEL",
        ["NPT", "NIV"],
        Anotar,
        false,
        "El nivel de piso terminado"
    ),
    o!(
        Norte,
        "NORTE",
        ["NO", "NORTH"],
        Anotar,
        false,
        "La flecha del norte"
    ),
    o!(
        Muro,
        "MURO",
        ["MU", "WALL"],
        Arquitectura,
        false,
        "Muros por su eje, con su espesor"
    ),
    o!(
        Ejes,
        "EJES",
        ["EJ", "GRID"],
        Arquitectura,
        false,
        "La malla de ejes con sus burbujas"
    ),
    o!(
        Puerta,
        "PUERTA",
        ["PU", "DOOR"],
        Arquitectura,
        false,
        "Una puerta con su giro"
    ),
    o!(
        Ventana,
        "VENTANA",
        ["VE", "WINDOW"],
        Arquitectura,
        false,
        "Una ventana en un muro"
    ),
    o!(
        Lamina,
        "LAMINA",
        ["LA", "MEMBRETE", "SHEET"],
        Anotar,
        false,
        "El marco y el membrete a escala"
    ),
    o!(
        Distancia,
        "DISTANCIA",
        ["DI", "DIS", "DIST"],
        Medir,
        false,
        "Medir entre dos puntos"
    ),
    o!(
        Area,
        "AREA",
        ["AA", "AR2"],
        Medir,
        false,
        "Area y perimetro"
    ),
    o!(
        Deshacer,
        "DESHACER",
        ["U", "H", "UNDO"],
        Edicion,
        false,
        "Deshacer la ultima orden"
    ),
    o!(
        Rehacer,
        "REHACER",
        ["REDO", "RH"],
        Edicion,
        false,
        "Rehacer"
    ),
];

/// Las ordenes de [`crate::herramientas`], detras de las de esta lista.
fn todas() -> impl Iterator<Item = &'static Orden> {
    ORDENES
        .iter()
        .chain(crate::herramientas::catalogo().iter().map(|r| &r.orden))
}

pub fn de(id: Id) -> &'static Orden {
    todas()
        .find(|o| o.id == id)
        .expect("toda orden esta en la lista")
}

/// La orden por su nombre o alias (sin importar mayusculas ni tildes).
pub fn buscar(texto: &str) -> Option<&'static Orden> {
    let t = sin_tildes(texto.trim()).to_uppercase();
    if t.is_empty() {
        return None;
    }
    todas().find(|o| o.nombre == t || o.alias.contains(&t.as_str()))
}

/// Las ordenes que empiezan por lo escrito (para el autocompletado).
pub fn sugerencias(texto: &str) -> Vec<&'static Orden> {
    let t = sin_tildes(texto.trim()).to_uppercase();
    if t.is_empty() {
        return Vec::new();
    }
    todas()
        .filter(|o| o.nombre.starts_with(&t) || o.alias.iter().any(|a| a.starts_with(&t)))
        .collect()
}

/// **Lo que sale en la pestana `g`** de la cinta, en su orden.
pub fn de_grupo(g: Grupo) -> Vec<Id> {
    let propias: &[Id] = match g {
        Grupo::Dibujo => &[
            Id::Linea,
            Id::Polilinea,
            Id::Rectangulo,
            Id::Circulo,
            Id::Arco,
            Id::Texto,
        ],
        Grupo::Modificar => &[
            Id::Mover,
            Id::Copiar,
            Id::Girar,
            Id::Escalar,
            Id::Simetria,
            Id::Desfase,
            Id::Recortar,
            Id::Alargar,
            Id::Empalme,
            Id::Chaflan,
            Id::MatrizRect,
            Id::MatrizPolar,
            Id::Descomponer,
            Id::Borrar,
        ],
        Grupo::Arquitectura => &[Id::Muro, Id::Ejes, Id::Puerta, Id::Ventana],
        Grupo::Anotar => &[Id::Cota, Id::Texto, Id::Nivel, Id::Norte, Id::Lamina],
        Grupo::Medir => &[Id::Distancia, Id::Area],
        Grupo::Edicion | Grupo::Bloques | Grupo::Vista => &[],
    };
    let mut v = propias.to_vec();
    v.extend(
        crate::herramientas::catalogo()
            .iter()
            .filter(|r| r.orden.grupo == g)
            .map(|r| r.orden.id),
    );
    v
}

/// **El nombre bonito** de una orden, con sus tildes (el de la cinta).
pub fn etiqueta(id: Id) -> &'static str {
    match id {
        Id::Linea => "Línea",
        Id::Polilinea => "Polilínea",
        Id::Rectangulo => "Rectángulo",
        Id::Circulo => "Círculo",
        Id::Arco => "Arco",
        Id::Texto => "Texto",
        Id::Mover => "Mover",
        Id::Copiar => "Copiar",
        Id::Girar => "Girar",
        Id::Escalar => "Escala",
        Id::Simetria => "Simetría",
        Id::Desfase => "Desfase",
        Id::Recortar => "Recortar",
        Id::Alargar => "Alargar",
        Id::Empalme => "Empalme",
        Id::Chaflan => "Chaflán",
        Id::MatrizRect => "Matriz",
        Id::MatrizPolar => "M. polar",
        Id::Borrar => "Borrar",
        Id::Descomponer => "Descomponer",
        Id::Cota => "Cota",
        Id::Nivel => "Nivel",
        Id::Norte => "Norte",
        Id::Muro => "Muro",
        Id::Ejes => "Ejes",
        Id::Puerta => "Puerta",
        Id::Ventana => "Ventana",
        Id::Lamina => "Lámina",
        Id::Distancia => "Distancia",
        Id::Area => "Área",
        Id::Deshacer => "Deshacer",
        Id::Rehacer => "Rehacer",
        Id::Otra(n) => crate::herramientas::catalogo()
            .iter()
            .find(|r| r.orden.nombre == n)
            .map_or(n, |r| r.etiqueta),
    }
}

pub fn sin_tildes(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'Á' => 'a',
            'é' | 'É' => 'e',
            'í' | 'Í' => 'i',
            'ó' | 'Ó' => 'o',
            'ú' | 'Ú' | 'ü' | 'Ü' => 'u',
            'ñ' => 'n',
            'Ñ' => 'N',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn alias_de_aqui_y_de_autocad() {
        assert_eq!(buscar("l").unwrap().id, Id::Linea);
        assert_eq!(buscar("TR").unwrap().id, Id::Recortar);
        assert_eq!(buscar("simetría").unwrap().id, Id::Simetria);
        assert_eq!(buscar("offset").unwrap().id, Id::Desfase);
        assert!(buscar("nada").is_none());
        assert!(buscar("").is_none());
    }

    #[test]
    fn ningun_alias_repetido() {
        let mut vistos = std::collections::HashSet::new();
        for o in todas() {
            assert!(vistos.insert(o.nombre), "{}", o.nombre);
            for a in o.alias {
                assert!(vistos.insert(a), "alias repetido {a}");
            }
        }
    }

    #[test]
    fn sugiere_por_el_principio() {
        let s: Vec<_> = sugerencias("re").iter().map(|o| o.id).collect();
        assert!(
            s.contains(&Id::Rectangulo) && s.contains(&Id::Recortar) && s.contains(&Id::Rehacer)
        );
    }
}
