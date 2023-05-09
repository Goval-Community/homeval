import * as api from "@replit/protocol";
import protobufjs from "protobufjs";
import buffer from "buffer";
// import { TextEncoder, TextDecoder} from "text-encoding"
globalThis.api = api.replit.goval.api;
globalThis.Buffer = buffer.Buffer;
globalThis.protobufjs = protobufjs;
// globalThis.TextDecoder = TextDecoder;
// globalThis.TextEncoder = TextEncoder;