use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    fs::copy("memory.x", out.join("memory.x")).expect("could not copy memory.x");
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");

    let cube_root = PathBuf::from("../drivers/STM32Cube_FW_F7_V1.17.0");
    let bsp = cube_root.join("Drivers/BSP/STM32746G-Discovery");
    let components = cube_root.join("Drivers/BSP/Components");
    let hal = cube_root.join("Drivers/STM32F7xx_HAL_Driver");
    let cmsis = cube_root.join("Drivers/CMSIS");
    let fonts = cube_root.join("Utilities/Fonts");

    println!("cargo:rerun-if-changed=c/display.c");
    println!("cargo:rerun-if-changed=c/board.c");
    println!("cargo:rerun-if-changed=c/stm32f7xx_hal_conf.h");

    let mut build = cc::Build::new();
    build
        .compiler("clang")
        .target("thumbv7em-none-eabihf")
        .define("USE_HAL_DRIVER", None)
        .define("STM32F746xx", None)
        .include(&bsp)
        .include(&components)
        .include(components.join("otm8009a"))
        .include(components.join("Common"))
        .include(hal.join("Inc"))
        .include(cmsis.join("Include"))
        .include(cmsis.join("Device/ST/STM32F7xx/Include"))
        .include(&fonts)
        .include("c")
        .flag("-target")
        .flag("thumbv7em-none-eabihf")
        .flag("-mcpu=cortex-m7")
        .flag("-mthumb")
        .flag("-mfpu=fpv5-sp-d16")
        .flag("-mfloat-abi=hard")
        .flag("-ffreestanding")
        .flag("-fdata-sections")
        .flag("-ffunction-sections")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-function");

    // HAL core + peripherals used by BSP LCD.
    for src in [
        "stm32f7xx_hal.c",
        "stm32f7xx_hal_cortex.c",
        "stm32f7xx_hal_rcc.c",
        "stm32f7xx_hal_rcc_ex.c",
        "stm32f7xx_hal_gpio.c",
        "stm32f7xx_hal_pwr.c",
        "stm32f7xx_hal_pwr_ex.c",
        "stm32f7xx_hal_dma.c",
        "stm32f7xx_hal_dma_ex.c",
        "stm32f7xx_hal_sdram.c",
        "stm32f7xx_hal_ltdc.c",
        "stm32f7xx_hal_ltdc_ex.c",
        "stm32f7xx_hal_dma2d.c",
        "stm32f7xx_hal_dsi.c",
        "stm32f7xx_hal_i2c.c",
        "stm32f7xx_hal_i2c_ex.c",
        "stm32f7xx_hal_flash.c",
        "stm32f7xx_hal_flash_ex.c",
        "stm32f7xx_hal_uart.c",
        "stm32f7xx_hal_uart_ex.c",
    ] {
        build.file(hal.join("Src").join(src));
    }

    // LL FMC for SDRAM init helpers.
    build.file(hal.join("Src").join("stm32f7xx_ll_fmc.c"));

    // Board support + LCD panel driver + system clock defaults.
    for src in [
        bsp.join("stm32746g_discovery.c"),
        bsp.join("stm32746g_discovery_lcd.c"),
        bsp.join("stm32746g_discovery_sdram.c"),
        components.join("otm8009a/otm8009a.c"),
        cmsis.join("Device/ST/STM32F7xx/Source/Templates/system_stm32f7xx.c"),
        PathBuf::from("c/display.c"),
        PathBuf::from("c/board.c"),
    ] {
        build.file(src);
    }

    build.compile("stm32f7-lcd");
}
