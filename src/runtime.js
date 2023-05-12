Deno.core.initializeAsyncOps();
// @ts-nocheck
// rome-ignore lint/suspicious/noShadowRestrictedNames: <explanation>
((globalThis) => {
	const core = Deno.core;

	function argsToMessage(...args) {
		return args.map((arg) => JSON.stringify(arg !== undefined ? arg : null))
			.join(" ");
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

globalThis.fs = {
	readDir: async (path) => {
		return await Deno.core.ops.op_list_dir(path);
	},
	writeFile: async (path, contents = "") => {
		return await Deno.core.ops.op_write_file(path, contents);
	},
	readFile: async (path) => {
		return await Deno.core.ops.op_read_file(path);
	},
	remove: async (path) => {
		return await Deno.core.ops.op_remove_file(path);
	},
	rename: async (oldPath, newPath) => {
		return await Deno.core.ops.op_move_file(oldPath, newPath);
	},
};

globalThis.Date = {
	now: () => {
		return BigInt(Deno.core.ops.op_time_milliseconds())
	}
}

class ServiceBase {
	constructor(id, service, name) {
		this.id = id;
		this.service = service;
		this.name = name;
		this.clients = [];
	}

	async _recv() {
		const message = await Deno.core.ops.op_recv_info(this.id);

		if (message.attach) {
			this._attach(message.attach);
		} else if (message.ipc) {
			const cmd = api.Command.decode(message.ipc.bytes);

			this.recv(cmd, message.session).then(async (res) => {
				if (res) {
					res.ref = cmd.ref;
					await this.send(res, message.ipc.session);
				}
			}).catch(async (err) => {
				const res = api.Command.create({ error: err.toString(), ref: cmd.ref });
				console.error(err.toString());
				await this.send(res, message.ipc.session);
			});
		} else if (message.close) {
			await this._detach(message.close, true)
		} else if (message.detach) {
			await this._detach(message.close, false)
		} else {
			console.error("Unknown IPC message", message)
		}

		await this._recv();
	}

	async recv(_c, _s) {
		throw new Error("Not implemented");
	}

	async send(cmd, session) {
		cmd.channel = this.id;
		cmd.session = session;
		const buf = [...Buffer.from(api.Command.encode(cmd).finish())];
		await Deno.core.ops.op_send_msg({
			bytes: buf,
			session: session,
		});
	}

	async _attach(session) {
		this.clients.push(session);
		await this.attach(session)
	}

	async attach(_) {
	}

	async _detach(session, forced) {
		this.clients = this.clients.filter(item => item !== session)
		await this.detach(session, forced)
	}

	async detach(_session, _forced) {}
}
