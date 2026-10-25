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
| Dibujo | LINEA (L), POLILINEA (PL), RECTANGULO (REC), CIRCULO (C), ARCO (A), TEXTO (T), PUNTO (PO), POLIGONO (POL), ELIPSE (EL), DIVIDIR (DIV), GRADUAR (ME) |
| Modificar | MOVER (M), COPIAR (CO), GIRAR (GI, RO), ESCALA (SC), SIMETRIA (MI), DESFASE (O), RECORTAR (TR), ALARGAR (EX), EMPALME (F), CHAFLAN (CHA), MATRIZ (AR), MATRIZPOLAR (MP), DESCOMPONER (X), BORRAR (E), ESTIRAR (S), PARTIR (BR), UNIR (J), LONGITUD (LEN), ALINEAR (ALI), IGUALAR (MA), EDITARPOL (PE) |
| Arquitectura | MURO (MU), EJES (EJ), PUERTA (PU), VENTANA (VE) |
| Anotar | COTALINEAL (DLI), COTAALINEADA (DAL), CONTINUA (DCO), LINEABASE (DBA), RADIO (DRA), DIAMETRO (DDI), ANGULAR (DAN), ESCALACOTA (EC), COTA desmontada (COT), MTEXTO (MT), EDITARTEXTO (ED), ALTOTEXTO (AT), NIVEL (NPT), NORTE (NO), LAMINA (LA) |
| Bloques | BLOQUE (B), INSERTAR (I), BIBLIOTECA (BIB), SOMBREADO (SO, HATCH) |
| Medir | DISTANCIA (DI), AREA (AA) |
| Vista y capas | ZOOM (Z), ZE, CAPA (CA), LAYISO, LAYUNISO, LAYOFF, LAYON, LAYMCUR, LAYLCK, LAYULK |
| Selección | SS (similares), SP (previa), SLC (por capa), INV (invertir) |
| Edición | DESHACER (U, Ctrl+Z), REHACER (Ctrl+Y), portapapeles (Ctrl+C, Ctrl+X, Ctrl+V) |

Cada orden nueva vive en su módulo de `crates/cad-nucleo/src/herramientas/` (el trait
`Herramienta`): añadir una no toca la sesión, y la cinta la saca sola por su grupo.

#### Bloques, sombreados y cotas

- **BLOQUE** convierte lo elegido en un bloque con su punto base; **INSERTAR** lo pone con
  escala y giro. La **biblioteca** trae 19 piezas en milímetros: inodoro, lavatorio, ducha,
  tina, lavadero, cocina, refrigeradora, camas, mesa con sillas, sofá, escritorio, puerta,
  tomacorriente, interruptor, luminaria, sumidero, registro y columna 30×30.
- **SOMBREADO** con un clic dentro: busca el contorno cerrado más pequeño alrededor (también
  entre líneas sueltas que se cruzan, con islas), con SÓLIDO, ANSI31, ANSI37 y ARENA. Cierra
  los huecos pequeños entre extremos (opción `T`; por defecto una diezmilésima del dibujo) y
  queda **asociativo**: sigue a sus bordes al moverlos o estirarlos.
- **Cotas de verdad** (entidades `DIMENSION` con su bloque, como las de AutoCAD): lineal,
  alineada, continua, línea base, radio, diámetro y angular, con marcas de arquitectura y un
  estilo `PIXPIN` a la escala del dibujo (y sus hijos `PIXPIN$2`/`$3`/`$4` con flechas para
  ángulos, diámetros y radios). Son **asociativas** (como DIMASSOC=2): siguen a lo medido al
  moverlo, estirarlo, girarlo o con un pinzamiento, en el mismo paso de deshacer; se guarda
  en su XDATA y vale tras abrir el DWG o DXF.
- Deshacer BLOQUE, INSERTAR una pieza nueva o una cota quita también su definición (no
  quedan bloques huérfanos).

#### Rendimiento en planos grandes

Pensado para 200 000 a 500 000 objetos en un Core i3 de 3.ª generación:

- El **índice espacial** se pone al día solo con lo que cambió (`Dibujo::cambios_desde`):
  mover una línea en un plano de 220 000 objetos pasa de 75 ms a 0,1 ms. Apagar, inutilizar
  o bloquear una capa solo pone al día lo de esa capa (en el índice y en la tarjeta), sin
  rehacerlo todo: con 55 000 objetos en la capa, unos 17 ms apagarla y 6 ms bloquearla
  frente a 100 ms del índice entero.
