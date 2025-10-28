# Layers

RMK uses a layer system similar to [QMK](https://docs.qmk.fm/keymap#keymap-and-layers). When determining the active key, layers are checked from the highest to the lowest.

For example, if you have two layers, 0 and 1, pressing a key will first check its position in layer 1. If the key is not found or is set as transparent, RMK will then check layer 0.

## Default Layer

The default layer is called the "base" layer. Generally, you cannot access any layers below the default layer. By default, layer 0 is set as the default layer, but you can change this using the `DF` key. Please be cautious when changing the default layer: if you do not have a key to revert the default layer on any layer above the new default, you may lose access to the lower layers. In such cases, you will need to use Vial to update your keymap and set another `DF` key on an accessible layer.
