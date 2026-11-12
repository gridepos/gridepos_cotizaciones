# GridePOS · Cotizaciones y facturas offline

Aplicación Rust para Windows 10/11 de 64 bits. Prepara cotizaciones con el logo y formato de GridePOS, y facturas comerciales con numeración independiente.

La interfaz se abre en el navegador de la propia computadora. No necesita internet para funcionar ni alojamiento externo.

## Funciones

- Cliente, conceptos, cantidades, precios e ITBIS por línea editables.
- Comparación de alternativas, servicios recurrentes separados y notas.
- Factura nueva o derivada de una cotización, conservando el documento de origen.
- Historial local, búsqueda, duplicación, revisiones anteriores y respaldo/restauración JSON.
- Impresión A4 y guardado como PDF desde Edge/Chrome actuales.

Las facturas son documentos comerciales: la aplicación no genera ni valida NCF o e-CF autorizados por la DGII. La referencia fiscal opcional se ingresa manualmente. No se asume una exención de servicios.

## Código y compilación

La implementación inicial está en la rama `codex/rust-offline-cotizaciones-facturas`.

Con Rust y las herramientas de compilación C/C++ disponibles en Windows:

```powershell
cargo test --locked
cargo build --release --locked
```

También puede usarse `COMPILAR-WINDOWS.ps1`, que ejecuta las pruebas y coloca el programa compilado en `bin\GridePOS-Cotizaciones.exe`. El desarrollo puede necesitar internet para descargar dependencias; el programa compilado funciona offline e incluye SQLite, interfaz y logo.

Para abrirlo, ejecuta `bin\GridePOS-Cotizaciones.exe` o `Abrir-GridePOS.cmd`. Se abre una dirección `127.0.0.1` con puerto dinámico. Conserva abierta la consola mientras trabajas. La interfaz es web local; no utiliza Tauri.

Para instalarlo para tu usuario, abre `Instalar-GridePOS.cmd`. Crea accesos directos y copia el programa a `%LOCALAPPDATA%\Programs\GridePOS-Cotizaciones`, sin tocar la carpeta de documentos. Consulta [GUIA-WINDOWS.md](GUIA-WINDOWS.md) para abrir, actualizar, imprimir y proteger los datos.

## Datos y respaldos

Los documentos se guardan por usuario en `%LOCALAPPDATA%\GridePOS\Cotizaciones\cotizaciones.sqlite3`, fuera del código. `GRIDEPOS_DATA_DIR` permite cambiar la carpeta. `GRIDEPOS_NO_BROWSER=1` evita la apertura automática del navegador durante pruebas.

Pulsa **Guardar** para conservar cambios. Cada guardado conserva una revisión; **Versiones** permite cargar una anterior y guardarla como una revisión nueva. Imprimir no guarda automáticamente. **Crear respaldo** descarga los documentos y revisiones guardados en JSON; **Restaurar respaldo** añade los faltantes y conserva los identificadores que ya existen. Límite de importación de respaldo: 8 MB. Los respaldos no están cifrados.

Se conserva una copia SQLite diaria al abrir el programa y otra antes de importar respaldos en `respaldos-automaticos`, dentro de la carpeta de datos. Estas copias completas son para recuperación técnica; el importador de la interfaz recibe JSON. No se borran automáticamente.

Para PDF, usa A4, escala 100% y desactiva los encabezados y pies automáticos del navegador. Los recurrentes no se incluyen en el total inicial ni se cobran automáticamente en una factura.

## Validación inicial

Verificado también en Windows x64: seis pruebas unitarias y una integración con reinicio real, guardado, control de conflictos, revisiones y respaldo/restauración. Compilación nativa de producción completada. En Chrome se comprobó edición, guardado, recuperación de una revisión, factura derivada, vista de impresión y descarga del respaldo. El guardado final en PDF mediante el diálogo nativo quedó pendiente por falta de acceso a ese diálogo; no se presenta como prueba completada.

Este repositorio contiene código, pruebas y recursos. No incluye ejecutables, bases de datos, credenciales, respaldos ni documentos reales de clientes.