- Enganches, clic, ventana y captura miran solo las celdas cercanas (de microsegundos a
  1–2 ms con todo el plano en el recuadro); la vista previa de mover o girar todo lo elegido
  transforma las formas del índice (de 157 ms a unos 3 ms).
- En la tarjeta gráfica el plano va **por cubos** de unos 4 000 objetos, convertidos en
  varios hilos: una edición vuelve a convertir solo los cubos que toca, y lo de antes sigue
  a la vista mientras tanto.
- El resaltado de lo elegido se guarda por versión y solo pinta lo visible; los
  movimientos del ratón seguidos se juntan en uno.

Las pruebas de tiempos: `cargo test -p cad-nucleo --release -- --ignored rendimiento --nocapture`.

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
  Arquitectura, Anotar, Bloques, Medir, Vista), capas y propiedades a la derecha, y abajo la línea de
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

#### Atajos

Para trabajar a gusto en planos grandes:

| Tecla u orden (alias) | Qué hace |
|---|---|
| ZOOM (`Z`) | Opciones `Extension` (`E`, o Enter), `Ventana` (`V`, o un clic directo: dos esquinas con su recuadro), `Previo` (`P`, las 20 últimas vistas), `Todo` (`T`, con los límites del dibujo) o un factor (`2`, `0.5`, `2x`) |
| ZOOMEXTENSION (`ZE`) | Ver todo lo dibujado, sin preguntar |
| `Ctrl+C`, `Ctrl+X` | Copiar / cortar lo elegido al portapapeles (COPIARPORTAPAPELES `COPYCLIP`, CORTARPORTAPAPELES `CUTCLIP`); la base es la esquina inferior izquierda |
| `Ctrl+Mayús+C` | Copiar con un punto base (COPIABASE, `COPYBASE`) |
| `Ctrl+V` | Pegar donde se haga clic, viendo dónde cae (PEGARPORTAPAPELES, `PEGAR`, `PASTECLIP`). Vale entre planos abiertos uno tras otro: las capas que falten se crean |
| CAPA (`CA`, `LAYER`) | Hacer actual una capa escribiendo su nombre, `Nueva` (`N`) para crearla o `Actual` (`A`). El **+** del panel de capas empieza una nueva |
| CAPAAISLAR (`LAYISO`) / CAPADESAISLAR (`LAYUNISO`) | Apagar todo menos las capas de lo elegido / volver a encenderlo |
| CAPAOFF (`LAYOFF`) / CAPAON (`LAYON`) | Apagar las capas de lo elegido (la actual no) / encender todas |
| CAPAACT (`LAYMCUR`) | La capa de un objeto pasa a ser la actual |
| CAPABLOQ (`LAYLCK`) / CAPADESBLOQ (`LAYULK`) | Bloquear las capas de lo elegido / desbloquear con un clic en lo bloqueado (o su nombre) |
| SELECCIONARSIMILAR (`SS`, `SELECTSIMILAR`) | Elegir todo lo del mismo tipo y capa que lo elegido |
| SELECCIONPREVIA (`SP`, `SELECTPREVIOUS`) | Volver a elegir lo de la última orden |
| SELECCIONARCAPA (`SLC`, `SELECTLAYER`) | Elegir todo lo de una capa: un clic en un objeto, su nombre o Enter para la actual |
| INVERTIRSELECCION (`INV`, `SELECTINVERSE`) | Elegir lo que no está elegido |
| `Ctrl+Mayús+Z` | Rehacer (como `Ctrl+Y`) |
| `F2` | Historial de la línea de órdenes alto (12 líneas) o bajo |
| `F7` | Pintar la rejilla (el paso de `F9`, o uno redondo a la escala de la vista) |
| `F11` | Pantalla completa |
| Rueda sobre las capas | Mover la lista; a la derecha de cada capa, cuántos objetos tiene |
| Doble clic en un texto | Editarlo (EDITARTEXTO, si está) |

**Autoguardado**: cada 5 minutos, si hay cambios sin guardar, una copia en DXF en
`%APPDATA%\CAD-pixpin\autoguardado\` (en otro hilo; la variable `CADPIXPIN_AUTOGUARDADO`
cambia los minutos y `0` lo apaga). Al guardar, o al cerrar sin guardar, la copia se borra.
Si la app se cerró de golpe, el inicio ofrece **Recuperar** o **Descartar** cada copia más
nueva que su plano.

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

- Portar desde `referencia/` el anotado con el motor del lienzo, imprimir láminas y
  exportar a PDF.
- Propiedades BIM: leer y editar los `Pset` de cada elemento del IFC.

## Licencia

MIT, como PixPin Max. Los crates copiados conservan su licencia y las dependencias la suya.
