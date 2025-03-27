#!/usr/bin/env python3
'''Convert a Vial layout to RMK keymap configuration.'''

import json
import re


def text_to_lookup(text):
    result = {}
    for line in text.strip().split('\n'):
        key_list, value = line.split('\t')
        for key in key_list.split(','):
            result[key] = value
    return result


qmk_key_to_rmk = text_to_lookup('''
KC_NO,XXXXXXX	No
KC_TRANSPARENT,KC_TRNS,_______	Transparent
KC_A	A
KC_B	B
KC_C	C
KC_D	D
KC_E	E
KC_F	F
KC_G	G
KC_H	H
KC_I	I
KC_J	J
KC_K	K
KC_L	L
KC_M	M
KC_N	N
KC_O	O
KC_P	P
KC_Q	Q
KC_R	R
KC_S	S
KC_T	T
KC_U	U
KC_V	V
KC_W	W
KC_X	X
KC_Y	Y
KC_Z	Z
KC_1	Kc1
KC_2	Kc2
KC_3	Kc3
KC_4	Kc4
KC_5	Kc5
KC_6	Kc6
KC_7	Kc7
KC_8	Kc8
KC_9	Kc9
KC_0	Kc0
KC_ENTER,KC_ENT	Enter
KC_ESCAPE,KC_ESC	Escape
KC_BACKSPACE,KC_BSPC,KC_BSPACE	Backspace
KC_TAB	Tab
KC_SPACE,KC_SPC	Space
KC_MINUS,KC_MINS	Minus
KC_EQUAL,KC_EQL	Equal
KC_LEFT_BRACKET,KC_LBRC,KC_LBRACKET	LeftBracket
KC_RIGHT_BRACKET,KC_RBRC,KC_RBRACKET	RightBracket
KC_BACKSLASH,KC_BSLS,KC_BSLASH	Backslash
KC_NONUS_HASH,KC_NUHS	NonusHash
KC_SEMICOLON,KC_SCLN,KC_SCOLON	Semicolon
KC_QUOTE,KC_QUOT	Quote
KC_GRAVE,KC_GRV	Grave
KC_COMMA,KC_COMM	Comma
KC_DOT	Dot
KC_SLASH,KC_SLSH	Slash
KC_CAPS_LOCK,KC_CAPSLOCK,KC_CAPS	CapsLock
KC_F1	F1
KC_F2	F2
KC_F3	F3
KC_F4	F4
KC_F5	F5
KC_F6	F6
KC_F7	F7
KC_F8	F8
KC_F9	F9
KC_F10	F10
KC_F11	F11
KC_F12	F12
KC_PRINT_SCREEN,KC_PSCR,KC_PSCREEN	PrintScreen
KC_SCROLL_LOCK,KC_SCRL,KC_BRMD,KC_SCROLLLOCK	ScrollLock
KC_PAUSE,KC_PAUS,KC_BRK,KC_BRMU	Pause
KC_INSERT,KC_INS	Insert
KC_HOME	Home
KC_PAGE_UP,KC_PGUP	PageUp
KC_DELETE,KC_DEL	Delete
KC_END	End
KC_PAGE_DOWN,KC_PGDN,KC_PGDOWN	PageDown
KC_RIGHT,KC_RGHT	Right
KC_LEFT	Left
KC_DOWN	Down
KC_UP	UP
KC_NUM_LOCK,KC_NUM,KC_NUMLOCK	NumLock
KC_KP_SLASH,KC_PSLS	KpSlash
KC_KP_ASTERISK,KC_PAST	KpAsterisk
KC_KP_MINUS,KC_PMNS	KpMinus
KC_KP_PLUS,KC_PPLS	KpPlus
KC_KP_ENTER,KC_PENT	KpEnter
KC_KP_1,KC_P1	Kp1
KC_KP_2,KC_P2	Kp2
KC_KP_3,KC_P3	Kp3
KC_KP_4,KC_P4	Kp4
KC_KP_5,KC_P5	Kp5
KC_KP_6,KC_P6	Kp6
KC_KP_7,KC_P7	Kp7
KC_KP_8,KC_P8	Kp8
KC_KP_9,KC_P9	Kp9
KC_KP_0,KC_P0	Kp0
KC_KP_DOT,KC_PDOT	KpDot
KC_NONUS_BACKSLASH,KC_NUBS	NonusBackslash
KC_APPLICATION,KC_APP	Application
KC_KB_POWER	KbPower
KC_KP_EQUAL,KC_PEQL	KpEqual
KC_F13	F13
KC_F14	F14
KC_F15	F15
KC_F16	F16
KC_F17	F17
KC_F18	F18
KC_F19	F19
KC_F20	F20
KC_F21	F21
KC_F22	F22
KC_F23	F23
KC_F24	F24
KC_EXECUTE,KC_EXEC	Execute
KC_HELP	Help
KC_MENU	Menu
KC_SELECT,KC_SLCT	Select
KC_STOP	Stop
KC_AGAIN,KC_AGIN	Again
KC_UNDO	Undo
KC_CUT	Cut
KC_COPY	Copy
KC_PASTE,KC_PSTE	Paste
KC_FIND	Find
KC_KB_MUTE	Mute
KC_KB_VOLUME_UP	VolumeUp
KC_KB_VOLUME_DOWN	VolumeDown
KC_LOCKING_CAPS_LOCK,KC_LCAP	LockingCapsLock
KC_LOCKING_NUM_LOCK,KC_LNUM	LockingNumLock
KC_LOCKING_SCROLL_LOCK,KC_LSCR	LockingScrollLock
KC_KP_COMMA,KC_PCMM	KpComma
KC_KP_EQUAL_AS400	KpEqualAs400
KC_INTERNATIONAL_1,KC_INT1	International1
KC_INTERNATIONAL_2,KC_INT2	International2
KC_INTERNATIONAL_3,KC_INT3	International3
KC_INTERNATIONAL_4,KC_INT4	International4
KC_INTERNATIONAL_5,KC_INT5	International5
KC_INTERNATIONAL_6,KC_INT6	International6
KC_INTERNATIONAL_7,KC_INT7	International7
KC_INTERNATIONAL_8,KC_INT8	International8
KC_INTERNATIONAL_9,KC_INT9	International9
KC_LANGUAGE_1,KC_LNG1	Language1
KC_LANGUAGE_2,KC_LNG2	Language2
KC_LANGUAGE_3,KC_LNG3	Language3
KC_LANGUAGE_4,KC_LNG4	Language4
KC_LANGUAGE_5,KC_LNG5	Language5
KC_LANGUAGE_6,KC_LNG6	Language6
KC_LANGUAGE_7,KC_LNG7	Language7
KC_LANGUAGE_8,KC_LNG8	Language8
KC_LANGUAGE_9,KC_LNG9	Language9
KC_ALTERNATE_ERASE,KC_ERAS	AlternateErase
KC_SYSTEM_REQUEST,KC_SYRQ	SystemRequest
KC_CANCEL,KC_CNCL	Cancel
KC_CLEAR,KC_CLR	Clear
KC_PRIOR,KC_PRIR	Prior
KC_RETURN,KC_RETN	Return
KC_SEPARATOR,KC_SEPR	Separator
KC_OUT	Out
KC_OPER	Oper
KC_CLEAR_AGAIN,KC_CLAG	ClearAgain
KC_CRSEL,KC_CRSL	Crsel
KC_EXSEL,KC_EXSL	Exsel
KC_LEFT_CTRL,KC_LCTL,KC_LCTRL	LCtrl
KC_LEFT_SHIFT,KC_LSFT,KC_LSHIFT	LShift
KC_LEFT_ALT,KC_LALT,KC_LOPT	LAlt
KC_LEFT_GUI,KC_LGUI,KC_LCMD,KC_LWIN	LGui
KC_RIGHT_CTRL,KC_RCTL,KC_RCTRL	RCtrl
KC_RIGHT_SHIFT,KC_RSFT,KC_RSHIFT	RShift
KC_RIGHT_ALT,KC_RALT,KC_ROPT,KC_ALGR	RAlt
KC_RIGHT_GUI,KC_RGUI,KC_RCMD,KC_RWIN	RGui
KC_SYSTEM_POWER,KC_PWR	SystemPower
KC_SYSTEM_SLEEP,KC_SLEP	SystemSleep
KC_SYSTEM_WAKE,KC_WAKE	SystemWake
KC_AUDIO_MUTE,KC_MUTE	AudioMute
KC_AUDIO_VOL_UP,KC_VOLU	AudioVolUp
KC_AUDIO_VOL_DOWN,KC_VOLD	AudioVolDown
KC_MEDIA_NEXT_TRACK,KC_MNXT	MediaNextTrack
KC_MEDIA_PREV_TRACK,KC_MPRV	MediaPrevTrack
KC_MEDIA_STOP,KC_MSTP	MediaStop
KC_MEDIA_PLAY_PAUSE,KC_MPLY	MediaPlayPause
KC_MEDIA_SELECT,KC_MSEL	MediaSelect
KC_MEDIA_EJECT,KC_EJCT	MediaEject
KC_MAIL	Mail
KC_CALCULATOR,KC_CALC	Calculator
KC_MY_COMPUTER,KC_MYCM	MyComputer
KC_WWW_SEARCH,KC_WSCH	WwwSearch
KC_WWW_HOME,KC_WHOM	WwwHome
KC_WWW_BACK,KC_WBAK	WwwBack
KC_WWW_FORWARD,KC_WFWD	WwwForward
KC_WWW_STOP,KC_WSTP	WwwStop
KC_WWW_REFRESH,KC_WREF	WwwRefresh
KC_WWW_FAVORITES,KC_WFAV	WwwFavorites
KC_MEDIA_FAST_FORWARD,KC_MFFD	MediaFastForward
KC_MEDIA_REWIND,KC_MRWD	MediaRewind
KC_BRIGHTNESS_UP,KC_BRIU	BrightnessUp
KC_BRIGHTNESS_DOWN,KC_BRID	BrightnessDown
KC_CONTROL_PANEL,KC_CPNL	ControlPanel
KC_ASSISTANT,KC_ASST	Assistant
KC_MISSION_CONTROL,KC_MCTL	MissionControl
KC_LAUNCHPAD,KC_LPAD	Launchpad
QK_MOUSE_CURSOR_UP,MS_UP,KC_MS_U	MouseUp
QK_MOUSE_CURSOR_DOWN,MS_DOWN,KC_MS_D	MouseDown
QK_MOUSE_CURSOR_LEFT,MS_LEFT,KC_MS_L	MouseLeft
QK_MOUSE_CURSOR_RIGHT,MS_RGHT,KC_MS_R	MouseRight
QK_MOUSE_BUTTON_1,MS_BTN1,KC_BTN1,M1	MouseBtn1
QK_MOUSE_BUTTON_2,MS_BTN2,KC_BTN2,M2	MouseBtn2
QK_MOUSE_BUTTON_3,MS_BTN3,KC_BTN3,M3	MouseBtn3
QK_MOUSE_BUTTON_4,MS_BTN4,KC_BTN4,M4	MouseBtn4
QK_MOUSE_BUTTON_5,MS_BTN5,KC_BTN5,M5	MouseBtn5
QK_MOUSE_BUTTON_6,MS_BTN6,KC_BTN6,M6	MouseBtn6
QK_MOUSE_BUTTON_7,MS_BTN7,KC_BTN7,M7	MouseBtn7
QK_MOUSE_BUTTON_8,MS_BTN8,KC_BTN8,M8	MouseBtn8
QK_MOUSE_WHEEL_UP,MS_WHLU,KC_WH_U	MouseWheelUp
QK_MOUSE_WHEEL_DOWN,MS_WHLD,KC_WH_D	MouseWheelDown
QK_MOUSE_WHEEL_LEFT,MS_WHLL,KC_WH_L	MouseWheelLeft
QK_MOUSE_WHEEL_RIGHT,MS_WHLR,KC_WH_R	MouseWheelRight
QK_MOUSE_ACCELERATION_0,MS_ACL0,KC_ACL0	MouseAccel0
QK_MOUSE_ACCELERATION_1,MS_ACL1,KC_ACL1	MouseAccel1
QK_MOUSE_ACCELERATION_2,MS_ACL2,KC_ACL2	MouseAccel2
QK_JOYSTICK_BUTTON_0,JS_0	JoystickButton0
QK_JOYSTICK_BUTTON_1,JS_1	JoystickButton1
QK_JOYSTICK_BUTTON_2,JS_2	JoystickButton2
QK_JOYSTICK_BUTTON_3,JS_3	JoystickButton3
QK_JOYSTICK_BUTTON_4,JS_4	JoystickButton4
QK_JOYSTICK_BUTTON_5,JS_5	JoystickButton5
QK_JOYSTICK_BUTTON_6,JS_6	JoystickButton6
QK_JOYSTICK_BUTTON_7,JS_7	JoystickButton7
QK_JOYSTICK_BUTTON_8,JS_8	JoystickButton8
QK_JOYSTICK_BUTTON_9,JS_9	JoystickButton9
QK_JOYSTICK_BUTTON_10,JS_10	JoystickButton10
QK_JOYSTICK_BUTTON_11,JS_11	JoystickButton11
QK_JOYSTICK_BUTTON_12,JS_12	JoystickButton12
QK_JOYSTICK_BUTTON_13,JS_13	JoystickButton13
QK_JOYSTICK_BUTTON_14,JS_14	JoystickButton14
QK_JOYSTICK_BUTTON_15,JS_15	JoystickButton15
QK_JOYSTICK_BUTTON_16,JS_16	JoystickButton16
QK_JOYSTICK_BUTTON_17,JS_17	JoystickButton17
QK_JOYSTICK_BUTTON_18,JS_18	JoystickButton18
QK_JOYSTICK_BUTTON_19,JS_19	JoystickButton19
QK_JOYSTICK_BUTTON_20,JS_20	JoystickButton20
QK_JOYSTICK_BUTTON_21,JS_21	JoystickButton21
QK_JOYSTICK_BUTTON_22,JS_22	JoystickButton22
QK_JOYSTICK_BUTTON_23,JS_23	JoystickButton23
QK_JOYSTICK_BUTTON_24,JS_24	JoystickButton24
QK_JOYSTICK_BUTTON_25,JS_25	JoystickButton25
QK_JOYSTICK_BUTTON_26,JS_26	JoystickButton26
QK_JOYSTICK_BUTTON_27,JS_27	JoystickButton27
QK_JOYSTICK_BUTTON_28,JS_28	JoystickButton28
QK_JOYSTICK_BUTTON_29,JS_29	JoystickButton29
QK_JOYSTICK_BUTTON_30,JS_30	JoystickButton30
QK_JOYSTICK_BUTTON_31,JS_31	JoystickButton31
QK_BOOTLOADER,QK_BOOT,RESET	Bootloader
QK_REBOOT,QK_RBT	Reboot
QK_DEBUG_TOGGLE,DB_TOGG	DebugToggle
QK_CLEAR_EEPROM,EE_CLR	ClearEeprom
KC_MEH	LCtrl | LShift | LAlt
KC_HYPR	LCtrl | LShift | LAlt | LGui
USER0	User0
USER1	User1
USER2	User2
USER3	User3
USER4	User4
USER5	User5
USER6	User6
USER7	User7
USER8	User8
USER9	User9
USER10	User10
USER11	User11
USER12	User12
USER13	User13
USER14	User14
USER15	User15
USER16	User16
USER17	User17
USER18	User18
USER19	User19
USER20	User20
USER21	User21
USER22	User22
USER23	User23
USER24	User24
USER25	User25
USER26	User26
USER27	User27
USER28	User28
USER29	User29
USER30	User30
USER31	User31
''')

