# Benchmark

The following benchmark compares `hazarc` with `arc-swap`, but also with standard `RwLock<Arc<T>>`. Refer to the [code](https://github.com/wyfo/hazarc/blob/main/benches/comparison.rs) for details of the benched functions. Guards (`RwLockReadGuard`/`ArcBorrow`/etc.) destructors are included in the measure.

## Results

### Apple M3 (aarch64)

```
Timer precision: 41 ns
comparison                       fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ arcswap_load                  1.906 ns      │ 15.88 ns      │ 1.926 ns      │ 1.952 ns      │ 9777    │ 20023296
├─ arcswap_load_contended        1.906 ns      │ 24.28 ns      │ 5.73 ns       │ 5.727 ns      │ 27395   │ 14026240
├─ arcswap_load_no_slot          5.121 ns      │ 63.3 ns       │ 5.608 ns      │ 5.693 ns      │ 13805   │ 14136320
├─ arcswap_load_no_slot_spin     15.16 ns      │ 188.1 ns      │ 15.98 ns      │ 16.37 ns      │ 21854   │ 5594624
├─ arcswap_load_none             1.906 ns      │ 5.446 ns      │ 1.967 ns      │ 2.028 ns      │ 14430   │ 29552640
├─ arcswap_load_spin             10.93 ns      │ 52.52 ns      │ 11.1 ns       │ 11.33 ns      │ 15349   │ 7858688
├─ arcswap_store                               │               │               │               │         │
│  ├─ 0                          58.3 ns       │ 930 ns        │ 60.25 ns      │ 69.13 ns      │ 22006   │ 1408384
│  ├─ 1                          58.94 ns      │ 512 ns        │ 81.74 ns      │ 74.55 ns      │ 20446   │ 1308544
│  ├─ 2                          73.92 ns      │ 1.094 µs      │ 77.83 ns      │ 89.96 ns      │ 33817   │ 1082144
│  ├─ 4                          75.22 ns      │ 663.7 ns      │ 77.17 ns      │ 84.76 ns      │ 18035   │ 1154240
│  ├─ 8                          75.22 ns      │ 1.023 µs      │ 98.67 ns      │ 102 ns        │ 15041   │ 962624
│  ╰─ 16                         75.22 ns      │ 604.5 ns      │ 86.3 ns       │ 94.42 ns      │ 16225   │ 1038400
├─ arcswap_store_contended                     │               │               │               │         │
│  ├─ 0                          75.22 ns      │ 425.4 ns      │ 77.19 ns      │ 87.95 ns      │ 17399   │ 1113536
│  ├─ 1                          77.83 ns      │ 1.731 µs      │ 182 ns        │ 181.5 ns      │ 16980   │ 543360
│  ├─ 2                          88.27 ns      │ 5.223 µs      │ 260.1 ns      │ 262.4 ns      │ 23480   │ 375680
│  ├─ 4                          207.9 ns      │ 6.447 µs      │ 718.4 ns      │ 807.7 ns      │ 30469   │ 121876
│  ├─ 8                          457.7 ns      │ 1.555 ms      │ 1.916 µs      │ 2.064 µs      │ 23760   │ 47520
│  ╰─ 16                         624.7 ns      │ 13.01 ms      │ 1.895 µs      │ 7.581 µs      │ 6565    │ 13130
├─ hazarc_load                   0.685 ns      │ 18.3 ns       │ 0.706 ns      │ 0.733 ns      │ 11886   │ 48685056
├─ hazarc_load_contended         0.685 ns      │ 95.16 ns      │ 3.697 ns      │ 3.856 ns      │ 37193   │ 19042816
├─ hazarc_load_no_slot           5.324 ns      │ 29.41 ns      │ 5.528 ns      │ 5.63 ns       │ 13912   │ 14245888
├─ hazarc_load_no_slot_spin      14.35 ns      │ 55.7 ns       │ 14.84 ns      │ 14.89 ns      │ 23840   │ 6103040
├─ hazarc_load_none              0.199 ns      │ 0.621 ns      │ 0.202 ns      │ 0.207 ns      │ 4070    │ 66682880
├─ hazarc_load_spin              9.23 ns       │ 95.65 ns      │ 9.474 ns      │ 9.573 ns      │ 17813   │ 9120256
├─ hazarc_store                                │               │               │               │         │
│  ├─ 0                          6.584 ns      │ 33.48 ns      │ 6.708 ns      │ 6.77 ns       │ 12018   │ 12306432
│  ├─ 1                          8.986 ns      │ 84.18 ns      │ 9.23 ns       │ 9.399 ns      │ 18114   │ 9274368
│  ├─ 2                          15.98 ns      │ 114.4 ns      │ 16.31 ns      │ 16.45 ns      │ 21782   │ 5576192
│  ├─ 4                          15.98 ns      │ 115.9 ns      │ 16.31 ns      │ 16.45 ns      │ 21780   │ 5575680
│  ├─ 8                          15.98 ns      │ 162.3 ns      │ 16.31 ns      │ 16.49 ns      │ 21728   │ 5562368
│  ╰─ 16                         15.98 ns      │ 139.8 ns      │ 16.31 ns      │ 16.44 ns      │ 21786   │ 5577216
├─ hazarc_store_contended                      │               │               │               │         │
│  ├─ 0                          15.98 ns      │ 197.9 ns      │ 16.31 ns      │ 16.46 ns      │ 21769   │ 5572864
│  ├─ 1                          15.97 ns      │ 722.3 ns      │ 63.5 ns       │ 64.1 ns       │ 23670   │ 1514880
│  ├─ 2                          17.92 ns      │ 1.217 µs      │ 155.9 ns      │ 156.2 ns      │ 19670   │ 629440
│  ├─ 4                          25.7 ns       │ 3.859 µs      │ 567.4 ns      │ 603.2 ns      │ 20489   │ 163912
│  ├─ 8                          176.7 ns      │ 2.447 ms      │ 1.343 µs      │ 1.725 µs      │ 14332   │ 57328
│  ╰─ 16                         166.4 ns      │ 4.121 ms      │ 1.322 µs      │ 4.134 µs      │ 5690    │ 22760
├─ rwlock_read                                 │               │               │               │         │
│  ├─ t=1                        4.306 ns      │ 62.98 ns      │ 4.388 ns      │ 4.428 ns      │ 16921   │ 17327104
│  ├─ t=2                        4.55 ns       │ 93.7 ns       │ 19.4 ns       │ 21.15 ns      │ 5956    │ 6098944
│  ├─ t=4                        4.255 ns      │ 311.5 ns      │ 6.865 ns      │ 26.58 ns      │ 18992   │ 1215488
│  ├─ t=8                        4.255 ns      │ 2.115 µs      │ 52.44 ns      │ 141.2 ns      │ 11936   │ 763904
│  ╰─ t=16                       3.615 ns      │ 4.236 µs      │ 27.05 ns      │ 313.8 ns      │ 14112   │ 451584
├─ rwlock_read_clone                           │               │               │               │         │
│  ├─ t=1                        6.87 ns       │ 47.68 ns      │ 6.952 ns      │ 7.031 ns      │ 11659   │ 11938816
│  ├─ t=2                        7.033 ns      │ 180.2 ns      │ 71.64 ns      │ 63.78 ns      │ 8106    │ 2075136
│  ├─ t=4                        6.865 ns      │ 807.6 ns      │ 124.3 ns      │ 168.3 ns      │ 11976   │ 766464
│  ├─ t=8                        4.834 ns      │ 2.463 µs      │ 15.33 ns      │ 110.1 ns      │ 14728   │ 117824
│  ╰─ t=16                       9.959 ns      │ 5.28 µs       │ 30.95 ns      │ 168.7 ns      │ 17024   │ 68096
├─ rwlock_read_clone_spin                      │               │               │               │         │
│  ├─ t=1                        12.16 ns      │ 57.57 ns      │ 12.32 ns      │ 12.45 ns      │ 14108   │ 7223296
│  ├─ t=2                        12.72 ns      │ 225.6 ns      │ 34.86 ns      │ 44.86 ns      │ 14550   │ 1862400
│  ├─ t=4                        15.33 ns      │ 986.6 ns      │ 23.14 ns      │ 69.54 ns      │ 20072   │ 321152
│  ├─ t=8                        12.77 ns      │ 2.791 µs      │ 155.9 ns      │ 278.1 ns      │ 13392   │ 214272
│  ╰─ t=16                       15.33 ns      │ 4.78 µs       │ 57.08 ns      │ 394.3 ns      │ 15760   │ 126080
├─ rwlock_read_contended                       │               │               │               │         │
│  ├─ t=1                        4.267 ns      │ 144.5 ns      │ 6.873 ns      │ 8.308 ns      │ 39975   │ 10233600
│  ├─ t=2                        4.916 ns      │ 168.1 ns      │ 20.7 ns       │ 24.12 ns      │ 12750   │ 3264000
│  ├─ t=4                        4.271 ns      │ 653.3 ns      │ 46.58 ns      │ 72.61 ns      │ 12220   │ 782080
│  ├─ t=8                        9.959 ns      │ 5.52 µs       │ 30.95 ns      │ 84.28 ns      │ 16256   │ 65024
│  ╰─ t=16                       4.255 ns      │ 4.38 µs       │ 403.6 ns      │ 902.1 ns      │ 8160    │ 522240
├─ rwlock_read_contended_clone                 │               │               │               │         │
│  ├─ t=1                        7.033 ns      │ 195.4 ns      │ 11.75 ns      │ 12.53 ns      │ 14028   │ 7182336
│  ├─ t=2                        7.841 ns      │ 251.3 ns      │ 17.28 ns      │ 33.69 ns      │ 15264   │ 1953792
│  ├─ t=4                        7.521 ns      │ 1.07 µs       │ 187.8 ns      │ 232.5 ns      │ 9108    │ 582912
│  ├─ t=8                        20.2 ns       │ 10.87 µs      │ 104.2 ns      │ 237.5 ns      │ 16216   │ 32432
│  ╰─ t=16                       4.834 ns      │ 7.051 µs      │ 166.3 ns      │ 585 ns        │ 15264   │ 122112
├─ rwlock_read_spin                            │               │               │               │         │
│  ├─ t=1                        8.498 ns      │ 79.86 ns      │ 8.66 ns       │ 8.781 ns      │ 19202   │ 9831424
│  ├─ t=2                        9.802 ns      │ 198.1 ns      │ 50.32 ns      │ 49.97 ns      │ 9528    │ 2439168
│  ├─ t=4                        10.13 ns      │ 1.084 µs      │ 91.5 ns       │ 96.64 ns      │ 14752   │ 944128
│  ├─ t=8                        10.11 ns      │ 2.045 µs      │ 153.3 ns      │ 208.6 ns      │ 12624   │ 403968
│  ╰─ t=16                       10.08 ns      │ 5.554 µs      │ 36.2 ns       │ 316.1 ns      │ 15280   │ 244480
├─ rwlock_write                                │               │               │               │         │
│  ├─ 0                          4.672 ns      │ 15.08 ns      │ 5.08 ns       │ 5.093 ns      │ 15034   │ 15394816
│  ├─ 1                          4.672 ns      │ 27.58 ns      │ 4.794 ns      │ 4.918 ns      │ 15548   │ 15921152
│  ├─ 2                          4.632 ns      │ 19.72 ns      │ 4.754 ns      │ 4.845 ns      │ 15774   │ 16152576
│  ├─ 4                          4.672 ns      │ 61.07 ns      │ 4.754 ns      │ 4.845 ns      │ 15778   │ 16156672
│  ├─ 8                          4.672 ns      │ 40.84 ns      │ 4.753 ns      │ 4.857 ns      │ 15730   │ 16107520
│  ╰─ 16                         4.672 ns      │ 41.13 ns      │ 4.794 ns      │ 4.897 ns      │ 15618   │ 15992832
├─ rwlock_write_contended                      │               │               │               │         │
│  ├─ 0                          4.672 ns      │ 32.79 ns      │ 4.754 ns      │ 4.893 ns      │ 15629   │ 16004096
│  ├─ 1                          40.7 ns       │ 190.1 ms      │ 40.7 ns       │ 4.322 ms      │ 100     │ 100
│  ├─ 2                          40.7 ns       │ 286.4 µs      │ 40.7 ns       │ 409.6 ns      │ 224276  │ 224276
│  ├─ 4                          40.7 ns       │ 10.02 ms      │ 40.7 ns       │ 14.3 µs       │ 7030    │ 7030
│  ├─ 8                          40.7 ns       │ 20.13 ms      │ 40.7 ns       │ 58.43 µs      │ 1818    │ 1818
│  ╰─ 16                         40.7 ns       │ 31.81 ms      │ 40.7 ns       │ 961.1 µs      │ 105     │ 105
╰─ rwlock_write_contended_clone                │               │               │               │         │
   ├─ 0                          5.242 ns      │ 75.71 ns      │ 5.406 ns      │ 5.478 ns      │ 14332   │ 14675968
   ├─ 1                          5.404 ns      │ 222.3 ns      │ 8.742 ns      │ 9.153 ns      │ 18548   │ 9496576
   ├─ 2                          5.892 ns      │ 412.4 ns      │ 21.68 ns      │ 56.56 ns      │ 6724    │ 1721344
   ├─ 4                          40.7 ns       │ 168.7 µs      │ 124.7 ns      │ 941.3 ns      │ 101518  │ 101518
   ├─ 8                          40.7 ns       │ 10.19 ms      │ 83.7 ns       │ 49.64 µs      │ 2133    │ 2133
   ╰─ 16                         40.7 ns       │ 26.18 ms      │ 124.7 ns      │ 296.1 µs      │ 347     │ 347
```

### AMD Ryzen 7 5800X (x86_64)

```
Timer precision: 20 ns
comparison                       fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ arcswap_load                  3.699 ns      │ 11.13 ns      │ 3.798 ns      │ 3.794 ns      │ 33626   │ 17216512
├─ arcswap_load_contended        3.779 ns      │ 49.56 ns      │ 15.87 ns      │ 15.86 ns      │ 22040   │ 5642240
├─ arcswap_load_no_slot          13.59 ns      │ 42.41 ns      │ 13.91 ns      │ 13.9 ns       │ 48564   │ 6216192
├─ arcswap_load_no_slot_spin     24.16 ns      │ 103.4 ns      │ 24.24 ns      │ 24.24 ns      │ 29583   │ 3786624
├─ arcswap_load_none             3.777 ns      │ 21.44 ns      │ 3.818 ns      │ 3.834 ns      │ 35448   │ 18149376
├─ arcswap_load_spin             19.15 ns      │ 44.59 ns      │ 19.23 ns      │ 19.21 ns      │ 36528   │ 4675584
├─ arcswap_store                               │               │               │               │         │
│  ├─ 0                          58.6 ns       │ 213.7 ns      │ 59.54 ns      │ 59.57 ns      │ 24909   │ 1594176
│  ├─ 1                          58.6 ns       │ 200.1 ns      │ 59.54 ns      │ 59.81 ns      │ 24789   │ 1586496
│  ├─ 2                          58.45 ns      │ 143.6 ns      │ 59.54 ns      │ 59.57 ns      │ 24912   │ 1594368
│  ├─ 4                          58.6 ns       │ 135.4 ns      │ 59.54 ns      │ 59.55 ns      │ 24917   │ 1594688
│  ├─ 8                          80.18 ns      │ 260.2 ns      │ 81.77 ns      │ 81.71 ns      │ 36233   │ 1159456
│  ╰─ 16                         79.87 ns      │ 347.9 ns      │ 81.46 ns      │ 81.71 ns      │ 36223   │ 1159136
├─ arcswap_store_contended                     │               │               │               │         │
│  ├─ 0                          80.18 ns      │ 441.8 ns      │ 81.77 ns      │ 81.79 ns      │ 36216   │ 1158912
│  ├─ 1                          83.06 ns      │ 895.1 ns      │ 167.5 ns      │ 167.9 ns      │ 35704   │ 571264
│  ├─ 2                          97.49 ns      │ 2.023 µs      │ 246.4 ns      │ 247.6 ns      │ 48063   │ 384504
│  ├─ 4                          149.9 ns      │ 985.3 ns      │ 379.2 ns      │ 381.4 ns      │ 31737   │ 253896
│  ├─ 8                          420.4 ns      │ 1.725 µs      │ 688.7 ns      │ 695.1 ns      │ 34542   │ 138168
│  ╰─ 16                         891.2 ns      │ 11.92 µs      │ 1.242 µs      │ 1.253 µs      │ 38239   │ 76478
├─ hazarc_load                   3.935 ns      │ 28.88 ns      │ 4.171 ns      │ 4.185 ns      │ 33275   │ 17036800
├─ hazarc_load_contended         4.091 ns      │ 44.86 ns      │ 28.55 ns      │ 28.52 ns      │ 12860   │ 3292160
├─ hazarc_load_no_slot           9.568 ns      │ 23.62 ns      │ 9.767 ns      │ 9.762 ns      │ 33642   │ 8612352
├─ hazarc_load_no_slot_spin      23.38 ns      │ 109.7 ns      │ 23.46 ns      │ 23.48 ns      │ 30716   │ 3931648
├─ hazarc_load_none              0.501 ns      │ 1.467 ns      │ 0.503 ns      │ 0.505 ns      │ 12018   │ 49225728
├─ hazarc_load_spin              18.13 ns      │ 117.8 ns      │ 18.21 ns      │ 18.21 ns      │ 38767   │ 4962176
├─ hazarc_store                                │               │               │               │         │
│  ├─ 0                          9.767 ns      │ 53.13 ns      │ 10.08 ns      │ 10.1 ns       │ 33348   │ 8537088
│  ├─ 1                          9.689 ns      │ 25.03 ns      │ 10.23 ns      │ 10.7 ns       │ 31733   │ 8123648
│  ├─ 2                          13.13 ns      │ 30.11 ns      │ 13.36 ns      │ 13.4 ns       │ 26019   │ 6660864
│  ├─ 4                          13.09 ns      │ 39.5 ns       │ 13.36 ns      │ 13.37 ns      │ 26069   │ 6673664
│  ├─ 8                          13.09 ns      │ 29.76 ns      │ 13.4 ns       │ 13.39 ns      │ 26040   │ 6666240
│  ╰─ 16                         16.49 ns      │ 70.89 ns      │ 16.96 ns      │ 17.09 ns      │ 41004   │ 5248512
├─ hazarc_store_contended                      │               │               │               │         │
│  ├─ 0                          16.41 ns      │ 147.7 ns      │ 16.96 ns      │ 17.47 ns      │ 40161   │ 5140608
│  ├─ 1                          17.12 ns      │ 222.5 ns      │ 40.76 ns      │ 41.26 ns      │ 35547   │ 2275008
│  ├─ 2                          17.27 ns      │ 421.8 ns      │ 109.3 ns      │ 109.7 ns      │ 27428   │ 877696
│  ├─ 4                          24.74 ns      │ 951.6 ns      │ 276.4 ns      │ 276.7 ns      │ 43229   │ 345832
│  ├─ 8                          57.24 ns      │ 2.208 µs      │ 503.2 ns      │ 506.3 ns      │ 47134   │ 188536
│  ╰─ 16                         370.2 ns      │ 3.27 µs       │ 851.2 ns      │ 859 ns        │ 54700   │ 109400
├─ rwlock_read                                 │               │               │               │         │
│  ├─ t=1                        4.15 ns       │ 16.04 ns      │ 4.189 ns      │ 4.2 ns        │ 34531   │ 17679872
│  ├─ t=2                        4.287 ns      │ 74.81 ns      │ 32.07 ns      │ 31.37 ns      │ 23528   │ 3011584
│  ├─ t=4                        5.373 ns      │ 211.3 ns      │ 127.4 ns      │ 125.7 ns      │ 38372   │ 613952
│  ├─ t=8                        4.466 ns      │ 539.5 ns      │ 209.2 ns      │ 180.6 ns      │ 24768   │ 792576
│  ╰─ t=16                       4.748 ns      │ 1.061 µs      │ 137.8 ns      │ 241.7 ns      │ 19536   │ 625152
├─ rwlock_read_clone                           │               │               │               │         │
│  ├─ t=1                        7.65 ns       │ 53.59 ns      │ 7.81 ns       │ 7.818 ns      │ 40406   │ 10343936
│  ├─ t=2                        8.826 ns      │ 143.7 ns      │ 44.35 ns      │ 44.12 ns      │ 27382   │ 1752448
│  ├─ t=4                        10.99 ns      │ 1.043 µs      │ 165 ns        │ 167.7 ns      │ 34836   │ 557376
│  ├─ t=8                        9.185 ns      │ 860.7 ns      │ 201.3 ns      │ 179.9 ns      │ 27736   │ 443776
│  ╰─ t=16                       9.748 ns      │ 882.6 ns      │ 14.81 ns      │ 109.1 ns      │ 21504   │ 344064
├─ rwlock_read_clone_spin                      │               │               │               │         │
│  ├─ t=1                        18.6 ns       │ 119.5 ns      │ 18.69 ns      │ 18.71 ns      │ 37767   │ 4834176
│  ├─ t=2                        18.84 ns      │ 114.1 ns      │ 74.26 ns      │ 74.25 ns      │ 21838   │ 1397632
│  ├─ t=4                        19.74 ns      │ 817.4 ns      │ 167.4 ns      │ 172.1 ns      │ 40856   │ 326848
│  ├─ t=8                        19.74 ns      │ 657.2 ns      │ 139.9 ns      │ 152.8 ns      │ 31432   │ 251456
│  ╰─ t=16                       18.84 ns      │ 18.01 µs      │ 611.5 ns      │ 612 ns        │ 16976   │ 543232
├─ rwlock_read_contended                       │               │               │               │         │
│  ├─ t=1                        4.248 ns      │ 83.77 ns      │ 10.39 ns      │ 10.44 ns      │ 32360   │ 8284160
│  ├─ t=2                        4.919 ns      │ 225.9 ns      │ 58.13 ns      │ 62.15 ns      │ 22788   │ 1458432
│  ├─ t=4                        8.498 ns      │ 2.342 µs      │ 178.7 ns      │ 216.9 ns      │ 37440   │ 299520
│  ├─ t=8                        7.248 ns      │ 1.346 µs      │ 58.62 ns      │ 176.9 ns      │ 29128   │ 233024
│  ╰─ t=16                       5.373 ns      │ 1.249 µs      │ 14.18 ns      │ 102.2 ns      │ 19120   │ 305920
├─ rwlock_read_contended_clone                 │               │               │               │         │
│  ├─ t=1                        7.888 ns      │ 238.1 ns      │ 50.01 ns      │ 50.47 ns      │ 29253   │ 1872192
│  ├─ t=2                        12.24 ns      │ 927.7 ns      │ 106.2 ns      │ 124.1 ns      │ 29934   │ 478944
│  ├─ t=4                        13.49 ns      │ 1.737 µs      │ 309.1 ns      │ 376.5 ns      │ 31940   │ 255520
│  ├─ t=8                        14.74 ns      │ 2.582 µs      │ 230.2 ns      │ 335.7 ns      │ 29664   │ 118656
│  ╰─ t=16                       10.99 ns      │ 1.969 µs      │ 92.37 ns      │ 219.4 ns      │ 20240   │ 161920
├─ rwlock_read_spin                            │               │               │               │         │
│  ├─ t=1                        17.9 ns       │ 122.7 ns      │ 17.98 ns      │ 17.97 ns      │ 38689   │ 4952192
│  ├─ t=2                        18.21 ns      │ 244.9 ns      │ 52.98 ns      │ 53.12 ns      │ 25432   │ 1627648
│  ├─ t=4                        18.49 ns      │ 468.1 ns      │ 167.6 ns      │ 166 ns        │ 35068   │ 561088
│  ├─ t=8                        18.49 ns      │ 707.2 ns      │ 48.49 ns      │ 101.6 ns      │ 33264   │ 266112
│  ╰─ t=16                       18.49 ns      │ 887 ns        │ 21.62 ns      │ 101 ns        │ 21296   │ 340736
├─ rwlock_write                                │               │               │               │         │
│  ├─ 0                          7.533 ns      │ 53.36 ns      │ 7.732 ns      │ 7.743 ns      │ 40717   │ 10423552
│  ├─ 1                          7.572 ns      │ 60.05 ns      │ 7.728 ns      │ 7.73 ns       │ 40710   │ 10421760
│  ├─ 2                          7.572 ns      │ 28.47 ns      │ 7.732 ns      │ 7.752 ns      │ 40587   │ 10390272
│  ├─ 4                          7.533 ns      │ 22.48 ns      │ 7.732 ns      │ 7.724 ns      │ 40799   │ 10444544
│  ├─ 8                          7.537 ns      │ 22.05 ns      │ 7.732 ns      │ 7.718 ns      │ 40813   │ 10448128
│  ╰─ 16                         7.572 ns      │ 20.37 ns      │ 7.693 ns      │ 7.722 ns      │ 40786   │ 10441216
├─ rwlock_write_contended                      │               │               │               │         │
│  ├─ 0                          7.533 ns      │ 59.19 ns      │ 7.728 ns      │ 7.724 ns      │ 40759   │ 10434304
│  ├─ 1                          7.732 ns      │ 2.805 µs      │ 2 µs          │ 1.982 µs      │ 394     │ 50432
│  ├─ 2                          29.74 ns      │ 18.35 µs      │ 2.775 µs      │ 2.75 µs       │ 35065   │ 35065
│  ├─ 4                          29.74 ns      │ 20.64 µs      │ 4.938 µs      │ 5.039 µs      │ 19422   │ 19422
│  ├─ 8                          29.74 ns      │ 25.74 µs      │ 5.901 µs      │ 6.02 µs       │ 16309   │ 16309
│  ╰─ 16                         29.74 ns      │ 10.7 ms       │ 10.32 µs      │ 17.29 µs      │ 5724    │ 5724
╰─ rwlock_write_contended_clone                │               │               │               │         │
   ├─ 0                          7.554 ns      │ 39.06 ns      │ 7.673 ns      │ 7.678 ns      │ 20872   │ 10686464
   ├─ 1                          7.81 ns       │ 120.2 ns      │ 37.55 ns      │ 37.73 ns      │ 19529   │ 2499712
   ├─ 2                          8.498 ns      │ 991.6 ns      │ 175.6 ns      │ 179.9 ns      │ 33343   │ 533488
   ├─ 4                          14.74 ns      │ 8.841 µs      │ 1.162 µs      │ 1.462 µs      │ 33023   │ 66046
   ├─ 8                          29.74 ns      │ 32.02 µs      │ 6.632 µs      │ 6.897 µs      │ 14235   │ 14235
   ╰─ 16                         29.74 ns      │ 10.9 ms       │ 10.5 µs       │ 16.93 µs      │ 5847    │ 5847
```

## Analysis

### `RwLock`

`RwLock` characteristics are completely different from `AtomicArc`/`ArcSwap`, it is notably blocking and suffers from contention even in read-only workloads. Comparing it is thus quite biased, nevertheless, it gives a sort of baseline, as it is still a standard and commonly used primitive for the use case where `AtomicArc` shines. 

By the way, there are two ways to use `RwLock`: keeping the lock for the entire read duration, or cloning the content (e.g. an `Arc`) and releasing it as soon as possible. The former is more efficient from a read perspective, but it can starve writes, and thus be not suitable for some use cases. 

Anyway, `AtomicArc::load` is 7x faster than `RwLock::read` on Apple M3, without even taking contention into account.

### aarch64

On ARM, `AtomicArc::load` is notably faster than `ArcSwap::load`. A few reasons explain this difference: `AtomicArc` uses a `store` instead of a `swap` in critical path, its thread-local storage is more efficient, and its critical path code is fully inlined.

`AtomicArc::store` is also significantly faster than `ArcSwap::store`, as it relies on `load` + `compare_exchange` instead of sole `compare_exchange`, which is a lot slower when it fails. Its algorithm is also wait-free, but it's hard to measure the impact of this part alone.

Due to the big differences in the `store` algorithm and performance, `_contended` benchmarks results are hard to compare.

### x86_64

Results on x86_64 are a bit surprising, as despite all the reasons given in previous section, `AtomicArc::load` seems slightly slower than `ArcSwap::load`. This is difficult to explain[^1], as both use the exact same atomic operations (seqcst `store` is indeed compiled as a `swap` on x86_64), but `AtomicArc::load` code has fewer instructions and is inlined. However, `load_spin` benchmark, which insert a `std::hint::spin_loop()` call before dropping the load guard, gives consistently better results for `AtomicArc` than `ArcSwap`.

Atomic RMW operations are very costly on x86_64, at the point that they seem to take the whole benchmark time for tiny operations like `AtomicArc::load`. However, in more realistic situation that `load_spin` benchmark tries to simulate, `AtomicArc` optimizations start to pay off. Atomic RMW cost is also visible in `load_no_slot` benchmark, i.e. measuring the fallback algorithm when no hazard pointer slot is available. While `AtomicArc` and `ArcSwap` give similar result on ARM, the higher number of RMW operations in `ArcSwap` fallback algorithm is visible on x86_64. 

[^1]: Actually, I don't even understand it, so any explanation is welcome
