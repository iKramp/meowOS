//!To stay at least slightly compatible, these are taken from linux
//!https://github.com/torvalds/linux/blob/master/include/uapi/linux/input-event-codes.h

/*
 * Event types
 */

pub type EventCode = u16;

pub const EV_TYPE_SYN: EventCode = 0x00;
pub const EV_TYPE_KEY: EventCode = 0x01;
pub const EV_TYPE_REL: EventCode = 0x02;
pub const EV_TYPE_ABS: EventCode = 0x03;
pub const EV_TYPE_MSC: EventCode = 0x04;
pub const EV_TYPE_SW: EventCode = 0x05;
pub const EV_TYPE_LED: EventCode = 0x11;
pub const EV_TYPE_SND: EventCode = 0x12;
pub const EV_TYPE_REP: EventCode = 0x14;
pub const EV_TYPE_FF: EventCode = 0x15;
pub const EV_TYPE_PWR: EventCode = 0x16;
pub const EV_TYPE_FF_STATUS: EventCode = 0x17;
pub const EV_TYPE_MAX: EventCode = 0x1f;
pub const EV_TYPE_CNT: EventCode = EV_TYPE_MAX + 1;

pub const KEY_RESERVED: EventCode = 0;
pub const KEY_ESC: EventCode = 1;
pub const KEY_1: EventCode = 2;
pub const KEY_2: EventCode = 3;
pub const KEY_3: EventCode = 4;
pub const KEY_4: EventCode = 5;
pub const KEY_5: EventCode = 6;
pub const KEY_6: EventCode = 7;
pub const KEY_7: EventCode = 8;
pub const KEY_8: EventCode = 9;
pub const KEY_9: EventCode = 10;
pub const KEY_0: EventCode = 11;
pub const KEY_MINUS: EventCode = 12;
pub const KEY_EQUAL: EventCode = 13;
pub const KEY_BACKSPACE: EventCode = 14;
pub const KEY_TAB: EventCode = 15;
pub const KEY_Q: EventCode = 16;
pub const KEY_W: EventCode = 17;
pub const KEY_E: EventCode = 18;
pub const KEY_R: EventCode = 19;
pub const KEY_T: EventCode = 20;
pub const KEY_Y: EventCode = 21;
pub const KEY_U: EventCode = 22;
pub const KEY_I: EventCode = 23;
pub const KEY_O: EventCode = 24;
pub const KEY_P: EventCode = 25;
pub const KEY_LEFTBRACE: EventCode = 26;
pub const KEY_RIGHTBRACE: EventCode = 27;
pub const KEY_ENTER: EventCode = 28;
pub const KEY_LEFTCTRL: EventCode = 29;
pub const KEY_A: EventCode = 30;
pub const KEY_S: EventCode = 31;
pub const KEY_D: EventCode = 32;
pub const KEY_F: EventCode = 33;
pub const KEY_G: EventCode = 34;
pub const KEY_H: EventCode = 35;
pub const KEY_J: EventCode = 36;
pub const KEY_K: EventCode = 37;
pub const KEY_L: EventCode = 38;
pub const KEY_SEMICOLON: EventCode = 39;
pub const KEY_APOSTROPHE: EventCode = 40;
pub const KEY_GRAVE: EventCode = 41;
pub const KEY_LEFTSHIFT: EventCode = 42;
pub const KEY_BACKSLASH: EventCode = 43;
pub const KEY_Z: EventCode = 44;
pub const KEY_X: EventCode = 45;
pub const KEY_C: EventCode = 46;
pub const KEY_V: EventCode = 47;
pub const KEY_B: EventCode = 48;
pub const KEY_N: EventCode = 49;
pub const KEY_M: EventCode = 50;
pub const KEY_COMMA: EventCode = 51;
pub const KEY_DOT: EventCode = 52;
pub const KEY_SLASH: EventCode = 53;
pub const KEY_RIGHTSHIFT: EventCode = 54;
pub const KEY_KPASTERISK: EventCode = 55;
pub const KEY_LEFTALT: EventCode = 56;
pub const KEY_SPACE: EventCode = 57;
pub const KEY_CAPSLOCK: EventCode = 58;
pub const KEY_F1: EventCode = 59;
pub const KEY_F2: EventCode = 60;
pub const KEY_F3: EventCode = 61;
pub const KEY_F4: EventCode = 62;
pub const KEY_F5: EventCode = 63;
pub const KEY_F6: EventCode = 64;
pub const KEY_F7: EventCode = 65;
pub const KEY_F8: EventCode = 66;
pub const KEY_F9: EventCode = 67;
pub const KEY_F10: EventCode = 68;
pub const KEY_NUMLOCK: EventCode = 69;
pub const KEY_SCROLLLOCK: EventCode = 70;
pub const KEY_KP7: EventCode = 71;
pub const KEY_KP8: EventCode = 72;
pub const KEY_KP9: EventCode = 73;
pub const KEY_KPMINUS: EventCode = 74;
pub const KEY_KP4: EventCode = 75;
pub const KEY_KP5: EventCode = 76;
pub const KEY_KP6: EventCode = 77;
pub const KEY_KPPLUS: EventCode = 78;
pub const KEY_KP1: EventCode = 79;
pub const KEY_KP2: EventCode = 80;
pub const KEY_KP3: EventCode = 81;
pub const KEY_KP0: EventCode = 82;
pub const KEY_KPDOT: EventCode = 83;

pub const KEY_ZENKAKUHANKAKU: EventCode = 85;
pub const KEY_102ND: EventCode = 86;
pub const KEY_F11: EventCode = 87;
pub const KEY_F12: EventCode = 88;
pub const KEY_RO: EventCode = 89;
pub const KEY_KATAKANA: EventCode = 90;
pub const KEY_HIRAGANA: EventCode = 91;
pub const KEY_HENKAN: EventCode = 92;
pub const KEY_KATAKANAHIRAGANA: EventCode = 93;
pub const KEY_MUHENKAN: EventCode = 94;
pub const KEY_KPJPCOMMA: EventCode = 95;
pub const KEY_KPENTER: EventCode = 96;
pub const KEY_RIGHTCTRL: EventCode = 97;
pub const KEY_KPSLASH: EventCode = 98;
pub const KEY_SYSRQ: EventCode = 99;
pub const KEY_RIGHTALT: EventCode = 100;
pub const KEY_LINEFEED: EventCode = 101;
pub const KEY_HOME: EventCode = 102;
pub const KEY_UP: EventCode = 103;
pub const KEY_PAGEUP: EventCode = 104;
pub const KEY_LEFT: EventCode = 105;
pub const KEY_RIGHT: EventCode = 106;
pub const KEY_END: EventCode = 107;
pub const KEY_DOWN: EventCode = 108;
pub const KEY_PAGEDOWN: EventCode = 109;
pub const KEY_INSERT: EventCode = 110;
pub const KEY_DELETE: EventCode = 111;
pub const KEY_MACRO: EventCode = 112;
pub const KEY_MUTE: EventCode = 113;
pub const KEY_VOLUMEDOWN: EventCode = 114;
pub const KEY_VOLUMEUP: EventCode = 115;
pub const KEY_POWER: EventCode = 116; // SC System Power Down
pub const KEY_KPEQUAL: EventCode = 117;
pub const KEY_KPPLUSMINUS: EventCode = 118;
pub const KEY_PAUSE: EventCode = 119;
pub const KEY_SCALE: EventCode = 120; // AL Compiz Scale (Expose)

