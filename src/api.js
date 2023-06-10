import * as api from "@replit/protocol";
import protobufjs from "protobufjs";
import buffer from "buffer";
import CRC32 from "crc-32"
import {TextEncoder} from 'fastestsmallesttextencoderdecoder';

globalThis.TextEncoder = TextEncoder
globalThis.api = api.replit.goval.api;
globalThis.Buffer = buffer.Buffer;
globalThis.protobufjs = protobufjs;
globalThis.CRC32 = CRC32