identical_special_ops = re.compile(r'^(?:DF|MO|LM|LT|OSL|TT|TG|TO)\(.*\)$')

qmk_mod_to_rmk_text = '''
LCTL,C	LCtrl
LSFT,S	LShift
LALT,A,LOPT	LAlt
LGUI,G,LCMD,LWIN	LGui
RCTL	RCtrl
RSFT	RShift
RALT,ROPT,ALGR	RAlt
RGUI,RCMD,RWIN	RGui
LSG,SGUI,SCMD,SWIN	LShift | LGui
LAG	LAlt | LGui
RSG	RShift | RGui
RAG	RAlt | RGui
LCS	LCtrl | LShift
C_S	LCtrl | LShift
LCA	LCtrl | LAlt
LSA	LShift | LAlt
RSA,SAGR	RShift | RAlt
RCS	RCtrl | RShift
LCAG	LCtrl | LAlt | LGui
MEH	LCtrl | LShift | LAlt
HYPR	LCtrl | LShift | LAlt | LGui
'''
qmk_mod_to_rmk = text_to_lookup(qmk_mod_to_rmk_text)
qmk_key_to_rmk[-1] = 'No'

qmk_mods = '|'.join(
    line.split('\t')[0] for line in qmk_mod_to_rmk_text.split('\n')
).replace(',', '|')
with_modifiers = re.compile(r'^(' + qmk_mods + r')\((.*)\)$')


