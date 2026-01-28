# Benchmark

The following benchmark compares `hazarc` with `arc-swap`, but also with standard `RwLock<Arc<T>>`. Refer to the [code](https://github.com/wyfo/hazarc/blob/main/benches/comparison.rs) for details of the benched functions. Guards (`RwLockReadGuard`/`ArcBorrow`/etc.) destructors are included in the measure.

## Results

### Apple M3 (aarch64)

```
Timer precision: 41 ns
comparison                       fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ arcswap_load                  1.922 ns      │ 6.357 ns      │ 2.105 ns      │ 2.108 ns      │ 9101    │ 18638848
├─ arcswap_load_contended        2.166 ns      │ 9.165 ns      │ 5.869 ns      │ 5.892 ns      │ 13407   │ 13728768
├─ arcswap_load_no_slot          5.543 ns      │ 12.7 ns       │ 5.99 ns       │ 5.865 ns      │ 13437   │ 13759488
├─ arcswap_load_no_slot_spin     15.51 ns      │ 67.59 ns      │ 16.65 ns      │ 16.71 ns      │ 21495   │ 5502720
├─ arcswap_load_none             1.942 ns      │ 5.828 ns      │ 2.145 ns      │ 2.154 ns      │ 13859   │ 28383232
├─ arcswap_load_spin             10.95 ns      │ 100.2 ns      │ 11.19 ns      │ 11.37 ns      │ 15322   │ 7844864
├─ arcswap_store                               │               │               │               │         │
│  ├─ 0                          58.96 ns      │ 210.9 ns      │ 59.62 ns      │ 59.83 ns      │ 12729   │ 1629312
│  ├─ 1                          58.96 ns      │ 121.4 ns      │ 59.61 ns      │ 59.69 ns      │ 12757   │ 1632896
│  ├─ 2                          75.89 ns      │ 259.4 ns      │ 77.19 ns      │ 77.21 ns      │ 19762   │ 1264768
│  ├─ 4                          116.9 ns      │ 383.8 ns      │ 119.5 ns      │ 119.3 ns      │ 25666   │ 821312
│  ├─ 8                          200.2 ns      │ 2.21 µs       │ 245.8 ns      │ 245.1 ns      │ 12622   │ 403904
│  ╰─ 16                         369.4 ns      │ 3.051 µs      │ 395.5 ns      │ 414.8 ns      │ 14942   │ 239072
├─ arcswap_store_contended                     │               │               │               │         │
│  ├─ 0                          369.4 ns      │ 890.3 ns      │ 374.7 ns      │ 375.5 ns      │ 16497   │ 263952
│  ├─ 1                          395.5 ns      │ 926.7 ns      │ 458.1 ns      │ 460.5 ns      │ 13471   │ 215536
│  ├─ 2                          489.3 ns      │ 12.24 µs      │ 572.6 ns      │ 572.1 ns      │ 21626   │ 173008
│  ├─ 4                          447.7 ns      │ 7.853 µs      │ 1.051 µs      │ 1.102 µs      │ 22442   │ 89768
│  ├─ 8                          1.291 µs      │ 5.037 ms      │ 2.145 µs      │ 2.429 µs      │ 20272   │ 40544
│  ╰─ 16                         614.2 ns      │ 4.446 ms      │ 1.864 µs      │ 4.341 µs      │ 5736    │ 22944
├─ rwlock_read                                 │               │               │               │         │
│  ├─ t=1                        4.322 ns      │ 13.39 ns      │ 4.404 ns      │ 4.415 ns      │ 17018   │ 17426432
│  ├─ t=2                        4.729 ns      │ 87.2 ns       │ 18.8 ns       │ 19.74 ns      │ 6246    │ 6395904
│  ├─ t=4                        4.271 ns      │ 273.8 ns      │ 6.24 ns       │ 24.74 ns      │ 17952   │ 1148928
│  ├─ t=8                        20.22 ns      │ 9.124 µs      │ 20.72 ns      │ 44.4 ns       │ 15984   │ 31968
│  ╰─ t=16                       3.631 ns      │ 3.829 µs      │ 21.85 ns      │ 317.9 ns      │ 14432   │ 461824
├─ rwlock_read_clone                           │               │               │               │         │
│  ├─ t=1                        6.886 ns      │ 18.07 ns      │ 6.968 ns      │ 6.994 ns      │ 11745   │ 12026880
│  ├─ t=2                        7.049 ns      │ 179.4 ns      │ 71.01 ns      │ 67.25 ns      │ 7896    │ 2021376
│  ├─ t=4                        6.225 ns      │ 652 ns        │ 10.16 ns      │ 70.43 ns      │ 19000   │ 608000
│  ├─ t=8                        4.912 ns      │ 2.629 µs      │ 17.94 ns      │ 231.1 ns      │ 13768   │ 220288
│  ╰─ t=16                       6.225 ns      │ 4.912 µs      │ 1.495 µs      │ 1.793 µs      │ 7984    │ 255488
├─ rwlock_read_clone_spin                      │               │               │               │         │
│  ├─ t=1                        12.17 ns      │ 32.84 ns      │ 12.33 ns      │ 12.38 ns      │ 14201   │ 7270912
│  ├─ t=2                        11.77 ns      │ 179.7 ns      │ 36.17 ns      │ 44.08 ns      │ 15020   │ 1922560
│  ├─ t=4                        14.03 ns      │ 641.6 ns      │ 136.4 ns      │ 162.4 ns      │ 15892   │ 508544
│  ├─ t=8                        15.35 ns      │ 3.343 µs      │ 197.6 ns      │ 294.3 ns      │ 13288   │ 212608
│  ╰─ t=16                       10.1 ns       │ 5.072 µs      │ 41.35 ns      │ 377 ns        │ 15520   │ 124160
├─ rwlock_read_contended                       │               │               │               │         │
│  ├─ t=1                        4.485 ns      │ 126.5 ns      │ 18.76 ns      │ 14.22 ns      │ 6272    │ 6422528
│  ├─ t=2                        4.928 ns      │ 135.1 ns      │ 10.79 ns      │ 16.07 ns      │ 18502   │ 2368256
│  ├─ t=4                        4.287 ns      │ 581.1 ns      │ 49.2 ns       │ 74.61 ns      │ 12312   │ 787968
│  ├─ t=8                        4.912 ns      │ 1.981 µs      │ 30.97 ns      │ 102.8 ns      │ 15832   │ 253312
│  ╰─ t=16                       3.631 ns      │ 43 µs         │ 51.81 ns      │ 251.3 ns      │ 13648   │ 436736
├─ rwlock_read_contended_clone                 │               │               │               │         │
│  ├─ t=1                        7.049 ns      │ 338.9 ns      │ 11.11 ns      │ 11.51 ns      │ 15158   │ 7760896
│  ├─ t=2                        9.162 ns      │ 264.3 ns      │ 21.53 ns      │ 39.6 ns       │ 14428   │ 1846784
│  ├─ t=4                        7.537 ns      │ 2.002 µs      │ 101.2 ns      │ 184.5 ns      │ 13684   │ 218944
│  ├─ t=8                        4.975 ns      │ 591 µs        │ 150.7 ns      │ 371 ns        │ 14256   │ 114048
│  ╰─ t=16                       4.85 ns       │ 7.437 µs      │ 171.6 ns      │ 606.4 ns      │ 15216   │ 121728
├─ rwlock_read_spin                            │               │               │               │         │
│  ├─ t=1                        8.514 ns      │ 127.4 ns      │ 8.676 ns      │ 8.697 ns      │ 18947   │ 9700864
│  ├─ t=2                        10.22 ns      │ 99.25 ns      │ 62.38 ns      │ 57.86 ns      │ 5064    │ 2592768
│  ├─ t=4                        10.13 ns      │ 903.3 ns      │ 14.06 ns      │ 47.52 ns      │ 19780   │ 632960
│  ├─ t=8                        10.78 ns      │ 2.118 µs      │ 301.1 ns      │ 380.5 ns      │ 8824    │ 564736
│  ╰─ t=16                       10.1 ns       │ 4.775 µs      │ 20.47 ns      │ 125.3 ns      │ 16976   │ 135808
├─ rwlock_write                                │               │               │               │         │
│  ├─ 0                          4.525 ns      │ 35.97 ns      │ 4.974 ns      │ 4.925 ns      │ 15491   │ 15862784
│  ├─ 1                          4.525 ns      │ 19.54 ns      │ 4.933 ns      │ 4.862 ns      │ 15683   │ 16059392
│  ├─ 2                          4.485 ns      │ 38.95 ns      │ 4.607 ns      │ 4.789 ns      │ 15904   │ 16285696
│  ├─ 4                          4.525 ns      │ 13.72 ns      │ 4.933 ns      │ 4.8 ns        │ 15874   │ 16254976
│  ├─ 8                          4.525 ns      │ 16.28 ns      │ 4.607 ns      │ 4.76 ns       │ 15998   │ 16381952
│  ╰─ 16                         4.525 ns      │ 21.37 ns      │ 4.607 ns      │ 4.781 ns      │ 15931   │ 16313344
├─ rwlock_write_contended                      │               │               │               │         │
│  ├─ 0                          4.525 ns      │ 12.9 ns       │ 4.729 ns      │ 4.796 ns      │ 15884   │ 16265216
│  ├─ 1                          40.72 ns      │ 80.68 ms      │ 40.72 ns      │ 1.441 ms      │ 100     │ 100
│  ├─ 2                          40.72 ns      │ 297.7 µs      │ 40.72 ns      │ 488.6 ns      │ 190208  │ 190208
│  ├─ 4                          40.72 ns      │ 10.02 ms      │ 40.72 ns      │ 16.64 µs      │ 6092    │ 6092
│  ├─ 8                          40.72 ns      │ 10.29 ms      │ 40.72 ns      │ 154.2 µs      │ 658     │ 658
│  ╰─ 16                         40.72 ns      │ 23.63 ms      │ 40.72 ns      │ 299.5 µs      │ 336     │ 336
├─ rwlock_write_contended_clone                │               │               │               │         │
│  ├─ 0                          5.258 ns      │ 16.36 ns      │ 5.502 ns      │ 5.52 ns       │ 14264   │ 14606336
│  ├─ 1                          5.42 ns       │ 62.55 ns      │ 9.814 ns      │ 10.03 ns      │ 34081   │ 8724736
│  ├─ 2                          4.928 ns      │ 1.366 µs      │ 25.11 ns      │ 62.49 ns      │ 24239   │ 1551296
│  ├─ 4                          40.72 ns      │ 114.9 µs      │ 124.7 ns      │ 969.2 ns      │ 98724   │ 98724
│  ├─ 8                          40.72 ns      │ 20.11 ms      │ 124.7 ns      │ 70.79 µs      │ 1438    │ 1438
│  ╰─ 16                         40.72 ns      │ 29.52 ms      │ 124.7 ns      │ 218.5 µs      │ 468     │ 468
├─ hazarc_load                                 │               │               │               │         │
│  ├─ Adaptive                   0.701 ns      │ 3.051 ns      │ 0.793 ns      │ 0.793 ns      │ 11408   │ 46727168
│  ├─ LockFree                   0.701 ns      │ 5.808 ns      │ 0.793 ns      │ 0.799 ns      │ 11340   │ 46448640
│  ╰─ WaitFree                   0.706 ns      │ 2.084 ns      │ 0.787 ns      │ 0.785 ns      │ 5769    │ 47259648
├─ hazarc_load_contended                       │               │               │               │         │
│  ├─ Adaptive                   0.741 ns      │ 11.93 ns      │ 4.038 ns      │ 4.052 ns      │ 18049   │ 18482176
│  ├─ LockFree                   0.783 ns      │ 14.9 ns       │ 5.503 ns      │ 5.604 ns      │ 14046   │ 14383104
│  ╰─ WaitFree                   0.742 ns      │ 15.59 ns      │ 5.828 ns      │ 5.899 ns      │ 13475   │ 13798400
├─ hazarc_load_no_slot                         │               │               │               │         │
│  ├─ Adaptive                   5.828 ns      │ 34.1 ns       │ 6.519 ns      │ 6.501 ns      │ 12355   │ 12651520
│  ├─ LockFree                   5.34 ns       │ 24.58 ns      │ 5.787 ns      │ 5.692 ns      │ 13806   │ 14137344
│  ╰─ WaitFree                   5.746 ns      │ 14.41 ns      │ 5.828 ns      │ 5.828 ns      │ 13500   │ 13824000
├─ hazarc_load_no_slot_spin                    │               │               │               │         │
│  ├─ Adaptive                   14.86 ns      │ 54.24 ns      │ 15.35 ns      │ 15.3 ns       │ 23299   │ 5964544
│  ├─ LockFree                   14.45 ns      │ 30.32 ns      │ 14.61 ns      │ 14.65 ns      │ 12195   │ 6243840
│  ╰─ WaitFree                   14.45 ns      │ 72.64 ns      │ 14.61 ns      │ 14.73 ns      │ 12132   │ 6211584
├─ hazarc_load_none                            │               │               │               │         │
│  ├─ Adaptive                   0.706 ns      │ 2.008 ns      │ 0.711 ns      │ 0.723 ns      │ 6088    │ 49872896
│  ├─ LockFree                   0.215 ns      │ 0.655 ns      │ 0.218 ns      │ 0.22 ns       │ 4096    │ 67108864
│  ╰─ WaitFree                   0.215 ns      │ 1.273 ns      │ 0.218 ns      │ 0.22 ns       │ 4096    │ 67108864
├─ hazarc_load_pthread                         │               │               │               │         │
│  ├─ Adaptive                   2.853 ns      │ 15.42 ns      │ 2.894 ns      │ 3.313 ns      │ 20674   │ 21170176
│  ├─ LockFree                   2.874 ns      │ 6.862 ns      │ 2.894 ns      │ 2.908 ns      │ 11397   │ 23341056
│  ╰─ WaitFree                   2.65 ns       │ 12.09 ns      │ 2.894 ns      │ 2.892 ns      │ 11452   │ 23453696
├─ hazarc_load_pthread_unsafe                  │               │               │               │         │
│  ├─ Adaptive                   1.033 ns      │ 3.749 ns      │ 1.043 ns      │ 1.052 ns      │ 10131   │ 41496576
│  ├─ LockFree                   0.941 ns      │ 10 ns         │ 1.043 ns      │ 1.042 ns      │ 10208   │ 41811968
│  ╰─ WaitFree                   0.941 ns      │ 2.976 ns      │ 1.063 ns      │ 1.057 ns      │ 10132   │ 41500672
├─ hazarc_load_spin                            │               │               │               │         │
│  ├─ Adaptive                   9.246 ns      │ 29.51 ns      │ 9.408 ns      │ 9.439 ns      │ 18080   │ 9256960
│  ├─ LockFree                   9.246 ns      │ 53.68 ns      │ 9.408 ns      │ 9.461 ns      │ 18040   │ 9236480
│  ╰─ WaitFree                   9.246 ns      │ 47.98 ns      │ 9.408 ns      │ 9.448 ns      │ 18058   │ 9245696
├─ hazarc_store                                │               │               │               │         │
│  ├─ Adaptive                                 │               │               │               │         │
│  │  ├─ 0                       7.455 ns      │ 15.63 ns      │ 8.31 ns       │ 8.296 ns      │ 10078   │ 10319872
│  │  ├─ 1                       10.71 ns      │ 29.67 ns      │ 11.6 ns       │ 11.41 ns      │ 15219   │ 7792128
│  │  ├─ 2                       17.3 ns       │ 85.66 ns      │ 17.62 ns      │ 17.63 ns      │ 20452   │ 5235712
│  │  ├─ 4                       22.34 ns      │ 189.3 ns      │ 22.83 ns      │ 22.91 ns      │ 16018   │ 4100608
│  │  ├─ 8                       35.85 ns      │ 208 ns        │ 36.83 ns      │ 37.36 ns      │ 20009   │ 2561152
│  │  ╰─ 16                      64.78 ns      │ 921.6 ns      │ 67.47 ns      │ 68.62 ns      │ 85797   │ 1372752
│  ├─ LockFree                                 │               │               │               │         │
│  │  ├─ 0                       7.129 ns      │ 31.78 ns      │ 7.293 ns      │ 7.282 ns      │ 22398   │ 11467776
│  │  ├─ 1                       9.082 ns      │ 32.03 ns      │ 9.328 ns      │ 9.408 ns      │ 18123   │ 9278976
│  │  ├─ 2                       15.99 ns      │ 125.5 ns      │ 16.32 ns      │ 16.44 ns      │ 21807   │ 5582592
│  │  ├─ 4                       23.48 ns      │ 187.7 ns      │ 23.97 ns      │ 23.92 ns      │ 15304   │ 3917824
│  │  ├─ 8                       38.78 ns      │ 150.1 ns      │ 39.44 ns      │ 39.56 ns      │ 37358   │ 2390912
│  │  ╰─ 16                      62.22 ns      │ 1.124 µs      │ 77.85 ns      │ 76.06 ns      │ 152357  │ 1218856
│  ╰─ WaitFree                                 │               │               │               │         │
│     ├─ 0                       7.333 ns      │ 30.36 ns      │ 7.578 ns      │ 7.576 ns      │ 10822   │ 11081728
│     ├─ 1                       10.05 ns      │ 34.88 ns      │ 10.46 ns      │ 10.43 ns      │ 16386   │ 8389632
│     ├─ 2                       17.3 ns       │ 164.1 ns      │ 17.79 ns      │ 17.97 ns      │ 19991   │ 5117696
│     ├─ 4                       21.04 ns      │ 67.92 ns      │ 22.83 ns      │ 22.75 ns      │ 16085   │ 4117760
│     ├─ 8                       34.88 ns      │ 98.03 ns      │ 35.52 ns      │ 36.19 ns      │ 20627   │ 2640256
│     ╰─ 16                      65.47 ns      │ 275.1 ns      │ 66.13 ns      │ 66.73 ns      │ 22720   │ 1454080
╰─ hazarc_store_contended                      │               │               │               │         │
   ├─ Adaptive                                 │               │               │               │         │
   │  ├─ 0                       66.77 ns      │ 226.9 ns      │ 67.42 ns      │ 67.98 ns      │ 22303   │ 1427392
   │  ├─ 1                       68.72 ns      │ 1.791 µs      │ 119.5 ns      │ 120.7 ns      │ 25372   │ 811904
   │  ├─ 2                       135.1 ns      │ 856.4 ns      │ 221.1 ns      │ 221.4 ns      │ 27770   │ 444320
   │  ├─ 4                       207.9 ns      │ 3.916 µs      │ 687.2 ns      │ 735.3 ns      │ 33423   │ 133692
   │  ├─ 8                       249.7 ns      │ 2.391 ms      │ 1.489 µs      │ 1.664 µs      │ 14853   │ 59412
   │  ╰─ 16                      239.2 ns      │ 2.595 ms      │ 1.504 µs      │ 3.84 µs       │ 3245    │ 25960
   ├─ LockFree                                 │               │               │               │         │
   │  ├─ 0                       64.81 ns      │ 355.8 ns      │ 66.13 ns      │ 66.53 ns      │ 22767   │ 1457088
   │  ├─ 1                       67.41 ns      │ 435.9 ns      │ 149.4 ns      │ 150.5 ns      │ 20427   │ 653664
   │  ├─ 2                       72.6 ns       │ 744.5 ns      │ 215.9 ns      │ 217.3 ns      │ 28275   │ 452400
   │  ├─ 4                       208.1 ns      │ 2.801 µs      │ 640.3 ns      │ 695.3 ns      │ 17782   │ 142256
   │  ├─ 8                       301.8 ns      │ 644.9 µs      │ 1.411 µs      │ 1.611 µs      │ 7710    │ 61680
   │  ╰─ 16                      239.2 ns      │ 4.153 ms      │ 1.426 µs      │ 3.192 µs      │ 8733    │ 34932
   ╰─ WaitFree                                 │               │               │               │         │
      ├─ 0                       65.47 ns      │ 179.4 ns      │ 66.13 ns      │ 66.75 ns      │ 22711   │ 1453504
      ├─ 1                       67.44 ns      │ 570 ns        │ 152 ns        │ 152.7 ns      │ 20124   │ 643968
      ├─ 2                       75.22 ns      │ 2.174 µs      │ 215.9 ns      │ 217.7 ns      │ 28226   │ 451616
      ├─ 4                       187.2 ns      │ 3.958 µs      │ 614.2 ns      │ 660.7 ns      │ 18721   │ 149768
      ├─ 8                       249.7 ns      │ 2.426 ms      │ 1.416 µs      │ 1.721 µs      │ 14343   │ 57372
      ╰─ 16                      280.9 ns      │ 4.05 ms       │ 1.416 µs      │ 2.67 µs       │ 9722    │ 38888
```

### AMD Ryzen 7 5800X (x86_64)

```
Timer precision: 20 ns
comparison                       fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ arcswap_load                  3.679 ns      │ 18.66 ns      │ 3.779 ns      │ 3.786 ns      │ 33698   │ 17253376
├─ arcswap_load_contended        3.779 ns      │ 31.33 ns      │ 15.2 ns       │ 15.23 ns      │ 22863   │ 5852928
├─ arcswap_load_no_slot          13.56 ns      │ 41.11 ns      │ 13.75 ns      │ 13.77 ns      │ 25009   │ 6402304
├─ arcswap_load_no_slot_spin     24.63 ns      │ 53.2 ns       │ 24.71 ns      │ 24.72 ns      │ 29036   │ 3716608
├─ arcswap_load_none             3.738 ns      │ 10.51 ns      │ 3.818 ns      │ 3.817 ns      │ 35561   │ 18207232
├─ arcswap_load_spin             19.38 ns      │ 51.56 ns      │ 19.46 ns      │ 19.45 ns      │ 36122   │ 4623616
├─ arcswap_store                               │               │               │               │         │
│  ├─ 0                          59.07 ns      │ 132 ns        │ 60.01 ns      │ 60.02 ns      │ 24641   │ 1577024
│  ├─ 1                          59.07 ns      │ 153.1 ns      │ 60.02 ns      │ 60.12 ns      │ 24605   │ 1574720
│  ├─ 2                          80.18 ns      │ 204.4 ns      │ 82.09 ns      │ 82.09 ns      │ 35986   │ 1151552
│  ├─ 4                          123.6 ns      │ 367.9 ns      │ 126.2 ns      │ 126.5 ns      │ 46657   │ 746512
│  ├─ 8                          229.5 ns      │ 499.4 ns      │ 235.1 ns      │ 235 ns        │ 25784   │ 412544
│  ╰─ 16                         435.4 ns      │ 1.595 µs      │ 445.6 ns      │ 445.9 ns      │ 27257   │ 218056
├─ arcswap_store_contended                     │               │               │               │         │
│  ├─ 0                          435.4 ns      │ 958.9 ns      │ 445.6 ns      │ 445.9 ns      │ 27245   │ 217960
│  ├─ 1                          447.9 ns      │ 1.84 µs       │ 502.9 ns      │ 505.3 ns      │ 47251   │ 189004
│  ├─ 2                          475.6 ns      │ 1.556 µs      │ 544.4 ns      │ 546.9 ns      │ 22325   │ 178600
│  ├─ 4                          518.2 ns      │ 2.394 µs      │ 645.7 ns      │ 649.9 ns      │ 37108   │ 148432
│  ├─ 8                          610.9 ns      │ 2.116 µs      │ 833.7 ns      │ 840.2 ns      │ 28934   │ 115736
│  ╰─ 16                         826.2 ns      │ 6.171 µs      │ 1.221 µs      │ 1.233 µs      │ 38797   │ 77594
├─ rwlock_read                                 │               │               │               │         │
│  ├─ t=1                        4.228 ns      │ 21.42 ns      │ 4.306 ns      │ 4.307 ns      │ 32585   │ 16683520
│  ├─ t=2                        4.521 ns      │ 61.34 ns      │ 35.36 ns      │ 35.17 ns      │ 22212   │ 2843136
│  ├─ t=4                        5.373 ns      │ 365.4 ns      │ 136.8 ns      │ 136.1 ns      │ 38328   │ 613248
│  ├─ t=8                        5.373 ns      │ 476.2 ns      │ 54.18 ns      │ 109.2 ns      │ 29800   │ 476800
│  ╰─ t=16                       4.748 ns      │ 1.134 µs      │ 239.8 ns      │ 298.2 ns      │ 19264   │ 616448
├─ rwlock_read_clone                           │               │               │               │         │
│  ├─ t=1                        7.65 ns       │ 33.48 ns      │ 7.806 ns      │ 7.792 ns      │ 40499   │ 10367744
│  ├─ t=2                        8.513 ns      │ 179.2 ns      │ 49.37 ns      │ 49.12 ns      │ 26618   │ 1703552
│  ├─ t=4                        10.37 ns      │ 515.7 ns      │ 175.6 ns      │ 171.5 ns      │ 33320   │ 533120
│  ├─ t=8                        9.123 ns      │ 527.6 ns      │ 263.3 ns      │ 255.2 ns      │ 22536   │ 721152
│  ╰─ t=16                       8.841 ns      │ 1.226 µs      │ 388.9 ns      │ 435.9 ns      │ 18112   │ 579584
├─ rwlock_read_clone_spin                      │               │               │               │         │
│  ├─ t=1                        18.84 ns      │ 66.75 ns      │ 18.92 ns      │ 18.96 ns      │ 36893   │ 4722304
│  ├─ t=2                        19.15 ns      │ 174.4 ns      │ 86.93 ns      │ 87.03 ns      │ 19946   │ 1276544
│  ├─ t=4                        19.74 ns      │ 488.8 ns      │ 154.4 ns      │ 156.6 ns      │ 35192   │ 563072
│  ├─ t=8                        19.12 ns      │ 597.1 ns      │ 217.6 ns      │ 208.9 ns      │ 27040   │ 432640
│  ╰─ t=16                       19.12 ns      │ 1.001 µs      │ 48.27 ns      │ 148.8 ns      │ 21232   │ 339712
├─ rwlock_read_contended                       │               │               │               │         │
│  ├─ t=1                        4.521 ns      │ 34.06 ns      │ 11.64 ns      │ 11.7 ns       │ 28781   │ 7367936
│  ├─ t=2                        5.076 ns      │ 209.9 ns      │ 61.43 ns      │ 64.12 ns      │ 22744   │ 1455616
│  ├─ t=4                        6.623 ns      │ 20.84 µs      │ 186.9 ns      │ 197.8 ns      │ 32596   │ 521536
│  ├─ t=8                        5.373 ns      │ 1.054 µs      │ 272.1 ns      │ 253.6 ns      │ 27712   │ 443392
│  ╰─ t=16                       5.373 ns      │ 1.258 µs      │ 15.43 ns      │ 117.6 ns      │ 19504   │ 312064
├─ rwlock_read_contended_clone                 │               │               │               │         │
│  ├─ t=1                        7.732 ns      │ 168.9 ns      │ 50.62 ns      │ 51.06 ns      │ 28913   │ 1850432
│  ├─ t=2                        13.84 ns      │ 481.9 ns      │ 138.7 ns      │ 147.5 ns      │ 21282   │ 681024
│  ├─ t=4                        17.24 ns      │ 3.538 µs      │ 250.2 ns      │ 338.8 ns      │ 39684   │ 158736
│  ├─ t=8                        14.74 ns      │ 3.032 µs      │ 247.7 ns      │ 360.5 ns      │ 30448   │ 121792
│  ╰─ t=16                       10.99 ns      │ 2.215 µs      │ 77.37 ns      │ 198 ns        │ 19328   │ 154624
├─ rwlock_read_spin                            │               │               │               │         │
│  ├─ t=1                        17.9 ns       │ 50.31 ns      │ 17.98 ns      │ 17.97 ns      │ 38709   │ 4954752
│  ├─ t=2                        18.21 ns      │ 112.4 ns      │ 51.4 ns       │ 51.77 ns      │ 25832   │ 1653248
│  ├─ t=4                        18.49 ns      │ 421.7 ns      │ 163.8 ns      │ 161.1 ns      │ 35528   │ 568448
│  ├─ t=8                        18.49 ns      │ 610.9 ns      │ 270.2 ns      │ 229.5 ns      │ 28272   │ 452352
│  ╰─ t=16                       18.21 ns      │ 1.056 µs      │ 511.7 ns      │ 463.2 ns      │ 17824   │ 570368
├─ rwlock_write                                │               │               │               │         │
│  ├─ 0                          7.572 ns      │ 28.27 ns      │ 7.732 ns      │ 7.735 ns      │ 40742   │ 10429952
│  ├─ 1                          7.572 ns      │ 27.18 ns      │ 7.732 ns      │ 7.737 ns      │ 40750   │ 10432000
│  ├─ 2                          7.572 ns      │ 30.08 ns      │ 7.732 ns      │ 7.741 ns      │ 40722   │ 10424832
│  ├─ 4                          7.572 ns      │ 32.15 ns      │ 7.728 ns      │ 7.729 ns      │ 40768   │ 10436608
│  ├─ 8                          7.572 ns      │ 24.79 ns      │ 7.732 ns      │ 7.75 ns       │ 40686   │ 10415616
│  ╰─ 16                         7.572 ns      │ 30.04 ns      │ 7.732 ns      │ 7.732 ns      │ 40766   │ 10436096
├─ rwlock_write_contended                      │               │               │               │         │
│  ├─ 0                          7.572 ns      │ 25.61 ns      │ 7.732 ns      │ 7.741 ns      │ 40714   │ 10422784
│  ├─ 1                          29.74 ns      │ 36.51 µs      │ 1.942 µs      │ 2.995 µs      │ 32313   │ 32313
│  ├─ 2                          29.74 ns      │ 19.67 µs      │ 2.855 µs      │ 2.953 µs      │ 32710   │ 32710
│  ├─ 4                          29.74 ns      │ 19.31 µs      │ 4.929 µs      │ 4.744 µs      │ 20594   │ 20594
│  ├─ 8                          29.74 ns      │ 24.04 µs      │ 5.68 µs       │ 5.727 µs      │ 17117   │ 17117
│  ╰─ 16                         19.74 ns      │ 9.847 ms      │ 10.38 µs      │ 15.58 µs      │ 6342    │ 6342
├─ rwlock_write_contended_clone                │               │               │               │         │
│  ├─ 0                          7.572 ns      │ 159.6 ns      │ 7.732 ns      │ 7.733 ns      │ 40730   │ 10426880
│  ├─ 1                          7.888 ns      │ 129.9 ns      │ 43.57 ns      │ 43.84 ns      │ 33379   │ 2136256
│  ├─ 2                          8.498 ns      │ 592.7 ns      │ 193.8 ns      │ 193.9 ns      │ 31021   │ 496336
│  ├─ 4                          14.74 ns      │ 7.809 µs      │ 1.222 µs      │ 1.506 µs      │ 32060   │ 64120
│  ├─ 8                          29.74 ns      │ 30.33 µs      │ 6.271 µs      │ 6.337 µs      │ 15490   │ 15490
│  ╰─ 16                         29.74 ns      │ 15.71 ms      │ 10.48 µs      │ 17 µs         │ 5823    │ 5823
├─ hazarc_load                                 │               │               │               │         │
│  ├─ Adaptive                   3.679 ns      │ 13.13 ns      │ 3.876 ns      │ 3.872 ns      │ 36799   │ 18841088
│  ├─ LockFree                   3.955 ns      │ 13.28 ns      │ 4.171 ns      │ 4.18 ns       │ 34818   │ 17826816
│  ╰─ WaitFree                   3.935 ns      │ 12.17 ns      │ 4.15 ns       │ 4.163 ns      │ 33451   │ 17126912
├─ hazarc_load_contended                       │               │               │               │         │
│  ├─ Adaptive                   3.896 ns      │ 72.84 ns      │ 25.89 ns      │ 25.87 ns      │ 28044   │ 3589632
│  ├─ LockFree                   4.123 ns      │ 95.7 ns       │ 31.06 ns      │ 31.07 ns      │ 46280   │ 2961920
│  ╰─ WaitFree                   4.13 ns       │ 62.2 ns       │ 26.43 ns      │ 26.4 ns       │ 27320   │ 3496960
├─ hazarc_load_no_slot                         │               │               │               │         │
│  ├─ Adaptive                   9.607 ns      │ 31.91 ns      │ 9.923 ns      │ 9.917 ns      │ 33888   │ 8675328
│  ├─ LockFree                   9.568 ns      │ 26.12 ns      │ 9.763 ns      │ 9.758 ns      │ 34383   │ 8802048
│  ╰─ WaitFree                   9.533 ns      │ 27.65 ns      │ 9.763 ns      │ 9.759 ns      │ 33657   │ 8616192
├─ hazarc_load_no_slot_spin                    │               │               │               │         │
│  ├─ Adaptive                   24.63 ns      │ 65.49 ns      │ 24.71 ns      │ 24.75 ns      │ 28994   │ 3711232
│  ├─ LockFree                   23.61 ns      │ 62.28 ns      │ 23.7 ns       │ 23.73 ns      │ 30157   │ 3860096
│  ╰─ WaitFree                   23.61 ns      │ 112 ns        │ 23.7 ns       │ 23.73 ns      │ 30142   │ 3858176
├─ hazarc_load_none                            │               │               │               │         │
│  ├─ Adaptive                   1.254 ns      │ 3.808 ns      │ 1.259 ns      │ 1.259 ns      │ 19082   │ 39079936
│  ├─ LockFree                   0.254 ns      │ 1.181 ns      │ 0.308 ns      │ 0.306 ns      │ 13312   │ 54525952
│  ╰─ WaitFree                   0.251 ns      │ 1.088 ns      │ 0.254 ns      │ 0.255 ns      │ 13686   │ 56057856
├─ hazarc_load_pthread                         │               │               │               │         │
│  ├─ Adaptive                   5.519 ns      │ 17.16 ns      │ 5.56 ns       │ 5.661 ns      │ 26625   │ 13632000
│  ├─ LockFree                   5.011 ns      │ 13.03 ns      │ 5.031 ns      │ 5.043 ns      │ 30176   │ 15450112
│  ╰─ WaitFree                   5.265 ns      │ 18.06 ns      │ 5.519 ns      │ 5.449 ns      │ 27377   │ 14017024
├─ hazarc_load_pthread_unsafe                  │               │               │               │         │
│  ├─ Adaptive                   3.757 ns      │ 15.26 ns      │ 3.915 ns      │ 3.92 ns       │ 34882   │ 17859584
│  ├─ LockFree                   3.818 ns      │ 10.27 ns      │ 4.013 ns      │ 4.007 ns      │ 35897   │ 18379264
│  ╰─ WaitFree                   3.835 ns      │ 15.02 ns      │ 3.994 ns      │ 3.999 ns      │ 35957   │ 18409984
├─ hazarc_load_spin                            │               │               │               │         │
│  ├─ Adaptive                   18.13 ns      │ 44.36 ns      │ 18.21 ns      │ 18.2 ns       │ 38272   │ 4898816
│  ├─ LockFree                   18.13 ns      │ 76.99 ns      │ 18.21 ns      │ 18.22 ns      │ 38283   │ 4900224
│  ╰─ WaitFree                   18.13 ns      │ 44.75 ns      │ 18.21 ns      │ 18.21 ns      │ 38295   │ 4901760
├─ hazarc_store                                │               │               │               │         │
│  ├─ Adaptive                                 │               │               │               │         │
│  │  ├─ 0                       11.83 ns      │ 33.75 ns      │ 11.88 ns      │ 11.92 ns      │ 28868   │ 7390208
│  │  ├─ 1                       17.2 ns       │ 48.74 ns      │ 17.98 ns      │ 17.9 ns       │ 39322   │ 5033216
│  │  ├─ 2                       23.14 ns      │ 52.27 ns      │ 23.92 ns      │ 23.69 ns      │ 30442   │ 3896576
│  │  ├─ 4                       33.87 ns      │ 232.9 ns      │ 34.66 ns      │ 34.76 ns      │ 41741   │ 2671424
│  │  ├─ 8                       59.52 ns      │ 185.7 ns      │ 60.18 ns      │ 60.33 ns      │ 48502   │ 1552064
│  │  ╰─ 16                      101.1 ns      │ 353.5 ns      │ 102.1 ns      │ 102.4 ns      │ 29337   │ 938784
│  ├─ LockFree                                 │               │               │               │         │
│  │  ├─ 0                       9.529 ns      │ 23.34 ns      │ 9.689 ns      │ 9.717 ns      │ 33780   │ 8647680
│  │  ├─ 1                       12.58 ns      │ 28.59 ns      │ 12.89 ns      │ 12.89 ns      │ 26497   │ 6783232
│  │  ├─ 2                       18.37 ns      │ 67.77 ns      │ 18.84 ns      │ 18.96 ns      │ 36930   │ 4727040
│  │  ├─ 4                       28.24 ns      │ 88.11 ns      │ 29.02 ns      │ 29.1 ns       │ 24967   │ 3195776
│  │  ├─ 8                       54.84 ns      │ 164.1 ns      │ 55.15 ns      │ 55.32 ns      │ 52488   │ 1679616
│  │  ╰─ 16                      94.87 ns      │ 890.2 ns      │ 97.37 ns      │ 97.52 ns      │ 113741  │ 909928
│  ╰─ WaitFree                                 │               │               │               │         │
│     ├─ 0                       9.49 ns       │ 35.24 ns      │ 9.689 ns      │ 9.697 ns      │ 33824   │ 8658944
│     ├─ 1                       12.58 ns      │ 33.91 ns      │ 12.9 ns       │ 12.94 ns      │ 26402   │ 6758912
│     ├─ 2                       19.08 ns      │ 58.6 ns       │ 21.19 ns      │ 21.11 ns      │ 33516   │ 4290048
│     ├─ 4                       28.38 ns      │ 144.7 ns      │ 29.65 ns      │ 30.24 ns      │ 47150   │ 3017600
│     ├─ 8                       53.27 ns      │ 191.6 ns      │ 54.7 ns       │ 54.92 ns      │ 27031   │ 1729984
│     ╰─ 16                      93.96 ns      │ 350.4 ns      │ 94.93 ns      │ 95.39 ns      │ 31351   │ 1003232
╰─ hazarc_store_contended                      │               │               │               │         │
   ├─ Adaptive                                 │               │               │               │         │
   │  ├─ 0                       101.1 ns      │ 359.8 ns      │ 102.1 ns      │ 102.7 ns      │ 29197   │ 934304
   │  ├─ 1                       102.4 ns      │ 502.5 ns      │ 223.9 ns      │ 224.8 ns      │ 26948   │ 431168
   │  ├─ 2                       103 ns        │ 830 ns        │ 260.2 ns      │ 261.5 ns      │ 23259   │ 372144
   │  ├─ 4                       225.1 ns      │ 1.074 µs      │ 405.4 ns      │ 406.2 ns      │ 29824   │ 238592
   │  ├─ 8                       410.4 ns      │ 1.71 µs       │ 620.9 ns      │ 624.9 ns      │ 38223   │ 152892
   │  ╰─ 16                      575.7 ns      │ 3.335 µs      │ 971.7 ns      │ 981.4 ns      │ 48121   │ 96242
   ├─ LockFree                                 │               │               │               │         │
   │  ├─ 0                       93.34 ns      │ 388.6 ns      │ 93.99 ns      │ 94.28 ns      │ 31656   │ 1012992
   │  ├─ 1                       94.31 ns      │ 467.4 ns      │ 178.2 ns      │ 179.3 ns      │ 33506   │ 536096
   │  ├─ 2                       106.1 ns      │ 530.7 ns      │ 228.9 ns      │ 229.7 ns      │ 26375   │ 422000
   │  ├─ 4                       97.37 ns      │ 960.2 ns      │ 332.8 ns      │ 334 ns        │ 36040   │ 288320
   │  ├─ 8                       240.2 ns      │ 1.512 µs      │ 553.2 ns      │ 555.8 ns      │ 42703   │ 170812
   │  ╰─ 16                      465.2 ns      │ 4.273 µs      │ 871.2 ns      │ 880.5 ns      │ 53334   │ 106668
   ╰─ WaitFree                                 │               │               │               │         │
      ├─ 0                       93.65 ns      │ 318.4 ns      │ 95.24 ns      │ 95.4 ns       │ 31315   │ 1002080
      ├─ 1                       94.93 ns      │ 788.7 ns      │ 176.9 ns      │ 177.8 ns      │ 33814   │ 541024
      ├─ 2                       96.18 ns      │ 468.1 ns      │ 229.5 ns      │ 230 ns        │ 26349   │ 421584
      ├─ 4                       142.4 ns      │ 727.3 ns      │ 330.3 ns      │ 331.7 ns      │ 36287   │ 290296
      ├─ 8                       202.7 ns      │ 2.852 µs      │ 553.4 ns      │ 558.2 ns      │ 42613   │ 170452
      ╰─ 16                      415.2 ns      │ 13 µs         │ 876.7 ns      │ 887.9 ns      │ 52954   │ 105908
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
