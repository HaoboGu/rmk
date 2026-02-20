#include "gazell_shim.h"

// Include Nordic Gazell SDK headers
#include "nrf_gzll.h"
#include "nrf_gzll_error.h"
#include "nrf.h"

// Maximum payload length from Nordic SDK
#define MAX_PAYLOAD_LENGTH 32

// Internal state management
static struct {
    bool initialized;
    gz_mode_t mode;

    // RX state (host mode)
    uint8_t rx_buffer[MAX_PAYLOAD_LENGTH];
    uint8_t rx_length;
    bool rx_ready;

    // TX state (device mode)
    volatile bool tx_success;
    volatile bool tx_failed;
} gz_state = {0};

// Forward declarations of callback functions
void nrf_gzll_device_tx_success(uint32_t pipe, nrf_gzll_device_tx_info_t tx_info);
void nrf_gzll_device_tx_failed(uint32_t pipe, nrf_gzll_device_tx_info_t tx_info);
void nrf_gzll_host_rx_data_ready(uint32_t pipe, nrf_gzll_host_rx_info_t rx_info);
void nrf_gzll_disabled(void);

//-----------------------------------------------------------------------------
// Gazell Callbacks (called from interrupt context)
//-----------------------------------------------------------------------------

/**
 * @brief Callback for successful device transmission
 * Called when ACK is received from host
 */
void nrf_gzll_device_tx_success(uint32_t pipe, nrf_gzll_device_tx_info_t tx_info) {
    (void)pipe;
    (void)tx_info;
    gz_state.tx_success = true;
}

/**
 * @brief Callback for failed device transmission
 * Called when max retries exceeded without ACK
 */
void nrf_gzll_device_tx_failed(uint32_t pipe, nrf_gzll_device_tx_info_t tx_info) {
    (void)pipe;
    (void)tx_info;
    gz_state.tx_failed = true;
}

/**
 * @brief Callback for host receiving data
 * Called when host receives a packet from device
 */
void nrf_gzll_host_rx_data_ready(uint32_t pipe, nrf_gzll_host_rx_info_t rx_info) {
    (void)rx_info;

    // Fetch the packet from RX FIFO
    gz_state.rx_length = MAX_PAYLOAD_LENGTH;
    if (nrf_gzll_fetch_packet_from_rx_fifo(pipe,
                                            gz_state.rx_buffer,
                                            &gz_state.rx_length)) {
        gz_state.rx_ready = true;
    }
}

/**
 * @brief Callback for Gazell disabled event
 */
void nrf_gzll_disabled(void) {
    // Optional: handle disable event
}

//-----------------------------------------------------------------------------
// API Implementation
//-----------------------------------------------------------------------------

gz_error_t gz_init(const gz_config_t* config) {
    if (config == NULL) {
        return GZ_ERR_INVALID_CONFIG;
    }

    // Validate configuration parameters
    if (config->channel > 100) {
        return GZ_ERR_INVALID_CONFIG;
    }

    if (config->data_rate > 2) {
        return GZ_ERR_INVALID_CONFIG;
    }

    if (config->max_retries > 15) {
        return GZ_ERR_INVALID_CONFIG;
    }

    if (config->ack_timeout_us < 250 || config->ack_timeout_us > 4000) {
        return GZ_ERR_INVALID_CONFIG;
    }

    // Clear state
    gz_state.initialized = false;
    gz_state.rx_ready = false;
    gz_state.tx_success = false;
    gz_state.tx_failed = false;

    // Initialize Gazell in device mode (will switch mode later if needed)
    if (!nrf_gzll_init(NRF_GZLL_MODE_DEVICE)) {
        return GZ_ERR_HARDWARE;
    }

    // Configure base address
    uint32_t base_addr = (config->base_address[3] << 24) |
                         (config->base_address[2] << 16) |
                         (config->base_address[1] << 8) |
                         (config->base_address[0]);
    nrf_gzll_set_base_address_0(base_addr);

    // Configure address prefix for pipe 0
    uint8_t prefix[8] = {config->address_prefix, 0, 0, 0, 0, 0, 0, 0};
    nrf_gzll_set_address_prefix_byte(0, config->address_prefix);

    // Configure TX power
    nrf_gzll_set_tx_power((nrf_gzll_tx_power_t)config->tx_power);

    // Configure data rate
    nrf_gzll_datarate_t rate;
    switch (config->data_rate) {
        case 0:
            rate = NRF_GZLL_DATARATE_250KBIT;
            break;
        case 1:
            rate = NRF_GZLL_DATARATE_1MBIT;
            break;
        case 2:
            rate = NRF_GZLL_DATARATE_2MBIT;
            break;
        default:
            return GZ_ERR_INVALID_CONFIG;
    }
    nrf_gzll_set_datarate(rate);

    // Configure channel
    uint8_t channels[] = {config->channel};
    nrf_gzll_set_channel_table(channels, 1);
    nrf_gzll_set_channel_table_size(1);

    // Configure max retries
    nrf_gzll_set_max_tx_attempts(config->max_retries);

    // Configure timeslot period (affects ACK timeout)
    // Convert microseconds to timeslot periods (each period is ~500us at 2Mbps)
    uint32_t timeslot = config->ack_timeout_us / 500;
    if (timeslot < 1) timeslot = 1;
    nrf_gzll_set_timeslot_period(timeslot);

    gz_state.initialized = true;

    return GZ_OK;
}

