# El editor avanzado — el armazón

**Fecha:** 2026-09-06
**Estado:** diseño aprobado por el usuario, sección a sección
**Alcance:** la máquina del editor 2D. No las herramientas nuevas (eso es la
Fase B del plan `2026-09-06-android-a-windows.md`), no el tacto del trazo.

---

## 1. Por qué esto y por qué ahora

PixPin Max sabe anotar. No sabe **editar**: no se puede seleccionar más de un
elemento, no se puede redimensionar ninguno, no se puede girar ninguno, y de
las cosas que se hacen solo tres se pueden deshacer. Eso no es una carencia de
herramientas, es una carencia de armazón — y mientras no exista, cada
herramienta nueva se construye torcida y hay que rehacerla después.

El Android tiene ese armazón desde hace años. Su motor son 55.385 líneas
contra nuestras 4.179, pero **31.718 de ellas no importan Android** y son
geometría pura, portable tal cual.

### Lo que hay hoy, medido

| Fichero | Líneas | Qué hace |
|---|---:|---|
| `camara.rs` | 422 | Mundo/pantalla, zoom, recorte |
| `pintado.rs` | 678 | Geometría de dibujo, nivel de detalle |
| `excalidraw.rs` | 590 | El puente con el móvil |
| `escena.rs` | 452 | La lista y un historial de tres variantes |
| `trazo.rs` | 375 | Contorno del lápiz |
| `elemento.rs` | 345 | El modelo |
| `formas.rs` | 303 | Rectángulo, elipse, flecha |
| `impacto.rs` | 294 | Picado, un elemento |
| `formato.rs` | 201 | Cargar y guardar |
| `aligerar.rs` | 170 | Ramer–Douglas–Peucker |
| `azar.rs` | 155 | Semilla reproducible |
| `vector.rs` | 150 | Punto y distancias |

Cimiento bueno, y ya pensado para equipos flojos. Le falta el piso de arriba.

### Los cuatro agujeros

| Hoy | Consecuencia |
|---|---|
| `elemento_en()` devuelve un `Option<u64>` | No existe seleccionar varios |
| `Cambio` tiene 3 variantes | Redimensionar, girar, cambiar de color y editar un texto **no se deshacen** |
| No hay una sola línea de redimensionado de elementos | Lo único que existe es `escalar_anclado`, que redimensiona la *ventana* del pin |
| `Elemento` no tiene `grupos` | El puente conserva los `groupIds` del móvil en el JSON, pero Windows no puede actuar sobre ellos |

---

## 2. Decisiones

Continúan la numeración del documento de rendimiento (D1–D19).

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D20 | **Estrategia de porte** | **Portar la geometría, rediseñar el gesto** | Los algoritmos son verdad matemática y valen igual; `DrawController.kt` (3.482 l.) es en buena parte gestión de dos, tres y cuatro dedos, que aquí no existen |
| D21 | **Dónde vive el editor** | **Ventana propia**, y el anotador y el pin se quedan como están | Lo pesado no debe lastrar lo rápido. El anotar rápido tiene un plazo sagrado de 50 ms que un editor completo pone en riesgo |
| D22 | **Dónde vive la máquina del gesto** | En `pixpin-motor2d`, no en `pixpin-ui` | Permite probar «pulsar, mover con Shift, soltar» sin abrir una ventana, y medir el camino caliente en la puerta automática |
| D23 | **Qué devuelve la máquina del gesto** | Toca la escena; **devuelve lo que la ventana debe hacer** | Un registro de órdenes pendientes en paralelo sería una segunda verdad sobre el mismo dibujo |
| D24 | **Forma del historial** | **Transacciones** con el estado anterior guardado entero | En coma flotante, `(a × 1,5) ÷ 1,5` no siempre devuelve `a`: deshacer paramétrico deforma el dibujo poco a poco |
| D25 | **Techo del historial** | **8 MB**, no un número de pasos | En 4 GB, un techo contado en pasos es una promesa incumplible; contado en megas, se cumple |
| D26 | **Estructura de selección** | `Vec<u64>` reutilizado, no `HashSet` | De uno a veinte elementos: la búsqueda lineal gana al hash, y `clear()` no reasigna. Arrastrar una marquesina es camino caliente |
| D27 | **Índice espacial** | **Rejilla uniforme**, no quadtree ni R-tree | Con 8.000 elementos la rejilla ya gana, no reasigna al consultar, y se demuestra correcta contra la fuerza bruta |
| D28 | **Tecnología de dibujo** | **Ninguna nueva**: Direct2D, como hasta ahora | wgpu/Skia/egui añaden megas y una traducción más. Lo que hace ligero al programa es no dibujar cuando no hay nada nuevo |
| D29 | **Marquesina** | Selecciona lo que queda **enteramente dentro** | Con trazos largos, «lo que toque» selecciona cosas que el usuario no ve venir |
| D30 | **Tamaño mínimo al escalar** | Por debajo del mínimo, **se voltea** | Aplastar a cero pierde información sin remedio; voltear es reversible y es lo que espera quien cruzó el ratón |

