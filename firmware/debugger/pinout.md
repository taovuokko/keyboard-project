## ✅ Full verified SWD / related pins (for both boards)

### **STM32F746G-DISCO (ST-LINK/V2-1 debug signals)**

The STM32F746G-DISCO board includes an **on-board ST-LINK/V2-1** that exposes SWD signals you can use to debug an external target. From the official schematic:

* **SWCLK** — SWD clock signal for SWD debug
* **SWDIO** — SWD data signal
* **NRST** — MCU reset (optional for controlling target)
* **3V3_ST_LINK / VTref** — reference voltage for target I/O levels
* **GND** — ground reference for debug link

These are present on the board’s SWD breakout area (CN8 pads) even if the header is unpopulated. The **CN14 USB connector** is *not* the SWD interface; it’s the ST-LINK USB connection itself. ([STMicroelectronics][1])

*(Important note: the discovery board’s ST-LINK is normally tied to its own STM32. You will use the same SWCLK/SWDIO signals but connect them to your external target instead.)*

---

### **nice!nano (nRF52840) SWD / debug pins**

According to the official docs, nice!nano exposes these on the back of the board:

* **SWDIO** — connects to the target MCU SWDIO pad
* **SWCLK** — connects to the target MCU SWCLK pad
* **VCC / VTref** — target reference voltage input
* **GND** — ground

This is the standard 4-wire SWD connection expected for nRF52 debugging. ([Nice Keyboards][2])

---

## 🔑 Critical pins for your STM32F746G-DISCO → nice!nano debug connection

You **only need the following four** to get SWD debug working reliably:

| Signal          | Source (STM32F746G-DISCO) | Destination (nice!nano) |
| --------------- | ------------------------- | ----------------------- |
| **VCC / VTref** | 3V3_ST_LINK (VTref) pad   | nice!nano VCC/VTref     |
| **GND**         | Ground                    | nice!nano GND           |
| **SWDIO**       | MCU SWDIO pad (CN8)       | nice!nano SWDIO         |
| **SWCLK**       | MCU SWCLK pad (CN8)       | nice!nano SWCLK         |

✔ These four signals form the **minimum SWD debug bus** required for a debugger to attach. ([STMicroelectronics][1])

---

## ⚠ Additional but optional pins

These are **not required for basic debug attach**, but useful:

* **NRST** — Allows debugger to reset the nice!nano target
* **SWO (Single Wire Output)** — Useful only if your target emits SWO debug output (rarely used with nRF apps)

These are optional and only needed if you want debugger-controlled resets or SWO traces.

---

## 🧠 Why this matters for your setup

* **You do not debug nice!nano over USB** — the USB port is only enumerated when the board’s firmware implements USB.
* **Connecting SWDIO and SWCLK from the STM32 debug module directly lets you control the nRF52840 SWD interface** — that’s how you can flash and use RTT or breakpoint debugging.
* The **ST-LINK USB connection on the discovery board (CN14)** is how your PC talks to the ST-LINK debugger; it is *not* a data interface for the nice!nano debug target itself. ([STMicroelectronics][3])

---

## Quick practical checklist

✔ Confirm 3V3_ST_LINK (VTref) is present and connected to nice!nano VCC → target will be powered at correct logic level.
✔ Connect GND on both boards.
✔ Connect SWDIO and SWCLK directly.
✔ Optionally connect NRST if you want debugger-initiated resets.
✔ Avoid long or loose jumpers; shorter wires improve reliability.
