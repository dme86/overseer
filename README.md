# Overseer ☢️

Overseer provides machine-readable Fallout 76 data as static JSON.

No authentication, API key or backend is required.

## Base URL

```text
https://dme86.github.io/overseer/
```

Currently supported locales:

- `de-DE`
- `en-US`

## Endpoints

### Minerva

Current Minerva sale:

```text
https://dme86.github.io/overseer/de-DE/minerva/current.json
https://dme86.github.io/overseer/en-US/minerva/current.json
```

Next known Minerva sale:

```text
https://dme86.github.io/overseer/de-DE/minerva/next.json
https://dme86.github.io/overseer/en-US/minerva/next.json
```

Known Minerva schedule:

```text
https://dme86.github.io/overseer/de-DE/minerva/schedule.json
https://dme86.github.io/overseer/en-US/minerva/schedule.json
```

Historical Minerva snapshots are stored by year and month:

```text
/de-DE/minerva/archive/YYYY/MM/<timestamp>.json
/en-US/minerva/archive/YYYY/MM/<timestamp>.json
```

### Atomic Shop

Currently available offers:

```text
https://dme86.github.io/overseer/de-DE/atomic-shop/current.json
https://dme86.github.io/overseer/en-US/atomic-shop/current.json
```

Upcoming known offers:

```text
https://dme86.github.io/overseer/de-DE/atomic-shop/upcoming.json
https://dme86.github.io/overseer/en-US/atomic-shop/upcoming.json
```

Monthly archives:

```text
https://dme86.github.io/overseer/de-DE/atomic-shop/archive/YYYY/MM.json
https://dme86.github.io/overseer/en-US/atomic-shop/archive/YYYY/MM.json
```

Example:

```text
https://dme86.github.io/overseer/de-DE/atomic-shop/archive/2026/09.json
```

Atomic Shop archives contain the published monthly schedule and do not contain dynamic `active` state.

### Events

Current event data:

```text
https://dme86.github.io/overseer/de-DE/events/current.json
https://dme86.github.io/overseer/en-US/events/current.json
```

Historical event snapshots are stored by year and month:

```text
/de-DE/events/archive/YYYY/MM/<timestamp>.json
/en-US/events/archive/YYYY/MM/<timestamp>.json
```

Each event includes a machine-readable `tags` array. Known values include
`treasure_hunter`, `double_score`, `double_xp`, `mutated_events`,
`double_mutations`, `spooky_scorched`, `fasnacht`, `minerva`,
`seasonal_event`, and `maintenance`.

### Challenges

Global/default daily and weekly challenges (personal rerolls are not included):

```text
https://dme86.github.io/overseer/de-DE/challenges/daily.json
https://dme86.github.io/overseer/en-US/challenges/daily.json
https://dme86.github.io/overseer/de-DE/challenges/weekly.json
https://dme86.github.io/overseer/en-US/challenges/weekly.json
```

Challenge archives use the daily or weekly reset period start, not the sync time:

```text
/{locale}/challenges/daily/archive/YYYY/MM/DD.json
/{locale}/challenges/weekly/archive/YYYY/MM/DD.json
```

### Daily Ops

```text
https://dme86.github.io/overseer/de-DE/daily-ops/current.json
https://dme86.github.io/overseer/en-US/daily-ops/current.json
/{locale}/daily-ops/archive/YYYY/MM/DD.json
```

### Nuke codes

```text
https://dme86.github.io/overseer/de-DE/nuke-codes/current.json
https://dme86.github.io/overseer/en-US/nuke-codes/current.json
/{locale}/nuke-codes/archive/YYYY/MM/DD.json
```

The archive date is the beginning of the codes' validity period.

### Season

```text
https://dme86.github.io/overseer/de-DE/season/current.json
https://dme86.github.io/overseer/en-US/season/current.json
/{locale}/season/archive/<season-number>.json
```

All archives above are stable semantic-period files. A later sync updates the
same archive when its source data changes rather than creating a run snapshot.

## Discovery

Root index:

```text
https://dme86.github.io/overseer/index.json
```

Locale indexes:

```text
https://dme86.github.io/overseer/de-DE/index.json
https://dme86.github.io/overseer/en-US/index.json
```
