# Crew Rotation Digital Twin Demo

## Primary link

Open the presentation scenario directly:

```text
https://des.syntog.ru/demo/?crew=CREW-0004&mode=presentation
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
.\deploy-des.ps1 -HostAlias <ssh-alias> -RemoteDir <remote-directory>
```

The deployment script requires an explicitly configured SSH alias and destination
root. The web server must serve the stable `<remote-directory>/current` symlink;
server configuration is intentionally maintained outside this public repository.
Each deployment uploads an immutable release directory, verifies
`RELEASE-MANIFEST.txt`, atomically activates `current`, and retains the newest
three releases.

Rollback to a retained release:

```powershell
.\deploy-des.ps1 -HostAlias <ssh-alias> -RemoteDir <remote-directory> `
  -RollbackReleaseId <release-id>
```

For a deployed HTTP smoke check, pass `-SmokeUrl <public-url>`; the script
requires Content-Security-Policy and `X-Content-Type-Options: nosniff`.

## Production smoke check

Run this before the presentation:

```powershell
cd C:\project\sd\rzd-des-engine\visualization
node smoke-check-local.mjs
```

The local smoke check validates the current JavaScript asset, network,
simulation, and findings contracts without requiring any private API
credentials. Set `BASE_URL` to a deployed public demo URL when needed.

## Pre-show checklist

- Open the primary URL once before the presentation so the Three.js vendor chunk is cached.
- Test `▶ История рейса`, `Назначить резервную бригаду`, and `↺ Сбросить демо`.
- Use Chrome or Edge with hardware acceleration/WebGL enabled.
- Keep the local fallback URL open in a second tab.
- If WebGL is unavailable, the app switches to the 2D SVG network fallback.