pub const KEY_KPCOMMA: EventCode = 121;
pub const KEY_HANGEUL: EventCode = 122;
pub const KEY_HANGUEL: EventCode = KEY_HANGEUL;
pub const KEY_HANJA: EventCode = 123;
pub const KEY_YEN: EventCode = 124;
pub const KEY_LEFTMETA: EventCode = 125;
pub const KEY_RIGHTMETA: EventCode = 126;
pub const KEY_COMPOSE: EventCode = 127;

pub const KEY_STOP: EventCode = 128; // AC Stop
pub const KEY_AGAIN: EventCode = 129;
pub const KEY_PROPS: EventCode = 130; // AC Properties
pub const KEY_UNDO: EventCode = 131; // AC Undo
pub const KEY_FRONT: EventCode = 132;
pub const KEY_COPY: EventCode = 133; // AC Copy
pub const KEY_OPEN: EventCode = 134; // AC Open
pub const KEY_PASTE: EventCode = 135; // AC Paste
pub const KEY_FIND: EventCode = 136; // AC Search
pub const KEY_CUT: EventCode = 137; // AC Cut
pub const KEY_HELP: EventCode = 138; // AL Integrated Help Center
pub const KEY_MENU: EventCode = 139; // Menu (show menu)
pub const KEY_CALC: EventCode = 140; // AL Calculator
pub const KEY_SETUP: EventCode = 141;
pub const KEY_SLEEP: EventCode = 142; // SC System Sleep
pub const KEY_WAKEUP: EventCode = 143; // System Wake Up
pub const KEY_FILE: EventCode = 144; // AL Local Machine Browser
pub const KEY_SENDFILE: EventCode = 145;
pub const KEY_DELETEFILE: EventCode = 146;
pub const KEY_XFER: EventCode = 147;
pub const KEY_PROG1: EventCode = 148;
pub const KEY_PROG2: EventCode = 149;
pub const KEY_WWW: EventCode = 150; // AL Internet Browser
pub const KEY_MSDOS: EventCode = 151;
pub const KEY_COFFEE: EventCode = 152; // AL Terminal Lock/Screensaver
pub const KEY_SCREENLOCK: EventCode = KEY_COFFEE;
pub const KEY_ROTATE_DISPLAY: EventCode = 153; // Display orientation for e.g. tablets
pub const KEY_DIRECTION: EventCode = KEY_ROTATE_DISPLAY;
pub const KEY_CYCLEWINDOWS: EventCode = 154;
pub const KEY_MAIL: EventCode = 155;
pub const KEY_BOOKMARKS: EventCode = 156; // AC Bookmarks
pub const KEY_COMPUTER: EventCode = 157;
pub const KEY_BACK: EventCode = 158; // AC Back
pub const KEY_FORWARD: EventCode = 159; // AC Forward
pub const KEY_CLOSECD: EventCode = 160;
pub const KEY_EJECTCD: EventCode = 161;
pub const KEY_EJECTCLOSECD: EventCode = 162;
pub const KEY_NEXTSONG: EventCode = 163;
pub const KEY_PLAYPAUSE: EventCode = 164;
pub const KEY_PREVIOUSSONG: EventCode = 165;
pub const KEY_STOPCD: EventCode = 166;
pub const KEY_RECORD: EventCode = 167;
pub const KEY_REWIND: EventCode = 168;
pub const KEY_PHONE: EventCode = 169; // Media Select Telephone
pub const KEY_ISO: EventCode = 170;
pub const KEY_CONFIG: EventCode = 171; // AL Consumer Control Configuration
pub const KEY_HOMEPAGE: EventCode = 172; // AC Home
pub const KEY_REFRESH: EventCode = 173; // AC Refresh
pub const KEY_EXIT: EventCode = 174; // AC Exit
pub const KEY_MOVE: EventCode = 175;
pub const KEY_EDIT: EventCode = 176;
pub const KEY_SCROLLUP: EventCode = 177;
pub const KEY_SCROLLDOWN: EventCode = 178;
pub const KEY_KPLEFTPAREN: EventCode = 179;
pub const KEY_KPRIGHTPAREN: EventCode = 180;
pub const KEY_NEW: EventCode = 181; // AC New
pub const KEY_REDO: EventCode = 182; // AC Redo/Repeat

pub const KEY_F13: EventCode = 183;
pub const KEY_F14: EventCode = 184;
pub const KEY_F15: EventCode = 185;
pub const KEY_F16: EventCode = 186;
pub const KEY_F17: EventCode = 187;
pub const KEY_F18: EventCode = 188;
pub const KEY_F19: EventCode = 189;
pub const KEY_F20: EventCode = 190;
pub const KEY_F21: EventCode = 191;
pub const KEY_F22: EventCode = 192;
pub const KEY_F23: EventCode = 193;
pub const KEY_F24: EventCode = 194;

pub const KEY_PLAYCD: EventCode = 200;
pub const KEY_PAUSECD: EventCode = 201;
pub const KEY_PROG3: EventCode = 202;
pub const KEY_PROG4: EventCode = 203;
pub const KEY_ALL_APPLICATIONS: EventCode = 204; // AC Desktop Show All Applications
pub const KEY_DASHBOARD: EventCode = KEY_ALL_APPLICATIONS;
pub const KEY_SUSPEND: EventCode = 205;
pub const KEY_CLOSE: EventCode = 206; // AC Close
pub const KEY_PLAY: EventCode = 207;
pub const KEY_FASTFORWARD: EventCode = 208;
pub const KEY_BASSBOOST: EventCode = 209;
pub const KEY_PRINT: EventCode = 210; // AC Print
pub const KEY_HP: EventCode = 211;
pub const KEY_CAMERA: EventCode = 212;
pub const KEY_SOUND: EventCode = 213;
pub const KEY_QUESTION: EventCode = 214;
pub const KEY_EMAIL: EventCode = 215;
pub const KEY_CHAT: EventCode = 216;
pub const KEY_SEARCH: EventCode = 217;
pub const KEY_CONNECT: EventCode = 218;
pub const KEY_FINANCE: EventCode = 219; // AL Checkbook/Finance
pub const KEY_SPORT: EventCode = 220;
pub const KEY_SHOP: EventCode = 221;
pub const KEY_ALTERASE: EventCode = 222;
pub const KEY_CANCEL: EventCode = 223; // AC Cancel
pub const KEY_BRIGHTNESSDOWN: EventCode = 224;
pub const KEY_BRIGHTNESSUP: EventCode = 225;
pub const KEY_MEDIA: EventCode = 226;

