# Majestic Law Assistant

Локальное Windows‑приложение на Tauri + React + SQLite для работы с законодательной базой Majestic.

## Возможности

- Полностью локальная база (SQLite + FTS5) для законов и памяток.
- Первичная загрузка источников при первом запуске.
- Обновление документов по ссылкам на форум Majestic.
- Справочник с вкладками «Законы», «Ассистент», «Памятки».
- Поиск по словам и по номеру статьи.
- Ассистент формирует ответ с обязательными источниками.

## Требования

- **Node.js LTS** (npm входит в комплект).
- **Rust toolchain (cargo)** — обязателен для Tauri.
- **Microsoft C++ Build Tools** (MSVC) для сборки на Windows.
- **WebView2 Runtime** (обычно уже установлен, иначе поставить вручную).

Если при запуске появляется ошибка `failed to get cargo metadata: program not found`,
это значит, что **Rust (cargo) не установлен или не в PATH**.

### Установка Rust (cargo)

Скачайте и установите Rust через rustup: https://www.rust-lang.org/tools/install  
После установки перезапустите терминал и проверьте:

```bash
cargo --version
```

## Быстрый старт

```bash
npm install
npm run tauri dev
```

### Если ошибка: `"vite" не является внутренней или внешней командой`

Это означает, что зависимости не установлены или запускается не из папки проекта.
Сделайте так:

1. Перейдите в папку проекта:
   ```bash
   cd C:\путь\к\police-assistant
   ```
2. Установите зависимости:
   ```bash
   npm install
   ```
3. Запустите снова:
   ```bash
   npm run tauri dev
   ```

`beforeDevCommand` в Tauri вызывает `npm run dev`, а он требует установленный `vite`
из `node_modules`, поэтому без `npm install` команда завершится ошибкой.

### Если ошибка: `link.exe not found` (MSVC linker)

Это означает, что **не установлены Microsoft C++ Build Tools (MSVC)** или их не видно в PATH.
Tauri на Windows требует именно MSVC‑линкер, одного VS Code недостаточно.

Что сделать:
1. Установить **Build Tools for Visual Studio** и выбрать компонент **Desktop development with C++**
   (он включает `link.exe`).
2. Перезапустить терминал и снова выполнить:
   ```bash
   npm run tauri dev
   ```

### Если ошибка: `icons/icon.ico not found` (tauri-build)

Tauri требует иконки, указанные в `src-tauri/tauri.conf.json`:
```json
"icon": ["icons/icon.ico", "icons/icon.png"]
```
Если файлов нет, сборка падает. Проверьте, что в папке
`src-tauri/icons/` действительно есть `icon.ico` и `icon.png`.
При необходимости создайте/замените их и повторите сборку:
```bash
npm run tauri dev
```

### Если ошибка: `icon.ico is not in 3.00 format` (RC2175)

Это означает, что файл `icon.ico` имеет неподдерживаемый формат.
Для Windows нужен корректный ICO (обычно 256×256, 32‑bit).

Что сделать:
1. Пересоздайте `icon.ico` в правильном формате (например, через ImageMagick
   или любой онлайн‑конвертер в ICO).
2. Замените файл в `src-tauri/icons/icon.ico`.
3. Повторите сборку:
   ```bash
   npm run tauri dev
   ```

## Сборка установщика .exe

```bash
npm run tauri build
```

Сборка создаст установщик в `src-tauri/target/release/bundle`.

## Источники

Вкладка «Законы» принимает ссылки на раздел «Законодательная база» на форуме Majestic
(по одной ссылке на строку). После сохранения нажмите «Скачать / обновить».

## Структура базы данных

- `documents`: документы закона, URL, хэш контента.
- `articles`: статьи документа.
- `memos`: памятки.
- `memo_links`: связи памяток и статей.
- `article_fts`, `memo_fts`: FTS5 индексы.
