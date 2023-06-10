class Service extends ServiceBase {
	constructor(...args) {
		super(...args);
		this.version = 1
		this.contents = ""
		this.path = null
		this.cursors = {}
		this.history = []
		this.session_info = {}

		this.watcher = new FileWatcher();
		this.watcher.add_listener(this.file_event.bind(this));
		this.watcher.init();
		this.watcher.start().catch((err) => {
			throw err;
		});
	}

	async recv(cmd, session) {
		if (!this.path && !cmd.otLinkFile) {
			console.error("Command sent before otLinkFile", cmd)
			return
		}

		if (cmd.otLinkFile) {
			const path = cmd.otLinkFile.file.path;
			const content = await fs.readFile(path);

			this.watcher.watch([path])

			this.path = path
			this.contents = await fs.readFileString(path);

			this.history.push({
				spookyVersion: this.version,
				op: [{insert: this.contents}],
				crc32: CRC32.str(this.contents),
				committed: {
					seconds: (Date.now()/ 1000n).toString(),
					nanos: 0
				},
				version: this.version,
				author: api.OTPacket.Author.USER,
				// use https://replit.com/@homeval for initial insert
				userId: 23352071
			})

			return api.Command.create({otLinkFileResponse:{version:this.version, linkedFile:{path, content}}})
		} else if (cmd.ot) {
			let cursor = 0
			let contents = [...this.contents]

			for (const op of cmd.ot.op) {
				if (op.skip) {
					const skip = op.skip
					if (skip + cursor > contents.length) {
						throw new Error("Invalid skip past bounds")
					}

					cursor += skip
				}
				if (op.insert) {
					const insert = op.insert
					contents = [...contents.slice(0, cursor), ...insert, ...contents.slice(cursor)]
					cursor += insert.length
				}
				if (op.delete) {
					const del = op.delete
					if (del + cursor > contents.length) {
						throw new Error("Invalid delete past bounds")
					}

					contents = [...contents.slice(0,cursor), ...contents.slice(cursor + del)]
				}
			}

			const final_contents = contents.join("")

			this.version += 1
			this.contents = final_contents

			let userId = 0; 
			
			if (cmd.ot.author !== api.OTPacket.Author.GHOSTWRITER) {
				if (this.session_info[session]) {
					userId = this.session_info[session].id
				} else {
					userId = 0
				}
			} else {
				userId = 22261053
			}

			const inner_packet = {
				spookyVersion: this.version,
				op: cmd.ot.op,
				crc32: CRC32.str(final_contents),
				committed: {
					seconds: (Date.now()/ 1000n).toString(),
					nanos: 0
				},
				version: this.version,
				author: cmd.ot.author,
				// use https://replit.com/@ghostwriterai if ghostwriter wrote it (for history)
				userId
			}

			this.history.push(inner_packet)

			const msg = api.Command.create({
				ot: inner_packet
			})

			await this.send(msg, 0)

			await fs.writeFileString(this.path, final_contents)

			return api.Command.create({
				ok: {}
			})
		} else if (cmd.otNewCursor) {
			const cursor = cmd.otNewCursor
			this.cursors[cursor.id] = cursor

			const msg = api.Command.create({ otNewCursor: cursor })

			await this.send(msg, -session)
		} else if (cmd.otDeleteCursor) {
			delete this.cursors[cmd.otDeleteCursor.id]

			const msg = api.Command.create({ otDeleteCursor: cmd.otDeleteCursor })

			await this.send(msg, -session)
		} else if (cmd.otFetchRequest) {
			return api.Command.create({
				otFetchResponse: {
					packets: this.history.slice(
						cmd.otFetchRequest.versionFrom - 1,
						cmd.otFetchRequest.versionTo + 1
					)
				}
			})
		} else if (cmd.flush) {
			return api.Command.create({
				ok: {}
			})
		}else {
			console.warn("Unknown ot command", cmd)
		}
	}

	async file_event(event) {
		if (event.modify === this.path) {
			let diff = await Deno.core.ops.op_diff_texts(this.contents, await fs.readFileString(this.path))
			if (diff.length === 0) {
				return
			}

			await this.recv(api.Command.create({ot:{op: diff}, author: api.OTPacket.Author.USER, userId: 0}), 0)
		} else {
			console.debug(event.modify, this.path, event)
		}
	}

	async attach(session) {
		this.session_info[session] = process.getUserInfo(session)
		
		if (!this.path) {
			await this.send(api.Command.create({otstatus: {}}), session)
			return
		}

		

		await this.send(api.Command.create({
			otstatus:{
				contents:this.contents, 
				version: this.version, 
				linkedFile: {path:this.path},
				cursors: Object.values(this.cursors)
			}
		}), session)
		
	}
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
