# 0.2.0

## Changed

- Rename `AtomicArc`/`AtomicArcOption` method `load_if_outdated` into `load_cached_or_reload` ([#2](https://github.com/wyfo/hazarc/pull/2))

## Added

- Add `Cache::load_shared` which takes a shared reference and don't update cache ([#3](https://github.com/wyfo/hazarc/pull/2))