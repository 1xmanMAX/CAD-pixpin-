# CAD-pixpin

**CAD-pixpin** es la rama de [PixPin Max](https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS)
dedicada a planos y modelos: abre **DWG, DXF, IFC y Revit**, y además de verlos y anotarlos,
**los edita**. Mantiene la línea de PixPin (Rust sobre lo que Windows ya trae, rápido en un
Core i3 con 4 GB, sin nube ni cuentas), pero cambia la forma de trabajar: en vez del chat por
proyectos, se abre en una **pantalla de inicio** y una **mesa de trabajo de CAD**.

| | PixPin Max | CAD-pixpin |
|---|---|---|
| Para qué | Capturar, fijar y anotar de todo | Dibujar y revisar planos y modelos |
| Al abrir | Chat por proyectos | Inicio: plano nuevo, abrir plano, abrir modelo BIM, recientes |
| Planos DWG/DXF | Ver, medir, anotar encima, imprimir | Todo eso **y editar**: órdenes de CAD, capas, guardar en DWG o DXF |
| Modelos BIM | Ver en 3D, cortar, niveles | Lo mismo, y **metrados por nivel y partida** a CSV |

## Qué hay dentro

### Copiado tal cual de PixPin Max

Con `cp -r`, sin tocar una línea (ver [`crates/LEEME.md`](crates/LEEME.md)):

- **`pixpin-cad`**: el visor de planos (Direct3D 11) y el de modelos 3D, la regla, los
  marcadores, las letras SHX y el puente para anotar encima. Lee con `opencadcodec`, la
  librería de **OpenCADStudio** (MPL-2.0), que también **escribe DWG y DXF**.
- **`pixpin-bim`**: IFC (ifc-lite), Revit `.rvt` (rvt-rs), LandXML de Civil 3D y niveles.
- **`pixpin-motor2d`, `pixpin-tinta`, `pixpin-ui`, `pixpin-render`**: el motor de anotado
  (figuras, flechas, cotas, textos, tinta) y Direct2D, y lo que necesitan para compilar.
- En [`referencia/`](referencia/LEEME.md), sin compilar, el código de la app PixPin que abre
  esos visores y el editor avanzado de anotado, para irlo portando.

### Nuevo: `cad-nucleo`, el núcleo de edición

Rust puro sobre `opencadcodec`: se prueba en Linux y en Windows. El dibujo es el
`CadDocument` tal cual, así que **al guardar no se pierde lo que no se tocó** (bloques,
cotas, sombreados, presentaciones).

- **Deshacer y rehacer** por orden (200 pasos), sobre las imágenes compartidas de cada entidad.
- **Índice espacial** para encontrar lo que hay bajo el cursor en planos grandes.
- **Enganches**: punto final, medio, centro, cuadrante, intersección, inserción,
  perpendicular y cercano; **orto**, **rastreo polar** cada 45° y **rejilla**.
- **Elegir** con clic, **ventana** (de izquierda a derecha: lo que cabe entero) y
  **captura** (de derecha a izquierda: lo que toca), como en AutoCAD. Las capas bloqueadas
  se ven y se enganchan pero no se eligen.
- **Pinzamientos**: estirar extremos, cambiar radios, mover vértices.
- **Línea de órdenes** con coordenadas `x,y`, `@dx,dy`, `@d<ángulo` y distancia directa.

#### Órdenes

| Grupo | Orden (alias) |
|---|---|
| Dibujo | LINEA (L), POLILINEA (PL), RECTANGULO (REC), CIRCULO (C), ARCO (A), TEXTO (T) |
| Modificar | MOVER (M), COPIAR (CO), GIRAR (GI, RO), ESCALA (SC), SIMETRIA (MI), DESFASE (O), RECORTAR (TR), ALARGAR (EX), EMPALME (F), CHAFLAN (CHA), MATRIZ (AR), MATRIZPOLAR (MP), DESCOMPONER (X), BORRAR (E, B) |
| Arquitectura | MURO (MU), EJES (EJ), PUERTA (PU), VENTANA (VE) |
| Anotar | COTA (COT, DAL), NIVEL (NPT), NORTE (NO), LAMINA (LA) |
| Medir | DISTANCIA (DI), AREA (AA) |
| Edición | DESHACER (U, Ctrl+Z), REHACER (Ctrl+Y) |

Los alias en inglés de AutoCAD también valen (`LINE`, `TRIM`, `OFFSET`, `FILLET`…).

#### Lo del oficio

Lo que en un CAD genérico son diez pasos:

- **Muro** por su eje y con su espesor (se escribe el espesor antes del primer punto),
  con esquinas a inglete y tapas.
