import { replit } from "@replit/protocol";
declare global {
    // [goval::generated::globals] (generated on the fly)

    var serviceInfo: {
        id: number,
        service: string,
        name: string | null,
    }
    
    // [goval::api.js] (api.js)

    var api: typeof replit.goval.api;
    var Buffer: typeof import("buffer").Buffer;
    var protobufjs: typeof import("protobufjs");
    var CRC32: typeof import("crc-32");

    // [goval::runtime.js] (src/runtime.js)
    
    namespace fs {
        function readDir(path: string): Promise<{
            path: string,
            type: "file" | "directory" | "symlink"
        }[]>;
        function writeFile(path: string, contents: number[]): Promise<null>;
        function writeFileString(path: string, contents: string): Promise<null>;
        function readFile(path: string): Promise<number[]>;
        function readFileString(path: string): Promise<string>;
        function remove(path: string): Promise<null>;
        function rename(oldPath: string, newPath: string): Promise<null>;
    }

    // @ts-ignore
    namespace Date {
    function now(): BigInt;
    }

    class ServiceBase {
        id: number
        name: string
        service: string
        clients: string[]

        constructor(id: number, service: string, name: string | null)

        start(): Promise<null>
        ipc_recv(): Promise<null>

        _recv(message: {ipc: {bytes: number[], session: number}}): Promise<null>
        recv(command: replit.goval.api.Command, session: number): Promise<replit.goval.api.Command | null>

        _send(cmd: replit.goval.api.Command, session: number): Promise<null>
        send(cmd: replit.goval.api.Command, session: number): Promise<null>

        _attach(session: number): Promise<null>
        attach(session: number): Promise<null>

        _detach(session: number, forced: boolean): Promise<null>
        detach(session: number, forced: boolean): Promise<null>
    }

    class PtyProcess {
        channel: number
        id: number
        command: string
        args: string[]

        constructor(channel: number, command: string, args: string[])
        
        init(): Promise<null>
        add_session(session: number): Promise<null>
        remove_session(session: number): Promise<null>
        write(input: string): Promise<null>
        _await_pty_exists(): Promise<null>
    }

    namespace process {
        var env: { [id: string]: string | null }

        function getUserInfo(session: number): { username: string, id: number }
    }
}