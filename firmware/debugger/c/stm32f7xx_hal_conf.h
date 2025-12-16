#ifndef STM32F7XX_HAL_CONF_H
#define STM32F7XX_HAL_CONF_H

#define HAL_MODULE_ENABLED
#define HAL_RCC_MODULE_ENABLED
#define HAL_GPIO_MODULE_ENABLED
#define HAL_DMA_MODULE_ENABLED
#define HAL_DMA2D_MODULE_ENABLED
#define HAL_FMC_MODULE_ENABLED
#define HAL_SDRAM_MODULE_ENABLED
#define HAL_LTDC_MODULE_ENABLED
#define HAL_DSI_MODULE_ENABLED
#define HAL_I2C_MODULE_ENABLED
#define HAL_PWR_MODULE_ENABLED
#define HAL_CORTEX_MODULE_ENABLED
#define HAL_FLASH_MODULE_ENABLED
#define HAL_UART_MODULE_ENABLED

#define HSE_VALUE ((uint32_t)25000000)
#define HSI_VALUE ((uint32_t)16000000)
#define LSE_VALUE ((uint32_t)32768)
#define LSI_VALUE ((uint32_t)32000)
#define HSE_STARTUP_TIMEOUT ((uint32_t)100)
#define LSE_STARTUP_TIMEOUT ((uint32_t)5000)
#define EXTERNAL_CLOCK_VALUE ((uint32_t)12288000)

#define VDD_VALUE ((uint32_t)3300)
#define TICK_INT_PRIORITY ((uint32_t)0x0F)
#define USE_RTOS 0
#define PREFETCH_ENABLE 1
#define ART_ACCELERATOR_ENABLE 1
#define INSTRUCTION_CACHE_ENABLE 1
#define DATA_CACHE_ENABLE 1

#include "stm32f7xx_hal_def.h"

#ifdef HAL_RCC_MODULE_ENABLED
#include "stm32f7xx_hal_rcc.h"
#include "stm32f7xx_hal_rcc_ex.h"
#endif

#ifdef HAL_GPIO_MODULE_ENABLED
#include "stm32f7xx_hal_gpio.h"
#endif

#ifdef HAL_DMA_MODULE_ENABLED
#include "stm32f7xx_hal_dma.h"
#include "stm32f7xx_hal_dma_ex.h"
#endif

#ifdef HAL_CORTEX_MODULE_ENABLED
#include "stm32f7xx_hal_cortex.h"
#endif

#ifdef HAL_PWR_MODULE_ENABLED
#include "stm32f7xx_hal_pwr.h"
#include "stm32f7xx_hal_pwr_ex.h"
#endif

#ifdef HAL_DMA2D_MODULE_ENABLED
#include "stm32f7xx_hal_dma2d.h"
#endif

#ifdef HAL_LTDC_MODULE_ENABLED
#include "stm32f7xx_hal_ltdc.h"
#include "stm32f7xx_hal_ltdc_ex.h"
#endif

#ifdef HAL_DSI_MODULE_ENABLED
#include "stm32f7xx_hal_dsi.h"
#endif

#ifdef HAL_FMC_MODULE_ENABLED
#include "stm32f7xx_ll_fmc.h"
#endif

#ifdef HAL_SDRAM_MODULE_ENABLED
#include "stm32f7xx_hal_sdram.h"
#endif

#ifdef HAL_I2C_MODULE_ENABLED
#include "stm32f7xx_hal_i2c.h"
#include "stm32f7xx_hal_i2c_ex.h"
#endif

#ifdef HAL_UART_MODULE_ENABLED
#include "stm32f7xx_hal_uart.h"
#include "stm32f7xx_hal_uart_ex.h"
#endif

#ifdef HAL_FMC_MODULE_ENABLED
#include "stm32f7xx_ll_fmc.h"
#endif

#ifdef HAL_FLASH_MODULE_ENABLED
#include "stm32f7xx_hal_flash.h"
#include "stm32f7xx_hal_flash_ex.h"
#endif

#ifndef assert_param
#define assert_param(expr) ((void)0U)
#endif

#endif /* STM32F7XX_HAL_CONF_H */