pub const KEY_SWITCHVIDEOMODE: EventCode = 227;
pub const KEY_KBDILLUMTOGGLE: EventCode = 228;
pub const KEY_KBDILLUMDOWN: EventCode = 229;
pub const KEY_KBDILLUMUP: EventCode = 230;

pub const KEY_SEND: EventCode = 231;
pub const KEY_REPLY: EventCode = 232;
pub const KEY_FORWARDMAIL: EventCode = 233;
pub const KEY_SAVE: EventCode = 234;
pub const KEY_DOCUMENTS: EventCode = 235;

pub const KEY_BATTERY: EventCode = 236;

pub const KEY_BLUETOOTH: EventCode = 237;
pub const KEY_WLAN: EventCode = 238;
pub const KEY_UWB: EventCode = 239;

pub const KEY_UNKNOWN: EventCode = 240;

pub const KEY_VIDEO_NEXT: EventCode = 241;
pub const KEY_VIDEO_PREV: EventCode = 242;
pub const KEY_BRIGHTNESS_CYCLE: EventCode = 243;
pub const KEY_BRIGHTNESS_AUTO: EventCode = 244;
pub const KEY_BRIGHTNESS_ZERO: EventCode = KEY_BRIGHTNESS_AUTO;
pub const KEY_DISPLAY_OFF: EventCode = 245;

pub const KEY_WWAN: EventCode = 246;
pub const KEY_WIMAX: EventCode = KEY_WWAN;
pub const KEY_RFKILL: EventCode = 247;

pub const KEY_MICMUTE: EventCode = 248;

pub const BTN_MISC: EventCode = 0x100;
pub const BTN_0: EventCode = 0x100;
pub const BTN_1: EventCode = 0x101;
pub const BTN_2: EventCode = 0x102;
pub const BTN_3: EventCode = 0x103;
pub const BTN_4: EventCode = 0x104;
pub const BTN_5: EventCode = 0x105;
pub const BTN_6: EventCode = 0x106;
pub const BTN_7: EventCode = 0x107;
pub const BTN_8: EventCode = 0x108;
pub const BTN_9: EventCode = 0x109;

pub const BTN_MOUSE: EventCode = 0x110;
pub const BTN_LEFT: EventCode = 0x110;
pub const BTN_RIGHT: EventCode = 0x111;
pub const BTN_MIDDLE: EventCode = 0x112;
pub const BTN_SIDE: EventCode = 0x113;
pub const BTN_EXTRA: EventCode = 0x114;
pub const BTN_FORWARD: EventCode = 0x115;
pub const BTN_BACK: EventCode = 0x116;
pub const BTN_TASK: EventCode = 0x117;

pub const BTN_JOYSTICK: EventCode = 0x120;
pub const BTN_TRIGGER: EventCode = 0x120;
pub const BTN_THUMB: EventCode = 0x121;
pub const BTN_THUMB2: EventCode = 0x122;
pub const BTN_TOP: EventCode = 0x123;
pub const BTN_TOP2: EventCode = 0x124;
pub const BTN_PINKIE: EventCode = 0x125;
pub const BTN_BASE: EventCode = 0x126;
pub const BTN_BASE2: EventCode = 0x127;
pub const BTN_BASE3: EventCode = 0x128;
pub const BTN_BASE4: EventCode = 0x129;
pub const BTN_BASE5: EventCode = 0x12a;
pub const BTN_BASE6: EventCode = 0x12b;
pub const BTN_DEAD: EventCode = 0x12f;

pub const BTN_GAMEPAD: EventCode = 0x130;
pub const BTN_SOUTH: EventCode = 0x130;
pub const BTN_A: EventCode = BTN_SOUTH;
pub const BTN_EAST: EventCode = 0x131;
pub const BTN_B: EventCode = BTN_EAST;
pub const BTN_C: EventCode = 0x132;
pub const BTN_NORTH: EventCode = 0x133;
pub const BTN_X: EventCode = BTN_NORTH;
pub const BTN_WEST: EventCode = 0x134;
pub const BTN_Y: EventCode = BTN_WEST;
pub const BTN_Z: EventCode = 0x135;
pub const BTN_TL: EventCode = 0x136;
pub const BTN_TR: EventCode = 0x137;
pub const BTN_TL2: EventCode = 0x138;
pub const BTN_TR2: EventCode = 0x139;
pub const BTN_SELECT: EventCode = 0x13a;
pub const BTN_START: EventCode = 0x13b;
pub const BTN_MODE: EventCode = 0x13c;
pub const BTN_THUMBL: EventCode = 0x13d;
pub const BTN_THUMBR: EventCode = 0x13e;

pub const BTN_DIGI: EventCode = 0x140;
pub const BTN_TOOL_PEN: EventCode = 0x140;
pub const BTN_TOOL_RUBBER: EventCode = 0x141;
pub const BTN_TOOL_BRUSH: EventCode = 0x142;
pub const BTN_TOOL_PENCIL: EventCode = 0x143;
pub const BTN_TOOL_AIRBRUSH: EventCode = 0x144;
pub const BTN_TOOL_FINGER: EventCode = 0x145;
pub const BTN_TOOL_MOUSE: EventCode = 0x146;
pub const BTN_TOOL_LENS: EventCode = 0x147;
pub const BTN_TOOL_QUINTTAP: EventCode = 0x148;
pub const BTN_STYLUS3: EventCode = 0x149;
pub const BTN_TOUCH: EventCode = 0x14a;
pub const BTN_STYLUS: EventCode = 0x14b;
pub const BTN_STYLUS2: EventCode = 0x14c;
pub const BTN_TOOL_DOUBLETAP: EventCode = 0x14d;
pub const BTN_TOOL_TRIPLETAP: EventCode = 0x14e;
pub const BTN_TOOL_QUADTAP: EventCode = 0x14f;
pub const BTN_WHEEL: EventCode = 0x150;
pub const BTN_GEAR_DOWN: EventCode = 0x150;
pub const BTN_GEAR_UP: EventCode = 0x151;