gz_error_t gz_set_mode(gz_mode_t mode) {
    if (!gz_state.initialized) {
        return GZ_ERR_NOT_INITIALIZED;
    }

    // Disable Gazell before mode change
    nrf_gzll_disable();
    while (nrf_gzll_is_enabled()) {
        // Wait for disable
    }

    // Set new mode
    nrf_gzll_mode_t nrf_mode;
    if (mode == GZ_MODE_DEVICE) {
        nrf_mode = NRF_GZLL_MODE_DEVICE;
    } else if (mode == GZ_MODE_HOST) {
        nrf_mode = NRF_GZLL_MODE_HOST;
    } else {
        return GZ_ERR_INVALID_CONFIG;
    }

    // Reinitialize with new mode
    if (!nrf_gzll_init(nrf_mode)) {
        return GZ_ERR_HARDWARE;
    }

    // Enable Gazell
    if (!nrf_gzll_enable()) {
        return GZ_ERR_HARDWARE;
    }

    gz_state.mode = mode;

    return GZ_OK;
}

gz_error_t gz_send(const uint8_t* data, uint8_t len) {
    if (!gz_state.initialized) {
        return GZ_ERR_NOT_INITIALIZED;
    }

    if (data == NULL) {
        return GZ_ERR_INVALID_CONFIG;
    }

    if (len == 0 || len > MAX_PAYLOAD_LENGTH) {
        return GZ_ERR_FRAME_TOO_LARGE;
    }

    if (gz_state.mode != GZ_MODE_DEVICE) {
        return GZ_ERR_INVALID_CONFIG;
    }

    // Clear TX flags
    gz_state.tx_success = false;
    gz_state.tx_failed = false;

    // Add packet to TX FIFO (pipe 0)
    if (!nrf_gzll_add_packet_to_tx_fifo(0, data, len)) {
        return GZ_ERR_BUSY;
    }

    // Wait for TX complete with timeout
    // Timeout calculation: max_retries * timeslot_period + margin
    // Conservative estimate: 10ms should be sufficient for most cases
    volatile uint32_t timeout = 100000; // ~10ms at 10 cycles per loop

    while (timeout-- > 0) {
        if (gz_state.tx_success) {
            return GZ_OK;
        }
        if (gz_state.tx_failed) {
            return GZ_ERR_SEND_FAILED;
        }
        // Busy wait (could be replaced with WFE in production)
        __NOP();
    }

    // Timeout occurred
    return GZ_ERR_SEND_FAILED;
}

gz_error_t gz_recv(uint8_t* out_buf, uint8_t* out_len, uint8_t max_len) {
    if (!gz_state.initialized) {
        return GZ_ERR_NOT_INITIALIZED;
    }

    if (out_buf == NULL || out_len == NULL) {
        return GZ_ERR_INVALID_CONFIG;
    }

    if (gz_state.mode != GZ_MODE_HOST) {
        return GZ_ERR_INVALID_CONFIG;
    }

    // Check if data is available
    if (!gz_state.rx_ready) {
        *out_len = 0;
        return GZ_OK; // No data available, not an error
    }

    // Check buffer size
    if (gz_state.rx_length > max_len) {
        return GZ_ERR_FRAME_TOO_LARGE;
    }

    // Copy data to output buffer
    for (uint8_t i = 0; i < gz_state.rx_length; i++) {
        out_buf[i] = gz_state.rx_buffer[i];
    }

    *out_len = gz_state.rx_length;
    gz_state.rx_ready = false;

    return GZ_OK;
}

bool gz_is_ready(void) {
    if (!gz_state.initialized) {
        return false;
    }

    // Check if TX FIFO has space
    return nrf_gzll_get_tx_fifo_packet_count(0) < NRF_GZLL_CONST_FIFO_LENGTH;
}

gz_error_t gz_flush(void) {
    if (!gz_state.initialized) {
        return GZ_ERR_NOT_INITIALIZED;
    }

    // Flush TX FIFO
    nrf_gzll_flush_tx_fifo(0);

    // Flush RX FIFO (for host mode)
    if (gz_state.mode == GZ_MODE_HOST) {
        nrf_gzll_flush_rx_fifo(0);
    }

    // Clear state flags
    gz_state.rx_ready = false;
    gz_state.tx_success = false;
    gz_state.tx_failed = false;

    return GZ_OK;
}

void gz_deinit(void) {
    if (gz_state.initialized) {
        nrf_gzll_disable();

        // Wait for disable to complete
        while (nrf_gzll_is_enabled()) {
            __NOP();
        }

        gz_state.initialized = false;
        gz_state.rx_ready = false;
        gz_state.tx_success = false;
        gz_state.tx_failed = false;
    }
}
