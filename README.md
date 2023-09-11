# thrust_test_stand

## Description
A collection of scripts to log and plot data obtained from the CUDS thrust test stand.

The scripts assume HX711 load cell amplifiers are connected to a Raspberry Pi. (See the code for GPIO pins).

The `logging` scripts are written in Rust (see [link](https://www.rust-lang.org/tools/install) for installation instructions) and generate .csv files.

After compiling the rust code with `cargo build`
```
cd logging/
./target/debug/pi-tts <DATASET_NAME.csv>
```

Visualization of the collected dataset in `plotting` is done by
```
cd plotting/
python3 plot_data.py
```