pub const KEY_OK: EventCode = 0x160;
pub const KEY_SELECT: EventCode = 0x161;
pub const KEY_GOTO: EventCode = 0x162;
pub const KEY_CLEAR: EventCode = 0x163;
pub const KEY_POWER2: EventCode = 0x164;
pub const KEY_OPTION: EventCode = 0x165;
pub const KEY_INFO: EventCode = 0x166;
pub const KEY_TIME: EventCode = 0x167;
pub const KEY_VENDOR: EventCode = 0x168;
pub const KEY_ARCHIVE: EventCode = 0x169;
pub const KEY_PROGRAM: EventCode = 0x16a;
pub const KEY_CHANNEL: EventCode = 0x16b;
pub const KEY_FAVORITES: EventCode = 0x16c;
pub const KEY_EPG: EventCode = 0x16d;
pub const KEY_PVR: EventCode = 0x16e;
pub const KEY_MHP: EventCode = 0x16f;
pub const KEY_LANGUAGE: EventCode = 0x170;
pub const KEY_TITLE: EventCode = 0x171;
pub const KEY_SUBTITLE: EventCode = 0x172;
pub const KEY_ANGLE: EventCode = 0x173;
pub const KEY_FULL_SCREEN: EventCode = 0x174;
pub const KEY_ZOOM: EventCode = KEY_FULL_SCREEN;
pub const KEY_MODE: EventCode = 0x175;
pub const KEY_KEYBOARD: EventCode = 0x176;
pub const KEY_ASPECT_RATIO: EventCode = 0x177;
pub const KEY_SCREEN: EventCode = KEY_ASPECT_RATIO;
pub const KEY_PC: EventCode = 0x178;
pub const KEY_TV: EventCode = 0x179;
pub const KEY_TV2: EventCode = 0x17a;
pub const KEY_VCR: EventCode = 0x17b;
pub const KEY_VCR2: EventCode = 0x17c;
pub const KEY_SAT: EventCode = 0x17d;
pub const KEY_SAT2: EventCode = 0x17e;
pub const KEY_CD: EventCode = 0x17f;
pub const KEY_TAPE: EventCode = 0x180;
pub const KEY_RADIO: EventCode = 0x181;
pub const KEY_TUNER: EventCode = 0x182;
pub const KEY_PLAYER: EventCode = 0x183;
pub const KEY_TEXT: EventCode = 0x184;
pub const KEY_DVD: EventCode = 0x185;
pub const KEY_AUX: EventCode = 0x186;
pub const KEY_MP3: EventCode = 0x187;
pub const KEY_AUDIO: EventCode = 0x188;
pub const KEY_VIDEO: EventCode = 0x189;
pub const KEY_DIRECTORY: EventCode = 0x18a;
pub const KEY_LIST: EventCode = 0x18b;
pub const KEY_MEMO: EventCode = 0x18c;
pub const KEY_CALENDAR: EventCode = 0x18d;
pub const KEY_RED: EventCode = 0x18e;
pub const KEY_GREEN: EventCode = 0x18f;
pub const KEY_YELLOW: EventCode = 0x190;
pub const KEY_BLUE: EventCode = 0x191;
pub const KEY_CHANNELUP: EventCode = 0x192;
pub const KEY_CHANNELDOWN: EventCode = 0x193;
pub const KEY_FIRST: EventCode = 0x194;
pub const KEY_LAST: EventCode = 0x195;
pub const KEY_AB: EventCode = 0x196;
pub const KEY_NEXT: EventCode = 0x197;
pub const KEY_RESTART: EventCode = 0x198;
pub const KEY_SLOW: EventCode = 0x199;
pub const KEY_SHUFFLE: EventCode = 0x19a;
pub const KEY_BREAK: EventCode = 0x19b;
pub const KEY_PREVIOUS: EventCode = 0x19c;
pub const KEY_DIGITS: EventCode = 0x19d;
pub const KEY_TEEN: EventCode = 0x19e;
pub const KEY_TWEN: EventCode = 0x19f;
pub const KEY_VIDEOPHONE: EventCode = 0x1a0;
pub const KEY_GAMES: EventCode = 0x1a1;
pub const KEY_ZOOMIN: EventCode = 0x1a2;
pub const KEY_ZOOMOUT: EventCode = 0x1a3;
pub const KEY_ZOOMRESET: EventCode = 0x1a4;
pub const KEY_WORDPROCESSOR: EventCode = 0x1a5;
pub const KEY_EDITOR: EventCode = 0x1a6;
pub const KEY_SPREADSHEET: EventCode = 0x1a7;
pub const KEY_GRAPHICSEDITOR: EventCode = 0x1a8;
pub const KEY_PRESENTATION: EventCode = 0x1a9;
pub const KEY_DATABASE: EventCode = 0x1aa;
pub const KEY_NEWS: EventCode = 0x1ab;
pub const KEY_VOICEMAIL: EventCode = 0x1ac;
pub const KEY_ADDRESSBOOK: EventCode = 0x1ad;
pub const KEY_MESSENGER: EventCode = 0x1ae;
pub const KEY_DISPLAYTOGGLE: EventCode = 0x1af;
pub const KEY_BRIGHTNESS_TOGGLE: EventCode = KEY_DISPLAYTOGGLE;
pub const KEY_SPELLCHECK: EventCode = 0x1b0;
pub const KEY_LOGOFF: EventCode = 0x1b1;

pub const KEY_DOLLAR: EventCode = 0x1b2;
pub const KEY_EURO: EventCode = 0x1b3;

pub const KEY_FRAMEBACK: EventCode = 0x1b4;
pub const KEY_FRAMEFORWARD: EventCode = 0x1b5;
pub const KEY_CONTEXT_MENU: EventCode = 0x1b6;
pub const KEY_MEDIA_REPEAT: EventCode = 0x1b7;
pub const KEY_10CHANNELSUP: EventCode = 0x1b8;
pub const KEY_10CHANNELSDOWN: EventCode = 0x1b9;
pub const KEY_IMAGES: EventCode = 0x1ba;
pub const KEY_NOTIFICATION_CENTER: EventCode = 0x1bc;
pub const KEY_PICKUP_PHONE: EventCode = 0x1bd;
pub const KEY_HANGUP_PHONE: EventCode = 0x1be;
pub const KEY_LINK_PHONE: EventCode = 0x1bf;

pub const KEY_DEL_EOL: EventCode = 0x1c0;
pub const KEY_DEL_EOS: EventCode = 0x1c1;
pub const KEY_INS_LINE: EventCode = 0x1c2;
pub const KEY_DEL_LINE: EventCode = 0x1c3;

