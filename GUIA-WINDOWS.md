# GridePOS en tu computadora

## Abrir ahora

Haz doble clic en **Abrir-GridePOS.cmd** en la carpeta del proyecto. También puedes abrir **bin\GridePOS-Cotizaciones.exe**. El programa abre el navegador en una dirección local `127.0.0.1` con puerto dinámico. La ventana de consola mantiene la aplicación en funcionamiento; ciérrala cuando termines, después de guardar tus cambios.

## Instalar

Haz doble clic en **Instalar-GridePOS.cmd**. Instala el programa únicamente para tu usuario y crea accesos directos en el escritorio y el menú Inicio. No requiere administrador. Los archivos del programa se guardan en `%LOCALAPPDATA%\Programs\GridePOS-Cotizaciones`; los documentos quedan en una carpeta separada.

El instalador no descarga nada ni instala servicios. Para instalar en otra computadora Windows de 64 bits, copia la carpeta entregada con `bin`, `INSTALAR.ps1` e `Instalar-GridePOS.cmd`. Para recompilar se necesita Rust y Visual Studio Build Tools; para usar el ejecutable en esta PC, no.

## Cotizar y facturar

1. Pulsa **Nueva cotización**, completa el cliente y agrega equipos, instalación o servicios. Los precios son antes de ITBIS; define la tasa por línea.
2. Pulsa **Guardar**. El historial de la izquierda permite buscar y abrir documentos guardados. Los números sugeridos se pueden editar; verifica que el número sea el que deseas antes de guardar.
3. Usa **Versiones** para cargar una revisión anterior. Guarda de nuevo para crear una revisión nueva conservando las anteriores. Si abres el mismo documento en dos ventanas, la aplicación rechaza guardar una versión desactualizada.
4. Con una cotización guardada, pulsa **Facturar cotización**. Elige la modalidad A o B y guarda la nueva factura. La cotización original permanece intacta. Los recurrentes se muestran por separado; para cobrar un período, añádelo como concepto del detalle.
5. Pulsa **Imprimir / guardar PDF**. En la vista abierta, pulsa el botón de impresión y selecciona **Guardar como PDF** o **Microsoft Print to PDF**. Usa A4, escala 100% y desactiva los encabezados y pies automáticos del navegador.

La factura es comercial. La aplicación no emite ni valida NCF o e-CF autorizados por la DGII. Las referencias fiscales se ingresan manualmente y no equivalen a validación fiscal.

## Datos y respaldo

Los datos se guardan en `%LOCALAPPDATA%\GridePOS\Cotizaciones\cotizaciones.sqlite3`, fuera del repositorio y de la instalación. Cada guardado conserva una revisión. Actualizar el programa no borra los documentos.

**Crear respaldo** descarga un JSON con los documentos guardados y sus revisiones. Consérvalo también en otra unidad. **Restaurar respaldo** importa los documentos que falten y sus versiones; los identificadores existentes se conservan sin sobrescribir. Los cambios sin guardar no están incluidos. El límite de importación es 8 MB; los respaldos no están cifrados. Las versiones generadas antes de esta actualización no pueden reconstruirse si no fueron conservadas.

Al abrir el programa se crea una copia diaria completa de SQLite en la subcarpeta `respaldos-automaticos`. También se crea una copia antes de importar un respaldo. Estas copias SQLite sirven para recuperación técnica: no se seleccionan en el importador JSON. No reemplaces la base activa ni sus archivos WAL mientras la aplicación esté abierta. Para recuperación técnica, conserva toda la carpeta de datos y solicita ayuda antes de reemplazarla.

## Mantener el proyecto

Rama de trabajo: `codex/rust-offline-cotizaciones-facturas`. Para compilar, ejecuta `COMPILAR-WINDOWS.ps1`; primero corre las pruebas y luego coloca el ejecutable en `bin`. Cierra GridePOS antes de reemplazar un ejecutable en uso. Después ejecuta el instalador de nuevo para actualizar la copia instalada. Se conserva el ejecutable anterior y los datos quedan fuera de la instalación.

`GRIDEPOS_DATA_DIR` permite usar una carpeta de datos separada para pruebas; `GRIDEPOS_NO_BROWSER=1` evita abrir automáticamente el navegador. Nunca apuntes las pruebas a la carpeta real de documentos. Los archivos de prueba y los PDF generados para verificación no se suben al repositorio.
