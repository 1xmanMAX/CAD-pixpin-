# Medir — cota, escalar y escala gráfica

**Fecha:** 2026-09-07
**Estado:** diseño aprobado por el usuario, sección a sección
**Alcance:** la familia B.3 del plan `2026-09-06-android-a-windows.md`. Las tres
herramientas de medir y el concepto de escala. No las otras diecisiete
herramientas.

---

## 1. Por qué esta familia primero

El plan maestro lo dice de las familias de medir y construir: *«son las que
convierten el anotador en una herramienta de trabajo y no en un rotulador».*
De las dos, medir es la que menos depende de lo demás: no necesita imán, ni
recorte, ni nudos.

El armazón del editor está terminado (751 pruebas), así que hay dónde
apoyarla: selección, tiradores, transformaciones, historial de transacciones y
una máquina del gesto que se prueba sin abrir una ventana.

### Lo que se porta, medido

| Fichero del Android | Líneas | Qué resuelve |
|---|---:|---|
| `Medida.kt` | 264 | `Escala`, longitud, ángulo, textos, fijar largo exacto, rótulo del revés |
| `EscalaGrafica.kt` | 129 | El reparto de la barra en cuadros de paso redondo |

Las dos son puras: cero imports de Android. Se portan enteras.

### Tres cosas que el Android ya tiene resueltas y que se hacen mal a la primera

1. **El rótulo se calcula al pintar.** No se guarda «2,45 m» en ningún sitio.
   Por eso una cota no puede mentir: si se mueve un extremo, el número cambia
   solo.
2. **El ángulo solo sale si la raya está torcida.** En una horizontal, «0°»
   ocupa media etiqueta para no decir nada.
3. **La barra elige pasos redondos** —1, 2 o 5 por una potencia de diez— en vez
   de repartir el ancho a partes iguales. Una barra con cuadros de «3,7 m» hay
   que leerla con calculadora.

---

## 2. Decisiones

Continúan la numeración del diseño del editor (D1–D30).

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D31 | **Dónde vive la escala** | En la **`Escena`**, no en cada cota | Un lienzo tiene una escala y todas sus cotas la usan. Con una por cota, dos cotas del mismo plano podrían discrepar, y un plano con dos escalas no sirve |
| D32 | **Unidades** | **Texto libre**, sin conversión | Se calibra en la unidad en la que se va a leer. Convertir inventa el problema de mezclarlas, que no existe si no se pueden mezclar |
| D33 | **Precisión** | **`f32`**, como todo el motor | Mezclar `f64` obligaría a convertir en cada rótulo. Siete cifras significativas sobran para una cota a dos decimales, y el rango cubre de milímetros a kilómetros por píxel |
| D34 | **El rótulo** | **Se deriva al pintar**, nunca se guarda | Una cota que guardara su texto podría decir una cosa y medir otra. En un plano, eso es peor que no tener cotas |
| D35 | **Sin calibrar** | La cota **mide en píxeles**, en gris y con sufijo `px` | Decisión del usuario. El Android ya devuelve píxeles cuando no hay escala; lo que su regla prohíbe es entrar a la herramienta, no el formato. El gris es el aviso |
| D36 | **Escala inválida** | Se **rechaza al construirla**, no al usarla | Que un error de calibrado no se propague a doscientas cotas |
| D37 | **La barra de escala** | Es un **elemento del dibujo**, no un adorno del editor | Se guarda, se mueve, se estira y sale en la exportación. Es lo que permite medir sobre la imagen que recibe otro |
| D38 | **La cota, tipo propio** | `Figura::Cota`, no una `Linea` con bandera | El móvil usa `pixpin-measure`; con una bandera se rompería la compatibilidad |
| D39 | **El separador decimal** | Llega **por parámetro**, no del sistema | Una prueba que dependiera del Windows donde corre daría verde en un equipo y rojo en otro |
| D40 | **Pedir datos desde el gesto** | `Respuesta` gana `pide: Option<Peticion>` | La máquina dice «hay que preguntar esto»; quién pregunta es asunto del que pinta. Mismo corte que `Region` y `FormaCursor` |

---

## 3. Dónde vive cada cosa

```
pixpin-motor2d  (L0, puro, #![forbid(unsafe_code)])
├── medida.rs        ← NUEVO  Escala, longitud, angulo, textos, largo exacto
├── escalabarra.rs   ← NUEVO  el reparto de la barra en cuadros redondos
├── elemento.rs      ← + Figura::Cota y Figura::EscalaGrafica
├── escena.rs        ← + campo `escala: Option<Escala>`
├── excalidraw.rs    ← leer y escribir `escala`, `pixpin-measure`, `pixpin-scalebar`
├── pintado.rs       ← las ordenes de una cota y de una barra
└── gesto.rs         ← las tres herramientas y la peticion de calibrado

pixpin-ui/src/propiedades.rs      ← que se ajusta de una cota y de una barra
apps/pixpin/src/ventana_editor.rs ← el cajetin de calibrar
```

`medida.rs` y `escalabarra.rs` son puros, así que las tres herramientas se
prueban sin abrir una ventana, igual que el resto del armazón.

---

## 4. El modelo

### La escala

```rust
/// Que mide un pixel. Vive en la Escena: un lienzo tiene UNA escala.
pub struct Escala {
    pub unidades_por_pixel: f32,
    /// Texto libre a proposito (D32): "m", "cm", "mm", "km", "ft", "in"...
    pub unidad: String,
    /// Dos son los de una cota de obra.
    pub decimales: u8,
}
```

Se construye por calibrado —«estos 100 píxeles son 3 metros»— y el
constructor **devuelve `None` si la calibración no puede ser cierta**: una
medida que no sea positiva, o una raya sin arrastrar (D36).