pub const KEY_FN: EventCode = 0x1d0;
pub const KEY_FN_ESC: EventCode = 0x1d1;
pub const KEY_FN_F1: EventCode = 0x1d2;
pub const KEY_FN_F2: EventCode = 0x1d3;
pub const KEY_FN_F3: EventCode = 0x1d4;
pub const KEY_FN_F4: EventCode = 0x1d5;
pub const KEY_FN_F5: EventCode = 0x1d6;
pub const KEY_FN_F6: EventCode = 0x1d7;
pub const KEY_FN_F7: EventCode = 0x1d8;
pub const KEY_FN_F8: EventCode = 0x1d9;
pub const KEY_FN_F9: EventCode = 0x1da;
pub const KEY_FN_F10: EventCode = 0x1db;
pub const KEY_FN_F11: EventCode = 0x1dc;
pub const KEY_FN_F12: EventCode = 0x1dd;
pub const KEY_FN_1: EventCode = 0x1de;
pub const KEY_FN_2: EventCode = 0x1df;
pub const KEY_FN_D: EventCode = 0x1e0;
pub const KEY_FN_E: EventCode = 0x1e1;
pub const KEY_FN_F: EventCode = 0x1e2;
pub const KEY_FN_S: EventCode = 0x1e3;
pub const KEY_FN_B: EventCode = 0x1e4;
pub const KEY_FN_RIGHT_SHIFT: EventCode = 0x1e5;

pub const KEY_BRL_DOT1: EventCode = 0x1f1;
pub const KEY_BRL_DOT2: EventCode = 0x1f2;
pub const KEY_BRL_DOT3: EventCode = 0x1f3;
pub const KEY_BRL_DOT4: EventCode = 0x1f4;
pub const KEY_BRL_DOT5: EventCode = 0x1f5;
pub const KEY_BRL_DOT6: EventCode = 0x1f6;
pub const KEY_BRL_DOT7: EventCode = 0x1f7;
pub const KEY_BRL_DOT8: EventCode = 0x1f8;
pub const KEY_BRL_DOT9: EventCode = 0x1f9;
pub const KEY_BRL_DOT10: EventCode = 0x1fa;

pub const KEY_NUMERIC_0: EventCode = 0x200;
pub const KEY_NUMERIC_1: EventCode = 0x201;
pub const KEY_NUMERIC_2: EventCode = 0x202;
pub const KEY_NUMERIC_3: EventCode = 0x203;
pub const KEY_NUMERIC_4: EventCode = 0x204;
pub const KEY_NUMERIC_5: EventCode = 0x205;
pub const KEY_NUMERIC_6: EventCode = 0x206;
pub const KEY_NUMERIC_7: EventCode = 0x207;
pub const KEY_NUMERIC_8: EventCode = 0x208;
pub const KEY_NUMERIC_9: EventCode = 0x209;
pub const KEY_NUMERIC_STAR: EventCode = 0x20a;
pub const KEY_NUMERIC_POUND: EventCode = 0x20b;
pub const KEY_NUMERIC_A: EventCode = 0x20c;
pub const KEY_NUMERIC_B: EventCode = 0x20d;
pub const KEY_NUMERIC_C: EventCode = 0x20e;
pub const KEY_NUMERIC_D: EventCode = 0x20f;

pub const KEY_CAMERA_FOCUS: EventCode = 0x210;
pub const KEY_WPS_BUTTON: EventCode = 0x211;

pub const KEY_TOUCHPAD_TOGGLE: EventCode = 0x212;
pub const KEY_TOUCHPAD_ON: EventCode = 0x213;
pub const KEY_TOUCHPAD_OFF: EventCode = 0x214;

pub const KEY_CAMERA_ZOOMIN: EventCode = 0x215;
pub const KEY_CAMERA_ZOOMOUT: EventCode = 0x216;
pub const KEY_CAMERA_UP: EventCode = 0x217;
pub const KEY_CAMERA_DOWN: EventCode = 0x218;
pub const KEY_CAMERA_LEFT: EventCode = 0x219;
pub const KEY_CAMERA_RIGHT: EventCode = 0x21a;

pub const KEY_ATTENDANT_ON: EventCode = 0x21b;
pub const KEY_ATTENDANT_OFF: EventCode = 0x21c;
pub const KEY_ATTENDANT_TOGGLE: EventCode = 0x21d;
pub const KEY_LIGHTS_TOGGLE: EventCode = 0x21e;

pub const BTN_DPAD_UP: EventCode = 0x220;
pub const BTN_DPAD_DOWN: EventCode = 0x221;
pub const BTN_DPAD_LEFT: EventCode = 0x222;
pub const BTN_DPAD_RIGHT: EventCode = 0x223;

pub const BTN_GRIPL: EventCode = 0x224;
pub const BTN_GRIPR: EventCode = 0x225;
pub const BTN_GRIPL2: EventCode = 0x226;
pub const BTN_GRIPR2: EventCode = 0x227;

pub const KEY_ALS_TOGGLE: EventCode = 0x230;
pub const KEY_ROTATE_LOCK_TOGGLE: EventCode = 0x231;
pub const KEY_REFRESH_RATE_TOGGLE: EventCode = 0x232;

pub const KEY_BUTTONCONFIG: EventCode = 0x240;
pub const KEY_TASKMANAGER: EventCode = 0x241;
pub const KEY_JOURNAL: EventCode = 0x242;
pub const KEY_CONTROLPANEL: EventCode = 0x243;
pub const KEY_APPSELECT: EventCode = 0x244;
pub const KEY_SCREENSAVER: EventCode = 0x245;
pub const KEY_VOICECOMMAND: EventCode = 0x246;
pub const KEY_ASSISTANT: EventCode = 0x247;
pub const KEY_KBD_LAYOUT_NEXT: EventCode = 0x248;
pub const KEY_EMOJI_PICKER: EventCode = 0x249;
pub const KEY_DICTATE: EventCode = 0x24a;
pub const KEY_CAMERA_ACCESS_ENABLE: EventCode = 0x24b;
pub const KEY_CAMERA_ACCESS_DISABLE: EventCode = 0x24c;
pub const KEY_CAMERA_ACCESS_TOGGLE: EventCode = 0x24d;
pub const KEY_ACCESSIBILITY: EventCode = 0x24e;
pub const KEY_DO_NOT_DISTURB: EventCode = 0x24f;

pub const KEY_BRIGHTNESS_MIN: EventCode = 0x250;
pub const KEY_BRIGHTNESS_MAX: EventCode = 0x251;

/*
 * Keycodes for hotkeys toggling the electronic privacy screen found on some
 * laptops on/off. Note when the embedded-controller turns on/off the eprivacy
 * screen itself then the state should be reported through drm connecter props:
 * https://www.kernel.org/doc/html/latest/gpu/drm-kms.html#standard-connector-properties
 * Except when implementing the drm connecter properties API is not possible
 * because e.g. the firmware does not allow querying the presence and/or status
 * of the eprivacy screen at boot.
 */
pub const KEY_EPRIVACY_SCREEN_ON: EventCode = 0x252;
pub const KEY_EPRIVACY_SCREEN_OFF: EventCode = 0x253;

pub const KEY_ACTION_ON_SELECTION: EventCode = 0x254;
pub const KEY_CONTEXTUAL_INSERT: EventCode = 0x255;
pub const KEY_CONTEXTUAL_QUERY: EventCode = 0x256;

