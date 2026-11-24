# Wave64 → wave32: состояние работы (сессия 24.11, вечер)

## Контекст
Хост (RTX 3060) отказывает пайплайнам с `WaveSize(64)` — максимум 32.
Из-за этого 4 compute-программы билдера RT-структур не создаются →
`builder_runs()` = all-or-nothing → весь билдер off → 718 дропов/кадр →
нет непрямого света (картинка тёмная).

Отказанные (хэши шейдеров из лога, сами дампы не публикуются):
`02384a60fda6f2d0` (group=128), `2cc4c197ee29e291`,
`369cbe906ea72764`, `2da4beb386509b77` (group=64).
Причина отказа (debug layer): «Shader wants wave lane count of 64 but
maximum wave lane count for device is 32».

## Что уже есть в форке (fxceless)
- Халвинг 64→32 в `crates/victory-shader/src/emit/lower.rs` (~1290–1450):
  консольная волна 64 = две host-волны 32, `wave_base` перемаркирует
  thread id, `exchange`-dwords в LDS для кросс-половинных операций
  (`v_mbcnt`, `v_readlane`, маски-как-данные).
- Ограничения халвинга: `crosses` (кросс-половинные операции) и
  разделяемый LDS в group>32 (`lds_ok`/`lds_meetings`).
- `lds_meetings` ищет точки встречи всех лайнов (начало/выход region);
  возвращает None, если в одном lane-branch region есть И чтение И запись
  LDS — барьер нельзя поставить в ветвящемся коде.

## Что сделано нами в этой сессии
1. **Пробник `lds_dump`** (`crates/victory-shader/examples/lds_dump.rs`):
   декодирует .cdxil, печатает все ds_* с полями и 12 инструкций перед
   ними. Запуск: `lds_dump.exe <file.cdxil>`.
2. **Taint-анализ `lds_lane_private(p)`** в `lower.rs` (перед
   `lds_meetings`): адрес каждого ds-доступа трассируется по SSA до
   источников. Taint: 1 = per-lane (entry-VGPR, т.е. thread id), 2 =
   uniform (SGPR/константа), 4 = unknown (память/LDS/vcc-мимо).
   Программа проходит, если КАЖДЫЙ адрес содержит per-lane источник.
   Such LDS needs no ordering between host waves → халвинг без барьеров.
3. **Врезка в решение** (~1404): `lds_private` разрешает и быстрый путь
   (`!crosses && (... || lds_private)`), и `lds_ok`, и убирает
   `split_lds`. Лог: `WAVE64-LDS-PRIVATE group=… lds=…`.
4. Проверено end-to-end: `03e3e26783a5799b` (group=64, lds=128) →
   probe.dxil с `WaveSize=32` (dxc -dumpbin). Раньше остался бы 64.

## Границы (честно)
- 4 отказанных программы билдера НЕ проходят через `lds_lane_private` —
  у них реально разделяемый LDS: например `2cc4c197`: block 0 — lane 0
  (guard `v1|v0 == 0`) пишет 2 dword по константному адресу 0, дальше
  все читают; доступы в divergent-регионе (blocks 1–35) → `lds_meetings`
  = None. Это не баг анализа — так и есть.
- Полностью sound-решение для таких — «секвенциальные сегменты»: одна
  host-волна 32 прогоняет 64 консольных потока двумя проходами через
  тело программы (LDS общий, т.к. один dispatch), барьеры = границы
  сегментов (чередование половин). D3D12-совместимо (LDS живёт в
  dispatch). Это большой кусок эмиттера; автор форка активно работает
  в этой области (коммиты 90228a7, e9765a7 от 24.11).
- `VICTORY_WAVE_LDS_ASSUME=1` — форс-флаг «считать всё lane-private»
  (для экспериментов; карантин `VICTORY_BVH_QUARANTINE=1` удержит мусор
  от записи в память тайтла).

## Скан всех дамп (в процессе)
Прогон пробника по всем дампам с `VICTORY_WAVE_WHY=1`: сколько `ok ->`
(wave32), сколько `WAVE64-WHY`, сколько `LDS-PRIVATE`.
Итог на 24.11: 1163 compute-программы, 1154 wave32-совместимы,
90 остались wave64, 37 прошли через `lds_lane_private`.

## Прочее из сессии (кратко)
- `VICTORY_TEXTURE_NO_WAIT=1`: текстуры 312–545 → 12–23 мс.
- Card-буферы (`VICTORY_CARD_BUFFERS_MIN_MB=8, MB=1536`): heap-pass
  262→15–24 мс, draws 1906→4833, fps 1.8→2.5–4.
- GPU-стены (run4-diag): indirect compute f22a76fc 22.4 мс/кадр
  (145 сэмплов), dispatch 4a05640e 20.2 (culling), mesh ef540905+pixel
  94d28115 14.6, compute cfd83392 10.5. Серебряной пули нет.
- 4 отказанных compute ВАЛИДНЫ по DXIL (validate_dump.exe) — дело только
  в WaveSize. 8 графических невалидны (сигнатуры ISG1/OSG1/PSV0 в
  `emit/container.rs`) — отдельная работа.
- Сейвы пишутся и перезапуски прогресс не теряют.