### Las dos figuras

```rust
Cota { puntos: Vec<Punto2> },   // dos puntos: el segmento acotado
EscalaGrafica,                  // usa x/y/ancho/alto, como el rectangulo
```

La cota **no guarda su texto**: ni el número, ni la unidad, ni el ángulo. Solo
dos puntos (D34).

### Cómo viaja al móvil

| Nuestro | Del móvil |
|---|---|
| `Figura::Cota` | `"pixpin-measure"` |
| `Figura::EscalaGrafica` | `"pixpin-scalebar"` |
| `Escena.escala` | clave `escala` al nivel del lienzo |

Los tres **ya sobreviven hoy** la ida y vuelta: `Entrada::Ajeno` conserva los
elementos que no entendemos y `Lienzo.resto` las claves de escena que no
leemos. Comprobado sobre un `.pixpin` real del usuario, cuyo lienzo lleva
`backgroundColor`, `luces`, `tablas`, `referenciasVisibles`, `alfileres` y
`vista` al nivel superior, todos intactos.

Lo que falta es **leerlos**. Es el mismo caso que fueron los `groupIds`.

---

## 5. Las tres herramientas

### Escalar — se usa una vez y desbloquea el resto

Se arrastra una raya sobre algo de medida conocida y al soltar se pregunta
cuánto mide de verdad.

- **La raya desaparece al calibrar.** Era un metro, no un dibujo. Si se quería
  que se quedara, la herramienta era la cota.
- **Con `Shift` se sujeta a horizontal o vertical.** Calibrar sobre el borde de
  una pared es el caso normal, y a pulso sale torcida: un grado de más son dos
  centímetros de error por metro.

### Cota — se usa todo el rato

El rótulo va **encima de la raya**, no en un panel. El propio Android lo
razona: *«una medida que hay que ir a leer a otro sitio se usa para comprobar
al final, cuando ya está todo mal»*.

Sin calibrar muestra `245 px` **en gris** (D35). El color es el aviso: un
número gris con sufijo `px` no se confunde con una medida de plano.

El rótulo **se da la vuelta entre 90° y 270°** para no leerse boca abajo.

### Escala gráfica — sobrevive a la fotocopia

Se arrastra para decir lo ancha que va, y ella elige cuántos cuadros pone y
cuánto mide cada uno, con paso redondo.

El encabezado del original lo justifica mejor que ninguna otra cosa: *«un
número —"escala 1:50"— solo vale mientras nadie toque el papel. En cuanto el
plano se fotocopia al 80 %, se manda por WhatsApp o se recorta, el número
miente y nadie se entera. La barra a cuadros no puede mentir: se encoge y se
estira con el dibujo».*

### Lo único que hay que añadir al armazón

```rust
pub struct Respuesta {
    pub region: Region,
    pub cursor: FormaCursor,
    pub pide: Option<Peticion>,     // NUEVO
}

pub enum Peticion {
    /// La ventana tiene que preguntar cuanto mide de verdad este trazo.
    Calibrar { largo_px: f32 },
}
```

La máquina sigue sin saber qué es una ventana (D40). Cuando la ventana tiene
la respuesta, llama a `calibrar(escena, largo_px, valor, unidad)`, que es puro
y se prueba sin pantalla.

**Esto le sirve también al texto**, que hoy no hace nada en el editor por no
tener por dónde pedir. No se construye ahora, pero el hueco queda en el sitio
correcto.

---

## 6. Cómo se demuestra

### La propiedad que justifica el diseño

**Una cota no puede mentir.** Se dibuja, se lee el rótulo, se mueve un extremo
y se vuelve a leer. Si el número no cambió, el diseño está roto. Es la única
prueba que de verdad justifica no guardar el texto.

### Carril automático

| Prueba | Qué exige |
|---|---|
| Calibrar 100 px = 3 m | Da 0,03 unidades por píxel |
| Calibración imposible | Cero, negativa o infinita se rechaza **al construir** |
| Rótulo sin escala | `245 px` |
| Rótulo con escala | `2,45 m`, con los decimales de la escala |
| El ángulo | Sale a 30°, **no sale** a 0° |
| Fijar un largo exacto | Mueve el otro extremo, **no el origen** |
| Rótulo del revés | Se da la vuelta entre 90° y 270° |
| Paso de la barra | Siempre 1, 2 o 5 por una potencia de diez |
| Ida y vuelta | `pixpin-measure`, `pixpin-scalebar` y `escala` |
| Separador decimal | Llega por parámetro; la prueba no depende de la máquina |

### Lo que necesita mano

Que el rótulo se lea bien encima de la raya, y que la barra se vea como la de
un plano. Eso no lo dice ninguna prueba.

### Pendiente del usuario

La prueba de **«un plano acotado del móvil vuelve idéntico»** necesita un
`.pixpin` con cotas y escala puestas. El que hay (`D:\TELEGRAM\Proyecto 2.pixpin`)
tiene 206 elementos pero ninguna cota y ninguna escala, así que hasta que
llegue uno de verdad esa prueba va con datos construidos a mano. **Queda
anotado como pendiente, no como hecho.**

---

## 7. Fuera de alcance

| Qué | Dónde va |
|---|---|
| Las otras diecisiete herramientas | Fases B.1, B.2, B.4, B.5, B.6 |
| El imán, que hace que medir sea exacto en vez de aproximado | B.6 |
| Ángulos internos, regla y transportador en pantalla | Más adelante; el maestro los menciona |
| Escribir texto en el editor | Segunda entrega del editor; esta tarea le deja el hueco |
| Tablas de cotas | No está pedido |
