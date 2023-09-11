import numpy as np
import matplotlib.pyplot as plt
import matplotlib
import pandas as pd
from scipy.interpolate import interp1d
from scipy.signal import savgol_filter

import os
import re
import csv

data_dirs = ['./datasets/tts', './datasets/psu']

colors = [
    '#1b9e77', '#d95f02', '#7570b3', '#e7298a', '#66a61e', '#e6ab02',
    '#a6761d', '#666666'
]

dataframes_dict = {}

# Define helper functions
dshot2percent = interp1d([0, 2000], [0, 1])


def dict_mean(dict_list):
    mean_dict = {}
    for key in dict_list[0].keys():
        mean_dict[key] = sum(d[key] for d in dict_list) / len(dict_list)
    return mean_dict


def extrapolate(x, y, order=3):
    coeffs = np.polyfit(x, y, order)
    x = np.linspace(0, 1, 100)
    y = np.polyval(coeffs, x)

    return x, y, coeffs


for dp in data_dirs:
    filenames = sorted(os.listdir(dp))

    for fn in filenames:
        match = re.search(r'^(.*?_\d+)', fn)

        if match:
            unique_name = match.group(1)
            fp = os.path.join(dp, fn)

            if unique_name not in dataframes_dict:
                dataframes_dict[unique_name] = {'tts': [], 'psu': []}

            if 'tts' in dp:
                dataframes_dict[unique_name]['tts'].append(pd.read_csv(fp))
            elif 'psu' in dp:
                dataframes_dict[unique_name]['psu'].append(pd.read_csv(fp))

final_data_dict = {}

for propeller, df_list in dataframes_dict.items():
    tts_runs = df_list['tts']
    psu_runs = df_list['psu']

    assert len(tts_runs) == len(psu_runs)

    prop_data = []

    for run_idx, (tts_df, psu_df) in enumerate(zip(tts_runs, psu_runs)):
        tts_timestamps = tts_df['Timestamp']
        psu_timestamps = psu_df['Timestamp']

        run_df = pd.DataFrame(
            columns=['cmd', 'Thrust', 'Voltage', 'Current', 'Omega'])

        abort_run = False

        for _, tts_row in tts_df.iterrows():
            tts_ts = tts_row['Timestamp']
            closest_idx = np.argmin(np.abs(psu_timestamps - tts_ts))
            psu_ts = psu_timestamps[closest_idx]

            if np.abs(psu_ts - tts_ts) > 1:
                print("Timestamp error: %s Run %i" % (propeller, run_idx + 1))
                abort_run = True
                break

            cmd = dshot2percent(tts_row['DShot_cmd'])
            thrust = tts_row['Thrust']
            omega = tts_row['Omega']
            voltage = psu_df['Voltage'].iloc[closest_idx]
            current = psu_df['Current'].iloc[closest_idx]

            row = {
                'cmd': cmd,
                'Thrust': thrust,
                'Omega': omega,
                'Voltage': voltage,
                'Current': current
            }

            run_df = pd.concat([run_df, pd.DataFrame([row])],
                               ignore_index=True)

        if abort_run == False: prop_data.append(run_df)

    if propeller not in final_data_dict:
        final_data_dict[propeller] = None

    final_data_dict[propeller] = dict_mean(prop_data)

# Plot Figures
mass = 710

plt.figure(1)

for idx, propeller in enumerate(final_data_dict.keys()):
    cmd = np.array(final_data_dict[propeller]['cmd'], dtype=float)
    thrust = np.array(final_data_dict[propeller]['Thrust'], dtype=float)

    plt.plot(cmd, thrust, label=propeller, color=colors[idx])

    cmd, thrust, _ = extrapolate(cmd, thrust)
    plt.plot(cmd, thrust, ls='-.', color=colors[idx])

    print("%s \tPeak thrust (predicted): %.1f \tTWR: %.2f" %
          (propeller, thrust[-1], thrust[-1] * 4 / mass))

plt.axhline(mass / 4, color='k', ls='--', label='hover')

plt.title(r'Thrust with RC command')
plt.xlabel(r'RC Command')
plt.ylabel(r'Thrust (g)')
plt.legend()
plt.tight_layout()

plt.figure(2)
for propeller in final_data_dict.keys():
    omega = np.array(final_data_dict[propeller]['Omega'])
    omega = savgol_filter(omega, window_length=6, polyorder=3, mode="nearest")

    plt.plot(
        final_data_dict[propeller]['cmd'],
        omega,
        label=propeller,
    )

plt.title(r'Angular Velocity with RC command')
plt.xlabel(r'RC Command')
plt.ylabel(r'$\Omega$ (rad/s)')
plt.legend()
plt.tight_layout()

plt.figure(3)

for propeller in final_data_dict.keys():
    thrust = np.array(final_data_dict[propeller]['Thrust'])
    current = np.array(final_data_dict[propeller]['Current'])
    voltage = np.array(final_data_dict[propeller]['Voltage'])

    efficiency = thrust / (current * voltage)
    efficiency = savgol_filter(efficiency,
                               window_length=20,
                               polyorder=3,
                               mode="nearest")

    plt.plot(
        final_data_dict[propeller]['cmd'],
        efficiency,
        label=propeller,
    )

    print("%s \tPeak efficiency: %.2f \tend efficiency (@ 0.8): %.2f" %
          (propeller, np.amax(efficiency), efficiency[-1]))

plt.axvline(0.4, color='k', ls='--', label='hover')

plt.title(r'Efficiency with RC command')
plt.xlabel(r'RC Command')
plt.ylabel(r'Efficiency (g/W)')
plt.legend(loc='lower right')
plt.tight_layout()

plt.figure(4)

for propeller in final_data_dict.keys():
    thrust = np.array(final_data_dict[propeller]['Thrust'])
    omega = np.array(final_data_dict[propeller]['Omega'])
    omega = savgol_filter(omega, window_length=11, polyorder=3, mode="nearest")

    _, _, coeffs = extrapolate(omega, thrust, order=2)

    print("%s \t coeffs: [%.2e u^2 + %.2e u + %.2e]" %
          (propeller, coeffs[0], coeffs[1], coeffs[2]))
    print(coeffs[0] * 400**2 + coeffs[1] * 400 + coeffs[2])

    plt.plot(
        omega,
        thrust,
        label=propeller,
    )

plt.title(r'Thrust with Angular Velocity')
plt.xlabel(r'$\Omega$ (rad/s)')
plt.ylabel(r'Thrust (g)')
plt.legend()
plt.tight_layout()

plt.show()
