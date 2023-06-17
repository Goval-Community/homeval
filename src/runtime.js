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
	async stat(path) {
		try {
			return await Deno.core.ops.op_stat_file(path)
		} catch(err) {
			// file doesnt exist
			return { exists: false }
		}
	},
	async readDir(path) {
		return await Deno.core.ops.op_list_dir(path);
	},
	async writeFile(path, contents = []) {
		return await Deno.core.ops.op_write_file(path, contents);
	},
	async writeFileString(path, contents = "") {
		return await Deno.core.ops.op_write_file_string(path, contents);
	},
	async readFile(path) {
		return await Deno.core.ops.op_read_file(path);
	},
	async readFileString(path) {
		return await Deno.core.ops.op_read_file_string(path);
	},
	async remove(path) {
		return await Deno.core.ops.op_remove_file(path);
	},
	async rename(oldPath, newPath) {
		return await Deno.core.ops.op_move_file(oldPath, newPath);
	},
};

globalThis.Date = {
	now: () => {
		return BigInt(Deno.core.ops.op_time_milliseconds());
	},
};

class ServiceBase {
	constructor(id, service, name) {
		this.id = id;
		this.service = service;
		this.name = name;
		this.clients = [];
	}

	async start() {
		while (true) {
			await this.ipc_recv();
		}
	}

	async ipc_recv() {
		const message = await Deno.core.ops.op_recv_info(this.id);

		if (message.attach) {
			await this._attach(message.attach);
		} else if (message.ipc) {
			await this._recv(message);
		} else if (message.close) {
			await this._detach(message.close, true);
		} else if (message.detach) {
			await this._detach(message.close, false);
		} else if (message.processDead) {
			await this.process_dead(message.processDead[0], message.processDead[1])
		} else if (message.cmdDead != null) {
			await this.process_dead(-1, message.cmdDead)
		} else if (message.replspace) {
			await this.on_replspace(message.replspace[0], message.replspace[1])
		} else {
			console.error("Unknown IPC message", message);
		}
	}

	async process_dead(proc_id, exit_code) {
		console.warn(`PTY/CMD ${proc_id} died with status ${exit_code} and channel ${this.id} doesn't have a listener`)
	}

	async _recv(message) {
		const cmd = api.Command.decode(message.ipc.bytes);

		let res = null;

		try {
			res = await this.recv(cmd, message.ipc.session);
		} catch (err) {
			res = api.Command.create({ error: err.toString(), ref: cmd.ref });
			console.error(err.toString() + err.stack ? `\n${err.stack}` : "");
		}

		if (res) {
			res.ref = cmd.ref;
			await this.send(res, message.ipc.session);
		}
	}

	async recv(_c, _s) {
		throw new Error("Not implemented");
	}

	async _send(cmd, session) {
		const buf = [...Buffer.from(api.Command.encode(cmd).finish())];
		await Deno.core.ops.op_send_msg({
			bytes: buf,
			session: session,
		});
	}

	async send(cmd, session) {
		cmd.channel = this.id;
		cmd.session = session;

		if (session > 0) {
			await this._send(cmd, session);
		} else if (session === 0) {
			for (let client of this.clients) {
				await this._send(cmd, client);
			}
		} else if (session < 0) {
			const ignore = Math.abs(session);
			for (let client of this.clients) {
				if (client === ignore) {
					continue;
				}
				await this._send(cmd, client);
			}
		}
	}

	async _attach(session) {
		this.clients.push(session);
		await this.attach(session);
	}

	async attach(_) {
	}

	async _detach(session, forced) {
		this.clients = this.clients.filter((item) => item !== session);
		await this.detach(session, forced);
	}

	async detach(_session, _forced) {}

	async on_replspace(_session, _msg) {}

	async replspace_reply(nonce, message) {
		await Deno.core.ops.op_replspace_reply(nonce, message);
	}
}

class PtyProcess {
	constructor(channel, command, args = [], env_vars = {}) {
		this.env_vars = env_vars;
		this.channel = channel;
		this.command = command;
		this.args = args;
		this.id = null;
	}

	async init(sessions = []) {
		this.id = await Deno.core.ops.op_register_pty([this.command, ...this.args], this.channel, sessions, this.env_vars);
	}

	async destroy() {
		await Deno.core.ops.op_destroy_pty(this.id, this.channel);
	}

	async add_session(session) {
		await this._await_pty_exists();
		await Deno.core.ops.op_pty_add_session(this.id, session);
	}

	async remove_session(session) {
		await this._await_pty_exists();
		await Deno.core.ops.op_pty_remove_session(this.id, session);
	}

	async write(input) {
		await this._await_pty_exists();
		await Deno.core.ops.op_pty_write_msg(this.id, input);
	}

	// ensure pty exists, if not wait in a non-blocking manner
	// used by functions that queue inputs instead of erroring
	// when the pty isn't initialized yet
	async _await_pty_exists() {
		// fast path
		if (this.id != null) return;

		let loops = 0;
		let warned = false;

		while (true) {
			if (this.id != null) break;

			await Deno.core.ops.op_sleep(1);
			loops += 1;

			if (loops > 1000 && !warned) {
				warned = true
				console.warn(
					"Pty has waited for more than 1 second to initialize, please check this out",
				);
			}
		}
	}
}

