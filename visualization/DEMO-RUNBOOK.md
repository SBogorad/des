# Crew Rotation Digital Twin Demo

## Primary link

Open the presentation scenario directly:

```text
https://syntog.ru/viz/?crew=CREW-0004&mode=presentation
```

CED mirror:

```text
https://ced.syntog.ru/viz/?crew=CREW-0004&mode=presentation
```

The page uses only synthetic data from the `Северная синтетическая дорога 88_05` scenario.

## Three-minute show flow

1. Open the primary link. The selected synthetic crew and its locomotive are already in focus.
2. Point out the KPI strip: 25 crews, 50 trips, three review cases, and the synthetic risk estimate.
3. Click `▶ История рейса`. The scene walks through locomotive acceptance, travel, and the control point.
4. Open `◱ Рабочий режим` to show the crew card and `Шкала смены`.
5. At the finding, explain that the system has detected a synthetic work/rest risk at the control station.
6. Click `Назначить резервную бригаду`.
7. Return to presentation mode. The control station and critical route leg turn green and the result confirms up to 42 minutes of synthetic overtime risk prevented.

## Speaker script

> Цифровой двойник контролирует оборот локомотивных бригад по всей синтетической дороге.
>
> Система сопоставляет маршрут, время работы и режим отдыха. На этом рейсе она выявила риск на контрольной станции Иркивальди.
>
> Откроем историю рейса: бригада принимает локомотив, следует по плечу, а затем система фиксирует точку, требующую диспетчерской проверки.
>
> В рабочем режиме видна вся смена: подготовка, поездка, оборот и возврат. Риск связан с режимом труда и отдыха на следующем плече.
>
> Смоделируем назначение резервной бригады. Контрольная точка и проблемное плечо становятся зелёными.
>
> Таким образом система не только обнаруживает отклонение, но и показывает эффект управленческого решения: до 42 минут синтетического риска переработки предотвращено.

## Controls

- `▣ Режим презентации`: clean projector view with KPIs, map, and narrative.
- `◱ Рабочий режим`: full operational interface, filters, timeline, and crew card.
- `▶ История рейса`: plays the guided investigation sequence.
- `⚠ Показать проблемный рейс`: jumps to the deterministic critical synthetic case.
- `↺ Сбросить демо`: clears filters, selection, what-if decision, narrative, and returns time to the scenario start.

## Offline fallback

The deployed site is static. A local copy can be served from the current build:

```powershell
cd C:\project\sd\rzd-des-engine\visualization
node node_modules\vite\bin\vite.js --host 127.0.0.1 --port 5201
```

Open the same scenario locally:

```text
http://127.0.0.1:5201/?crew=CREW-0004&mode=presentation
```

For a stable offline production preview:

```powershell
cd C:\project\sd\rzd-des-engine\visualization
$env:BASE = '/'
node node_modules\vite\bin\vite.js preview --host 127.0.0.1 --port 5201
```

Build the production artifact for deployment:

```powershell
cd C:\project\sd\rzd-des-engine\visualization
$env:BASE = '/viz/'
node node_modules\vite\bin\vite.js build
```

## Repeat deployment

```powershell
cd C:\project\sd\rzd-des-engine\visualization
.\deploy-main.ps1
```

This primary-domain script rebuilds for `/viz/`, copies the static files to `/var/www/site/ced-hub/viz/` through the existing `stroy` SSH alias, and confirms the current main asset is present. It does not touch Docker services or nginx.

For the CED mirror, use `./deploy.ps1`. It publishes to `/opt/ced/viz/`.

## Production smoke check

Run this before the presentation:

```powershell
cd C:\project\sd\rzd-des-engine\visualization
.\smoke-check.ps1 -BaseUrl https://syntog.ru -SkipApiCheck
.\smoke-check.ps1 -BaseUrl https://ced.syntog.ru
```

The first command verifies the main-domain static visualization only because `syntog.ru/api` requires authorization. The second also verifies CED API health. Both checks validate the current JavaScript asset, network, simulation, and findings contracts.

## Pre-show checklist

- Open the primary URL once before the presentation so the Three.js vendor chunk is cached.
- Test `▶ История рейса`, `Назначить резервную бригаду`, and `↺ Сбросить демо`.
- Use Chrome or Edge with hardware acceleration/WebGL enabled.
- Keep the local fallback URL open in a second tab.
- If WebGL is unavailable, the app switches to the 2D SVG network fallback.
