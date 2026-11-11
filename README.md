# Music Cloud Core

Общие библиотеки Music Cloud.

- `mc-core` — доменные типы, ID, ошибки.
- `mc-auth` — пароли, сессии, Subsonic-токены.
- `mc-crypto` — шифрование секретов.
- `mc-db` — PostgreSQL, миграции, репозитории.
- `mc-jobs` — очередь заданий.
- `mc-media` — теги и обложки.
- `mc-proto` — gRPC-контракты.
- `mc-runtime` — реестр хранилищ.
- `mc-scanner` — сканирование библиотек.
- `mc-storage` — local/memory/S3.
- `mc-telemetry` — логи, health, метрики.

Сервисы подключают крейты через git-тег:

```toml
mc-core = { git = "ssh://git@github.com/moshms/core.git", tag = "v0.1.0" }
```

CI: fmt, clippy, тесты на всех ветках и PR.