- **Ejes**: la malla entera con sus burbujas (1, 2, 3… y A, B, C…, sin I ni O), escribiendo
  las separaciones: `5000,4500` o `3*6000`.
- **Puerta** con su hoja y su giro; **ventana** en el muro con su vidrio.
- **Cota** alineada con marcas a 45° y el texto siempre legible.
- **Nivel** de piso terminado (`NPT +0.15`), **flecha del norte**.
- **Lámina** A4 a A0 a cualquier escala, con margen para anillar y **membrete**
  (proyecto, plano, escala, autor, fecha, número de lámina).
- **Capas de plantilla** por especialidad: `A-MUROS`, `A-PUERTAS`, `A-VENTANAS`, `A-COTAS`,
  `A-EJES`, `A-NIVELES`, `E-ESTRUCTURAS`, `IE-ELECTRICAS`, `IS-SANITARIAS`, `L-LAMINA`…
  Cada herramienta dibuja en la suya.
- **Metrados**: del plano, por capa (cantidad, longitud en m, área de lo cerrado en m²); de
  un modelo IFC o Revit, por nivel y partida (muros, losas, columnas, vigas…: cantidad,
  volumen en m³, áreas en planta y en alzado). Salen en CSV para la hoja del presupuesto.

### Nuevo: `apps/cadpixpin`, la app de Windows

- **Inicio**: plano nuevo, abrir plano, abrir modelo BIM, metrado BIM, recientes y las
  órdenes de siempre. Se puede arrastrar un archivo a la ventana.
- **Mesa de trabajo**: el plano en el centro (con la GPU del visor de PixPin; se vuelve a
  dibujar en otro hilo con cada cambio), la cinta por pestañas (Dibujo, Modificar,
  Arquitectura, Anotar, Medir), capas y propiedades a la derecha, y abajo la línea de
  órdenes con autocompletado y la barra de estado.
- Cursor en cruz, símbolos de enganche, vista previa de cada orden, recuadro azul o verde.
- Los modelos BIM se abren en la **ventana 3D de PixPin** (órbita, caja de sección, niveles,
  medidas de cada pieza).

#### Teclado y ratón

| | |
|---|---|
| Escribir + `Enter` o `Espacio` | La orden, un punto, un número o una opción |
| `Enter` / `Espacio` sin nada | Repetir la última orden |
| Botón derecho | Enter |
| `Esc` | Cancelar la orden o soltar lo elegido |
| `Supr` | Borrar lo elegido |
| Rueda / botón central | Zoom al cursor / mover la vista; doble clic central: ver todo |
| `F3` `F8` `F9` `F10` | Enganche, orto, rejilla, polar |
| `Ctrl+S`, `Ctrl+Mayús+S`, `Ctrl+O`, `Ctrl+N` | Guardar, guardar como, abrir, nuevo |
| `Ctrl+A` | Elegir todo |

## Compilar y probar

Rust 1.97.1 (fijado en `rust-toolchain.toml`), Windows 10 21H2 u 11.

```sh
cargo run -p cadpixpin --release            # la app
cargo run -p cadpixpin --release -- plano.dwg
cargo test -p cad-nucleo                     # el núcleo, en cualquier sistema
cargo test --workspace -- --test-threads=1   # todo, en Windows
```

La CI (`.github/workflows/ci.yml`) comprueba el formato, prueba el núcleo en Linux, pasa
Clippy y las pruebas en Windows y deja el `cadpixpin.exe` como artefacto.

## Proyectos de código abierto en los que se apoya

- **OpenCADStudio** — su librería `opencadcodec` (MPL-2.0) lee y escribe DWG y DXF y trae
  las transformaciones de cada entidad. Se usa como dependencia; de la aplicación (GPL) no
  se copia código.
- **ifc-lite** (MPL-2.0) y **rvt-rs** (Apache-2.0) para IFC y Revit, como en PixPin.
- **FreeCAD** y su entorno BIM, y **AutoCAD**, como referencia de cómo se trabaja (los
  nombres de las órdenes, la ventana y la captura, los enganches); no se copia su código.

Ver [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

## Lo que sigue

- Cotas como entidades `DIMENSION` de verdad (hoy van desmontadas en líneas y texto, que
  se ven igual en cualquier programa pero no se actualizan solas).
- Sombreados, insertar bloques y bibliotecas de bloques (mobiliario, aparatos sanitarios).
- Portar desde `referencia/` el anotado con el motor del lienzo, imprimir láminas y
  exportar a PDF.
- Propiedades BIM: leer y editar los `Pset` de cada elemento del IFC.

## Licencia

MIT, como PixPin Max. Los crates copiados conservan su licencia y las dependencias la suya.