---

## 3. Dónde vive cada cosa

Lo puro primero, la ventana después. El armazón entero es geometría y
estados: nada necesita una pantalla para probarse.

```
pixpin-motor2d  (L0, puro, #![forbid(unsafe_code)])
├── escena.rs        ← historial de transacciones y selección múltiple
├── impacto.rs       ← picado exacto con caja girada
├── elemento.rs      ← + campo `grupos`
├── transformar.rs   ← NUEVO  mover / escalar / girar con ancla
├── tiradores.rs     ← NUEVO  los 8 + el de giro
├── organizar.rs     ← NUEVO  orden, grupos, alinear, distribuir
├── gesto.rs         ← NUEVO  la máquina de estados de ratón y teclado
└── indice.rs        ← NUEVO  rejilla espacial

pixpin-shell
└── overlay.rs       ← + `alt: bool` en `Tecla`, + cursor de giro

apps/pixpin
└── ventana_editor.rs ← NUEVO  la ventana
```

**El anotador de pantalla y el pin no se tocan en esta entrega.** Siguen con
sus once herramientas y su rapidez. Cuando el armazón esté probado se les
puede ofrecer lo que les sirva, pero como decisión aparte.

**Efecto de rebote deseado:** `pixpin-ui/anotador.rs` tiene 1.097 líneas y
hace tres trabajos. Sacarle la máquina de estados lo deja siendo lo que debe
ser: la interfaz del anotar rápido.

---

## 4. Selección

```rust
pub struct Seleccion {
    ids: Vec<u64>,   // el orden no significa nada; es para iterar sin sorpresas
}
```

| Gesto | Qué hace |
|---|---|
| Clic sobre algo | Solo eso |
| `Shift` + clic | Lo añade o lo quita |
| Arrastrar en vacío | Marquesina: lo que quede enteramente dentro (D29) |
| `Ctrl+A` | Todo lo visible |
| Clic en vacío, `Escape` | Nada |

**La caja de varios es paralela a los ejes y cada elemento conserva su propio
ángulo.** Girar una selección de cinco gira las cinco posiciones alrededor del
centro común *y* suma el ángulo a cada uno. Es lo que hacen Excalidraw y el
Android.

**Grupos:** `Elemento` gana `grupos: Vec<String>`, leído de los `groupIds` que
`excalidraw.rs` ya conserva en el JSON original. Con eso, un plano hecho en el
móvil se abre aquí y sus grupos siguen siendo grupos.

---

## 5. Historial

```rust
pub struct Paso { cambios: Vec<Cambio> }

enum Cambio {
    Anadido(u64),
    Borrado(u64),
    Editado { id: u64, antes: Box<Elemento> },
}
```

Un arrastre que mueve cuarenta elementos es **un** paso, no cuarenta.

**Por qué el estado anterior entero y no la operación inversa (D24).** La
tentación es guardar «se escaló por 1,5» y deshacer dividiendo. No funciona:
en coma flotante la ida y la vuelta no coinciden siempre, y treinta ciclos de
deshacer y rehacer deforman el dibujo. Es un fallo que aparece en casa del
usuario y no en las pruebas.

**Lo que cuesta.** Lo que se hace mil veces al día —dibujar un trazo— es
`Anadido(id)`: 8 bytes. Lo caro es transformar un trazo de 492 puntos, unos
4 KB. De ahí el techo en memoria (D25): 8 MB, y al pasarse se tira el paso más
antiguo.

