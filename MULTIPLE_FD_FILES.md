# Поддержка множественных *.fd.h файлов

## Обновление

Build script теперь автоматически обрабатывает **все `*.fd.h` файлы** в директории `ubftab/`.

## Как это работает

### 1. Сканирование директории

```rust
// endurox-sys/build.rs
fn generate_ubf_constants() {
    let ubftab_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../ubftab");
    
    // Сканируем все *.fd.h файлы
    if let Ok(entries) = fs::read_dir(&ubftab_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() == Some("h") && 
               path.file_name().unwrap().to_str().unwrap().ends_with(".fd.h") {
                // Обрабатываем файл
                parse_ubf_header(&content, &mut rust_code);
            }
        }
    }
}
```

### 2. Результат генерации

Все константы из всех `.fd.h` файлов объединяются в один модуль:

```rust
// Автогенерированный файл: $OUT_DIR/ubf_fields.rs

// Fields from test.fd.h
pub const T_NAME_FLD: i32 = 167773162;
pub const T_ID_FLD: i32 = 33555444;
// ...

// Fields from custom.fd.h  
pub const CUSTOM_FIELD_1: i32 = 335545601;
pub const CUSTOM_FIELD_2: i32 = 33556482;
// ...
```

### 3. Использование

Все константы доступны через один импорт:

#### Вариант 1: Прямая работа с UBF буфером
```rust
use endurox_sys::ubf_fields::*;

// Используем поля из разных файлов
buf.add_string(T_NAME_FLD, "test")?;           // из test.fd.h
buf.add_string(CUSTOM_FIELD_1, "custom")?;     // из custom.fd.h
```

#### Вариант 2: С UbfStruct derive macro
```rust
use endurox_sys::UbfStruct;
use endurox_sys::ubf_fields::*;

#[derive(Debug, Clone, UbfStruct)]
struct Payment {
    #[ubf(field = PAYMENT_ID)]        // из payment.fd.h
    id: i64,
    
    #[ubf(field = PAYMENT_AMOUNT)]    // из payment.fd.h
    amount: f64,
    
    #[ubf(field = T_NAME_FLD)]        // из test.fd.h
    name: String,
}

// Автоматическая конвертация
let payment = Payment {
    id: 12345,
    amount: 99.99,
    name: "Payment".to_string(),
};

let ubf = payment.to_ubf()?;
let restored = Payment::from_ubf(&ubf)?;
```

## Преимущества

✅ **Автоматическое обнаружение** - новые `.fd.h` файлы подхватываются автоматически  
✅ **Единая точка доступа** - все константы в одном модуле  
✅ **Нет дубликатов** - каждый файл обрабатывается один раз  
✅ **Инкрементальная сборка** - пересборка только при изменении `.fd.h` файлов  

## Добавление нового набора полей

### Шаг 1: Создать .fd файл

```bash
cat > ubftab/payment.fd << 'EOF'
$#ifndef _PAYMENT_FD
$#define _PAYMENT_FD

*base 3000

PAYMENT_ID        1  long    -  "Payment ID"
PAYMENT_AMOUNT    2  double  -  "Payment amount"
PAYMENT_STATUS    3  string  -  "Payment status"

$#endif
EOF
```

### Шаг 2: Сгенерировать заголовок

```bash
cd ubftab
mkfldhdr payment.fd
# Создастся payment.fd.h
```

### Шаг 3: Пересобрать проект

```bash
cargo clean
cargo build
```

### Шаг 4: Использовать новые поля

```rust
use endurox_sys::ubf_fields::*;

buf.add_long(PAYMENT_ID, 12345)?;
buf.add_double(PAYMENT_AMOUNT, 99.99)?;
buf.add_string(PAYMENT_STATUS, "PENDING")?;
```

## Организация полей по модулям

Рекомендуется группировать поля логически:

```
ubftab/
├── common.fd       # Общие поля (T_NAME_FLD, T_STATUS_FLD, ...)
├── payment.fd      # Поля для платежей (PAYMENT_ID, PAYMENT_AMOUNT, ...)
├── user.fd         # Поля для пользователей (USER_ID, USER_EMAIL, ...)
├── transaction.fd  # Поля для транзакций (TXN_ID, TXN_TYPE, ...)
└── ...
```

Каждый `.fd` файл должен иметь:
- Уникальный `*base` (например, 1000, 2000, 3000...)
- Уникальный guard (`#ifndef _XXX_FD`)

## Автоматический rebuild

Build script автоматически отслеживает изменения:

```rust
// При изменении любого .fd.h файла, cargo пересоберет проект
println!("cargo:rerun-if-changed=../ubftab/{}", filename);
println!("cargo:rerun-if-changed=../ubftab");
```

## Проверка генерации

Посмотреть сгенерированные константы:

```bash
# Найти файл
find target/release/build/endurox-sys-*/out -name ubf_fields.rs

# Просмотреть содержимое
cat $(find target/release/build/endurox-sys-*/out -name ubf_fields.rs | head -1)
```

## Пример многофайловой структуры

```
ubftab/
├── test.fd        (*base 1000)
│   ├── T_NAME_FLD
│   ├── T_ID_FLD
│   └── T_PRICE_FLD
│
├── custom.fd      (*base 2000)
│   ├── CUSTOM_FIELD_1
│   └── CUSTOM_FIELD_2
│
└── payment.fd     (*base 3000)
    ├── PAYMENT_ID
    ├── PAYMENT_AMOUNT
    └── PAYMENT_STATUS
```

Все эти поля будут доступны через:

```rust
use endurox_sys::ubf_fields::*;
```

## Важные замечания

⚠️ **Base ranges** - убедитесь, что `*base` не пересекаются между файлами  
⚠️ **Field numbering** - номера полей должны быть уникальны в пределах одного base  
✅ **Изоляция** - разные base ranges позволяют разным командам работать независимо  
✅ **Расширяемость** - легко добавлять новые наборы полей без конфликтов  

## Миграция существующего кода

Если у вас уже есть код с жестко закодированными константами:

### Было:
```rust
const T_NAME_FLD: i32 = 167773162;
const T_ID_FLD: i32 = 33555444;
const CUSTOM_FIELD_1: i32 = 335545601;
```

### Стало:
```rust
use endurox_sys::ubf_fields::*;
// Все константы уже определены автоматически
```

## Заключение

Обновленный build script обеспечивает:
- ✅ Автоматическую обработку всех `.fd.h` файлов
- ✅ Централизованный доступ ко всем UBF полям
- ✅ Простое добавление новых наборов полей
- ✅ Правильное кодирование типов для всех полей
- ✅ Инкрементальную сборку с отслеживанием изменений
