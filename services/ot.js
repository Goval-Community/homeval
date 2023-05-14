class Service extends ServiceBase {
	constructor(...args) {
		super(...args);
		this.version = 1
		this.contents = ""
		this.path = null
		this.cursors = {}
		this.history = []
		this.session_info = {}
	}

	async recv(cmd, session) {
		console.log(session)
		if (!this.path && !cmd.otLinkFile) {
			console.error("Command sent before otLinkFile", cmd)
			return
		}

		if (cmd.otLinkFile) {
			const path = cmd.otLinkFile.file.path;
			const content = await fs.readFile(path);
			this.path = path
			this.contents = await Deno.core.ops.op_read_file_string(path);

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
				userId: 20567961
			})

			return api.Command.create({otLinkFileResponse:{version:this.version, linkedFile:{path, content}}})
		} else if (cmd.ot) {
			let cursor = 0
			let contents = this.contents.toString()

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
					contents = contents.slice(0, cursor) + insert + contents.slice(cursor)
					cursor += insert.length
				}
				if (op.delete) {
					const del = op.delete
					if (del + cursor > contents.length) {
						throw new Error("Invalid delete past bounds")
					}

					contents = contents.slice(0,cursor) + contents.slice(cursor + del)
				}
			}
			this.version += 1
			this.contents = contents

			console.log(this.session_info, session)

			const inner_packet = {
				spookyVersion: this.version,
				op: cmd.ot.op,
				crc32: CRC32.str(contents),
				committed: {
					seconds: (Date.now()/ 1000n).toString(),
					nanos: 0
				},
				version: this.version,
				author: cmd.ot.author,
				// use https://replit.com/@ghostwriterai if ghostwriter wrote it (for history)
				userId: cmd.ot.author !== api.OTPacket.Author.GHOSTWRITER ? this.session_info[session].id : 22261053
			}

			this.history.push(inner_packet)

			const msg = api.Command.create({
				ot: inner_packet,
				ref: cmd.ref
			})

			await this.send(msg,session)

			for (const arr_session of this.clients) {
				if (arr_session === session) {continue}
				await this.send(msg, arr_session)
			}

			await Deno.core.ops.op_write_file_string(this.path, contents)
		} else if (cmd.otNewCursor) {
			const cursor = cmd.otNewCursor
			this.cursors[cursor.id] = cursor

			const msg = api.Command.create({otNewCursor: cursor, session:-session})

			for (const arr_session of this.clients) {
				if (arr_session === session) {continue}
				await this._send(msg, arr_session)
			}
		} else if (cmd.otDeleteCursor) {
			delete this.cursors[cmd.otDeleteCursor]

			const msg = api.Command.create({otDeleteCursor: cmd.otDeleteCursor, session: -session})

			for (const arr_session of this.clients) {
				if (arr_session === session) {continue}
				await this._send(msg, arr_session)
			}
		} else if (cmd.otFetchRequest) {
			console.log(cmd, this.history)
			return api.Command.create({
				otFetchResponse: {
					packets: this.history.slice(
						cmd.otFetchRequest.versionFrom - 1,
						cmd.otFetchRequest.versionTo + 1
					)
				}
			})
		} else {
			console.warn("Unknown ot command", cmd)
		}

		// console.log(cmd)
	}

	async _send(cmd, session) {
		cmd.channel = this.id;
		const buf = [...Buffer.from(api.Command.encode(cmd).finish())];
		await Deno.core.ops.op_send_msg({
			bytes: buf,
			session: session,
		});
	}

	async attach(session) {
		this.session_info[session] = Deno.core.ops.op_user_info(session)
		
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

console.log(serviceInfo);
const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
