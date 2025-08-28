import time
import numpy as np
import sys
import pandas as pd

import pickle

# get the file name from the arguments
if len(sys.argv) != 2:
    print("Error: Invalid number of arguments")
    print("Usage: python plot_test_data.py <file_name>")
    sys.exit()

# Getting back the objects:
filename = sys.argv[1]
df = pd.read_csv(filename)
t = np.array(df['timestamp'])
pos = np.array([df['present_ring'],df['present_center']]).T
pos_mot = np.array([df['present_pos_a'],df['present_pos_b']]).T
tar = np.array([df['target_ring'],df['target_center']]).T
vel = np.array([df['present_velocity_a'],df['present_velocity_b']]).T
torque = np.array([df['present_torque_ring'],df['present_torque_center']]).T
current = np.array([df['present_current_a'],df['present_current_b']]).T
axis_sensors = np.array([df['axis_sensor_ring'],df['axis_sensor_center']]).T
axis_zeros = np.array([df['axis_zeros_ring'],df['axis_zeros_center']]).T
n_axis = 2


print("Plotting")
import matplotlib.pyplot as plt

fig, axs = plt.subplots(3,n_axis, figsize=(10,10), sharex=True)

for i, a in enumerate(axs.T):
    a[0].step(t,pos_mot[:,i], label = "measured")
    a[1].step(t,vel[:,i], label = "measured")
    a[2].step(t, current[:,i], label = "measured")
    a[2].step(t, np.ones_like(t)*200, 'k--', linewidth=2, label = "max allowed current")
    a[2].step(t, np.ones_like(t)*-200, 'k--', linewidth=2)

axs[0,0].set_title("A")
axs[0,1].set_title("B")



# set title of the figure overall
fig.suptitle("Motor variables during the test")

for i, a in enumerate(axs[:].T):
    if i == 0:
        a[0].set_ylabel("position [rad]")
        a[1].set_ylabel("velocity [rad/s]")
        a[2].set_ylabel("current [mA]")
    a[2].set_ylim([-500,500])
    a[0].grid()
    a[1].grid()
    a[2].grid()
a[0].legend()
a[1].legend()
a[2].legend()

plt.show()

def wrap(angle):
    return (angle + 2 * np.pi) % (2 * np.pi)

axis_readings_initial = np.array(axis_zeros)

axis_sensors = axis_sensors
axis_sensors = wrap(axis_sensors - axis_readings_initial)

axis_calc = pos_mot.T % (2*np.pi) - axis_readings_initial.T
axis_calc = wrap(axis_calc)

axis_error = axis_calc - axis_sensors.T
for i, ax_e in enumerate(axis_error):
    for j,a in enumerate(ax_e):
        if np.abs(a) > np.pi:
            axis_error[i,j] = a - (np.sign(a))*2*np.pi


fig, axs = plt.subplots(3, n_axis, figsize=(10,10), sharex=True)

# add the title
fig.suptitle("Absolute position of the orbita and backlash axis")

for i, a in enumerate(axs.T):
    a[0].step(t, np.rad2deg(pos[:,i]), label="measured")
    a[0].step(t, np.rad2deg(tar[:,i]), label="target")
    a[1].step(t, np.rad2deg(axis_sensors[:,i]), label = "actual [deg]")
    a[1].step(t, np.rad2deg(axis_calc[i,:]), label = "estimated [deg]")
    a[2].step(t, np.rad2deg(axis_error[i,:]), label = "backlash [deg]")

for i, a in enumerate(axs[:].T):
    if i == 0:
        a[0].set_ylabel("orbita position [deg]")
        a[2].set_ylabel("backlash axis position [deg]")
        a[1].set_ylabel("motor position [deg]")
    a[0].grid()
    a[1].grid()
    a[2].grid()
a[0].legend()
a[1].legend()
a[2].legend()



axs[0,0].set_title("Ring")
axs[0,1].set_title("Center")

axs[1,0].set_title("A")
axs[1,1].set_title("B")

plt.legend()

plt.legend()
plt.show()
