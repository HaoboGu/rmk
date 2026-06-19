use rmk::event::{LayerChangeEvent, PointingProcessorEvent, publish_event};
use rmk::input_device::pointing::PointingMode;
use rmk::macros::processor;
use rmk::types::keycode::HidKeyCode;

#[processor(subscribe = [LayerChangeEvent])]
pub struct PointingProcessorController;

impl PointingProcessorController {
    pub fn new() -> Self {
        Self
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        match event.0 {
            0 => {
                publish_event(PointingProcessorEvent {
                    device_id: 255,
                    mode: PointingMode::Cursor(rmk::input_device::pointing::CursorConfig::default()),
                });
            }
            1 => {
                publish_event(PointingProcessorEvent {
                    device_id: 255,
                    mode: PointingMode::Sniper(rmk::input_device::pointing::SniperConfig {
                        multiplier: 1,
                        divisor: 8,
                        invert_x: false,
                        invert_y: false,
                    }),
                });
            }
            2 => {
                publish_event(PointingProcessorEvent {
                    device_id: 255,
                    mode: PointingMode::Scroll(rmk::input_device::pointing::ScrollConfig {
                        multiplier_x: 1,
                        divisor_x: 16,
                        multiplier_y: 1,
                        divisor_y: 16,
                        invert_x: false,
                        invert_y: false,
                    }),
                });
            }
            3 => {
                publish_event(PointingProcessorEvent {
                    device_id: 255,
                    mode: PointingMode::Caret(rmk::input_device::pointing::CaretConfig {
                        disable_x: false,
                        disable_y: false,
                        invert_x: false,
                        invert_y: false,
                        threshold: 100,
                        keycode_up: HidKeyCode::Up,
                        keycode_down: HidKeyCode::Down,
                        keycode_left: HidKeyCode::Left,
                        keycode_right: HidKeyCode::Right,
                    }),
                });
            }
            _ => {}
        }
    }
}
