# SAV station vehicle history — issue #397

Updated: 2026-09-06

The SAV bridge now reads `STNN.normal.had_vehicle_of_type` for modern and
legacy station rows, hydrates it into `Station.had_vehicle_of_type`, and emits
the native byte again when writing `STNN`. Missing fields retain the OpenTTD
zero default. A parser fixture and a writer→parser round-trip cover a non-zero
bitset (`0x2A`/`0x0E`) without changing station/order index contracts.

This is only the `0x8A` history byte. `last_vehicle_type`, native road-stop
status (`0xF2`/`0xF3`) and the remaining station scopes are still tracked by
parent issue #329.
