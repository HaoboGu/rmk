# Layers

RMK now supports simple layer switch, which is same with QMK's MO(n). 

## Implementation details

### [Layer Cache](https://github.com/qmk/qmk_firmware/blob/master/quantum/action_layer.c#L299)

When a key is pressed, the current layer is cached. This cached layer is valid until when the key is released.
