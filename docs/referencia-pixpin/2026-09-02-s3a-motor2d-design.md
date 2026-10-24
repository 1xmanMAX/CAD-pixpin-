# S3-A · `pixpin-motor2d`: el motor de edición avanzada en 2D

**Fecha:** 2026-09-02 · **Estado:** aprobado (decisiones auto-aprobadas bajo la
autorización del usuario del 2026-09-01) · **Fase:** S3-A del plan maestro.

**Fuente de estudio:** `docs/investigacion/2026-09-02-excalidraw-analisis.md`
(1157 líneas, análisis del original hecho para reimplementar sin volver a
mirarlo). Excalidraw es **MIT**: sus algoritmos se leen y se adaptan. Lo que se
construye aquí es un motor **nuevo**, en Rust sobre Direct2D — no una copia ni
una extensión de un proyecto ajeno.

## 1. Qué es y para qué

Un motor de dibujo vectorial 2D, sin interfaz, que los demás lo usan:

- **S3-B** — anotar *dentro* de un pin (doble clic entra en modo edición).
- **S3-C** — anotar *sobre la pantalla*, congelada o en capa viva.
- **S6** — anotar PDF, reutilizando exactamente el mismo motor.

Por eso es un crate propio y no código dentro del pin: tres consumidores muy
distintos, una sola verdad sobre qué es un trazo.

## 2. Decisiones

Continúan la numeración del proyecto (D1-D35).

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D36 | **Alcance del motor** | Modelo de elementos + geometría + hit-test + serialización. **Sin ventana, sin bucle de eventos, sin herramientas de UI**: eso es de S3-B/S3-C | Un motor que abre ventanas no se puede meter dentro de un pin, ni de un PDF |
| D37 | **Capa** | **L1**, y **sin dependencia de `pixpin-render`**: el motor produce polígonos y quien dibuja es el consumidor, que ya tiene pintor | Corregido al implementar: `pixpin-render` es L1 igual que el motor, y la regla del proyecto prohíbe que una capa dependa de sí misma (`apps/pixpin/tests/capas.rs` lo habría rechazado). El resultado es además mejor: el motor queda **100 % puro y probable sin GPU**, y el mismo polígono vale para Direct2D hoy y para exportar a SVG mañana |
| D38 | **Determinismo del «hecho a mano»** | LCG de Lehmer replicado **bit a bit** (`seed = seed.wrapping_mul(48271) & 0x7FFF_FFFF`, normalizado dividiendo por `2^31`), una instancia **por elemento** sembrada con su `semilla` | El mismo dibujo debe salir idéntico al reabrirlo, hoy y dentro de diez años. Es además el mismo generador que el Android replica en `Rand.kt` |
| D39 | **Trazo a mano alzada** | Algoritmo de `perfect-freehand` portado (puntos suavizados → contorno con radio por presión → polígono cerrado), con las constantes exactas del informe | Es lo que hace que un trazo parezca tinta y no un cable |
| D40 | **Presión** | Simulada a partir de la velocidad (`RATE_OF_PRESSURE_CHANGE = 0.275`), con la puerta abierta a presión real de tableta más adelante | Casi nadie anota con tableta; la simulación es lo que se ve el 99 % del tiempo |
| D41 | **Formato** | `.pixpin2d`, **JSON** con la misma disciplina que el resto del proyecto (`serde(default)`, claves desconocidas ignoradas, temporal+rename) | Coherente con `indice.json` y los ajustes; legible y diffeable. Un importador de `.excalidraw` es trivial de añadir después y no bloquea nada |
| D42 | **Sin dependencias nuevas** | Vectores 2D propios (una docena de funciones triviales), sin `glam` ni `lyon` | El proyecto tiene 4 dependencias por crate como mucho, y esto son 40 líneas |
| D43 | **Rasterización** | `ID2D1PathGeometry` construida una vez por elemento y **cacheada**; se reconstruye solo si el elemento cambia | Es el equivalente al «no repintar al mover» del pin: en el suelo (i3, HD 4000) redibujar cien trazos por fotograma no cabe |
| D44 | **Elementos v1** | Lápiz, resaltador, línea, flecha, rectángulo, elipse, texto, imagen. **Sin** diamante, ni conectores con enrutado, ni marcos, ni bucket-fill | Son las herramientas que el usuario pidió; el resto es peso del original que no se ha pedido |
| D45 | **Resaltador** | Un lápiz con alfa 0,35 y modo de mezcla multiplicar, grosor ×3, **sin** rugosidad | Es la única forma de que resaltar sobre texto lo deje legible |

## 3. El modelo

