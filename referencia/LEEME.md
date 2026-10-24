# referencia/ — el pegamento de la app PixPin Max

Copia tal cual (commit `831a7b6` de PIXPIN_PRO_WINDOWS) de los módulos de
`apps/pixpin/src` que abren el visor de planos, el de modelos BIM y el
editor avanzado de anotado:

- `plano_cad.rs`, `plano_cad/` — abrir un DWG/DXF en otro proceso, cache,
  anotar encima (`anotar.rs`), exportar (HTML, PDF, PNG, DXF), imprimir láminas.
- `modelo_bim.rs`, `croquis3d.rs` — abrir IFC / Revit / LandXML y el croquis 3D.
- `lector.rs` — el lector que decide qué visor abre cada fichero.
- `editor.rs`, `ventana_editor*`, `dibujo/`, `panel_dibujo.rs`, `caja_dibujo.rs`
  — el editor avanzado de anotado (figuras, cotas, medir, lupas, exportar).

**No se compila**: depende del resto de la app (chat, almacén, sincronizar).
Está aquí para portar sus piezas a `apps/cadpixpin` poco a poco, sin tener
que ir a buscarlas al otro repositorio.