pub const KEY_KBDINPUTASSIST_PREV: EventCode = 0x260;
pub const KEY_KBDINPUTASSIST_NEXT: EventCode = 0x261;
pub const KEY_KBDINPUTASSIST_PREVGROUP: EventCode = 0x262;
pub const KEY_KBDINPUTASSIST_NEXTGROUP: EventCode = 0x263;
pub const KEY_KBDINPUTASSIST_ACCEPT: EventCode = 0x264;
pub const KEY_KBDINPUTASSIST_CANCEL: EventCode = 0x265;

/* Diagonal movement keys */
pub const KEY_RIGHT_UP: EventCode = 0x266;
pub const KEY_RIGHT_DOWN: EventCode = 0x267;
pub const KEY_LEFT_UP: EventCode = 0x268;
pub const KEY_LEFT_DOWN: EventCode = 0x269;

pub const KEY_ROOT_MENU: EventCode = 0x26a;
/* Show Top Menu of the Media (e.g. DVD) */
pub const KEY_MEDIA_TOP_MENU: EventCode = 0x26b;
pub const KEY_NUMERIC_11: EventCode = 0x26c;
pub const KEY_NUMERIC_12: EventCode = 0x26d;
/*
 * Toggle Audio Description: refers to an audio service that helps blind and
 * visually impaired consumers understand the action in a program. Note: in
 * some countries this is referred to as "Video Description".
 */
pub const KEY_AUDIO_DESC: EventCode = 0x26e;
pub const KEY_3D_MODE: EventCode = 0x26f;
pub const KEY_NEXT_FAVORITE: EventCode = 0x270;
pub const KEY_STOP_RECORD: EventCode = 0x271;
pub const KEY_PAUSE_RECORD: EventCode = 0x272;
pub const KEY_VOD: EventCode = 0x273;
pub const KEY_UNMUTE: EventCode = 0x274;
pub const KEY_FASTREVERSE: EventCode = 0x275;
pub const KEY_SLOWREVERSE: EventCode = 0x276;
/*
 * Control a data application associated with the currently viewed channel,
 * e.g. teletext or data broadcast application (MHEG, MHP, HbbTV, etc.)
 */
pub const KEY_DATA: EventCode = 0x277;
pub const KEY_ONSCREEN_KEYBOARD: EventCode = 0x278;
/* Electronic privacy screen control */
pub const KEY_PRIVACY_SCREEN_TOGGLE: EventCode = 0x279;

/* Select an area of screen to be copied */
pub const KEY_SELECTIVE_SCREENSHOT: EventCode = 0x27a;

/* Move the focus to the next or previous user controllable element within a UI container */
pub const KEY_NEXT_ELEMENT: EventCode = 0x27b;
pub const KEY_PREVIOUS_ELEMENT: EventCode = 0x27c;

/* Toggle Autopilot engagement */
pub const KEY_AUTOPILOT_ENGAGE_TOGGLE: EventCode = 0x27d;

/* Shortcut Keys */
pub const KEY_MARK_WAYPOINT: EventCode = 0x27e;
pub const KEY_SOS: EventCode = 0x27f;
pub const KEY_NAV_CHART: EventCode = 0x280;
pub const KEY_FISHING_CHART: EventCode = 0x281;
pub const KEY_SINGLE_RANGE_RADAR: EventCode = 0x282;
pub const KEY_DUAL_RANGE_RADAR: EventCode = 0x283;
pub const KEY_RADAR_OVERLAY: EventCode = 0x284;
pub const KEY_TRADITIONAL_SONAR: EventCode = 0x285;
pub const KEY_CLEARVU_SONAR: EventCode = 0x286;
pub const KEY_SIDEVU_SONAR: EventCode = 0x287;
pub const KEY_NAV_INFO: EventCode = 0x288;
pub const KEY_BRIGHTNESS_MENU: EventCode = 0x289;

/*
 * Some keyboards have keys which do not have a defined meaning, these keys
 * are intended to be programmed / bound to macros by the user. For most
 * keyboards with these macro-keys the key-sequence to inject, or action to
 * take, is all handled by software on the host side. So from the kernel's
 * point of view these are just normal keys.
 *
 * The KEY_MACRO# codes below are intended for such keys, which may be labeled
 * e.g. G1-G18, or S1 - S30. The KEY_MACRO# codes MUST NOT be used for keys
 * where the marking on the key does indicate a defined meaning / purpose.
 *
 * The KEY_MACRO# codes MUST also NOT be used as fallback for when no existing
 * KEY_FOO define matches the marking / purpose. In this case a new KEY_FOO
 * define MUST be added.
 */
pub const KEY_MACRO1: EventCode = 0x290;
pub const KEY_MACRO2: EventCode = 0x291;
pub const KEY_MACRO3: EventCode = 0x292;
pub const KEY_MACRO4: EventCode = 0x293;
pub const KEY_MACRO5: EventCode = 0x294;
pub const KEY_MACRO6: EventCode = 0x295;
pub const KEY_MACRO7: EventCode = 0x296;
pub const KEY_MACRO8: EventCode = 0x297;
pub const KEY_MACRO9: EventCode = 0x298;
pub const KEY_MACRO10: EventCode = 0x299;
pub const KEY_MACRO11: EventCode = 0x29a;
pub const KEY_MACRO12: EventCode = 0x29b;
pub const KEY_MACRO13: EventCode = 0x29c;
pub const KEY_MACRO14: EventCode = 0x29d;
pub const KEY_MACRO15: EventCode = 0x29e;
pub const KEY_MACRO16: EventCode = 0x29f;
pub const KEY_MACRO17: EventCode = 0x2a0;
pub const KEY_MACRO18: EventCode = 0x2a1;
pub const KEY_MACRO19: EventCode = 0x2a2;
pub const KEY_MACRO20: EventCode = 0x2a3;
pub const KEY_MACRO21: EventCode = 0x2a4;
pub const KEY_MACRO22: EventCode = 0x2a5;
pub const KEY_MACRO23: EventCode = 0x2a6;
pub const KEY_MACRO24: EventCode = 0x2a7;
pub const KEY_MACRO25: EventCode = 0x2a8;
pub const KEY_MACRO26: EventCode = 0x2a9;
pub const KEY_MACRO27: EventCode = 0x2aa;
pub const KEY_MACRO28: EventCode = 0x2ab;
pub const KEY_MACRO29: EventCode = 0x2ac;
pub const KEY_MACRO30: EventCode = 0x2ad;

/*
 * Some keyboards with the macro-keys described above have some extra keys
 * for controlling the host-side software responsible for the macro handling:
 * -A macro recording start/stop key. Note that not all keyboards which emit
 *  KEY_MACRO_RECORD_START will also emit KEY_MACRO_RECORD_STOP if
 *  KEY_MACRO_RECORD_STOP is not advertised, then KEY_MACRO_RECORD_START
 *  should be interpreted as a recording start/stop toggle;
 * -Keys for switching between different macro (pre)sets, either a key for
 *  cycling through the configured presets or keys to directly select a preset.
 */
