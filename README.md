# tcol — теговая коллекция файлов

Плоский теговый диск вместо иерархии папок. Файлы остаются на месте; коллекция —
это tagdb-записи (`.eml`), поиск по тегам через X-Query с **header-only scan**
(тело файлов не читается при поиске).

Построен на [emlbox](https://github.com/kostyk348/mime-os) (single-file .eml
контейнер + eml-tag).

## Установка

```bash
cargo build --release   # ./target/release/tcol
```

## Использование

```bash
tcol init ~/.tcol

# добавить файл с тегами (дубликаты перезаписываются, теги обновляются)
tcol add ~/.tcol ./shaders/retro.glsl --tag shader --tag retro --tag mpv

# добавить дерево целиком
tcol add ~/.tcol ~/vita/homebrew --tag vita --tag homebrew --recursive

# поиск: X-Query
tcol find ~/.tcol 'X-Tag == "shader" AND X-Tag == "mpv"'
tcol find ~/.tcol 'X-Tag != "game"'
tcol find ~/.tcol 'X-Timestamp >= 1786500000'

# вся коллекция, теги с счётчиками, статистика
tcol ls ~/.tcol
tcol tags ~/.tcol
tcol stats ~/.tcol

# удалить из коллекции (файл не трогается)
tcol rm ~/.tcol ./shaders/retro.glsl
```

## Как это работает

Каждая запись — атомарный `.eml` (tmp+rename, сбой не оставляет torn-файл):

```
X-Record-ID: f_<sha256 пути>
X-Tag: shader
X-Tag: mpv
X-Timestamp: 1786500000
Content-Type: application/json

{"path": "/abs/path", "size": 1234, "mtime": ..., "sha256": "..."}
```

Поиск читает только заголовки (≤4 КБ на запись, тело никогда). Одна сломанная
запись не роняет коллекцию. Один файл может входить в сколько угодно «папок»
(тегов) без копирования.

## Почему не папки

- Один файл в N категориях — без дублирования
- Поиск по комбинации тегов мгновенный (header scan, не grep по всему диску)
- Коллекция переносится копированием директории
- Integrity: sha256 каждого файла в записи — расхождение = файл менялся

## Тесты

```bash
cargo test
```