class Converter:
    def __init__(self, output_rust=False):
        self.output_rust = output_rust

    def get_rmk_keycode(self, qmk_or_vial_keycode):
        if isinstance(qmk_or_vial_keycode, str):
            if identical_special_ops.match(qmk_or_vial_keycode) is not None:
                return qmk_or_vial_keycode
            match = with_modifiers.match(qmk_or_vial_keycode)
            if match is not None:
                modifier, key = match.groups()
                rmk_mod = qmk_mod_to_rmk[modifier]
                return f'WM({self.get_rmk_keycode(key)}, {rmk_mod})'
        return qmk_key_to_rmk[qmk_or_vial_keycode]

    def convert_key(self, key):
        rmk_key = self.get_rmk_keycode(key)
        if self.output_rust:
            if rmk_key == 'No' or rmk_key == 'Transparent':
                return f'a!({rmk_key})'
            else:
                return f'k!({rmk_key})'
        else:
            if rmk_key == 'No':
                return '"No"'
            elif rmk_key == 'Transparent':
                return '"__"'
            else:
                return f'"{rmk_key}"'

    def convert_row(self, row):
        return f'[{', '.join(self.convert_key(key) for key in row)}]'

    def convert_layer(self, layer):
        converted_rows = (self.convert_row(row) for row in layer)
        if self.output_rust:
            return '\n'.join((
                'layer!([',
                '            ' + ',\n            '.join(converted_rows),
                '        ])'
            ))
        else:
            return f'[\n        {',\n        '.join(converted_rows)}\n    ]'

    def convert_parsed_vial_layout(self, vial_layout):
        converted_layers = (self.convert_layer(layer) for layer in vial_layout)
        if self.output_rust:
            return '\n'.join((
                '#[rustfmt::skip]',
                'pub const fn get_default_keymap() -> '
                + '[[[KeyAction; COL]; ROW]; NUM_LAYER] {',
                '    [',
                '        ' + ',\n        '.join(converted_layers),
                '    ]',
                '}',
            ))
        else:
            return f'keymap = [\n    {',\n    '.join(converted_layers)}\n]'

    def convert_vial_layout_file(self, filename):
        with open(filename, 'r') as layout_file:
            parsed = json.load(layout_file)
            return self.convert_parsed_vial_layout(parsed['layout'])


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(
        prog='via2rmk.py',
        description=globals()['__doc__'],
        epilog='''By default this will output a `keymap = [...]` snippet to be
            added to your `keyboard.toml`.'''
    )

    parser.add_argument('filename',
                        help='The Vial layout file (`.vil`) to convert')
    parser.add_argument('-r', '--output-rust', action='store_true',
                        help='output Rust code instead of TOML')

    args = parser.parse_args()

    converter = Converter(output_rust=args.output_rust)
    print(converter.convert_vial_layout_file(args.filename))
