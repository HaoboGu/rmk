#ifndef GAZELL_SHIM_H
#define GAZELL_SHIM_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// Error codes (maps 1:1 to Rust WirelessError)
typedef enum {
    GZ_OK = 0,
    GZ_ERR_SEND_FAILED = -1,
    GZ_ERR_RECEIVE_FAILED = -2,
    GZ_ERR_FRAME_TOO_LARGE = -3,
    GZ_ERR_NOT_INITIALIZED = -4,
    GZ_ERR_BUSY = -5,
    GZ_ERR_INVALID_CONFIG = -6,
    GZ_ERR_HARDWARE = -7,
} gz_error_t;

// Configuration (matches GazellConfig in Rust)
typedef struct {
    uint8_t channel;              // RF channel: 0-100
    uint8_t data_rate;            // Data rate: 0=250kbps, 1=1Mbps, 2=2Mbps
    int8_t tx_power;              // TX power in dBm: -40, -20, -16, -12, -8, -4, 0, +3, +4
    uint8_t max_retries;          // Max TX retries: 0-15
    uint16_t ack_timeout_us;      // ACK timeout in microseconds: 250-4000
    uint8_t base_address[4];      // Base address (4 bytes)
    uint8_t address_prefix;       // Address prefix for pipe 0
} gz_config_t;

// Mode selection
typedef enum {
    GZ_MODE_DEVICE = 0,  // Transmitter mode (keyboard)
    GZ_MODE_HOST = 1,    // Receiver mode (dongle)
} gz_mode_t;

/**
 * @brief Initialize Gazell with the given configuration
 *
 * Must be called before any other gz_* functions.
 *
 * @param config Pointer to configuration structure
 * @return GZ_OK on success, error code otherwise
 */
gz_error_t gz_init(const gz_config_t* config);

/**
 * @brief Set Gazell operating mode
 *
 * @param mode GZ_MODE_DEVICE (transmitter) or GZ_MODE_HOST (receiver)
 * @return GZ_OK on success, error code otherwise
 */
gz_error_t gz_set_mode(gz_mode_t mode);

/**
 * @brief Send a frame (blocking with timeout)
 *
 * This function blocks until:
 * - ACK is received (success)
 * - Max retries exceeded (failure)
 * - Timeout occurs (failure)
 *
 * @param data Pointer to data buffer
 * @param len Length of data (max 32 bytes)
 * @return GZ_OK on successful transmission and ACK, error code otherwise
 */
gz_error_t gz_send(const uint8_t* data, uint8_t len);

/**
 * @brief Receive a frame (non-blocking)
 *
 * Checks if a frame is available and copies it to the output buffer.
 * Returns immediately if no data is available.
 *
 * @param out_buf Buffer to store received data
 * @param out_len Pointer to store actual received length (set to 0 if no data)
 * @param max_len Maximum buffer size
 * @return GZ_OK on success (even if no data), error code on failure
 */
gz_error_t gz_recv(uint8_t* out_buf, uint8_t* out_len, uint8_t max_len);

/**
 * @brief Check if Gazell is ready to transmit
 *
 * @return true if TX FIFO has space, false otherwise
 */
bool gz_is_ready(void);

/**
 * @brief Flush all TX and RX FIFOs
 *
 * @return GZ_OK on success, error code otherwise
 */
gz_error_t gz_flush(void);

/**
 * @brief Deinitialize Gazell and disable radio
 *
 * Should be called before entering low-power modes.
 */
void gz_deinit(void);

#ifdef __cplusplus
}
#endif

#endif // GAZELL_SHIM_H
