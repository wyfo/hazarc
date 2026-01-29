# Benchmark

The following benchmark compares `hazarc` with `arc-swap`, but also with standard `RwLock<Arc<T>>`. Refer to the [code](https://github.com/wyfo/hazarc/blob/main/benches/comparison.rs) for details of the benched functions. Guards (`RwLockReadGuard`/`ArcBorrow`/etc.) destructors are included in the measurement.

## Results

### Apple M3 (aarch64)

```
Timer precision: 41 ns
comparison                       fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ arcswap_load                  1.937 ns      │ 30.42 ns      │ 2.12 ns       │ 2.11 ns       │ 9027    │ 18487296
├─ arcswap_load_contended        1.978 ns      │ 25.86 ns      │ 6.128 ns      │ 6.11 ns       │ 13022   │ 13334528
├─ arcswap_load_no_slot          5.111 ns      │ 40.55 ns      │ 5.681 ns      │ 5.838 ns      │ 13458   │ 13780992
├─ arcswap_load_no_slot_spin     16.3 ns       │ 44.45 ns      │ 16.62 ns      │ 16.69 ns      │ 21493   │ 5502208
├─ arcswap_load_none             1.897 ns      │ 7.98 ns       │ 2.08 ns       │ 2.093 ns      │ 13984   │ 28639232
├─ arcswap_load_spin             10.93 ns      │ 102.8 ns      │ 11.58 ns      │ 11.52 ns      │ 15119   │ 7740928
├─ arcswap_store                               │               │               │               │         │
│  ├─ 0                          58.94 ns      │ 288.1 ns      │ 59.59 ns      │ 66.82 ns      │ 11418   │ 1461504
│  ├─ 1                          58.94 ns      │ 291 ns        │ 59.59 ns      │ 59.92 ns      │ 12703   │ 1625984
│  ├─ 2                          75.21 ns      │ 217.1 ns      │ 77.16 ns      │ 77.23 ns      │ 19757   │ 1264448
│  ├─ 4                          116.8 ns      │ 349.9 ns      │ 119.4 ns      │ 119.4 ns      │ 25647   │ 820704
│  ├─ 8                          200.2 ns      │ 1.935 µs      │ 245.7 ns      │ 247.3 ns      │ 12513   │ 400416
│  ╰─ 16                         369.4 ns      │ 3.142 µs      │ 416.3 ns      │ 409 ns        │ 15153   │ 242448
├─ arcswap_store_contended                     │               │               │               │         │
│  ├─ 0                          369.4 ns      │ 932 ns        │ 374.7 ns      │ 375.1 ns      │ 16512   │ 264192
│  ├─ 1                          392.9 ns      │ 958 ns        │ 452.8 ns      │ 454 ns        │ 13660   │ 218560
│  ├─ 2                          494.4 ns      │ 1.489 µs      │ 562.2 ns      │ 563 ns        │ 21968   │ 175744
│  ├─ 4                          416.2 ns      │ 573.4 µs      │ 1.062 µs      │ 1.208 µs      │ 40517   │ 81034
│  ├─ 8                          604.2 ns      │ 4.632 ms      │ 1.957 µs      │ 2.494 µs      │ 19756   │ 39512
│  ╰─ 16                         583.2 ns      │ 6.841 ms      │ 1.916 µs      │ 4.523 µs      │ 11859   │ 23718
├─ hazarc_load                   0.681 ns      │ 1.592 ns      │ 0.762 ns      │ 0.763 ns      │ 5744    │ 47054848
├─ hazarc_load_contended         0.676 ns      │ 63.25 ns      │ 4.094 ns      │ 4.204 ns      │ 34827   │ 17831424
├─ hazarc_load_no_slot           5.315 ns      │ 32.33 ns      │ 5.763 ns      │ 5.722 ns      │ 13701   │ 14029824
├─ hazarc_load_no_slot_spin      14.59 ns      │ 28.99 ns      │ 14.75 ns      │ 14.85 ns      │ 12023   │ 6155776
├─ hazarc_load_none              0.19 ns       │ 0.663 ns      │ 0.193 ns      │ 0.195 ns      │ 4099    │ 67158016
├─ hazarc_load_pthread           2.832 ns      │ 16.22 ns      │ 2.873 ns      │ 3.292 ns      │ 20673   │ 21169152
├─ hazarc_load_pthread_unsafe    1.012 ns      │ 8.051 ns      │ 1.032 ns      │ 1.034 ns      │ 10110   │ 41410560
├─ hazarc_load_spin              9.221 ns      │ 56.82 ns      │ 10.36 ns      │ 10.38 ns      │ 16592   │ 8495104
├─ hazarc_store                                │               │               │               │         │
│  ├─ 0                          6.739 ns      │ 15.12 ns      │ 7.309 ns      │ 7.201 ns      │ 11338   │ 11610112
│  ├─ 1                          9.301 ns      │ 26.55 ns      │ 10.11 ns      │ 10.14 ns      │ 16908   │ 8656896
│  ├─ 2                          16.46 ns      │ 81.73 ns      │ 16.95 ns      │ 17.04 ns      │ 21077   │ 5395712
│  ├─ 4                          21.67 ns      │ 64.8 ns       │ 21.99 ns      │ 22.13 ns      │ 16536   │ 4233216
│  ├─ 8                          35.5 ns       │ 235 ns        │ 35.83 ns      │ 36.08 ns      │ 20686   │ 2647808
│  ╰─ 16                         66.1 ns       │ 228.8 ns      │ 66.76 ns      │ 67.5 ns       │ 22459   │ 1437376
├─ hazarc_store_contended                      │               │               │               │         │
│  ├─ 0                          66.1 ns       │ 181.9 ns      │ 66.76 ns      │ 67.2 ns       │ 22546   │ 1442944
│  ├─ 1                          67.41 ns      │ 478.8 ns      │ 153.3 ns      │ 154.2 ns      │ 19948   │ 638336
│  ├─ 2                          75.2 ns       │ 1.068 µs      │ 219.7 ns      │ 219.9 ns      │ 14033   │ 449056
│  ├─ 4                          114.2 ns      │ 7.708 µs      │ 655.9 ns      │ 755.2 ns      │ 32437   │ 129748
│  ├─ 8                          489.2 ns      │ 2.487 ms      │ 1.395 µs      │ 1.617 µs      │ 15275   │ 61100
│  ╰─ 16                         239.2 ns      │ 4.313 ms      │ 1.457 µs      │ 3.406 µs      │ 7545    │ 30180
├─ rwlock_read                                 │               │               │               │         │
│  ├─ t=1                        4.297 ns      │ 11.25 ns      │ 4.378 ns      │ 4.38 ns       │ 17060   │ 17469440
│  ├─ t=2                        4.5 ns        │ 181.3 ns      │ 19.63 ns      │ 19.59 ns      │ 10440   │ 5345280
│  ├─ t=4                        4.262 ns      │ 305.6 ns      │ 6.856 ns      │ 26.89 ns      │ 19308   │ 1235712
│  ├─ t=8                        4.246 ns      │ 1.987 µs      │ 94.74 ns      │ 172.4 ns      │ 11696   │ 748544
│  ╰─ t=16                       4.825 ns      │ 3.395 µs      │ 15.32 ns      │ 57.43 ns      │ 17456   │ 139648
├─ rwlock_read_clone                           │               │               │               │         │
│  ├─ t=1                        6.861 ns      │ 14.59 ns      │ 6.943 ns      │ 6.966 ns      │ 11749   │ 12030976
│  ├─ t=2                        7.024 ns      │ 179.8 ns      │ 68.54 ns      │ 63.4 ns       │ 8136    │ 2082816
│  ├─ t=4                        6.2 ns        │ 646.8 ns      │ 10.13 ns      │ 72.25 ns      │ 18680   │ 597760
│  ├─ t=8                        20.2 ns       │ 6.895 µs      │ 41.2 ns       │ 86.64 ns      │ 15168   │ 30336
│  ╰─ t=16                       4.825 ns      │ 4.885 µs      │ 15.32 ns      │ 267.2 ns      │ 17952   │ 143616
├─ rwlock_read_clone_spin                      │               │               │               │         │
│  ├─ t=1                        12.15 ns      │ 26.55 ns      │ 12.31 ns      │ 12.4 ns       │ 14150   │ 7244800
│  ├─ t=2                        12.55 ns      │ 171.2 ns      │ 108.7 ns      │ 106.4 ns      │ 5632    │ 1441792
│  ├─ t=4                        14.01 ns      │ 642.9 ns      │ 118.2 ns      │ 155.6 ns      │ 16020   │ 512640
│  ├─ t=8                        12.76 ns      │ 2.387 µs      │ 208 ns        │ 298.5 ns      │ 13456   │ 215296
│  ╰─ t=16                       10.2 ns       │ 5.645 µs      │ 67.45 ns      │ 446.8 ns      │ 16128   │ 129024
├─ rwlock_read_contended                       │               │               │               │         │
│  ├─ t=1                        4.5 ns        │ 47.26 ns      │ 6.331 ns      │ 6.323 ns      │ 12694   │ 12998656
│  ├─ t=2                        4.826 ns      │ 88.4 ns       │ 23.86 ns      │ 26.54 ns      │ 8284    │ 4241408
│  ├─ t=4                        4.582 ns      │ 489.2 ns      │ 83.03 ns      │ 97.91 ns      │ 9504    │ 1216512
│  ├─ t=8                        4.887 ns      │ 1.825 µs      │ 45.26 ns      │ 124.2 ns      │ 14392   │ 460544
│  ╰─ t=16                       2.325 ns      │ 3.874 µs      │ 33.57 ns      │ 269.1 ns      │ 14592   │ 233472
├─ rwlock_read_contended_clone                 │               │               │               │         │
│  ├─ t=1                        7.104 ns      │ 64.8 ns       │ 11.9 ns       │ 12.94 ns      │ 13618   │ 6972416
│  ├─ t=2                        7.832 ns      │ 245.1 ns      │ 18.57 ns      │ 33.37 ns      │ 15302   │ 1958656
│  ├─ t=4                        7.512 ns      │ 1.339 µs      │ 165 ns        │ 215.3 ns      │ 11796   │ 377472
│  ├─ t=8                        7.512 ns      │ 4.27 µs       │ 234 ns        │ 366.6 ns      │ 13496   │ 215936
│  ╰─ t=16                       4.825 ns      │ 6.708 µs      │ 181.9 ns      │ 601.1 ns      │ 15360   │ 122880
├─ rwlock_read_spin                            │               │               │               │         │
│  ├─ t=1                        8.489 ns      │ 28.83 ns      │ 8.651 ns      │ 8.662 ns      │ 18966   │ 9710592
│  ├─ t=2                        10.11 ns      │ 114.5 ns      │ 63.17 ns      │ 58.24 ns      │ 5002    │ 2561024
│  ├─ t=4                        10.76 ns      │ 362.3 ns      │ 98.98 ns      │ 100 ns        │ 14868   │ 951552
│  ├─ t=8                        10.1 ns       │ 1.987 µs      │ 159.8 ns      │ 218.4 ns      │ 12536   │ 401152
│  ╰─ t=16                       10.1 ns       │ 5.412 µs      │ 405.9 ns      │ 701 ns        │ 12320   │ 394240
├─ rwlock_write                                │               │               │               │         │
│  ├─ 0                          4.663 ns      │ 58.57 ns      │ 5.112 ns      │ 5.106 ns      │ 14987   │ 15346688
│  ├─ 1                          4.663 ns      │ 49.78 ns      │ 5.071 ns      │ 5.042 ns      │ 15166   │ 15529984
│  ├─ 2                          4.663 ns      │ 13.65 ns      │ 5.071 ns      │ 5.014 ns      │ 15246   │ 15611904
│  ├─ 4                          4.663 ns      │ 13.77 ns      │ 5.112 ns      │ 5.081 ns      │ 15058   │ 15419392
│  ├─ 8                          4.663 ns      │ 16.46 ns      │ 5.112 ns      │ 5.06 ns       │ 15118   │ 15480832
│  ╰─ 16                         4.663 ns      │ 13.08 ns      │ 4.826 ns      │ 4.932 ns      │ 15481   │ 15852544
├─ rwlock_write_contended                      │               │               │               │         │
│  ├─ 0                          4.663 ns      │ 30.7 ns       │ 5.071 ns      │ 4.985 ns      │ 15328   │ 15695872
│  ├─ 1                          40.7 ns       │ 200 ms        │ 40.7 ns       │ 4.231 ms      │ 100     │ 100
│  ├─ 2                          40.7 ns       │ 221.2 µs      │ 40.7 ns       │ 347.5 ns      │ 260347  │ 260347
│  ├─ 4                          40.7 ns       │ 10.03 ms      │ 40.7 ns       │ 25.82 µs      │ 3867    │ 3867
│  ├─ 8                          40.7 ns       │ 18.38 ms      │ 40.7 ns       │ 264.5 µs      │ 378     │ 378
│  ╰─ 16                         40.7 ns       │ 20.26 ms      │ 40.7 ns       │ 81.93 µs      │ 1344    │ 1344
╰─ rwlock_write_contended_clone                │               │               │               │         │
   ├─ 0                          5.233 ns      │ 30.17 ns      │ 5.518 ns      │ 5.572 ns      │ 14063   │ 14400512
   ├─ 1                          5.397 ns      │ 140.4 ns      │ 8.815 ns      │ 10.59 ns      │ 16269   │ 8329728
   ├─ 2                          5.883 ns      │ 465.3 ns      │ 21.02 ns      │ 31.04 ns      │ 11981   │ 3067136
   ├─ 4                          40.7 ns       │ 143.6 µs      │ 124.7 ns      │ 1.043 µs      │ 91835   │ 91835
   ├─ 8                          40.7 ns       │ 10.09 ms      │ 124.7 ns      │ 24.27 µs      │ 4489    │ 4489
   ╰─ 16                         40.7 ns       │ 22.43 ms      │ 165.7 ns      │ 315.5 µs      │ 327     │ 327
```

### AMD Ryzen 7 5800X (x86_64)

```
Timer precision: 20 ns
comparison                       fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ arcswap_load                  3.682 ns      │ 19.96 ns      │ 3.78 ns       │ 3.792 ns      │ 35231   │ 18038272
├─ arcswap_load_contended        3.78 ns       │ 37.16 ns      │ 18.02 ns      │ 18.02 ns      │ 19888   │ 5091328
├─ arcswap_load_no_slot          13.56 ns      │ 30.54 ns      │ 13.76 ns      │ 13.78 ns      │ 25388   │ 6499328
├─ arcswap_load_no_slot_spin     24.63 ns      │ 68.24 ns      │ 24.71 ns      │ 24.75 ns      │ 29256   │ 3744768
├─ arcswap_load_none             3.719 ns      │ 15.14 ns      │ 3.817 ns      │ 3.817 ns      │ 35571   │ 18212352
├─ arcswap_load_spin             19.15 ns      │ 61.42 ns      │ 19.23 ns      │ 19.21 ns      │ 36905   │ 4723840
├─ arcswap_store                               │               │               │               │         │
│  ├─ 0                          58.76 ns      │ 123.8 ns      │ 59.7 ns       │ 59.67 ns      │ 24959   │ 1597376
│  ├─ 1                          58.6 ns       │ 123.1 ns      │ 59.56 ns      │ 59.66 ns      │ 24962   │ 1597568
│  ├─ 2                          79.9 ns       │ 204.4 ns      │ 81.78 ns      │ 81.91 ns      │ 36223   │ 1159136
│  ├─ 4                          123 ns        │ 348.4 ns      │ 126.2 ns      │ 126.3 ns      │ 46903   │ 750448
│  ├─ 8                          226.3 ns      │ 687.2 ns      │ 241.4 ns      │ 243.5 ns      │ 48853   │ 390824
│  ╰─ 16                         434.3 ns      │ 1.188 µs      │ 459.3 ns      │ 460.9 ns      │ 26410   │ 211280
├─ arcswap_store_contended                     │               │               │               │         │
│  ├─ 0                          435.6 ns      │ 903.8 ns      │ 459.3 ns      │ 461 ns        │ 26407   │ 211256
│  ├─ 1                          445.7 ns      │ 2.291 µs      │ 523.2 ns      │ 527.1 ns      │ 45396   │ 181584
│  ├─ 2                          468.2 ns      │ 1.998 µs      │ 560.7 ns      │ 565.1 ns      │ 42486   │ 169944
│  ├─ 4                          518.2 ns      │ 1.582 µs      │ 655.9 ns      │ 659.9 ns      │ 36586   │ 146344
│  ├─ 8                          645.9 ns      │ 2.306 µs      │ 841.2 ns      │ 848.5 ns      │ 28668   │ 114672
│  ╰─ 16                         926.2 ns      │ 3.366 µs      │ 1.221 µs      │ 1.233 µs      │ 38845   │ 77690
├─ hazarc_load                   3.582 ns      │ 17.32 ns      │ 4.133 ns      │ 4.146 ns      │ 34917   │ 17877504
├─ hazarc_load_contended         4.053 ns      │ 52.66 ns      │ 25.89 ns      │ 25.82 ns      │ 28115   │ 3598720
├─ hazarc_load_no_slot           9.569 ns      │ 32.5 ns       │ 9.764 ns      │ 9.769 ns      │ 34344   │ 8792064
├─ hazarc_load_no_slot_spin      23.61 ns      │ 65.42 ns      │ 23.7 ns       │ 23.73 ns      │ 30412   │ 3892736
├─ hazarc_load_none              0.252 ns      │ 1.681 ns      │ 0.274 ns      │ 0.278 ns      │ 15668   │ 64176128
├─ hazarc_load_pthread           5.266 ns      │ 30.39 ns      │ 5.286 ns      │ 5.298 ns      │ 27954   │ 14312448
├─ hazarc_load_pthread_unsafe    3.838 ns      │ 14.05 ns      │ 3.995 ns      │ 4.001 ns      │ 34347   │ 17585664
├─ hazarc_load_spin              18.13 ns      │ 49.99 ns      │ 18.21 ns      │ 18.21 ns      │ 38673   │ 4950144
├─ hazarc_store                                │               │               │               │         │
│  ├─ 0                          9.764 ns      │ 32.93 ns      │ 9.924 ns      │ 9.947 ns      │ 33112   │ 8476672
│  ├─ 1                          9.374 ns      │ 30.39 ns      │ 9.69 ns       │ 10.36 ns      │ 31995   │ 8190720
│  ├─ 2                          12.81 ns      │ 35.08 ns      │ 13.13 ns      │ 14.07 ns      │ 24535   │ 6280960
│  ├─ 4                          19.7 ns       │ 57.2 ns       │ 20.25 ns      │ 21.54 ns      │ 32914   │ 4212992
│  ├─ 8                          42.63 ns      │ 114.6 ns      │ 42.79 ns      │ 45.47 ns      │ 32310   │ 2067840
│  ╰─ 16                         71.43 ns      │ 317.5 ns      │ 73.03 ns      │ 75.84 ns      │ 39010   │ 1248320
├─ hazarc_store_contended                      │               │               │               │         │
│  ├─ 0                          71.74 ns      │ 470.3 ns      │ 74.87 ns      │ 79.4 ns       │ 37359   │ 1195488
│  ├─ 1                          73.62 ns      │ 383.6 ns      │ 163.8 ns      │ 165.3 ns      │ 36307   │ 580912
│  ├─ 2                          75.49 ns      │ 610.3 ns      │ 214.4 ns      │ 214.3 ns      │ 28256   │ 452096
│  ├─ 4                          76.12 ns      │ 1.042 µs      │ 322.7 ns      │ 323.6 ns      │ 37223   │ 297784
│  ├─ 8                          252.7 ns      │ 1.454 µs      │ 548.2 ns      │ 550.8 ns      │ 43127   │ 172508
│  ╰─ 16                         460.7 ns      │ 2.865 µs      │ 881.2 ns      │ 889.2 ns      │ 52942   │ 105884
├─ rwlock_read                                 │               │               │               │         │
│  ├─ t=1                        4.229 ns      │ 20.09 ns      │ 4.307 ns      │ 4.316 ns      │ 32565   │ 16673280
│  ├─ t=2                        4.678 ns      │ 76.92 ns      │ 35.99 ns      │ 35.89 ns      │ 21932   │ 2807296
│  ├─ t=4                        5.374 ns      │ 409.9 ns      │ 133.1 ns      │ 131.7 ns      │ 37844   │ 605504
│  ├─ t=8                        4.749 ns      │ 457.1 ns      │ 222 ns        │ 189.1 ns      │ 25056   │ 801792
│  ╰─ t=16                       4.749 ns      │ 1.097 µs      │ 173 ns        │ 252.5 ns      │ 19216   │ 614912
├─ rwlock_read_clone                           │               │               │               │         │
│  ├─ t=1                        7.616 ns      │ 33.6 ns       │ 7.811 ns      │ 7.826 ns      │ 41217   │ 10551552
│  ├─ t=2                        8.514 ns      │ 145.8 ns      │ 57.51 ns      │ 57.32 ns      │ 24954   │ 1597056
│  ├─ t=4                        10.37 ns      │ 478.1 ns      │ 173.8 ns      │ 178.1 ns      │ 34272   │ 548352
│  ├─ t=8                        9.124 ns      │ 603.9 ns      │ 223.2 ns      │ 192.8 ns      │ 26888   │ 430208
│  ╰─ t=16                       9.124 ns      │ 982.2 ns      │ 16.62 ns      │ 139.5 ns      │ 21648   │ 346368
├─ rwlock_read_clone_spin                      │               │               │               │         │
│  ├─ t=1                        18.6 ns       │ 57.43 ns      │ 18.69 ns      │ 18.71 ns      │ 37722   │ 4828416
│  ├─ t=2                        18.84 ns      │ 178.9 ns      │ 79.12 ns      │ 79.66 ns      │ 20900   │ 1337600
│  ├─ t=4                        19.12 ns      │ 441.8 ns      │ 173.1 ns      │ 177.1 ns      │ 35672   │ 570752
│  ├─ t=8                        19.74 ns      │ 658.4 ns      │ 160.1 ns      │ 170 ns        │ 29248   │ 233984
│  ╰─ t=16                       19.12 ns      │ 1.141 µs      │ 26.06 ns      │ 159.2 ns      │ 21024   │ 336384
├─ rwlock_read_contended                       │               │               │               │         │
│  ├─ t=1                        4.366 ns      │ 35.63 ns      │ 12.43 ns      │ 12.51 ns      │ 27169   │ 6955264
│  ├─ t=2                        5.077 ns      │ 222.2 ns      │ 58.13 ns      │ 61.28 ns      │ 23274   │ 1489536
│  ├─ t=4                        9.749 ns      │ 1.019 µs      │ 185.1 ns      │ 199.3 ns      │ 38628   │ 309024
│  ├─ t=8                        5.999 ns      │ 1.341 µs      │ 88.62 ns      │ 202.9 ns      │ 30320   │ 242560
│  ╰─ t=16                       4.905 ns      │ 1.58 µs       │ 726.5 ns      │ 661.1 ns      │ 12128   │ 776192
├─ rwlock_read_contended_clone                 │               │               │               │         │
│  ├─ t=1                        7.733 ns      │ 147 ns        │ 49.84 ns      │ 50.47 ns      │ 29352   │ 1878528
│  ├─ t=2                        13.49 ns      │ 580.8 ns      │ 118.1 ns      │ 133.4 ns      │ 30178   │ 482848
│  ├─ t=4                        17.49 ns      │ 2.173 µs      │ 255.2 ns      │ 343.7 ns      │ 38752   │ 155008
│  ├─ t=8                        12.24 ns      │ 1.906 µs      │ 500.6 ns      │ 493.5 ns      │ 26768   │ 214144
│  ╰─ t=16                       9.749 ns      │ 2.03 µs       │ 87.37 ns      │ 220.4 ns      │ 20272   │ 162176
├─ rwlock_read_spin                            │               │               │               │         │
│  ├─ t=1                        17.9 ns       │ 62.28 ns      │ 17.98 ns      │ 17.98 ns      │ 38621   │ 4943488
│  ├─ t=2                        18.06 ns      │ 89.28 ns      │ 57.2 ns       │ 57.4 ns       │ 24702   │ 1580928
│  ├─ t=4                        18.49 ns      │ 433.7 ns      │ 167.5 ns      │ 165.7 ns      │ 35748   │ 571968
│  ├─ t=8                        18.49 ns      │ 714.2 ns      │ 237.6 ns      │ 205.8 ns      │ 28528   │ 456448
│  ╰─ t=16                       18.49 ns      │ 923.9 ns      │ 21.06 ns      │ 92.23 ns      │ 21280   │ 340480
├─ rwlock_write                                │               │               │               │         │
│  ├─ 0                          7.534 ns      │ 30.39 ns      │ 7.694 ns      │ 7.713 ns      │ 41888   │ 10723328
│  ├─ 1                          7.534 ns      │ 57.51 ns      │ 7.694 ns      │ 7.715 ns      │ 41796   │ 10699776
│  ├─ 2                          7.534 ns      │ 32.38 ns      │ 7.694 ns      │ 7.719 ns      │ 41855   │ 10714880
│  ├─ 4                          7.538 ns      │ 26.44 ns      │ 7.694 ns      │ 7.708 ns      │ 41912   │ 10729472
│  ├─ 8                          7.534 ns      │ 54.96 ns      │ 7.729 ns      │ 7.729 ns      │ 41794   │ 10699264
│  ╰─ 16                         7.534 ns      │ 147.3 ns      │ 7.694 ns      │ 7.719 ns      │ 41824   │ 10706944
├─ rwlock_write_contended                      │               │               │               │         │
│  ├─ 0                          7.538 ns      │ 24.48 ns      │ 7.729 ns      │ 7.726 ns      │ 41821   │ 10706176
│  ├─ 1                          12.24 ns      │ 216.4 µs      │ 2.281 µs      │ 2.519 µs      │ 9812    │ 39248
│  ├─ 2                          19.74 ns      │ 25.26 µs      │ 2.944 µs      │ 2.96 µs       │ 32644   │ 32644
│  ├─ 4                          29.74 ns      │ 20.01 µs      │ 4.988 µs      │ 5.116 µs      │ 19123   │ 19123
│  ├─ 8                          29.74 ns      │ 21.48 µs      │ 5.73 µs       │ 5.874 µs      │ 16696   │ 16696
│  ╰─ 16                         29.74 ns      │ 9.487 ms      │ 10.32 µs      │ 14.98 µs      │ 6945    │ 6945
╰─ rwlock_write_contended_clone                │               │               │               │         │
   ├─ 0                          7.573 ns      │ 252 ns        │ 7.729 ns      │ 7.733 ns      │ 40735   │ 10428160
   ├─ 1                          7.889 ns      │ 204.8 ns      │ 36.07 ns      │ 36.4 ns       │ 39709   │ 2541376
   ├─ 2                          8.499 ns      │ 594.6 ns      │ 161.9 ns      │ 166.1 ns      │ 36030   │ 576480
   ├─ 4                          29.74 ns      │ 22.17 µs      │ 1.111 µs      │ 1.827 µs      │ 51880   │ 51880
   ├─ 8                          29.74 ns      │ 29.58 µs      │ 9.326 µs      │ 8.871 µs      │ 11072   │ 11072
   ╰─ 16                         29.74 ns      │ 11.23 ms      │ 10.34 µs      │ 14.96 µs      │ 6607    │ 6607
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