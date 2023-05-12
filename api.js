import * as api from "@replit/protocol"; // @replit/protocol protobufjs buffer crc-32
import protobufjs from "protobufjs";
import buffer from "buffer";
import CRC32 from "crc-32"
// import { TextEncoder, TextDecoder} from "text-encoding"
globalThis.api = api.replit.goval.api;
globalThis.Buffer = buffer.Buffer;
globalThis.protobufjs = protobufjs;
globalThis.CRC32 = CRC32
// globalThis.TextDecoder = TextDecoder;
// globalThis.TextEncoder = TextEncoder;