class Process {
	constructor(channel, command, args = [], env_vars = {}) {
		this.env_vars = env_vars;
		this.channel = channel;
		this.command = command;
		this.args = args;
		this.id = null;
	}

	async init(sessions = []) {
		this.id = await Deno.core.ops.op_register_cmd([this.command, ...this.args], this.channel, sessions, this.env_vars);
	}

	async destroy() {
		await Deno.core.ops.op_destroy_cmd(this.id, this.channel);
	}

	async add_session(session) {
		await this._await_cmd_exists();
		await Deno.core.ops.op_cmd_add_session(this.id, session);
	}

	async remove_session(session) {
		await this._await_cmd_exists();
		await Deno.core.ops.op_cmd_remove_session(this.id, session);
	}

	async write(input) {
		await this._await_cmd_exists();
		await Deno.core.ops.op_cmd_write_msg(this.id, input);
	}

	// ensure cmd exists, if not wait in a non-blocking manner
	// used by functions that queue inputs instead of erroring
	// when the cmd isn't initialized yet
	async _await_cmd_exists() {
		// fast path
		if (this.id != null) return;

		let loops = 0;
		let warned = false;

		while (true) {
			if (this.id != null) break;

			await Deno.core.ops.op_sleep(1);
			loops += 1;

			if (loops > 1000 && !warned) {
				warned = true
				console.warn(
					"Cmd has waited for more than 1 second to initialize, please check this out",
				);
			}
		}
	}
}

class FileWatcher {
	constructor() {
		this.listeners = []
		this.watched_files = 0
	}

	async init() {
		this.id = await Deno.core.ops.op_make_filewatcher();
	}

	async watch(paths) {
		this.watched_files += paths.length
		await this._await_watcher_exists()
		await Deno.core.ops.op_watch_files(this.id, paths)
	}

	add_listener(listener) {
		this.listeners.push(listener)
	}

	async start() {
		await this._await_watcher_exists()
		let attempts = 0;
		while (true) {
			const msg = await Deno.core.ops.op_recv_fsevent(this.id)
			if (msg.err) {
				if (attempts === 3) {
					throw new Error("FileWatcher has had 3 consecutive errors")
				}

				console.warn("Got error in FileWatcher, retrying:", msg.err)
				attempts += 1;
				await Deno.core.ops.op_sleep(100);
				continue
			}

			for (let listener of this.listeners) {
				listener(msg)
			}
			attempts = 0;
		}
	}

	async _await_watcher_exists() {
		// fast path
		if (this.id != null) return;

		let loops = 0;
		let warned = false;

		while (true) {
			if (this.id != null) break;

			await Deno.core.ops.op_sleep(1);
			loops += 1;

			if (loops > 1000 && !warned) {
				warned = true
				console.warn(
					"File watcher has waited for more than 1 second to initialize, please check this out",
				);
			}
		}
	}
}

globalThis.process = {
	env: new Proxy({}, {
		get(_target, prop, _recveiver) {
			return Deno.core.ops.op_get_env_var(prop)
		},
		set() {
			throw new Error("Setting env vars is currently unimplemented");
		},
	}),
	system: {
		async cpuTime() {
			return await Deno.core.ops.op_cpu_info()
		},
		async memoryUsage() {
			return await Deno.core.ops.op_memory_info()
		},
		async diskUsage() {
			return await Deno.core.ops.op_disk_info();
		},
		get os() {
			return Deno.core.ops.op_get_running_os();
		}
	},
	database: {
		_supported: null,
		get supported() {
			if (process.database._supported != null) {
				return process.database._supported
			}

			let support = Deno.core.ops.op_database_exists();
			process.database._supported = support;
			return support;
		},
		async getFile(name) {
			if (!process.database.supported) {
				throw new Error("No database support :/")
			}

			return await Deno.core.ops.op_database_get_file(name)
		},
		async setFile(file_model) {
			if (!process.database.supported) {
				throw new Error("No database support :/")
			}

			return await Deno.core.ops.op_database_set_file(file_model)
		}
	},
	server: {
		name() {
			return Deno.core.ops.op_server_name()
		},
		version() {
			return Deno.core.ops.op_server_version()
		},
		license() {
			return Deno.core.ops.op_server_license()
		},
		repository() {
			return Deno.core.ops.op_server_repository()
		},
		description() {
			return Deno.core.ops.op_server_description()
		},
		uptime() {
			return Deno.core.ops.op_server_uptime()
		},
		services() {
			return Deno.core.ops.op_get_supported_services()
		},
		authors() {
			const authors = Deno.core.ops.op_server_authors();
			return authors.split(":")
		}
	},
	async getUserInfo(session) {
		return await Deno.core.ops.op_user_info(session)
	},
	getDotreplitConfig() {
		return Deno.core.ops.op_get_dotreplit_config()
	},

	async quickCommand(args, channel, sessions, env = {}) {
		return await Deno.core.ops.op_run_cmd(args, channel, sessions, env)
	}
};

globalThis.diffText = async (old_text, new_text) => {
	return await Deno.core.ops.op_diff_texts(old_text, new_text)
}