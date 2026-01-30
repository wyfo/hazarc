# Benchmark

The following benchmark compares `hazarc` with `arc-swap`, but also with standard `RwLock<Arc<T>>`. Refer to the [code](https://github.com/wyfo/hazarc/blob/main/benches/comparison.rs) for details of the benchmarked functions. `AtomicArc` uses the default write policy, `Concurrent`; guards (`RwLockReadGuard`/`ArcBorrow`/etc.) destructors are included in the measurement.

## Results

### Apple M3 (aarch64)

```
Timer precision: 41 ns
comparison                       fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ arcswap_load                  1.91 ns       │ 5.531 ns      │ 1.93 ns       │ 1.95 ns       │ 9883    │ 20240384
├─ arcswap_load_contended        1.991 ns      │ 12.24 ns      │ 5.694 ns      │ 5.784 ns      │ 13659   │ 13986816
├─ arcswap_load_no_slot          5.125 ns      │ 39.62 ns      │ 5.694 ns      │ 5.795 ns      │ 13584   │ 13910016
├─ arcswap_load_no_slot_spin     14.84 ns      │ 219.1 ns      │ 15.98 ns      │ 16.01 ns      │ 22309   │ 5711104
├─ arcswap_load_none             1.91 ns       │ 7.749 ns      │ 2.093 ns      │ 2.074 ns      │ 14177   │ 29034496
├─ arcswap_load_spin             10.94 ns      │ 36.65 ns      │ 11.1 ns       │ 11.23 ns      │ 15483   │ 7927296
├─ arcswap_store                               │               │               │               │         │
│  ├─ 0                          58.95 ns      │ 149.1 ns      │ 59.61 ns      │ 59.87 ns      │ 12716   │ 1627648
│  ├─ 1                          54.72 ns      │ 123.7 ns      │ 59.28 ns      │ 59.35 ns      │ 12823   │ 1641344
│  ├─ 2                          75.22 ns      │ 202.1 ns      │ 76.54 ns      │ 77.01 ns      │ 19814   │ 1268096
│  ├─ 4                          116.9 ns      │ 340.8 ns      │ 119.4 ns      │ 119.4 ns      │ 25648   │ 820736
│  ├─ 8                          200.2 ns      │ 1.386 µs      │ 223.6 ns      │ 239.9 ns      │ 12894   │ 412608
│  ╰─ 16                         369.4 ns      │ 2.752 µs      │ 416.4 ns      │ 412.6 ns      │ 15021   │ 240336
├─ arcswap_store_contended                     │               │               │               │         │
│  ├─ 0                          369.4 ns      │ 882.5 ns      │ 374.7 ns      │ 375.5 ns      │ 16493   │ 263888
│  ├─ 1                          382.5 ns      │ 903.3 ns      │ 458 ns        │ 458.4 ns      │ 13532   │ 216512
│  ├─ 2                          489.2 ns      │ 1.671 µs      │ 562.2 ns      │ 562 ns        │ 22006   │ 176048
│  ├─ 4                          478.7 ns      │ 7.749 µs      │ 1.03 µs       │ 1.207 µs      │ 20482   │ 81928
│  ├─ 8                          1.218 µs      │ 1.998 ms      │ 2.395 µs      │ 2.555 µs      │ 9706    │ 38824
│  ╰─ 16                         645.2 ns      │ 14.35 ms      │ 2.208 µs      │ 6.713 µs      │ 7605    │ 15210
├─ hazarc_load                   0.689 ns      │ 2.805 ns      │ 0.77 ns       │ 0.761 ns      │ 11620   │ 47595520
├─ hazarc_load_contended         0.71 ns       │ 20.36 ns      │ 4.046 ns      │ 4.493 ns      │ 8375    │ 17152000
├─ hazarc_load_no_slot           5.816 ns      │ 58.5 ns       │ 6.019 ns      │ 6.132 ns      │ 13001   │ 13313024
├─ hazarc_load_no_slot_spin      15.82 ns      │ 46.09 ns      │ 15.99 ns      │ 16.14 ns      │ 22176   │ 5677056
├─ hazarc_load_none              0.203 ns      │ 0.664 ns      │ 0.206 ns      │ 0.208 ns      │ 4093    │ 67059712
├─ hazarc_load_pthread           2.845 ns      │ 10.82 ns      │ 2.927 ns      │ 3.419 ns      │ 20195   │ 20679680
├─ hazarc_load_pthread_unsafe    0.933 ns      │ 2.327 ns      │ 1.045 ns      │ 1.034 ns      │ 10211   │ 41824256
├─ hazarc_load_spin              9.234 ns      │ 26.32 ns      │ 9.396 ns      │ 9.454 ns      │ 18032   │ 9232384
├─ hazarc_store                                │               │               │               │         │
│  ├─ 0                          6.671 ns      │ 17.57 ns      │ 7.24 ns       │ 7.099 ns      │ 11500   │ 11776000
│  ├─ 1                          9.316 ns      │ 56.84 ns      │ 9.56 ns       │ 9.749 ns      │ 17552   │ 8986624
│  ├─ 2                          15.98 ns      │ 260.4 ns      │ 16.31 ns      │ 16.4 ns       │ 21849   │ 5593344
│  ├─ 4                          20.54 ns      │ 109.2 ns      │ 21.52 ns      │ 21.63 ns      │ 16905   │ 4327680
│  ├─ 8                          34.86 ns      │ 110.3 ns      │ 35.51 ns      │ 35.54 ns      │ 20998   │ 2687744
│  ╰─ 16                         65.46 ns      │ 187.2 ns      │ 66.77 ns      │ 66.94 ns      │ 22641   │ 1449024
├─ hazarc_store_contended                      │               │               │               │         │
│  ├─ 0                          65.46 ns      │ 185.9 ns      │ 66.77 ns      │ 67 ns         │ 22628   │ 1448192
│  ├─ 1                          67.4 ns       │ 395.5 ns      │ 165 ns        │ 165 ns        │ 18664   │ 597248
│  ├─ 2                          75.21 ns      │ 2.499 µs      │ 218.4 ns      │ 220.3 ns      │ 27897   │ 446352
│  ├─ 4                          140.3 ns      │ 3.275 µs      │ 645.4 ns      │ 703.2 ns      │ 17591   │ 140728
│  ├─ 8                          239.2 ns      │ 292.5 µs      │ 1.468 µs      │ 1.487 µs      │ 16582   │ 66328
│  ╰─ 16                         270.4 ns      │ 4.587 ms      │ 1.489 µs      │ 2.65 µs       │ 7344    │ 29376
├─ rwlock_read                                 │               │               │               │         │
│  ├─ t=1                        4.31 ns       │ 34.09 ns      │ 4.391 ns      │ 4.407 ns      │ 16992   │ 17399808
│  ├─ t=2                        4.514 ns      │ 150.9 ns      │ 19.4 ns       │ 20.78 ns      │ 6042    │ 6187008
│  ├─ t=4                        4.259 ns      │ 351.2 ns      │ 6.869 ns      │ 24.56 ns      │ 19268   │ 1233152
│  ├─ t=8                        4.838 ns      │ 1.765 µs      │ 15.33 ns      │ 31.41 ns      │ 14664   │ 117312
│  ╰─ t=16                       3.619 ns      │ 4.021 µs      │ 25.74 ns      │ 329.6 ns      │ 14448   │ 462336
├─ rwlock_read_clone                           │               │               │               │         │
│  ├─ t=1                        6.874 ns      │ 21.48 ns      │ 6.956 ns      │ 6.996 ns      │ 11722   │ 12003328
│  ├─ t=2                        6.548 ns      │ 190.1 ns      │ 9.15 ns       │ 8.899 ns      │ 22436   │ 2871808
│  ├─ t=4                        6.213 ns      │ 623.4 ns      │ 10.15 ns      │ 75.92 ns      │ 18776   │ 600832
│  ├─ t=8                        4.838 ns      │ 2.359 µs      │ 15.33 ns      │ 111 ns        │ 14808   │ 118464
│  ╰─ t=16                       4.9 ns        │ 4.543 µs      │ 193.7 ns      │ 539.2 ns      │ 14992   │ 239872
├─ rwlock_read_clone_spin                      │               │               │               │         │
│  ├─ t=1                        12 ns         │ 27.46 ns      │ 12.32 ns      │ 12.39 ns      │ 14183   │ 7261696
│  ├─ t=2                        12.73 ns      │ 165.4 ns      │ 109 ns        │ 104.8 ns      │ 5664    │ 1449984
│  ├─ t=4                        14.02 ns      │ 721 ns        │ 110.4 ns      │ 157 ns        │ 16040   │ 513280
│  ├─ t=8                        15.33 ns      │ 2.773 µs      │ 197.6 ns      │ 300.9 ns      │ 13240   │ 211840
│  ╰─ t=16                       15.33 ns      │ 4.754 µs      │ 56.96 ns      │ 410.3 ns      │ 16272   │ 130176
├─ rwlock_read_contended                       │               │               │               │         │
│  ├─ t=1                        4.267 ns      │ 138.8 ns      │ 6.873 ns      │ 6.278 ns      │ 50520   │ 12933120
│  ├─ t=2                        4.923 ns      │ 166.7 ns      │ 10.45 ns      │ 16.25 ns      │ 18692   │ 2392576
│  ├─ t=4                        4.275 ns      │ 568 ns        │ 49.19 ns      │ 74.5 ns       │ 12484   │ 798976
│  ├─ t=8                        4.838 ns      │ 3.405 µs      │ 30.96 ns      │ 98.92 ns      │ 15672   │ 125376
│  ╰─ t=16                       2.275 ns      │ 4.299 µs      │ 33.58 ns      │ 255.3 ns      │ 15904   │ 254464
├─ rwlock_read_contended_clone                 │               │               │               │         │
│  ├─ t=1                        6.873 ns      │ 194 ns        │ 11.75 ns      │ 13.14 ns      │ 26752   │ 6848512
│  ├─ t=2                        7.853 ns      │ 272.8 ns      │ 20.22 ns      │ 36.45 ns      │ 15058   │ 1927424
│  ├─ t=4                        7.525 ns      │ 1.407 µs      │ 159.8 ns      │ 214.2 ns      │ 11924   │ 381568
│  ├─ t=8                        9.963 ns      │ 7.03 µs       │ 93.46 ns      │ 276.4 ns      │ 15936   │ 63744
│  ╰─ t=16                       4.963 ns      │ 6.708 µs      │ 171.5 ns      │ 583.1 ns      │ 15568   │ 124544
├─ rwlock_read_spin                            │               │               │               │         │
│  ├─ t=1                        8.502 ns      │ 36.33 ns      │ 8.664 ns      │ 8.69 ns       │ 19397   │ 9931264
│  ├─ t=2                        9.966 ns      │ 96.23 ns      │ 50.82 ns      │ 50.73 ns      │ 9498    │ 2431488
│  ├─ t=4                        10.11 ns      │ 387 ns        │ 104.5 ns      │ 103.4 ns      │ 14888   │ 952832
│  ├─ t=8                        10.11 ns      │ 2.011 µs      │ 161.1 ns      │ 225.9 ns      │ 12440   │ 398080
│  ╰─ t=16                       10.11 ns      │ 5.295 µs      │ 399.4 ns      │ 669.9 ns      │ 12464   │ 398848
├─ rwlock_write                                │               │               │               │         │
│  ├─ 0                          4.676 ns      │ 30.75 ns      │ 5.124 ns      │ 5.102 ns      │ 15034   │ 15394816
│  ├─ 1                          4.676 ns      │ 23.39 ns      │ 5.084 ns      │ 5.008 ns      │ 15296   │ 15663104
│  ├─ 2                          4.676 ns      │ 25.91 ns      │ 4.962 ns      │ 4.95 ns       │ 15468   │ 15839232
│  ├─ 4                          4.676 ns      │ 12.61 ns      │ 4.798 ns      │ 4.916 ns      │ 15569   │ 15942656
│  ├─ 8                          4.676 ns      │ 15.58 ns      │ 5.084 ns      │ 4.987 ns      │ 15362   │ 15730688
│  ╰─ 16                         4.676 ns      │ 12.24 ns      │ 4.839 ns      │ 4.951 ns      │ 15465   │ 15836160
├─ rwlock_write_contended                      │               │               │               │         │
│  ├─ 0                          4.676 ns      │ 15.7 ns       │ 4.839 ns      │ 4.954 ns      │ 15452   │ 15822848
│  ├─ 1                          40.71 ns      │ 195.9 ms      │ 40.71 ns      │ 2.634 ms      │ 100     │ 100
│  ├─ 2                          40.71 ns      │ 199.6 µs      │ 40.71 ns      │ 378.2 ns      │ 240854  │ 240854
│  ├─ 4                          40.71 ns      │ 10.01 ms      │ 40.71 ns      │ 12.14 µs      │ 8559    │ 8559
│  ├─ 8                          40.71 ns      │ 10.18 ms      │ 40.71 ns      │ 49.42 µs      │ 2023    │ 2023
│  ╰─ 16                         40.71 ns      │ 17.87 ms      │ 40.71 ns      │ 25.29 µs      │ 3953    │ 3953
╰─ rwlock_write_contended_clone                │               │               │               │         │
   ├─ 0                          5.246 ns      │ 14.93 ns      │ 5.491 ns      │ 5.499 ns      │ 13787   │ 14117888
   ├─ 1                          5.408 ns      │ 65.87 ns      │ 8.746 ns      │ 13.86 ns      │ 12606   │ 6454272
   ├─ 2                          5.736 ns      │ 535 ns        │ 21.52 ns      │ 37.78 ns      │ 9864    │ 2525184
   ├─ 4                          40.71 ns      │ 122.8 µs      │ 124.7 ns      │ 1.037 µs      │ 92361   │ 92361
   ├─ 8                          40.71 ns      │ 10.1 ms       │ 124.7 ns      │ 17.37 µs      │ 5741    │ 5741
   ╰─ 16                         40.71 ns      │ 21.13 ms      │ 249.7 ns      │ 325.5 µs      │ 311     │ 311
```

### AMD Ryzen 7 5800X (x86_64)

```
Timer precision: 20 ns
comparison                       fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ arcswap_load                  3.699 ns      │ 13.21 ns      │ 3.779 ns      │ 3.795 ns      │ 33597   │ 17201664
├─ arcswap_load_contended        3.74 ns       │ 35.71 ns      │ 17.98 ns      │ 18.09 ns      │ 19576   │ 5011456
├─ arcswap_load_no_slot          13.56 ns      │ 31.49 ns      │ 13.75 ns      │ 13.77 ns      │ 24999   │ 6399744
├─ arcswap_load_no_slot_spin     24.39 ns      │ 58.52 ns      │ 24.48 ns      │ 24.48 ns      │ 29302   │ 3750656
├─ arcswap_load_none             3.738 ns      │ 9.238 ns      │ 3.818 ns      │ 3.819 ns      │ 35530   │ 18191360
├─ arcswap_load_spin             19.15 ns      │ 49.29 ns      │ 19.16 ns      │ 19.2 ns       │ 36505   │ 4672640
├─ arcswap_store                               │               │               │               │         │
│  ├─ 0                          58.91 ns      │ 109 ns        │ 59.85 ns      │ 59.88 ns      │ 24695   │ 1580480
│  ├─ 1                          58.91 ns      │ 143.9 ns      │ 59.85 ns      │ 59.94 ns      │ 24664   │ 1578496
│  ├─ 2                          80.52 ns      │ 187.3 ns      │ 82.09 ns      │ 82.03 ns      │ 36003   │ 1152096
│  ├─ 4                          123.6 ns      │ 467.4 ns      │ 126.2 ns      │ 126.4 ns      │ 46719   │ 747504
│  ├─ 8                          230.1 ns      │ 595.2 ns      │ 235.1 ns      │ 235.2 ns      │ 25766   │ 412256
│  ╰─ 16                         429.7 ns      │ 6.451 µs      │ 470.7 ns      │ 474.2 ns      │ 175849  │ 175849
├─ arcswap_store_contended                     │               │               │               │         │
│  ├─ 0                          435.4 ns      │ 1.082 µs      │ 451.7 ns      │ 452.6 ns      │ 26880   │ 215040
│  ├─ 1                          457.9 ns      │ 1.592 µs      │ 520.7 ns      │ 523.7 ns      │ 45711   │ 182844
│  ├─ 2                          470.4 ns      │ 2.639 µs      │ 555.7 ns      │ 560.5 ns      │ 42834   │ 171336
│  ├─ 4                          528.2 ns      │ 1.855 µs      │ 658.4 ns      │ 662.5 ns      │ 36459   │ 145836
│  ├─ 8                          570.7 ns      │ 3.18 µs       │ 866.2 ns      │ 870.3 ns      │ 27867   │ 111468
│  ╰─ 16                         720.7 ns      │ 3.175 µs      │ 1.231 µs      │ 1.241 µs      │ 38605   │ 77210
├─ hazarc_load                   3.896 ns      │ 18.12 ns      │ 4.15 ns       │ 4.157 ns      │ 33447   │ 17124864
├─ hazarc_load_contended         4.13 ns       │ 62.13 ns      │ 26.99 ns      │ 26.86 ns      │ 26874   │ 3439872
├─ hazarc_load_no_slot           9.646 ns      │ 27.76 ns      │ 9.845 ns      │ 9.852 ns      │ 33411   │ 8553216
├─ hazarc_load_no_slot_spin      23.92 ns      │ 97.74 ns      │ 24 ns         │ 23.99 ns      │ 30114   │ 3854592
├─ hazarc_load_none              0.251 ns      │ 0.968 ns      │ 0.264 ns      │ 0.262 ns      │ 15858   │ 64954368
├─ hazarc_load_pthread           5.011 ns      │ 12.25 ns      │ 5.07 ns       │ 5.078 ns      │ 30002   │ 15361024
├─ hazarc_load_pthread_unsafe    3.974 ns      │ 10.76 ns      │ 4.267 ns      │ 4.277 ns      │ 34203   │ 17511936
├─ hazarc_load_spin              18.13 ns      │ 44.67 ns      │ 18.21 ns      │ 18.21 ns      │ 38727   │ 4957056
├─ hazarc_store                                │               │               │               │         │
│  ├─ 0                          9.884 ns      │ 22.52 ns      │ 10.47 ns      │ 10.48 ns      │ 31624   │ 8095744
│  ├─ 1                          9.568 ns      │ 24.79 ns      │ 9.806 ns      │ 10.59 ns      │ 31330   │ 8020480
│  ├─ 2                          12.15 ns      │ 27.29 ns      │ 12.58 ns      │ 13 ns         │ 26257   │ 6721792
│  ├─ 4                          21.12 ns      │ 125.9 ns      │ 21.59 ns      │ 23.1 ns       │ 30832   │ 3946496
│  ├─ 8                          47.32 ns      │ 136.1 ns      │ 47.49 ns      │ 48.75 ns      │ 30204   │ 1933056
│  ╰─ 16                         75.49 ns      │ 519.4 ns      │ 78.06 ns      │ 81.38 ns      │ 70576   │ 1129216
├─ hazarc_store_contended                      │               │               │               │         │
│  ├─ 0                          75.18 ns      │ 335.3 ns      │ 77.99 ns      │ 81.11 ns      │ 36593   │ 1170976
│  ├─ 1                          78.68 ns      │ 578.3 ns      │ 153.8 ns      │ 156.1 ns      │ 38330   │ 613280
│  ├─ 2                          77.43 ns      │ 521.9 ns      │ 229.5 ns      │ 229.5 ns      │ 26428   │ 422848
│  ├─ 4                          78.62 ns      │ 863.8 ns      │ 337.8 ns      │ 338.4 ns      │ 35641   │ 285128
│  ├─ 8                          172.4 ns      │ 1.94 µs       │ 530.7 ns      │ 534.8 ns      │ 44437   │ 177748
│  ╰─ 16                         310.7 ns      │ 3.245 µs      │ 861.2 ns      │ 868.9 ns      │ 54135   │ 108270
├─ rwlock_read                                 │               │               │               │         │
│  ├─ t=1                        4.169 ns      │ 16.1 ns       │ 4.228 ns      │ 4.233 ns      │ 32940   │ 16865280
│  ├─ t=2                        4.287 ns      │ 67.21 ns      │ 23.45 ns      │ 22.98 ns      │ 26650   │ 3411200
│  ├─ t=4                        5.373 ns      │ 461.8 ns      │ 132.4 ns      │ 129.6 ns      │ 36820   │ 589120
│  ├─ t=8                        5.373 ns      │ 444.9 ns      │ 48.62 ns      │ 106.7 ns      │ 30136   │ 482176
│  ╰─ t=16                       4.451 ns      │ 1.107 µs      │ 744.8 ns      │ 663.4 ns      │ 13072   │ 836608
├─ rwlock_read_clone                           │               │               │               │         │
│  ├─ t=1                        7.65 ns       │ 32.66 ns      │ 7.81 ns       │ 7.818 ns      │ 40359   │ 10331904
│  ├─ t=2                        8.513 ns      │ 119 ns        │ 46.24 ns      │ 46.45 ns      │ 26784   │ 1714176
│  ├─ t=4                        10.99 ns      │ 987.8 ns      │ 173.8 ns      │ 174.2 ns      │ 40292   │ 322336
│  ├─ t=8                        9.748 ns      │ 577.7 ns      │ 223.2 ns      │ 191.2 ns      │ 27216   │ 435456
│  ╰─ t=16                       9.748 ns      │ 1.086 µs      │ 16.62 ns      │ 146.6 ns      │ 21376   │ 342016
├─ rwlock_read_clone_spin                      │               │               │               │         │
│  ├─ t=1                        18.6 ns       │ 132.7 ns      │ 18.69 ns      │ 18.71 ns      │ 37765   │ 4833920
│  ├─ t=2                        18.84 ns      │ 161.4 ns      │ 75.2 ns       │ 75.26 ns      │ 21368   │ 1367552
│  ├─ t=4                        19.12 ns      │ 652.2 ns      │ 175.6 ns      │ 178.8 ns      │ 35612   │ 569792
│  ├─ t=8                        19.12 ns      │ 639.7 ns      │ 271.4 ns      │ 247.8 ns      │ 25448   │ 407168
│  ╰─ t=16                       19.12 ns      │ 1.163 µs      │ 105.5 ns      │ 205 ns        │ 21088   │ 337408
├─ rwlock_read_contended                       │               │               │               │         │
│  ├─ t=1                        4.208 ns      │ 68.66 ns      │ 14.11 ns      │ 14.23 ns      │ 24243   │ 6206208
│  ├─ t=2                        6.482 ns      │ 213.9 ns      │ 60.63 ns      │ 61.8 ns       │ 22846   │ 1462144
│  ├─ t=4                        5.998 ns      │ 2.334 µs      │ 275.2 ns      │ 322.3 ns      │ 34908   │ 279264
│  ├─ t=8                        5.373 ns      │ 1.008 µs      │ 318.4 ns      │ 298.9 ns      │ 25536   │ 408576
│  ╰─ t=16                       5.06 ns       │ 39.34 µs      │ 423.9 ns      │ 458.2 ns      │ 15472   │ 495104
├─ rwlock_read_contended_clone                 │               │               │               │         │
│  ├─ t=1                        7.873 ns      │ 238.6 ns      │ 49.84 ns      │ 51.23 ns      │ 56123   │ 1795936
│  ├─ t=2                        12.87 ns      │ 635.3 ns      │ 132.4 ns      │ 138.2 ns      │ 29038   │ 464608
│  ├─ t=4                        17.49 ns      │ 2.266 µs      │ 352.9 ns      │ 398.9 ns      │ 37140   │ 148560
│  ├─ t=8                        12.24 ns      │ 3.218 µs      │ 315.2 ns      │ 393.5 ns      │ 29408   │ 117632
│  ╰─ t=16                       9.748 ns      │ 2.185 µs      │ 106.2 ns      │ 253.4 ns      │ 19776   │ 158208
├─ rwlock_read_spin                            │               │               │               │         │
│  ├─ t=1                        17.9 ns       │ 64.71 ns      │ 17.98 ns      │ 17.96 ns      │ 38618   │ 4943104
│  ├─ t=2                        18.06 ns      │ 109.1 ns      │ 54.06 ns      │ 53.69 ns      │ 25376   │ 1624064
│  ├─ t=4                        18.56 ns      │ 448.6 ns      │ 140.6 ns      │ 140 ns        │ 37016   │ 592256
│  ├─ t=8                        18.21 ns      │ 477.1 ns      │ 295.9 ns      │ 274.7 ns      │ 23280   │ 744960
│  ╰─ t=16                       18.49 ns      │ 834.4 ns      │ 21.68 ns      │ 69.52 ns      │ 21472   │ 343552
├─ rwlock_write                                │               │               │               │         │
│  ├─ 0                          7.533 ns      │ 56.96 ns      │ 7.732 ns      │ 7.731 ns      │ 41713   │ 10678528
│  ├─ 1                          7.533 ns      │ 27.29 ns      │ 7.732 ns      │ 7.734 ns      │ 41698   │ 10674688
│  ├─ 2                          7.572 ns      │ 28.55 ns      │ 7.732 ns      │ 7.729 ns      │ 41727   │ 10682112
│  ├─ 4                          7.533 ns      │ 27.57 ns      │ 7.732 ns      │ 7.738 ns      │ 41692   │ 10673152
│  ├─ 8                          7.572 ns      │ 27.81 ns      │ 7.732 ns      │ 7.725 ns      │ 41734   │ 10683904
│  ╰─ 16                         7.533 ns      │ 47.45 ns      │ 7.728 ns      │ 7.726 ns      │ 41738   │ 10684928
├─ rwlock_write_contended                      │               │               │               │         │
│  ├─ 0                          7.533 ns      │ 24.87 ns      │ 7.732 ns      │ 7.741 ns      │ 41778   │ 10695168
│  ├─ 1                          29.74 ns      │ 23.72 µs      │ 1.942 µs      │ 2.412 µs      │ 39824   │ 39824
│  ├─ 2                          19.74 ns      │ 13.92 µs      │ 2.263 µs      │ 2.578 µs      │ 37314   │ 37314
│  ├─ 4                          29.74 ns      │ 23.38 µs      │ 4.978 µs      │ 4.919 µs      │ 19877   │ 19877
│  ├─ 8                          29.74 ns      │ 22.58 µs      │ 6.201 µs      │ 6.527 µs      │ 15032   │ 15032
│  ╰─ 16                         29.74 ns      │ 8.842 ms      │ 10.29 µs      │ 16.36 µs      │ 6046    │ 6046
╰─ rwlock_write_contended_clone                │               │               │               │         │
   ├─ 0                          7.533 ns      │ 30.5 ns       │ 7.732 ns      │ 7.729 ns      │ 40756   │ 10433536
   ├─ 1                          7.888 ns      │ 250.3 ns      │ 49.37 ns      │ 49.75 ns      │ 29628   │ 1896192
   ├─ 2                          8.498 ns      │ 1.188 µs      │ 233.9 ns      │ 236.2 ns      │ 50149   │ 401192
   ├─ 4                          14.74 ns      │ 10.68 µs      │ 1.442 µs      │ 1.709 µs      │ 28370   │ 56740
   ├─ 8                          29.74 ns      │ 22.51 µs      │ 5.749 µs      │ 5.345 µs      │ 18332   │ 18332
   ╰─ 16                         29.74 ns      │ 11.2 ms       │ 10.01 µs      │ 13.98 µs      │ 7056    │ 7056
   ```

## Analysis

### `RwLock`

The characteristics of `RwLock` are completely different from those of `AtomicArc`/`ArcSwap`; it is notably blocking and suffers from contention even in read-only workloads. Comparing it is thus somewhat biased, nevertheless, it gives a sort of baseline, as it is still a standard and commonly used primitive for the use cases where `AtomicArc` shines. 

There are essentially two ways to use `RwLock`: keeping the lock for the entire read duration, or cloning the content (e.g. an `Arc`) and releasing it as soon as possible. The former is more efficient from a read perspective, but it can starve writes, and thus be not suitable for some use cases. 

Overall, `AtomicArc::load` is 7x faster than `RwLock::read` on Apple M3, without even taking contention into account.

### aarch64

On ARM, `AtomicArc::load` is notably faster than `ArcSwap::load`. A few reasons explain this difference: `AtomicArc` uses a `store` instead of a `swap` in critical path, its thread-local storage is more efficient, and its critical path code is fully inlined.

`AtomicArc::store` is also significantly faster than `ArcSwap::store`, as it relies on `load` + `compare_exchange` instead of a single `compare_exchange`, which is a lot slower when it fails. Its algorithm is also wait-free, but it's hard to measure the impact of this part alone.

Due to the significant differences in the `store` algorithm and performance, `_contended` benchmarks results are hard to compare.

### x86_64

Results on x86_64 are a bit surprising, as despite all the reasons given in the previous section, `AtomicArc::load` seems slightly slower than `ArcSwap::load`. This is difficult to explain[^1], as both use the exact same atomic operations (seqcst `store` is indeed compiled as a `swap` on x86_64), but `AtomicArc::load` code has fewer instructions and is inlined. However, `load_spin` benchmark, which inserts a `std::hint::spin_loop()` call before dropping the load guard, gives consistently better results for `AtomicArc` than `ArcSwap`.

Atomic RMW operations are very costly on x86_64, at the point that they seem to take the whole benchmark time for tiny operations like `AtomicArc::load`. However, in more realistic situations that `load_spin` benchmark tries to simulate, `AtomicArc` optimizations start to pay off. Atomic RMW cost is also visible in `load_no_slot` benchmark, i.e., measuring the fallback algorithm when no hazard pointer slot is available. While `AtomicArc` and `ArcSwap` give similar result on ARM, the higher number of RMW operations in `ArcSwap` fallback algorithm is visible on x86_64. 

[^1]: Actually, I don't even understand it, so any explanation is welcome.