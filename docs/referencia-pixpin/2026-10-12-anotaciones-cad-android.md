# Lo anotado sobre un plano DWG/DXF: el PC ya hace lo mismo que Android

*12-oct-2026 · PixPin para Windows, rama `vh-cad-tinta` · guía corta para PixPin Android*

El usuario pidió: «cuando pinto con el lápiz en el archivo CAD está desproporcionado [...] esto ya lo
arreglé en Android» y «esas anotaciones tienen que sincronizarse entre el celular y la PC; en realidad
todo cambio tiene que ser sincronizable». El PC sigue ahora `CapaDelPlano.kt`, `EscalaDeLaCapa` y
`docs/planos-que-viajan.md` de Android (v0.115 en adelante). Aquí, lo que el móvil tiene que saber
y lo poco que convendría cambiar allí.

## Lo que hace ahora el PC (igual que Android)

- **Ficheros**: los de siempre del mensaje del plano, `pins/draw/anot-<uid>.excalidraw.gz` y
  `pins/draw/anot-<uid>.hoja`. Sin terminaciones nuevas. Un plano sin nada hecho no deja ficheros.
  La `.hoja` se escribe antes que la capa y solo si cambia.
- **Unidad de la capa nueva**: la de `EscalaDeLaCapa.deLasLetras` (la letra más común × 1,4 / 20;
  sin letras, la milésima del lado / 5), con el cero en el origen del modelo. Era la causa del lápiz
  desproporcionado en el PC: con la milésima del lado, cada unidad medía cientos de píxeles al
  acercarse y las medidas fijas del motor (radio mínimo, tramo final tomado por ruido) engordaban y
  deformaban el trazo. Medido: el mismo gesto daba 65,8 px de grueso contra 5,2 px en un lienzo; ahora
  5,2 px a cualquier zoom.
- **Al leer**: manda la `.hoja` (`encajeDe`). Sin ella, si ya hay capa, la unidad antigua (la
  milésima del lado) con el cero en el origen; si no, la de la letra. Una `.hoja` que no se entiende
  se ignora. En memoria el PC trabaja siempre en la unidad de la letra y escribe en las unidades del
  fichero (lo que no se tocó vuelve con sus números exactos).
- **Cotas y marcos para imprimir** van en la capa: `line` con `groupIds = ["plano-cota"]` y `frame`
  con `groupIds = ["plano-marco"]`, el id de su forma (`huella`, FNV-1a con `%.4f;`, empates hacia
  arriba como Java), naranja `#ff8a00` y azul `#3d8bff`, `strokeWidth` 1. Quitar uno lo deja con
  `isDeleted` y una versión nueva. Fuera del modo anotar el PC no las pinta con el motor (las pinta el
  visor con sus medidas), igual que `sinLoDelPlano`.

Probado con un móvil de mentira (`DiscoAndroid`) por el protocolo de verdad: el PC anota, acota y
marca; llega al móvil y cae en su sitio con la cuenta de `encajeDe`; el móvil borra el trazo y añade
su cota; vuelve; el PC quita el marco y llega borrado. Lo de otro plano no se mezcla, y una capa o una
`.hoja` dañadas se leen vacías sin romper nada.

## Lo que convendría cambiar en Android

1. **Juntar las cotas y los marcos también por su forma, no solo por el id.** `CapaDelPlano.con`
   recalcula el id de cada cota desde sus coordenadas. Una cota que llegó del PC (o de otro aparato)
   y se leyó con `aPlanoX/aPlanoY` puede no dar el mismo id por los decimales (el PC guarda en `f32`),
   y entonces `con` la marca borrada y crea otra igual con otro id: viaja de más en cada guardado y el
   otro aparato ve borrar y nacer la misma cota. El PC ya lo hace así: una figura del grupo vale por
   la querida si tiene el mismo id **o** la misma forma con menos de una milésima de unidad de capa de
   diferencia, y al revivirla conserva su id.
2. **El orden de los marcos.** Android ordena las hojas por `updated`. El PC no guarda `updated` por
   figura: al escribir pone la hora de ese guardado a lo cambiado, así que dos marcos nuevos guardados
   a la vez salen con el mismo `updated`, y el orden queda en manos de la ordenación estable (el del
   fichero, que es el de creación). Funciona, pero si se quiere un orden explícito, mejor guardarlo en
   la figura (p. ej. el número de hoja en `name` del `frame`) que confiar en `updated`.
3. **`.escala` no viaja** (no está en `TERMINACIONES`). No hace falta: Android escribe siempre la
   `.hoja` al guardar y la `.hoja` manda. Basta con no depender de `.escala` cuando hay `.hoja`, como
   ya hace `leerOPoner`.

## Lo que no viaja

La caché de planos leídos y lo anotado sobre un plano que no es de ningún chat (en el PC va a
`cache/cad/<clave>.anot.excalidraw` y `.anot.hoja`, solo en ese equipo), como en Android.
