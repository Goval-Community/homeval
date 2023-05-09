import * as api from "@replit/protocol";
import protobufjs from "protobufjs";
import buffer from "buffer";
globalThis.api = api.replit.goval.api;
globalThis.Buffer = buffer.Buffer;
globalThis.protobufjs = protobufjs;
