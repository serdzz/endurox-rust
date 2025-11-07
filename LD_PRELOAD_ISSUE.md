# LD_PRELOAD libnstd.so Issue

## Проблема

При запуске Rust приложений, использующих Enduro/X UBF библиотеки, возникает ошибка с неопределенными символами:

```
error while loading shared libraries: libubf.so: cannot open shared object file: No such file or directory
```

или

```
undefined symbol: ndrx_Bget_long
undefined symbol: ndrx_Badd_string
...
```

## Причина

Библиотека `libubf.so` зависит от символов из `libnstd.so` (Enduro/X standard library), но стандартный линковщик не всегда правильно разрешает эти зависимости при динамической загрузке библиотек через FFI в Rust приложениях.

### Техническая деталь

Enduro/X использует многоуровневую архитектуру библиотек:
- **libnstd.so** - базовая библиотека с utility функциями и общими символами
- **libubf.so** - UBF (Unified Buffer Format) библиотека, зависящая от libnstd
- **libatmi.so** - ATMI (Application-to-Transaction Monitor Interface), также зависящая от libnstd

При загрузке `libubf.so` через Rust FFI, динамический линковщик не всегда автоматически загружает `libnstd.so`, что приводит к ошибкам "undefined symbol".

## Решение

Использовать `LD_PRELOAD` для принудительной загрузки `libnstd.so` **перед** запуском приложения:

```bash
export LD_PRELOAD=/opt/endurox/lib/libnstd.so
./your_rust_app
```

### Для Docker

В `Dockerfile` добавить переменную окружения:

```dockerfile
ENV LD_PRELOAD=/opt/endurox/lib/libnstd.so
```

Пример из проекта:
```dockerfile
ENV PATH="/opt/endurox/bin:${PATH}" \
    LD_LIBRARY_PATH="/opt/endurox/lib:${LD_LIBRARY_PATH}" \
    LD_PRELOAD=/opt/endurox/lib/libnstd.so \
    NDRX_HOME="/opt/endurox"
```

### Для shell скриптов

В скриптах запуска добавить export:

```bash
#!/bin/bash

# Source environment
. /app/setenv.sh

# Preload libnstd.so to provide symbols for libubf.so
export LD_PRELOAD=/opt/endurox/lib/libnstd.so

# Run application
/app/bin/your_app
```

### Для systemd сервисов

В unit файле добавить:

```ini
[Service]
Environment="LD_PRELOAD=/opt/endurox/lib/libnstd.so"
Environment="LD_LIBRARY_PATH=/opt/endurox/lib"
ExecStart=/app/bin/your_rust_app
```

## Альтернативные решения

### 1. Статическая линковка (не рекомендуется)

Можно попробовать статически слинковать все Enduro/X библиотеки, но это:
- Увеличивает размер бинарника
- Усложняет обновление Enduro/X
- Может вызывать конфликты версий

### 2. Явная загрузка через dlopen (сложно)

В `build.rs` можно добавить явную загрузку `libnstd.so`:

```rust
// В build.rs
println!("cargo:rustc-link-arg=-Wl,--no-as-needed");
println!("cargo:rustc-link-lib=dylib=nstd");
println!("cargo:rustc-link-lib=dylib=ubf");
```

Но это не всегда работает корректно с FFI.

### 3. Использование rpath (работает в некоторых случаях)

```rust
// В build.rs
println!("cargo:rustc-link-arg=-Wl,-rpath,/opt/endurox/lib");
```

Однако `LD_PRELOAD` остается самым надежным решением.

## Как проверить

### 1. Проверить зависимости библиотеки

```bash
ldd /opt/endurox/lib/libubf.so
```

Должно показать:
```
libnstd.so => /opt/endurox/lib/libnstd.so (0x...)
libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x...)
...
```

### 2. Проверить загруженные библиотеки в runtime

```bash
LD_PRELOAD=/opt/endurox/lib/libnstd.so ldd /app/bin/your_app
```

### 3. Проверить экспортируемые символы

```bash
nm -D /opt/endurox/lib/libnstd.so | grep ndrx_
```

Должно показать все `ndrx_*` функции.

## Проявление проблемы

Проблема проявляется в следующих случаях:

### ✅ Работает БЕЗ LD_PRELOAD:
- C/C++ приложения, скомпилированные с правильными `-l` флагами
- Приложения, использующие только ATMI (без UBF)
- Статически слинкованные приложения

### ❌ НЕ работает БЕЗ LD_PRELOAD:
- Rust приложения с FFI к UBF функциям
- Go приложения с CGo вызовами UBF
- Python приложения с ctypes/cffi к libubf.so
- Любые динамически загружаемые модули

## В нашем проекте

### Где настроено:

1. **Dockerfile** (строка 40):
   ```dockerfile
   ENV LD_PRELOAD=/opt/endurox/lib/libnstd.so
   ```

2. **test_derive.sh** (строка 7):
   ```bash
   export LD_PRELOAD=/opt/endurox/lib/libnstd.so
   ```

3. **GitLab CI** (.gitlab-ci.yml, строка 50):
   ```yaml
   variables:
     LD_PRELOAD: /opt/endurox/lib/libnstd.so
   ```

### Какие компоненты нуждаются в LD_PRELOAD:

- ✅ **ubf_test_client** - использует UBF напрямую
- ✅ **derive_macro_example** - использует UBF через derive macro
- ✅ **ubfsvr_rust** - UBF сервер
- ✅ **samplesvr_rust** - использует UBF для TRANSACTION сервиса
- ✅ **Unit tests** - тесты в endurox-sys/tests/

### Компоненты, которые МОГУТ работать без LD_PRELOAD:

- ⚠️ **rest_gateway** - если не использует UBF напрямую (зависит от реализации)
- ⚠️ **xadmin** утилиты - нативные C приложения Enduro/X

## Отладка

Если проблема сохраняется даже с `LD_PRELOAD`:

### 1. Проверить путь к библиотеке:
```bash
ls -la /opt/endurox/lib/libnstd.so
```

### 2. Проверить права доступа:
```bash
# Должен быть readable для всех
chmod 644 /opt/endurox/lib/libnstd.so
```

### 3. Проверить ldconfig cache:
```bash
ldconfig -p | grep nstd
```

Если не найдено:
```bash
echo "/opt/endurox/lib" > /etc/ld.so.conf.d/endurox.conf
ldconfig
```

### 4. Использовать LD_DEBUG для диагностики:
```bash
LD_DEBUG=libs LD_PRELOAD=/opt/endurox/lib/libnstd.so ./your_app 2>&1 | grep nstd
```

### 5. Проверить конфликты версий:
```bash
strings /opt/endurox/lib/libnstd.so | grep "NDRX"
strings /opt/endurox/lib/libubf.so | grep "NDRX"
```

Версии должны совпадать.

## Заключение

`LD_PRELOAD=/opt/endurox/lib/libnstd.so` - это **обязательная** настройка для Rust приложений, использующих Enduro/X UBF.

### Checklist для новых компонентов:

- [ ] Добавить `LD_PRELOAD` в Dockerfile если используется
- [ ] Добавить `export LD_PRELOAD` в shell скрипты запуска
- [ ] Добавить в CI/CD pipeline если есть тесты с UBF
- [ ] Документировать в README компонента
- [ ] Проверить что работает в контейнере и на голом железе

### Ссылки:

- [Enduro/X Documentation](https://www.endurox.org/dokuwiki/)
- [Linux LD_PRELOAD](https://man7.org/linux/man-pages/man8/ld.so.8.html)
- [Dynamic Linker Tricks](https://www.akkadia.org/drepper/dsohowto.pdf)
