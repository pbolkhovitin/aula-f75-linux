# AULA F75 Status — виджет Noctalia

Виджет для бара Noctalia: показывает подключение клавиатуры AULA F75, активный
профиль, эффект подсветки, яркость, переназначенные клавиши и макросы. Правый
клик или правый клик открывает панель с подробностями; кнопка в панели запускает F75 Control Center.

Плагин не работает с hidraw напрямую и не дублирует протокол. Он вызывает
`f75-control --status` — тот же бинарник, что и графическое приложение, — и
разбирает один JSON-объект. Поэтому приложение остаётся единственным источником
истины: discovery, `state.json`, права доступа и ограничения прошивки описаны
только в нём.

```
┌────────────────────┐   --status (JSON, schema 1)   ┌──────────────────┐
│ F75 Control Center │ ────────────────────────────▶ │ Noctalia widget  │
│  (Rust, egui)      │                               │  (Luau, бар)     │
└────────────────────┘                               └──────────────────┘
```

## Что показывает

- значок `keyboard` / `keyboard-off` и цвет по состоянию:
  - зелёный (`primary`) — запись настроек доступна (USB);
  - жёлтый (`secondary`) — приёмник 2,4 ГГц, запись фактически не работает;
  - серый — клавиатура не найдена;
  - красный — нет прав на `/dev/hidraw*`;
- текст в баре (настраивается): `F75 · Default · Flash away`,
  `Default · Flash away 100%` или только значок;
- подсказка: профиль, эффект, яркость, скорость, цвет, переназначения,
  макросы, антидребезг, сон, предупреждения (например, про приёмник);
- панель по клику: состояние подсветки, переназначения, макросы и подсказка, если канал только для чтения.

## Установка

Нужны собранный `f75-control` и Noctalia 5.x с поддержкой `plugin_api >= 43`.

```bash
# 1. Собрать приложение (если ещё не собрано)
cargo build --release --manifest-path ../Cargo.toml

# 2. Положить бинарник в PATH
mkdir -p ~/.local/bin
ln -sfn "$PWD/../target/release/f75-control" ~/.local/bin/f75-control
f75-control --status | head -c 200

# 3. Подключить плагин как локальный источник
mkdir -p ~/.local/share/noctalia-plugin-source-f75
ln -sfn "$PWD" ~/.local/share/noctalia-plugin-source-f75/aula-f75
noctalia msg plugins source remove f75 2>/dev/null || true
noctalia msg plugins source add f75 path ~/.local/share/noctalia-plugin-source-f75
noctalia msg plugins enable pbolkhovitin/aula-f75

# 4. Добавить виджет в бар
```

Шаг 4 проще сделать в GUI: **Settings → Bar → Add widget → AULA F75 Status**.
Если править файл вручную, бар хранится не в `~/.config/noctalia/config.toml`,
а в `~/.local/state/noctalia/settings.toml`:

```toml
[bar.default]
end = [ "tray", "clipboard", "group:g3", "widget_2", "recorder", "f75" ]

[widget.f75]
type = "pbolkhovitin/aula-f75:status"
```

После правки перезапустите Noctalia (или `noctalia msg config-reload`).

Готовый скрипт для шагов 2–3: `./install.sh`.

## Настройки виджета

| Ключ | Тип | По умолчанию | Смысл |
|---|---|---|---|
| `command` | string | `f75-control` | имя в PATH или полный путь |
| `refresh_seconds` | int 2..120 | `5` | период опроса |
| `display` | select | `full` | `full`, `compact`, `glyph` |
| `show_profile` | bool | `true` | имя профиля в баре |
| `show_effect` | bool | `true` | эффект в баре |
| `show_connection` | bool | `false` | USB / 2,4 ГГц в баре |
| `colorize` | bool | `true` | цвет по состоянию |
| `hide_when_disconnected` | bool | `false` | скрывать без клавиатуры |
| `open_app_on_click` | bool | `false` | клик запускает приложение вместо панели |

Настройки лежат в state-каталоге Noctalia, а не в этом репозитории.

## Контракт `--status`

`f75-control --status` печатает один объект, `--status --pretty` — с отступами.
`--watch --interval 3000` печатает новую строку только при изменении состояния.
Поля, на которые опирается виджет:

- `schema` — версия контракта (сейчас `1`);
- `device.present`, `device.transport` (`usb` / `wireless` / `none`),
  `device.access` (`ok` / `denied` / `error` / `no-control` / `missing`),
  `device.write_supported`, `device.control_path`, `device.keyboard_path`,
  `device.firmware`, `device.layout`;
- `state.profile.name`, `state.lighting.effect_name`,
  `state.lighting.brightness_percent`, `state.lighting.speed`,
  `state.lighting.color`, `state.keys.remapped`, `state.keys.remapped_fn`,
  `state.macros.count`, `state.macros.steps`, `state.prefs.debounce_ms`,
  `state.prefs.sleep_seconds`;
- `warnings[]` — человекочитаемые предупреждения.

Команда ничего не пишет на клавиатуру и не открывает устройство на запись,
поэтому её безопасно вызывать каждые несколько секунд.

## Разработка

```bash
noctalia plugins lint .     # манифест, переводы, entry-файлы
```

Правки `.luau` подхватываются Noctalia на лету; при изменении `plugin.toml`
нужен перезапуск. Логи: запустите Noctalia с `-d` и смотрите stderr.

## Ограничения

- Без USB-кабеля запись настроек через приёмник 2,4 ГГц всё равно упирается в
  таймаут — виджет честно показывает «только чтение».
- Модель и прошивка в статусе — константы приложения (F75 / 0x18), а не ответ
  устройства: вендорский протокол не отдаёт их отдельным запросом.
- Виджет показывает сохранённое состояние приложения. Если на клавиатуре
  включён аппаратный Gaming Mode (ручка регулирует яркость вместо громкости),
  это режим прошивки, а не настройка приложения; переключается удержанием ручки
  3–5 секунд.
