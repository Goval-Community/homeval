Deno.core.initializeAsyncOps();
// @ts-nocheck
// rome-ignore lint/suspicious/noShadowRestrictedNames: <explanation>
((globalThis) => {
	const core = Deno.core;

	function argsToMessage(...args) {
		return args.map((arg) => JSON.stringify(arg)).join(" ");
	}

	function makeLog(level) {
		return (...args) => {
			Deno.core.ops.op_console_log(
				level,
				serviceInfo,
				`${argsToMessage(...args)}`,
			);
		};
	}

	globalThis.console = {
		debug: makeLog("debug"),
		log: makeLog("info"),
		info: makeLog("info"),
		warn: makeLog("warn"),
		error: makeLog("error"),
		trace: makeLog("trace"),
	};
})(globalThis);

class ServiceBase {
	constructor(id, service, name) {
		this.id = id;
		this.service = service;
		this.name = name;
		this.clients = [];
	}

	async _recv() {
		const message = await Deno.core.ops.op_recv_info(this.id);
		const cmd = api.Command.decode(Buffer.from(message.contents, "base64"));
		const res = await this.recv(cmd, message.session);
		if (res) {
			res.ref = cmd.ref;
			await this.send(res, message.session);
		}

		await this._recv();
	}

	async recv(_c, _s) {
		throw new Error("Not implemented");
	}

	async send(cmd, session) {
		cmd.channel = this.id;
		cmd.session = session;
		await Deno.core.ops.op_send_msg({
			contents: Buffer.from(api.Command.encode(cmd).finish()).toString(
				"base64",
			),
			session: session,
		});
	}

	attach(session) {
		this.clients.push(session);
	}
}
