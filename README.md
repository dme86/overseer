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
https://dme86.github.io/overseer/de-DE/atomic-shop/archive/YYYY-MM.json
https://dme86.github.io/overseer/en-US/atomic-shop/archive/YYYY-MM.json
```

### Events

```text
https://dme86.github.io/overseer/de-DE/events/current.json
https://dme86.github.io/overseer/en-US/events/current.json
```

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