**Lo que no cambia:** el historial sigue siendo `#[serde(skip)]`, de la sesión
y no del documento; y el borrado lógico con `compactar()` al guardar se queda
como está.

---

## 6. Tiradores y transformación

Ocho de tamaño —cuatro esquinas, cuatro lados— más uno de giro.

**Viven en el mundo, miden en pantalla.** Un tirador tiene 8 píxeles siempre;
en el mundo eso es `8.0 / zoom`. Su zona de picado es de 10 píxeles y **manda
sobre el picado de elementos**: sin esa prioridad, un tirador encima de un
trazo es inalcanzable.

### El problema del ancla

Redimensionar tirando de una esquina significa que la esquina de enfrente no
se mueve. Con el elemento girado, la forma evidente está mal:

> Cambio `ancho` y `alto` → el centro se desplaza → el elemento se dibuja
> girado **alrededor de un centro nuevo** → la esquina anclada se va.

Cómo se hace bien:

1. Llevar el cursor al marco propio del elemento: girarlo `−ángulo` alrededor
   del centro actual.
2. Ahí calcular el nuevo tamaño con el ancla quieta. Sin ángulos, es una resta.
3. El centro se ha movido en ese marco. Girar ese desplazamiento `+ángulo`.
4. De ahí sale el centro verdadero en el mundo, y de él la `x` y la `y`.

### La regla que se me habría escapado

`Elemento::caja()` calcula la caja **de los puntos**, no de `x/ancho`, para
lápiz, resaltador, línea y flecha. Por tanto: **toda transformación que toque
una figura con puntos, toca los puntos.** Si solo se cambia `ancho`, el trazo
no escala y el marco se despega del dibujo. Es el mismo motivo por el que
`mover()` ya mueve los puntos.

### Teclas

| Tecla | Al escalar | Al girar |
|---|---|---|
| — | Libre | Continuo |
| `Shift` | En proporción | A saltos de 15° |
| `Alt` | Desde el centro | — |

### Varios a la vez

Escalar multiplica posiciones y tamaños de cada miembro; girar gira las
posiciones alrededor del centro común y suma el ángulo a cada uno. Cada
elemento pasa por la misma rutina de un elemento: una sola verdad sobre qué es
transformar.

---

## 7. La máquina del gesto

### Los estados

```
Reposo ──┬─ sobre un tirador ──→ Escalando { cual } ─┐
         ├─ sobre el de giro ──→ Girando ────────────┤
         ├─ sobre algo ya elegido → Moviendo ────────┼→ Soltar → Reposo
         ├─ sobre algo no elegido → elegir + Moviendo┤
         ├─ en vacío, herramienta ─→ Dibujando ──────┤
         ├─ en vacío, seleccionar → Marquesina ──────┘
         └─ botón central o Espacio → Encuadrando
```

Más `Escribiendo`, que no es un arrastre: se entra al soltar y se sale con
`Escape` o con un clic fuera.

**El orden de esa lista es la decisión.** Un tirador manda sobre lo que haya
debajo; lo ya seleccionado manda sobre lo de encima. Sin la segunda regla,
mover un grupo se convierte en seleccionar por accidente lo que estaba encima.

### El paso se abre y se cierra con el gesto

Al pulsar se abre; al soltar se cierra. Y sale gratis una cosa que suele
costar: **`Escape` a mitad de un arrastre lo cancela**, porque el paso ya
guarda el estado anterior. Cancelar es aplicarlo y tirar el paso.

### El camino caliente

Mover el ratón mientras se dibuja tiene el único plazo sagrado del editor: un
fotograma del refresco real, 16 ms a 60 Hz.

1. **Sin asignar memoria.** El `Vec` del trazo en curso se reserva de una vez
   —512 puntos, más que el trazo más largo del fichero del móvil— y se
   reutiliza entre trazos con `clear()`.
2. **Se ensucia el segmento, no la pantalla.** Al añadir un punto se invalida
   la caja del último tramo más el grosor.
3. **Nada de suavizados todavía.** El filtro de un euro, el pulso por
   velocidad y la espina Catmull-Rom son la segunda entrega. Aquí solo queda
   el hueco: los puntos entran por una función y ahí se enchufarán.

