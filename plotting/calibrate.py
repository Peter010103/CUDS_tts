import numpy as np
import matplotlib.pyplot as plt
import matplotlib

font = {'size': 12}
matplotlib.rc('font', **font)

# If False, the script only plots from file
calibration = True

# Boolean to indicate when to stop collecting data
collect = True

weight_list = []
lc_outputs = []

plt.figure()

if calibration == True:
    weight = 0.0

    while (collect):
        if len(lc_outputs) == 0:
            lc_val = float(input("lc_output: "))

        else:
            weight += float(input("Input weight (g): "))
            lc_val = float(input("%.2f (g) lc_output: " % weight))

        weight_list.append(weight)
        lc_outputs.append(lc_val)

        if weight > 950.0:
            collect = False

    calibration_data = np.array([weight_list, lc_outputs])
    np.save('./datasets/calibration/calibration_data.npy', calibration_data)

else:
    calibration_data = np.load('./calibration_data.npy')

    weight_list = calibration_data[0].astype(float)
    lc_outputs = calibration_data[1].astype(float)

    # Zero calibration
    lc_outputs -= lc_outputs[0]
    # lc_outputs += 0.02793610

plt.plot(weight_list,
         lc_outputs,
         lw=0.8,
         ls=':',
         marker='x',
         markersize=3,
         color='k')

fit = np.polyfit(weight_list, lc_outputs, 1)
func = np.poly1d(fit)
print('Weight (g) --> Load cell:\n\tf(x) = %.8E x + %.8E' % (fit[0], fit[1]))

x_range = np.linspace(weight_list[0], 2000, 100)
plt.plot(x_range, func(x_range), lw=0.8, color='r')

plt.title(r'f(x)=%.4E x + %.4E' % (fit[0], fit[1]))
plt.xlabel('Weight (g)')
plt.ylabel('Load Cell Output')

plt.grid(ls=':')

fit = np.polyfit(lc_outputs, weight_list, 1)
func = np.poly1d(fit)
print('Load cell --> Weight (g):\n\tg(x) = %.8f x + %.8f' % (fit[0], fit[1]))

plt.tight_layout()
plt.show()
