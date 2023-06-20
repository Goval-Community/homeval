import { replit } from "@replit/protocol";

type DotReplit = {
    run?: Exec,
    language?: string,
    entrypoint?: string,
    languages?: { [id: string]: DotReplitLanguage },
    hidden?: string[]
}

type DotReplitLanguage = {
    pattern?: string,
    syntax?: string,
    languageServer: LanguageServerConfig,
}

type LanguageServerConfig = {
    start?: Exec,
    configurationJson?: string,
    initializationOptionsJson?: string,
}

type ExecLifecycle = "NonBlocking" | "Stdin" | "Blocking";

type Exec = {
    args?: string[],
    env?: { [id: string]: string },
    blocking?: boolean,
    // TODO: confirm if this is actually how it is returned
    lifecycle?: ExecLifecycle,
    split_stderr?: boolean,
    split_logs?: boolean,
}

type ReplspaceMessage = {
    githubTokenReq?: string,
    openFileReq?: [string, boolean, string],
    openMultipleFiles?: [string[], string],

    githubTokenRes?: string,
    openFileRes?: {},
}

type DatabaseFile = {
    name: string,
    crc32: number,
    contents: string,
    history: string[],
}

declare global {
    // [goval::generated::globals] (generated on the fly)

    let serviceInfo: {
        id: number,
        service: string,
        name: string | null,
    }

    // [goval::api.js] (api.js)

    let api: typeof replit.goval.api;
    let Buffer: typeof import("buffer").Buffer;
    let protobufjs: typeof import("protobufjs");
    let CRC32: typeof import("crc-32");

    // [goval::runtime.js] (src/runtime.js)

    namespace fs {
        function stat(path: string): Promise<{
            exists: boolean,
            type: "file" | "directory" | "symlink",
            size: number,
            fileMode: string,
            modTime: number,
        }>;
        function readDir(path: string): Promise<{
            path: string,
            type: "file" | "directory" | "symlink"
        }[]>;
        function makeDir(path: string): Promise<null>;
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
        clients: number[]
        _online: boolean

        constructor(id: number, service: string, name: string | null)

        stop(): Promise<null>
        start(): Promise<null>
        ipc_recv(): Promise<null>

        _recv(message: { ipc: { bytes: number[], session: number } }): Promise<null>
        recv(command: replit.goval.api.Command, session: number): Promise<replit.goval.api.Command | null>

        _send(cmd: replit.goval.api.Command, session: number): Promise<null>
        send(cmd: replit.goval.api.Command, session: number): Promise<null>

        _attach(session: number): Promise<null>
        attach(session: number): Promise<null>

        _detach(session: number, forced: boolean): Promise<null>
        detach(session: number, forced: boolean): Promise<null>

        process_died(proc_id: number, exit_code: number): Promise<null>

        on_replspace(session: number, msg: ReplspaceMessage): Promise<null>
        replspace_reply(nonce: string, message: ReplspaceMessage): Promise<null>
    }

    class Process {
        channel: number
        id: number
        command: string
        args: string[]

        constructor(channel: number, command: string, args: string[], env_vars: { [id: string]: string })

        init(sessions: number[] | undefined): Promise<null>
        destroy(): Promise<null>
        add_session(session: number): Promise<null>
        remove_session(session: number): Promise<null>
        write(input: string): Promise<null>
        _await_pty_exists(): Promise<null>
    }

    class PtyProcess extends Process { }

    type FileEvent = {
        remove?: string,
        create?: String,
        modify?: String,
        rename?: [string, string],
        err?: string
    };
    class FileWatcher {
        id: number
        online: boolean
        watched_files: number
        listeners: ((event: FileEvent) => Promise<null>)[]

        constructor()

        init(): Promise<null>

        watch(paths: string[]): Promise<null>

        add_listener(listener: (event: FileEvent) => Promise<null>): null

        stop(): Promise<null>
        start(): Promise<null>

        _await_watcher_exists(): Promise<null>
    }

    namespace process {
        namespace system {
            function cpuTime(): Promise<number>;
            function memoryUsage(): Promise<{
                total: number,
                free: number
            }>;
            function diskUsage(): Promise<{
                available: number,
                total: number,
                free: number
            }>;
            let os: string;
        }

        namespace database {
            let _supported: boolean;
            const supported: boolean;

            function getFile(name: string): Promise<DatabaseFile>;
            function setFile(file_model: DatabaseFile): Promise<null>;
        }

        namespace server {
            function name(): string;
            function version(): string;
            function license(): string;
            function repository(): string;
            function description(): string;
            function services(): string[];
            function authors(): string[];
            function uptime(): number;
        }

        let env: { [id: string]: string | null }

        function getUserInfo(session: number): Promise<{ username: string, id: number }>
        function getDotreplitConfig(): DotReplit

        function quickCommand(args: string[], channel: number, sessions: number[], env: { [id: string]: string }): Promise<number>
    }

    function diffText(old_text: string, new_text: string): Promise<{
        insert?: string,
        delete?: number,
        skip?: number,
    }[]>
}