### Detalles pequeños que se notan

- **El cursor de escalar va girado con el elemento.** En una figura a 45°, el
  tirador de la esquina enseña la flecha que de verdad apunta hacia donde va a
  crecer.
- **La rueda acerca hacia el cursor**, no hacia el centro. Con lienzo
  infinito, lo segundo obliga a encuadrar después de cada zoom.
- **`Espacio` encuadra** sin soltar la herramienta.

---

## 8. La ventana

No hace falta fontanería nueva. `VentanaOverlay` de `pixpin-shell` ya sirve
ventanas completas —el editor de grabaciones la usa así— y ya trae los eventos
que hacen falta: ratón, teclas con `Shift` y `Ctrl`, `Caracter(char)` con IME,
`CambioDpi`, cursores diagonales, y el bucle por eventos que da el 0 % de CPU
en reposo.

**Lo único que le falta:** `alt: bool` en `Tecla` y un cursor de giro.

```
ventana_editor.rs
├── el lienzo         → pixpin-motor2d::gesto
├── la barra          → pixpin-ui::caja_herramientas (existe, crece)
├── el panel lateral  → NUEVO, propiedades de lo seleccionado
└── el pintado        → pixpin-render::Pintor (existe)
```

El fichero de la ventana **traduce y nada más**. Si crece más allá de eso, es
que se ha colado lógica que debería estar en el motor.

### El panel de propiedades

Enseña **solo lo que tiene sentido para lo seleccionado**: con un trazo a mano
no aparece «relleno»; con un texto aparece la fuente y no la rugosidad. Esa
tabla ya está escrita en `DrawProperties.kt` (332 líneas, puras) y es lo que
se porta. **No hay panel cuando no hay nada seleccionado**: un panel con todo
en gris es ruido, y en un portátil viejo es espacio robado al lienzo.

### Qué abre esta ventana

- Un `.pixpin` del móvil (Fase A ya lo lee).
- Un lienzo vacío, desde la bandeja.
- Un pin o una captura anotada, con el botón **«abrir en el editor»**. Ese
  botón es todo el puente que necesitan el anotador y el pin.

### Lo que expresamente no lleva

- **Animaciones y transiciones.** En un editor no aportan: aquí no hay que
  sorprender a nadie, hay que responder.
- **Pestañas o varios documentos.** Con 4 GB, dos escenas de ocho mil
  elementos en memoria es una promesa que no se puede hacer.
- **Minimapa.** De lo primero que se pide y de lo que menos se usa, y obliga a
  pintar la escena entera fuera de pantalla en cada cambio.

---

## 9. Rendimiento

### Las tres piezas que cambian algo

**1. Caché de geometría por `version`.**
Hoy `pintado::ordenes()` regenera el garabato de cada elemento en cada
fotograma: ocho mil generaciones de ruido, sesenta veces por segundo, para un
dibujo que no ha cambiado. El campo `version` de `Elemento` existe justo para
esto —lo dice su propio comentario— y hoy no lo usa nadie.

La caché se indexa por `(id, version, nivel de detalle)`. El nivel de detalle
es un entero pequeño sacado del zoom, **no el zoom en bruto**: si fuera el
zoom, mover la rueda un grado tiraría los ocho mil. En concreto, el nivel es
la octava del zoom, `log2(zoom)` redondeado hacia abajo. Entre los topes que
ya tiene la cámara —5 % y 3.000 %— eso va de `-5` a `+4`: **diez valores en
todo el recorrido**, y solo cambia al doblar o al partir por la mitad el
aumento. Encuadrar no cambia el nivel, así que arrastrar el lienzo no
invalida nada.

Es la mayor de las tres y la que menos código cuesta.

**2. Rejilla espacial (`indice.rs`).**
`elemento_en()` y `recortar()` recorren la lista entera. La rejilla se
mantiene al insertar y al cambiar `version`; picar mira la celda del cursor y
recortar las celdas de la vista. Rejilla y no árbol (D27).

**3. La capa estática.**
Mientras se arrastra un elemento, los otros 7.999 no cambian: se pintan una
vez a un mapa de bits, y cada fotograma copia el mapa y pinta encima solo lo
que se mueve. En una iGPU con memoria compartida, es la diferencia entre
arrastrar y ver arrastrar.