```rust
pub struct Elemento {
    pub id: u64,
    pub tipo: TipoElemento,       // la geometria concreta
    pub x: f32, pub y: f32,       // esquina superior izquierda de su caja
    pub ancho: f32, pub alto: f32,
    pub angulo: f32,              // radianes, sentido horario
    pub trazo: Color,             // color de linea
    pub relleno: Option<Color>,
    pub grosor: f32,              // px logicos
    pub estilo_trazo: EstiloTrazo,      // Solido | Discontinuo | Punteado
    pub rugosidad: f32,           // 0 = regla, 1 = a mano, 2 = muy a mano
    pub opacidad: f32,            // 0..1
    pub semilla: u32,             // D38: el "hecho a mano" es reproducible
    pub version: u32,             // sube en cada cambio: invalida el cache
    pub borrado: bool,            // borrado logico, para deshacer barato
}

pub enum TipoElemento {
    Lapiz { puntos: Vec<Punto2>, presiones: Vec<f32> },
    Resaltador { puntos: Vec<Punto2> },
    Linea { puntos: Vec<Punto2> },
    Flecha { puntos: Vec<Punto2>, punta_inicio: bool, punta_fin: bool },
    Rectangulo,
    Elipse,
    Texto { texto: String, tam: f32, familia: String },
    Imagen { id_objeto: u64 },    // el bitmap lo aporta el consumidor
}
```

`Escena` es la lista de elementos más el contador de ids. Nada más: la
selección, el zoom y la herramienta activa son estado de la *interfaz*, no del
documento (esa es la lección del `appState` de Excalidraw, del que solo cinco
claves sobreviven al fichero).

## 4. Los módulos

| Módulo | Qué hace | Puro |
|---|---|---|
| `azar.rs` | El LCG de D38 y `Rugoso`, que ofrece «desplaza este punto como lo haría una mano» | sí |
| `vector.rs` | Los 12 auxiliares 2D (`per`, `dpr`, `uni`, `lrp`, `prj`, `rot`…) | sí |
| `trazo.rs` | El pipeline de D39: puntos crudos → suavizados → contorno → polígono | sí |
| `formas.rs` | Rectángulo, elipse, línea y flecha «a mano»: dos pasadas con el LCG, como el original | sí |
| `elemento.rs` | El modelo, la caja envolvente y las mutaciones que suben `version` | sí |
| `escena.rs` | La lista, el orden de dibujo, añadir/borrar/deshacer | sí |
| `impacto.rs` | Hit-test por tipo con tolerancia (spec §f del informe) | sí |
| `formato.rs` | `.pixpin2d`: serde tolerante, temporal+rename | sí |
| `pintado.rs` | Traduce un elemento a **órdenes de dibujo** (polígonos y polilíneas con su color): lo que el consumidor manda a su pintor | sí |

**Los nueve módulos son puros y se prueban en CI sin escritorio.** El motor no
abre una ventana ni toca la GPU: produce geometría, y `pixpin-render` (que el
pin y el PDF ya usan) la pinta. `Pintor` gana `poligono` y `polilinea`, que
reciben pares de `f32` y por eso no atan el renderizador a este crate.

## 5. Las pruebas que importan

- **Determinismo (la puerta de la fase):** dibujar el mismo elemento dos veces
  con la misma semilla da **exactamente** los mismos puntos, y con semilla
  distinta da puntos distintos. Sin esto, un dibujo cambia de aspecto cada vez
  que se abre.
- **El LCG, contra valores conocidos:** semilla 1 → la secuencia exacta del
  original. Es el test que garantiza la paridad con el Android.
- **Ida y vuelta del formato**, con un fichero de una versión futura (claves
  desconocidas) y otro al que le faltan campos.
- **Hit-test con casos negativos:** un punto a 3 px de una línea fina la toca
  (tolerancia); a 30 px, no. Dentro de una elipse sin relleno **no** la toca,
  sobre su borde sí.
- **Un trazo de un solo punto** produce un círculo, no un polígono vacío (el
  caso que rompe todas las implementaciones ingenuas).

## 6. Rendimiento

| Métrica | Objetivo |
|---|---|
| Trazo con 500 puntos: puntos → polígono | < 2 ms (suelo) |
| Redibujar 100 elementos cacheados | ≤ 1 fotograma |
| Escena de 1000 elementos en memoria | < 20 MB |
| Abrir un `.pixpin2d` de 1000 elementos | < 150 ms |

La regla de siempre: la velocidad sale de no rehacer trabajo (D43), no de
lenguajes más rápidos.

## 7. Lo que NO hace S3-A

Ventanas, herramientas, barra de herramientas, cursores, deshacer con interfaz,
colaboración, exportar a SVG/PNG (eso es S4), importar `.excalidraw`,
enrutado de conectores, bucket-fill, marcos. Cada una de esas cosas tiene su
fase o su decisión explícita de no existir.

## 8. Criterios de aceptación

- [ ] Los ocho tipos de elemento se crean, se mueven, se redimensionan y se dibujan
- [ ] El mismo fichero produce el mismo dibujo píxel a píxel, siempre
- [ ] El LCG coincide con el original en una secuencia de referencia
- [ ] Hit-test correcto para los ocho tipos, con sus casos negativos
- [ ] `.pixpin2d` sobrevive a la ida y vuelta y a versiones futuras
- [ ] Las cuatro puertas de §6 medidas y anotadas en `medidas/`