pub const KEY_MACRO_RECORD_START: EventCode = 0x2b0;
pub const KEY_MACRO_RECORD_STOP: EventCode = 0x2b1;
pub const KEY_MACRO_PRESET_CYCLE: EventCode = 0x2b2;
pub const KEY_MACRO_PRESET1: EventCode = 0x2b3;
pub const KEY_MACRO_PRESET2: EventCode = 0x2b4;
pub const KEY_MACRO_PRESET3: EventCode = 0x2b5;

/*
 * Some keyboards have a buildin LCD panel where the contents are controlled
 * by the host. Often these have a number of keys directly below the LCD
 * intended for controlling a menu shown on the LCD. These keys often don't
 * have any labeling so we just name them KEY_KBD_LCD_MENU#
 */
pub const KEY_KBD_LCD_MENU1: EventCode = 0x2b8;
pub const KEY_KBD_LCD_MENU2: EventCode = 0x2b9;
pub const KEY_KBD_LCD_MENU3: EventCode = 0x2ba;
pub const KEY_KBD_LCD_MENU4: EventCode = 0x2bb;
pub const KEY_KBD_LCD_MENU5: EventCode = 0x2bc;

/* Performance Boost key (Alienware)/G-Mode key (Dell) */
pub const KEY_PERFORMANCE: EventCode = 0x2bd;

pub const BTN_TRIGGER_HAPPY: EventCode = 0x2c0;
pub const BTN_TRIGGER_HAPPY1: EventCode = 0x2c0;
pub const BTN_TRIGGER_HAPPY2: EventCode = 0x2c1;
pub const BTN_TRIGGER_HAPPY3: EventCode = 0x2c2;
pub const BTN_TRIGGER_HAPPY4: EventCode = 0x2c3;
pub const BTN_TRIGGER_HAPPY5: EventCode = 0x2c4;
pub const BTN_TRIGGER_HAPPY6: EventCode = 0x2c5;
pub const BTN_TRIGGER_HAPPY7: EventCode = 0x2c6;
pub const BTN_TRIGGER_HAPPY8: EventCode = 0x2c7;
pub const BTN_TRIGGER_HAPPY9: EventCode = 0x2c8;
pub const BTN_TRIGGER_HAPPY10: EventCode = 0x2c9;
pub const BTN_TRIGGER_HAPPY11: EventCode = 0x2ca;
pub const BTN_TRIGGER_HAPPY12: EventCode = 0x2cb;
pub const BTN_TRIGGER_HAPPY13: EventCode = 0x2cc;
pub const BTN_TRIGGER_HAPPY14: EventCode = 0x2cd;
pub const BTN_TRIGGER_HAPPY15: EventCode = 0x2ce;
pub const BTN_TRIGGER_HAPPY16: EventCode = 0x2cf;
pub const BTN_TRIGGER_HAPPY17: EventCode = 0x2d0;
pub const BTN_TRIGGER_HAPPY18: EventCode = 0x2d1;
pub const BTN_TRIGGER_HAPPY19: EventCode = 0x2d2;
pub const BTN_TRIGGER_HAPPY20: EventCode = 0x2d3;
pub const BTN_TRIGGER_HAPPY21: EventCode = 0x2d4;
pub const BTN_TRIGGER_HAPPY22: EventCode = 0x2d5;
pub const BTN_TRIGGER_HAPPY23: EventCode = 0x2d6;
pub const BTN_TRIGGER_HAPPY24: EventCode = 0x2d7;
pub const BTN_TRIGGER_HAPPY25: EventCode = 0x2d8;
pub const BTN_TRIGGER_HAPPY26: EventCode = 0x2d9;
pub const BTN_TRIGGER_HAPPY27: EventCode = 0x2da;
pub const BTN_TRIGGER_HAPPY28: EventCode = 0x2db;
pub const BTN_TRIGGER_HAPPY29: EventCode = 0x2dc;
pub const BTN_TRIGGER_HAPPY30: EventCode = 0x2dd;
pub const BTN_TRIGGER_HAPPY31: EventCode = 0x2de;
pub const BTN_TRIGGER_HAPPY32: EventCode = 0x2df;
pub const BTN_TRIGGER_HAPPY33: EventCode = 0x2e0;
pub const BTN_TRIGGER_HAPPY34: EventCode = 0x2e1;
pub const BTN_TRIGGER_HAPPY35: EventCode = 0x2e2;
pub const BTN_TRIGGER_HAPPY36: EventCode = 0x2e3;
pub const BTN_TRIGGER_HAPPY37: EventCode = 0x2e4;
pub const BTN_TRIGGER_HAPPY38: EventCode = 0x2e5;
pub const BTN_TRIGGER_HAPPY39: EventCode = 0x2e6;
pub const BTN_TRIGGER_HAPPY40: EventCode = 0x2e7;

/* We avoid low common keys in module aliases so they don't get huge. */
pub const KEY_MIN_INTERESTING: EventCode = KEY_MUTE;
pub const KEY_MAX: EventCode = 0x2ff;
pub const KEY_CNT: EventCode = KEY_MAX + 1;

/*
 * Relative axes
 */

pub const REL_X: EventCode = 0x00;
pub const REL_Y: EventCode = 0x01;
pub const REL_Z: EventCode = 0x02;
pub const REL_RX: EventCode = 0x03;
pub const REL_RY: EventCode = 0x04;
pub const REL_RZ: EventCode = 0x05;
pub const REL_HWHEEL: EventCode = 0x06;
pub const REL_DIAL: EventCode = 0x07;
pub const REL_WHEEL: EventCode = 0x08;
pub const REL_MISC: EventCode = 0x09;
/*
 * 0x0a is reserved and should not be used in input drivers.
 * It was used by HID as REL_MISC+1 and userspace needs to detect if
 * the next REL_* event is correct or is just REL_MISC + n.
 * We define here REL_RESERVED so userspace can rely on it and detect
 * the situation described above.
 */
pub const REL_RESERVED: EventCode = 0x0a;
pub const REL_WHEEL_HI_RES: EventCode = 0x0b;
pub const REL_HWHEEL_HI_RES: EventCode = 0x0c;
pub const REL_MAX: EventCode = 0x0f;
pub const REL_CNT: EventCode = REL_MAX + 1;

/*
 * Absolute axes
 */

