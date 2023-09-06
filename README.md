# RMK

Keyboard firmware written in Rust. Tested on STM32H7.

## Compile

```
cargo build
```

### compile and check size
```
cargo size --release
cargo size --profile dev
```

## Flash

Requires `openocd`.

VSCode: Press `F5`, the firmware will be automatically compiled and flashed. A debug session is started after flashing. Check `.vscode/tasks.json` for details.

Or you can do it manually using this command after compile:
```shell
openocd -f openocd.cfg -c "program target/thumbv7em-none-eabihf/debug/rmk-stm32h7 preverify verify reset exit"
``` 

## Roadmap

- [x] basic keyboard: matrix, keycode, usb
- [ ] system/media keys
- [ ] layer
- [ ] macro
- [ ] via/vial support
- [ ] encoder
- [ ] RGB
- [ ] cli