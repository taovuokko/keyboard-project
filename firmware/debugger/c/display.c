#include <stdbool.h>
#include <stdint.h>

#include "stm32f7xx_hal.h"
#include "stm32746g_discovery.h"
#include "stm32746g_discovery_lcd.h"

static bool lcd_ready = false;
static bool last_rx = false;
static bool last_tx = false;
static uint32_t last_err = 0xffffffffu;

void display_init(void) {
    if (BSP_LCD_Init() != LCD_OK) {
        return;
    }

    BSP_LCD_LayerDefaultInit(0, LCD_FB_START_ADDRESS);
    BSP_LCD_SelectLayer(0);
    BSP_LCD_DisplayOn();

    BSP_LCD_Clear(LCD_COLOR_BLACK);
    BSP_LCD_SetBackColor(LCD_COLOR_BLACK);
    BSP_LCD_SetTextColor(LCD_COLOR_WHITE);
    BSP_LCD_SetFont(&Font24);

    lcd_ready = true;
}

static void draw_status(bool rx_ok, bool tx_ok, uint32_t err) {
    BSP_LCD_Clear(LCD_COLOR_BLACK);

    BSP_LCD_SetTextColor(LCD_COLOR_WHITE);
    BSP_LCD_DisplayStringAt(0, 20, (uint8_t *)"DEBUG STATUS", CENTER_MODE);

    BSP_LCD_SetTextColor(rx_ok ? LCD_COLOR_GREEN : LCD_COLOR_RED);
    BSP_LCD_DisplayStringAt(0, 80, (uint8_t *)(rx_ok ? "RX (D2) : OK" : "RX (D2) : FAIL"), LEFT_MODE);

    BSP_LCD_SetTextColor(tx_ok ? LCD_COLOR_GREEN : LCD_COLOR_RED);
    BSP_LCD_DisplayStringAt(0, 120, (uint8_t *)(tx_ok ? "TX (D3) : OK" : "TX (D3) : FAIL"), LEFT_MODE);

    BSP_LCD_SetTextColor(err == 0 ? LCD_COLOR_WHITE : LCD_COLOR_RED);

    char buf[] = "ERR CODE : 0x00";
    uint8_t val = (uint8_t)(err & 0xFF);
    const char hex[] = "0123456789ABCDEF";
    buf[13] = hex[(val >> 4) & 0xF];
    buf[14] = hex[val & 0xF];
    BSP_LCD_DisplayStringAt(0, 180, (uint8_t *)buf, LEFT_MODE);
}

void display_update(bool rx_ok, bool tx_ok, uint32_t err_code) {
    if (!lcd_ready) {
        return;
    }

    if (rx_ok == last_rx && tx_ok == last_tx && err_code == last_err) {
        return;
    }

    last_rx = rx_ok;
    last_tx = tx_ok;
    last_err = err_code;

    draw_status(rx_ok, tx_ok, err_code);
}