pub const ABS_X: EventCode = 0x00;
pub const ABS_Y: EventCode = 0x01;
pub const ABS_Z: EventCode = 0x02;
pub const ABS_RX: EventCode = 0x03;
pub const ABS_RY: EventCode = 0x04;
pub const ABS_RZ: EventCode = 0x05;
pub const ABS_THROTTLE: EventCode = 0x06;
pub const ABS_RUDDER: EventCode = 0x07;
pub const ABS_WHEEL: EventCode = 0x08;
pub const ABS_GAS: EventCode = 0x09;
pub const ABS_BRAKE: EventCode = 0x0a;
pub const ABS_HAT0X: EventCode = 0x10;
pub const ABS_HAT0Y: EventCode = 0x11;
pub const ABS_HAT1X: EventCode = 0x12;
pub const ABS_HAT1Y: EventCode = 0x13;
pub const ABS_HAT2X: EventCode = 0x14;
pub const ABS_HAT2Y: EventCode = 0x15;
pub const ABS_HAT3X: EventCode = 0x16;
pub const ABS_HAT3Y: EventCode = 0x17;
pub const ABS_PRESSURE: EventCode = 0x18;
pub const ABS_DISTANCE: EventCode = 0x19;
pub const ABS_TILT_X: EventCode = 0x1a;
pub const ABS_TILT_Y: EventCode = 0x1b;
pub const ABS_TOOL_WIDTH: EventCode = 0x1c;

pub const ABS_VOLUME: EventCode = 0x20;
pub const ABS_PROFILE: EventCode = 0x21;
pub const ABS_SND_PROFILE: EventCode = 0x22;

pub const ABS_MISC: EventCode = 0x28;

/*
 * 0x2e is reserved and should not be used in input drivers.
 * It was used by HID as ABS_MISC+6 and userspace needs to detect if
 * the next ABS_* event is correct or is just ABS_MISC + n.
 * We define here ABS_RESERVED so userspace can rely on it and detect
 * the situation described above.
 */
pub const ABS_RESERVED: EventCode = 0x2e;

pub const ABS_MT_SLOT: EventCode = 0x2f;
pub const ABS_MT_TOUCH_MAJOR: EventCode = 0x30;
pub const ABS_MT_TOUCH_MINOR: EventCode = 0x31;
pub const ABS_MT_WIDTH_MAJOR: EventCode = 0x32;
pub const ABS_MT_WIDTH_MINOR: EventCode = 0x33;
pub const ABS_MT_ORIENTATION: EventCode = 0x34;
pub const ABS_MT_POSITION_X: EventCode = 0x35;
pub const ABS_MT_POSITION_Y: EventCode = 0x36;
pub const ABS_MT_TOOL_TYPE: EventCode = 0x37;
pub const ABS_MT_BLOB_ID: EventCode = 0x38;
pub const ABS_MT_TRACKING_ID: EventCode = 0x39;
pub const ABS_MT_PRESSURE: EventCode = 0x3a;
pub const ABS_MT_DISTANCE: EventCode = 0x3b;
pub const ABS_MT_TOOL_X: EventCode = 0x3c;
pub const ABS_MT_TOOL_Y: EventCode = 0x3d;

pub const ABS_MAX: EventCode = 0x3f;
pub const ABS_CNT: EventCode = ABS_MAX + 1;

/*
 * Switch events
 */

pub const SW_LID: EventCode = 0x00;
pub const SW_TABLET_MODE: EventCode = 0x01;
pub const SW_HEADPHONE_INSERT: EventCode = 0x02;
pub const SW_RFKILL_ALL: EventCode = 0x03;
pub const SW_RADIO: EventCode = SW_RFKILL_ALL;
pub const SW_MICROPHONE_INSERT: EventCode = 0x04;
pub const SW_DOCK: EventCode = 0x05;
pub const SW_LINEOUT_INSERT: EventCode = 0x06;
pub const SW_JACK_PHYSICAL_INSERT: EventCode = 0x07;
pub const SW_VIDEOOUT_INSERT: EventCode = 0x08;
pub const SW_CAMERA_LENS_COVER: EventCode = 0x09;
pub const SW_KEYPAD_SLIDE: EventCode = 0x0a;
pub const SW_FRONT_PROXIMITY: EventCode = 0x0b;
pub const SW_ROTATE_LOCK: EventCode = 0x0c;
pub const SW_LINEIN_INSERT: EventCode = 0x0d;
pub const SW_MUTE_DEVICE: EventCode = 0x0e;
pub const SW_PEN_INSERTED: EventCode = 0x0f;
pub const SW_MACHINE_COVER: EventCode = 0x10;
pub const SW_USB_INSERT: EventCode = 0x11;
pub const SW_MAX: EventCode = 0x11;
pub const SW_CNT: EventCode = SW_MAX + 1;

/*
 * Misc events
 */

pub const MSC_SERIAL: EventCode = 0x00;
pub const MSC_PULSELED: EventCode = 0x01;
pub const MSC_GESTURE: EventCode = 0x02;
pub const MSC_RAW: EventCode = 0x03;
pub const MSC_SCAN: EventCode = 0x04;
pub const MSC_TIMESTAMP: EventCode = 0x05;
pub const MSC_MAX: EventCode = 0x07;
pub const MSC_CNT: EventCode = MSC_MAX + 1;

/*
 * LEDs
 *
 * Do not add any new LED definitions to the input subsystem. The existing
 * definitions are legacy and grandfathered for backwards compatibility with
 * userspace (via evdev). Any new LEDs should be implemented using the
 * LED subsystem (struct led_classdev). The input core provides a bridge to
 * the LED subsystem in drivers/input/input-leds.c.
 */

pub const LED_NUML: EventCode = 0x00;
pub const LED_CAPSL: EventCode = 0x01;
pub const LED_SCROLLL: EventCode = 0x02;
pub const LED_COMPOSE: EventCode = 0x03;
pub const LED_KANA: EventCode = 0x04;
pub const LED_SLEEP: EventCode = 0x05;
pub const LED_SUSPEND: EventCode = 0x06;
pub const LED_MUTE: EventCode = 0x07;
pub const LED_MISC: EventCode = 0x08;
pub const LED_MAIL: EventCode = 0x09;
pub const LED_CHARGING: EventCode = 0x0a;
pub const LED_MAX: EventCode = 0x0f;
pub const LED_CNT: EventCode = LED_MAX + 1;

/*
 * Autorepeat values
 */

pub const REP_DELAY: EventCode = 0x00;
pub const REP_PERIOD: EventCode = 0x01;
pub const REP_MAX: EventCode = 0x01;
pub const REP_CNT: EventCode = REP_MAX + 1;

/*
 * Sounds
 */

pub const SND_CLICK: EventCode = 0x00;
pub const SND_BELL: EventCode = 0x01;
pub const SND_TONE: EventCode = 0x02;
pub const SND_MAX: EventCode = 0x07;
pub const SND_CNT: EventCode = SND_MAX + 1;

/*
 * ABS_SND_PROFILE values
 */

pub const SND_PROFILE_SILENT: EventCode = 0x00;
pub const SND_PROFILE_VIBRATE: EventCode = 0x01;
pub const SND_PROFILE_RING: EventCode = 0x02;