Su coste, sin adornos: en 1080p son unos 8 MB, y en la máquina suelo la
memoria de vídeo sale de los mismos 4 GB. Es **una de las tres copias vivas**
que el presupuesto concede al nivel `Ligero`. Queda marcada para medir, no
para prometer.

### Lo que NO se hace

| Descartado | Por qué |
|---|---|
| wgpu, Skia, egui | Direct2D ya está acelerado e integrado. Añadir una capa son megas de binario y una traducción más |
| Pintar en varios hilos | Dos núcleos físicos en el suelo. Un pool compite consigo mismo |
| `target-cpu=native` | Ivy Bridge no tiene AVX2: el binario muere con instrucción ilegal al arrancar, sin mensaje útil |
| Índice incremental sofisticado | Una rejilla demostrable vale más que un árbol que haya que depurar |

---

## 10. Cómo se demuestra

### Carril automático

Corre en cualquier máquina, sin GPU y sin escritorio; entra en la puerta de
cada rama.

| Prueba | Qué exige |
|---|---|
| Ancla de ida y vuelta | Escalar y devolver deja la esquina anclada donde estaba, a 0°, 30°, 90° y 180° |
| Deshacer exacto | Tras 30 ciclos de deshacer/rehacer, la escena es idéntica bit a bit a la de partida |
| Caché fiel | Lo cacheado es igual a lo recién generado, para escenas al azar |
| Rejilla fiel | La rejilla devuelve lo mismo que la fuerza bruta, para escenas al azar |
| Gesto completo | Pulsar–mover–soltar produce exactamente un paso de deshacer |
| Cero asignaciones | Mover el ratón dibujando asigna cero veces |
| Techo del historial | 500 transformaciones de un trazo de 492 puntos no pasan de 8 MB |

El **asignador que cuenta no existe todavía** en el proyecto, y el presupuesto
de rendimiento lo da por hecho. Se construye aquí y sirve para lo que venga
después.

### Carril manual

**Hecho encontrado al planificar:** las nueve mediciones del proyecto se
llaman todas `equipo-desarrollo`. **La máquina suelo nunca se ha medido.** El
presupuesto está escrito y es bueno; no hay una sola prueba de que se cumpla
donde importa.

El usuario no tiene el i3 a mano. Se mide, por tanto, así:

1. **En el equipo de desarrollo** (i7-10510U, 4 núcleos físicos, 15,8 GB,
   Intel UHD + MX250, Windows 11).
2. **Con `Ligero` forzado** desde `pixpinmax.toml`. La decisión D15 dice con
   estas palabras que esa anulación «no es un lujo: es lo que permite
   ejercitar y medir la ruta ligera en cualquier máquina».
3. **Con carga sintética**: la escena real multiplicada hasta 8.000 y 20.000
   elementos, para que este equipo sude lo que sudaría el i3 con menos.
4. **Con la escena de verdad**: un `.pixpin` del usuario, no una inventada.

Se miden cuatro cosas: latencia de trazo, fotogramas al arrastrar, RAM en
reposo y RAM en pico.

**Y el informe dirá lo que es.** Medido aquí, no en el suelo. La medición del
i3 queda anotada como pendiente, no como hecha.

**Un riesgo que este carril no puede cubrir:** el equipo de desarrollo es un
Comet Lake **con AVX2**. Un binario compilado con `target-cpu=native`
funcionaría aquí perfectamente y moriría en el i3. Eso no se pilla midiendo:
se pilla vigilando la configuración de compilación, y por eso D17 fija el
baseline en `x86-64` explícito.

---

## 11. Fuera de alcance

| Qué | Dónde va |
|---|---|
| El tacto del trazo — filtro de un euro, pulso, espina Catmull-Rom | Segunda entrega del editor |
| Las veinte herramientas que faltan | Fase B de `2026-09-06-android-a-windows.md` |
| Que el anotador y el pin hereden tiradores e historial | Decisión aparte, después de que el armazón esté probado |
| El croquis 3D | Fase D |
| La sincronización por WiFi | Aplazada por el usuario |
